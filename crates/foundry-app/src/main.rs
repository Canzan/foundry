//! foundry — slice 1 binary.
//!
//! Startup sequence (per architecture.md + auth.md):
//!   1. Load `.env` (dev convenience).
//!   2. Init structured logging.
//!   3. Connect to Postgres, run migrations under advisory lock.
//!   4. If no workspace exists, mint a bootstrap token and log it.
//!   5. Bind the router and serve until SIGTERM.

use anyhow::Context;
use foundry_app::{
    build_notifier, build_router, metrics_server, mint_bootstrap_if_needed, AppState, SystemClock,
    DEFAULT_FILE_UPLOAD_MAX_MB, DEFAULT_SSE_HEARTBEAT_MS,
};
use foundry_store::Store;
use secrecy::SecretString;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tracing_subscriber::EnvFilter;

/// Slice 6 ADR-012 — default cadence for the pool-stats poll task.
/// Operators may tune via `METRICS_POOL_POLL_SECONDS` (used in tests to
/// shorten the wait window for the connection-in-use acceptance scenario).
const DEFAULT_METRICS_POOL_POLL_SECONDS: u64 = 5;

/// Slice 8 (ADR-018) — gauge: count of unprocessed outbox rows. Folded
/// into the 5s pool-poll loop (D1 = A). Unlabelled (1 series).
const OUTBOX_PENDING_JOBS: &str = "outbox_pending_jobs";

/// Slice 8 (ADR-018) — gauge: count of active unclaimed admin bootstrap
/// tokens. Same poll loop. Unlabelled (1 series).
const BOOTSTRAP_TOKENS_UNCLAIMED: &str = "bootstrap_tokens_unclaimed";

/// Slice 8 (ADR-019 / D5) — counter: startup-probe failures, labelled
/// with the bounded code-defined `probe_name`. The Principle-9 recursive
/// self-monitoring metric.
const PROBE_FAILURES_TOTAL: &str = "probe_failures_total";

/// Slice 8 (ADR-019 / D5 / D6) — the closed, code-defined `probe_name`
/// set. Every probe in the startup sequence has its name here so it
/// register-at-0's (Grafana shows the full "all probes passing"
/// baseline) AND increments the counter on failure. Adding a probe MUST
/// add its name here. Bounded + code-defined; never request-derived
/// (the slice-6 ADR-011 cardinality invariant, extended by D6).
const PROBE_NAMES: &[&str] = &["store", "metrics", "machine_token"];

/// Slice 7 (ADR-015) — default cadence for the daily tombstone GC
/// sweep. 86400 seconds = 24 hours. Operators tune via
/// `FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS`; the acceptance suite
/// overrides to 1 to exercise the first-tick-soon invariant within a
/// per-scenario wall-clock budget.
const DEFAULT_TOMBSTONE_GC_INTERVAL_SECONDS: u64 = 86_400;

/// Slice 7 (ADR-015) — default retention threshold for tombstoned
/// comments. 90 days. Operators tune via
/// `FOUNDRY_TOMBSTONE_GC_OLDER_THAN_DAYS`; tests use the default
/// (the scenarios seed `deleted_at` at 91d / 89d on either side).
const DEFAULT_TOMBSTONE_GC_OLDER_THAN_DAYS: u64 = 90;

/// Slice 7 (ADR-015) — default per-invocation cap on the number of
/// tombstones the GC sweep will hard-delete in a single tick.
/// 10,000 rows. Insurance against misconfigured `deleted_at`
/// (the textbook "GC hit the wrong threshold" disaster).
const DEFAULT_TOMBSTONE_GC_MAX_PER_RUN: u64 = 10_000;

/// Slice 7 (ADR-015) — batch size for the inner DELETE loop. Fixed at
/// 1000; the cap controls the total per invocation, the batch controls
/// the per-round-trip work. Not env-tunable — operators with extreme
/// needs can raise the cap instead.
const TOMBSTONE_GC_BATCH_SIZE: u64 = 1_000;

