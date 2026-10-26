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
    CancelCandidate, ClaimOutcome, ClaimView, ConnectionChange, ConnectionWriteOutcome,
    DeliveryResult, DeliveryTarget, DisconnectOutcome, DisconnectPreview, DispatchBatch,
    DispatchOutcome, NewConnection, NewDelivery, NewRoute, Pruned, Pruning, RecordOutcome,
    RouteChange, RouteChangeOutcome, RouteLane, RouteWriteOutcome,
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

/// The claim choice of a sender that keeps the channel's pace: nothing while
/// the destination is paced past `now`, else [`head_if_free`].
fn paced_head(now: OffsetDateTime) -> impl FnOnce(&ClaimView) -> Option<i64> {
    move |view| {
        if view.destination_not_before.is_some_and(|paced| paced > now) {
            return None;
        }
        head_if_free(now)(view)
    }
}

/// A record's counted `(kind, event, status)`.
fn tally_of(outcome: &RecordOutcome) -> Option<(&str, &str, &str)> {
    outcome
        .tally()
        .map(|t| (t.kind.as_str(), t.event_kind.as_str(), t.status.as_str()))
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

/// C-9, the rollback floor: the previous release's boot migrator
/// (`Store::migrate`, built without 0028-0031, so it knows 0001-0027) boots
/// on a database carrying them, ignoring the applied versions it does not
/// know. It undoes nothing: the newer objects are all still there, the
/// schema is still ready for this release, and an old outbox writer still
/// writes.
#[tokio::test]
async fn previous_release_migrator_accepts_0028_to_0031() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    let store = Store::from_pool(pool.clone());
    run_migrations(&pool)
        .await
        .expect("this release migrates to 0031");
    store.probe().await.expect("ready at 0031");
    let previous_release = staged("0027");
    run_boot_migrations_from_dir(&pool, &previous_release)
        .await
        .expect("the previous release boots on 0028-0031");
    store
        .probe()
        .await
        .expect("nothing of 0028-0031 was undone");
    assert!(has_column(&pool, "integration_email_choices", "scope").await);
    old_write(&pool, None).await;
}

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

/// A delivery's `give_up_at`, as dispatch stored it.
async fn give_up_at(pool: &PgPool, id: i64) -> OffsetDateTime {
    sqlx::query_scalar("SELECT give_up_at FROM integration_deliveries WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .expect("give_up_at")
}

/// ADR-EIP-002 decision 9, DDR-5 (review finding A2): a pending route head at
/// its give-up time, holding no lease, is never claimed, whatever the choice
/// names: the claim is a compare-and-set, and a row at or past `give_up_at`
/// belongs to the expiry sweep, never to a sender. The row and the
/// destination's pace are untouched.
///
/// CONTRACT_SHAPE: unbounded-preservation
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "RED until review fix A2"]
async fn a_route_head_at_its_give_up_time_is_never_claimed() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations(&pool).await.expect("head");
    let lab = lab(&pool).await;
    let id = lab.one_delivery().await;
    let before = lab.delivery(id).await;
    let now = give_up_at(&pool, id).await;

    let outcome = lab
        .store
        .claim_delivery(lab.route, now, LEASE, head_if_free(now))
        .await
        .expect("claim");

    assert!(
        !matches!(outcome, ClaimOutcome::Claimed(_)),
        "a head at its give-up time is not claimed: {outcome:?}"
    );
    assert_eq!(lab.delivery(id).await, before, "the row is untouched");
    assert_eq!(lab.pacing("C0AUTHDEV").await, None, "nothing is paced");
}

/// As [`a_route_head_at_its_give_up_time_is_never_claimed`], for a member's
/// project email (ADR-EIP-007 decision 2): the same compare-and-set.
///
/// CONTRACT_SHAPE: unbounded-preservation
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "RED until review fix A2"]
async fn an_email_at_its_give_up_time_is_never_claimed() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations(&pool).await.expect("head");
    let lab = lab(&pool).await;
    old_write(&pool, Some(lab.auth)).await;
    let lena = lab.lena;
    let outcome = lab
        .store
        .dispatch_tick(100, OffsetDateTime::now_utc(), GIVE_UP, |batch| {
            batch
                .events
                .iter()
                .map(|event| NewDelivery {
                    workspace_id: lab.ws,
                    project_id: lab.auth,
                    target: DeliveryTarget::Recipient(lena),
                    event_kind: "issue.created".to_string(),
                    event_id: event.event_id,
                    outbox_ids: vec![event.id],
                    issue_id: None,
                    issue_key: event.payload["key"].as_str().map(str::to_string),
                    occurred_at: event.created_at,
                })
                .collect()
        })
        .await
        .expect("dispatch");
    assert!(
        matches!(outcome, DispatchOutcome::Dispatched { deliveries: 1, .. }),
        "precondition: one email queued for Lena: {outcome:?}"
    );
    let id: i64 =
        sqlx::query_scalar("SELECT id FROM integration_deliveries WHERE recipient_user_id = $1")
            .bind(lena)
            .fetch_one(&pool)
            .await
            .expect("the email's id");
    let before = lab.delivery(id).await;
    let now = give_up_at(&pool, id).await;

    let claimed = lab
        .store
        .claim_email_delivery(lena, now, LEASE, head_if_free(now))
        .await
        .expect("claim");

    assert!(
        !matches!(claimed, ClaimOutcome::Claimed(_)),
        "an email at its give-up time is not claimed: {claimed:?}"
    );
    assert_eq!(lab.delivery(id).await, before, "the row is untouched");
}

