//! Slice-8 step definitions — deferred-metrics emission.
//!
//! Ships the 5 deferred observability metrics from the slice-6 D0
//! catalog (outbox/bootstrap gauges, migration histogram, listen-
//! disconnect + probe-failure counters). EVERY metric assertion is read
//! at the operator's observable port — the `/metrics` scrape — via the
//! bounded-poll helpers in `support::metrics_scrape` (never a one-shot
//! exact scrape of an async-updated metric; see
//! `docs/evolution/2026-05-28-gc-transient-state-hardening.md`).
//!
//! REUSED phrases (registered by slice-6 `handler_instrumentation.rs` /
//! slice-1 Background — NOT re-registered here, per the phrase-collision
//! check in `distill/step-skeletons.md`):
//!   - `the operator's foundry instance is running`
//!   - `the operator scrapes the metrics endpoint [immediately]`
//!   - `the scrape returns HTTP 200`
//!   - `the scrape body contains the line "{}"`
//!   - `the scrape body's "{}" sample settles to {} within {} seconds`
//!   - `the scrape body's "{}" samples carry only the label keys "{}"`
//!     (slice-6 impl extended in this slice to branch on the empty CSV
//!     case for the 3 unlabelled metrics — DISTILL disambiguation #1)
//!   - `the foundry subprocess is alive`
//!
//! World additions used by these steps live in the slice-8 block of
//! `FoundryWorld`.
//!
//! Subprocess strategy mirrors slice-6/7: a real `foundry` subprocess
//! per scenario (the in-process harness skips `install_recorder()`, so
//! only the subprocess provides a real recorder + `/metrics` sidecar).
//! The slice-8 gauge/counter scenarios spawn the subprocess via the
//! slice-6 `FoundrySubprocess` with a per-scenario poll cadence; the
//! migration + disconnect + probe-failure scenarios spawn bespoke
//! subprocess variants (fresh-unmigrated schema / dedicated restartable
//! DB / pre-bound metrics port) defined below.

#![allow(unused_imports)]

use crate::steps::handler_instrumentation::FoundrySubprocess;
use crate::support::harness::{
    ensure_postgres, fresh_schema_pool_no_migrations, fresh_schema_pool_with_url,
};
use crate::support::metrics_scrape::{
    parse_exposition, poll_until_sample, scrape_metrics, scrape_metrics_raw, MetricSample,
    ScrapeSnapshot,
};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use sha2::{Digest, Sha256};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::{Connection, PgPool};
use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::ContainerAsync;

/// 32+-byte test session secret (production rejects shorter). Mirrors
/// the slice-6 fixture so the subprocess boots identically.
const TEST_SESSION_SECRET: &str = "slice-8-test-secret-must-be-at-least-32-bytes-long-please-yes";

// =====================================================================
// DedicatedDb — a restartable per-scenario Postgres for the real
// LISTEN-disconnect scenario (#7b). Owning its OWN container (NOT the
// shared one) lets the scenario `stop()` + `start()` it to force a real
// LISTEN drop without poisoning siblings and WITHOUT a production seam
// (DD-5 / slice-7 deviation #2 honoured).
// =====================================================================

/// A dedicated Postgres container the listen-disconnect scenario owns
/// and can restart. The `public` schema is migrated by the slice's
/// `run_migrations` (the timed migrator) before the subprocess spawns;
/// the subprocess then boots with `FOUNDRY_SKIP_MIGRATIONS=1`.
pub struct DedicatedDb {
    container: ContainerAsync<Postgres>,
    /// `postgres://...` URL pointing at the container's default db.
    pub url: String,
}

impl std::fmt::Debug for DedicatedDb {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DedicatedDb")
            .field("url", &self.url)
            .finish()
    }
}

impl DedicatedDb {
    /// Boot a fresh dedicated Postgres + migrate its `public` schema.
    async fn spawn() -> Self {
        let container: ContainerAsync<Postgres> = Postgres::default()
            .start()
            .await
            .expect("start dedicated postgres container");
        let host = container.get_host().await.expect("dedicated pg host");
        let port = container
            .get_host_port_ipv4(5432)
            .await
            .expect("dedicated pg port");
        let url = format!("postgres://postgres:postgres@{host}:{port}/postgres");

        // Migrate the dedicated DB's public schema so the subprocess can
        // boot against it with FOUNDRY_SKIP_MIGRATIONS=1.
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .acquire_timeout(Duration::from_secs(10))
            .connect(&url)
            .await
            .expect("connect to dedicated pg for migration");
        foundry_store::run_migrations(&pool)
            .await
            .expect("migrate dedicated pg");
        pool.close().await;

        Self { container, url }
    }

    /// Stop then start the container — forces a REAL LISTEN drop on the
    /// subprocess's `run_pg_listener` task, then lets it reconnect. The
    /// host port mapping persists across a stop/start (the container is
    /// not removed), so the listener reconnects to the same address.
    async fn restart(&self) {
        self.container.stop().await.expect("stop dedicated pg");
        self.container.start().await.expect("start dedicated pg");
    }
}

// =====================================================================
// Helpers
// =====================================================================