/// Slice 7 (ADR-015) — production first-tick offset. At production
/// cadence (86400s) the GC task waits this long after process boot
/// before its first tick — gives the startup self-scrape probe a beat
/// to settle and provides operators a "GC is alive" signal within
/// ~30s of boot (instead of waiting the full 24h cadence). At test
/// cadence (1-2s), the first-tick offset equals the cadence — exactly
/// one tick per cadence interval after the first, with the first
/// tick aligned to the cadence boundary so the test wait windows
/// see deterministic tick counts.
const TOMBSTONE_GC_PROD_FIRST_TICK_OFFSET: Duration = Duration::from_secs(30);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Subcommand dispatch happens BEFORE we initialise tracing /
    // load `.env` / connect to Postgres — operator CLI subcommands
    // (`doctor backup-verify`) must be invocable on a host that does
    // not have DATABASE_URL or SESSION_SECRET set.
    //
    // The default invocation (no args, or `serve`) boots the HTTP
    // listener exactly as before. The only recognised subcommand is
    // `doctor backup-verify <file>`; unknown subcommands print a usage
    // hint and exit non-zero.
    if let Some(code) = dispatch_subcommand() {
        std::process::exit(code);
    }

    let _ = dotenvy::dotenv();
    init_tracing();

    let host: String = std::env::var("FOUNDRY_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port: u16 = std::env::var("FOUNDRY_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let metrics_port: u16 = std::env::var("METRICS_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(9090);
    let metrics_host: String = std::env::var("METRICS_HOST").unwrap_or_else(|_| "0.0.0.0".into());

    // Install the metrics recorder before anything else so any module
    // can emit `metrics::counter!` / `metrics::histogram!` from the
    // first line of work. The matching sidecar listener is spawned
    // a few lines down once we know we have a valid config (we don't
    // bind the metrics port until we've confirmed we'll actually
    // serve).
    let metrics_handle =
        metrics_server::install_recorder().context("install Prometheus recorder")?;
    let public_url: String =
        std::env::var("FOUNDRY_PUBLIC_URL").unwrap_or_else(|_| format!("http://localhost:{port}"));
    let database_url: String = std::env::var("DATABASE_URL").context("DATABASE_URL is required")?;

    let store = Store::connect(&database_url)
        .await
        .context("connect to Postgres")?;
    // Allow the acceptance harness to skip migrations when it has
    // already provisioned the per-scenario schema. Production paths
    // never set FOUNDRY_SKIP_MIGRATIONS; the slice-6 acceptance
    // suite sets it to avoid an advisory-lock pile-up between the
    // in-process harness's migrate (slice-1 pattern) and the per-
    // scenario subprocess's migrate (slice-6 pattern, both running
    // against the same Postgres container).
    let skip_migrations = std::env::var("FOUNDRY_SKIP_MIGRATIONS")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false);
    if !skip_migrations {
        store.migrate().await.context("run migrations")?;
    }

    if let Some(url) = mint_bootstrap_if_needed(&store, &public_url).await? {
        // Stdout — the acceptance suite greps `docker compose logs` for
        // this exact prefix. Do NOT change the prefix without updating
        // `foundry_app::bootstrap_log_line` and the US-01 step body.
        println!("{}", foundry_app::bootstrap_log_line(&url));
    } else {
        tracing::info!("workspace already claimed — no bootstrap token minted");
    }

    let session_secret = std::env::var("SESSION_SECRET").context("SESSION_SECRET is required")?;
    if session_secret.len() < 32 {
        anyhow::bail!("SESSION_SECRET must be at least 32 bytes");
    }
    let session_cookie_secure = std::env::var("SESSION_COOKIE_SECURE")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(true);

    // Feature A (US-W05b, ADR-W02) — build the Ed25519 machine-token
    // verifier from MACHINE_TOKEN_PUBLIC_KEYS, EXACTLY as session_secret
    // is built from SESSION_SECRET. Comma-separated, newest first; the
    // verifier holds the SET and tries each (overlapping-key rotation).
    // A malformed key fails here and refuses startup below.
    // PEM keys carry newlines; env transport (.env / compose / subprocess)
    // commonly encodes them as literal `\n`, so we normalize those back to
    // real newlines before parsing. Comma separates keys (PEM bodies are
    // base64 + dashes, never commas), so this split is unambiguous.
    let machine_token_public_keys: Vec<String> = std::env::var("MACHINE_TOKEN_PUBLIC_KEYS")
        .context("MACHINE_TOKEN_PUBLIC_KEYS is required")?
        .split(',')
        .map(|k| k.trim().replace("\\n", "\n"))
        .filter(|k| !k.is_empty())
        .collect();
    let machine_token_verifier =
        match foundry_auth::MachineTokenVerifier::from_public_keys(&machine_token_public_keys) {
            Ok(verifier) => verifier,
            Err(err) => {
                // Earned Trust — refuse startup on malformed key material,
                // mirroring the session_secret/store/metrics probes. The
                // metrics recorder is already installed, so increment the
                // probe-failure counter before propagating.
                tracing::error!(
                    event = "health.startup.refused",
                    probe = "machine_token",
                    reason = "machine_token_key",
                    detail = %err,
                    "machine-token public key material invalid — refusing to start"
                );
                metrics::counter!(PROBE_FAILURES_TOTAL, "probe_name" => "machine_token")
                    .increment(1);
                return Err(anyhow::Error::from(err).context("build machine-token verifier"));
            }
        };

    // Earned-Trust key probe + signer retention (ADR-MT01 / DD1, signer.md):
    // if a signing key is configured (issuing binary), parse it into a
    // SecretString, sign+verify a throwaway claim set to prove the keypair
    // round-trips in THIS environment, and RETAIN the parsed signer ONLY after
    // the probe passes (the type "we have a usable signer" is constructible
    // post-probe). A mismatched signing/public key refuses startup rather than
    // silently 401-ing every token in prod. The key value is wrapped in
    // SecretString immediately and is NEVER logged on any path (success,
    // failure, or absent).
    let machine_token_signer: Option<Arc<foundry_auth::MachineTokenSigner>> =
        match std::env::var("MACHINE_TOKEN_SIGNING_KEY") {
            Ok(raw) => {
                // SHIPPED \n-normalization, identical to the public-key path.
                let pem = SecretString::new(raw.replace("\\n", "\n").into());
                let probe = foundry_auth::MachineTokenSigner::from_pkcs8_pem(&pem)
                    .and_then(|signer| machine_token_verifier.self_test(&signer).map(|()| signer));
                match probe {
                    Ok(signer) => Some(Arc::new(signer)),
                    Err(err) => {
                        tracing::error!(
                            event = "health.startup.refused",
                            probe = "machine_token",
                            reason = "machine_token_key",
                            detail = %err,
                            "machine-token keypair self-test failed — refusing to start"
                        );
                        metrics::counter!(PROBE_FAILURES_TOTAL, "probe_name" => "machine_token")
                            .increment(1);
                        return Err(
                            anyhow::Error::from(err).context("machine-token keypair self-test")
                        );
                    }
                }
            }
            // Verifier-only binary — graceful (OD1/DD2). No key, no signer,
            // nothing logged about the absent key.
            Err(_) => None,
        };

    let realtime_tx = foundry_realtime::build_broadcast();
    // Spawn the dedicated LISTEN connection task. It owns its own
    // Postgres connection (NOT borrowed from the request pool); the
    // task survives transient Postgres errors with exponential
    // backoff and is aborted at process exit.
    let _listener_task =
        foundry_realtime::spawn_pg_listener(database_url.clone(), realtime_tx.clone());

    let sse_heartbeat_ms = std::env::var("SSE_HEARTBEAT_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DEFAULT_SSE_HEARTBEAT_MS);
    let file_upload_max_mb = std::env::var("FILE_UPLOAD_MAX_MB")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(DEFAULT_FILE_UPLOAD_MAX_MB);

    // recipient-notification-preferences (ADR-003) — Arc the store BEFORE the
    // notifier so the suppression policy can share it, then wire `StoreSuppression`
    // into the dispatcher (wire → use: the store is already probed at startup). With
    // an empty `notification_unsubscribes` table the point-read returns `Ok(false)`,
    // so delivery is byte-for-byte unchanged (NFR-7).
    let store = Arc::new(store);
    let notifier = Arc::new(
        // ADR-003: the composition root reads NOTIFICATION_DELIVERY_TIMEOUT_MS
        // (per-provider fan-out timeout) and wires it into the dispatcher.
        build_notifier(foundry_app::notify::delivery_timeout_from_env())
            .await
            .context("build notification providers from NOTIFICATION_PROVIDERS")?
            .with_suppression(Arc::new(foundry_app::StoreSuppression::new(store.clone()))),
    );

    let state = AppState {
        store: store.clone(),
        session_secret: Arc::new(SecretString::new(session_secret.into())),
        machine_token_verifier: Arc::new(machine_token_verifier),
        // machine-token-admin-ux (US-MT00/ADR-MT01/DD1): the signer parsed and
        // probed above is retained here ONLY after its self_test passed —
        // `Some(..)` makes this binary an issuer, `None` keeps it verifier-only
        // (graceful, OD1/DD2). The signing key value is never logged on any
        // path; the signer holds only the parsed EncodingKey (no Debug leak).
        machine_token_signer,
        session_cookie_secure,
        db_schema: std::env::var("FOUNDRY_DB_SCHEMA").unwrap_or_else(|_| "public".to_string()),
        public_url: public_url.clone(),
        clock: Arc::new(SystemClock),
        // notification-delivery-providers (ADR-002) + recipient-notification-
        // preferences (ADR-003): the config-selected provider set wired with the
        // StoreSuppression gate above.
        notifier,
        // US-TMA05 — production guardrail at the ratified defaults (C=20, R=1/sec).
        revoke_rate_limiter: Arc::new(foundry_app::rate_limit::RevokeRateLimiter::default()),
        realtime_tx,
        sse_heartbeat_ms,
        file_upload_max_mb,
        // US-02 test-only seam: only the binary built with the
        // `test-support` feature carries this field. The production
        // release build excludes it via `cfg(any(test, feature = ...))`.
        // The acceptance crate pulls foundry-app with `test-support` on
        // (see foundry-acceptance/Cargo.toml), so this code path is
        // exercised by every cargo build that includes the harness.
        #[cfg(any(test, feature = "test-support"))]
        db_unreachable: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        // US-B01 test-only render-injection seam (parallel to
        // `db_unreachable`). Only the `test-support` build carries it; the
        // production binary never forces a render failure.
        #[cfg(any(test, feature = "test-support"))]
        force_board_render_failure: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        // US-04 test-only seams: only the binary built with
        // `test-support` carries these. Production replicas use the
        // compile-time `migrate!` path (foundry_store::run_migrations)
        // already invoked from `Store::migrate`; the runtime variant
        // is purely a test affordance.
        #[cfg(any(test, feature = "test-support"))]
        test_migrations_dir: None,
        #[cfg(any(test, feature = "test-support"))]
        applied_migrations: std::sync::Arc::new(std::sync::Mutex::new(
            foundry_store::MigrationReport::default(),
        )),
        #[cfg(any(test, feature = "test-support"))]
        test_migration_delay_ms: 0,
    };

    // notification-delivery-providers (ADR-004) — register the per-provider
    // delivery counter at 0 over the bounded cross-product of the ACTIVE
    // providers × the `NotificationEvent` catalog × {delivered,failed}, BEFORE
    // any notification fires. The live emission lives in `Notifier::notify`
    // (notify.rs — `foundry_notification_deliveries_total{provider,event,outcome}`),
    // but a fresh instance has delivered nothing, so without this baseline the
    // family is ABSENT from the first `/metrics` scrape and the Grafana delivery
    // panel reads "no data" — the same deploy-time correctness failure the
    // slice-8 register-at-0 work removed for its metrics. Only ACTIVE providers
    // mint series (an unconfigured channel is never wired, so it has no series);
    // every label value is drawn from a closed enum, keeping the cardinality
    // bounded at {provider,event,outcome} (ADR-004/ADR-011). `describe_counter!`
    // alone emits only HELP/TYPE comments (no sample line a scraper counts as a
    // series), so the concrete `.absolute(0)` registration below is what makes
    // each series present at zero.
    metrics::describe_counter!(
        foundry_app::NOTIFICATION_DELIVERIES_METRIC,
        "Per-provider notification delivery decisions, labelled by the bounded \
         triple {provider,event,outcome}. Registered at 0 for every active-provider \
         series so the delivery family is present on the first scrape (ADR-004)."
    );
    for (provider, event, outcome) in
        foundry_app::notify::delivery_zero_series(&state.notifier.active_kinds())
    {
        metrics::counter!(
            foundry_app::NOTIFICATION_DELIVERIES_METRIC,
            "provider" => provider.as_str(),
            "event" => event.as_str(),
            "outcome" => outcome.as_str(),
        )
        .absolute(0);
    }

    // recipient-notification-preferences (ADR-005) — register the SIBLING
    // suppression counter at 0 over the FULL `NotificationEvent` catalog, BEFORE
    // any notification fires. The live increment lives in `Notifier::notify`
    // (notify.rs — `foundry_notification_suppressions_total{event}`, fired only on
    // the suppression early-return), but a fresh instance has suppressed nothing,
    // so without this baseline the family is ABSENT from the first `/metrics`
    // scrape. Registering EVERY event (not just the suppressible ones) makes the
    // mandatory events' series a permanent, scrapeable `…{event="password_reset"}
    // 0` — the never-suppressed invariant (US-07 / NFR-3) is observable. The only
    // label is the bounded `event` (∈ NotificationEvent::ALL, snake_case): no
    // `provider`, no `workspace`, no recipient email/token — PII-free by
    // construction (ADR-005 cardinality discipline).
    metrics::describe_counter!(
        foundry_app::NOTIFICATION_SUPPRESSIONS_METRIC,
        "Per-event suppressed-notification decisions, labelled by the bounded \
         `event` only (no provider/workspace/recipient — PII-free). Registered at \
         0 for every event so mandatory events show a permanent 0 (ADR-005)."
    );
    for event in foundry_app::notify::suppressions_zero_series() {
        metrics::counter!(
            foundry_app::NOTIFICATION_SUPPRESSIONS_METRIC,
            "event" => event.as_str(),
        )
        .absolute(0);
    }

    // Slice 6 (ADR-012, D4 = A) — register `db_connections_in_use` at
    // value 0 BEFORE the poll task spawns. Grafana sees the metric line
    // immediately; the first poll tick (within METRICS_POOL_POLL_SECONDS)
    // overwrites with live pool state. Without this, the dashboard panel
    // would show "no data" for the first ~5s of every replica boot.
    metrics::gauge!("db_connections_in_use").set(0.0);

    // Slice 8 (ADR-018 / D3, ADR-019) — register the new gauges +
    // counters at 0 BEFORE their emitters run, same precedent as
    // slice-6's `db_connections_in_use` and slice-7's GC metrics. The
    // two DB-state gauges are refreshed by the existing 5s pool-poll
    // loop below (D1 = A — piggyback, no new task); the disconnect
    // counter is incremented in `foundry-realtime::run_pg_listener`; the
    // probe-failure counter is incremented by the wrapped startup probes.
    // All UNLABELLED except `probe_failures_total`, which carries the
    // bounded code-defined `probe_name` set {store, metrics} (D5 / D6).
    // The migration histogram has NO register-at-0 (ADR-020 — histograms
    // have no current value; its panel stays empty until the first
    // apply). Both probe_name series register at 0 so Grafana shows the
    // full probe set as flat-zero "all probes passing" lines.
    metrics::gauge!(OUTBOX_PENDING_JOBS).set(0.0);
    metrics::gauge!(BOOTSTRAP_TOKENS_UNCLAIMED).set(0.0);
    metrics::counter!(foundry_realtime::REALTIME_LISTEN_DISCONNECTS_TOTAL).absolute(0);
    for probe_name in PROBE_NAMES {
        metrics::counter!(PROBE_FAILURES_TOTAL, "probe_name" => *probe_name).absolute(0);
    }

    // token-mutations-metric-export — register the per-principal
    // revoke-storm guardrail counter at 0 BEFORE the first revoke. The
    // live emission lives in `RateLimiter::check`
    // (rate_limit.rs — `foundry_token_mutations_total{principal,outcome}`),
    // but a fresh instance has had no revoke, so without this baseline the
    // metric family is ABSENT from the first `/metrics` scrape and the
    // Grafana "token mutations" panel shows "no-data" — the same
    // deploy-time correctness failure the slice-8 register-at-0 work
    // (ADR-018 / D4) removed for its five metrics, deferred for this one.
    //
    // `describe_counter!` alone only emits HELP/TYPE comment lines (no
    // sample line), which a Prometheus scraper treats as "no series yet"
    // — so we ALSO register a concrete zero series. The `principal` label
    // is per-UUID at the live call-site, which makes a concrete zero
    // series awkward; we use a sentinel `system` principal so the family
    // appears with a real sample at zero. Both bounded `outcome` arms
    // (`ok`/`throttled`) register so the panel shows the full
    // mutation-outcome dimension as a flat-zero baseline (mirrors the
    // slice-8 register-at-0 over the bounded `probe_name` set). The
    // SHIPPED `{principal,outcome}` contract is unchanged — the live
    // emission still keys on the real bound `user_id`.
    //
    // CARDINALITY TRADEOFF (rate-guardrail.md §Metric / OD-TMA-1b): the
    // `principal` label is per-UUID (unbounded) — intentional for
    // per-principal abuse attribution, bounded in practice by the count
    // of ACTIVE principals plus the shipped per-principal bucket eviction
    // (ADR-005 idle + LRU). A bounded-aggregate variant (drop `principal`,
    // keep `outcome`) is a DEFERRED follow-up IF dashboard cardinality
    // becomes a concern; the contract is not broken now.
    metrics::describe_counter!(
        foundry_app::rate_limit::TOKEN_MUTATIONS_METRIC,
        "Per-principal management-mutation (revoke) decisions, labelled by \
         accountable principal user_id and outcome (ok|throttled). The \
         per-principal revoke-storm guardrail signal (NFR-TMA-SEC-07)."
    );
    const TOKEN_MUTATION_SENTINEL_PRINCIPAL: &str = "system";
    for outcome in ["ok", "throttled"] {
        metrics::counter!(
            foundry_app::rate_limit::TOKEN_MUTATIONS_METRIC,
            "principal" => TOKEN_MUTATION_SENTINEL_PRINCIPAL,
            "outcome" => outcome,
        )
        .absolute(0);
    }

    // Slice 6 (ADR-012) — background pool-stats poll task. Reads
    // `Store::pool_stats()` every METRICS_POOL_POLL_SECONDS and updates
    // the `db_connections_in_use` gauge. Aborts on graceful shutdown
    // (D5 = A: tokio drops the task when `axum::serve` returns; no
    // special wiring needed). Tests may shorten the cadence via the
    // env var to keep the connection-hold scenario fast.
    let pool_poll_seconds = std::env::var("METRICS_POOL_POLL_SECONDS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(DEFAULT_METRICS_POOL_POLL_SECONDS);
    let store_for_poll = state.store.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(pool_poll_seconds));
        // First tick fires immediately; subsequent ticks at the cadence.
        // The immediate first tick refreshes the registered-at-0 gauge
        // with the actual pool snapshot as soon as the runtime is awake.
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            let stats = store_for_poll.pool_stats();
            metrics::gauge!("db_connections_in_use").set(stats.in_use as f64);

            // Slice 8 (ADR-018 / D1 = A) — fold the two DB-state gauges
            // into the SAME tick. Two index-served `count(*)` reads —
            // negligible at the 5s cadence. Failure semantics match the
            // slice-7 pending-gauge pattern: on a query error the gauge
            // is simply not updated this tick (stale value ages flat;
            // operators alert on flatness, not on a missing series).
            match store_for_poll.count_pending_outbox().await {
                Ok(pending) => metrics::gauge!(OUTBOX_PENDING_JOBS).set(pending as f64),
                Err(err) => tracing::warn!(
                    error = %err,
                    "outbox_pending_jobs poll query failed; gauge stale this tick"
                ),
            }
            let now = time::OffsetDateTime::now_utc();
            match store_for_poll.count_unclaimed_bootstrap_tokens(now).await {
                Ok(unclaimed) => metrics::gauge!(BOOTSTRAP_TOKENS_UNCLAIMED).set(unclaimed as f64),
                Err(err) => tracing::warn!(
                    error = %err,
                    "bootstrap_tokens_unclaimed poll query failed; gauge stale this tick"
                ),
            }
        }
    });

    // Slice 7 (ADR-016 / D4 = A) — register the two new GC metrics at
    // value 0 BEFORE the GC task spawns. Same precedent as slice-6's
    // `db_connections_in_use` register-at-0: Grafana sees the metric
    // lines immediately; the first GC tick (within ~30s of boot or
    // FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS, whichever is shorter)
    // overwrites with live state. Without this, dashboards would show
    // "no data" for the first cadence window. Both metrics UNLABELLED
    // — bounded at exactly 1 series each (slice-6 D2 cardinality
    // invariant; slice-6 unit test in metrics_server.rs covers them).
    metrics::counter!("comments_tombstones_purged_total").absolute(0);
    metrics::gauge!("comments_tombstones_pending").set(0.0);

    // Slice 7 (ADR-015) — background tombstone GC task. Runs every
    // FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS (default 86400 = daily)
    // with a ~30s offset before the first tick. Advisory lock
    // (TOMBSTONE_GC_LOCK_ID) ensures only one replica actually
    // deletes; sibling replicas exit gracefully with Ok(0). Per
    // ADR-015 / D7 = A, errors are logged and the task continues —
    // the daily cadence IS the backoff.
    let gc_interval_seconds = std::env::var("FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(DEFAULT_TOMBSTONE_GC_INTERVAL_SECONDS);
    let gc_older_than_days = std::env::var("FOUNDRY_TOMBSTONE_GC_OLDER_THAN_DAYS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(DEFAULT_TOMBSTONE_GC_OLDER_THAN_DAYS);
    let gc_max_per_run = std::env::var("FOUNDRY_TOMBSTONE_GC_MAX_PER_RUN")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|n| *n > 0)
        .unwrap_or(DEFAULT_TOMBSTONE_GC_MAX_PER_RUN);
    let gc_older_than = Duration::from_secs(gc_older_than_days * 86_400);
    let store_for_gc = state.store.clone();
    // First-tick-soon offset:
    //   - Production cadence (86400s = 24h): offset is bounded by
    //     TOMBSTONE_GC_PROD_FIRST_TICK_OFFSET (30s) so operators see
    //     "the GC is alive" within ~30s of boot rather than waiting
    //     the full 24h.
    //   - Test cadence (1-2s): offset equals the cadence. The first
    //     tick fires at +cadence, then ticks every +cadence after
    //     that. The acceptance scenarios assume this aligned-cadence
    //     timing to make the per-scenario "wait N seconds" → "expect
    //     exactly M ticks fired" assertions deterministic.
    let cadence = Duration::from_secs(gc_interval_seconds);
    let first_tick_offset = if cadence < TOMBSTONE_GC_PROD_FIRST_TICK_OFFSET {
        // Test mode (short cadence) — offset == cadence so the first
        // tick is aligned to the cadence boundary, NOT to subprocess
        // boot. With cadence=2s + wait=2s after spawn+seed (~0.1s
        // overhead), each scrape lands at subprocess_t ≈ N*cadence +
        // 0.1s — just past the Nth tick boundary, comfortably before
        // the (N+1)th. The acceptance scenarios assume this aligned-
        // cadence timing.
        cadence
    } else {
        // Production cadence — bounded offset; ~30s before the first
        // tick, then full cadence thereafter.
        TOMBSTONE_GC_PROD_FIRST_TICK_OFFSET
    };
    tokio::spawn(async move {
        // Use interval_at so the FIRST tick fires at +offset (not
        // immediately). Without this, tokio::time::interval's
        // default first-tick-immediate behavior would race the
        // startup probe AND make scenario #6's "wait N seconds,
        // expect exactly M ticks to have fired" assertion
        // non-deterministic.
        let start = tokio::time::Instant::now() + first_tick_offset;
        let mut interval = tokio::time::interval_at(start, cadence);
        // Skip — under a slow tick we don't want to fire the next
        // tick "immediately" the moment the slow one completes.
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            // Slice 7 — the DISTILL D5 clarification originally proposed
            // an env-var test hook here (FOUNDRY_TEST_HOOK_GC_FAIL_NEXT).
            // DELIVER's chosen mechanism is different: the acceptance
            // test scenario for failure-survives takes the
            // TOMBSTONE_GC_LOCK_ID advisory lock from a separate test
            // pool, causing the next tick to observe contention and
            // return Ok(0) — observable as "no rows deleted" without
            // synthesising a fake error. The task survives identically
            // either way (per ADR-015 / D7 = A: log + continue), so the
            // observable contract is the same. NO test-only seam in
            // production code.
            match store_for_gc
                .gc_tombstoned_comments(
                    gc_older_than,
                    TOMBSTONE_GC_BATCH_SIZE as usize,
                    gc_max_per_run as usize,
                )
                .await
            {
                Ok(deleted) => {
                    if deleted > 0 {
                        tracing::info!(deleted_count = deleted, "tombstone GC tick completed");
                    }
                    metrics::counter!("comments_tombstones_purged_total").increment(deleted);
                }
                Err(err) => {
                    // Per ADR-015 / D7 = A — log + continue. The
                    // task survives transient errors; next tick fires
                    // at normal cadence; the daily cadence IS the
                    // backoff. No retry-with-backoff state.
                    tracing::warn!(
                        error = %err,
                        "tombstone GC tick failed; will retry next interval"
                    );
                }
            }
            // Always refresh the pending gauge after a tick, even on
            // GC error — operators want the pending count to reflect
            // current state regardless of whether the latest sweep
            // succeeded. count_pending_tombstones is a pure read with
            // no lock; safe to call after a lock-contention no-op.
            match store_for_gc.count_pending_tombstones(gc_older_than).await {
                Ok(pending) => {
                    metrics::gauge!("comments_tombstones_pending").set(pending as f64);
                }
                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        "tombstone GC pending-count query failed; gauge stale"
                    );
                }
            }
        }
    });

    // Slice 8 (ADR-019 / D5) — hold a Store handle for the `store`
    // startup probe before `state` moves into the router.
    let store_for_probe = state.store.clone();

    let router = build_router(state);

    // Spawn the metrics sidecar listener before the main HTTP listener
    // binds — `probe.metrics.endpoint_reachable` (observability-infra.md)
    // wants the metrics port up by the time the app is ready.
    //
    // Slice 8 (ADR-019 / D5): binding the metrics port is itself part of
    // the `metrics` self-check. If the port is already held (the
    // slice-6 ADR-014 "METRICS_PORT pre-bound" failure mode), the bind
    // fails BEFORE the self-scrape probe can run — so treat a serve-bind
    // failure as a `metrics` probe failure: emit the `health.startup.refused`
    // line + increment `probe_failures_total{probe_name="metrics"}`, then
    // refuse to start (ADR-014 posture). Without this the operator would
    // see a bare `bind` error with no probe-failure signal on the
    // dashboard or in the structured refuse-to-start log.
    let metrics_addr =
        match metrics_server::serve(&metrics_host, metrics_port, metrics_handle).await {
            Ok(addr) => addr,
            Err(err) => {
                tracing::error!(
                    event = "health.startup.refused",
                    probe = "metrics",
                    reason = "metrics_listener_bind_failed",
                    error = %err,
                    "metrics sidecar listener failed to bind — refusing to start"
                );
                metrics::counter!(PROBE_FAILURES_TOTAL, "probe_name" => "metrics").increment(1);
                return Err(err.context("bind metrics listener"));
            }
        };
    tracing::info!(%metrics_addr, "foundry metrics listening");
    metrics::counter!("foundry_app_startup_total").increment(1);

    // Slice 8 (ADR-019 / D5) — the `store` startup probe. Validates
    // Postgres reachability + the slice-5 migration-0006 columns
    // (Earned Trust). Wrapped so a failure increments
    // `probe_failures_total{probe_name="store"}` BEFORE the error
    // propagates and the process refuses to start (ADR-014 posture).
    record_probe_result(
        "store",
        store_for_probe
            .probe()
            .await
            .map(|_report| ())
            .map_err(anyhow::Error::from),
        "startup store probe failed",
    )?;

    // Slice 6 (ADR-014) — self-scrape `/metrics` startup probe.
    // Refuses to start if the sidecar listener is unreachable, returns
    // a non-200, returns an empty body, or omits the
    // `foundry_app_startup_total` line. On failure the process exits
    // non-zero — container orchestrator restarts the pod; the restart
    // loop surfaces the misconfig loudly instead of silently serving
    // traffic with broken metrics.
    //
    // Slice 8 (ADR-019 / D5) — wrapped so a failure increments
    // `probe_failures_total{probe_name="metrics"}` before propagating.
    record_probe_result(
        "metrics",
        metrics_server::probe(metrics_addr).await,
        "startup metrics probe failed",
    )?;

    let addr: SocketAddr = format!("{host}:{port}").parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    // Slice 6: log the BOUND addr (with the actually-allocated port
    // when FOUNDRY_PORT=0 is requested) so the acceptance subprocess
    // helper can parse it. Production deployments with non-zero
    // FOUNDRY_PORT see the same value either way.
    let bound = listener.local_addr().unwrap_or(addr);
    tracing::info!(addr = %bound, "foundry listening");

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

