//! instance-admin-workspace-rename (D5, D10; DESIGN DDD-2/3/5/11,
//! ADR-WORKSPACE-RENAME-001): a workspace rename and its record succeed or fail
//! together. Migration 0018 adds `workspace_rename_events` (workspace FK ON DELETE
//! CASCADE, actor FK with no ON DELETE action, `CHECK (old_name <> new_name)`,
//! index `(workspace_id, created_at)`); `Store::rename_workspace_with_audit` locks
//! the row, compares, updates and appends in ONE transaction, reading `old_name`
//! under the lock; `Store::probe` refuses a schema without the record.
//!
//! SCAFFOLD: true — DISTILL 2026-10-05 (DESIGN OQ-D2; DoD 2 and 5). Every test
//! here is `#[ignore]`d until DELIVER writes 0018, the store method and the probe
//! check. They compile today because they reach the new table through SQL only,
//! and the new store method through [`rename_workspace_with_audit`], a shim that
//! panics. DELIVER: replace the shim's body with a call to
//! `store.rename_workspace_with_audit(workspace_id, actor_id, new_name)` mapping
//! `WorkspaceRenameWrite` onto [`Write`] (or swap [`Write`] for the real enum),
//! and remove the `#[ignore]`s one at a time.
//!
//! Not reachable over HTTP: the actor is always a real session user, so the
//! forced audit-write failure (a non-existent `actor_id`) and the lock
//! interleaving live here, not in the acceptance lane (OQ-D2).
//!
//! WHY-NEW-FILE: crates/foundry-store/tests/workspace_rename_with_audit.rs
//!   CLOSEST-EXISTING: crates/foundry-store/tests/reposition_issue.rs
//!   EXTENSION-COST: that file pins issue ordering inside its transaction; this
//!     pins a different table, a different migration and an audit append.
//!   PARALLEL-RATIONALE: "a move keeps positions contiguous" and "a rename and its
//!     record commit together" are unrelated properties.
//!
//! Real Postgres (testcontainers): the transaction, the row lock, the FK fault and
//! the CHECK cannot be faked. Integration-level, example-based (Mandates 9/11).

use foundry_store::{run_migrations, run_migrations_from_dir, Store};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};

const LEGACY_NAME: &str = "Canzan Labs Platform Engineering and Site Reliability";

/// The outcome DESIGN pins for the store write (`WorkspaceRenameWrite`).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Write {
    Renamed { old_name: String },
    Unchanged,
    NotFound,
}

/// SCAFFOLD: stands in for `Store::rename_workspace_with_audit` (DDD-2), which
/// does not exist yet. `Err` carries the store error's text.
async fn rename_workspace_with_audit(
    _store: &Store,
    _workspace_id: uuid::Uuid,
    _actor_id: uuid::Uuid,
    _new_name: &str,
) -> Result<Write, String> {
    panic!(
        "SCAFFOLD: Store::rename_workspace_with_audit (DDD-2) not yet implemented -- RED scaffold"
    )
}

fn production_migrations_dir() -> PathBuf {
    let manifest =
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is always set by cargo");
    PathBuf::from(manifest).join("migrations")
}

async fn empty_pool() -> (PgPool, ContainerAsync<Postgres>) {
    let container = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("start postgres container");
    let host = container.get_host().await.expect("container host");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("container port");
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&format!(
            "postgres://postgres:postgres@{host}:{port}/postgres"
        ))
        .await
        .expect("connect pool");
    (pool, container)
}

async fn migrated() -> (Store, PgPool, ContainerAsync<Postgres>) {
    let (pool, container) = empty_pool().await;
    run_migrations(&pool).await.expect("apply every migration");
    (Store::from_pool(pool.clone()), pool, container)
}

/// A private copy of the production migrations up to and including version
/// `through`, so a schema can stand at a past version before moving forward.
fn staged_migrations(through: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("foundry-0018-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&dir).expect("stage dir");
    let mut names: Vec<String> = std::fs::read_dir(production_migrations_dir())
        .expect("read migrations")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".sql") && n.get(..4).is_some_and(|v| v <= through))
        .collect();
    names.sort();
    for n in names {
        std::fs::copy(production_migrations_dir().join(&n), dir.join(&n)).expect("stage file");
    }
    dir
}

async fn insert_workspace(pool: &PgPool, name: &str) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(id)
        .bind(name)
        .execute(pool)
        .await
        .expect("insert workspace");
    id
}

async fn insert_user(pool: &PgPool, email: &str) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, 'Priya Raman', 'not-a-real-hash')",
    )
    .bind(id)
    .bind(email)
    .execute(pool)
    .await
    .expect("insert user");
    id
}

