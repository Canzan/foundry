//! Slice-6 step definitions — handler-instrumentation.
//!
//! Spawns a real `foundry` subprocess per scenario (slice-3 US-03
//! precedent via `assert_cmd::Command::cargo_bin("foundry")`) because
//! the in-process `InProcHarness` deliberately SKIPS
//! `install_recorder()` to avoid the "global recorder already installed"
//! panic on the second scenario. The `/metrics` substrate requires a
//! real recorder install + real sidecar listener; only the subprocess
//! path provides both honestly.
//!
//! See `docs/feature/handler-instrumentation/distill/driver.md` § 1-2
//! for the rationale + the rejected alternatives (docker-compose +
//! process-wide `OnceCell` recorder).
//!
//! World additions used by these steps (slice-6 block at the bottom
//! of `FoundryWorld`):
//!   - world.slice6_foundry            : Option<FoundrySubprocess>
//!   - world.slice6_last_scrape        : Option<ScrapeSnapshot>
//!   - world.slice6_last_scrape_status : Option<StatusCode>
//!   - world.slice6_request_count      : u64
//!   - world.slice6_request_count_by_route :
//!     HashMap<(String, String), u64>
//!   - world.slice6_sse_subscription   : Option<reqwest::Response>
//!   - world.slice6_held_connection    :
//!     Option<sqlx::pool::PoolConnection<sqlx::Postgres>>
//!   - world.slice6_schema             : Option<String>

#![allow(unused_imports)]

use crate::support::harness::ensure_postgres;
use crate::support::metrics_scrape::{scrape_metrics, scrape_metrics_raw, ScrapeSnapshot};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::AsyncBufReadExt;
use tokio::process::{Child, Command};

/// Test fixture session secret (32+ bytes — production rejects shorter).
const TEST_SESSION_SECRET: &str = "slice-6-test-secret-must-be-at-least-32-bytes-long-please-yes";

/// Total wall-clock we wait for both ports to bind before declaring
/// the subprocess broken. Slice-3 precedent uses `.output()` (no
/// explicit timeout — relies on the subprocess running to completion).
/// Slice 6 spawns a long-lived subprocess so we need an explicit
/// timeout; 30s gives the dev-profile binary ample headroom for
/// migration runs + Postgres connect + bind.
const SPAWN_BIND_TIMEOUT: Duration = Duration::from_secs(30);

// =====================================================================
// FoundrySubprocess — per-scenario `cargo run --bin foundry` instance.
// =====================================================================

/// The foundry subprocess for the current scenario.
///
/// Spawned by the "the operator's foundry instance is running" Given.
/// Dropped at scenario teardown via the Drop impl below; on Drop the
/// process is killed + reaped to avoid leaking children.
///
/// Uses `tokio::process::Child` instead of `std::process::Child` for
/// two reasons:
///   1. Async line-tailing of stdout — needed because the macOS pipe
///      EOFs immediately when used with sync read on a piped child.
///   2. Tokio handles SIGCHLD reaping internally — no race between
///      our `try_wait` and the kernel's wait4 reaping.
pub struct FoundrySubprocess {
    /// The tokio child handle. `Drop` kills it.
    process: Child,
    /// The bound main HTTP port (parsed from the
    /// `foundry listening on {addr}` log line emitted by main.rs).
    pub main_addr: SocketAddr,
    /// The bound metrics sidecar port (parsed from the
    /// `foundry metrics listening on {addr}` log line emitted by
    /// main.rs).
    pub metrics_addr: SocketAddr,
    /// The per-scenario PG schema name (slice-1 pattern). Captured
    /// so teardown can drop it.
    pub db_schema: String,
}

impl std::fmt::Debug for FoundrySubprocess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FoundrySubprocess")
            .field("main_addr", &self.main_addr)
            .field("metrics_addr", &self.metrics_addr)
            .field("db_schema", &self.db_schema)
            .finish()
    }
}

impl FoundrySubprocess {
    /// Spawn the foundry binary as a subprocess with:
    ///   - `DATABASE_URL` pointing at the slice-1 testcontainers
    ///     Postgres + per-scenario PG schema
    ///   - `METRICS_PORT=0` (ephemeral)
    ///   - `FOUNDRY_PORT=0` (ephemeral)
    ///   - `SESSION_SECRET=<32-byte test fixture>`
    ///   - `SESSION_COOKIE_SECURE=false`
    ///   - `METRICS_HOST=127.0.0.1`
    ///   - `FOUNDRY_HOST=127.0.0.1`
    ///   - `METRICS_POOL_POLL_SECONDS=1` so the connection-in-use
    ///     scenario doesn't have to wait the full 5s
    ///   - `RUST_LOG_FORMAT=pretty` so stdout is line-grep-able
    ///
    /// Waits up to [`SPAWN_BIND_TIMEOUT`] for both ports to bind by
    /// async-tailing the subprocess's stdout for the `foundry
    /// listening` + `foundry metrics listening` info lines.
    pub async fn spawn(
        database_url_with_schema: &str,
        db_schema: String,
        pool_poll_seconds: u64,
    ) -> anyhow::Result<Self> {
        Self::spawn_with_env_overrides(database_url_with_schema, db_schema, pool_poll_seconds, &[])
            .await
    }

