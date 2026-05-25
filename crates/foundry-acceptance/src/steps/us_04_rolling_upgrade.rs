//! US-04 rolling-upgrade step definitions.
//!
//! Per `distill/driver.md` §2d + `wave-decisions.md` §US-04: the
//! scenarios stage a per-scenario migrations directory (production
//! base copy + a `0099_*.sql` test migration) and race N replicas for
//! the `pg_advisory_lock(MIGRATION_LOCK_ID)` exposed by
//! `foundry_store::run_migrations_from_dir`.
//!
//! The "workspace exists with admin" Background step is inherited
//! from `us_06_signin.rs`; this module does NOT redefine it. The
//! "database is at schema version 0001" assertion is a no-op
//! confirmation: the slice-1 harness already applied 0001-0005 by
//! the time Background completes.

use crate::support::harness::InProcHarness;
use crate::support::multi_replica_harness::{MultiReplicaHarness, SpawnConcurrentError};
use crate::support::test_migration;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use sqlx::PgPool;
use std::time::{Duration, Instant};

const TEST_NOW: &str = "2026-01-15T12:00:00Z";
const TEST_MIGRATION_VERSION_99: i64 = 99;

fn now_anchor() -> time::OffsetDateTime {
    time::OffsetDateTime::parse(TEST_NOW, &time::format_description::well_known::Rfc3339)
        .expect("parse anchor")
}

// ----- Background --------------------------------------------------------

#[given(regex = r"^the database is at schema version (\d+)$")]
async fn database_at_schema_version(world: &mut FoundryWorld, _version: u32) {
    // After Background's "a workspace ... exists with admin ..." step,
    // the slice-1 InProcHarness has applied production migrations
    // 0001..0005 into the per-scenario schema. This step is a
    // narrative anchor — the actual assertion is that the workspace
    // tables exist (Background's INSERTs would have errored if they
    // did not). Nothing to do.
    let harness = world.harness.as_ref().expect("Background ensured harness");
    // Sanity check: the workspace table is present (would 42P01 otherwise).
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM workspaces")
        .fetch_one(harness.app.state.store.pool())
        .await
        .expect("workspaces table query after Background");
    assert!(count.0 >= 1, "Background must have seeded ≥1 workspace");
}

// ----- Given: stage the test-only migration ------------------------------

#[given(
    regex = r#"^the new Foundry version ships a forward-compatible schema update labeled "(\d+)" that adds a new optional field to the issues domain$"#
)]
async fn ships_forward_compat_update(world: &mut FoundryWorld, version: u32) {
    let filename = format!("0{:03}_us04_add_dummy_column.sql", version);
    // Add a nullable column to `issues`. Nullable so it's
    // forward-compatible with the existing rows the Background may
    // (not) have created.
    let sql = "ALTER TABLE issues ADD COLUMN us04_dummy_field TEXT;\n";
    let dir = test_migration::stage(&[(filename.as_str(), sql)])
        .expect("stage forward-compat test migration");
    world.us_04_migrations_dir = Some(dir);
}

#[given(
    regex = r#"^the new Foundry version ships a broken schema update labeled "(\d+)" that references a non-existent table$"#
)]
async fn ships_broken_update(world: &mut FoundryWorld, version: u32) {
    let filename = format!("0{:03}_us04_broken.sql", version);
    let sql = "ALTER TABLE nonexistent_table_us04 ADD COLUMN x INT;\n";
    let dir =
        test_migration::stage(&[(filename.as_str(), sql)]).expect("stage broken test migration");
    world.us_04_migrations_dir = Some(dir);
}

