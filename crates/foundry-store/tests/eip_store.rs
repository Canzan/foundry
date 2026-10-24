//! external-integration-plugins — store gold tests (DESIGN [REF] Driven Ports
//! and Contract Shapes, ADR-EIP-002; DEVOPS eip-store, C-9, C-12, U-D7, U-D12).
//! Pending stubs written by DISTILL 2026-10-21 (ADR-025 scaffolded RED): each
//! is `#[ignore]`d with its DELIVER step id and panics with the RED marker until
//! DELIVER replaces its body (testcontainers Postgres, like the neighbouring
//! store tests; Docker required once un-ignored).
//!
//! DELIVER 01-02 replaced the bodies of the migration, dispatch, claim and
//! live-project-lock stubs. Every test runs on its own container with the pool
//! pinned to a non-`public` schema, so the search_path-scoped advisory locks
//! (U-D2) are the ones exercised. The dispatch plan and the claim choice are
//! supplied by the test as closures (the store's contract: it applies a plan, it
//! never makes one); the pure planners arrive in the core crate (01-04).

use foundry_store::integrations::{
    ClaimOutcome, ClaimView, DeliveryResult, DeliveryTarget, DispatchBatch, DispatchOutcome,
    NewDelivery, NewRoute, RecordOutcome, RouteLane, RouteWriteOutcome,
};
use foundry_store::{
    run_boot_migrations_from_dir, run_migrations, run_migrations_from_dir, Store, StoreError,
};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Connection, PgPool};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};
use time::OffsetDateTime;

macro_rules! pending {
    ($name:ident, $step:literal, $what:literal) => {
        #[test]
        #[ignore = concat!("pending: external-integration-plugins ", $step, " ", $what)]
        fn $name() {
            panic!(concat!(
                "Not yet implemented -- RED scaffold (external-integration-plugins ",
                $step,
                ": ",
                $what,
                ")"
            ));
        }
    };
}

type Id = uuid::Uuid;

/// U-D12: 0028's `lock_timeout`.
const LOCK_TIMEOUT: Duration = Duration::from_secs(2);
/// DDR-5: the give-up window.
const GIVE_UP: Duration = Duration::from_secs(24 * 3600);
/// DDR-4: the default lease.
const LEASE: Duration = Duration::from_secs(45);
const SCHEMA: &str = "eip";
const INDEX_0029: &str = "idx_outbox_undispatched";

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

struct Pg {
    base: String,
    _c: ContainerAsync<Postgres>,
}

async fn postgres() -> Pg {
    let c = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("start postgres container");
    let host = c.get_host().await.expect("container host");
    let port = c.get_host_port_ipv4(5432).await.expect("container port");
    let base = format!("postgres://postgres:postgres@{host}:{port}/postgres");
    let mut admin = sqlx::PgConnection::connect(&base)
        .await
        .expect("admin connect");
    sqlx::query(&format!("CREATE SCHEMA {SCHEMA}"))
        .execute(&mut admin)
        .await
        .expect("create schema");
    Pg { base, _c: c }
}

impl Pg {
    /// A pool pinned to the test schema, with extra session settings.
    async fn pool_with(&self, settings: &[(&str, &str)]) -> PgPool {
        let mut options = vec![("search_path", SCHEMA)];
        options.extend_from_slice(settings);
        let opts = PgConnectOptions::from_str(&self.base)
            .expect("parse url")
            .options(options);
        PgPoolOptions::new()
            .max_connections(10)
            .acquire_timeout(Duration::from_secs(20))
            .connect_with(opts)
            .await
            .expect("connect pool")
    }

    async fn pool(&self) -> PgPool {
        self.pool_with(&[]).await
    }

    /// A pool pinned to another schema of the same database, created here.
    async fn pool_in(&self, schema: &str) -> PgPool {
        let mut admin = sqlx::PgConnection::connect(&self.base)
            .await
            .expect("admin connect");
        sqlx::query(&format!("CREATE SCHEMA {schema}"))
            .execute(&mut admin)
            .await
            .expect("create schema");
        let opts = PgConnectOptions::from_str(&self.base)
            .expect("parse url")
            .options([("search_path", schema)]);
        PgPoolOptions::new()
            .max_connections(4)
            .acquire_timeout(Duration::from_secs(20))
            .connect_with(opts)
            .await
            .expect("connect pool")
    }
}

fn production_migrations_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations")
}

/// A private copy of the production migrations up to and including `through`.
fn staged(through: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("foundry-eip-{}", Id::now_v7()));
    std::fs::create_dir_all(&dir).expect("stage dir");
    let mut copied = 0;
    for entry in std::fs::read_dir(production_migrations_dir()).expect("read migrations") {
        let name = entry
            .expect("entry")
            .file_name()
            .to_string_lossy()
            .into_owned();
        if name.ends_with(".sql") && name.get(..4).is_some_and(|v| v <= through) {
            std::fs::copy(production_migrations_dir().join(&name), dir.join(&name))
                .expect("stage file");
            copied += 1;
        }
    }
    assert!(copied > 0, "nothing staged through {through}");
    dir
}

async fn exec(pool: &PgPool, sql: &str) {
    sqlx::raw_sql(sql).execute(pool).await.expect(sql);
}

/// The SQL every shipped (and every pre-0028) outbox writer runs.
const OLD_WRITER: &str =
    "INSERT INTO outbox (event_type, payload) VALUES ('IssueCreated', $1) RETURNING id";

fn payload(project: Option<Id>) -> serde_json::Value {
    match project {
        Some(p) => serde_json::json!({ "project_id": p.to_string(), "key": "AUTH-1" }),
        None => serde_json::json!({}),
    }
}

async fn old_write<'c, E>(executor: E, project: Option<Id>) -> i64
where
    E: sqlx::PgExecutor<'c>,
{
    sqlx::query_scalar(OLD_WRITER)
        .bind(payload(project))
        .fetch_one(executor)
        .await
        .expect("old writer insert")
}

fn sqlstate(err: &StoreError) -> Option<String> {
    let db = match err {
        StoreError::MigrationFailed(sqlx::migrate::MigrateError::ExecuteMigration(e, _)) => e,
        StoreError::Sqlx(e) => e,
        _ => return None,
    };
    db.as_database_error()
        .and_then(|d| d.code().map(|c| c.into_owned()))
}

async fn has_column(pool: &PgPool, table: &str, column: &str) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM information_schema.columns
                         WHERE table_schema = current_schema()
                           AND table_name = $1 AND column_name = $2)",
    )
    .bind(table)
    .bind(column)
    .fetch_one(pool)
    .await
    .expect("column lookup")
}