    /// Slice 7 variant — same as [`spawn`] but accepts a slice of
    /// additional `(key, value)` env-var overrides applied after the
    /// baseline slice-6 env. The slice-7 GC scenarios use this to
    /// inject `FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS=1` and the
    /// per-run cap override so the GC tick fires within a per-scenario
    /// wall-clock budget. Production paths never set these vars.
    pub async fn spawn_with_env_overrides(
        database_url_with_schema: &str,
        db_schema: String,
        pool_poll_seconds: u64,
        env_overrides: &[(&str, String)],
    ) -> anyhow::Result<Self> {
        // `assert_cmd::cargo::cargo_bin` returns a PathBuf to the
        // workspace-built binary (the same path the slice-3 doctor
        // step uses).
        let binary_path = assert_cmd::cargo::cargo_bin("foundry");
        if !binary_path.exists() {
            return Err(anyhow::anyhow!(
                "foundry binary not found at {} — build with `cargo build` first",
                binary_path.display(),
            ));
        }

        // Per-scenario stdout/stderr capture files. We tee the
        // subprocess's pipes into these so a spawn failure or test
        // failure leaves the full subprocess log behind for diagnosis.
        let log_dir = std::path::PathBuf::from("/tmp/foundry-slice6");
        let _ = std::fs::create_dir_all(&log_dir);
        let stdout_path = log_dir.join(format!("{db_schema}.stdout.log"));
        let stderr_path = log_dir.join(format!("{db_schema}.stderr.log"));

        let mut cmd = Command::new(&binary_path);
        cmd.env("DATABASE_URL", database_url_with_schema)
            .env("METRICS_PORT", "0")
            .env("FOUNDRY_PORT", "0")
            .env("METRICS_HOST", "127.0.0.1")
            .env("FOUNDRY_HOST", "127.0.0.1")
            .env("SESSION_SECRET", TEST_SESSION_SECRET)
            .env("SESSION_COOKIE_SECURE", "false")
            // Tell foundry-app to look up its `session` table on
            // the per-scenario schema (slice-1 InProcHarness pattern).
            // Without this the tower-sessions middleware queries
            // `public.session` which doesn't exist in the per-scenario
            // schema, returning "relation does not exist" on every
            // session save.
            .env("FOUNDRY_DB_SCHEMA", &db_schema)
            // Skip migrations — the InProcHarness already migrated
            // the per-scenario schema. Without this, the subprocess
            // contends with the in-process harness for the global
            // migration advisory lock, adding seconds of wall-clock
            // per scenario start.
            .env("FOUNDRY_SKIP_MIGRATIONS", "1")
            // pretty format with NO_COLOR so the log lines are easy
            // to grep without ANSI escape sequences interleaving.
            .env("RUST_LOG", "info,foundry=info,sqlx=warn")
            .env("RUST_LOG_FORMAT", "pretty")
            .env("NO_COLOR", "1")
            .env("METRICS_POOL_POLL_SECONDS", pool_poll_seconds.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Tokio-only: ensure the child is killed if its `Child`
            // handle drops without explicit shutdown. Belt-and-braces
            // on top of the explicit Drop impl below.
            .kill_on_drop(true);

        // Slice 7 — additional env overrides applied AFTER the
        // baseline slice-6 env so callers can shorten the GC cadence
        // (FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS=1) or tune the
        // per-run cap (FOUNDRY_TOMBSTONE_GC_MAX_PER_RUN=2). Production
        // paths pass an empty slice — the slice-6 baseline is
        // unchanged.
        for (k, v) in env_overrides {
            cmd.env(k, v);
        }

        let mut child = cmd
            .spawn()
            .map_err(|err| anyhow::anyhow!("spawn foundry subprocess: {err}"))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow::anyhow!("subprocess stdout not piped (tokio)"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow::anyhow!("subprocess stderr not piped (tokio)"))?;

        let (addr_tx, mut addr_rx) = tokio::sync::mpsc::unbounded_channel::<(bool, SocketAddr)>();
        let addr_tx_for_stdout = addr_tx.clone();

        // Async-tail stdout. tokio::io's BufReader is line-aware and
        // doesn't need page-buffer flushing the way the std file
        // redirect does. The captured lines are appended to the
        // per-scenario stdout log file as they arrive.
        let stdout_path_clone = stdout_path.clone();
        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt as _;
            let mut log_file = tokio::fs::File::create(&stdout_path_clone).await.ok();
            let mut reader = tokio::io::BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if let Some(f) = &mut log_file {
                    let _ = f.write_all(line.as_bytes()).await;
                    let _ = f.write_all(b"\n").await;
                }
                if line.contains("foundry metrics listening") {
                    if let Some(addr) = parse_addr_from_line(&line) {
                        let _ = addr_tx_for_stdout.send((true, addr));
                    }
                } else if line.contains("foundry listening") {
                    if let Some(addr) = parse_addr_from_line(&line) {
                        let _ = addr_tx_for_stdout.send((false, addr));
                    }
                }
            }
        });

        // Async-tail stderr to its own file. Stderr only carries
        // anyhow `Error: …` output from a process that failed to
        // start; useful for diagnostics on spawn failure.
        let stderr_path_clone = stderr_path.clone();
        tokio::spawn(async move {
            use tokio::io::AsyncWriteExt as _;
            let mut log_file = tokio::fs::File::create(&stderr_path_clone).await.ok();
            let mut reader = tokio::io::BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                if let Some(f) = &mut log_file {
                    let _ = f.write_all(line.as_bytes()).await;
                    let _ = f.write_all(b"\n").await;
                }
            }
        });

        // Wait for both addr-bind log lines (or for the subprocess
        // to die, or for the deadline to expire).
        let deadline = Instant::now() + SPAWN_BIND_TIMEOUT;
        let mut main_addr: Option<SocketAddr> = None;
        let mut metrics_addr: Option<SocketAddr> = None;
        while Instant::now() < deadline {
            if main_addr.is_some() && metrics_addr.is_some() {
                break;
            }
            if let Some(status) = child.try_wait().ok().flatten() {
                tokio::time::sleep(Duration::from_millis(200)).await;
                let out = tokio::fs::read_to_string(&stdout_path)
                    .await
                    .unwrap_or_default();
                let err = tokio::fs::read_to_string(&stderr_path)
                    .await
                    .unwrap_or_default();
                return Err(anyhow::anyhow!(
                    "foundry subprocess exited before binding (status: {status}); \
                     stdout:\n{out}\nstderr:\n{err}",
                ));
            }
            let recv = tokio::time::timeout(Duration::from_millis(200), addr_rx.recv()).await;
            match recv {
                Ok(Some((is_metrics, addr))) => {
                    if is_metrics {
                        metrics_addr.get_or_insert(addr);
                    } else {
                        main_addr.get_or_insert(addr);
                    }
                }
                Ok(None) => {
                    // Sender closed — subprocess pipes EOF'd. Loop
                    // continues; try_wait will catch the exit on the
                    // next iteration.
                }
                Err(_) => { /* recv timeout — loop */ }
            }
        }

        let (Some(main_addr), Some(metrics_addr)) = (main_addr, metrics_addr) else {
            let _ = child.kill().await;
            let final_status = child.wait().await;
            // Give the async-tail tasks a beat to flush their final
            // lines to disk before we read them.
            tokio::time::sleep(Duration::from_millis(200)).await;
            let out = tokio::fs::read_to_string(&stdout_path)
                .await
                .unwrap_or_default();
            let err = tokio::fs::read_to_string(&stderr_path)
                .await
                .unwrap_or_default();
            return Err(anyhow::anyhow!(
                "foundry subprocess did not bind both ports within {:?} \
                 (main_addr={main_addr:?}, metrics_addr={metrics_addr:?}, \
                 final={final_status:?}); stdout:\n{out}\nstderr:\n{err}",
                SPAWN_BIND_TIMEOUT,
            ));
        };

        Ok(Self {
            process: child,
            main_addr,
            metrics_addr,
            db_schema,
        })
    }

    /// Best-effort graceful shutdown — kill + reap. Used at scenario
    /// teardown by [`Drop`].
    pub fn shutdown(self) {
        // Drop runs at end of scope.
        let _ = self;
    }
}