#[given(
    regex = r#"^the new Foundry version ships a schema update labeled "(\d+)" that takes about (\d+) seconds to apply$"#
)]
async fn ships_slow_update(world: &mut FoundryWorld, version: u32, seconds: u64) {
    let filename = format!("0{:03}_us04_slow.sql", version);
    let sql = "ALTER TABLE issues ADD COLUMN us04_slow_field TEXT;\n";
    let dir =
        test_migration::stage(&[(filename.as_str(), sql)]).expect("stage slow test migration");
    world.us_04_migrations_dir = Some(dir);
    // Record the slow-migration delay on the world; the When step
    // passes it to spawn_concurrent_sharing_schema_with_delay. The
    // delay is per-call inside run_migrations_from_dir_with_delay
    // AND gated on has_work — the lock-race WINNER pays the cost;
    // the LOSER observes no work and skips. This keeps the slow
    // scenario isolated from parallel scenarios under cucumber's
    // max_concurrent_scenarios concurrency.
    //
    // We trim "about N seconds" to N*1000-300ms so the second
    // replica's observed wait sits comfortably inside the
    // [1500, 3000] ms window asserted by the Then step even under
    // CI load (where pool acquisition + migrator overhead can add
    // 100-300ms tail). The scenario's "about 2 seconds" wording
    // accommodates this trim.
    let trimmed_ms = (seconds * 1000).saturating_sub(300);
    world.us_04_slow_migration_delay_ms = Some(trimmed_ms);
}

// Idempotency Given: stage 0099 AND apply it once via a single-replica
// concurrent harness. Drops the harness; next step re-spawns against
// the same schema.
#[given(regex = r#"^a replica has already applied schema update "(\d+)"$"#)]
async fn replica_already_applied(world: &mut FoundryWorld, version: u32) {
    let filename = format!("0{:03}_us04_idempotent.sql", version);
    let sql = "ALTER TABLE issues ADD COLUMN us04_idempotent_field TEXT;\n";
    let dir = test_migration::stage(&[(filename.as_str(), sql)])
        .expect("stage idempotency test migration");
    world.us_04_migrations_dir = Some(dir);

    let existing = world.harness.as_ref().expect("Background harness present");
    // Re-stage the dir for the first-apply spawn. We can't move out of
    // the world.us_04_migrations_dir; instead, stage a SECOND copy for
    // this first invocation and keep the world copy for the restart.
    let first_dir = test_migration::stage(&[(filename.as_str(), sql)])
        .expect("stage first-pass test migration");
    let harness =
        MultiReplicaHarness::spawn_concurrent_sharing_schema(1, first_dir, existing, now_anchor())
            .await
            .expect("first-pass migration must succeed");

    // Sanity: the first pass actually applied 0099.
    let report = harness.applied_migrations(0);
    assert!(
        report.applied.contains(&(version as i64)),
        "first replica must have applied version {version}; report={report:?}"
    );
    // Drop the harness so the restart step is a true re-spawn.
    drop(harness);
}

// ----- When: replica boot scenarios --------------------------------------

#[when(
    regex = r"^the operator starts (\d+) replicas of the new version simultaneously against the same database$"
)]
async fn operator_starts_n_replicas(world: &mut FoundryWorld, n: usize) {
    let dir = world
        .us_04_migrations_dir
        .take()
        .expect("the 'ships ... schema update' Given must run first");
    let existing = world.harness.as_ref().expect("Background harness present");
    let delay_ms = world.us_04_slow_migration_delay_ms.unwrap_or(0);
    match MultiReplicaHarness::spawn_concurrent_sharing_schema_with_delay(
        n,
        dir,
        existing,
        delay_ms,
        now_anchor(),
    )
    .await
    {
        Ok(h) => {
            // Capture per-replica reports + boot durations on the world.
            world.us_04_migration_reports = (0..n).map(|i| h.applied_migrations(i)).collect();
            world.us_04_boot_durations = h.boot_durations.clone();
            world.us_04_concurrent = Some(h);
        }
        Err(e) => {
            world.us_04_spawn_error = Some(e.to_string());
        }
    }
}