/// Two routes to one channel share its pace (NFR-EIP-3), across senders: a
/// claim on the second route made while the first route's claim is still
/// open waits for it, then finds the channel paced, even when no claim had
/// paced the channel before. A second later the second route is claimed.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_routes_to_one_channel_share_its_pace() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations(&pool).await.expect("head");
    let lab = lab(&pool).await;
    let second = match lab.store.create_route(&lab.route_spec("C0AUTHDEV")).await {
        Ok(RouteWriteOutcome::Created(id)) => id,
        other => panic!("the second route is created: {other:?}"),
    };
    old_write(&pool, Some(lab.auth)).await;
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
        matches!(outcome, DispatchOutcome::Dispatched { deliveries: 2, .. }),
        "precondition: one delivery per route; {outcome:?}"
    );
    assert_eq!(
        lab.pacing("C0AUTHDEV").await,
        None,
        "precondition: never paced"
    );
    let now = OffsetDateTime::now_utc().replace_nanosecond(0).unwrap();

    let rival_store = Store::from_pool(pool.clone());
    let mut rival = None;
    let first = lab
        .store
        .claim_delivery(lab.route, now, LEASE, |view| {
            rival = Some(tokio::spawn(async move {
                rival_store
                    .claim_delivery(second, now, LEASE, paced_head(now))
                    .await
                    .expect("rival claim")
            }));
            // Time for the rival to finish, were it not made to wait.
            tokio::task::block_in_place(|| std::thread::sleep(Duration::from_millis(500)));
            paced_head(now)(view)
        })
        .await
        .expect("first claim");
    assert!(
        matches!(first, ClaimOutcome::Claimed(_)),
        "the first route is claimed: {first:?}"
    );
    let rival = rival.expect("the rival ran").await.expect("rival task");
    assert_eq!(
        rival,
        ClaimOutcome::NothingClaimable,
        "the second route waits for the channel's pace"
    );

    let later = now + Duration::from_secs(1);
    let next = lab
        .store
        .claim_delivery(second, later, LEASE, paced_head(later))
        .await
        .expect("claim a second later");
    assert!(
        matches!(next, ClaimOutcome::Claimed(_)),
        "a second later the second route is claimed: {next:?}"
    );
    assert_eq!(
        lab.pacing("C0AUTHDEV").await,
        Some(later + Duration::from_secs(1))
    );
}

/// The connection and route gauges' census (U-D8): connections by kind and
/// state; routes by kind and whether paused, only those of live projects.
#[tokio::test]
async fn census_counts_connections_and_live_routes_by_state() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations(&pool).await.expect("head");
    let lab = lab(&pool).await;
    let second = match lab.store.create_route(&lab.route_spec("C0AUTHREL")).await {
        Ok(RouteWriteOutcome::Created(id)) => id,
        other => panic!("the second route is created: {other:?}"),
    };
    sqlx::query("UPDATE integration_routes SET paused_at = now() WHERE id = $1")
        .bind(second)
        .execute(&pool)
        .await
        .expect("pause");
    let sorted = |mut rows: Vec<(String, String, i64)>| {
        rows.sort();
        rows
    };
    let row = |k: &str, s: &str, n: i64| (k.to_string(), s.to_string(), n);

    let census = lab.store.integration_census().await.expect("census");
    assert_eq!(census.connections, vec![row("slack", "connected", 1)]);
    assert_eq!(
        sorted(census.routes),
        vec![row("slack", "active", 1), row("slack", "paused", 1)]
    );

    sqlx::query("UPDATE projects SET deleted_at = now(), deleted_by = $2 WHERE id = $1")
        .bind(lab.auth)
        .bind(lab.lena)
        .execute(&pool)
        .await
        .expect("delete the project");
    let census = lab.store.integration_census().await.expect("census");
    assert_eq!(census.connections, vec![row("slack", "connected", 1)]);
    assert_eq!(
        census.routes,
        Vec::new(),
        "a deleted project's routes are not counted"
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
    assert_eq!(
        tally_of(&retried),
        Some(("slack", "issue.created", "pending")),
        "a retry is recorded and counted as one"
    );
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
    assert_eq!(
        tally_of(&delivered),
        Some(("slack", "issue.created", "delivered")),
        "a delivery is recorded and counted as one"
    );
    let lag = delivered.tally().expect("tally").lag_seconds;
    assert!(
        (0.0..60.0).contains(&lag),
        "the lag is the database's now() less the event's time; {lag}"
    );
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

/// The expiry and retention sweeps apply one compare-and-set over exactly
/// the ids their plan returned: an id the plan leaves out is untouched, and an
/// id the plan names that no longer qualifies is untouched too. (The cancel
/// sweep's half of this contract is `pause_applies_exactly_the_cancel_plan_ids`.)
#[tokio::test]
async fn sweeps_apply_exactly_the_plan_ids() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations(&pool).await.expect("head");
    let lab = lab(&pool).await;
    let t0 = OffsetDateTime::now_utc().replace_nanosecond(0).unwrap();
    let ids = [
        lab.one_delivery().await,
        lab.one_delivery().await,
        lab.one_delivery().await,
        lab.one_delivery().await,
    ];

    // Expiry: all four are past their give-up time a day later; the plan
    // names the first and the third, and only those fail.
    let late = t0 + GIVE_UP + Duration::from_secs(3600);
    let mut seen_due = Vec::new();
    let failed = lab
        .store
        .expire_deliveries(late, "gave_up_24h", |due| {
            seen_due = due.iter().map(|d| d.id).collect();
            vec![ids[0], ids[2]]
        })
        .await
        .expect("expire");
    assert_eq!(seen_due, ids.to_vec(), "the sweep reads every due delivery");
    assert_eq!(
        failed
            .iter()
            .map(|t| (t.kind.as_str(), t.event_kind.as_str(), t.status.as_str()))
            .collect::<Vec<_>>(),
        vec![("slack", "issue.created", "failed"); 2],
        "the sweep counts each delivery it failed"
    );
    for (n, id) in ids.iter().enumerate() {
        let row = lab.delivery(*id).await;
        let want = if n == 0 || n == 2 {
            "failed"
        } else {
            "pending"
        };
        assert_eq!(row.status, want, "delivery {n}");
    }
    // The fourth is delivered now; the second stays pending.
    let ClaimOutcome::Claimed(claim) = lab
        .store
        .claim_delivery(lab.route, late, LEASE, |view: &ClaimView| {
            view.deliveries
                .iter()
                .find(|d| d.id == ids[3])
                .map(|d| d.id)
        })
        .await
        .expect("claim the fourth")
    else {
        panic!("the fourth is claimable");
    };
    lab.store
        .record_delivery(ids[3], claim.claim_id, late, DeliveryResult::Delivered)
        .await
        .expect("delivered");

    // Retention, read 31 days on: the finished rows are offered with how many
    // of the route's deliveries are newer; outbox rows with whether a pending
    // delivery reads them.
    let outbox_of = |id: i64| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Vec<i64>>(
                "SELECT outbox_ids FROM integration_deliveries WHERE id = $1",
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("outbox ids")[0]
        }
    };
    let outbox = [
        outbox_of(ids[0]).await,
        outbox_of(ids[1]).await,
        outbox_of(ids[2]).await,
        outbox_of(ids[3]).await,
    ];
    lab.store
        .beat_worker(
            "dispatcher",
            "replica-gone",
            t0 - Duration::from_secs(8 * 86400),
        )
        .await
        .expect("an old heartbeat");
    lab.store
        .beat_worker("dispatcher", "replica-alive", late)
        .await
        .expect("a live heartbeat");
    let read_before = late + Duration::from_secs(31 * 86400);
    let mut view = None;
    let pruned = lab
        .store
        .prune_deliveries(read_before, |v| {
            view = Some(v.clone());
            Pruning {
                // the second is pending: named, yet never deleted.
                deliveries: vec![ids[0], ids[1], ids[3]],
                // the second's outbox row is read by a pending delivery.
                outbox: vec![outbox[0], outbox[1], outbox[3]],
                heartbeats_before: t0 - Duration::from_secs(7 * 86400),
            }
        })
        .await
        .expect("prune");
    let view = view.expect("the plan saw the view");
    assert_eq!(
        view.deliveries
            .iter()
            .map(|d| (d.id, d.newer_on_target))
            .collect::<Vec<_>>(),
        vec![(ids[0], 3), (ids[2], 1), (ids[3], 0)],
        "finished rows only, each with its newer count"
    );
    assert_eq!(
        view.outbox
            .iter()
            .filter(|o| outbox.contains(&o.id))
            .map(|o| (o.id, o.referenced_by_pending))
            .collect::<Vec<_>>(),
        vec![
            (outbox[0], false),
            (outbox[1], true),
            (outbox[2], false),
            (outbox[3], false)
        ]
    );
    assert_eq!(
        pruned,
        Pruned {
            deliveries: 2,
            outbox: 2,
            heartbeats: 1
        }
    );
    let left: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM integration_deliveries WHERE route_id = $1 ORDER BY id")
            .bind(lab.route)
            .fetch_all(&pool)
            .await
            .expect("left deliveries");
    assert_eq!(left, vec![ids[1], ids[2]]);
    let outbox_left: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM outbox WHERE id = ANY($1) ORDER BY id")
            .bind(outbox.to_vec())
            .fetch_all(&pool)
            .await
            .expect("left outbox");
    assert_eq!(outbox_left, vec![outbox[1], outbox[2]]);
    let replicas: Vec<String> =
        sqlx::query_scalar("SELECT replica FROM integration_worker_heartbeats ORDER BY replica")
            .fetch_all(&pool)
            .await
            .expect("heartbeats");
    assert_eq!(replicas, vec!["replica-alive".to_string()]);
}