impl Drop for FoundrySubprocess {
    fn drop(&mut self) {
        // `kill_on_drop(true)` was set at spawn time; tokio handles
        // SIGKILL + reap when the `Child` is dropped. Drop must not
        // unwind in test teardown — we intentionally do nothing
        // synchronous here. The test runner's process group cleanup
        // is the ultimate safety net.
    }
}

/// Extract `127.0.0.1:NNNNN` from a tracing pretty-format line like:
///   `… INFO …: foundry listening addr=127.0.0.1:54321`
fn parse_addr_from_line(line: &str) -> Option<SocketAddr> {
    // Find `addr=...` and parse the rest until whitespace.
    let idx = line.find("addr=")?;
    let rest = &line[idx + "addr=".len()..];
    let end = rest
        .find(|c: char| c.is_whitespace() || c == ',')
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

// =====================================================================
// Helpers for spawning the subprocess + setting up the per-scenario
// schema.
// =====================================================================

async fn ensure_subprocess_running(
    world: &mut FoundryWorld,
    pool_poll_seconds: u64,
) -> anyhow::Result<()> {
    if world.slice6_foundry.is_some() {
        return Ok(());
    }

    // Reuse the slice-1 InProcHarness schema if the Background steps
    // already created one — that schema is already seeded with
    // workspace/team/project/issue rows we want the subprocess to
    // see. Otherwise create a fresh schema (rare — most slice-6
    // scenarios have a Background block).
    let (schema, database_url) = match &world.harness {
        Some(harness) => {
            let base = ensure_postgres().await;
            let schema = harness.schema.clone();
            let url = format!("{base}?options=-csearch_path%3D{schema}");
            (schema, url)
        }
        None => {
            // No Background — provision a fresh schema using slice-1
            // helper. This branch is hit only by scenarios that don't
            // need any pre-seeded state.
            let (schema, _pool, url) = crate::support::harness::fresh_schema_pool_with_url().await;
            (schema, url)
        }
    };

    world.slice6_schema = Some(schema.clone());

    // FoundrySubprocess::spawn is async (uses tokio::process under
    // the hood) so we just await directly.
    let subprocess =
        FoundrySubprocess::spawn(&database_url, schema.clone(), pool_poll_seconds).await?;

    world.slice6_foundry = Some(subprocess);
    Ok(())
}

fn current_metrics_addr(world: &FoundryWorld) -> SocketAddr {
    world
        .slice6_foundry
        .as_ref()
        .expect("foundry subprocess running")
        .metrics_addr
}

fn current_main_addr(world: &FoundryWorld) -> SocketAddr {
    world
        .slice6_foundry
        .as_ref()
        .expect("foundry subprocess running")
        .main_addr
}

// ---------------------------------------------------------------------
// Givens
// ---------------------------------------------------------------------

#[given("the operator's foundry instance is running")]
async fn given_foundry_instance_is_running(world: &mut FoundryWorld) {
    // Use a 1-second poll cadence by default so the
    // `db_connections_in_use` scenario stays under ~8s wall-clock
    // (a connection held for 6s covers several poll ticks).
    ensure_subprocess_running(world, 1)
        .await
        .expect("spawn foundry subprocess");
}

#[given(regex = r"^the operator's foundry instance has been running for at least (\d+) seconds$")]
async fn given_foundry_instance_has_been_running_for(world: &mut FoundryWorld, seconds: u64) {
    // Idempotent — if not yet running, start it; then wait for the
    // requested wall-clock. Used by the SSE-Drop scenarios to give
    // the server side a beat to register the abrupt disconnect.
    if world.slice6_foundry.is_none() {
        ensure_subprocess_running(world, 1)
            .await
            .expect("spawn foundry subprocess");
    }
    tokio::time::sleep(Duration::from_secs(seconds)).await;
}

#[given(regex = r#"^Mei has subscribed to events on "([^"]+)"$"#)]
async fn given_mei_has_subscribed_to_events(world: &mut FoundryWorld, project_name: String) {
    // Sign Mei in against the subprocess, then open an SSE
    // subscription against the project's `/events` route.
    if world.slice6_foundry.is_none() {
        ensure_subprocess_running(world, 1)
            .await
            .expect("spawn foundry subprocess");
    }

    let base = format!("http://{}", current_main_addr(world));
    let http = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(false)
        .build()
        .expect("build sse http client");

    let cookie = sign_in_to_subprocess(
        &http,
        &base,
        "mei@acme.com",
        "mei-correct-horse-battery-staple",
    )
    .await;

    // Derive the project's URL slugs from its name. The slice-1
    // Background uses slug = lowercase, hyphenated.
    let project_slug = slugify(&project_name);
    // All slice-6 scenarios use the "Backend" team per the Background.
    let team_slug = "backend";

    let url = format!("{base}/team/{team_slug}/project/{project_slug}/events");
    let resp = http
        .get(&url)
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("open SSE subscription against subprocess");
    assert_eq!(
        resp.status(),
        reqwest::StatusCode::OK,
        "SSE subscription open expected 200, got {}; body: {}",
        resp.status(),
        resp.text().await.unwrap_or_default(),
    );

    // Hold the Response — its underlying TCP connection stays open
    // (Body::from_stream serves until the client disconnects). Drop
    // closes the TCP connection mid-stream which the server's
    // SubscriberGauge Drop observes via broken pipe.
    world.slice6_sse_subscription = Some(resp);
    // Brief settle so the server-side handler reaches the
    // SubscriberGauge::new line before the next assertion scrapes.
    tokio::time::sleep(Duration::from_millis(200)).await;
}

// ---------------------------------------------------------------------
// Whens
// ---------------------------------------------------------------------

#[when("the operator scrapes the metrics endpoint")]
async fn when_operator_scrapes_metrics_endpoint(world: &mut FoundryWorld) {
    let addr = current_metrics_addr(world);
    let (status, body) = scrape_metrics_raw(addr).await;
    let snapshot = ScrapeSnapshot {
        samples: crate::support::metrics_scrape::parse_exposition(&body),
        raw_body: body,
    };
    world.slice6_last_scrape_status = Some(status);
    world.slice6_last_scrape = Some(snapshot);
}

#[when("the operator scrapes the metrics endpoint immediately")]
async fn when_operator_scrapes_metrics_endpoint_immediately(world: &mut FoundryWorld) {
    // Identical to the regular scrape — the "immediately" word is the
    // user-facing intent ("before the first poll tick fires"), which
    // is naturally satisfied because:
    //   - The Given spawns the subprocess + waits for it to bind +
    //     return Ok before this When runs (so the recorder install +
    //     register-at-0 already happened in main.rs).
    //   - The first poll tick fires within METRICS_POOL_POLL_SECONDS
    //     (1s for tests). If it has fired by now the pool is empty
    //     anyway (the subprocess has no in-flight requests), so the
    //     gauge value remains 0.
    let addr = current_metrics_addr(world);
    let (status, body) = scrape_metrics_raw(addr).await;
    let snapshot = ScrapeSnapshot {
        samples: crate::support::metrics_scrape::parse_exposition(&body),
        raw_body: body,
    };
    world.slice6_last_scrape_status = Some(status);
    world.slice6_last_scrape = Some(snapshot);
}

#[when(regex = r#"^Mei posts a comment on "(\w+)-(\d+)" with body "([\s\S]*)"$"#)]
async fn when_mei_posts_comment(world: &mut FoundryWorld, prefix: String, n: i32, body: String) {
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

    // Look up the project slug from the schema — the Background
    // step seeds it.
    let (team_slug, project_slug) = lookup_team_and_project_slug(world, &prefix).await;
    let csrf_token = fetch_csrf_token(&http, &base, &session_cookie).await;
    let combined_cookie = format!("{session_cookie}; foundry_csrf={csrf_token}");

    let mut form = std::collections::HashMap::new();
    form.insert("body", body);
    form.insert("_csrf", csrf_token.clone());

    let url = format!("{base}/team/{team_slug}/project/{project_slug}/issues/{n}/comments");
    let resp = http
        .post(&url)
        .header(reqwest::header::COOKIE, combined_cookie)
        .form(&form)
        .send()
        .await
        .expect("post comment to subprocess");
    // We don't assert status here — slice-6 cares about the metric
    // emission, not the comment outcome (separate slices test that).
    // But we do drain the body so the connection releases cleanly.
    let _ = resp.text().await;
    let _ = prefix; // silence unused var
    world.slice6_request_count += 1;
}

#[when(
    regex = r#"^the operator issues (\d+) HTTP requests across the routes "([^"]+)" and "([^"]+)"$"#
)]
async fn when_operator_issues_requests_across_routes(
    world: &mut FoundryWorld,
    count: u64,
    route_a: String,
    route_b: String,
) {
    let base = format!("http://{}", current_main_addr(world));
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("build burst http client");
    // Alternate between the two routes; the breakdown assertion
    // expects both routes to be observed in the counter.
    for i in 0..count {
        let route = if i % 2 == 0 { &route_a } else { &route_b };
        let url = format!("{base}{route}");
        let _ = http.get(&url).send().await;
        let key = (route.clone(), "GET".to_string());
        *world.slice6_request_count_by_route.entry(key).or_insert(0) += 1;
        world.slice6_request_count += 1;
    }
    // Tiny settle to let the recorder flush counter increments
    // before the next scrape.
    tokio::time::sleep(Duration::from_millis(50)).await;
}

