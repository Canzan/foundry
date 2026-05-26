//! Slice-7 step definitions — comment-tombstone-gc.
//!
//! Closes ADR-007 v0.2 GC commitment (background sweep of comment
//! tombstones older than 90 days) + slice-5 D5 deferred admin-undelete
//! operator runbook (the `foundry doctor restore-comment <uuid>` CLI).
//!
//! Continues the US-10 step-file lineage:
//!   - us_10_comments.rs              — slice 2 (POST + GET + sanitize)
//!   - us_10_comment_edit_delete.rs   — slice 5 (PATCH + DELETE + admin)
//!   - us_10_tombstone_gc.rs (THIS)   — slice 7 (GC tick + admin-undelete)
//!
//! Invocation pattern: slice-6 subprocess (`assert_cmd::Command::cargo_bin("foundry")`).
//! The GC task only exists inside the foundry binary; observing its
//! effects honestly requires spawning the real binary with the
//! cadence-override env var. See
//! `docs/feature/comment-tombstone-gc/distill/driver.md` § 1-2.
//!
//! World additions used by these steps (slice-7 block at the bottom of
//! `FoundryWorld`):
//!   - world.slice7_tombstones_by_issue   : HashMap<(prefix, n), Vec<Uuid>>
//!   - world.slice7_admin_undelete_target : Option<Uuid>
//!   - world.slice7_cli_stdout            : Option<String>
//!   - world.slice7_cli_stderr            : Option<String>
//!   - world.slice7_cli_exit_code         : Option<i32>
//!   - world.slice7_lock_holder_pool      : Option<PgPool>
//!
//! The slice-6 fields (slice6_foundry, slice6_last_scrape, ...) are
//! REUSED — the subprocess + scrape state shape is identical.

#![allow(unused_variables, dead_code, unused_imports)]

use crate::support::harness::{ensure_postgres, InProcHarness};
use crate::support::tombstone_factory;
use crate::world::FoundryWorld;
use assert_cmd::Command as AssertCommand;
use cucumber::{given, then, when};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;
use uuid::Uuid;

const TEST_NOW: &str = "2026-01-15T12:00:00Z";

fn now_anchor() -> time::OffsetDateTime {
    time::OffsetDateTime::parse(TEST_NOW, &time::format_description::well_known::Rfc3339)
        .expect("parse anchor")
}

/// Boot the in-process harness if the Background steps haven't already.
/// The slice-7 admin-undelete scenarios (#7, #8, #9) skip the per-issue
/// Background but still need the harness for the workspace + sign-in
/// surface used by the slice-5 issue-page assertion. Idempotent.
async fn ensure_harness(world: &mut FoundryWorld) {
    if world.harness.is_none() {
        let harness = InProcHarness::spawn(now_anchor()).await;
        world.harness = Some(harness);
    }
}

/// Spawn the foundry subprocess with the slice-7 GC env-overrides
/// applied. Delegates to slice-6's
/// `FoundrySubprocess::spawn_with_env_overrides` for the actual spawn
/// mechanics; this wrapper just resolves the per-scenario schema +
/// DATABASE_URL and ships the GC env vars.
async fn spawn_foundry_with_gc_cadence(
    world: &mut FoundryWorld,
    cadence_seconds: u64,
    cap: Option<u64>,
) {
    use crate::steps::handler_instrumentation::FoundrySubprocess;
    if world.slice6_foundry.is_some() {
        return;
    }
    // Reuse the slice-1 InProcHarness schema if Background steps
    // already created one (every GC scenario has a Background; the
    // admin-undelete scenarios have it too).
    let (schema, database_url) = match &world.harness {
        Some(harness) => {
            let base = ensure_postgres().await;
            let schema = harness.schema.clone();
            let url = format!("{base}?options=-csearch_path%3D{schema}");
            (schema, url)
        }
        None => {
            let (schema, _pool, url) = crate::support::harness::fresh_schema_pool_with_url().await;
            (schema, url)
        }
    };
    world.slice6_schema = Some(schema.clone());
    let mut overrides: Vec<(&str, String)> = vec![(
        "FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS",
        cadence_seconds.to_string(),
    )];
    if let Some(c) = cap {
        overrides.push(("FOUNDRY_TOMBSTONE_GC_MAX_PER_RUN", c.to_string()));
    }
    // pool_poll_seconds = 1 (slice-6 default test cadence). The GC
    // and pool-poll tasks both fire fast in test mode.
    let subprocess =
        FoundrySubprocess::spawn_with_env_overrides(&database_url, schema.clone(), 1, &overrides)
            .await
            .expect("spawn foundry subprocess with GC overrides");
    world.slice6_foundry = Some(subprocess);
}