/// Canzan Labs: one workspace, Lena, the Backend team, the Identity Platform
/// project (AUTH, with its default lanes), a connected Slack workspace and one
/// route of it to #auth-dev for New issue.
struct Lab {
    store: Store,
    pool: PgPool,
    ws: Id,
    team: Id,
    lena: Id,
    auth: Id,
    connection: Id,
    route: Id,
}

async fn lab(pool: &PgPool) -> Lab {
    let store = Store::from_pool(pool.clone());
    let ws = Id::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'Canzan Labs')")
        .bind(ws)
        .execute(pool)
        .await
        .expect("workspace");
    let lena = Id::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, 'lena@canzan.dev', 'lena@canzan.dev', 'Lena', 'not-a-real-hash')",
    )
    .bind(lena)
    .execute(pool)
    .await
    .expect("user");
    let team = Id::now_v7();
    sqlx::query(
        "INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'Backend', 'backend')",
    )
    .bind(team)
    .bind(ws)
    .execute(pool)
    .await
    .expect("team");
    let auth = Id::now_v7();
    store
        .insert_project(auth, ws, team, "Identity Platform", "auth", "AUTH")
        .await
        .expect("project");
    let connection = Id::now_v7();
    sqlx::query(
        "INSERT INTO integration_connections
                (id, workspace_id, kind, state, external_id, label,
                 sealed_credential, credential_saved_at, connected_by)
         VALUES ($1, $2, 'slack', 'connected', 'T0CANZAN', 'Canzan Labs',
                 '\\x00'::bytea, now(), $3)",
    )
    .bind(connection)
    .bind(ws)
    .bind(lena)
    .execute(pool)
    .await
    .expect("connection");
    let mut lab = Lab {
        store,
        pool: pool.clone(),
        ws,
        team,
        lena,
        auth,
        connection,
        route: Id::nil(),
    };
    lab.route = match lab.store.create_route(&lab.route_spec("C0AUTHDEV")).await {
        Ok(RouteWriteOutcome::Created(id)) => id,
        other => panic!("the route is created: {other:?}"),
    };
    lab
}

impl Lab {
    fn route_spec(&self, destination: &str) -> NewRoute {
        NewRoute {
            workspace_id: self.ws,
            project_id: self.auth,
            connection_id: Some(self.connection),
            kind: "slack".to_string(),
            destination_ref: destination.to_string(),
            destination_label: "#auth-dev".to_string(),
            events: vec!["issue.created".to_string(), "issue.moved".to_string()],
            content_level: "summary".to_string(),
            shared_confirmed: false,
            lanes: vec![RouteLane {
                lane_id: Id::now_v7(),
                label_at_save: "Done".to_string(),
            }],
            created_by: self.lena,
            created_via: "web".to_string(),
            created_at: time::OffsetDateTime::now_utc(),
        }
    }

    /// One pending delivery of a fresh event to the lab's route, through the
    /// real dispatch; returns its id.
    async fn one_delivery(&self) -> i64 {
        old_write(&self.pool, Some(self.auth)).await;
        let outcome = self
            .store
            .dispatch_tick(
                100,
                OffsetDateTime::now_utc(),
                GIVE_UP,
                every_matching_route,
            )
            .await
            .expect("dispatch");
        assert!(
            matches!(outcome, DispatchOutcome::Dispatched { deliveries: 1, .. }),
            "{outcome:?}"
        );
        sqlx::query_scalar("SELECT max(id) FROM integration_deliveries WHERE route_id = $1")
            .bind(self.route)
            .fetch_one(&self.pool)
            .await
            .expect("delivery id")
    }

    async fn delivery(&self, id: i64) -> DeliveryRow {
        sqlx::query_as(
            "SELECT status, attempts, claim_id, lease_until, first_attempt_at,
                    next_attempt_at, last_vendor_reason, final_reason, finished_at
               FROM integration_deliveries WHERE id = $1",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .expect("delivery row")
    }

    async fn pacing(&self, destination: &str) -> Option<OffsetDateTime> {
        sqlx::query_scalar(
            "SELECT not_before FROM integration_destination_pacing
              WHERE connection_id = $1 AND destination_ref = $2",
        )
        .bind(self.connection)
        .bind(destination)
        .fetch_optional(&self.pool)
        .await
        .expect("pacing row")
    }
}

#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
struct DeliveryRow {
    status: String,
    attempts: i32,
    claim_id: Option<Id>,
    lease_until: Option<OffsetDateTime>,
    first_attempt_at: Option<OffsetDateTime>,
    next_attempt_at: Option<OffsetDateTime>,
    last_vendor_reason: Option<String>,
    final_reason: Option<String>,
    finished_at: Option<OffsetDateTime>,
}

/// The test's dispatch plan: every event to every route of its project.
fn every_matching_route(batch: &DispatchBatch) -> Vec<NewDelivery> {
    batch
        .events
        .iter()
        .flat_map(|event| {
            batch
                .routes
                .iter()
                .filter(move |r| {
                    event.payload["project_id"].as_str() == Some(r.project_id.to_string().as_str())
                })
                .map(move |r| NewDelivery {
                    workspace_id: r.workspace_id,
                    project_id: r.project_id,
                    target: DeliveryTarget::Route(r.id),
                    event_kind: "issue.created".to_string(),
                    event_id: event.event_id,
                    outbox_ids: vec![event.id],
                    issue_id: None,
                    issue_key: event.payload["key"].as_str().map(str::to_string),
                    occurred_at: event.created_at,
                })
        })
        .collect()
}

/// The test's claim choice: the head, if it holds no live lease.
fn head_if_free(now: OffsetDateTime) -> impl FnOnce(&ClaimView) -> Option<i64> {
    move |view| {
        view.deliveries
            .first()
            .filter(|d| d.lease_until.is_none_or(|l| l <= now))
            .map(|d| d.id)
    }
}

async fn undispatched(pool: &PgPool) -> Vec<i64> {
    sqlx::query_scalar("SELECT id FROM outbox WHERE dispatched_at IS NULL ORDER BY id")
        .fetch_all(pool)
        .await
        .expect("undispatched rows")
}

async fn delivery_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM integration_deliveries")
        .fetch_one(pool)
        .await
        .expect("count deliveries")
}