#[when(regex = r"^that replica is stopped and restarted against the same database$")]
async fn replica_stopped_and_restarted(world: &mut FoundryWorld) {
    let dir = world
        .us_04_migrations_dir
        .take()
        .expect("the 'already applied schema update' Given must have staged the dir");
    let existing = world.harness.as_ref().expect("Background harness present");
    // Spawn ONE replica against the same schema; sqlx sees 0099 in
    // _sqlx_migrations and the report should classify it as
    // already-applied with zero new applications.
    let h = MultiReplicaHarness::spawn_concurrent_sharing_schema(1, dir, existing, now_anchor())
        .await
        .expect("restart must succeed");
    world.us_04_migration_reports = vec![h.applied_migrations(0)];
    world.us_04_boot_durations = h.boot_durations.clone();
    world.us_04_concurrent = Some(h);
}

#[when(regex = r#"^a replica boots and attempts to apply schema update "(\d+)"$"#)]
async fn replica_boots_attempts_apply(world: &mut FoundryWorld, _version: u32) {
    // Same shape as `operator_starts_n_replicas(1)` but the
    // broken-migration scenario expects this to fail.
    let dir = world
        .us_04_migrations_dir
        .take()
        .expect("the 'ships a broken schema update' Given must run first");
    let existing = world.harness.as_ref().expect("Background harness present");
    match MultiReplicaHarness::spawn_concurrent_sharing_schema(1, dir, existing, now_anchor()).await
    {
        Ok(h) => {
            world.us_04_migration_reports = vec![h.applied_migrations(0)];
            world.us_04_concurrent = Some(h);
        }
        Err(e) => {
            world.us_04_spawn_error = Some(e.to_string());
        }
    }
}

#[when(
    regex = r"^the operator starts (\d+) replicas of the new version simultaneously and the first replica acquires the migration lock$"
)]
async fn operator_starts_n_replicas_first_acquires_lock(world: &mut FoundryWorld, n: usize) {
    // Same call as the WS scenario; the "first acquires the lock"
    // phrasing is observed via the per-replica boot durations
    // captured by the harness. The per-AppState slow-migration delay
    // (set by the prior Given) is honoured inside
    // `run_migrations_from_dir_with_delay` ONLY when the replica
    // observes migration work — the winner pays the cost; the loser
    // blocks on the advisory lock then proceeds with no delay.
    operator_starts_n_replicas(world, n).await;
}

// ----- Then: per-replica migration outcomes ------------------------------

#[then(regex = r#"^exactly one replica reports having applied schema update "(\d+)"$"#)]
async fn exactly_one_replica_applied(world: &mut FoundryWorld, version: u32) {
    let applied_count = world
        .us_04_migration_reports
        .iter()
        .filter(|r| r.applied.contains(&(version as i64)))
        .count();
    assert_eq!(
        applied_count, 1,
        "expected exactly 1 replica to apply version {version}; \
         reports were {:?}",
        world.us_04_migration_reports
    );
}

#[then(
    regex = r#"^the other replica reports having observed schema update "(\d+)" as already-applied$"#
)]
async fn other_replica_observed_already_applied(world: &mut FoundryWorld, version: u32) {
    let already_count = world
        .us_04_migration_reports
        .iter()
        .filter(|r| r.already_applied.contains(&(version as i64)))
        .count();
    assert!(
        already_count >= 1,
        "expected at least 1 replica to observe version {version} as already-applied; \
         reports were {:?}",
        world.us_04_migration_reports
    );
}

