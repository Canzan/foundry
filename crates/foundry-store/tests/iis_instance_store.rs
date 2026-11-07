//! instance-integration-setup — store tests for migration 0033 and the
//! instance integrations store (ADR-IIS-001 r2 and r5, ADR-IIS-003 r1,
//! ADR-IIS-004 r1, r2, r3, r4 and r8, ADR-IIS-005).
//!
//! Bounded-change tests, example-based against real Postgres: each one dumps
//! every table a write could reach, applies one store call, and names exactly
//! the tables that may differ. The pool is pinned to a non-`public` schema, so
//! the search_path-scoped instance-integrations lock is the one exercised. The
//! store sees only wrapped and sealed bytes; the tests' "seals" are opaque
//! byte strings.

use foundry_store::integrations::{
    ConnectionWriteOutcome, KeyCheck, KeyWrapper, NewConnection, NewSecretsKey, ReplicaState,
    RewrapOutcome, RotateOutcome, SealedSlot, SecretsKeyOutcome, SecretsKeyRewrap, SlackAppSave,
    SlackSaveOutcome, INSTANCE_INTEGRATIONS_LOCK,
};
use foundry_store::{run_migrations, run_migrations_from_dir, Store};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Connection, PgPool};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};
use time::OffsetDateTime;

type Id = uuid::Uuid;

const SCHEMA: &str = "iis";

/// Every table an instance integrations write could reach, plus the
/// workspace-level neighbours it must never touch.
const OBSERVED: [&str; 10] = [
    "integration_secrets_key",
    "instance_slack_app",
    "integration_config_version",
    "integration_replica_states",
    "instance_integration_audit_events",
    "integration_key_checks",
    "integration_connections",
    "integration_audit_events",
    "integration_worker_heartbeats",
    "workspaces",
];

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
    async fn pool(&self) -> PgPool {
        let opts = PgConnectOptions::from_str(&self.base)
            .expect("parse url")
            .options([("search_path", SCHEMA)]);
        PgPoolOptions::new()
            .max_connections(10)
            .acquire_timeout(Duration::from_secs(20))
            .connect_with(opts)
            .await
            .expect("connect pool")
    }

    async fn migrated(&self) -> PgPool {
        let pool = self.pool().await;
        run_migrations(&pool).await.expect("migrate to head");
        pool
    }
}

/// A private copy of the production migrations up to and including `through`.
fn staged(through: &str) -> PathBuf {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let dir = std::env::temp_dir().join(format!("foundry-iis-{}", Id::now_v7()));
    std::fs::create_dir_all(&dir).expect("stage dir");
    for entry in std::fs::read_dir(&source).expect("read migrations") {
        let name = entry
            .expect("entry")
            .file_name()
            .to_string_lossy()
            .into_owned();
        if name.ends_with(".sql") && name.get(..4).is_some_and(|v| v <= through) {
            std::fs::copy(source.join(&name), dir.join(&name)).expect("stage file");
        }
    }
    dir
}

/// A fixed instant far from the wall clock, so a store that read `now()`
/// instead of the caller's time is caught.
fn at(minutes: i64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(978_307_200 + minutes * 60).expect("instant")
}

type Dump = BTreeMap<&'static str, Vec<String>>;

/// Every observed table's rows as JSON text, in a stable order.
async fn dump(pool: &PgPool) -> Dump {
    let mut out = Dump::new();
    for table in OBSERVED {
        let rows: Vec<String> = sqlx::query_scalar(&format!(
            "SELECT row_to_json(t)::text FROM {table} t ORDER BY 1"
        ))
        .fetch_all(pool)
        .await
        .unwrap_or_else(|e| panic!("dump {table}: {e}"));
        out.insert(table, rows);
    }
    out
}

fn changed(before: &Dump, after: &Dump) -> BTreeSet<&'static str> {
    OBSERVED
        .into_iter()
        .filter(|table| before[table] != after[table])
        .collect()
}

async fn workspace(pool: &PgPool) -> Id {
    let ws = Id::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'Acme Robotics')")
        .bind(ws)
        .execute(pool)
        .await
        .expect("workspace");
    ws
}

async fn admin(pool: &PgPool) -> Id {
    let id = Id::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, 'ana@example.com', 'ana@example.com', 'Ana', 'not-a-real-hash')",
    )
    .bind(id)
    .execute(pool)
    .await
    .expect("user");
    id
}