/// Old-writer inserts in a loop until `stop`; returns the worst single-insert
/// latency and the number of failed inserts.
fn writers(
    pool: &PgPool,
    n: usize,
    stop: &Arc<AtomicBool>,
) -> Vec<tokio::task::JoinHandle<(Duration, usize, usize)>> {
    (0..n)
        .map(|_| {
            let pool = pool.clone();
            let stop = stop.clone();
            tokio::spawn(async move {
                let (mut worst, mut ok, mut failed) = (Duration::ZERO, 0, 0);
                while !stop.load(Ordering::SeqCst) {
                    let started = Instant::now();
                    match sqlx::query(OLD_WRITER)
                        .bind(payload(None))
                        .execute(&pool)
                        .await
                    {
                        Ok(_) => ok += 1,
                        Err(_) => failed += 1,
                    }
                    worst = worst.max(started.elapsed());
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                (worst, ok, failed)
            })
        })
        .collect()
}

async fn join_writers(
    handles: Vec<tokio::task::JoinHandle<(Duration, usize, usize)>>,
) -> (Duration, usize, usize) {
    let mut total = (Duration::ZERO, 0, 0);
    for h in handles {
        let (worst, ok, failed) = h.await.expect("writer task");
        total = (total.0.max(worst), total.1 + ok, total.2 + failed);
    }
    total
}

// ---------------------------------------------------------------------------
// Migrations 0028-0030 under rolling upgrade (slice 01).
// ---------------------------------------------------------------------------

/// U-D12: old-writer inserts keep flowing while 0028 applies. A writer
/// transaction held past `lock_timeout` makes 0028 give up (the pod would
/// restart and retry) instead of stalling the board behind its lock request;
/// the retry then applies. The worst stall is at most `lock_timeout` + 1 s.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn board_writes_continue_while_0028_applies() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations_from_dir(&pool, &staged("0027"))
        .await
        .expect("stand at 0027");
    let bound = LOCK_TIMEOUT + Duration::from_secs(1);

    // Attempt 1: a long board transaction holds the outbox past lock_timeout.
    let mut long_write = pool.begin().await.expect("long write");
    old_write(&mut *long_write, None).await;
    let stop = Arc::new(AtomicBool::new(false));
    let running = writers(&pool, 3, &stop);
    tokio::time::sleep(Duration::from_millis(300)).await;
    let refused = run_boot_migrations_from_dir(&pool, &production_migrations_dir())
        .await
        .expect_err("0028 gives up after lock_timeout while a writer holds the outbox");
    tokio::time::sleep(Duration::from_millis(300)).await;
    stop.store(true, Ordering::SeqCst);
    let (worst, ok, failed) = join_writers(running).await;
    long_write.commit().await.expect("long write commits");
    assert_eq!(
        sqlstate(&refused).as_deref(),
        Some("55P03"),
        "lock_not_available: {refused}"
    );
    assert!(
        !has_column(&pool, "outbox", "event_id").await,
        "0028 rolled back whole"
    );
    assert_eq!(failed, 0, "no board write fails");
    assert!(ok > 0, "board writes ran during the attempt");
    assert!(
        worst >= Duration::from_secs(1),
        "the writers queued behind 0028's lock request (worst {worst:?})"
    );
    assert!(
        worst <= bound,
        "worst stall {worst:?} exceeds lock_timeout + 1 s"
    );

    // Attempt 2 (the restarted pod): applies with board writes running.
    let stop = Arc::new(AtomicBool::new(false));
    let running = writers(&pool, 3, &stop);
    tokio::time::sleep(Duration::from_millis(200)).await;
    run_boot_migrations_from_dir(&pool, &production_migrations_dir())
        .await
        .expect("the retry applies 0028-0030");
    tokio::time::sleep(Duration::from_millis(300)).await;
    stop.store(true, Ordering::SeqCst);
    let (worst, ok, failed) = join_writers(running).await;
    assert_eq!(failed, 0, "no board write fails");
    assert!(ok > 0, "board writes ran during the upgrade");
    assert!(
        worst <= bound,
        "worst stall {worst:?} exceeds lock_timeout + 1 s"
    );
    let (with_event, newest_has_event): (i64, bool) = sqlx::query_as(
        "SELECT count(*) FILTER (WHERE event_id IS NOT NULL),
                bool_and(event_id IS NOT NULL) FILTER (WHERE id > (SELECT outbox_id FROM integration_dispatch_start))
           FROM outbox",
    )
    .fetch_one(&pool)
    .await
    .expect("outbox after the upgrade");
    assert!(
        with_event > 0 && newest_has_event,
        "every write after 0028 carries an event id"
    );
}

/// Rows at or below `integration_dispatch_start`, and any row with a NULL
/// `event_id`, never become deliveries; a fresh row does.
#[tokio::test]
async fn rows_from_before_0028_are_never_dispatched() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations_from_dir(&pool, &staged("0027"))
        .await
        .expect("stand at 0027");
    let mut before = Vec::new();
    for _ in 0..3 {
        before.push(old_write(&pool, None).await);
    }
    run_migrations(&pool).await.expect("0028-0030 apply");
    let lab = lab(&pool).await;
    // Pre-0028 rows naming the routed project, as an old payload would.
    sqlx::query("UPDATE outbox SET payload = $1 WHERE id = ANY($2)")
        .bind(payload(Some(lab.auth)))
        .bind(&before)
        .execute(&pool)
        .await
        .expect("name the project");
    let start: i64 = sqlx::query_scalar("SELECT outbox_id FROM integration_dispatch_start")
        .fetch_one(&pool)
        .await
        .expect("dispatch start");
    assert_eq!(
        start,
        *before.last().unwrap(),
        "the dispatch start is the highest id at 0028"
    );
    let null_event: i64 = sqlx::query_scalar(
        "INSERT INTO outbox (event_type, payload, event_id) VALUES ('IssueCreated', $1, NULL) RETURNING id",
    )
    .bind(payload(Some(lab.auth)))
    .fetch_one(&pool)
    .await
    .expect("a row without an event id");
    let fresh = old_write(&pool, Some(lab.auth)).await;

    let mut seen = Vec::new();
    let outcome = lab
        .store
        .dispatch_tick(100, OffsetDateTime::now_utc(), GIVE_UP, |batch| {
            seen = batch.events.iter().map(|e| e.id).collect();
            every_matching_route(batch)
        })
        .await
        .expect("dispatch");
    assert_eq!(seen, vec![fresh], "only the fresh row is read");
    assert!(
        matches!(
            outcome,
            DispatchOutcome::Dispatched {
                events: 1,
                deliveries: 1
            }
        ),
        "{outcome:?}"
    );
    let delivered: Vec<Vec<i64>> =
        sqlx::query_scalar("SELECT outbox_ids FROM integration_deliveries")
            .fetch_all(&pool)
            .await
            .expect("deliveries");
    assert_eq!(delivered, vec![vec![fresh]]);
    let mut never = before.clone();
    never.push(null_event);
    assert_eq!(
        undispatched(&pool).await,
        never,
        "the old and NULL-event rows stay undispatched"
    );

    let again = lab
        .store
        .dispatch_tick(
            100,
            OffsetDateTime::now_utc(),
            GIVE_UP,
            every_matching_route,
        )
        .await
        .expect("dispatch again");
    assert!(
        matches!(
            again,
            DispatchOutcome::Dispatched {
                events: 0,
                deliveries: 0
            }
        ),
        "{again:?}"
    );
}