/// Spawn a subprocess against the slice-1 testcontainers Postgres with a
/// FRESH, UNMIGRATED per-scenario schema, NOT skipping migrations — so
/// the production `store.migrate()` boot path applies the real migration
/// set and emits one `migration_apply_duration_seconds{migration_id}`
/// observation per migration. Returns the subprocess + the schema name
/// (so a sibling boot can target the same, now-migrated, schema).
async fn spawn_subprocess_migrating_fresh_schema(
    world: &mut FoundryWorld,
) -> (FoundrySubprocess, String) {
    let (schema, pool, url) = fresh_schema_pool_no_migrations().await;
    // Drop the helper pool — the subprocess opens its own.
    pool.close().await;
    let subprocess = spawn_with_overrides(
        &url,
        schema.clone(),
        1,
        &[
            // Do NOT skip migrations — the subprocess must run the real
            // migration set so the histogram records each apply.
            ("FOUNDRY_SKIP_MIGRATIONS", "0".to_string()),
        ],
    )
    .await;
    let _ = world;
    (subprocess, schema)
}

/// Spawn a subprocess against an ALREADY-migrated schema, NOT skipping
/// migrations — the production `migrate!` set is all already-applied, so
/// ZERO new histogram observations are recorded (the honest no-op
/// semantic). Used by #6's second boot + #11.
async fn spawn_subprocess_against_migrated_schema(
    schema: &str,
    pool_poll_seconds: u64,
) -> FoundrySubprocess {
    let base = ensure_postgres().await;
    let url = format!("{base}?options=-csearch_path%3D{schema}");
    spawn_with_overrides(
        &url,
        schema.to_string(),
        pool_poll_seconds,
        &[("FOUNDRY_SKIP_MIGRATIONS", "0".to_string())],
    )
    .await
}

/// Thin wrapper around `FoundrySubprocess::spawn_with_env_overrides`
/// that panics on spawn failure with the captured subprocess log.
async fn spawn_with_overrides(
    database_url_with_schema: &str,
    db_schema: String,
    pool_poll_seconds: u64,
    overrides: &[(&str, String)],
) -> FoundrySubprocess {
    FoundrySubprocess::spawn_with_env_overrides(
        database_url_with_schema,
        db_schema,
        pool_poll_seconds,
        overrides,
    )
    .await
    .expect("spawn foundry subprocess")
}

/// The metrics addr of the current scenario's subprocess.
fn current_metrics_addr(world: &FoundryWorld) -> SocketAddr {
    world
        .slice6_foundry
        .as_ref()
        .expect("foundry subprocess running")
        .metrics_addr
}

/// The main HTTP addr of the current scenario's subprocess.
fn current_main_addr(world: &FoundryWorld) -> SocketAddr {
    world
        .slice6_foundry
        .as_ref()
        .expect("foundry subprocess running")
        .main_addr
}

/// The pool of the slice-1 Background InProcHarness (the subprocess
/// shares this schema). Used to seed bootstrap-token fixtures directly.
fn harness_pool(world: &FoundryWorld) -> &PgPool {
    world
        .harness
        .as_ref()
        .expect("Background steps create the in-process harness")
        .app
        .state
        .store
        .pool()
}

// =====================================================================
// Givens
// =====================================================================

#[given(
    expr = "the operator's foundry instance is running with the gauge poll cadence set to {int} second"
)]
async fn given_foundry_running_with_gauge_cadence(world: &mut FoundryWorld, seconds: u64) {
    if world.slice6_foundry.is_some() {
        return;
    }
    // Reuse the slice-1 Background schema (seeded with workspace/team/
    // project/issue rows the subprocess sees), or a fresh migrated
    // schema if no Background ran.
    let (schema, database_url) = match &world.harness {
        Some(harness) => {
            let base = ensure_postgres().await;
            let schema = harness.schema.clone();
            (
                schema.clone(),
                format!("{base}?options=-csearch_path%3D{schema}"),
            )
        }
        None => {
            let (schema, _pool, url) = fresh_schema_pool_with_url().await;
            (schema, url)
        }
    };
    world.slice6_schema = Some(schema.clone());
    let subprocess = spawn_with_overrides(&database_url, schema, seconds, &[]).await;
    world.slice6_foundry = Some(subprocess);
}

/// Direct-SQL bootstrap-token fixture (mirrors slice-7 tombstone_factory:
/// production handler untouched). Seeds an unclaimed, unexpired token.
/// Stores the RAW token so the claim step (#4) can drive the real
/// `/bootstrap` claim path.
#[given(expr = "an unclaimed admin bootstrap token that has not yet expired exists")]
async fn given_unclaimed_unexpired_bootstrap_token(world: &mut FoundryWorld) {
    let raw = "slice8-unclaimed-token-001";
    let expires_at = time::OffsetDateTime::now_utc() + time::Duration::hours(24);
    insert_bootstrap_token(harness_pool(world), raw, expires_at, /*used=*/ false).await;
    world
        .minted_tokens
        .insert("slice8-unclaimed".to_string(), raw.to_string());
}