async fn workspace_names(pool: &PgPool) -> BTreeMap<uuid::Uuid, String> {
    sqlx::query_as::<_, (uuid::Uuid, String)>("SELECT id, name FROM workspaces")
        .fetch_all(pool)
        .await
        .expect("read workspaces")
        .into_iter()
        .collect()
}

/// `(workspace_id, actor_id, old_name, new_name)` of every record, oldest first.
async fn records(pool: &PgPool) -> Vec<(uuid::Uuid, uuid::Uuid, String, String)> {
    sqlx::query_as(
        "SELECT workspace_id, actor_id, old_name, new_name
           FROM workspace_rename_events ORDER BY created_at, id",
    )
    .fetch_all(pool)
    .await
    .expect("read workspace_rename_events (0018 creates it)")
}

/// Fixture: Priya, "Bailey Family" (renamed) and "Canzan Labs" (bystander).
struct Fixture {
    store: Store,
    pool: PgPool,
    _container: ContainerAsync<Postgres>,
    priya: uuid::Uuid,
    bailey: uuid::Uuid,
}

async fn fixture() -> Fixture {
    let (store, pool, container) = migrated().await;
    let priya = insert_user(&pool, "priya@canzan.test").await;
    let bailey = insert_workspace(&pool, "Bailey Family").await;
    insert_workspace(&pool, "Canzan Labs").await;
    Fixture {
        store,
        pool,
        _container: container,
        priya,
        bailey,
    }
}

// ---------------------------------------------------------------------------
// Migration 0018 (DDD-5, DoD 5)
// ---------------------------------------------------------------------------

/// DoD 5: 0018 applies forward over a database that already holds workspaces —
/// including a name longer than the 24-character rule (D10: no CHECK on
/// `workspaces.name`, so legacy names stay) — rewrites none of them, starts the
/// record empty, and re-applying the set is a no-op.
#[tokio::test]
#[ignore = "SCAFFOLD: DELIVER writes migration 0018_workspace_rename_events"]
async fn migration_0018_applies_cleanly_over_existing_workspaces_including_legacy_long_names() {
    let (pool, _container) = empty_pool().await;
    run_migrations_from_dir(&pool, &staged_migrations("0017"))
        .await
        .expect("stand the schema at 0017");
    insert_workspace(&pool, "Bailey Family").await;
    insert_workspace(&pool, LEGACY_NAME).await;
    let before = workspace_names(&pool).await;

    run_migrations(&pool).await.expect("0018 applies forward");
    run_migrations(&pool)
        .await
        .expect("re-applying the set is a no-op");

    assert_eq!(
        workspace_names(&pool).await,
        before,
        "0018 is additive: no workspace row is rewritten, legacy long names included"
    );
    assert!(
        records(&pool).await.is_empty(),
        "0018 back-fills nothing: the record starts empty"
    );
}

/// DDD-5: the record's shape — columns, the two foreign keys and their delete
/// actions, and the `(workspace_id, created_at)` index.
#[tokio::test]
#[ignore = "SCAFFOLD: DELIVER writes migration 0018_workspace_rename_events"]
async fn the_rename_record_has_the_designed_columns_keys_and_index() {
    let (_store, pool, _container) = migrated().await;
    let columns: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT column_name, data_type, is_nullable FROM information_schema.columns
          WHERE table_schema = current_schema() AND table_name = 'workspace_rename_events'
          ORDER BY column_name",
    )
    .fetch_all(&pool)
    .await
    .expect("read columns");
    let expected: Vec<(String, String, String)> = [
        ("actor_id", "uuid"),
        ("created_at", "timestamp with time zone"),
        ("id", "uuid"),
        ("new_name", "text"),
        ("old_name", "text"),
        ("workspace_id", "uuid"),
    ]
    .iter()
    .map(|(c, t)| (c.to_string(), t.to_string(), "NO".to_string()))
    .collect();
    assert_eq!(columns, expected, "columns per DDD-5, all NOT NULL");

    // confdeltype: 'c' = CASCADE, 'a' = NO ACTION.
    let fks: Vec<(String, String)> = sqlx::query_as(
        "SELECT confrelid::regclass::text, confdeltype::text FROM pg_constraint
          WHERE conrelid = 'workspace_rename_events'::regclass AND contype = 'f'
          ORDER BY 1",
    )
    .fetch_all(&pool)
    .await
    .expect("read foreign keys");
    assert_eq!(
        fks,
        vec![
            ("users".to_string(), "a".to_string()),
            ("workspaces".to_string(), "c".to_string())
        ],
        "the record lives and dies with its workspace; an actor on record cannot be deleted"
    );

    let (indexed,): (bool,) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM pg_indexes
          WHERE schemaname = current_schema() AND tablename = 'workspace_rename_events'
            AND indexdef LIKE '%(workspace_id, created_at)%')",
    )
    .fetch_one(&pool)
    .await
    .expect("read indexes");
    assert!(indexed, "index (workspace_id, created_at) per DDD-5");
}