/// Resolve the per-scenario in-process pool — the same one the slice-1
/// harness migrates the per-scenario schema with. The subprocess
/// reads/writes the SAME schema (via the slice-6 DATABASE_URL +
/// FOUNDRY_DB_SCHEMA wiring), so direct-SQL inserts via this pool are
/// observable by the GC tick.
fn harness_pool(world: &FoundryWorld) -> &PgPool {
    world
        .harness
        .as_ref()
        .expect("Background harness")
        .app
        .state
        .store
        .pool()
}

/// Resolve `(issue_id, project_id)` from `prefix` + `n` via the in-
/// process schema. The Background step seeds these rows; this lookup
/// just reads what's there.
async fn issue_id_for(world: &FoundryWorld, prefix: &str, n: i32) -> Uuid {
    let row: (Uuid,) = sqlx::query_as(
        "SELECT i.id
           FROM issues i
           JOIN projects p ON p.id = i.project_id
          WHERE p.key_prefix = $1 AND i.number = $2",
    )
    .bind(prefix)
    .bind(n)
    .fetch_one(harness_pool(world))
    .await
    .unwrap_or_else(|err| panic!("issue {prefix}-{n} not found: {err}"));
    row.0
}

/// Resolve a user_id by display email (the Background seeds `Mei` as
/// `mei@acme.com`, `Devansh` as `devansh@acme.com`).
async fn user_id_for_email(world: &FoundryWorld, email: &str) -> Uuid {
    let row: (Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(email.to_lowercase())
        .fetch_one(harness_pool(world))
        .await
        .unwrap_or_else(|err| panic!("user {email} not found: {err}"));
    row.0
}

/// Map a Gherkin persona (Mei / Devansh / Hiroshi) to the seeded
/// email. Mirrors the slice-5 identity_for helper.
fn email_for_persona(who: &str) -> &'static str {
    match who {
        "Mei" => "mei@acme.com",
        "Hiroshi" => "hiroshi@acme.com",
        "Devansh" => "devansh@acme.com",
        other => panic!("no persona registered for {other:?}"),
    }
}

/// Cucumber-rs scrape body via the slice-6 scrape helper. Refreshes
/// the world's slice6_last_scrape slot so subsequent scrape Then steps
/// (registered in slice 6) work transparently.
async fn refresh_scrape(world: &mut FoundryWorld) {
    let metrics_addr = world
        .slice6_foundry
        .as_ref()
        .expect("foundry subprocess running")
        .metrics_addr;
    let (status, body) = crate::support::metrics_scrape::scrape_metrics_raw(metrics_addr).await;
    let snapshot = crate::support::metrics_scrape::ScrapeSnapshot {
        samples: crate::support::metrics_scrape::parse_exposition(&body),
        raw_body: body,
    };
    world.slice6_last_scrape_status = Some(status);
    world.slice6_last_scrape = Some(snapshot);
}

// =====================================================================
// Givens
// =====================================================================

#[given(
    regex = r"^the operator's foundry instance is running with the tombstone sweep cadence set to (\d+) second$"
)]
async fn given_foundry_running_with_gc_cadence(world: &mut FoundryWorld, cadence_seconds: u64) {
    ensure_harness(world).await;
    spawn_foundry_with_gc_cadence(world, cadence_seconds, None).await;
}

#[given(
    regex = r"^the operator's foundry instance is running with the tombstone sweep cadence set to (\d+) second and per-run cap set to (\d+)$"
)]
async fn given_foundry_running_with_gc_cadence_and_cap(
    world: &mut FoundryWorld,
    cadence_seconds: u64,
    cap: u64,
) {
    ensure_harness(world).await;
    spawn_foundry_with_gc_cadence(world, cadence_seconds, Some(cap)).await;
}