#[given(expr = "a used admin bootstrap token exists")]
async fn given_used_bootstrap_token(world: &mut FoundryWorld) {
    let raw = "slice8-used-token-002";
    let expires_at = time::OffsetDateTime::now_utc() + time::Duration::hours(24);
    insert_bootstrap_token(harness_pool(world), raw, expires_at, /*used=*/ true).await;
}

#[given(expr = "an expired admin bootstrap token exists")]
async fn given_expired_bootstrap_token(world: &mut FoundryWorld) {
    let raw = "slice8-expired-token-003";
    // expires_at in the past — unclaimed but expired, so it must NOT
    // count toward `bootstrap_tokens_unclaimed`.
    let expires_at = time::OffsetDateTime::now_utc() - time::Duration::hours(1);
    insert_bootstrap_token(harness_pool(world), raw, expires_at, /*used=*/ false).await;
}

/// Migration-timing setup (#5). Spawns a subprocess against a FRESH
/// UNMIGRATED schema so the production migrate path applies + times the
/// real migration set.
///
/// DEVIATION (documented): the DISTILL phrasing said "staged with one
/// extra migration on top of the production set" using the slice-4
/// `test_migrations_dir` seam. That seam is a `test-support`-gated
/// AppState field NOT carried by the production `foundry` binary the
/// subprocess runs, and wiring runtime migration injection into the
/// production binary would add exactly the production test-only seam
/// DD-5 / constraint #4 forbids. Instead we boot against a fresh schema
/// so the REAL production migration set (6 files) applies and is timed —
/// the observable contract ("≥1 observation, labelled migration_id") is
/// satisfied honestly, with NO production seam.
#[given(
    expr = "the operator's foundry instance is staged with one extra migration on top of the production set"
)]
async fn given_foundry_staged_with_extra_migration(world: &mut FoundryWorld) {
    // The boot itself happens in the When; here we only record that this
    // scenario wants the fresh-schema migrating subprocess. We reuse the
    // staged-migrations slot as a marker (None body — fresh schema is
    // provisioned at boot time).
    world.slice8_migrated_schema = None;
}

/// Migration no-op setup (#6) + cardinality setup (#11): boot a
/// subprocess that migrates a fresh schema, then remember the
/// now-migrated schema so a SECOND boot (or the cardinality scrape)
/// targets it.
#[given(expr = "the operator's foundry instance has already applied its full migration set")]
async fn given_foundry_already_migrated(world: &mut FoundryWorld) {
    let (subprocess, schema) = spawn_subprocess_migrating_fresh_schema(world).await;
    world.slice8_migrated_schema = Some(schema);
    world.slice6_foundry = Some(subprocess);
}

/// Record the current migration-timing observation count as the baseline
/// for the "has not grown" assertion (#6). Bounded-poll the count to
/// appear (the histogram is absent until the first apply) so the
/// baseline is captured AFTER the first boot's migrations landed.
#[given(expr = "the migration-timing observation count has been recorded")]
async fn given_migration_observation_count_recorded(world: &mut FoundryWorld) {
    let addr = current_metrics_addr(world);
    // Wait for the histogram to appear (first boot's applies), then
    // record the count line's value.
    let sample = poll_until_sample(
        addr,
        "migration_apply_duration_seconds_count",
        |s: &MetricSample| s.value >= 1.0,
        Duration::from_secs(15),
    )
    .await;
    let _ = sample;
    let snap = scrape_metrics(addr).await;
    let count = snap.histogram_observation_count("migration_apply_duration_seconds");
    world.slice8_recorded_observation_count = Some(count);
}

/// Listen-disconnect setup (#7b): boot a dedicated, restartable Postgres,
/// migrate it, then spawn the subprocess against it (skipping migrations
/// — the dedicated DB is already migrated). No production seam (DD-5).
#[given(
    expr = "the operator's foundry instance is running against a dedicated database it can lose"
)]
async fn given_foundry_running_against_dedicated_db(world: &mut FoundryWorld) {
    let db = DedicatedDb::spawn().await;
    // Spawn the subprocess against the dedicated DB's public schema.
    let subprocess = spawn_with_overrides(
        &db.url,
        "public".to_string(),
        1,
        &[("FOUNDRY_SKIP_MIGRATIONS", "1".to_string())],
    )
    .await;
    world.slice8_dedicated_db = Some(db);
    world.slice6_foundry = Some(subprocess);
    // Give the LISTEN task a beat to establish the connection so the
    // first drop is observable as a real disconnect.
    tokio::time::sleep(Duration::from_secs(1)).await;
}

/// Probe-failure setup (#8): pre-bind the metrics port so the
/// subprocess's `metrics_server::serve` bind fails and the startup probe
/// path refuses to start (slice-6 ADR-014 precedent). We bind a real
/// `std::net::TcpListener` and hold it on the world.
#[given(expr = "the metrics port is already bound by another process before boot")]
async fn given_metrics_port_prebound(world: &mut FoundryWorld) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind a port to squat on for the probe-failure scenario");
    let port = listener.local_addr().expect("local_addr").port();
    world.slice8_prebound_metrics_listener = Some(listener);
    world.slice8_prebound_metrics_port = Some(port);
}