/// D4 in the schema: a record whose old and new names are equal is refused by
/// `CHECK (old_name <> new_name)` (SQLSTATE 23514), even written by hand.
#[tokio::test]
#[ignore = "SCAFFOLD: DELIVER writes migration 0018_workspace_rename_events"]
async fn the_rename_record_refuses_an_entry_that_changes_nothing() {
    let f = fixture().await;
    let err = sqlx::query(
        "INSERT INTO workspace_rename_events (id, workspace_id, actor_id, old_name, new_name)
              VALUES ($1, $2, $3, 'Household', 'Household')",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(f.bailey)
    .bind(f.priya)
    .execute(&f.pool)
    .await
    .expect_err("a no-op entry must be refused by the schema");
    let code = err
        .as_database_error()
        .and_then(|e| e.code())
        .map(|c| c.into_owned());
    assert_eq!(
        code.as_deref(),
        Some("23514"),
        "refused by the CHECK, not by anything else: {err}"
    );
    let (case_only_ok,) : (i64,) = sqlx::query_as(
        "WITH ins AS (INSERT INTO workspace_rename_events (id, workspace_id, actor_id, old_name, new_name)
              VALUES ($1, $2, $3, 'Household', 'household') RETURNING 1) SELECT count(*) FROM ins",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(f.bailey)
    .bind(f.priya)
    .fetch_one(&f.pool)
    .await
    .expect("a case-only change is a real change (D4)");
    assert_eq!(case_only_ok, 1);
}

// ---------------------------------------------------------------------------
// The atomic write (DDD-2, DDD-3; bounded change)
// ---------------------------------------------------------------------------

/// Bounded change: an effective rename moves exactly one name and appends exactly
/// one record — the old name from the row, the new name, the actor — and every
/// other workspace is byte-identical.
#[tokio::test]
#[ignore = "SCAFFOLD: DELIVER writes Store::rename_workspace_with_audit and 0018"]
async fn an_effective_rename_changes_one_name_and_appends_one_record() {
    let f = fixture().await;
    let before = workspace_names(&f.pool).await;

    let outcome = rename_workspace_with_audit(&f.store, f.bailey, f.priya, "Household").await;

    assert_eq!(
        outcome,
        Ok(Write::Renamed {
            old_name: "Bailey Family".to_string()
        })
    );
    let mut expected = before.clone();
    expected.insert(f.bailey, "Household".to_string());
    assert_eq!(
        workspace_names(&f.pool).await,
        expected,
        "only the renamed name moves"
    );
    assert_eq!(
        records(&f.pool).await,
        vec![(
            f.bailey,
            f.priya,
            "Bailey Family".to_string(),
            "Household".to_string()
        )],
        "exactly one record, naming who renamed what, from what, to what"
    );
}

/// D4: the same name writes nothing — no UPDATE, no record — while a case-only
/// change is a real rename.
#[tokio::test]
#[ignore = "SCAFFOLD: DELIVER writes Store::rename_workspace_with_audit and 0018"]
async fn the_same_name_writes_nothing_and_a_case_change_is_a_rename() {
    let f = fixture().await;
    let before = workspace_names(&f.pool).await;

    let same = rename_workspace_with_audit(&f.store, f.bailey, f.priya, "Bailey Family").await;
    assert_eq!(same, Ok(Write::Unchanged));
    assert_eq!(workspace_names(&f.pool).await, before);
    assert!(records(&f.pool).await.is_empty(), "a no-op records nothing");

    let case = rename_workspace_with_audit(&f.store, f.bailey, f.priya, "bailey family").await;
    assert_eq!(
        case,
        Ok(Write::Renamed {
            old_name: "Bailey Family".to_string()
        })
    );
    assert_eq!(records(&f.pool).await.len(), 1);
}

/// An unknown workspace writes nothing anywhere.
#[tokio::test]
#[ignore = "SCAFFOLD: DELIVER writes Store::rename_workspace_with_audit and 0018"]
async fn an_unknown_workspace_writes_nothing() {
    let f = fixture().await;
    let before = workspace_names(&f.pool).await;
    let outcome =
        rename_workspace_with_audit(&f.store, uuid::Uuid::now_v7(), f.priya, "Household").await;
    assert_eq!(outcome, Ok(Write::NotFound));
    assert_eq!(workspace_names(&f.pool).await, before);
    assert!(records(&f.pool).await.is_empty());
}

/// OQ-D2 / DoD 2: a forced audit-write failure — an actor that does not exist
/// fails the INSERT after the UPDATE — rolls the whole rename back: the name is
/// unchanged and there is no record. A rename without its record never commits.
#[tokio::test]
#[ignore = "SCAFFOLD: DELIVER writes Store::rename_workspace_with_audit and 0018"]
async fn a_failed_record_write_leaves_the_name_unchanged_and_nothing_on_record() {
    let f = fixture().await;
    let before = workspace_names(&f.pool).await;
    let ghost_actor = uuid::Uuid::now_v7();

    let outcome = rename_workspace_with_audit(&f.store, f.bailey, ghost_actor, "Household").await;

    assert!(
        outcome.is_err(),
        "a record that cannot be written fails the rename; got {outcome:?}"
    );
    assert_eq!(
        workspace_names(&f.pool).await,
        before,
        "the name must be rolled back with the failed record (D5)"
    );
    assert!(records(&f.pool).await.is_empty(), "no record either");
}

/// DDD-3: the old name is read UNDER the row lock. Another transaction holds the
/// workspace row and renames it to "Kitchen" while our rename to "Household" is
/// in flight; our rename must wait for it, then record "Kitchen" -> "Household".
/// An implementation that reads the old name before taking the lock records
/// "Bailey Family" -> "Household" (a record that never matched the row), and one
/// that does not lock at all finishes before the holder commits.
#[tokio::test]
#[ignore = "SCAFFOLD: DELIVER writes Store::rename_workspace_with_audit and 0018"]
async fn the_old_name_on_record_is_the_one_read_under_the_lock() {
    let f = fixture().await;
    let mut holder = f.pool.begin().await.expect("begin holder");
    sqlx::query("SELECT name FROM workspaces WHERE id = $1 FOR UPDATE")
        .bind(f.bailey)
        .fetch_one(&mut *holder)
        .await
        .expect("holder locks the row");
    sqlx::query("UPDATE workspaces SET name = 'Kitchen' WHERE id = $1")
        .bind(f.bailey)
        .execute(&mut *holder)
        .await
        .expect("holder renames under its lock");

    let store = f.store.clone();
    let (bailey, priya) = (f.bailey, f.priya);
    let rename = tokio::spawn(async move {
        rename_workspace_with_audit(&store, bailey, priya, "Household").await
    });
    tokio::time::sleep(Duration::from_millis(500)).await;
    if rename.is_finished() {
        let early = rename.await;
        panic!(
            "the rename must wait for the row lock the holder has; it finished early: {early:?}"
        );
    }
    holder.commit().await.expect("holder commits");

    let outcome = rename.await.expect("join rename");
    assert_eq!(
        outcome,
        Ok(Write::Renamed {
            old_name: "Kitchen".to_string()
        })
    );
    assert_eq!(
        records(&f.pool).await,
        vec![(
            f.bailey,
            f.priya,
            "Kitchen".to_string(),
            "Household".to_string()
        )],
        "the record names the name the row held when the rename took its lock"
    );
}

/// Two concurrent renames serialize: their records chain (the second's old name
/// is the first's new name) and the final name is the last record's new name,
/// whichever order they ran in.
#[tokio::test]
#[ignore = "SCAFFOLD: DELIVER writes Store::rename_workspace_with_audit and 0018"]
async fn concurrent_renames_serialize_into_a_chain_of_records() {
    let f = fixture().await;
    let (a, b) = tokio::join!(
        rename_workspace_with_audit(&f.store, f.bailey, f.priya, "Household"),
        rename_workspace_with_audit(&f.store, f.bailey, f.priya, "Kitchen"),
    );
    assert!(a.is_ok() && b.is_ok(), "both renames succeed: {a:?} {b:?}");
    let recs = records(&f.pool).await;
    assert_eq!(
        recs.len(),
        2,
        "two effective renames, two records: {recs:?}"
    );
    assert_eq!(
        recs[0].2, "Bailey Family",
        "the first record starts from the original name"
    );
    assert_eq!(
        recs[1].2, recs[0].3,
        "the second record starts where the first ended"
    );
    assert_eq!(
        workspace_names(&f.pool).await[&f.bailey],
        recs[1].3,
        "the stored name is the last record's new name"
    );
}

// ---------------------------------------------------------------------------
// Readiness (DDD-11)
// ---------------------------------------------------------------------------

/// DDD-11: a schema missing the record fails the readiness probe instead of
/// turning the first rename into a 500.
#[tokio::test]
#[ignore = "SCAFFOLD: DELIVER extends Store::probe for 0018"]
async fn the_probe_refuses_a_schema_without_the_rename_record() {
    let (store, pool, _container) = migrated().await;
    store
        .probe()
        .await
        .expect("a fully migrated schema is ready");
    sqlx::query("DROP TABLE IF EXISTS workspace_rename_events")
        .execute(&pool)
        .await
        .expect("drop the record");
    assert!(
        store.probe().await.is_err(),
        "the probe must refuse a schema without workspace_rename_events"
    );
}
