//! foundry-app library — composition root pieces that need to be
//! reused by binary + acceptance harness.
//!
//! Slice 1 surface:
//! - [`AppState`] — shared state passed to handlers.
//! - [`build_router`] — pure router construction (testable).
//! - [`mint_bootstrap_if_needed`] — the startup hook that drives US-01.
//! - `/bootstrap` GET/POST — admin claim flow (US-05 scenarios 1-3).
//! - `/invites` POST — generate a shareable invite link (US-05 scenario 4).
//! - `/workspaces` POST — second-workspace guard (US-05 scenario 5).
//! - `/dashboard` GET — minimal post-claim landing (US-05 scenario 1 redirect target).

#![forbid(unsafe_code)]
#![deny(clippy::all)]

pub mod bootstrap;
pub mod clock;
pub mod comments;
pub mod csrf;
pub mod email;
pub mod events;
pub mod issues;
pub mod keyboard;
pub mod projects;
pub mod session;
pub mod signin;

use axum::extract::State;
use axum::http::StatusCode;
use axum::middleware;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use foundry_realtime::EventPayload;
use foundry_store::Store;
use secrecy::{ExposeSecret, SecretString};
use std::sync::Arc;
use tokio::sync::broadcast;

/// Default SSE heartbeat interval (`:keepalive\n\n` comment lines)
/// so load balancers do not idle-kill a long-lived stream. Production
/// default is 25s per realtime-roadmap.md; the acceptance harness
/// overrides via `SSE_HEARTBEAT_MS_OVERRIDE` for the heartbeat
/// scenario in US-09.
pub const DEFAULT_SSE_HEARTBEAT_MS: u64 = 25_000;

pub use clock::{Clock, SystemClock};
pub use email::{EmailSender, NoopEmailSender, SentEmail};

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<Store>,
    pub session_secret: Arc<SecretString>,
    pub session_cookie_secure: bool,
    /// Postgres schema where the `session` table lives. `"public"` in
    /// production, a per-scenario name like `"test_s17_ab12"` in the
    /// acceptance harness.
    pub db_schema: String,
    pub public_url: String,
    pub clock: Arc<dyn Clock>,
    pub email: Arc<dyn EmailSender>,
    /// Broadcast channel for realtime events. Cloning the sender is
    /// cheap (Arc inside); each SSE connection subscribes for a fresh
    /// Receiver. The pg-listener task is the sole publisher.
    pub realtime_tx: broadcast::Sender<EventPayload>,
    /// SSE keepalive interval in ms. Defaulted from
    /// `DEFAULT_SSE_HEARTBEAT_MS`; tests override via the harness
    /// helper `support::heartbeat_env::override_heartbeat_ms`.
    pub sse_heartbeat_ms: u64,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("session_cookie_secure", &self.session_cookie_secure)
            .field("public_url", &self.public_url)
            .finish_non_exhaustive()
    }
}