/// A connected connection holding `sealed` as its credential.
async fn connection(pool: &PgPool, ws: Id, kind: &str, sealed: &[u8]) -> Id {
    let id = Id::now_v7();
    sqlx::query(
        "INSERT INTO integration_connections
                (id, workspace_id, kind, state, external_id, label,
                 sealed_credential, credential_saved_at)
         VALUES ($1, $2, $3, 'connected', 'X1', 'Acme', $4, now())",
    )
    .bind(id)
    .bind(ws)
    .bind(kind)
    .bind(sealed)
    .execute(pool)
    .await
    .expect("connection");
    id
}

async fn sealed_credentials(pool: &PgPool) -> BTreeMap<Id, Vec<u8>> {
    let rows: Vec<(Id, Vec<u8>)> =
        sqlx::query_as("SELECT id, sealed_credential FROM integration_connections ORDER BY id")
            .fetch_all(pool)
            .await
            .expect("sealed credentials");
    rows.into_iter().collect()
}

async fn activity(pool: &PgPool) -> Vec<(Option<Id>, String, Option<String>, Option<i32>)> {
    sqlx::query_as(
        "SELECT actor_id, action, client_id, count FROM instance_integration_audit_events
          ORDER BY created_at, id",
    )
    .fetch_all(pool)
    .await
    .expect("activity")
}

fn new_key(key_id: &str, created_by: Option<Id>) -> NewSecretsKey {
    NewSecretsKey {
        key_id: key_id.to_string(),
        wrapped_key: format!("wrapped:{key_id}").into_bytes(),
        wrapped_by_key_id: "ik-1".to_string(),
        wrapped_by: KeyWrapper::File,
        created_by,
    }
}

fn canary(key_id: &str) -> KeyCheck {
    KeyCheck {
        key_id: key_id.to_string(),
        sealed_canary: format!("canary:{key_id}").into_bytes(),
    }
}

fn reseal_all(_: SealedSlot, sealed: &[u8]) -> Result<Option<Vec<u8>>, String> {
    Ok(Some([b"sk:".as_slice(), sealed].concat()))
}

fn slack_app(client_id: &str, sealed_under: Option<&str>, saved_by: Id) -> SlackAppSave {
    SlackAppSave {
        client_id: client_id.to_string(),
        sealed_client_secret: b"sealed-client-secret".to_vec(),
        sealed_signing_secret: b"sealed-signing-secret".to_vec(),
        sealed_under: sealed_under.map(str::to_string),
        saved_by,
        saved_at: at(5),
    }
}

// ---------------------------------------------------------------------------
// Migration 0033
// ---------------------------------------------------------------------------

/// 0033 seeds the version row at zero, and the probe refuses readiness until
/// 0033's tables exist, naming the migration.
#[tokio::test]
async fn migration_0033_seeds_the_version_and_the_probe_asks_for_it() {
    let pg = postgres().await;
    let pool = pg.pool().await;
    let store = Store::from_pool(pool.clone());
    run_migrations_from_dir(&pool, &staged("0032"))
        .await
        .expect("stand at 0032");
    let err = store
        .probe()
        .await
        .expect_err("not ready without the instance integration tables");
    assert!(err.to_string().contains("migration-0033"), "{err}");
    run_migrations(&pool).await.expect("head");
    store.probe().await.expect("ready at 0033");
    assert_eq!(store.config_version().await.expect("version"), 0);
}

/// A second current secrets key, a second Slack app or version row, and an
/// action outside the closed list are non-representable.
#[tokio::test]
async fn single_rows_and_the_closed_action_list_are_enforced() {
    let pg = postgres().await;
    let pool = pg.migrated().await;
    let store = Store::from_pool(pool.clone());
    let outcome = store
        .store_secrets_key(&new_key("sk-1", None), &canary("sk-1"), at(0), reseal_all)
        .await
        .expect("generate");
    assert!(matches!(outcome, SecretsKeyOutcome::Stored { .. }));
    for refused in [
        "INSERT INTO integration_secrets_key
              (id, key_id, wrapped_key, wrapped_by_key_id, wrapped_by_source, created_at)
         VALUES (2, 'sk-2', '\\x00', 'ik-1', 'file', now())",
        "INSERT INTO integration_secrets_key
              (key_id, wrapped_key, wrapped_by_key_id, wrapped_by_source, created_at)
         VALUES ('sk-2', '\\x00', 'ik-1', 'file', now())",
        "UPDATE integration_secrets_key SET wrapped_by_source = 'vault'",
        "INSERT INTO instance_slack_app
              (id, client_id, sealed_client_secret, sealed_signing_secret, saved_at)
         VALUES (2, '1.2', '\\x00', '\\x00', now())",
        "INSERT INTO integration_config_version (id, version) VALUES (2, 0)",
        "INSERT INTO instance_integration_audit_events (id, action, created_at)
         VALUES (gen_random_uuid(), 'secrets_rotated', now())",
    ] {
        assert!(
            sqlx::query(refused).execute(&pool).await.is_err(),
            "refused: {refused}"
        );
    }
    let stored: i64 = sqlx::query_scalar("SELECT count(*) FROM integration_secrets_key")
        .fetch_one(&pool)
        .await
        .expect("count");
    assert_eq!(stored, 1);
}