/// C-12: a pre-0028 writer's row written after 0028 carries `event_id` and
/// `source_txid` and is dispatched; R-6's UPDATE marks exactly the
/// undispatched rows with an event id, and none of the pre-0028 rows.
#[tokio::test]
async fn old_writer_rows_after_0028_get_event_and_txid() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations_from_dir(&pool, &staged("0027"))
        .await
        .expect("stand at 0027");
    let pre = old_write(&pool, None).await;
    run_migrations(&pool).await.expect("0028-0030 apply");
    let lab = lab(&pool).await;

    let first = old_write(&pool, Some(lab.auth)).await;
    let second = old_write(&pool, Some(lab.auth)).await;
    let stamped: Vec<(i64, bool, bool)> = sqlx::query_as(
        "SELECT id, event_id IS NOT NULL, source_txid IS NOT NULL FROM outbox ORDER BY id",
    )
    .fetch_all(&pool)
    .await
    .expect("stamps");
    assert_eq!(
        stamped,
        vec![
            (pre, false, false),
            (first, true, true),
            (second, true, true)
        ],
        "only the rows written after 0028 carry the defaults"
    );
    let txids: i64 =
        sqlx::query_scalar("SELECT count(DISTINCT source_txid) FROM outbox WHERE id = ANY($1)")
            .bind(vec![first, second])
            .fetch_one(&pool)
            .await
            .expect("txids");
    assert_eq!(txids, 2, "each writer transaction stamps its own id");
    let outcome = lab
        .store
        .dispatch_tick(
            100,
            OffsetDateTime::now_utc(),
            GIVE_UP,
            every_matching_route,
        )
        .await
        .expect("dispatch");
    assert!(
        matches!(
            outcome,
            DispatchOutcome::Dispatched {
                events: 2,
                deliveries: 2
            }
        ),
        "{outcome:?}"
    );

    // A rollback window: more old-writer rows, then R-6 before rolling forward.
    let third = old_write(&pool, Some(lab.auth)).await;
    let fourth = old_write(&pool, Some(lab.auth)).await;
    let mut marked: Vec<i64> = sqlx::query_scalar(
        "UPDATE outbox SET dispatched_at = now()
          WHERE dispatched_at IS NULL AND event_id IS NOT NULL
            AND created_at < now() - interval '0 seconds'
         RETURNING id",
    )
    .fetch_all(&pool)
    .await
    .expect("R-6");
    marked.sort();
    assert_eq!(
        marked,
        vec![third, fourth],
        "R-6 marks exactly the rollback-window rows"
    );
    assert_eq!(
        undispatched(&pool).await,
        vec![pre],
        "the pre-0028 row is untouched"
    );
    let after = lab
        .store
        .dispatch_tick(
            100,
            OffsetDateTime::now_utc(),
            GIVE_UP,
            every_matching_route,
        )
        .await
        .expect("dispatch after R-6");
    assert!(
        matches!(
            after,
            DispatchOutcome::Dispatched {
                events: 0,
                deliveries: 0
            }
        ),
        "{after:?}"
    );
    assert_eq!(delivery_count(&pool).await, 2);
}

pending!(
    previous_release_migrator_accepts_0028_to_0031,
    "01-02",
    "C-9: the rollback floor; an older binary's Store::migrate boots on the newer schema"
);

/// `Store::probe` refuses readiness until the 0028 columns, the 0029 index and
/// the 0030 tables exist, naming the missing migration.
#[tokio::test]
async fn probe_refuses_a_schema_without_0028_to_0030() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    let store = Store::from_pool(pool.clone());
    for (through, missing) in [("0027", "0028"), ("0028", "0029"), ("0029", "0030")] {
        run_migrations_from_dir(&pool, &staged(through))
            .await
            .unwrap_or_else(|e| panic!("stand at {through}: {e}"));
        let err = store
            .probe()
            .await
            .expect_err("not ready without the integration schema");
        assert!(
            err.to_string().contains(&format!("migration-{missing}")),
            "at {through} the probe names {missing}: {err}"
        );
    }
    run_migrations(&pool).await.expect("head");
    store.probe().await.expect("ready at 0030");
}

/// U-D7, reproduced: a `CREATE INDEX CONCURRENTLY` cancelled while it waits
/// for a board transaction leaves an INVALID index and no migration row; the
/// next boot refuses with "already exists"; the store reports the invalid
/// index; `DROP INDEX CONCURRENTLY` and a restart recover (R-7).
#[tokio::test]
async fn invalid_0029_index_is_reported_at_boot() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    let store = Store::from_pool(pool.clone());
    run_migrations_from_dir(&pool, &staged("0028"))
        .await
        .expect("stand at 0028");
    assert!(store
        .invalid_index_warnings()
        .await
        .expect("warnings")
        .is_empty());

    // The failure: the CIC waits for an open board transaction and is cancelled.
    let impatient = pg.pool_with(&[("statement_timeout", "1500")]).await;
    let mut board = pool.begin().await.expect("board transaction");
    old_write(&mut *board, None).await;
    let failed = run_boot_migrations_from_dir(&impatient, &staged("0029"))
        .await
        .expect_err("the cancelled CIC fails the boot");
    board.commit().await.expect("board commits");
    assert_eq!(
        sqlstate(&failed).as_deref(),
        Some("57014"),
        "query_canceled: {failed}"
    );

    // Observed state: an INVALID index, and no row (not a dirty one) for 0029.
    let valid: Option<bool> = sqlx::query_scalar(
        "SELECT i.indisvalid FROM pg_index i
           JOIN pg_class c ON c.oid = i.indexrelid
           JOIN pg_namespace n ON n.oid = c.relnamespace
          WHERE n.nspname = current_schema() AND c.relname = $1",
    )
    .bind(INDEX_0029)
    .fetch_optional(&pool)
    .await
    .expect("index lookup");
    assert_eq!(
        valid,
        Some(false),
        "the failed CIC leaves the index INVALID"
    );
    let rows: Vec<(i64, bool)> =
        sqlx::query_as("SELECT version, success FROM _sqlx_migrations WHERE version >= 29")
            .fetch_all(&pool)
            .await
            .expect("migration rows");
    assert_eq!(
        rows,
        vec![],
        "sqlx records nothing for a failed no-transaction migration"
    );

    // Boot reports it.
    let warnings = store.invalid_index_warnings().await.expect("warnings");
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(
        warnings[0].contains(INDEX_0029) && warnings[0].contains("DROP INDEX CONCURRENTLY"),
        "the warning names the index and the remedy: {}",
        warnings[0]
    );

    // The next boot (no blocker now; the production `Store::migrate`, which
    // also logs the warning) refuses: the invalid index already exists.
    let again = store
        .migrate()
        .await
        .expect_err("a restart cannot re-create the index over the invalid one");
    assert_eq!(
        sqlstate(&again).as_deref(),
        Some("42P07"),
        "duplicate_table: {again}"
    );

    // R-7: drop it concurrently and restart.
    exec(&pool, &format!("DROP INDEX CONCURRENTLY {INDEX_0029}")).await;
    store
        .migrate()
        .await
        .expect("the restart after the drop applies 0029 and 0030");
    assert!(store
        .invalid_index_warnings()
        .await
        .expect("warnings")
        .is_empty());
    store.probe().await.expect("ready");
}