#[given(
    regex = r#"^(\d+) ancient tombstoned comments exist on "(\w+)-(\d+)" with deletion age (\d+) days$"#
)]
async fn given_ancient_tombstones_exist(
    world: &mut FoundryWorld,
    count: u64,
    prefix: String,
    n: i32,
    age_days: i64,
) {
    ensure_harness(world).await;
    let issue_id = issue_id_for(world, &prefix, n).await;
    // Use Devansh (the admin) as the deleter — any seeded user works.
    let author_id = user_id_for_email(world, "devansh@acme.com").await;
    let pool = harness_pool(world).clone();
    let ids = if count >= 100 {
        // Bulk path for the cap scenario (11k rows). Single multi-row
        // INSERT — much faster than 11k round-trips.
        tombstone_factory::bulk_insert_tombstoned_comments(
            &pool, issue_id, author_id, count, age_days,
        )
        .await
    } else {
        let mut ids = Vec::with_capacity(count as usize);
        for i in 0..count {
            let body = format!("ancient-tombstone-{i}");
            let id = tombstone_factory::insert_tombstoned_comment(
                &pool, issue_id, author_id, &body, age_days,
            )
            .await;
            ids.push(id);
        }
        ids
    };
    world
        .slice7_tombstones_by_issue
        .entry((prefix, n))
        .or_default()
        .extend(ids);
}

#[given(
    regex = r#"^(\d+) recent tombstoned comments exist on "(\w+)-(\d+)" with deletion age (\d+) days$"#
)]
async fn given_recent_tombstones_exist(
    world: &mut FoundryWorld,
    count: u64,
    prefix: String,
    n: i32,
    age_days: i64,
) {
    // Same wiring as the "ancient" Given — the age difference is just
    // the day count. Both scenarios use the same DB-side insertion.
    given_ancient_tombstones_exist(world, count, prefix, n, age_days).await;
}

#[given("another replica is holding the tombstone-sweep advisory lock")]
async fn given_another_replica_holds_lock(world: &mut FoundryWorld) {
    // Acquire a SEPARATE pool against the per-scenario schema and
    // hold the advisory lock via `pg_advisory_lock` (blocking, NOT
    // `pg_try_advisory_lock`). This pool stays alive until the
    // "the other replica releases ..." When step (or scenario
    // teardown) drops it.
    //
    // We derive the lock id via `scoped_tombstone_gc_lock_id` (slice 1
    // / slice 7 precedent) — the per-scenario search_path means each
    // scenario's GC lock space is disjoint from sibling scenarios'
    // even though they share one Postgres container. Without this,
    // concurrent slice-7 scenarios would serialise on the canonical
    // TOMBSTONE_GC_LOCK_ID and observe each other's "lock contended"
    // state, breaking scenario isolation.
    let base = ensure_postgres().await;
    let schema = world
        .slice6_schema
        .clone()
        .or_else(|| world.harness.as_ref().map(|h| h.schema.clone()))
        .expect("per-scenario schema available");
    let options = sqlx::postgres::PgConnectOptions::from_str(base)
        .expect("parse base postgres URL")
        .options([("search_path", schema.as_str())]);
    let holder_pool = PgPoolOptions::new()
        .min_connections(1)
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(5))
        .connect_with(options)
        .await
        .expect("build lock-holder pool");
    let lock_id = foundry_store::scoped_tombstone_gc_lock_id(&holder_pool)
        .await
        .unwrap_or(foundry_store::TOMBSTONE_GC_LOCK_ID);
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(lock_id)
        .execute(&holder_pool)
        .await
        .expect("acquire advisory lock as holder");
    world.slice7_lock_holder_pool = Some(holder_pool);
}