/// The record applies what an answer means beyond its row, fenced like the
/// row: a "retry after" paces the destination, a refused destination stops
/// its route, a refused connection is lost once; and a stopped route or a
/// lost connection is no sender's candidate.
#[tokio::test]
async fn record_applies_slow_down_route_and_connection_refusals() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations(&pool).await.expect("head");
    let lab = lab(&pool).await;
    let t0 = OffsetDateTime::now_utc().replace_nanosecond(0).unwrap();
    let id = lab.one_delivery().await;
    let claim = |at: OffsetDateTime| {
        let store = &lab.store;
        let route = lab.route;
        async move {
            match store
                .claim_delivery(route, at, LEASE, head_if_free(at))
                .await
                .expect("claim")
            {
                ClaimOutcome::Claimed(c) => c,
                other => panic!("claimed: {other:?}"),
            }
        }
    };

    let first = claim(t0).await;
    let wait_until = t0 + Duration::from_secs(30);
    lab.store
        .record_delivery(
            id,
            first.claim_id,
            t0,
            DeliveryResult::SlowedDown {
                next_attempt_at: wait_until,
                not_before: Some(wait_until),
                vendor_reason: "slack_slow_down".to_string(),
            },
        )
        .await
        .expect("slowed down");
    assert_eq!(lab.pacing("C0AUTHDEV").await, Some(wait_until));
    let slowed: bool =
        sqlx::query_scalar("SELECT slowed_down FROM integration_deliveries WHERE id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("slowed_down");
    assert!(slowed);

    let second = claim(wait_until).await;
    lab.store
        .record_delivery(
            id,
            second.claim_id,
            wait_until,
            DeliveryResult::RouteRefused {
                vendor_reason: "channel_not_found".to_string(),
            },
        )
        .await
        .expect("route refused");
    let row = lab.delivery(id).await;
    assert_eq!(
        (row.status.as_str(), row.next_attempt_at, row.lease_until),
        ("pending", None, None)
    );
    let problem: Option<String> =
        sqlx::query_scalar("SELECT problem_reason FROM integration_routes WHERE id = $1")
            .bind(lab.route)
            .fetch_one(&pool)
            .await
            .expect("problem");
    assert_eq!(problem.as_deref(), Some("channel_not_found"));
    assert!(lab
        .store
        .routes_with_pending_deliveries(10)
        .await
        .expect("candidates")
        .is_empty());
    assert_eq!(
        lab.store
            .claim_delivery(lab.route, wait_until, LEASE, head_if_free(wait_until))
            .await
            .expect("claim a stopped route"),
        ClaimOutcome::RouteBusy
    );

    // Resume (03-03's door) clears the problem; a refused connection is lost.
    exec(&pool, "UPDATE integration_routes SET problem_reason = NULL").await;
    assert_eq!(
        lab.store
            .routes_with_pending_deliveries(10)
            .await
            .expect("candidates"),
        vec![lab.route],
        "a resumed route is a candidate again"
    );
    let later = wait_until + Duration::from_secs(60);
    let third = claim(later).await;
    lab.store
        .record_delivery(
            id,
            third.claim_id,
            later,
            DeliveryResult::ConnectionRefused {
                vendor_reason: "token_revoked".to_string(),
            },
        )
        .await
        .expect("connection refused");
    let (state, lost_reason): (String, Option<String>) =
        sqlx::query_as("SELECT state, lost_reason FROM integration_connections WHERE id = $1")
            .bind(lab.connection)
            .fetch_one(&pool)
            .await
            .expect("connection");
    assert_eq!(
        (state.as_str(), lost_reason.as_deref()),
        ("lost", Some("token_revoked"))
    );
    assert!(lab
        .store
        .routes_with_pending_deliveries(10)
        .await
        .expect("candidates")
        .is_empty());
    assert_eq!(lab.delivery(id).await.status, "pending");
}