// ---------------------------------------------------------------------------
// Dispatch and claims (slice 01).
// ---------------------------------------------------------------------------

/// SC-2: an outbox row from an aborted board transaction is never dispatched,
/// whether the dispatcher ticks while it is in flight or after it aborts.
#[tokio::test]
async fn rolled_back_board_transaction_yields_no_delivery() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations(&pool).await.expect("head");
    let lab = lab(&pool).await;

    let mut board = pool.begin().await.expect("board transaction");
    let aborted = old_write(&mut *board, Some(lab.auth)).await;
    let mut read = Vec::new();
    lab.store
        .dispatch_tick(100, OffsetDateTime::now_utc(), GIVE_UP, |batch| {
            read.extend(batch.events.iter().map(|e| e.id));
            every_matching_route(batch)
        })
        .await
        .expect("tick during the transaction");
    board.rollback().await.expect("the board action aborts");
    lab.store
        .dispatch_tick(100, OffsetDateTime::now_utc(), GIVE_UP, |batch| {
            read.extend(batch.events.iter().map(|e| e.id));
            every_matching_route(batch)
        })
        .await
        .expect("tick after the abort");
    assert!(!read.contains(&aborted), "the aborted row is never read");
    assert_eq!(delivery_count(&pool).await, 0, "no delivery");

    // Positive control: a committed write is dispatched.
    let committed = old_write(&pool, Some(lab.auth)).await;
    lab.store
        .dispatch_tick(
            100,
            OffsetDateTime::now_utc(),
            GIVE_UP,
            every_matching_route,
        )
        .await
        .expect("tick");
    let delivered: Vec<Vec<i64>> =
        sqlx::query_scalar("SELECT outbox_ids FROM integration_deliveries")
            .fetch_all(&pool)
            .await
            .expect("deliveries");
    assert_eq!(delivered, vec![vec![committed]]);
}

/// A second dispatcher tick over the same rows inserts nothing and changes no
/// `dispatched_at`; the first tick wrote the plan's rows with their give-up. A
/// second dispatcher ticking while the first holds the lock (another replica)
/// is `Busy` and reads nothing, while a dispatcher on another schema is not
/// held up by it (the lock is scoped by `search_path`, U-D2).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dispatch_rerun_is_a_no_op() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations(&pool).await.expect("head");
    let lab = lab(&pool).await;
    let sibling_pool = pg.pool_in("eip_sibling").await;
    run_migrations(&sibling_pool).await.expect("sibling head");
    let sibling = Store::from_pool(sibling_pool);
    let first = old_write(&pool, Some(lab.auth)).await;
    let second = old_write(&pool, Some(lab.auth)).await;
    let unrouted = old_write(&pool, Some(Id::now_v7())).await;

    let (mut replica, mut other_schema) = (None, None);
    let outcome = lab
        .store
        .dispatch_tick(100, OffsetDateTime::now_utc(), GIVE_UP, |batch| {
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    replica = Some(
                        lab.store
                            .dispatch_tick(100, OffsetDateTime::now_utc(), GIVE_UP, |_| {
                                panic!("a busy dispatcher plans nothing")
                            })
                            .await
                            .expect("replica tick"),
                    );
                    other_schema = Some(
                        sibling
                            .dispatch_tick(
                                100,
                                OffsetDateTime::now_utc(),
                                GIVE_UP,
                                every_matching_route,
                            )
                            .await
                            .expect("sibling tick"),
                    );
                })
            });
            every_matching_route(batch)
        })
        .await
        .expect("first tick");
    assert_eq!(
        replica,
        Some(DispatchOutcome::Busy),
        "one active dispatcher"
    );
    assert_eq!(
        other_schema,
        Some(DispatchOutcome::Dispatched {
            events: 0,
            deliveries: 0
        }),
        "another schema's dispatcher is not held"
    );
    assert!(
        matches!(
            outcome,
            DispatchOutcome::Dispatched {
                events: 3,
                deliveries: 2
            }
        ),
        "{outcome:?}"
    );
    assert_eq!(
        undispatched(&pool).await,
        Vec::<i64>::new(),
        "every read row is marked, routed or not"
    );

    #[derive(Debug, PartialEq, sqlx::FromRow)]
    struct Written {
        workspace_id: Id,
        project_id: Id,
        route_id: Option<Id>,
        recipient_user_id: Option<Id>,
        event_kind: String,
        event_id: Id,
        outbox_ids: Vec<i64>,
        issue_key: Option<String>,
        occurred_matches: bool,
        give_up_is_dispatch_plus_window: bool,
        status: String,
        attempts: i32,
    }
    let written: Vec<Written> = sqlx::query_as(
        "SELECT d.workspace_id, d.project_id, d.route_id, d.recipient_user_id, d.event_kind,
                d.event_id, d.outbox_ids, d.issue_key,
                d.occurred_at = o.created_at AS occurred_matches,
                d.give_up_at = o.dispatched_at + interval '24 hours' AS give_up_is_dispatch_plus_window,
                d.status, d.attempts
           FROM integration_deliveries d JOIN outbox o ON o.id = d.outbox_ids[1]
          ORDER BY d.id",
    )
    .fetch_all(&pool)
    .await
    .expect("written deliveries");
    let event_ids: Vec<Id> =
        sqlx::query_scalar("SELECT event_id FROM outbox WHERE id = ANY($1) ORDER BY id")
            .bind(vec![first, second])
            .fetch_all(&pool)
            .await
            .expect("event ids");
    let expected: Vec<Written> = [first, second]
        .iter()
        .zip(event_ids)
        .map(|(id, event_id)| Written {
            workspace_id: lab.ws,
            project_id: lab.auth,
            route_id: Some(lab.route),
            recipient_user_id: None,
            event_kind: "issue.created".to_string(),
            event_id,
            outbox_ids: vec![*id],
            issue_key: Some("AUTH-1".to_string()),
            occurred_matches: true,
            give_up_is_dispatch_plus_window: true,
            status: "pending".to_string(),
            attempts: 0,
        })
        .collect();
    assert_eq!(written, expected);
    let _ = unrouted;

    let marks_before: Vec<(i64, OffsetDateTime)> =
        sqlx::query_as("SELECT id, dispatched_at FROM outbox ORDER BY id")
            .fetch_all(&pool)
            .await
            .expect("marks");
    let mut read = 0;
    let rerun = lab
        .store
        .dispatch_tick(100, OffsetDateTime::now_utc(), GIVE_UP, |batch| {
            read = batch.events.len();
            every_matching_route(batch)
        })
        .await
        .expect("second tick");
    assert_eq!(read, 0, "nothing left to read");
    assert!(
        matches!(
            rerun,
            DispatchOutcome::Dispatched {
                events: 0,
                deliveries: 0
            }
        ),
        "{rerun:?}"
    );
    let marks_after: Vec<(i64, OffsetDateTime)> =
        sqlx::query_as("SELECT id, dispatched_at FROM outbox ORDER BY id")
            .fetch_all(&pool)
            .await
            .expect("marks");
    assert_eq!(marks_after, marks_before, "no dispatched_at changes");
    assert_eq!(delivery_count(&pool).await, 2, "no delivery inserted");
}