/// Store-probe-failure setup — provision a schema that is fully migrated
/// EXCEPT for the migration-0006 `comments` columns, which we drop. The
/// pre-probe boot steps (notably the `workspaces` bootstrap check at
/// main.rs:142) still succeed, so when the subprocess boots with
/// migrations skipped the `store` startup probe — which counts the
/// `updated_at/deleted_at/deleted_by` columns — is the SOLE refuse-to-start
/// cause. That failure flows through `record_probe_result`, so the
/// `record_probe_result -> Ok(())` mutant (which would swallow it and let
/// the process boot) is caught by the refuse-to-start assertions.
#[given(
    expr = "the operator's foundry instance is missing the latest migration's database columns"
)]
async fn given_foundry_missing_latest_migration_columns(world: &mut FoundryWorld) {
    // Use a DEDICATED single-schema container so the test is deterministic and
    // independent of the shared harness's per-scenario search_path. The
    // dedicated DB has only `public`, so the subprocess's current_schema()
    // resolves to it; dropping `public.comments`' migration-0006 columns there
    // makes `Store::probe()`'s (now current_schema()-scoped) count fall below
    // 3. The `comments` table itself remains, so the pre-probe bootstrap
    // `workspaces` check still passes and the `store` probe is the sole
    // refuse-to-start cause (exercising `record_probe_result`). Booting with
    // migrations skipped keeps the columns dropped.
    let db = DedicatedDb::spawn().await;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&db.url)
        .await
        .expect("connect to dedicated pg to degrade its schema");
    sqlx::query(
        "ALTER TABLE comments \
           DROP COLUMN updated_at, \
           DROP COLUMN deleted_at, \
           DROP COLUMN deleted_by",
    )
    .execute(&pool)
    .await
    .expect("drop migration-0006 comments columns to fail the store probe");
    pool.close().await;
    world.slice8_store_probe_db = Some((db.url.clone(), "public".to_string()));
    // Hold the container alive for the duration of the scenario.
    world.slice8_dedicated_db = Some(db);
}

// =====================================================================
// Whens
// =====================================================================

/// Multi-comment write (#1) — each POST enqueues an outbox row via the
/// COMMIT-time NOTIFY trigger. Reuses the slice-6 single-comment POST
/// path in a loop.
#[when(expr = "Mei posts {int} comments on {string}")]
async fn when_mei_posts_n_comments(world: &mut FoundryWorld, n: u32, issue: String) {
    let (prefix, number) = split_issue_key(&issue);
    let base = format!("http://{}", current_main_addr(world));
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(false)
        .build()
        .expect("build comment http client");
    let session_cookie = sign_in_to_subprocess(
        &http,
        &base,
        "mei@acme.com",
        "mei-correct-horse-battery-staple",
    )
    .await;
    let (team_slug, project_slug) = lookup_team_and_project_slug(world, &prefix).await;
    let csrf_token = fetch_csrf_token(&http, &base, &session_cookie).await;
    let combined_cookie = format!("{session_cookie}; foundry_csrf={csrf_token}");
    let url = format!("{base}/team/{team_slug}/project/{project_slug}/issues/{number}/comments");
    for i in 0..n {
        let mut form = std::collections::HashMap::new();
        form.insert("body", format!("slice-8 outbox comment {i}"));
        form.insert("_csrf", csrf_token.clone());
        let resp = http
            .post(&url)
            .header(reqwest::header::COOKIE, combined_cookie.clone())
            .form(&form)
            .send()
            .await
            .expect("post comment to subprocess");
        let _ = resp.text().await;
    }
}

/// Claim admin with the seeded unclaimed token (#4) — drives the REAL
/// `/bootstrap` claim path (`Store::claim_bootstrap_token` marks
/// `used_at`), so the gauge transitions 1 -> 0.
#[when(expr = "the operator claims admin with the unclaimed bootstrap token")]
async fn when_operator_claims_admin(world: &mut FoundryWorld) {
    let raw = world
        .minted_tokens
        .get("slice8-unclaimed")
        .expect("unclaimed token seeded by the prior Given")
        .clone();
    let base = format!("http://{}", current_main_addr(world));
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(false)
        .build()
        .expect("build claim http client");
    // GET /bootstrap to mint a CSRF cookie+token.
    let get = http
        .get(format!("{base}/bootstrap?token={raw}"))
        .send()
        .await
        .expect("get /bootstrap for csrf");
    let csrf_token = extract_csrf_from_set_cookie(&get);
    let mut form = std::collections::HashMap::new();
    form.insert("email", "claimer@acme.com".to_string());
    form.insert(
        "password",
        "claimer-correct-horse-battery-staple".to_string(),
    );
    form.insert("display_name", "Claimer".to_string());
    form.insert("workspace_name", "Claimed Workspace".to_string());
    form.insert("_csrf", csrf_token.clone());
    // POST /bootstrap?token=<raw>. The handler claims the token FIRST
    // (the observable that drives the gauge); the subsequent workspace
    // creation may fail (a workspace already exists from Background), but
    // the claim UPDATE — the gauge driver — has already fired.
    let resp = http
        .post(format!("{base}/bootstrap?token={raw}"))
        .header(
            reqwest::header::COOKIE,
            format!("foundry_csrf={csrf_token}"),
        )
        .form(&form)
        .send()
        .await
        .expect("post /bootstrap claim");
    let _ = resp.text().await;
}

