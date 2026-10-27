//! FU-21 / MIGRATE-LOCK — two replicas that boot together on one database
//! must both come up, with every index valid.
//!
//! Scaffolded RED by DISTILL (2026-10-27, ADR-025). Before the fix both tests
//! failed with Postgres `deadlock detected` (40P01): `Store::migrate`
//! serialised replicas with a BLOCKING `SELECT pg_advisory_lock($1)`. A
//! replica parked in that statement holds a snapshot for as long as it waits.
//! Migration 0029's `CREATE INDEX CONCURRENTLY` waits for every older snapshot in the database,
//! so the lock holder waits for the waiter while the waiter waits for the lock
//! holder. Postgres kills one of them. If it kills the index build, the index
//! is left INVALID and every later boot refuses with 42P07 until an operator
//! runs runbook R-7.
//!
//! The race is made deterministic with a gate. The test holds the migration
//! lock itself, starts both replicas, waits until each has asked for the lock,
//! and then lets go. Whichever replica wins runs 0029 while the other is
//! certainly still waiting. The "has asked" check reads the last statement of
//! each session in `pg_stat_activity`, so it holds for a blocking wait
//! (`pg_advisory_lock`, active) and for a polling wait (`pg_try_advisory_lock`,
//! idle between tries) alike. The fix's shape does not change the test.
//!
//! Each test runs on its own container in the `public` schema, which is how the
//! production binary runs: `Store::migrate` always uses the canonical lock id,
//! and the test-support twin derives the same canonical id from a `public`
//! search_path.

use foundry_store::{run_migrations_from_dir, Store, StoreError};
use sqlx::{Connection, PgConnection, PgPool};
use std::future::Future;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};

/// The runbook-documented migration lock (`MIGRATION_LOCK_ID` in
/// `foundry-store/src/lib.rs`). Private there, so it is restated here; the
/// gate must hold exactly the lock the replicas ask for.
const MIGRATION_LOCK_ID: i64 = 0xF00D_BABE_F00D_BABE_u64 as i64;
/// The index migration 0029 builds concurrently.
const INDEX_0029: &str = "idx_outbox_undispatched";
/// A boot that has not finished by now is hung, not slow.
const BOOT_BOUND: Duration = Duration::from_secs(45);

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

struct Pg {
    url: String,
    _c: ContainerAsync<Postgres>,
}

async fn fresh_database() -> Pg {
    let c = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("start postgres container");
    let host = c.get_host().await.expect("container host");
    let port = c.get_host_port_ipv4(5432).await.expect("container port");
    let url = format!("postgres://postgres:postgres@{host}:{port}/postgres");
    Pg { url, _c: c }
}

impl Pg {
    async fn connection(&self) -> PgConnection {
        PgConnection::connect(&self.url)
            .await
            .expect("connect to the database")
    }

    async fn pool(&self) -> PgPool {
        PgPool::connect(&self.url).await.expect("connect pool")
    }
}

fn production_migrations_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations")
}

/// Every version the release ships, from the migration file names.
fn shipped_versions() -> Vec<i64> {
    let mut versions: Vec<i64> = std::fs::read_dir(production_migrations_dir())
        .expect("read migrations dir")
        .filter_map(|entry| {
            let name = entry.ok()?.file_name().to_string_lossy().into_owned();
            name.strip_suffix(".sql")?;
            name.split('_').next()?.parse().ok()
        })
        .collect();
    versions.sort_unstable();
    versions
}

/// Two replicas boot while the migration lock is held elsewhere; once both
/// are waiting for it, the lock is released and both boots run to the end.
/// Returns each replica's outcome.
async fn boot_two_replicas_together<A, B, T>(
    pg: &Pg,
    first: A,
    second: B,
) -> (Result<T, StoreError>, Result<T, StoreError>)
where
    A: Future<Output = Result<T, StoreError>>,
    B: Future<Output = Result<T, StoreError>>,
{
    let mut gate = pg.connection().await;
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(MIGRATION_LOCK_ID)
        .execute(&mut gate)
        .await
        .expect("the gate takes the migration lock");

    let release_once_both_wait = async {
        wait_until_two_replicas_asked_for_the_lock(&mut gate).await;
        sqlx::query("SELECT pg_advisory_unlock($1)")
            .bind(MIGRATION_LOCK_ID)
            .execute(&mut gate)
            .await
            .expect("the gate releases the migration lock");
    };
    let (first, second, ()) = tokio::join!(
        tokio::time::timeout(BOOT_BOUND, first),
        tokio::time::timeout(BOOT_BOUND, second),
        release_once_both_wait,
    );
    let hung = |_| panic!("a boot hung for over {BOOT_BOUND:?}");
    (first.unwrap_or_else(hung), second.unwrap_or_else(hung))
}