// ---------------------------------------------------------------------------
// Generate and Move (ADR-IIS-001 r5, ADR-IIS-004 r1 and r3)
// ---------------------------------------------------------------------------

/// Generate (nothing to re-seal): the key row, its canary, one "generated"
/// line by its actor and one version bump; nothing else changes.
#[tokio::test]
async fn generate_stores_the_key_canary_activity_and_version_only() {
    let pg = postgres().await;
    let pool = pg.migrated().await;
    let store = Store::from_pool(pool.clone());
    let ana = admin(&pool).await;
    let before = dump(&pool).await;

    let outcome = store
        .store_secrets_key(
            &new_key("sk-1", Some(ana)),
            &canary("sk-1"),
            at(0),
            |_, _| Ok::<_, String>(None),
        )
        .await
        .expect("generate");

    assert!(
        matches!(
            outcome,
            SecretsKeyOutcome::Stored {
                resealed: 0,
                version: 1
            }
        ),
        "{outcome:?}"
    );
    let after = dump(&pool).await;
    assert_eq!(
        changed(&before, &after),
        BTreeSet::from([
            "integration_secrets_key",
            "integration_key_checks",
            "instance_integration_audit_events",
            "integration_config_version",
        ])
    );
    let snapshot = store.instance_snapshot().await.expect("snapshot");
    let key = snapshot.secrets_key.expect("a stored key");
    assert_eq!(key.key_id, "sk-1");
    assert_eq!(key.wrapped_key, b"wrapped:sk-1");
    assert_eq!(key.wrapped_by_key_id, "ik-1");
    assert_eq!(key.wrapped_by, KeyWrapper::File);
    assert_eq!(key.rewrapped_from_path, None);
    assert_eq!(key.created_by, Some(ana));
    assert_eq!(key.created_at, at(0));
    assert!(snapshot
        .key_checks
        .iter()
        .any(|check| check == &canary("sk-1")));
    assert_eq!(
        activity(&pool).await,
        vec![(Some(ana), "secrets_key_generated".to_string(), None, None)]
    );
    assert_eq!(store.config_version().await.expect("version"), 1);
}

/// Move: every sealed value is handed to the caller and what it returns is
/// stored; values it keeps stay as they were; the outcome and the line count
/// re-sealed credentials (I-22): each connection one, the saved app one.
#[tokio::test]
async fn move_reseals_every_sealed_value_in_the_same_transaction() {
    let pg = postgres().await;
    let pool = pg.migrated().await;
    let store = Store::from_pool(pool.clone());
    let ws = workspace(&pool).await;
    let ana = admin(&pool).await;
    let slack = connection(&pool, ws, "slack", b"direct-slack").await;
    let matrix = connection(&pool, ws, "matrix", b"direct-matrix").await;
    store
        .save_slack_app(&slack_app("1.2", None, ana))
        .await
        .expect("save in direct mode");
    let before = dump(&pool).await;
    let mut offered = Vec::new();

    let outcome = store
        .store_secrets_key(
            &new_key("sk-1", None),
            &canary("sk-1"),
            at(9),
            |slot, sealed| {
                offered.push(slot);
                if slot == SealedSlot::Connection(matrix) {
                    return Ok::<_, String>(None);
                }
                reseal_all(slot, sealed)
            },
        )
        .await
        .expect("move");

    assert!(
        matches!(
            outcome,
            SecretsKeyOutcome::Stored {
                resealed: 2,
                version: 2
            }
        ),
        "one connection and the app: {outcome:?}"
    );
    let mut connections = vec![
        SealedSlot::Connection(slack),
        SealedSlot::Connection(matrix),
    ];
    connections.sort();
    offered.sort();
    let mut expected = connections;
    expected.extend([
        SealedSlot::SlackClientSecret,
        SealedSlot::SlackSigningSecret,
    ]);
    expected.sort();
    assert_eq!(offered, expected, "every sealed value is offered once");
    let after = dump(&pool).await;
    assert_eq!(
        changed(&before, &after),
        BTreeSet::from([
            "integration_secrets_key",
            "instance_slack_app",
            "integration_connections",
            "integration_key_checks",
            "instance_integration_audit_events",
            "integration_config_version",
        ])
    );
    let credentials = sealed_credentials(&pool).await;
    assert_eq!(credentials[&slack], b"sk:direct-slack");
    assert_eq!(credentials[&matrix], b"direct-matrix");
    let app = store
        .instance_snapshot()
        .await
        .expect("snapshot")
        .slack_app
        .expect("saved app");
    assert_eq!(app.sealed_client_secret, b"sk:sealed-client-secret");
    assert_eq!(app.sealed_signing_secret, b"sk:sealed-signing-secret");
    assert_eq!(
        activity(&pool).await.last(),
        Some(&(None, "credentials_moved".to_string(), None, Some(2)))
    );
}