/// Slice 8 (ADR-019 / D5) — wrap a startup probe's result so a failure
/// increments `probe_failures_total{probe_name}` BEFORE the error
/// propagates (the process still refuses to start — ADR-014 posture
/// preserved). On success, the register-at-0 baseline is left untouched
/// (the counter stays flat at 0, the "probe passing" signal). `context`
/// is attached so the refuse-to-start error carries the probe name.
fn record_probe_result(
    probe_name: &str,
    result: anyhow::Result<()>,
    context: &'static str,
) -> anyhow::Result<()> {
    if result.is_err() {
        metrics::counter!(PROBE_FAILURES_TOTAL, "probe_name" => probe_name.to_string())
            .increment(1);
    }
    result.context(context)
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    // NFR-OBS-01: structured JSON to stdout in production. Operators
    // running `cargo run` locally can flip `RUST_LOG_FORMAT=pretty`
    // for human-readable output.
    let format = std::env::var("RUST_LOG_FORMAT").unwrap_or_else(|_| "json".into());
    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true);
    if format == "pretty" {
        builder.init();
    } else {
        builder.json().init();
    }
}

/// Inspect `std::env::args()` for a recognised subcommand. Returns
/// `Some(exit_code)` when a subcommand handled the invocation and the
/// process should exit; returns `None` when the binary should fall
/// through to the default HTTP-server boot path.
fn dispatch_subcommand() -> Option<i32> {
    let args: Vec<String> = std::env::args().collect();
    // args[0] is the binary path; user args start at args[1].
    let first = args.get(1).map(|s| s.as_str()).unwrap_or("");
    match first {
        // Default boot path — explicit `serve` is the same as no
        // subcommand so docker-compose CMDs can name the action.
        "" | "serve" => None,
        "doctor" => {
            let action = args.get(2).map(|s| s.as_str()).unwrap_or("");
            match action {
                "backup-verify" => {
                    let Some(file) = args.get(3) else {
                        eprintln!(
                            "foundry doctor backup-verify: missing <file> argument. \
                             Usage: foundry doctor backup-verify <backup-file>"
                        );
                        return Some(2);
                    };
                    let code =
                        foundry_app::admin_cli::run_backup_verify(std::path::Path::new(file));
                    Some(code)
                }
                // Slice 7 (ADR-016 / D5 = C) — restore a soft-deleted
                // comment by clearing `deleted_at` + `deleted_by`. Reads
                // DATABASE_URL to reach the LIVE production DB (unlike
                // backup-verify which uses FOUNDRY_DOCTOR_PROBE_URL).
                "restore-comment" => {
                    let Some(uuid) = args.get(3) else {
                        eprintln!(
                            "foundry doctor restore-comment: missing <comment-uuid> argument. \
                             Usage: foundry doctor restore-comment <comment-uuid>"
                        );
                        return Some(2);
                    };
                    let code = foundry_app::admin_cli::run_restore_comment(uuid);
                    Some(code)
                }
                // multi-workspace-provisioning (US-MWT07, ADR-002 / D2) — the
                // CLI-FIRST provisioning surface. Reads DATABASE_URL +
                // SESSION_SECRET to reach the LIVE DB and sign the invite link.
                "provision-workspace" => {
                    let opt = |flag: &str| -> Option<String> {
                        args.iter()
                            .position(|a| a == flag)
                            .and_then(|i| args.get(i + 1))
                            .cloned()
                    };
                    let name = opt("--name").unwrap_or_default();
                    let admin_email = opt("--admin-email").unwrap_or_default();
                    let acting_email = opt("--as").unwrap_or_default();
                    if name.is_empty() || admin_email.is_empty() {
                        eprintln!(
                            "foundry doctor provision-workspace: missing required flags. \
                             Usage: foundry doctor provision-workspace --name <name> \
                             --admin-email <addr> --as <super-admin-email>"
                        );
                        return Some(2);
                    }
                    let code = foundry_app::admin_cli::run_provision_workspace(
                        &name,
                        &admin_email,
                        &acting_email,
                    );
                    Some(code)
                }
                // multi-workspace-provisioning (US-MWT07, ADR-001 / D1) — the
                // UPGRADE path. Records an existing user as the first instance
                // super-admin via the idempotent grant. Reads DATABASE_URL to
                // reach the LIVE DB. Reachable ONLY here, never the bearer API.
                "grant-super-admin" => {
                    let opt = |flag: &str| -> Option<String> {
                        args.iter()
                            .position(|a| a == flag)
                            .and_then(|i| args.get(i + 1))
                            .cloned()
                    };
                    let email = opt("--email").unwrap_or_default();
                    if email.is_empty() {
                        eprintln!(
                            "foundry doctor grant-super-admin: missing required flag. \
                             Usage: foundry doctor grant-super-admin --email <operator-email>"
                        );
                        return Some(2);
                    }
                    let code = foundry_app::admin_cli::run_grant_super_admin(&email);
                    Some(code)
                }
                // per-workspace-backup (US-PWB-01, ADR-002/003) — export ONE
                // workspace's data across the ten TENANT_TABLES to a single
                // verifiable tar archive. Reads DATABASE_URL to reach the LIVE DB.
                // Selector resolves by id OR case-insensitive name (DRIFT-1).
                // per-workspace-backup (US-PWB-01, AC-01.1, DRIFT-1) — print every
                // workspace's identity (id + name; no slug column) so the operator
                // can pick a target for export-workspace. Reads DATABASE_URL.
                "list-workspaces" => {
                    let code = foundry_app::admin_cli::run_list_workspaces();
                    Some(code)
                }
                "export-workspace" => {
                    let Some(selector) = args.get(3) else {
                        eprintln!(
                            "foundry doctor export-workspace: missing <id|name> and <out-path>. \
                             Usage: foundry doctor export-workspace <id|name> <out-path>"
                        );
                        return Some(2);
                    };
                    let Some(out_path) = args.get(4) else {
                        eprintln!(
                            "foundry doctor export-workspace: missing <out-path>. \
                             Usage: foundry doctor export-workspace <id|name> <out-path>"
                        );
                        return Some(2);
                    };
                    let code = foundry_app::admin_cli::run_export_workspace(selector, out_path);
                    Some(code)
                }
                "verify-export" => {
                    let Some(path) = args.get(3) else {
                        eprintln!(
                            "foundry doctor verify-export: missing <path>. \
                             Usage: foundry doctor verify-export <archive-path>"
                        );
                        return Some(4);
                    };
                    let code = foundry_app::admin_cli::run_verify_export(path);
                    Some(code)
                }
                "" => {
                    eprintln!(
                        "foundry doctor: subcommand required. \
                         Available: backup-verify <file>, restore-comment <comment-uuid>, \
                         provision-workspace --name <name> --admin-email <addr> --as <addr>, \
                         grant-super-admin --email <addr>, \
                         list-workspaces, \
                         export-workspace <id|name> <out-path>, \
                         verify-export <archive-path>"
                    );
                    Some(2)
                }
                other => {
                    eprintln!(
                        "foundry doctor: unknown subcommand {other:?}. \
                         Available: backup-verify <file>, restore-comment <comment-uuid>, \
                         provision-workspace --name <name> --admin-email <addr> --as <addr>, \
                         grant-super-admin --email <addr>, \
                         list-workspaces, \
                         export-workspace <id|name> <out-path>, \
                         verify-export <archive-path>"
                    );
                    Some(2)
                }
            }
        }
        other => {
            eprintln!(
                "foundry: unknown subcommand {other:?}. \
                 Available: serve (default), doctor backup-verify <file>, \
                 doctor restore-comment <comment-uuid>"
            );
            Some(2)
        }
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install ctrl_c handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received");
}