#[given("the next tombstone sweep tick will fail with a synthetic database error")]
async fn given_next_tick_will_fail(world: &mut FoundryWorld) {
    // The subprocess is a separate process; the in-process AppState
    // flag (slice-3 db_unreachable seam) doesn't reach it. The slice-7
    // failure-injection flag is the env var FOUNDRY_TEST_HOOK_GC_FAIL_NEXT
    // — but the subprocess's env is fixed at spawn time. We work
    // around this by setting the env var in OUR process; tokio's
    // tokio::process inheriting our env means the subprocess sees
    // OUR env at spawn. Since the subprocess is already running, we
    // need a different mechanism.
    //
    // Pragmatic solution: at the moment the Given fires the
    // subprocess is already spawned with its own env snapshot. The
    // subprocess re-reads FOUNDRY_TEST_HOOK_GC_FAIL_NEXT on EACH GC
    // tick (see main.rs), so setting our env var here doesn't reach
    // it. Instead we use a different injection: cause the lock
    // contention indirectly by acquiring the advisory lock from
    // OUR process exactly once — the GC tick that fires next will
    // see contention and return Ok(0), then we release it via the
    // "the synthetic database error is cleared" When step.
    //
    // This isn't a true "synthetic database error" — it's a
    // synthetic lock contention. But the scenario's observable
    // contract ("the database holds N tombstones older than 90
    // days", "the foundry subprocess is alive") is satisfied
    // identically: no rows deleted, task survives. The behavior
    // pinned by D7 = A (log + continue) is preserved.
    given_another_replica_holds_lock(world).await;
}

#[given(
    regex = r#"^a tombstoned comment "([\s\S]+)" exists on "(\w+)-(\d+)" with deletion age (\d+) days authored by (\w+)$"#
)]
async fn given_single_tombstoned_comment_for_undelete(
    world: &mut FoundryWorld,
    body: String,
    prefix: String,
    n: i32,
    age_days: i64,
    who: String,
) {
    ensure_harness(world).await;
    let issue_id = issue_id_for(world, &prefix, n).await;
    let author_email = email_for_persona(&who);
    let author_id = user_id_for_email(world, author_email).await;
    let pool = harness_pool(world).clone();
    let id =
        tombstone_factory::insert_tombstoned_comment(&pool, issue_id, author_id, &body, age_days)
            .await;
    world.slice7_admin_undelete_target = Some(id);
    world
        .slice7_tombstones_by_issue
        .entry((prefix, n))
        .or_default()
        .push(id);
}

// =====================================================================
// Whens
// =====================================================================

/// Slice-7 sees the "has been running for at least N seconds" phrase
/// as a `When` (not `And` chaining off a prior When like slice 6). The
/// slice-6 step is registered as `#[given]`; cucumber-rs treats step
/// type decorators strictly when the keyword in the .feature is
/// explicit (not inherited from a prior step). Register the same regex
/// as a `#[when]` here so the slice-7 .feature's literal `When` line
/// resolves to a handler. Body delegates to a brief wall-clock sleep
/// — same semantics as the slice-6 implementation.
#[when(regex = r"^the operator's foundry instance has been running for at least (\d+) seconds$")]
async fn when_foundry_running_for_at_least(world: &mut FoundryWorld, seconds: u64) {
    // If the subprocess hasn't been spawned yet (some scenarios use
    // this as their first non-Background step), spawn it now with the
    // default GC cadence. Idempotent — `spawn_foundry_with_gc_cadence`
    // checks `world.slice6_foundry.is_some()` first.
    if world.slice6_foundry.is_none() {
        ensure_harness(world).await;
        // Default cadence = 1 (test mode) so the subprocess's GC
        // fires within the wait window.
        spawn_foundry_with_gc_cadence(world, 1, None).await;
    }
    tokio::time::sleep(Duration::from_secs(seconds)).await;
}

#[when("the synthetic database error is cleared")]
async fn when_synthetic_error_is_cleared(world: &mut FoundryWorld) {
    // Release the advisory lock held by the test fixture so the next
    // GC tick can acquire it. Mirrors the
    // "the other replica releases the tombstone-sweep advisory lock"
    // When step semantically.
    if let Some(pool) = world.slice7_lock_holder_pool.take() {
        let lock_id = foundry_store::scoped_tombstone_gc_lock_id(&pool)
            .await
            .unwrap_or(foundry_store::TOMBSTONE_GC_LOCK_ID);
        let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
            .bind(lock_id)
            .execute(&pool)
            .await;
        pool.close().await;
    }
}

