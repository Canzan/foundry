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
    build_router, mint_bootstrap_if_needed, AppState, NoopEmailSender, SystemClock,
    DEFAULT_SSE_HEARTBEAT_MS,
};
use foundry_store::Store;
use secrecy::SecretString;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    init_tracing();

    let host: String = std::env::var("FOUNDRY_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port: u16 = std::env::var("FOUNDRY_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
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
    };
    let router = build_router(state);

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
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .init();
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