#[when(regex = r#"^the operator issues (\d+) HTTP requests to "([^"]+)"$"#)]
async fn when_operator_issues_requests_to(world: &mut FoundryWorld, count: u64, route: String) {
    let base = format!("http://{}", current_main_addr(world));
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .expect("build burst http client");
    for _ in 0..count {
        let url = format!("{base}{route}");
        let _ = http.get(&url).send().await;
        world.slice6_request_count += 1;
    }
    tokio::time::sleep(Duration::from_millis(50)).await;
}

#[when(regex = r"^Mei holds an open database connection for (\d+) seconds$")]
async fn when_mei_holds_open_db_connection_for(world: &mut FoundryWorld, seconds: u64) {
    // The subprocess owns its own sqlx pool. To make the subprocess's
    // `db_connections_in_use` gauge transition from 0 to >0 across at
    // least one poll tick (and to keep it non-zero for the scrape
    // that follows in the SAME scenario), we kick off a long-running
    // background task that hammers `/readyz` continuously. The task
    // runs until the SUBPROCESS is torn down at scenario teardown
    // (its handle is forgotten — fire-and-forget). Each /readyz hit
    // runs `Store::probe()` which does `SELECT 1` + the slice-5
    // column-existence query, holding a pool connection for ~10-20ms.
    // With 32 concurrent in-flight requests sustained for the
    // requested duration, the pool's in_use count is reliably >= 1
    // across the next scrape AND across any subsequent scrape in
    // this scenario (until the scenario teardown drops the subprocess).
    //
    // We can't use `/__test/slow` here — that endpoint requires the
    // `test-support` Cargo feature which only the acceptance crate
    // enables; the production `foundry` binary (cargo_bin output)
    // does NOT carry it.
    let base = format!("http://{}", current_main_addr(world));
    let url = format!("{base}/readyz");

    // Spawn N concurrent sustained-load tasks. Each loops in the
    // background hitting /readyz back-to-back. They run for the
    // remainder of the scenario; the subprocess teardown at scenario
    // end cuts them off (the requests return errors, which we ignore).
    let deadline = Instant::now() + Duration::from_secs(seconds + 30);
    for _ in 0..32 {
        let url_clone = url.clone();
        tokio::spawn(async move {
            let http = reqwest::Client::builder()
                .timeout(Duration::from_secs(2))
                .build()
                .expect("build hold http client");
            while Instant::now() < deadline {
                let _ = http.get(&url_clone).send().await;
            }
        });
    }

    // Wait for the requested hold window so the 1s poll task has
    // at least one tick to observe in_use > 0 BEFORE the next
    // scrape captures the gauge.
    tokio::time::sleep(Duration::from_secs(seconds)).await;
    // Extra settle so the most recent poll tick fired during the
    // burst is reflected in the next scrape.
    tokio::time::sleep(Duration::from_millis(500)).await;
}