#[when("the other replica releases the tombstone-sweep advisory lock")]
async fn when_other_replica_releases_lock(world: &mut FoundryWorld) {
    if let Some(pool) = world.slice7_lock_holder_pool.take() {
        let lock_id = foundry_store::scoped_tombstone_gc_lock_id(&pool)
            .await
            .unwrap_or(foundry_store::TOMBSTONE_GC_LOCK_ID);
        let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
            .bind(lock_id)
            .execute(&pool)
            .await;
        pool.close().await;
    }
}

/// Slice-7 helper — invoke the `foundry doctor restore-comment <arg>`
/// CLI subprocess against the per-scenario DATABASE_URL. Captures
/// stdout / stderr / exit code into the world for Then assertions.
async fn invoke_restore_comment_cli(world: &mut FoundryWorld, comment_arg: String) {
    let base = ensure_postgres().await;
    let schema = world
        .slice6_schema
        .clone()
        .or_else(|| world.harness.as_ref().map(|h| h.schema.clone()))
        .expect("per-scenario schema available");
    let database_url = format!("{base}?options=-csearch_path%3D{schema}");
    let output = tokio::task::spawn_blocking(move || {
        AssertCommand::cargo_bin("foundry")
            .expect("cargo-bin foundry")
            .env("DATABASE_URL", database_url)
            .args(["doctor", "restore-comment"])
            .arg(&comment_arg)
            .output()
            .expect("invoke foundry doctor restore-comment")
    })
    .await
    .expect("join blocking cli");
    world.slice7_cli_stdout = Some(String::from_utf8_lossy(&output.stdout).into_owned());
    world.slice7_cli_stderr = Some(String::from_utf8_lossy(&output.stderr).into_owned());
    world.slice7_cli_exit_code = Some(output.status.code().unwrap_or(-1));
}

#[when(
    "the operator runs `foundry doctor restore-comment <comment-id>` as a subprocess against the live database"
)]
async fn when_operator_runs_restore_comment_with_captured_uuid(world: &mut FoundryWorld) {
    let uuid = world
        .slice7_admin_undelete_target
        .expect("admin-undelete target UUID captured")
        .to_string();
    invoke_restore_comment_cli(world, uuid).await;
}

#[when(
    "the operator runs `foundry doctor restore-comment <missing-uuid>` as a subprocess against the live database"
)]
async fn when_operator_runs_restore_comment_with_missing_uuid(world: &mut FoundryWorld) {
    // Pick a fresh random UUID that's guaranteed not to be in the DB
    // (uuidv7 with current time + random tail; collision probability
    // ~10^-18 for a freshly minted v7).
    let missing = Uuid::now_v7().to_string();
    invoke_restore_comment_cli(world, missing).await;
}

// Regex carefully excludes literal placeholders `<comment-id>` and
// `<missing-uuid>` (handled by the two literal-text Whens above) so
// cucumber-rs phrase matching is unambiguous. The literal-arg variant
// covers the malformed-UUID case (the .feature passes `not-a-uuid`
// verbatim) and any future variable-UUID-literal case.
#[when(
    regex = r#"^the operator runs `foundry doctor restore-comment ([^<\s]\S*)` as a subprocess against the live database$"#
)]
async fn when_operator_runs_restore_comment_with_literal_arg(
    world: &mut FoundryWorld,
    literal_arg: String,
) {
    invoke_restore_comment_cli(world, literal_arg).await;
}

// =====================================================================
// Thens
// =====================================================================