#[then(regex = r"^both replicas reach a healthy /readyz within (\d+) seconds$")]
async fn both_replicas_healthy_within(world: &mut FoundryWorld, secs: u64) {
    let h = world
        .us_04_concurrent
        .as_ref()
        .expect("concurrent harness must be present");
    let client = reqwest::Client::new();
    let deadline = Instant::now() + Duration::from_secs(secs);
    for (idx, replica) in h.replicas.iter().enumerate() {
        let url = format!("http://{}/readyz", replica.addr);
        loop {
            let resp = client.get(&url).send().await;
            if let Ok(r) = resp {
                if r.status().is_success() {
                    break;
                }
            }
            assert!(
                Instant::now() < deadline,
                "replica {idx} did not reach healthy /readyz within {secs}s"
            );
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
}

#[then(regex = r"^the new optional field is present in the issues domain on both replicas$")]
async fn new_field_present_in_issues(world: &mut FoundryWorld) {
    let h = world
        .us_04_concurrent
        .as_ref()
        .expect("concurrent harness must be present");
    // Both replicas point at the same schema; one query against the
    // shared pool covers "both replicas observe the column" since
    // postgres catalog is per-schema, not per-replica.
    let pool = &h.shared_pool;
    let exists = column_exists_in_schema(pool, &h.schema, "issues", "us04_dummy_field").await;
    assert!(
        exists,
        "expected `us04_dummy_field` to exist in `issues` after the schema update"
    );
}

#[then(regex = r"^the replica reports zero schema updates executed during this boot$")]
async fn replica_reports_zero_executed(world: &mut FoundryWorld) {
    assert_eq!(world.us_04_migration_reports.len(), 1, "expected 1 replica");
    let report = &world.us_04_migration_reports[0];
    assert!(
        report.applied.is_empty(),
        "expected zero applied on restart; got {:?}",
        report.applied
    );
}

#[then(regex = r"^the replica reaches a healthy /readyz within (\d+) seconds$")]
async fn single_replica_healthy_within(world: &mut FoundryWorld, secs: u64) {
    let h = world
        .us_04_concurrent
        .as_ref()
        .expect("concurrent harness must be present");
    let client = reqwest::Client::new();
    let deadline = Instant::now() + Duration::from_secs(secs);
    let replica = h.replicas.first().expect("at least one replica");
    let url = format!("http://{}/readyz", replica.addr);
    loop {
        let resp = client.get(&url).send().await;
        if let Ok(r) = resp {
            if r.status().is_success() {
                return;
            }
        }
        assert!(
            Instant::now() < deadline,
            "replica did not reach healthy /readyz within {secs}s"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[then(
    regex = r#"^the migration history records exactly one application of schema update "(\d+)"$"#
)]
async fn migration_history_one_application(world: &mut FoundryWorld, version: u32) {
    let h = world
        .us_04_concurrent
        .as_ref()
        .expect("concurrent harness must be present");
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM _sqlx_migrations WHERE version = $1")
        .bind(version as i64)
        .fetch_one(&h.shared_pool)
        .await
        .expect("query _sqlx_migrations");
    assert_eq!(
        count.0, 1,
        "expected exactly one application of version {version}; got {}",
        count.0
    );
}

#[then(regex = r"^the replica reports a schema-update error and exits with a non-zero status$")]
async fn replica_reports_error_nonzero(world: &mut FoundryWorld) {
    let err = world
        .us_04_spawn_error
        .as_ref()
        .expect("expected a SpawnConcurrentError to have been raised by the broken migration");
    assert!(
        err.contains("migration failed"),
        "expected migration-failure error; got: {err}"
    );
    // The "non-zero status" observable is: the harness's spawn
    // returned Err (i.e. there's no `us_04_concurrent` running). A
    // production binary surfacing this Err would exit with non-zero.
    assert!(
        world.us_04_concurrent.is_none(),
        "expected no running concurrent harness after broken migration"
    );
}

#[then(regex = r#"^the migration history records no application of schema update "(\d+)"$"#)]
async fn migration_history_no_application(world: &mut FoundryWorld, version: u32) {
    // Reach back through the slice-1 harness's pool — the broken
    // migration scenario failed to build us_04_concurrent, so we
    // assert against the seed harness's shared schema pool.
    let harness = world.harness.as_ref().expect("Background harness");
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM _sqlx_migrations WHERE version = $1")
        .bind(version as i64)
        .fetch_one(harness.app.state.store.pool())
        .await
        .expect("query _sqlx_migrations");
    assert_eq!(
        count.0, 0,
        "expected no row for failed version {version}; got {}",
        count.0
    );
}

#[then(regex = r"^every previously-applied schema update is unchanged$")]
async fn every_previous_update_unchanged(world: &mut FoundryWorld) {
    // The Background ran 0001..0005 from the production migrations
    // dir. After a failed 0099, those rows must still be intact.
    let harness = world.harness.as_ref().expect("Background harness");
    let rows: Vec<(i64,)> =
        sqlx::query_as("SELECT version FROM _sqlx_migrations WHERE version < 99 ORDER BY version")
            .fetch_all(harness.app.state.store.pool())
            .await
            .expect("query _sqlx_migrations base rows");
    let versions: Vec<i64> = rows.into_iter().map(|(v,)| v).collect();
    // Slice-1 ships 0001..0005; assert all five are present.
    assert!(
        versions.contains(&1)
            && versions.contains(&2)
            && versions.contains(&3)
            && versions.contains(&4)
            && versions.contains(&5),
        "expected production base versions 1..=5 intact; got {versions:?}"
    );
}

#[then(regex = r"^the second replica's boot is blocked for between (\d+) and (\d+) milliseconds$")]
async fn second_replica_blocked_between(world: &mut FoundryWorld, lo_ms: u64, hi_ms: u64) {
    assert!(
        world.us_04_boot_durations.len() >= 2,
        "expected ≥2 boot durations; got {:?}",
        world.us_04_boot_durations
    );
    // The "second replica" is whichever finishes later. Take max.
    let max = world
        .us_04_boot_durations
        .iter()
        .max()
        .expect("non-empty boot durations");
    let max_ms = max.as_millis() as u64;
    assert!(
        max_ms >= lo_ms && max_ms <= hi_ms,
        "expected second-replica boot in [{lo_ms}, {hi_ms}] ms; got {max_ms}ms (all durations: {:?})",
        world.us_04_boot_durations
    );
}

#[then(
    regex = r"^after the first replica releases the lock the second replica observes the schema update as already-applied$"
)]
async fn second_observes_already_applied_post_release(world: &mut FoundryWorld) {
    // By the time `spawn_concurrent` returns, the slow-migration
    // delay has expired and the second replica has applied or
    // observed-as-applied. Exactly one applied + one already_applied
    // is the production-meaningful invariant.
    let applied_count = world
        .us_04_migration_reports
        .iter()
        .filter(|r| r.applied.contains(&TEST_MIGRATION_VERSION_99))
        .count();
    let already_count = world
        .us_04_migration_reports
        .iter()
        .filter(|r| r.already_applied.contains(&TEST_MIGRATION_VERSION_99))
        .count();
    assert_eq!(
        applied_count, 1,
        "expected exactly 1 replica to have applied 0099; reports={:?}",
        world.us_04_migration_reports
    );
    assert_eq!(
        already_count, 1,
        "expected exactly 1 replica to have observed 0099 as already-applied; reports={:?}",
        world.us_04_migration_reports
    );
}

// ----- helpers ----------------------------------------------------------

async fn column_exists_in_schema(pool: &PgPool, schema: &str, table: &str, column: &str) -> bool {
    let row: (bool,) = sqlx::query_as(
        "SELECT EXISTS (
            SELECT 1 FROM information_schema.columns
            WHERE table_schema = $1 AND table_name = $2 AND column_name = $3
        )",
    )
    .bind(schema)
    .bind(table)
    .bind(column)
    .fetch_one(pool)
    .await
    .expect("information_schema.columns query");
    row.0
}

// Suppress dead-code warnings for the InProcHarness import — it's
// used transitively for type inference inside `world.harness`.
#[allow(dead_code)]
fn _ensure_inproc_harness_type_used(_: &InProcHarness) {}

// Suppress dead-code warnings for SpawnConcurrentError — it's used
// as the error type returned by harness calls; the Then steps assert
// against `world.us_04_spawn_error.to_string()`.
#[allow(dead_code)]
fn _ensure_spawn_error_type_used(_: &SpawnConcurrentError) {}
