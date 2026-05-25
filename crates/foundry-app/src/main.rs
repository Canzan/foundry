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
    build_router, metrics_server, mint_bootstrap_if_needed, AppState, NoopEmailSender, SystemClock,
    DEFAULT_FILE_UPLOAD_MAX_MB, DEFAULT_SSE_HEARTBEAT_MS,
};
use foundry_store::Store;
use secrecy::SecretString;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

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
    store.migrate().await.context("run migrations")?;

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

    let state = AppState {
        store: Arc::new(store),
        session_secret: Arc::new(SecretString::new(session_secret.into())),
        session_cookie_secure,
        db_schema: std::env::var("FOUNDRY_DB_SCHEMA").unwrap_or_else(|_| "public".to_string()),
        public_url: public_url.clone(),
        clock: Arc::new(SystemClock),
        email: Arc::new(NoopEmailSender),
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
    let router = build_router(state);

    // Spawn the metrics sidecar listener before the main HTTP listener
    // binds — `probe.metrics.endpoint_reachable` (observability-infra.md)
    // wants the metrics port up by the time the app is ready.
    let metrics_addr = metrics_server::serve(&metrics_host, metrics_port, metrics_handle)
        .await
        .context("bind metrics listener")?;
    tracing::info!(%metrics_addr, "foundry metrics listening");
    metrics::counter!("foundry_app_startup_total").increment(1);

    let addr: SocketAddr = format!("{host}:{port}").parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!(%addr, "foundry listening");

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
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
                "" => {
                    eprintln!(
                        "foundry doctor: subcommand required. \
                         Available: backup-verify <file>"
                    );
                    Some(2)
                }
                other => {
                    eprintln!(
                        "foundry doctor: unknown subcommand {other:?}. \
                         Available: backup-verify <file>"
                    );
                    Some(2)
                }
            }
        }
        other => {
            eprintln!(
                "foundry: unknown subcommand {other:?}. \
                 Available: serve (default), doctor backup-verify <file>"
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