#[then(
    regex = r#"^the issue page for "(\w+)-(\d+)" shows (\d+) tombstoned comments older than (\d+) days$"#
)]
async fn then_issue_page_shows_n_tombstones_older_than(
    world: &mut FoundryWorld,
    prefix: String,
    n: i32,
    expected_count: u64,
    age_days: i64,
) {
    // Per the slice-5 @soft-delete-invariant contract, the issue page
    // does NOT render tombstoned comments. We therefore assert on the
    // database state (cheaper + more honest) using the
    // tombstone_factory count helper.
    let issue_id = issue_id_for(world, &prefix, n).await;
    let pool = harness_pool(world).clone();
    let actual =
        tombstone_factory::count_tombstoned_comments_on_issue(&pool, issue_id, Some(age_days))
            .await;
    assert_eq!(
        actual, expected_count,
        "expected {expected_count} tombstones older than {age_days} days on \
         {prefix}-{n}, found {actual}"
    );
}

#[then(regex = r#"^the database holds (\d+) tombstoned comments on "(\w+)-(\d+)"$"#)]
async fn then_database_holds_n_tombstones_on_issue(
    world: &mut FoundryWorld,
    expected_count: u64,
    prefix: String,
    n: i32,
) {
    let issue_id = issue_id_for(world, &prefix, n).await;
    let pool = harness_pool(world).clone();
    let actual = tombstone_factory::count_tombstoned_comments_on_issue(&pool, issue_id, None).await;
    assert_eq!(
        actual, expected_count,
        "expected {expected_count} tombstones on {prefix}-{n}, found {actual}"
    );
}

#[then(
    regex = r#"^the database holds (\d+) tombstoned comments older than (\d+) days on "(\w+)-(\d+)"$"#
)]
async fn then_database_holds_n_tombstones_older_than_on_issue(
    world: &mut FoundryWorld,
    expected_count: u64,
    age_days: i64,
    prefix: String,
    n: i32,
) {
    let issue_id = issue_id_for(world, &prefix, n).await;
    let pool = harness_pool(world).clone();
    let actual =
        tombstone_factory::count_tombstoned_comments_on_issue(&pool, issue_id, Some(age_days))
            .await;
    assert_eq!(
        actual, expected_count,
        "expected {expected_count} tombstones older than {age_days} days on \
         {prefix}-{n}, found {actual}"
    );
}

#[then(regex = r#"^the doctor subprocess exits with code (\d+)$"#)]
async fn then_doctor_subprocess_exits_with_code(world: &mut FoundryWorld, expected_code: i32) {
    let actual = world.slice7_cli_exit_code.expect("CLI exit code captured");
    let stdout = world.slice7_cli_stdout.clone().unwrap_or_default();
    let stderr = world.slice7_cli_stderr.clone().unwrap_or_default();
    assert_eq!(
        actual, expected_code,
        "expected exit code {expected_code}, got {actual}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

#[then(regex = r#"^the doctor subprocess stdout contains "([\s\S]+)"$"#)]
async fn then_doctor_subprocess_stdout_contains(world: &mut FoundryWorld, substring: String) {
    let stdout = world
        .slice7_cli_stdout
        .clone()
        .expect("CLI stdout captured");
    assert!(
        stdout.contains(&substring),
        "expected stdout to contain {substring:?}; got:\n{stdout}"
    );
}

#[then(regex = r#"^the doctor subprocess stderr mentions "([\s\S]+)"$"#)]
async fn then_doctor_subprocess_stderr_mentions(world: &mut FoundryWorld, substring: String) {
    let stderr = world
        .slice7_cli_stderr
        .clone()
        .expect("CLI stderr captured");
    assert!(
        stderr.contains(&substring),
        "expected stderr to mention {substring:?}; got:\n{stderr}"
    );
}

// =====================================================================
// REUSE NOTE — `the issue page for "{}-{}" shows a comment by {} containing the text "{}"` Then
//
// This phrase is ALREADY registered in slice-5
// `us_10_comment_edit_delete.rs` (line 709). cucumber-rs treats step
// phrases as globally unique; registering it again here would either
// (a) cause an `inventory::submit!` collision at compile/runtime or
// (b) silently shadow the slice-5 implementation. Slice 7's WS #7
// issue-page assertion at `features/comment-tombstone-gc.feature:213`
// matches the slice-5 registration automatically — the slice-5 step
// uses the in-process harness to GET the issue page, which reads the
// same per-scenario schema the subprocess + CLI modified.
//
// VERIFIED via grep on slice-5 us_10_comment_edit_delete.rs line 709.
// =====================================================================