/// 03-02 (DDR-11, DDR-23): Slack's word that the Foundry app left a Slack
/// workspace turns only that app's connection there `connected → lost`, with
/// one audit record; only the call (or refusal record) that turns it gets the
/// workspace's admins to tell, so two replicas and repeated events tell them
/// once; and the workspace's page reads the lost state and its reason.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_lost_connection_is_told_once_to_the_workspace_admins() {
    use foundry_store::integrations::LostConnection;
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations(&pool).await.expect("head");
    let lab = lab(&pool).await;
    exec(
        &pool,
        "UPDATE integration_connections SET app_id = 'A0EIPFOUNDRY'",
    )
    .await;
    let priya = Id::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, 'priya@canzan.dev', 'Priya@canzan.dev', 'Priya', 'not-a-real-hash')",
    )
    .bind(priya)
    .execute(&pool)
    .await
    .expect("admin");
    for (user, role) in [(priya, "admin"), (lab.lena, "member")] {
        sqlx::query(
            "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, $3)",
        )
        .bind(lab.ws)
        .bind(user)
        .bind(role)
        .execute(&pool)
        .await
        .expect("membership");
    }
    let now = OffsetDateTime::now_utc().replace_nanosecond(0).unwrap();
    let state = || async {
        sqlx::query_as::<_, (String, Option<String>, Option<OffsetDateTime>)>(
            "SELECT state, lost_reason, lost_at FROM integration_connections WHERE id = $1",
        )
        .bind(lab.connection)
        .fetch_one(&pool)
        .await
        .expect("connection")
    };
    let audits = || async {
        sqlx::query_as::<_, (String, String, String)>(
            "SELECT via, kind, subject FROM integration_audit_events
              WHERE action = 'connection_lost' ORDER BY created_at",
        )
        .fetch_all(&pool)
        .await
        .expect("audit")
    };

    for (team, app) in [
        ("T0PERSONAL", "A0EIPFOUNDRY"),
        ("T0CANZAN", "A0SOMEOTHERAPP"),
    ] {
        assert_eq!(
            lab.store
                .slack_app_lost(team, app, "app_removed", now)
                .await
                .expect("foreign"),
            None,
            "{team} {app} is not this connection"
        );
    }
    assert_eq!(state().await, ("connected".to_string(), None, None));

    let (a, b) = tokio::join!(
        lab.store
            .slack_app_lost("T0CANZAN", "A0EIPFOUNDRY", "app_removed", now),
        lab.store
            .slack_app_lost("T0CANZAN", "A0EIPFOUNDRY", "app_removed", now),
    );
    let told: Vec<LostConnection> = [a.expect("first"), b.expect("second")]
        .into_iter()
        .flatten()
        .collect();
    assert_eq!(
        told,
        vec![LostConnection {
            workspace_id: lab.ws,
            workspace_name: "Canzan Labs".to_string(),
            kind: "slack".to_string(),
            reason: Some("app_removed".to_string()),
            admins: vec!["Priya@canzan.dev".to_string()],
        }],
        "exactly one of two concurrent calls turns it, and gets only the admins"
    );
    assert_eq!(
        state().await,
        (
            "lost".to_string(),
            Some("app_removed".to_string()),
            Some(now)
        )
    );
    assert_eq!(
        audits().await,
        vec![(
            "slack".to_string(),
            "slack".to_string(),
            "Canzan Labs".to_string()
        )]
    );
    let shown = lab
        .store
        .workspace_connections(lab.ws)
        .await
        .expect("connections");
    assert_eq!(
        shown
            .iter()
            .map(|c| (
                c.state.as_str(),
                c.lost_reason.as_deref(),
                c.workspace_name.as_str()
            ))
            .collect::<Vec<_>>(),
        vec![("lost", Some("app_removed"), "Canzan Labs")]
    );
    assert_eq!(
        lab.store
            .slack_app_lost("T0CANZAN", "A0EIPFOUNDRY", "app_removed", now)
            .await
            .expect("again"),
        None,
        "a connection already lost is not lost again"
    );

    // A refusal recorded after the connection was lost is only recorded; one
    // that turns a connected connection lost gets its admins, once.
    let id = lab.one_delivery().await;
    let claim = |at: OffsetDateTime| {
        let store = &lab.store;
        let route = lab.route;
        async move {
            match store
                .claim_delivery(route, at, LEASE, head_if_free(at))
                .await
                .expect("claim")
            {
                ClaimOutcome::Claimed(c) => c.claim_id,
                other => panic!("claimed: {other:?}"),
            }
        }
    };
    let refuse = |claim_id: Id, at: OffsetDateTime| {
        let store = &lab.store;
        async move {
            store
                .record_delivery(
                    id,
                    claim_id,
                    at,
                    DeliveryResult::ConnectionRefused {
                        vendor_reason: "token_revoked".to_string(),
                    },
                )
                .await
                .expect("record")
        }
    };
    exec(
        &pool,
        "UPDATE integration_connections SET state = 'connected'",
    )
    .await;
    let later = now + Duration::from_secs(60);
    let held = claim(later).await;
    assert!(lab
        .store
        .slack_app_lost("T0CANZAN", "A0EIPFOUNDRY", "app_removed", later)
        .await
        .expect("lost while a send was out")
        .is_some());
    let refused = refuse(held, later).await;
    assert!(
        matches!(refused, RecordOutcome::Recorded(ref t) if t.status == "pending"),
        "a refusal on a lost connection tells no one: {refused:?}"
    );
    assert_eq!(audits().await.len(), 2);

    exec(
        &pool,
        "UPDATE integration_connections SET state = 'connected'",
    )
    .await;
    let last = later + Duration::from_secs(60);
    let claim_id = claim(last).await;
    match refuse(claim_id, last).await {
        RecordOutcome::ConnectionLost { lost, tally } => assert_eq!(
            (lost.reason.as_deref(), lost.admins, tally.status.as_str()),
            (
                Some("token_revoked"),
                vec!["Priya@canzan.dev".to_string()],
                "pending"
            )
        ),
        other => panic!("the refusal turns the connection lost: {other:?}"),
    }
    let audits = audits().await;
    assert_eq!(audits.len(), 3);
    assert_eq!(audits[2].0, "system");
}

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