/// All rows of one `source_txid` are read and marked in one dispatch
/// transaction, however small the tick's transaction budget.
#[tokio::test]
async fn dispatch_never_splits_a_source_transaction() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations(&pool).await.expect("head");
    let lab = lab(&pool).await;
    let mut groups = Vec::new();
    for size in [3, 2] {
        let mut tx = pool.begin().await.expect("board transaction");
        let mut ids = Vec::new();
        for _ in 0..size {
            ids.push(old_write(&mut *tx, Some(lab.auth)).await);
        }
        tx.commit().await.expect("commit");
        groups.push(ids);
    }
    // Interleave a third transaction's row between nothing: ids stay grouped.
    let mut batches: Vec<Vec<(i64, u64)>> = Vec::new();
    for _ in 0..3 {
        lab.store
            .dispatch_tick(1, OffsetDateTime::now_utc(), GIVE_UP, |batch| {
                batches.push(batch.events.iter().map(|e| (e.id, e.source_txid)).collect());
                every_matching_route(batch)
            })
            .await
            .expect("tick");
    }
    let ids: Vec<Vec<i64>> = batches
        .iter()
        .map(|b| b.iter().map(|(id, _)| *id).collect())
        .collect();
    assert_eq!(
        ids,
        vec![groups[0].clone(), groups[1].clone(), vec![]],
        "one whole transaction per tick"
    );
    for batch in &batches[..2] {
        assert!(
            batch.windows(2).all(|w| w[0].1 == w[1].1),
            "a batch carries one source transaction: {batch:?}"
        );
    }
    assert_ne!(batches[0][0].1, batches[1][0].1);
    assert_eq!(delivery_count(&pool).await, 5);
}

/// `claim_delivery` applies the chosen id only if the row is as the choice saw
/// it: a sweep that ended the row meanwhile makes the claim miss and write
/// nothing; an unchanged row is claimed with a fresh lease and pacing.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn claim_compare_and_set_misses_a_changed_row() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations(&pool).await.expect("head");
    let lab = lab(&pool).await;
    let id = lab.one_delivery().await;
    let now = OffsetDateTime::now_utc().replace_nanosecond(0).unwrap();

    let sweeper = pool.clone();
    let missed = lab
        .store
        .claim_delivery(lab.route, now, LEASE, |view| {
            let chosen = head_if_free(now)(view);
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    sqlx::query(
                        "UPDATE integration_deliveries
                            SET status = 'failed', final_reason = 'gave_up_24h', finished_at = now()
                          WHERE id = $1",
                    )
                    .bind(id)
                    .execute(&sweeper)
                    .await
                    .expect("a sweep ends the row meanwhile");
                })
            });
            chosen
        })
        .await
        .expect("claim");
    assert_eq!(missed, ClaimOutcome::Missed);
    let row = lab.delivery(id).await;
    assert_eq!(
        (row.status.as_str(), row.attempts, row.claim_id),
        ("failed", 0, None)
    );
    assert_eq!(
        lab.pacing("C0AUTHDEV").await,
        None,
        "a missed claim paces nothing"
    );

    // Positive control: an unchanged head is claimed.
    let head = lab.one_delivery().await;
    let mut viewed = None;
    let claimed = lab
        .store
        .claim_delivery(lab.route, now, LEASE, |view| {
            viewed = Some(view.clone());
            head_if_free(now)(view)
        })
        .await
        .expect("claim");
    let view = viewed.expect("the choice saw the route");
    assert_eq!(view.route_id, lab.route);
    assert_eq!(view.connection_id, Some(lab.connection));
    assert_eq!(view.destination_ref, "C0AUTHDEV");
    assert_eq!(view.destination_not_before, None);
    assert_eq!(
        view.deliveries.iter().map(|d| d.id).collect::<Vec<_>>(),
        vec![head],
        "only pending rows"
    );
    let ClaimOutcome::Claimed(claim) = claimed else {
        panic!("the unchanged head is claimed: {claimed:?}");
    };
    assert_eq!((claim.id, claim.attempts), (head, 1));
    let row = lab.delivery(head).await;
    assert_eq!(row.status, "pending");
    assert_eq!(row.attempts, 1);
    assert_eq!(row.claim_id, Some(claim.claim_id));
    assert_eq!(row.lease_until, Some(now + LEASE));
    assert_eq!(claim.lease_until, now + LEASE);
    assert_eq!(row.first_attempt_at, Some(now));
    assert_eq!(
        lab.pacing("C0AUTHDEV").await,
        Some(now + Duration::from_secs(1)),
        "one attempt per destination per second"
    );

    // Nothing claimable: the choice declines while the lease lives.
    let declined = lab
        .store
        .claim_delivery(lab.route, now + Duration::from_secs(5), LEASE, |view| {
            assert_eq!(
                view.destination_not_before,
                Some(now + Duration::from_secs(1))
            );
            head_if_free(now + Duration::from_secs(5))(view)
        })
        .await
        .expect("claim");
    assert_eq!(declined, ClaimOutcome::NothingClaimable);
    assert_eq!(
        lab.delivery(head).await,
        row,
        "a declined claim writes nothing"
    );
}