/// Build the axum router for slice 1.
pub fn build_router(state: AppState) -> Router {
    let session_layer = session::build_session_layer(
        state.store.pool().clone(),
        &state.db_schema,
        state.session_cookie_secure,
    );
    Router::new()
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz))
        .route("/dashboard", get(bootstrap::dashboard))
        .route(
            "/bootstrap",
            get(bootstrap::show_form).post(bootstrap::submit),
        )
        .route("/invites", post(bootstrap::create_invite))
        .route("/workspaces", post(bootstrap::create_workspace))
        .route(
            "/sign-in",
            get(signin::show_form).post(signin::submit_signin),
        )
        .route("/sign-out", post(signin::submit_signout))
        .route(
            "/forgot-password",
            get(signin::show_forgot_form).post(signin::submit_forgot),
        )
        .route(
            "/team/{team_slug}/projects/new",
            get(projects::show_create_form),
        )
        .route("/team/{team_slug}/projects", post(projects::submit_create))
        .route(
            "/team/{team_slug}/project/{project_slug}",
            get(projects::show_board),
        )
        .route(
            "/team/{team_slug}/project/{project_slug}/issues",
            post(issues::submit_create),
        )
        .route(
            "/team/{team_slug}/project/{project_slug}/issues/new",
            get(keyboard::show_new_issue_modal),
        )
        .route(
            "/team/{team_slug}/project/{project_slug}/search",
            get(keyboard::search_issues),
        )
        .route(
            "/team/{team_slug}/project/{project_slug}/issues/{issue_number}/state",
            post(issues::submit_state_change),
        )
        .route(
            "/team/{team_slug}/project/{project_slug}/events",
            get(events::sse_stream),
        )
        .route(
            "/team/{team_slug}/project/{project_slug}/issue/{issue_number}",
            get(comments::show_issue),
        )
        .route(
            "/team/{team_slug}/project/{project_slug}/issue/{issue_number}/comments",
            post(comments::submit_comment),
        )
        .route("/keyboard-help", get(keyboard::show_keyboard_help))
        .route("/", get(signin::dashboard_root))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            csrf::csrf_middleware,
        ))
        .layer(session_layer)
        .with_state(state)
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn readyz(State(state): State<AppState>) -> impl IntoResponse {
    match state.store.probe().await {
        Ok(_) => (StatusCode::OK, r#"{"status":"ready"}"#).into_response(),
        Err(err) => (
            StatusCode::SERVICE_UNAVAILABLE,
            format!(r#"{{"status":"not_ready","reason":"db_unreachable","detail":"{err}"}}"#),
        )
            .into_response(),
    }
}

/// If no workspace exists yet, mint a bootstrap token, persist its
/// hash, and return the URL the operator should visit. Returns `None`
/// when the instance is already claimed.
pub async fn mint_bootstrap_if_needed(
    store: &Store,
    public_url: &str,
) -> anyhow::Result<Option<String>> {
    if store.any_workspace_exists().await? {
        return Ok(None);
    }
    let token = foundry_auth::BootstrapToken::generate();
    let expires_at = time::OffsetDateTime::now_utc() + time::Duration::minutes(30);
    store
        .insert_bootstrap_token(uuid::Uuid::now_v7(), &token.hash, expires_at)
        .await?;
    let url = format!(
        "{}/bootstrap?token={}",
        public_url.trim_end_matches('/'),
        token.raw.expose_secret()
    );
    Ok(Some(url))
}

/// Format the single canonical bootstrap log line.
pub fn bootstrap_log_line(url: &str) -> String {
    format!("[BOOTSTRAP] Visit {url} to claim admin")
}

#[cfg(any(test, feature = "test-support"))]
pub mod test_support {
    //! Helpers exposed only to the acceptance suite. Gated behind the
    //! `test-support` feature so release builds never carry them.

    use super::*;
    use std::net::SocketAddr;
    use tokio::net::TcpListener;

    pub struct TestApp {
        pub addr: SocketAddr,
        pub state: AppState,
        pub shutdown: tokio::sync::oneshot::Sender<()>,
        /// Background pg_listener task — kept so the test harness can
        /// abort it at scenario teardown (the per-scenario schema is
        /// dropped; the listener's connection would otherwise log
        /// noisily on the way down).
        pub listener_task: Option<tokio::task::JoinHandle<()>>,
    }

    impl Drop for TestApp {
        fn drop(&mut self) {
            if let Some(task) = self.listener_task.take() {
                task.abort();
            }
        }
    }

    /// Spin up the slice-1 router on an ephemeral port bound to 127.0.0.1.
    pub async fn spawn_app(state: AppState) -> anyhow::Result<TestApp> {
        let router = build_router(state.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let (tx, rx) = tokio::sync::oneshot::channel::<()>();
        tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    let _ = rx.await;
                })
                .await
                .ok();
        });
        Ok(TestApp {
            addr,
            state,
            shutdown: tx,
            listener_task: None,
        })
    }

    /// As [`spawn_app`] but also spawns the pg_listener background
    /// task against `database_url`. Used by US-09+ scenarios where
    /// the SSE handler needs a live broadcast feed.
    pub async fn spawn_app_with_listener(
        state: AppState,
        database_url: String,
    ) -> anyhow::Result<TestApp> {
        let mut app = spawn_app(state.clone()).await?;
        let task = foundry_realtime::spawn_pg_listener(database_url, state.realtime_tx.clone());
        app.listener_task = Some(task);
        Ok(app)
    }
}