/// The cancel half of the sweeps' contract (DD-8): a pause hands the route's
/// pending deliveries to the plan and skips exactly the ids it names that are
/// still pending and hold no live lease; the leased one is left to its sender.
/// Each route change writes one audit record, and asking for what is already
/// so writes nothing. A resumed route's earlier deliveries read as paused to
/// their sender (DDR-27), and a delete takes the route's deliveries with it.
#[tokio::test]
async fn pause_applies_exactly_the_cancel_plan_ids() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations(&pool).await.expect("head");
    let lab = lab(&pool).await;
    let ids = [
        lab.one_delivery().await,
        lab.one_delivery().await,
        lab.one_delivery().await,
        lab.one_delivery().await,
    ];
    let now = OffsetDateTime::now_utc().replace_nanosecond(0).unwrap();
    let ClaimOutcome::Claimed(claim) = lab
        .store
        .claim_delivery(lab.route, now, LEASE, head_if_free(now))
        .await
        .expect("claim the head")
    else {
        panic!("the head is claimable");
    };
    assert_eq!(claim.id, ids[0]);
    let audit = || {
        let pool = pool.clone();
        let ws = lab.ws;
        async move {
            sqlx::query_scalar::<_, String>(
                "SELECT action FROM integration_audit_events
                  WHERE workspace_id = $1 AND action <> 'route_created'
                  ORDER BY created_at, id",
            )
            .bind(ws)
            .fetch_all(&pool)
            .await
            .expect("audit")
        }
    };
    let change = RouteChange {
        project_id: lab.auth,
        route_id: lab.route,
        actor_id: lab.lena,
        via: "web".to_string(),
        at: now + Duration::from_secs(1),
    };

    let mut seen: Vec<CancelCandidate> = Vec::new();
    let paused = lab
        .store
        .pause_route(&change, |candidates| {
            seen = candidates.to_vec();
            // the leased head is named, yet never skipped; the fourth is left out.
            vec![ids[0], ids[1], ids[2]]
        })
        .await
        .expect("pause");
    assert_eq!(paused, RouteChangeOutcome::Changed { cancelled: 2 });
    assert_eq!(
        seen.iter().map(|c| c.id).collect::<Vec<_>>(),
        ids.to_vec(),
        "the plan sees every pending delivery of the route"
    );
    assert_eq!(
        seen[0].lease_until,
        Some(claim.lease_until),
        "with the head's live lease"
    );
    for (n, id) in ids.iter().enumerate() {
        let row = lab.delivery(*id).await;
        let want = if n == 1 || n == 2 {
            ("skipped", Some("route_paused"))
        } else {
            ("pending", None)
        };
        assert_eq!(
            (row.status.as_str(), row.final_reason.as_deref()),
            want,
            "delivery {n}"
        );
    }
    assert_eq!(audit().await, vec!["route_paused"]);

    // Pausing a paused route changes nothing and asks no plan.
    let again = lab
        .store
        .pause_route(&change, |_| panic!("no plan for a paused route"))
        .await
        .expect("pause again");
    assert_eq!(again, RouteChangeOutcome::Unchanged);
    assert_eq!(audit().await, vec!["route_paused"]);

    // Resume: one record; resuming a live route changes nothing.
    assert_eq!(
        lab.store.resume_route(&change).await.expect("resume"),
        RouteChangeOutcome::Changed { cancelled: 0 }
    );
    assert_eq!(
        lab.store.resume_route(&change).await.expect("resume again"),
        RouteChangeOutcome::Unchanged
    );
    assert_eq!(audit().await, vec!["route_paused", "route_resumed"]);
    let context = lab
        .store
        .delivery_context(ids[3])
        .await
        .expect("context")
        .expect("the fourth is still there");
    assert!(
        context.route_paused,
        "an event from before the resume is never sent"
    );

    // Delete: the route and its deliveries go, with one record; again, nothing.
    assert_eq!(
        lab.store.delete_route(&change).await.expect("delete"),
        RouteChangeOutcome::Changed { cancelled: 0 }
    );
    assert_eq!(
        lab.store.delete_route(&change).await.expect("delete again"),
        RouteChangeOutcome::Unchanged
    );
    assert_eq!(
        audit().await,
        vec!["route_paused", "route_resumed", "route_deleted"]
    );
    let left: i64 =
        sqlx::query_scalar("SELECT count(*) FROM integration_deliveries WHERE route_id = $1")
            .bind(lab.route)
            .fetch_one(&pool)
            .await
            .expect("count");
    assert_eq!(left, 0, "the route's deliveries went with it");
}