/// A recorder whose lease was taken over (the claim id moved on) changes no
/// row; the current holder's record lands, once.
#[tokio::test]
async fn late_recorder_writes_nothing() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations(&pool).await.expect("head");
    let lab = lab(&pool).await;
    let id = lab.one_delivery().await;
    let t0 = OffsetDateTime::now_utc().replace_nanosecond(0).unwrap();

    let ClaimOutcome::Claimed(slow) = lab
        .store
        .claim_delivery(lab.route, t0, LEASE, head_if_free(t0))
        .await
        .expect("first claim")
    else {
        panic!("the first sender claims the head");
    };
    // The lease expires mid-send; another replica takes the head over.
    let t1 = t0 + Duration::from_secs(60);
    let ClaimOutcome::Claimed(fast) = lab
        .store
        .claim_delivery(lab.route, t1, LEASE, head_if_free(t1))
        .await
        .expect("takeover claim")
    else {
        panic!("an expired lease is taken over");
    };
    assert_eq!(fast.id, id);
    assert_ne!(fast.claim_id, slow.claim_id);
    assert_eq!(fast.attempts, 2);
    let taken_over = lab.delivery(id).await;

    let late = lab
        .store
        .record_delivery(
            id,
            slow.claim_id,
            t1 + Duration::from_secs(1),
            DeliveryResult::Failed {
                final_reason: "channel_not_found".to_string(),
                vendor_reason: Some("channel_not_found".to_string()),
            },
        )
        .await
        .expect("late record");
    assert_eq!(late, RecordOutcome::LeaseLost);
    assert_eq!(
        lab.delivery(id).await,
        taken_over,
        "the late recorder changed nothing"
    );

    let retry_at = t1 + Duration::from_secs(30);
    let retried = lab
        .store
        .record_delivery(
            id,
            fast.claim_id,
            t1 + Duration::from_secs(2),
            DeliveryResult::Retry {
                next_attempt_at: retry_at,
                vendor_reason: Some("ratelimited".to_string()),
            },
        )
        .await
        .expect("retry record");
    assert_eq!(retried, RecordOutcome::Recorded);
    let row = lab.delivery(id).await;
    assert_eq!(
        (
            row.status.as_str(),
            row.lease_until,
            row.next_attempt_at,
            row.last_vendor_reason.as_deref(),
            row.finished_at
        ),
        ("pending", None, Some(retry_at), Some("ratelimited"), None),
        "a retry releases the lease and schedules the next attempt"
    );

    let t2 = retry_at;
    let ClaimOutcome::Claimed(last) = lab
        .store
        .claim_delivery(lab.route, t2, LEASE, head_if_free(t2))
        .await
        .expect("third claim")
    else {
        panic!("the retried head is claimed again");
    };
    let done_at = t2 + Duration::from_secs(1);
    let delivered = lab
        .store
        .record_delivery(id, last.claim_id, done_at, DeliveryResult::Delivered)
        .await
        .expect("record");
    assert_eq!(delivered, RecordOutcome::Recorded);
    let row = lab.delivery(id).await;
    assert_eq!(
        (
            row.status.as_str(),
            row.attempts,
            row.lease_until,
            row.finished_at,
            row.final_reason.as_deref()
        ),
        ("delivered", 3, None, Some(done_at), None)
    );
    let twice = lab
        .store
        .record_delivery(
            id,
            last.claim_id,
            done_at + Duration::from_secs(1),
            DeliveryResult::Skipped {
                final_reason: "issue_deleted".to_string(),
            },
        )
        .await
        .expect("second record");
    assert_eq!(
        twice,
        RecordOutcome::LeaseLost,
        "a finished delivery is never rewritten"
    );
    assert_eq!(lab.delivery(id).await, row);
}

pending!(sweeps_apply_exactly_the_plan_ids, "03-01", "expire, cancel and prune apply one compare-and-set over exactly the ids their plan function returned");

/// Route writers open with `lock_live_project`: a delete in flight makes the
/// write wait and then refuse; a deleted project refuses at once.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn route_and_choice_writers_take_the_live_project_lock() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations(&pool).await.expect("head");
    let lab = lab(&pool).await;

    let (receives_from, lanes): (Option<OffsetDateTime>, i64) = sqlx::query_as(
        "SELECT r.receives_from, (SELECT count(*) FROM integration_route_lanes l WHERE l.route_id = r.id)
           FROM integration_routes r WHERE r.id = $1",
    )
    .bind(lab.route)
    .fetch_one(&pool)
    .await
    .expect("the created route");
    assert!(receives_from.is_some(), "receives_from is set at create");
    assert_eq!(lanes, 1, "the route's lane filter is saved with it");

    // A project delete in flight (the soft delete's lock and marker, uncommitted).
    let mut delete = pool.begin().await.expect("delete transaction");
    sqlx::query("SELECT 1 FROM live_projects WHERE id = $1 FOR UPDATE")
        .bind(lab.auth)
        .execute(&mut *delete)
        .await
        .expect("the delete locks the project");
    sqlx::query(
        "UPDATE projects SET deleted_at = clock_timestamp(), deleted_by = $2 WHERE id = $1",
    )
    .bind(lab.auth)
    .bind(lab.lena)
    .execute(&mut *delete)
    .await
    .expect("the delete marks the project");
    let store = lab.store.clone();
    let spec = lab.route_spec("C0RACING");
    let racing = tokio::spawn(async move { store.create_route(&spec).await });
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        !racing.is_finished(),
        "the route write waits for the delete"
    );
    delete.commit().await.expect("the delete commits");
    let outcome = racing.await.expect("join").expect("route write");
    assert_eq!(outcome, RouteWriteOutcome::ProjectDeleted);

    let refused = lab
        .store
        .create_route(&lab.route_spec("C0LATE"))
        .await
        .expect("route write");
    assert_eq!(refused, RouteWriteOutcome::ProjectDeleted);
    let routes: i64 = sqlx::query_scalar("SELECT count(*) FROM integration_routes")
        .fetch_one(&pool)
        .await
        .expect("routes");
    assert_eq!(routes, 1, "only the route saved before the delete exists");
    let _ = lab.team;
}