/// A Move whose re-seal refuses partway changes nothing at all (distill
/// upstream-issues item 11).
#[tokio::test]
async fn move_refused_partway_leaves_everything_unchanged() {
    let pg = postgres().await;
    let pool = pg.migrated().await;
    let store = Store::from_pool(pool.clone());
    let ws = workspace(&pool).await;
    let ana = admin(&pool).await;
    connection(&pool, ws, "slack", b"direct-slack").await;
    connection(&pool, ws, "matrix", b"direct-matrix").await;
    store
        .save_slack_app(&slack_app("1.2", None, ana))
        .await
        .expect("save in direct mode");
    let before = dump(&pool).await;
    let mut calls = 0;

    let outcome = store
        .store_secrets_key(
            &new_key("sk-1", None),
            &canary("sk-1"),
            at(9),
            |slot, sealed| {
                calls += 1;
                if calls == 2 {
                    return Err("does not open".to_string());
                }
                reseal_all(slot, sealed)
            },
        )
        .await
        .expect("move");

    assert!(
        matches!(&outcome, SecretsKeyOutcome::Refused(why) if why == "does not open"),
        "{outcome:?}"
    );
    assert_eq!(calls, 2, "the re-seal stops at the first refusal");
    assert_eq!(dump(&pool).await, before);
}

/// Create-if-absent: once a key is stored, another Generate stores nothing,
/// and two concurrent Generates store exactly one key, one line, one bump.
#[tokio::test]
async fn concurrent_generates_store_exactly_one_key() {
    let pg = postgres().await;
    let pool = pg.migrated().await;
    let first = Store::from_pool(pool.clone());
    let second = Store::from_pool(pool.clone());

    let (key_a, canary_a) = (new_key("sk-a", None), canary("sk-a"));
    let (key_b, canary_b) = (new_key("sk-b", None), canary("sk-b"));
    let (a, b) = tokio::join!(
        first.store_secrets_key(&key_a, &canary_a, at(0), reseal_all),
        second.store_secrets_key(&key_b, &canary_b, at(0), reseal_all),
    );
    let outcomes = [a.expect("first"), b.expect("second")];

    let stored: Vec<&str> = outcomes
        .iter()
        .filter_map(|o| match o {
            SecretsKeyOutcome::Stored { .. } => Some("stored"),
            _ => None,
        })
        .collect();
    assert_eq!(stored.len(), 1, "{outcomes:?}");
    let winner = match (&outcomes[0], &outcomes[1]) {
        (SecretsKeyOutcome::Stored { .. }, SecretsKeyOutcome::AlreadyStored { key_id }) => {
            assert_eq!(key_id, "sk-a");
            "sk-a"
        }
        (SecretsKeyOutcome::AlreadyStored { key_id }, SecretsKeyOutcome::Stored { .. }) => {
            assert_eq!(key_id, "sk-b");
            "sk-b"
        }
        other => panic!("one stored, one already stored: {other:?}"),
    };
    let keys: Vec<String> = sqlx::query_scalar("SELECT key_id FROM integration_secrets_key")
        .fetch_all(&pool)
        .await
        .expect("keys");
    assert_eq!(keys, vec![winner.to_string()]);
    assert_eq!(activity(&pool).await.len(), 1);
    assert_eq!(first.config_version().await.expect("version"), 1);

    let before = dump(&pool).await;
    let again = first
        .store_secrets_key(&new_key("sk-c", None), &canary("sk-c"), at(1), reseal_all)
        .await
        .expect("generate again");
    assert!(
        matches!(&again, SecretsKeyOutcome::AlreadyStored { key_id } if key_id == winner),
        "{again:?}"
    );
    assert_eq!(dump(&pool).await, before);
}