// US-EIP-06 (03-04): resuming a route that only a vendor refusal stopped
// keeps the deliveries waiting for it (D-8: drained once Resume re-checked
// the channel), unlike resuming a paused route (DDR-27).
#[tokio::test]
async fn resuming_a_route_a_refusal_stopped_keeps_its_waiting_deliveries() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations(&pool).await.expect("head");
    let lab = lab(&pool).await;
    let waiting = lab.one_delivery().await;
    sqlx::query(
        "UPDATE integration_routes SET problem_reason = 'not_in_channel', problem_at = now()
          WHERE id = $1",
    )
    .bind(lab.route)
    .execute(&pool)
    .await
    .expect("the refusal");
    let settings = |lab: &Lab| {
        let store = lab.store.clone();
        let (project, route) = (lab.auth, lab.route);
        async move {
            store
                .route_settings(project, route)
                .await
                .expect("settings")
                .expect("the route")
        }
    };
    assert_eq!(
        settings(&lab).await.problem_reason.as_deref(),
        Some("not_in_channel"),
        "the edit form's settings carry the route's problem"
    );
    let receives_from = || {
        let pool = pool.clone();
        let route = lab.route;
        async move {
            sqlx::query_scalar::<_, OffsetDateTime>(
                "SELECT receives_from FROM integration_routes WHERE id = $1",
            )
            .bind(route)
            .fetch_one(&pool)
            .await
            .expect("receives_from")
        }
    };
    let before = receives_from().await;
    let change = RouteChange {
        project_id: lab.auth,
        route_id: lab.route,
        actor_id: lab.lena,
        via: "web".to_string(),
        at: OffsetDateTime::now_utc(),
    };
    assert_eq!(
        lab.store.resume_route(&change).await.expect("resume"),
        RouteChangeOutcome::Changed { cancelled: 0 }
    );
    assert_eq!(
        settings(&lab).await.problem_reason,
        None,
        "the problem is cleared"
    );
    assert_eq!(receives_from().await, before, "the route's boundary stays");
    let context = lab
        .store
        .delivery_context(waiting)
        .await
        .expect("context")
        .expect("the waiting delivery is still there");
    assert!(
        !context.route_paused,
        "what waited is sent after the resume"
    );
    assert_eq!(lab.delivery(waiting).await.status, "pending");
}

// US-EIP-06 (03-04): a Slack install for another team than the connection's
// changes nothing (state, team, credential, audit); one for the same team
// restores the lost connection in place, with its new credential.
#[tokio::test]
async fn reconnect_keeps_its_slack_team() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations(&pool).await.expect("head");
    let lab = lab(&pool).await;
    sqlx::query(
        "UPDATE integration_connections
            SET state = 'lost', lost_reason = 'app_removed', lost_at = now()
          WHERE id = $1",
    )
    .bind(lab.connection)
    .execute(&pool)
    .await
    .expect("lose the connection");
    let row = || {
        let pool = pool.clone();
        let id = lab.connection;
        async move {
            sqlx::query_as::<_, (String, Option<String>, String, String, Vec<u8>)>(
                "SELECT state, lost_reason, external_id, label, sealed_credential
                   FROM integration_connections WHERE id = $1",
            )
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("the connection")
        }
    };
    let audit = || {
        let pool = pool.clone();
        let ws = lab.ws;
        async move {
            sqlx::query_scalar::<_, String>(
                "SELECT action FROM integration_audit_events
                  WHERE workspace_id = $1 AND action <> 'route_created' ORDER BY id",
            )
            .bind(ws)
            .fetch_all(&pool)
            .await
            .expect("audit")
        }
    };
    let install = |team: &str, label: &str| NewConnection {
        id: lab.connection,
        workspace_id: lab.ws,
        kind: "slack".to_string(),
        external_id: team.to_string(),
        app_id: Some("A0EIPFOUNDRY".to_string()),
        bot_user_id: Some("U0FOUNDRYBOT".to_string()),
        label: label.to_string(),
        detail: Some("canzan.slack.com".to_string()),
        sealed_credential: vec![7, 7, 7],
        connected_by: lab.lena,
        connected_at: OffsetDateTime::now_utc(),
    };
    let lost = row().await;

    let other = lab
        .store
        .upsert_connection(&install("T0PERSONAL", "Canzan Personal"))
        .await
        .expect("the other team's install");
    assert_eq!(
        other,
        ConnectionWriteOutcome::OtherWorkspace {
            connected_label: "Canzan Labs".to_string()
        }
    );
    assert_eq!(row().await, lost, "the connection is untouched");
    assert!(audit().await.is_empty(), "nothing is recorded");

    let same = lab
        .store
        .upsert_connection(&install("T0CANZAN", "Canzan Labs"))
        .await
        .expect("the same team's install");
    assert_eq!(same, ConnectionWriteOutcome::Reconnected);
    assert_eq!(
        row().await,
        (
            "connected".to_string(),
            None,
            "T0CANZAN".to_string(),
            "Canzan Labs".to_string(),
            vec![7, 7, 7]
        ),
        "the connection is restored with its new credential"
    );
    assert_eq!(audit().await, vec!["connection_reconnected"]);
}