/// A stand-in for the secret box: `<key>|<row id>|<plaintext>`, so a value
/// opens only under its key and for its row (the row-bound AAD).
fn toy_seal(key: &str, row: Id, plaintext: &str) -> Vec<u8> {
    format!("{key}|{row}|{plaintext}").into_bytes()
}

fn toy_open(key: &str, row: Id, sealed: &[u8]) -> Option<String> {
    let text = std::str::from_utf8(sealed).ok()?;
    text.strip_prefix(&format!("{key}|{row}|"))
        .map(str::to_string)
}

async fn credentials(pool: &PgPool) -> Vec<(Id, Vec<u8>)> {
    sqlx::query_as(
        "SELECT id, sealed_credential FROM integration_connections
          WHERE sealed_credential IS NOT NULL ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .expect("credentials")
}

async fn key_check_rows(pool: &PgPool) -> Vec<(String, Vec<u8>)> {
    sqlx::query_as("SELECT key_id, sealed_canary FROM integration_key_checks ORDER BY key_id")
        .fetch_all(pool)
        .await
        .expect("key checks")
}

/// C-11: `doctor rotate-secrets` re-seals every credential under the new key
/// with its plaintext unchanged, adds no row, writes the new canary, and a
/// refusal on the last credential changes nothing at all.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rotate_secrets_reseals_every_credential_or_nothing() {
    use foundry_store::integrations::{KeyCheck, ResealOutcome};
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations(&pool).await.expect("head");
    let lab = lab(&pool).await;
    let matrix = Id::now_v7();
    sqlx::query(
        "INSERT INTO integration_connections
                (id, workspace_id, kind, state, external_id, label,
                 sealed_credential, credential_saved_at, connected_by)
         VALUES ($1, $2, 'matrix', 'connected', '@foundry:canzan.dev', 'matrix.canzan.dev',
                 $3, now(), $4)",
    )
    .bind(matrix)
    .bind(lab.ws)
    .bind(toy_seal("old", matrix, "matrix-token"))
    .bind(lab.lena)
    .execute(&pool)
    .await
    .expect("a second connection");
    sqlx::query("UPDATE integration_connections SET sealed_credential = $2 WHERE id = $1")
        .bind(lab.connection)
        .bind(toy_seal("old", lab.connection, "slack-token"))
        .execute(&pool)
        .await
        .expect("the Slack credential under the old key");
    let plaintexts = [
        (lab.connection, "slack-token".to_string()),
        (matrix, "matrix-token".to_string()),
    ];

    let new_canary = KeyCheck {
        key_id: "new".into(),
        sealed_canary: b"canary-new".to_vec(),
    };
    let outcome = lab
        .store
        .reseal_credentials(
            |row, sealed| {
                toy_open("old", row, sealed)
                    .map(|plain| toy_seal("new", row, &plain))
                    .ok_or(row)
            },
            &new_canary,
        )
        .await
        .expect("rotate");
    assert_eq!(outcome, ResealOutcome::Resealed { credentials: 2 });
    let after = credentials(&pool).await;
    assert_eq!(after.len(), 2, "no credential row is added or removed");
    for (row, plain) in &plaintexts {
        let sealed = &after.iter().find(|(id, _)| id == row).expect("row kept").1;
        assert_eq!(
            toy_open("new", *row, sealed).as_deref(),
            Some(plain.as_str()),
            "the credential opens under the new key to the same plaintext"
        );
        assert_eq!(
            toy_open("old", *row, sealed),
            None,
            "and no longer under the old one"
        );
    }
    assert_eq!(
        key_check_rows(&pool).await,
        vec![("new".to_string(), b"canary-new".to_vec())],
        "the new key's canary is written"
    );

    // A refusal on the last credential, after the first was re-sealed.
    let before_failure = (credentials(&pool).await, key_check_rows(&pool).await);
    let mut seen = 0;
    let outcome = lab
        .store
        .reseal_credentials(
            |row, sealed| {
                seen += 1;
                if seen == 2 {
                    return Err(row);
                }
                toy_open("new", row, sealed)
                    .map(|plain| toy_seal("newer", row, &plain))
                    .ok_or(row)
            },
            &KeyCheck {
                key_id: "newer".into(),
                sealed_canary: b"canary-newer".to_vec(),
            },
        )
        .await
        .expect("the store answers");
    assert!(matches!(outcome, ResealOutcome::Refused(_)), "{outcome:?}");
    assert_eq!(
        seen, 2,
        "the refusal came after one credential was re-sealed"
    );
    assert_eq!(
        (credentials(&pool).await, key_check_rows(&pool).await),
        before_failure,
        "a mid-way failure changes nothing"
    );
}

/// The secrets probe's check values (ADR-EIP-004 decision 4): recording one
/// persists it, a second recording for the same key keeps the first (replicas
/// of a rollout race to write it), and `key_checks` returns every stored
/// value in key id order.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn record_key_check_keeps_the_first_value_per_key_and_reads_back() {
    use foundry_store::integrations::KeyCheck;
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations(&pool).await.expect("head");
    let store = Store::from_pool(pool.clone());
    let check = |key_id: &str, canary: &[u8]| KeyCheck {
        key_id: key_id.into(),
        sealed_canary: canary.to_vec(),
    };
    let read = |checks: Vec<KeyCheck>| -> Vec<(String, Vec<u8>)> {
        checks
            .into_iter()
            .map(|c| (c.key_id, c.sealed_canary))
            .collect()
    };
    assert!(
        store.key_checks().await.expect("read").is_empty(),
        "no check value before the first boot"
    );

    store
        .record_key_check(&check("k2", b"canary-k2"))
        .await
        .expect("record k2");
    assert_eq!(
        key_check_rows(&pool).await,
        vec![("k2".to_string(), b"canary-k2".to_vec())],
        "the check value is persisted"
    );

    store
        .record_key_check(&check("k2", b"canary-k2-later"))
        .await
        .expect("a second replica's record is not an error");
    store
        .record_key_check(&check("k1", b"canary-k1"))
        .await
        .expect("record k1");

    let expected = vec![
        ("k1".to_string(), b"canary-k1".to_vec()),
        ("k2".to_string(), b"canary-k2".to_vec()),
    ];
    assert_eq!(
        key_check_rows(&pool).await,
        expected,
        "the first value for a key stays"
    );
    assert_eq!(
        read(store.key_checks().await.expect("read")),
        expected,
        "key_checks returns every stored value, in key id order"
    );
}