// ---------------------------------------------------------------------------
// Re-wrap (ADR-IIS-002 r5) and doctor rotation (ADR-IIS-001 decision 8)
// ---------------------------------------------------------------------------

fn rewrap_of(from_key_id: &str, to_key_id: &str) -> SecretsKeyRewrap {
    SecretsKeyRewrap {
        key_id: "sk-a".to_string(),
        from_key_id: from_key_id.to_string(),
        wrapped_key: format!("wrapped-under:{to_key_id}").into_bytes(),
        wrapped_by_key_id: to_key_id.to_string(),
        wrapped_by: KeyWrapper::Operator,
        rewrapped_from_path: Some("/var/lib/foundry/instance.key".to_string()),
    }
}

/// Rotation whose re-wrap names a key the stored one is not wrapped under:
/// the compare-and-set matches no row, so the rotation is refused and
/// nothing changes (no re-seal, canary or version bump without the re-wrap).
#[tokio::test]
async fn rotation_whose_rewrap_matches_no_row_changes_nothing() {
    let pg = postgres().await;
    let pool = pg.migrated().await;
    let store = Store::from_pool(pool.clone());
    let ws = workspace(&pool).await;
    connection(&pool, ws, "slack", b"ik-1:direct").await;
    store
        .store_secrets_key(&new_key("sk-a", None), &canary("sk-a"), at(0), |_, _| {
            Ok::<_, String>(None)
        })
        .await
        .expect("generate");
    let before = dump(&pool).await;

    let outcome = store
        .rotate_instance_key(
            |_| Ok::<_, String>(Some(rewrap_of("ik-elsewhere", "op-2"))),
            |_, sealed| Ok(Some([b"sk-a:".as_slice(), sealed].concat())),
            &canary("op-2"),
        )
        .await
        .expect("rotate");
    assert_eq!(outcome, RotateOutcome::KeyChanged);
    assert_eq!(dump(&pool).await, before, "nothing changed");
}

/// Replicas racing the re-wrap off the key file: exactly one wins, records
/// Foundry's one system line and bumps the version once; the loser changes
/// nothing. The row then names the operator's key and the file it left.
#[tokio::test]
async fn replicas_racing_the_rewrap_record_exactly_one_system_line() {
    let pg = postgres().await;
    let pool = pg.migrated().await;
    let first = Store::from_pool(pool.clone());
    let second = Store::from_pool(pool.clone());
    first
        .store_secrets_key(&new_key("sk-a", None), &canary("sk-a"), at(0), reseal_all)
        .await
        .expect("generate");
    let before = dump(&pool).await;

    let rewrap = rewrap_of("ik-1", "op-1");
    let (a, b) = tokio::join!(
        first.rewrap_secrets_key(&rewrap, at(5)),
        second.rewrap_secrets_key(&rewrap, at(5)),
    );
    let mut outcomes = [a.expect("first"), b.expect("second")];
    outcomes.sort_by_key(|o| matches!(o, RewrapOutcome::AlreadyRewrapped));
    assert_eq!(
        outcomes,
        [
            RewrapOutcome::Rewrapped { version: 2 },
            RewrapOutcome::AlreadyRewrapped
        ]
    );
    assert_eq!(
        changed(&before, &dump(&pool).await),
        BTreeSet::from([
            "integration_secrets_key",
            "integration_config_version",
            "instance_integration_audit_events",
        ])
    );
    let lines = activity(&pool).await;
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(
        lines[1],
        (None, "secrets_key_rewrapped".to_string(), None, None)
    );
    let row: (Vec<u8>, String, String, Option<String>) = sqlx::query_as(
        "SELECT wrapped_key, wrapped_by_key_id, wrapped_by_source, rewrapped_from_path
           FROM integration_secrets_key",
    )
    .fetch_one(&pool)
    .await
    .expect("row");
    assert_eq!(
        row,
        (
            b"wrapped-under:op-1".to_vec(),
            "op-1".to_string(),
            "operator".to_string(),
            Some("/var/lib/foundry/instance.key".to_string())
        )
    );

    let settled = dump(&pool).await;
    let again = first
        .rewrap_secrets_key(&rewrap_of("ik-1", "op-1"), at(6))
        .await
        .expect("again");
    assert_eq!(again, RewrapOutcome::AlreadyRewrapped);
    assert_eq!(dump(&pool).await, settled);
}