// US-EIP-07 (03-05), R-9: a disconnect counts the routes that post now; then,
// in one transaction, it skips exactly the plan's unleased waiting deliveries
// of the connection's routes, forgets the credential (handing it back, still
// sealed, once) and records it. What it left reads as paused. A reconnect,
// even to another Slack team, receives from then on: nothing from before.
#[tokio::test]
async fn disconnect_forgets_the_credential_and_queues_nothing() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations(&pool).await.expect("head");
    let lab = lab(&pool).await;
    let ids = [
        lab.one_delivery().await,
        lab.one_delivery().await,
        lab.one_delivery().await,
    ];
    let now = OffsetDateTime::now_utc().replace_nanosecond(0).unwrap();
    let ClaimOutcome::Claimed(claim) = lab
        .store
        .claim_delivery(lab.route, now, LEASE, head_if_free(now))
        .await
        .expect("claim the head")
    else {
        panic!("the head is claimable");
    };
    let paused_route = match lab.store.create_route(&lab.route_spec("C0AUTHREL")).await {
        Ok(RouteWriteOutcome::Created(id)) => id,
        other => panic!("a second route: {other:?}"),
    };
    let pause = RouteChange {
        project_id: lab.auth,
        route_id: paused_route,
        actor_id: lab.lena,
        via: "web".to_string(),
        at: now,
    };
    lab.store
        .pause_route(&pause, |_| Vec::new())
        .await
        .expect("pause the second route");
    let audit = || {
        let pool = pool.clone();
        let ws = lab.ws;
        async move {
            sqlx::query_scalar::<_, String>(
                "SELECT action FROM integration_audit_events
                  WHERE workspace_id = $1 AND action LIKE 'connection_%' ORDER BY id",
            )
            .bind(ws)
            .fetch_all(&pool)
            .await
            .expect("audit")
        }
    };
    assert_eq!(
        lab.store
            .disconnect_preview(lab.ws, "slack")
            .await
            .expect("preview"),
        Some(DisconnectPreview {
            label: "Canzan Labs".to_string(),
            routes: 1,
            projects: 1,
        }),
        "the paused route does not stop: it already has"
    );
    lab.store
        .delete_route(&pause)
        .await
        .expect("delete the second route");
    assert_eq!(
        lab.store
            .disconnect_preview(lab.ws, "matrix")
            .await
            .expect("preview"),
        None
    );
    let change = ConnectionChange {
        workspace_id: lab.ws,
        kind: "slack".to_string(),
        actor_id: lab.lena,
        via: "web".to_string(),
        at: now + Duration::from_secs(1),
    };

    let mut seen: Vec<CancelCandidate> = Vec::new();
    let outcome = lab
        .store
        .disconnect_connection(&change, |candidates| {
            seen = candidates.to_vec();
            // the leased head is named, yet never skipped; the third is left out.
            vec![ids[0], ids[1]]
        })
        .await
        .expect("disconnect");
    assert_eq!(
        outcome,
        DisconnectOutcome::Disconnected {
            connection_id: lab.connection,
            sealed_credential: vec![0],
            cancelled: 1,
        }
    );
    let shown = format!("{outcome:?}");
    assert!(
        shown.starts_with("Disconnected") && shown.contains(&lab.connection.to_string()),
        "{shown}"
    );
    assert!(
        !shown.contains("sealed_credential"),
        "the handed-back credential is never printed: {shown}"
    );
    assert_eq!(
        seen.iter().map(|c| (c.id, c.route_id)).collect::<Vec<_>>(),
        ids.iter().map(|id| (*id, lab.route)).collect::<Vec<_>>(),
        "the plan sees every waiting delivery of the connection's routes"
    );
    assert_eq!(seen[0].lease_until, Some(claim.lease_until));
    let statuses = || {
        let pool = pool.clone();
        async move {
            sqlx::query_as::<_, (String, Option<String>)>(
                "SELECT status, final_reason FROM integration_deliveries ORDER BY id",
            )
            .fetch_all(&pool)
            .await
            .expect("deliveries")
        }
    };
    assert_eq!(
        statuses().await,
        vec![
            ("pending".to_string(), None),
            ("skipped".to_string(), Some("route_paused".to_string())),
            ("pending".to_string(), None),
        ]
    );
    let row: (String, Option<Vec<u8>>, Option<OffsetDateTime>) = sqlx::query_as(
        "SELECT state, sealed_credential, credential_saved_at FROM integration_connections",
    )
    .fetch_one(&pool)
    .await
    .expect("the connection");
    assert_eq!(row, ("disconnected".to_string(), None, None));
    assert_eq!(audit().await, vec!["connection_disconnected"]);
    let context = lab
        .store
        .delivery_context(ids[2])
        .await
        .expect("context")
        .expect("the third is still there");
    assert!(context.route_paused, "nothing waiting is sent");
    assert_eq!(
        lab.store
            .disconnect_preview(lab.ws, "slack")
            .await
            .expect("preview"),
        None,
        "nothing left to disconnect"
    );
    assert_eq!(
        lab.store
            .disconnect_connection(&change, |_| panic!("no plan once disconnected"))
            .await
            .expect("disconnect again"),
        DisconnectOutcome::Unchanged
    );
    assert_eq!(audit().await, vec!["connection_disconnected"]);

    let receives_from = || {
        let pool = pool.clone();
        let route = lab.route;
        async move {
            sqlx::query_scalar::<_, OffsetDateTime>(
                "SELECT receives_from FROM integration_routes WHERE id = $1",
            )
            .bind(route)
            .fetch_one(&pool)
            .await
            .expect("receives_from")
        }
    };
    let before = receives_from().await;
    let reconnected = lab
        .store
        .upsert_connection(&NewConnection {
            id: lab.connection,
            workspace_id: lab.ws,
            kind: "slack".to_string(),
            external_id: "T0PERSONAL".to_string(),
            app_id: Some("A0EIPFOUNDRY".to_string()),
            bot_user_id: Some("U0FOUNDRYBOT".to_string()),
            label: "Canzan Personal".to_string(),
            detail: None,
            sealed_credential: vec![9],
            connected_by: lab.lena,
            connected_at: OffsetDateTime::now_utc(),
        })
        .await
        .expect("connect again");
    assert_eq!(reconnected, ConnectionWriteOutcome::Reconnected);
    assert!(
        receives_from().await > before,
        "the route receives from now"
    );
    let context = lab
        .store
        .delivery_context(ids[2])
        .await
        .expect("context")
        .expect("the third");
    assert!(
        context.route_paused,
        "what happened before the reconnect is never sent"
    );
    let fresh = lab.one_delivery().await;
    let context = lab
        .store
        .delivery_context(fresh)
        .await
        .expect("context")
        .expect("the fresh one");
    assert!(!context.route_paused, "what happens after it is sent");
}

// US-EIP-07 (03-05), D-8: what waited when its project was deleted reads as
// the project gone, even once the project is restored; what happens after
// the restore is sent.
#[tokio::test]
async fn a_delivery_waiting_at_a_project_delete_stays_unsendable_after_restore() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations(&pool).await.expect("head");
    let lab = lab(&pool).await;
    let waiting = lab.one_delivery().await;
    let project_now = |id: i64| {
        let store = lab.store.clone();
        async move {
            store
                .delivery_context(id)
                .await
                .expect("context")
                .expect("the delivery")
                .project
                .map(|p| p.name)
        }
    };
    assert_eq!(
        project_now(waiting).await.as_deref(),
        Some("Identity Platform")
    );
    lab.store
        .soft_delete_project(lab.ws, lab.team, lab.auth, lab.lena)
        .await
        .expect("delete the project");
    assert_eq!(project_now(waiting).await, None);
    lab.store
        .restore_deleted_project(lab.auth)
        .await
        .expect("restore the project");
    assert_eq!(
        project_now(waiting).await,
        None,
        "what waited at the delete is never sent"
    );
    let fresh = lab.one_delivery().await;
    assert_eq!(
        project_now(fresh).await.as_deref(),
        Some("Identity Platform"),
        "the restored project's routes post again"
    );
}