/// Migration boot (#5) — boot a subprocess that migrates a fresh schema.
#[when(expr = "the operator's foundry instance boots and applies its migrations")]
async fn when_foundry_boots_and_migrates(world: &mut FoundryWorld) {
    let (subprocess, schema) = spawn_subprocess_migrating_fresh_schema(world).await;
    world.slice8_migrated_schema = Some(schema);
    world.slice6_foundry = Some(subprocess);
}

/// Second boot against the already-migrated schema (#6) — applies ZERO
/// new migrations.
#[when(expr = "a second foundry instance boots against the already-migrated schema")]
async fn when_second_foundry_boots_already_migrated(world: &mut FoundryWorld) {
    let schema = world
        .slice8_migrated_schema
        .clone()
        .expect("a prior boot recorded the migrated schema");
    // Drop the first subprocess so its metrics addr is replaced by the
    // second instance's (the second instance shares the schema state but
    // has its own recorder / `/metrics`).
    world.slice6_foundry = None;
    let subprocess = spawn_subprocess_against_migrated_schema(&schema, 1).await;
    world.slice6_foundry = Some(subprocess);
}

/// Force a REAL LISTEN drop by restarting the dedicated DB (#7b).
#[when(expr = "the realtime LISTEN connection is dropped by restarting that database")]
async fn when_listen_connection_dropped_by_db_restart(world: &mut FoundryWorld) {
    let db = world
        .slice8_dedicated_db
        .as_ref()
        .expect("dedicated DB booted by the prior Given");
    db.restart().await;
}

/// Probe-failure boot (#8) — spawn a subprocess with METRICS_PORT bound
/// to the pre-squatted port; the bind fails, the startup probe path
/// refuses to start. Capture the exit code + stdout/stderr.
#[when(expr = "the operator's foundry instance attempts to start")]
async fn when_foundry_attempts_to_start(world: &mut FoundryWorld) {
    let port = world
        .slice8_prebound_metrics_port
        .expect("metrics port pre-bound by the prior Given");
    // Provision a fresh migrated schema for the subprocess so the ONLY
    // failure is the metrics-port bind, not a migration error.
    let (schema, pool, url) = fresh_schema_pool_with_url().await;
    // Release the helper pool's connections promptly — the subprocess
    // doesn't need them and holding 10 conns during the boot-wait adds
    // needless pressure on the shared container under @all contention.
    pool.close().await;
    let outcome = spawn_subprocess_expecting_refuse_to_start(&url, &schema, port).await;
    world.slice8_refused_start_outcome = Some(outcome);
}

/// Store-probe-failure boot — spawn against the migration-0006-degraded
/// schema with migrations skipped and an ephemeral metrics port (so the
/// metrics sidecar binds fine and the `store` probe is the sole failure).
/// Capture the refuse-to-start outcome into the shared slot.
#[when(expr = "the operator's foundry instance attempts to start without applying migrations")]
async fn when_foundry_attempts_to_start_without_migrations(world: &mut FoundryWorld) {
    let (url, schema) = world
        .slice8_store_probe_db
        .clone()
        .expect("store-probe-failing schema provisioned by the prior Given");
    let outcome = spawn_subprocess_expecting_store_probe_failure(&url, &schema).await;
    world.slice8_refused_start_outcome = Some(outcome);
}

// =====================================================================
// Thens
// =====================================================================

/// Gauge/counter lower-bound bounded-poll (#1, #7b) — "value >= N",
/// held to the deadline via `poll_until_sample`. Serves BOTH gauges and
/// monotonic counters (the predicate is identical — DISTILL
/// disambiguation #2).
#[then(
    expr = "the scrape body's {string} sample is eventually at least {int} within {int} seconds"
)]
async fn then_sample_eventually_at_least(
    world: &mut FoundryWorld,
    metric: String,
    n: i64,
    secs: u64,
) {
    let addr = current_metrics_addr(world);
    let threshold = n as f64;
    let _ = poll_until_sample(
        addr,
        &metric,
        move |s: &MetricSample| s.value >= threshold,
        Duration::from_secs(secs),
    )
    .await;
}