/// Rotation re-wraps the stored key, re-seals what the caller returns, writes
/// the canary and bumps once, in one transaction; a refusal partway (after
/// the re-wrap) leaves everything as it was.
#[tokio::test]
async fn rotation_rewraps_and_reseals_all_or_nothing() {
    let pg = postgres().await;
    let pool = pg.migrated().await;
    let store = Store::from_pool(pool.clone());
    let ws = workspace(&pool).await;
    let direct = connection(&pool, ws, "slack", b"ik-1:direct").await;
    let under_secrets = connection(&pool, ws, "matrix", b"sk-a:sealed").await;
    store
        .store_secrets_key(&new_key("sk-a", None), &canary("sk-a"), at(0), |_, _| {
            Ok::<_, String>(None)
        })
        .await
        .expect("generate");
    let before = dump(&pool).await;

    let refused = store
        .rotate_instance_key(
            |_| Ok(Some(rewrap_of("ik-1", "op-2"))),
            |_, _| Err("cannot open"),
            &canary("op-2"),
        )
        .await
        .expect("rotate");
    assert_eq!(refused, RotateOutcome::Refused("cannot open"));
    assert_eq!(dump(&pool).await, before);

    let rotated = store
        .rotate_instance_key(
            |stored| {
                assert_eq!(stored.wrapped_by_key_id, "ik-1");
                Ok::<_, String>(Some(rewrap_of("ik-1", "op-2")))
            },
            |_, sealed| {
                Ok(sealed
                    .starts_with(b"ik-1:")
                    .then(|| [b"sk-a:".as_slice(), sealed].concat()))
            },
            &canary("op-2"),
        )
        .await
        .expect("rotate");
    assert_eq!(
        rotated,
        RotateOutcome::Rotated {
            rewrapped: true,
            resealed: 1,
            version: 2
        }
    );
    assert_eq!(
        changed(&before, &dump(&pool).await),
        BTreeSet::from([
            "integration_secrets_key",
            "integration_config_version",
            "integration_key_checks",
            "integration_connections",
        ])
    );
    let sealed = sealed_credentials(&pool).await;
    assert_eq!(sealed[&direct], b"sk-a:ik-1:direct".to_vec());
    assert_eq!(sealed[&under_secrets], b"sk-a:sealed".to_vec());
}

// ---------------------------------------------------------------------------
// Slack save (ADR-IIS-003 r1, ADR-IIS-004 r2)
// ---------------------------------------------------------------------------

/// A save sealed under the stored secrets key stores the app, one line naming
/// the client ID and one bump; a save sealed under another key is stale and
/// changes nothing.
#[tokio::test]
async fn slack_save_fences_on_the_sealing_key() {
    let pg = postgres().await;
    let pool = pg.migrated().await;
    let store = Store::from_pool(pool.clone());
    let ana = admin(&pool).await;
    store
        .store_secrets_key(&new_key("sk-1", None), &canary("sk-1"), at(0), reseal_all)
        .await
        .expect("generate");
    let before = dump(&pool).await;

    for stale in [None, Some("ik-1")] {
        let outcome = store
            .save_slack_app(&slack_app("1.2", stale, ana))
            .await
            .expect("stale save");
        assert_eq!(
            outcome,
            SlackSaveOutcome::StaleKey,
            "sealed under {stale:?}"
        );
        assert_eq!(dump(&pool).await, before);
    }

    let outcome = store
        .save_slack_app(&slack_app("1.2", Some("sk-1"), ana))
        .await
        .expect("save");
    assert_eq!(outcome, SlackSaveOutcome::Saved { version: 2 });
    let after = dump(&pool).await;
    assert_eq!(
        changed(&before, &after),
        BTreeSet::from([
            "instance_slack_app",
            "instance_integration_audit_events",
            "integration_config_version",
        ])
    );
    let app = store
        .instance_snapshot()
        .await
        .expect("snapshot")
        .slack_app
        .expect("saved app");
    assert_eq!(app.client_id, "1.2");
    assert_eq!(app.sealed_client_secret, b"sealed-client-secret");
    assert_eq!(app.sealed_signing_secret, b"sealed-signing-secret");
    assert_eq!(app.saved_by, Some(ana));
    assert_eq!(app.saved_at, at(5));
    assert_eq!(
        activity(&pool).await.last(),
        Some(&(
            Some(ana),
            "slack_app_saved".to_string(),
            Some("1.2".to_string()),
            None
        ))
    );

    store
        .save_slack_app(&slack_app("3.4", Some("sk-1"), ana))
        .await
        .expect("save over");
    assert_eq!(
        activity(&pool).await.last().map(|line| line.1.as_str()),
        Some("slack_app_replaced")
    );
}