#[when("Mei abruptly disconnects from the SSE stream")]
async fn when_mei_abruptly_disconnects_from_sse(world: &mut FoundryWorld) {
    // Dropping the captured Response closes the underlying TCP
    // connection. The subprocess's SSE handler's send loop notices
    // the broken pipe on its next heartbeat tick (~25s production
    // default; the test fixture runs at the production heartbeat),
    // at which point the streaming future drops and the
    // SubscriberGauge Drop fires.
    //
    // Force a faster signal by also waiting for the heartbeat
    // window — the subsequent "running for at least 1 seconds" Given
    // gives the server side time to observe the disconnect.
    world.slice6_sse_subscription = None;
    tokio::time::sleep(Duration::from_millis(200)).await;
}

// `@manual` scenario #10 — middleware overhead budget contract. The
// `@manual` filter in `tests/acceptance.rs` excludes this scenario
// from default runs; the body remains so an explicit
// `FOUNDRY_ACCEPTANCE_TAGS=all` invocation surfaces it loudly
// (matches slice-1 US-01 + slice-4 US-13 precedent).
//
// Per DD-12 the contract is enforced by the criterion microbench
// DELIVER ships at `crates/foundry-app/benches/middleware_overhead.rs`
// (sub-deliverable F per wave-decisions.md). The scenario is the
// contract anchor; the bench is the enforcement.
#[when("the operator runs the middleware overhead criterion microbench across the 27 routes")]
async fn when_operator_runs_middleware_overhead_microbench(_world: &mut FoundryWorld) {
    panic!(
        "Scenario is `@manual` — perf budget contract is enforced by the criterion microbench \
         at `crates/foundry-app/benches/middleware_overhead.rs`. \
         Run `cargo bench -p foundry-app --bench middleware_overhead` instead."
    );
}