/// Histogram observation-count bounded-poll (#5) — poll the
/// `{name}_count` series until its summed value reaches >= N.
#[then(
    expr = "the scrape body eventually contains a {string} observation count of at least {int} within {int} seconds"
)]
async fn then_histogram_observation_count_at_least(
    world: &mut FoundryWorld,
    metric: String,
    n: u64,
    secs: u64,
) {
    let addr = current_metrics_addr(world);
    let count_series = format!("{metric}_count");
    let threshold = n as f64;
    // The exporter renders one `_count` line per migration_id; the
    // observation total is their SUM. poll_until_sample matches a single
    // sample, so we instead poll-scrape and sum until the deadline.
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    loop {
        let snap = scrape_metrics(addr).await;
        let total = snap.histogram_observation_count(&metric);
        if total >= n {
            // Capture the satisfying snapshot so a following slice-6
            // `samples carry only the label keys "..."` Then (which reads
            // `slice6_last_scrape`) has a body to assert against — #5 has
            // no explicit scrape When.
            world.slice6_last_scrape_status = Some(reqwest::StatusCode::OK);
            world.slice6_last_scrape = Some(snap);
            return;
        }
        if std::time::Instant::now() >= deadline {
            panic!(
                "histogram `{metric}` observation count was {total}, expected >= {threshold} \
                 within {secs}s. `{count_series}` samples:\n{:#?}",
                snap.samples_for(&count_series),
            );
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

/// Histogram no-op semantic (#6) — the observation count did not grow
/// past the recorded baseline. Bounded-poll a few seconds to give the
/// second boot's tick a chance to (incorrectly) emit, then assert the
/// total is unchanged.
#[then(expr = "the scrape body's {string} observation count has not grown")]
async fn then_histogram_observation_count_unchanged(world: &mut FoundryWorld, metric: String) {
    let baseline = world
        .slice8_recorded_observation_count
        .expect("baseline observation count recorded by the prior Given");
    let addr = current_metrics_addr(world);
    // Wait a couple of seconds so any (incorrect) emission from the
    // second boot would have landed. The second instance has its OWN
    // recorder, so on an already-migrated schema it MUST record ZERO
    // observations (every migration is already-applied → no `conn.apply`
    // → no `.record(...)`). Asserting == 0 (not merely <= baseline)
    // makes this falsifiable: if the honest no-op semantic broke and the
    // second instance re-timed the already-applied migrations, it would
    // emit `baseline`-many observations on its fresh recorder and this
    // assertion would fail (a `<= baseline` check would have passed it
    // silently — Testing Theater).
    let _ = baseline;
    tokio::time::sleep(Duration::from_secs(2)).await;
    let snap = scrape_metrics(addr).await;
    let now = snap.histogram_observation_count(&metric);
    assert_eq!(
        now,
        0,
        "the second instance recorded {now} migration-timing observations against an \
         ALREADY-migrated schema; expected 0 (ADR-020 honest no-op: only migrations that \
         actually run are timed). Baseline from the first boot was {baseline}. \
         Samples:\n{:#?}",
        snap.samples_for(&format!("{metric}_count")),
    );
}

/// Migration-id label VALUE assertion (#5) — the histogram samples must
/// carry a `migration_id` whose value is the real `{version:04}_{desc}`
/// filename stem (e.g. `0001_init`), not an empty or constant placeholder.
/// The label-KEY assertion alone passes even when the value is bogus, so
/// this pins `migration_id_label`'s output (the gap surfaced by mutation
/// testing: `migration_id_label -> ""` and `-> "xyzzy"` both survived a
/// key-only check).
#[then(expr = "the scrape body's {string} samples include the migration_id value {string}")]
async fn then_samples_include_migration_id_value(
    world: &mut FoundryWorld,
    metric: String,
    expected: String,
) {
    let addr = current_metrics_addr(world);
    let snap = scrape_metrics(addr).await;
    let observed: std::collections::BTreeSet<String> = snap
        .samples_with_prefix(&metric)
        .into_iter()
        .filter_map(|s| s.labels.get("migration_id").cloned())
        .collect();
    assert!(
        observed.contains(&expected),
        "`{metric}` carried migration_id values {observed:?}, expected to include {expected:?} \
         (the real `{{version:04}}_{{desc}}` stem). Catches `migration_id_label` emitting an \
         empty or constant placeholder. Samples:\n{:#?}",
        snap.samples_with_prefix(&metric),
    );
}

/// Bounded probe-name value set (#9) — the closed {store, metrics} set.
#[then(expr = "the scrape body's {string} samples carry only the probe names {string}")]
async fn then_samples_carry_only_probe_names(
    world: &mut FoundryWorld,
    metric: String,
    csv_names: String,
) {
    let addr = current_metrics_addr(world);
    let snap = scrape_metrics(addr).await;
    let expected: std::collections::BTreeSet<String> =
        csv_names.split(',').map(|s| s.trim().to_string()).collect();
    let observed: std::collections::BTreeSet<String> = snap
        .samples_for(&metric)
        .into_iter()
        .filter_map(|s| s.labels.get("probe_name").cloned())
        .collect();
    assert_eq!(
        observed,
        expected,
        "`{metric}` probe_name values were {observed:?}, expected exactly {expected:?}. \
         Samples:\n{:#?}",
        snap.samples_for(&metric),
    );
}

/// Refuse-to-start: non-zero exit (#8).
#[then(expr = "the foundry subprocess exits non-zero")]
async fn then_foundry_exits_nonzero(world: &mut FoundryWorld) {
    let (code, stdout, stderr) = world
        .slice8_refused_start_outcome
        .as_ref()
        .expect("probe-failure boot outcome captured");
    match code {
        Some(c) => assert_ne!(
            *c, 0,
            "expected non-zero exit on refuse-to-start, got {c}.\nstdout:\n{stdout}\nstderr:\n{stderr}"
        ),
        None => panic!(
            "subprocess did not exit (no code captured) — expected refuse-to-start.\n\
             stdout:\n{stdout}\nstderr:\n{stderr}"
        ),
    }
}

/// Refuse-to-start: the `health.startup.refused` log line (#8).
#[then(expr = "the foundry startup log mentions {string}")]
async fn then_startup_log_mentions(world: &mut FoundryWorld, fragment: String) {
    let (_, stdout, stderr) = world
        .slice8_refused_start_outcome
        .as_ref()
        .expect("probe-failure boot outcome captured");
    let haystack = format!("{stdout}\n{stderr}");
    assert!(
        haystack.contains(&fragment),
        "startup log did not mention {fragment:?}.\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

/// Refuse-to-start: the probe-name in the failure log (#8).
#[then(expr = "the foundry startup log mentions probe failure for probe {string}")]
async fn then_startup_log_mentions_probe_failure(world: &mut FoundryWorld, probe_name: String) {
    let (_, stdout, stderr) = world
        .slice8_refused_start_outcome
        .as_ref()
        .expect("probe-failure boot outcome captured");
    let haystack = format!("{stdout}\n{stderr}");
    // The slice-6 metrics probe logs `probe="metrics"` on
    // `health.startup.refused`. Accept either the structured field
    // (`probe="metrics"` / `probe=metrics`) or the bare name in context.
    let matched = haystack.contains(&format!("probe=\"{probe_name}\""))
        || haystack.contains(&format!("probe={probe_name}"))
        || haystack.contains(&format!("\"probe\": \"{probe_name}\""))
        || (haystack.contains("health.startup.refused") && haystack.contains(&probe_name));
    assert!(
        matched,
        "startup log did not mention probe failure for probe {probe_name:?}.\n\
         stdout:\n{stdout}\nstderr:\n{stderr}"
    );
}

// =====================================================================
// Subprocess refuse-to-start spawn (probe-failure scenario #8).
// =====================================================================

/// Spawn a subprocess with `METRICS_PORT` bound to a port already held
/// by the test. The subprocess's `metrics_server::serve` bind fails (or
/// the startup probe path otherwise refuses to start). We DON'T wait for
/// both addr-bind lines (it will never bind both); instead we wait for
/// the process to exit, capturing (exit_code, stdout, stderr).
async fn spawn_subprocess_expecting_refuse_to_start(
    database_url_with_schema: &str,
    db_schema: &str,
    prebound_metrics_port: u16,
) -> (Option<i32>, String, String) {
    use std::process::Stdio;
    use tokio::process::Command;

    let binary_path = assert_cmd::cargo::cargo_bin("foundry");
    let mut cmd = Command::new(&binary_path);
    cmd.env("DATABASE_URL", database_url_with_schema)
        .env("METRICS_PORT", prebound_metrics_port.to_string())
        .env("FOUNDRY_PORT", "0")
        .env("METRICS_HOST", "127.0.0.1")
        .env("FOUNDRY_HOST", "127.0.0.1")
        .env("SESSION_SECRET", TEST_SESSION_SECRET)
        .env("SESSION_COOKIE_SECURE", "false")
        .env("FOUNDRY_DB_SCHEMA", db_schema)
        // The schema is already migrated by fresh_schema_pool_with_url,
        // so skip migrations — the ONLY failure should be the metrics
        // bind, not a migration error.
        .env("FOUNDRY_SKIP_MIGRATIONS", "1")
        .env("RUST_LOG", "info,foundry=info,sqlx=warn")
        .env("RUST_LOG_FORMAT", "pretty")
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let child = cmd.spawn().expect("spawn refuse-to-start subprocess");
    // Wait for the process to exit (it should refuse to start quickly).
    // Cap at 30s so a hang surfaces as a test failure rather than wedging.
    let output = tokio::time::timeout(Duration::from_secs(30), child.wait_with_output())
        .await
        .expect("refuse-to-start subprocess did not exit within 30s")
        .expect("collect refuse-to-start subprocess output");
    let code = output.status.code();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (code, stdout, stderr)
}

/// Spawn a subprocess against a schema missing the migration-0006 columns
/// with `FOUNDRY_SKIP_MIGRATIONS=1` and an ephemeral metrics port
/// (`METRICS_PORT=0`). The metrics sidecar binds fine; the `store` startup
/// probe then fails its migration-0006 column check and the process
/// refuses to start via `record_probe_result`. Waits for exit, capturing
/// (exit_code, stdout, stderr). Under the `record_probe_result -> Ok(())`
/// mutant the probe failure is swallowed and the process keeps running, so
/// the 30s wait elapses without an exit — surfacing as a caught mutant.
async fn spawn_subprocess_expecting_store_probe_failure(
    database_url_with_schema: &str,
    db_schema: &str,
) -> (Option<i32>, String, String) {
    use std::process::Stdio;
    use tokio::process::Command;

    let binary_path = assert_cmd::cargo::cargo_bin("foundry");
    let mut cmd = Command::new(&binary_path);
    cmd.env("DATABASE_URL", database_url_with_schema)
        // Ephemeral metrics port — the bind SUCCEEDS, isolating the store
        // probe (not a metrics-bind failure) as the refuse-to-start cause.
        .env("METRICS_PORT", "0")
        .env("FOUNDRY_PORT", "0")
        .env("METRICS_HOST", "127.0.0.1")
        .env("FOUNDRY_HOST", "127.0.0.1")
        .env("SESSION_SECRET", TEST_SESSION_SECRET)
        .env("SESSION_COOKIE_SECURE", "false")
        .env("FOUNDRY_DB_SCHEMA", db_schema)
        // Skip migrations so the dropped 0006 columns stay dropped — the
        // `store` probe's column check is the intended failure.
        .env("FOUNDRY_SKIP_MIGRATIONS", "1")
        .env("RUST_LOG", "info,foundry=info,sqlx=warn")
        .env("RUST_LOG_FORMAT", "pretty")
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let child = cmd.spawn().expect("spawn store-probe-failure subprocess");
    let output = tokio::time::timeout(Duration::from_secs(30), child.wait_with_output())
        .await
        .expect("store-probe-failure subprocess did not exit within 30s")
        .expect("collect store-probe-failure subprocess output");
    let code = output.status.code();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (code, stdout, stderr)
}

// =====================================================================
// SQL + HTTP helpers (bootstrap-token seeding, sign-in, CSRF, slug).
// =====================================================================

/// Insert a bootstrap-token row directly (mirrors slice-7
/// tombstone_factory's direct-SQL approach; production handler
/// untouched). `used=true` sets `used_at=now()`.
async fn insert_bootstrap_token(
    pool: &PgPool,
    raw_token: &str,
    expires_at: time::OffsetDateTime,
    used: bool,
) {
    let mut hasher = Sha256::new();
    hasher.update(raw_token.as_bytes());
    let hash: Vec<u8> = hasher.finalize().to_vec();
    let id = uuid::Uuid::now_v7();
    if used {
        sqlx::query(
            "INSERT INTO bootstrap_tokens (id, token_hash, expires_at, used_at)
             VALUES ($1, $2, $3, now())",
        )
        .bind(id)
        .bind(&hash)
        .bind(expires_at)
        .execute(pool)
        .await
        .expect("seed used bootstrap token");
    } else {
        sqlx::query(
            "INSERT INTO bootstrap_tokens (id, token_hash, expires_at) VALUES ($1, $2, $3)",
        )
        .bind(id)
        .bind(&hash)
        .bind(expires_at)
        .execute(pool)
        .await
        .expect("seed unclaimed bootstrap token");
    }
}

/// Sign in against the subprocess; return the `foundry_session=...`
/// cookie pair. Mirrors the slice-6 helper.
async fn sign_in_to_subprocess(
    http: &reqwest::Client,
    base: &str,
    email: &str,
    password: &str,
) -> String {
    let csrf_get = http
        .get(format!("{base}/sign-in"))
        .send()
        .await
        .expect("get /sign-in for csrf");
    let csrf_token = extract_csrf_from_set_cookie(&csrf_get);
    let mut form = std::collections::HashMap::new();
    form.insert("email", email.to_string());
    form.insert("password", password.to_string());
    form.insert("_csrf", csrf_token.clone());
    let resp = http
        .post(format!("{base}/sign-in"))
        .header(
            reqwest::header::COOKIE,
            format!("foundry_csrf={csrf_token}"),
        )
        .form(&form)
        .send()
        .await
        .expect("post /sign-in to subprocess");
    let session_cookie = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_session="))
        .map(|s| s.to_string())
        .unwrap_or_else(|| panic!("no session cookie on sign-in (status {})", resp.status()));
    session_cookie
        .split(';')
        .next()
        .unwrap_or(&session_cookie)
        .to_string()
}

async fn fetch_csrf_token(http: &reqwest::Client, base: &str, session_cookie: &str) -> String {
    let resp = http
        .get(format!("{base}/sign-in"))
        .header(reqwest::header::COOKIE, session_cookie.to_string())
        .send()
        .await
        .expect("csrf fetch");
    extract_csrf_from_set_cookie(&resp)
}

/// Pull the `foundry_csrf` token value out of a response's Set-Cookie.
fn extract_csrf_from_set_cookie(resp: &reqwest::Response) -> String {
    resp.headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .and_then(|s| s.strip_prefix("foundry_csrf="))
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string()
}

/// Look up (team_slug, project_slug) for an issue key prefix.
async fn lookup_team_and_project_slug(world: &FoundryWorld, key_prefix: &str) -> (String, String) {
    let pool = harness_pool(world);
    let row: (String, String) = sqlx::query_as(
        "SELECT t.slug, p.slug
           FROM projects p
           JOIN teams t ON t.id = p.team_id
          WHERE p.key_prefix = $1
          LIMIT 1",
    )
    .bind(key_prefix)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|err| panic!("lookup project slug for prefix {key_prefix}: {err}"));
    row
}

/// Split an issue key like `AUTH-3` into ("AUTH", 3).
fn split_issue_key(key: &str) -> (String, i32) {
    let (prefix, num) = key
        .rsplit_once('-')
        .unwrap_or_else(|| panic!("malformed issue key {key:?} (expected PREFIX-N)"));
    (prefix.to_string(), num.parse().unwrap_or(0))
}