// ---------------------------------------------------------------------------
// Connection writes: the seal fence (ADR-IIS-004 r2)
// ---------------------------------------------------------------------------

/// A Slack connection for the workspace `ws`, sealed under `sealed_under`.
fn slack_connection(ws: Id, by: Id, sealed_under: Option<&str>) -> NewConnection {
    NewConnection {
        id: Id::now_v7(),
        workspace_id: ws,
        kind: "slack".to_string(),
        external_id: "T1".to_string(),
        app_id: Some("A1".to_string()),
        bot_user_id: Some("U1".to_string()),
        label: "Canzan Labs".to_string(),
        detail: Some("canzan-labs".to_string()),
        sealed_credential: b"sealed-bot-token".to_vec(),
        sealed_under: sealed_under.map(str::to_string),
        connected_by: by,
        connected_at: at(5),
    }
}

/// A connection write sealed under anything but the sealing key — a key id
/// while none is stored, then no key id or the instance key's once one is —
/// is refused in its transaction and changes nothing; sealed under the
/// stored secrets key, it writes the connection and its audit row only.
#[tokio::test]
async fn connection_write_fences_on_the_sealing_key() {
    let pg = postgres().await;
    let pool = pg.migrated().await;
    let store = Store::from_pool(pool.clone());
    let (ws, ana) = (workspace(&pool).await, admin(&pool).await);

    let before = dump(&pool).await;
    let outcome = store
        .upsert_connection(&slack_connection(ws, ana, Some("sk-1")))
        .await
        .expect("stale connect");
    assert_eq!(
        outcome,
        ConnectionWriteOutcome::StaleKey,
        "no key stored yet"
    );
    assert_eq!(dump(&pool).await, before);

    store
        .store_secrets_key(&new_key("sk-1", None), &canary("sk-1"), at(0), reseal_all)
        .await
        .expect("generate");
    let before = dump(&pool).await;
    for stale in [None, Some("ik-1")] {
        let outcome = store
            .upsert_connection(&slack_connection(ws, ana, stale))
            .await
            .expect("stale connect");
        assert_eq!(
            outcome,
            ConnectionWriteOutcome::StaleKey,
            "sealed under {stale:?}"
        );
        assert_eq!(dump(&pool).await, before);
    }

    let connection = slack_connection(ws, ana, Some("sk-1"));
    let outcome = store.upsert_connection(&connection).await.expect("connect");
    assert_eq!(outcome, ConnectionWriteOutcome::Connected);
    assert_eq!(
        changed(&before, &dump(&pool).await),
        BTreeSet::from(["integration_connections", "integration_audit_events"])
    );
    assert_eq!(
        sealed_credentials(&pool).await,
        BTreeMap::from([(connection.id, b"sealed-bot-token".to_vec())])
    );
}

/// The connection write takes the instance-integrations lock shared: it
/// waits while an instance-level write holds the lock exclusive, and goes
/// on once that write ends.
#[tokio::test]
async fn connection_write_waits_for_an_instance_write() {
    let pg = postgres().await;
    let pool = pg.migrated().await;
    let store = Store::from_pool(pool.clone());
    let (ws, ana) = (workspace(&pool).await, admin(&pool).await);

    let mut instance_write = pool.begin().await.expect("begin");
    sqlx::query(
        "SELECT pg_advisory_xact_lock(
                    hashtextextended($1 || '|' || current_setting('search_path'), 0))",
    )
    .bind(INSTANCE_INTEGRATIONS_LOCK)
    .execute(&mut *instance_write)
    .await
    .expect("hold the lock exclusive");
    let connect = tokio::spawn(async move {
        store
            .upsert_connection(&slack_connection(ws, ana, None))
            .await
    });
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert!(
        !connect.is_finished(),
        "the connection write waits for the lock"
    );
    assert!(sealed_credentials(&pool).await.is_empty());

    instance_write.commit().await.expect("release the lock");
    let outcome = tokio::time::timeout(Duration::from_secs(20), connect)
        .await
        .expect("the connection write goes on")
        .expect("join")
        .expect("connect");
    assert_eq!(outcome, ConnectionWriteOutcome::Connected);
}