// ---------------------------------------------------------------------
// Thens
// ---------------------------------------------------------------------

#[then("the scrape returns HTTP 200")]
async fn then_scrape_returns_200(world: &mut FoundryWorld) {
    let status = world
        .slice6_last_scrape_status
        .expect("scrape status captured");
    assert_eq!(
        status,
        reqwest::StatusCode::OK,
        "expected scrape HTTP 200, got {status}",
    );
}

#[then(regex = r#"^the scrape body contains the line "([^"]+)"$"#)]
async fn then_scrape_body_contains_line(world: &mut FoundryWorld, metric_name: String) {
    let snap = world.slice6_last_scrape.as_ref().expect("scrape captured");
    assert!(
        snap.contains_metric_line(&metric_name),
        "scrape body missing line for `{metric_name}`. Body:\n{}",
        snap.raw_body
    );
}

#[then(regex = r#"^the scrape body contains a sample for "([^"]+)" with labels "([^"]+)"$"#)]
async fn then_scrape_body_contains_sample_with_labels(
    world: &mut FoundryWorld,
    metric_name: String,
    labels_csv: String,
) {
    let snap = world.slice6_last_scrape.as_ref().expect("scrape captured");
    let expected = parse_labels_csv(&labels_csv);
    let matched = snap.samples_for(&metric_name).into_iter().any(|s| {
        expected
            .iter()
            .all(|(k, v)| s.labels.get(k).map(|sv| sv == v).unwrap_or(false))
    });
    assert!(
        matched,
        "no sample for `{metric_name}` matched labels `{labels_csv}`. \
         Available samples for `{metric_name}`:\n{:#?}",
        snap.samples_for(&metric_name),
    );
}