// US-EIP-08 (04-02): a workspace's Matrix connection sits beside its Slack
// one; the route doors read its homeserver and bot, and the sender reads the
// homeserver by connection id. Neither read ever returns another kind's row.
#[tokio::test]
async fn a_matrix_connection_is_read_with_its_homeserver_and_bot() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations(&pool).await.expect("head");
    let lab = lab(&pool).await;
    assert_eq!(
        lab.store
            .sealed_connection(lab.ws, "matrix")
            .await
            .expect("read"),
        None,
        "no Matrix connection yet"
    );
    let matrix = Id::now_v7();
    let outcome = lab
        .store
        .upsert_connection(&NewConnection {
            id: matrix,
            workspace_id: lab.ws,
            kind: "matrix".to_string(),
            external_id: "https://matrix.canzan.dev".to_string(),
            app_id: None,
            bot_user_id: Some("@foundry:canzan.dev".to_string()),
            label: "matrix.canzan.dev".to_string(),
            detail: Some("@foundry:canzan.dev".to_string()),
            sealed_credential: vec![4, 2],
            connected_by: lab.lena,
            connected_at: OffsetDateTime::now_utc(),
        })
        .await
        .expect("connect Matrix");
    assert_eq!(outcome, ConnectionWriteOutcome::Connected);
    let sealed = lab
        .store
        .sealed_connection(lab.ws, "matrix")
        .await
        .expect("read")
        .expect("the Matrix connection");
    assert_eq!(sealed.id, matrix);
    assert_eq!(sealed.state, "connected");
    assert_eq!(sealed.external_id, "https://matrix.canzan.dev");
    assert_eq!(sealed.bot_user_id.as_deref(), Some("@foundry:canzan.dev"));
    assert_eq!(sealed.sealed_credential, Some(vec![4, 2]));
    let slack = lab
        .store
        .sealed_connection(lab.ws, "slack")
        .await
        .expect("read")
        .expect("the lab's Slack connection");
    assert_eq!(slack.id, lab.connection, "each kind reads its own row");
    assert_ne!(slack.external_id, sealed.external_id);
    assert_eq!(
        lab.store
            .connection_external_id(matrix)
            .await
            .expect("read")
            .as_deref(),
        Some("https://matrix.canzan.dev")
    );
    assert_eq!(
        lab.store
            .connection_external_id(Id::now_v7())
            .await
            .expect("read"),
        None,
        "no row, no address"
    );
}

// US-EIP-09 (04-04): a deleted Matrix route's bot leaves only a room no other
// route of the workspace uses; the store says whether a destination is still
// routed on the workspace's connection of a kind.
#[tokio::test]
async fn a_destination_is_routed_while_a_route_of_the_workspace_sends_to_it() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations(&pool).await.expect("head");
    let lab = lab(&pool).await;
    let routed = |workspace: Id, kind: &'static str, destination: &'static str| {
        let store = lab.store.clone();
        async move {
            store
                .destination_routed(workspace, kind, destination)
                .await
                .expect("routed")
        }
    };
    assert!(routed(lab.ws, "slack", "C0AUTHDEV").await);
    assert!(
        !routed(lab.ws, "slack", "C0OTHER").await,
        "another destination"
    );
    assert!(!routed(lab.ws, "matrix", "C0AUTHDEV").await, "another kind");
    assert!(
        !routed(Id::now_v7(), "slack", "C0AUTHDEV").await,
        "another workspace"
    );
    let change = RouteChange {
        project_id: lab.auth,
        route_id: lab.route,
        actor_id: lab.lena,
        via: "web".to_string(),
        at: OffsetDateTime::now_utc(),
    };
    lab.store.delete_route(&change).await.expect("delete");
    assert!(
        !routed(lab.ws, "slack", "C0AUTHDEV").await,
        "no route sends to it once its route is deleted"
    );
}

// US-EIP-09 (04-04): two routes of the workspace share one Matrix room (from
// two projects). Deleting one leaves the room routed, so the bot stays for
// the other; deleting the last one leaves it unrouted, so the bot may leave.
#[tokio::test]
async fn a_matrix_room_two_routes_share_stays_routed_until_the_last_is_deleted() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    run_migrations(&pool).await.expect("head");
    let lab = lab(&pool).await;
    let matrix = Id::now_v7();
    lab.store
        .upsert_connection(&NewConnection {
            id: matrix,
            workspace_id: lab.ws,
            kind: "matrix".to_string(),
            external_id: "https://matrix.canzan.dev".to_string(),
            app_id: None,
            bot_user_id: Some("@foundry:canzan.dev".to_string()),
            label: "matrix.canzan.dev".to_string(),
            detail: Some("@foundry:canzan.dev".to_string()),
            sealed_credential: vec![4, 2],
            connected_by: lab.lena,
            connected_at: OffsetDateTime::now_utc(),
        })
        .await
        .expect("connect Matrix");
    let ops = Id::now_v7();
    lab.store
        .insert_project(ops, lab.ws, lab.team, "Homelab Ops", "ops", "OPS")
        .await
        .expect("a second project");
    const ROOM: &str = "!auth:canzan.dev";
    let to_the_room = |project: Id| NewRoute {
        project_id: project,
        connection_id: Some(matrix),
        kind: "matrix".to_string(),
        destination_ref: ROOM.to_string(),
        destination_label: "#auth:canzan.dev".to_string(),
        lanes: Vec::new(),
        ..lab.route_spec(ROOM)
    };
    let mut routes = Vec::new();
    for project in [lab.auth, ops] {
        match lab.store.create_route(&to_the_room(project)).await {
            Ok(RouteWriteOutcome::Created(id)) => routes.push((project, id)),
            other => panic!("the route to the room is created: {other:?}"),
        }
    }
    let routed = || async {
        lab.store
            .destination_routed(lab.ws, "matrix", ROOM)
            .await
            .expect("routed")
    };
    let delete = |(project, route): (Id, Id)| RouteChange {
        project_id: project,
        route_id: route,
        actor_id: lab.lena,
        via: "web".to_string(),
        at: OffsetDateTime::now_utc(),
    };
    assert!(routed().await, "control: both routes send to the room");
    lab.store
        .delete_route(&delete(routes[0]))
        .await
        .expect("delete the first route");
    assert!(
        routed().await,
        "the other project's route still sends to the room"
    );
    lab.store
        .delete_route(&delete(routes[1]))
        .await
        .expect("delete the last route");
    assert!(
        !routed().await,
        "no route sends to the room once both are deleted"
    );
}