// ---------------------------------------------------------------------------
// Reload snapshot and replica state (ADR-IIS-004 r4 and r8)
// ---------------------------------------------------------------------------

/// A write committed after the snapshot's version read is absent from that
/// snapshot and its version, so the next snapshot carries both.
#[tokio::test]
async fn snapshot_is_consistent_with_the_version_it_reports() {
    let pg = postgres().await;
    let pool = pg.migrated().await;
    let store = Store::from_pool(pool.clone());
    let ana = admin(&pool).await;
    let ws = workspace(&pool).await;
    connection(&pool, ws, "matrix", b"direct-matrix").await;
    let writer = Store::from_pool(pool.clone());

    let snapshot = store
        .instance_snapshot_with(|version| async move {
            assert_eq!(version, 0);
            let saved = writer
                .save_slack_app(&slack_app("1.2", None, ana))
                .await
                .expect("a save lands between the reads");
            assert_eq!(saved, SlackSaveOutcome::Saved { version: 1 });
        })
        .await
        .expect("snapshot");

    assert_eq!(snapshot.version, 0);
    assert!(snapshot.slack_app.is_none(), "the late write is not seen");
    assert!(snapshot.secrets_key.is_none());
    assert_eq!(snapshot.sealed_credentials, vec![b"direct-matrix".to_vec()]);
    let next = store.instance_snapshot().await.expect("next snapshot");
    assert_eq!(next.version, 1);
    assert_eq!(next.slack_app.expect("now seen").client_id, "1.2");
}

/// A reload (snapshot, then its own replica-state beat) writes only its own
/// row, stamped with the caller's time.
#[tokio::test]
async fn reload_writes_only_its_own_replica_state() {
    let pg = postgres().await;
    let pool = pg.migrated().await;
    let store = Store::from_pool(pool.clone());
    let other = ReplicaState {
        replica: "replica-b".to_string(),
        applied_version: 0,
        key_mode: "direct".to_string(),
        opens_secrets_key: false,
        beat_at: at(0),
    };
    store.beat_replica_state(&other).await.expect("b beats");
    store
        .beat_replica_state(&ReplicaState {
            replica: "replica-a".to_string(),
            ..other.clone()
        })
        .await
        .expect("a beats");
    store
        .store_secrets_key(&new_key("sk-1", None), &canary("sk-1"), at(0), reseal_all)
        .await
        .expect("generate");
    let before = dump(&pool).await;

    let snapshot = store.instance_snapshot().await.expect("snapshot");
    let mine = ReplicaState {
        replica: "replica-a".to_string(),
        applied_version: snapshot.version,
        key_mode: "sealed".to_string(),
        opens_secrets_key: true,
        beat_at: at(7),
    };
    store.beat_replica_state(&mine).await.expect("a applies");

    let after = dump(&pool).await;
    assert_eq!(
        changed(&before, &after),
        BTreeSet::from(["integration_replica_states"])
    );
    assert_eq!(
        store.replica_states().await.expect("states"),
        vec![mine, other]
    );
}

/// The prune deletes exactly the rows that beat before the caller's cutoff.
#[tokio::test]
async fn prune_uses_the_callers_time() {
    let pg = postgres().await;
    let pool = pg.migrated().await;
    let store = Store::from_pool(pool.clone());
    let state = |replica: &str, beat: i64| ReplicaState {
        replica: replica.to_string(),
        applied_version: 0,
        key_mode: "direct".to_string(),
        opens_secrets_key: false,
        beat_at: at(beat),
    };
    for (replica, beat) in [("stale", 0), ("edge", 60), ("fresh", 90)] {
        store
            .beat_replica_state(&state(replica, beat))
            .await
            .expect("beat");
    }

    let pruned = store.prune_replica_states(at(60)).await.expect("prune");

    assert_eq!(pruned, 1);
    assert_eq!(
        store.replica_states().await.expect("states"),
        vec![state("edge", 60), state("fresh", 90)]
    );
}