#[then(regex = r#"^the scrape body's "([^"]+)" sample sums to (\d+)$"#)]
async fn then_scrape_body_sample_sums_to(
    world: &mut FoundryWorld,
    metric_name: String,
    expected_sum: u64,
) {
    let snap = world.slice6_last_scrape.as_ref().expect("scrape captured");
    let actual = snap.sum_for(&metric_name);
    assert_eq!(
        actual as u64,
        expected_sum,
        "sum for `{metric_name}` was {actual}, expected {expected_sum}. \
         Samples:\n{:#?}",
        snap.samples_for(&metric_name),
    );
}

#[then(regex = r#"^the scrape body's "([^"]+)" sample has value (\d+)$"#)]
async fn then_scrape_body_sample_has_value(
    world: &mut FoundryWorld,
    metric_name: String,
    expected_value: u64,
) {
    let snap = world.slice6_last_scrape.as_ref().expect("scrape captured");
    let samples = snap.samples_for(&metric_name);
    assert!(
        !samples.is_empty(),
        "no samples for `{metric_name}` in body:\n{}",
        snap.raw_body
    );
    // For gauges the dashboard query is typically `sum(metric)`; we
    // match the sum-equals expected pattern. At slice-1 scale all
    // gauges have at most one series at a time for the assertion
    // shapes we use.
    let total = snap.sum_for(&metric_name);
    assert_eq!(
        total as u64, expected_value,
        "value for `{metric_name}` was {total}, expected {expected_value}. \
         Samples:\n{samples:#?}",
    );
}

#[then(regex = r#"^the scrape body's "([^"]+)" samples carry only the label keys "([^"]+)"$"#)]
async fn then_scrape_body_samples_carry_only_label_keys(
    world: &mut FoundryWorld,
    metric_name: String,
    permitted_keys_csv: String,
) {
    let snap = world.slice6_last_scrape.as_ref().expect("scrape captured");
    let observed = snap.label_keys_for(&metric_name);
    let permitted: BTreeSet<String> = permitted_keys_csv
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();
    assert_eq!(
        observed, permitted,
        "label keys for `{metric_name}` were {observed:?}, expected exactly {permitted:?}",
    );
}

#[then(
    regex = r#"^the scrape body's "([^"]+)" samples do NOT carry any of the label keys "([^"]+)"$"#
)]
async fn then_scrape_body_samples_do_not_carry_label_keys(
    world: &mut FoundryWorld,
    metric_name: String,
    forbidden_keys_csv: String,
) {
    let snap = world.slice6_last_scrape.as_ref().expect("scrape captured");
    let observed = snap.label_keys_for(&metric_name);
    let forbidden: BTreeSet<String> = forbidden_keys_csv
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();
    let intersection: BTreeSet<&String> = observed.intersection(&forbidden).collect();
    assert!(
        intersection.is_empty(),
        "forbidden label keys observed on `{metric_name}`: {intersection:?}. \
         Full observed key set: {observed:?}",
    );
}

#[then(regex = r#"^the scrape body's "([^"]+)" sample's "([^"]+)" label is "([^"]+)"$"#)]
async fn then_scrape_body_sample_label_is(
    world: &mut FoundryWorld,
    metric_name: String,
    label_key: String,
    expected_label_value: String,
) {
    let snap = world.slice6_last_scrape.as_ref().expect("scrape captured");
    let matched = snap.samples_for(&metric_name).into_iter().any(|s| {
        s.labels
            .get(&label_key)
            .map(|v| v == &expected_label_value)
            .unwrap_or(false)
    });
    assert!(
        matched,
        "no sample for `{metric_name}` carries `{label_key}=\"{expected_label_value}\"`. \
         Samples:\n{:#?}",
        snap.samples_for(&metric_name),
    );
}

#[then(
    regex = r#"^the scrape body's "([^"]+)" histogram has at least one bucket with count >= (\d+)$"#
)]
async fn then_scrape_body_histogram_bucket_count(
    world: &mut FoundryWorld,
    metric_name: String,
    min_count: u64,
) {
    let snap = world.slice6_last_scrape.as_ref().expect("scrape captured");
    let count = snap.histogram_observation_count(&metric_name);
    assert!(
        count >= min_count,
        "histogram `{metric_name}` observation count was {count}, expected >= {min_count}. \
         Body sample:\n{}",
        // First 1KB of the body for diagnostic context.
        &snap.raw_body[..snap.raw_body.len().min(1024)],
    );
}

#[then(regex = r#"^the scrape body's "([^"]+)" sample is greater than (\d+)$"#)]
async fn then_scrape_body_sample_is_greater_than(
    world: &mut FoundryWorld,
    metric_name: String,
    threshold: u64,
) {
    let snap = world.slice6_last_scrape.as_ref().expect("scrape captured");
    let total = snap.sum_for(&metric_name);
    assert!(
        total > threshold as f64,
        "sum for `{metric_name}` was {total}, expected > {threshold}. \
         Samples:\n{:#?}",
        snap.samples_for(&metric_name),
    );
}