/// Both replicas have reached the lock: each other session's last statement
/// asked for an advisory lock (blocking or polling).
async fn wait_until_two_replicas_asked_for_the_lock(gate: &mut PgConnection) {
    let started = Instant::now();
    loop {
        let asking: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_stat_activity
              WHERE datname = current_database()
                AND pid <> pg_backend_pid()
                AND query ILIKE '%advisory_lock(%'
                AND query NOT ILIKE '%advisory_unlock%'",
        )
        .fetch_one(&mut *gate)
        .await
        .expect("read pg_stat_activity");
        if asking >= 2 {
            return;
        }
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "both replicas should be waiting for the migration lock; {asking} are"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// Indexes in the application schema that Postgres would not use.
async fn invalid_indexes(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT c.relname::text FROM pg_index i
           JOIN pg_class c ON c.oid = i.indexrelid
           JOIN pg_namespace n ON n.oid = c.relnamespace
          WHERE n.nspname = 'public' AND NOT i.indisvalid
          ORDER BY 1",
    )
    .fetch_all(pool)
    .await
    .expect("list invalid indexes")
}

async fn index_0029_is_valid(pool: &PgPool) -> Option<bool> {
    sqlx::query_scalar(
        "SELECT i.indisvalid FROM pg_index i
           JOIN pg_class c ON c.oid = i.indexrelid
           JOIN pg_namespace n ON n.oid = c.relnamespace
          WHERE n.nspname = 'public' AND c.relname = $1",
    )
    .bind(INDEX_0029)
    .fetch_optional(pool)
    .await
    .expect("look up the 0029 index")
}

/// `(version, success)` for every recorded migration.
async fn recorded_migrations(pool: &PgPool) -> Vec<(i64, bool)> {
    sqlx::query_as("SELECT version, success FROM _sqlx_migrations ORDER BY version")
        .fetch_all(pool)
        .await
        .expect("read _sqlx_migrations")
}

/// The database a pair of boots leaves behind is the one a single boot would:
/// every shipped migration recorded as a success, every index usable, and
/// nothing for the operator to repair.
async fn assert_fully_migrated_and_healthy(pool: &PgPool) {
    let expected: Vec<(i64, bool)> = shipped_versions().into_iter().map(|v| (v, true)).collect();
    assert_eq!(
        recorded_migrations(pool).await,
        expected,
        "every shipped migration, through the head, is recorded once as applied"
    );
    assert_eq!(
        index_0029_is_valid(pool).await,
        Some(true),
        "0029's {INDEX_0029} exists and is valid"
    );
    assert_eq!(
        invalid_indexes(pool).await,
        Vec::<String>::new(),
        "no index is left INVALID"
    );
    let warnings = Store::from_pool(pool.clone())
        .invalid_index_warnings()
        .await
        .expect("read the boot warnings");
    assert!(
        warnings.is_empty(),
        "no runbook R-7 repair is asked for: {warnings:?}"
    );
}

// ---------------------------------------------------------------------------
// Scenarios
// ---------------------------------------------------------------------------

/// Two replicas of this release boot at once against a fresh database (a
/// rolling deploy with surge, a scale-out, a restart racing a new pod). Both
/// come up, and the database holds every migration with every index valid.
/// At HEAD, one boot dies with "deadlock detected" (or the 0029 build is the
/// victim and its index is left INVALID).
#[tokio::test]
async fn two_replicas_booting_together_both_migrate_a_fresh_database() {
    let pg = fresh_database().await;
    let replica_a = Store::connect(&pg.url).await.expect("replica A connects");
    let replica_b = Store::connect(&pg.url).await.expect("replica B connects");

    let (a, b) =
        boot_two_replicas_together(&pg, async move { replica_a.migrate().await }, async move {
            replica_b.migrate().await
        })
        .await;

    assert!(
        a.is_ok() && b.is_ok(),
        "both replicas finish migrating; replica A: {a:?}; replica B: {b:?}"
    );
    assert_fully_migrated_and_healthy(&pg.pool().await).await;
}

/// The test-support twin (`run_migrations_from_dir`, which US-04's
/// multi-replica harness boots through) takes the same blocking lock, so it
/// has the same deadlock with 0029. No caller races it on a fresh database
/// today (US-04 races only its staged 0099 on an already-migrated schema), but
/// the fix must keep the twin in step with production.
#[tokio::test]
async fn two_test_support_boots_together_both_migrate_a_fresh_database() {
    let pg = fresh_database().await;
    let pool_a = pg.pool().await;
    let pool_b = pg.pool().await;
    let dir = production_migrations_dir();
    let dir_b = dir.clone();

    let (a, b) = boot_two_replicas_together(
        &pg,
        async move { run_migrations_from_dir(&pool_a, &dir).await },
        async move { run_migrations_from_dir(&pool_b, &dir_b).await },
    )
    .await;

    let (a, b) = match (a, b) {
        (Ok(a), Ok(b)) => (a, b),
        (a, b) => panic!("both boots finish migrating; boot A: {a:?}; boot B: {b:?}"),
    };
    let mut applied: Vec<i64> = a.applied.iter().chain(&b.applied).copied().collect();
    applied.sort_unstable();
    assert_eq!(
        applied,
        shipped_versions(),
        "between them the two boots apply each shipped migration exactly once"
    );
    assert_fully_migrated_and_healthy(&pg.pool().await).await;
}