#[then(regex = r#"^the scrape body's "([^"]+)" sample returns to (\d+)$"#)]
async fn then_scrape_body_sample_returns_to(
    world: &mut FoundryWorld,
    metric_name: String,
    baseline: u64,
) {
    let snap = world.slice6_last_scrape.as_ref().expect("scrape captured");
    let total = snap.sum_for(&metric_name);
    assert_eq!(
        total as u64,
        baseline,
        "sum for `{metric_name}` was {total}, expected baseline {baseline}. \
         Samples:\n{:#?}",
        snap.samples_for(&metric_name),
    );
}

#[then("the foundry subprocess is alive")]
async fn then_foundry_subprocess_is_alive(world: &mut FoundryWorld) {
    let foundry = world
        .slice6_foundry
        .as_mut()
        .expect("foundry subprocess running");
    // tokio::process::Child::try_wait returns Result<Option<ExitStatus>, io::Error>;
    // None == still running.
    match foundry.process.try_wait() {
        Ok(None) => {} // still running — good
        Ok(Some(status)) => panic!("foundry subprocess exited unexpectedly: {status}"),
        Err(err) => panic!("foundry subprocess try_wait failed: {err}"),
    }
}

#[then("the bench reports added P95 overhead below 10 microseconds")]
async fn then_bench_reports_p95_below_10us(_world: &mut FoundryWorld) {
    // `@manual` scenario; in default runs the `@manual` filter
    // excludes the parent Scenario so this body never executes. If it
    // does (explicit `FOUNDRY_ACCEPTANCE_TAGS=all`), this is the
    // documented hand-off to the criterion bench.
    panic!(
        "Scenario is `@manual` — the perf budget contract is enforced by the criterion \
         microbench at `crates/foundry-app/benches/middleware_overhead.rs`. \
         Run `cargo bench -p foundry-app --bench middleware_overhead` and inspect P95 there."
    );
}

// =====================================================================
// Subprocess HTTP helpers (sign-in + CSRF + slug lookup).
// =====================================================================

/// Sign in against the subprocess and return the `foundry_session=...`
/// cookie string. Same shape as slice-2/5 helpers but routed through
/// the SUBPROCESS, not the in-process harness.
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
    let csrf_full = csrf_get
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .map(|s| s.to_string())
        .expect("csrf cookie issued");
    let csrf_token = csrf_full
        .strip_prefix("foundry_csrf=")
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string();
    let csrf_pair = format!("foundry_csrf={csrf_token}");

    let mut form = std::collections::HashMap::new();
    form.insert("email", email.to_string());
    form.insert("password", password.to_string());
    form.insert("_csrf", csrf_token);
    let resp = http
        .post(format!("{base}/sign-in"))
        .header(reqwest::header::COOKIE, csrf_pair)
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
        .unwrap_or_else(|| {
            panic!(
                "no session cookie on sign-in response (status: {}, body: {:?})",
                resp.status(),
                // can't await here; body already consumed by header borrow
                "<body not captured>",
            )
        });
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
    let cookie = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .map(|s| s.to_string())
        .unwrap_or_default();
    cookie
        .strip_prefix("foundry_csrf=")
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string()
}

/// Look up (team_slug, project_slug) for a given issue key prefix
/// by querying the per-scenario PG schema directly.
async fn lookup_team_and_project_slug(world: &FoundryWorld, key_prefix: &str) -> (String, String) {
    // Reuse the slice-1 harness pool (the schema is shared with the
    // subprocess; both read/write the same rows).
    let pool = world
        .harness
        .as_ref()
        .expect("Background steps create harness")
        .app
        .state
        .store
        .pool();
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

/// Slugify the same way slice-1 does — lowercase + non-alphanumerics
/// to hyphen + collapse + trim hyphens. Mirrors
/// `support::compose_harness::slugify` semantics for the project-name
/// strings we use in scenarios.
fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_hyphen = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            for low in ch.to_lowercase() {
                out.push(low);
            }
            last_hyphen = false;
        } else if !last_hyphen {
            out.push('-');
            last_hyphen = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    while out.starts_with('-') {
        out.remove(0);
    }
    out
}

/// Parse `path=/healthz,method=GET,status=200` into a sorted Vec of
/// (k, v) pairs. Used by the "contains a sample with labels" Then
/// step. Tolerates `=` inside values that don't contain `,` (which
/// none of the slice-6 scenarios use).
fn parse_labels_csv(csv: &str) -> Vec<(String, String)> {
    csv.split(',')
        .filter_map(|pair| {
            let mut split = pair.splitn(2, '=');
            let k = split.next()?.trim().to_string();
            let v = split.next()?.trim().to_string();
            Some((k, v))
        })
        .collect()
}
