//! foundry-app library — composition root pieces that need to be
//! reused by binary + acceptance harness.
//!
//! Slice 1 surface:
//! - [`AppState`] — shared state passed to handlers.
//! - [`build_router`] — pure router construction (testable).
//! - [`mint_bootstrap_if_needed`] — the startup hook that drives US-01.
//! - `/bootstrap` GET/POST — admin claim flow (US-05 scenarios 1-3).
//! - `/invites` POST — generate a shareable invite link (US-05 scenario 4).
//! - `/dashboard` GET — minimal post-claim landing (US-05 scenario 1 redirect target).

#![forbid(unsafe_code)]
#![deny(clippy::all)]

pub mod admin_cli;
pub mod admin_tokens;
pub mod attachments;
pub mod bootstrap;
pub mod clock;
pub mod comments;
pub mod csrf;
pub mod email;
pub mod events;
pub mod instance_admin;
pub mod invites_accept;
pub mod issues;
pub mod keyboard;
pub mod member_invites;
pub mod metrics_server;
pub mod projects;
pub mod rate_limit;
pub mod session;
pub mod signin;
pub mod views;

use axum::extract::State;
use axum::http::StatusCode;
use axum::middleware;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use foundry_realtime::EventPayload;
use foundry_store::Store;
use secrecy::{ExposeSecret, SecretString};
#[cfg(any(test, feature = "test-support"))]
use std::path::PathBuf;
#[cfg(any(test, feature = "test-support"))]
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
#[cfg(any(test, feature = "test-support"))]
use std::sync::Mutex;
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
    /// Feature A (US-W05b, ADR-W02) — the Ed25519 machine-token verifier,
    /// built at boot from `MACHINE_TOKEN_PUBLIC_KEYS` exactly as
    /// `session_secret` is built from `SESSION_SECRET`. Holds a SET of
    /// public keys to support overlapping-key rotation; tries each. The
    /// per-request bearer extractor (02-03, foundry-api) reads it via
    /// `FromRef`. Always present (every binary verifies).
    pub machine_token_verifier: Arc<foundry_auth::MachineTokenVerifier>,
    /// machine-token-admin-ux (US-MT00, ADR-MT01/DD1) — the OPTIONAL Ed25519
    /// SIGNER. `Some` ⇒ this binary is an ISSUER (the mint surface is offered);
    /// `None` ⇒ verifier-only (mint disabled + UI hidden / 403, graceful per
    /// OD1/DD2). Loaded in `main.rs` from `MACHINE_TOKEN_SIGNING_KEY` in the SAME
    /// block that already runs the boot self-test, retained ONLY after the probe
    /// passes (signer.md). NOT exposed via `FromRef`; the `admin_tokens` handler
    /// reads it directly from `State<AppState>` and passes it to
    /// `services.mint_token(signer, …)` (DD4 — confined to the mint call path).
    pub machine_token_signer: Option<Arc<foundry_auth::MachineTokenSigner>>,
    pub session_cookie_secure: bool,
    /// Postgres schema where the `session` table lives. `"public"` in
    /// production, a per-scenario name like `"test_s17_ab12"` in the
    /// acceptance harness.
    pub db_schema: String,
    pub public_url: String,
    pub clock: Arc<dyn Clock>,
    pub email: Arc<dyn EmailSender>,
    /// US-TMA05 (NFR-TMA-SEC-07 / OD-TMA-1) — the in-process per-principal
    /// revoke-storm guardrail. A token bucket keyed by the bound `user_id`,
    /// checked on the DELETE token route AFTER auth and BEFORE
    /// `Services::revoke_token`. Derived into the foundry-api adapter via
    /// `FromRef` exactly like `Services` / `machine_token_verifier`, so
    /// foundry-api reads the shared guardrail through the existing seam and
    /// gains no new crate dependency. The bucket reads `self.clock` so its
    /// refill is deterministic under the acceptance harness's `MockClock`.
    /// This is an adapter transport-rate policy, not a domain rule — it lives
    /// in the composition root, never in foundry-services.
    pub revoke_rate_limiter: Arc<rate_limit::RevokeRateLimiter>,
    /// Broadcast channel for realtime events. Cloning the sender is
    /// cheap (Arc inside); each SSE connection subscribes for a fresh
    /// Receiver. The pg-listener task is the sole publisher.
    pub realtime_tx: broadcast::Sender<EventPayload>,
    /// SSE keepalive interval in ms. Defaulted from
    /// `DEFAULT_SSE_HEARTBEAT_MS`; tests override via the harness
    /// helper `support::heartbeat_env::override_heartbeat_ms`.
    pub sse_heartbeat_ms: u64,
    /// US-11 — maximum upload size for issue attachments in MEGABYTES
    /// (NFR-PERF-02). Sourced from the `FILE_UPLOAD_MAX_MB` env var
    /// (default 10). Acceptance uses 10; production deployments may
    /// raise to 50.
    pub file_upload_max_mb: u64,
    /// US-02 (NFR-OBS-02) — test-only health-injection flag. When set,
    /// `/readyz` short-circuits to 503 without touching the real DB
    /// probe. Lets the multi-replica acceptance scenarios simulate
    /// "database unreachable" without poisoning the shared
    /// testcontainers Postgres for sibling scenarios.
    ///
    /// Compiled only under the `test-support` feature (or tests), so
    /// release builds never carry the seam. The production `/readyz`
    /// path remains the real DB round-trip.
    #[cfg(any(test, feature = "test-support"))]
    pub db_unreachable: Arc<AtomicBool>,
    /// US-B01 (error-and-observability.md §"Render-error handling") —
    /// test-only render-injection flag. When set, `GET` of the board view
    /// forces the board template render to return `Err` so the handler can
    /// be observed mapping a render failure to a CLEAN 500 (never a
    /// half-emitted page; an htmx-fragment request gets a 500 fragment).
    /// Mirrors [`Self::db_unreachable`] exactly: an `Arc<AtomicBool>` the
    /// acceptance harness flips per-scenario, compiled only under
    /// `cfg(any(test, feature = "test-support"))` so release builds never
    /// carry the seam.
    #[cfg(any(test, feature = "test-support"))]
    pub force_board_render_failure: Arc<AtomicBool>,
    /// US-04 — optional path to a runtime migrations directory the test
    /// harness has staged (typically a `tempfile::TempDir`). When `Some`,
    /// the boot helper runs migrations via
    /// `foundry_store::run_migrations_from_dir` instead of the compile-
    /// time `migrate!` macro. Production never sets this; the field is
    /// gated behind `test-support` so release builds without the feature
    /// do not carry it.
    #[cfg(any(test, feature = "test-support"))]
    pub test_migrations_dir: Option<PathBuf>,
    /// US-04 — per-replica record of which migration versions THIS
    /// replica observed as newly applied during boot. The "exactly one
    /// replica reports having applied schema update '0099'" assertion
    /// reads this from each replica's AppState. Shared
    /// `Arc<Mutex<MigrationReport>>` so the boot path can populate it
    /// from outside `AppState::clone()`. Released only via
    /// `applied_migration_versions()`.
    #[cfg(any(test, feature = "test-support"))]
    pub applied_migrations: Arc<Mutex<foundry_store::MigrationReport>>,
    /// US-04 — per-replica slow-migration delay in milliseconds. The
    /// boot path forwards this to
    /// `foundry_store::run_migrations_from_dir_with_delay`. Non-zero
    /// values are honoured ONLY when this replica has migration work
    /// to do (i.e. won the advisory-lock race); the loser observes
    /// no-op work and skips the sleep. This models the "winner is
    /// slow / loser blocks on the lock then proceeds" semantic the
    /// US-04 lock-race scenario asserts against.
    #[cfg(any(test, feature = "test-support"))]
    pub test_migration_delay_ms: u64,
}

/// Default upload cap per NFR-PERF-02. The env var overrides this.
pub const DEFAULT_FILE_UPLOAD_MAX_MB: u64 = 10;

/// Expose the shared application-service handle to sub-routers (foundry-api's
/// `/api/v1` group) via axum's `FromRef`, so `foundry_api::routes::<AppState>()`
/// can extract `State<Services>` without foundry-api depending on foundry-app —
/// and WITHOUT foundry-api naming `foundry_store::Store` (the `Services` handle
/// owns the only `Arc<Store>`, the structural fact the `foundry-api ⊀
/// foundry-store` boundary-guard ban enforces). Cloning is an `Arc` clone.
impl axum::extract::FromRef<AppState> for foundry_services::Services {
    fn from_ref(state: &AppState) -> Self {
        foundry_services::Services::new(state.store.clone())
    }
}

/// Feature A (US-W05b) — expose the machine-token verifier to the
/// `foundry-api` bearer extractor (02-03) via `FromRef`, the same way
/// `Arc<Store>` is exposed, so foundry-api need not depend on foundry-app.
impl axum::extract::FromRef<AppState> for Arc<foundry_auth::MachineTokenVerifier> {
    fn from_ref(state: &AppState) -> Self {
        state.machine_token_verifier.clone()
    }
}

/// US-TMA05 — expose the per-principal revoke guardrail to the foundry-api
/// DELETE token handler as the `foundry_api::RevokeRateGuard` driven port via
/// `FromRef`, the same way the verifier is exposed. foundry-api extracts
/// `State<Arc<dyn RevokeRateGuard>>` and never names the concrete bucket, so it
/// depends on neither foundry-app nor a new crate. The guard binds the shared
/// limiter (bucket STATE persists across requests behind its own `Mutex`) to the
/// SHIPPED `clock` seam, so refill is deterministic under the harness MockClock.
impl axum::extract::FromRef<AppState> for Arc<dyn foundry_api::RevokeRateGuard> {
    fn from_ref(state: &AppState) -> Self {
        Arc::new(rate_limit::ClockedRevokeGuard::new(
            state.revoke_rate_limiter.clone(),
            state.clock.clone(),
        ))
    }
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("session_cookie_secure", &self.session_cookie_secure)
            .field("public_url", &self.public_url)
            .finish_non_exhaustive()
    }
}

/// Resolve the directory `ServeDir` serves `/static` from.
///
/// The deployed binary runs with the vendored `static/` directory `COPY`'d
/// alongside it (cwd-relative — `docs/feature/htmx-web-tier/design/assets.md`
/// §Dockerfile note), so a plain cwd-relative `static` is preferred when it
/// exists. The in-process acceptance harness, benches, and `cargo test` run
/// with a different cwd (the test package, not `crates/foundry-app/`), so we
/// fall back to the crate-local `static/` resolved at compile time via
/// `CARGO_MANIFEST_DIR`. This keeps ONE serving path correct in both
/// production and test without a runtime env var.
fn static_dir() -> std::path::PathBuf {
    let cwd_relative = std::path::PathBuf::from("static");
    if cwd_relative.is_dir() {
        return cwd_relative;
    }
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static")
}

/// Build the axum router for slice 1.
pub fn build_router(state: AppState) -> Router {
    let session_layer = session::build_session_layer(
        state.store.pool().clone(),
        &state.db_schema,
        state.session_cookie_secure,
    );
    // US-11 attachments: separate sub-router so the per-route
    // DefaultBodyLimit cap rides only on the upload POST and doesn't
    // affect the rest of the surface.
    let attachment_routes = attachments::build_routes(state.clone());
    // Feature B (US-B02 / design/assets.md) — vendored static assets served by
    // the binary itself: pure pre-vendored blobs under
    // `crates/foundry-app/static/` (htmx/Alpine `.min.js` + the content-hashed
    // `foundry.<hash>.css`), served via `tower_http::services::ServeDir` (already
    // a dep). Mounted on the base router OUTSIDE the session + CSRF layers —
    // `/static` is GET-only public, non-secret, vendored content that needs no
    // auth. ServeDir refuses `..` traversal by construction (US-B02 traversal
    // @error). The long-lived immutable cache header is correct because EVERY
    // served blob's content is pinned by its committed name: the vendored libs
    // carry the upstream version, and the hand-authored CSS carries a content
    // hash in its filename (`foundry.<sha256-prefix>.css`, ADR-B03 / assets.md
    // Decision #4 option 4a). A CSS edit changes the hash → the filename → the
    // URL `base.html` references, so `immutable` never pins stale CSS. See
    // VENDOR.md.
    let static_service = tower::ServiceBuilder::new()
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("public, max-age=31536000, immutable"),
        ))
        .service(tower_http::services::ServeDir::new(static_dir()));
    let router = Router::new()
        .nest_service("/static", static_service)
        .merge(attachment_routes)
        .route("/healthz", get(healthz))
        .route("/readyz", get(readyz));
    // US-02 test-only long-running endpoint. Compiled only under the
    // `test-support` feature so production builds never expose it.
    // The acceptance crate enables `test-support` (see
    // foundry-acceptance/Cargo.toml); the binary build of `foundry`
    // does not. SIGTERM-drain + in-flight-completes scenarios POST to
    // this endpoint to occupy a request slot for ~3 seconds while
    // other steps run against /readyz / GET /dashboard concurrently.
    #[cfg(any(test, feature = "test-support"))]
    let router = router.route("/__test/slow", get(test_slow));
    router
        .route("/dashboard", get(bootstrap::dashboard))
        .route(
            "/bootstrap",
            get(bootstrap::show_form).post(bootstrap::submit),
        )
        .route("/invites", post(bootstrap::create_invite))
        .route(
            "/sign-in",
            get(signin::show_form).post(signin::submit_signin),
        )
        .route("/sign-out", post(signin::submit_signout))
        // invite-accept-flow 01-01 (ADR-001/003/004) — the PUBLIC claim-your-account
        // route pair. Mounted HERE, alongside the PUBLIC `/sign-in` + `/bootstrap`
        // (NOT behind the instance-admin gate — the invitee is signed OUT), so it
        // sits UNDER `csrf_middleware` + `session_layer` below: the GET mints the
        // double-submit CSRF cookie (signed-out, like sign-in) and the POST is
        // CSRF-screened before the handler runs. GET verifies the signed
        // `InviteToken` + advisory liveness and renders the set-password form; POST
        // runs the min-12 policy, the one-TX consume+write, the session establish,
        // and the 303 onto the workspace (invites_accept.rs).
        .route(
            "/invites/accept",
            get(invites_accept::show_accept_form).post(invites_accept::submit_accept),
        )
        // multi-workspace-tenancy 02-05 (ADR-005) — the multi-membership active-
        // workspace switcher. Mounted HERE so it sits UNDER `csrf_middleware` +
        // `session_layer` below (a real signed-in cookie + double-submit `_csrf`,
        // like every browser POST). Membership-guarded + fail-closed inside the
        // handler (Store::set_active_workspace).
        .route("/workspace/switch", post(session::submit_switch))
        // workspace-member-invites 01-01 (US-01) — the admin-gated member-invite
        // ISSUANCE surface. Mounted HERE on the SHARED layer (UNDER `csrf_middleware`
        // + `session_layer` below) alongside `/admin/tokens` + `/workspace/switch`: a
        // real signed-in `foundry_session` cookie + double-submit `_csrf` apply. The
        // handler gates on the SHIPPED `is_workspace_admin` (D7) — a non-admin /
        // signed-out caller gets the SHIPPED non-enumerable uniform 404 (NFR-1). It
        // resolves the acting workspace from the SESSION (no request-parsed workspace
        // id), so it is LAYER-1e safe (no check_arch allow-list line). The POST
        // inserts the invite (`created_by = the inviter`) + signs + emits the accept
        // link + best-effort email + renders the "invite sent" fragment
        // (member_invites.rs).
        .route(
            "/workspace/invites",
            get(member_invites::show_invite_form).post(member_invites::submit_invite),
        )
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
        // issue-edit-dialog (ADR-001) — GET the pre-filled edit dialog, POST the
        // save. Same shared layer (UNDER `csrf_middleware` + `session_layer`): the
        // save is a double-submit `_csrf` browser POST, and the no-JS fallback is a
        // native `method="post"` form (so POST, not PATCH). Tenancy is scoped by the
        // resolved acting workspace inside the service (ADR-002); a foreign issue
        // 404s non-enumerably (ADR-003).
        .route(
            "/team/{team_slug}/project/{project_slug}/issues/{issue_number}/edit",
            get(issues::show_edit_form).post(issues::submit_edit),
        )
        .route(
            "/team/{team_slug}/project/{project_slug}/events",
            get(events::sse_stream),
        )
        .route(
            "/team/{team_slug}/project/{project_slug}/issues/{issue_number}",
            get(comments::show_issue),
        )
        .route(
            "/team/{team_slug}/project/{project_slug}/issues/{issue_number}/comments",
            post(comments::submit_comment),
        )
        // Slice-5 (US-10 deferred ACs): edit + delete + edit-form + cancel.
        .route(
            "/team/{team_slug}/project/{project_slug}/issues/{issue_number}/comments/{comment_id}/edit",
            get(comments::show_edit_form),
        )
        .route(
            "/team/{team_slug}/project/{project_slug}/issues/{issue_number}/comments/{comment_id}",
            get(comments::show_single_comment)
                .patch(comments::submit_edit_comment)
                .delete(comments::submit_delete_comment),
        )
        .route("/keyboard-help", get(keyboard::show_keyboard_help))
        // machine-token-admin-ux (ADR-MT03/DD5) — the browser admin surface for
        // machine-token issuance + lifecycle. Mounted HERE, ALONGSIDE the HTML
        // routes, so it sits UNDER `csrf::csrf_middleware` + `session_layer`
        // below (double-submit `_csrf` + tower-sessions apply unchanged,
        // NFR-MT-SEC-07) — NOT the CSRF-exempt `/api/v1` mount. Workspace is
        // implicit from the session (OD3). RED scaffold handlers (admin_tokens.rs).
        .route(
            "/admin/tokens",
            get(admin_tokens::show_index).post(admin_tokens::submit_mint),
        )
        .route(
            "/admin/tokens/{jti}/revoke",
            post(admin_tokens::submit_revoke),
        )
        // web-provisioning-flow 01-01 (ADR-001 / D2) — the INSTANCE super-admin
        // provisioning surface. Mounted HERE, ALONGSIDE the HTML routes, so it
        // sits UNDER `csrf::csrf_middleware` + `session_layer` below (a real
        // signed-in `foundry_session` cookie + double-submit `_csrf`, like every
        // browser POST). The `require_instance_admin` session gate inside the
        // handler refuses a signed-out / non-super-admin caller with the SHIPPED
        // non-enumerable uniform 404 (ADR-002). Drives the SHIPPED
        // `Services::provision_workspace` use-case (instance_admin.rs).
        .route(
            "/admin/instance/workspaces",
            get(instance_admin::show_dashboard).post(instance_admin::submit_provision),
        )
        // web-provisioning-flow 01-03 (ADR-001 / D1) — the INSTANCE super-admin
        // grant surface. Same mount (UNDER `csrf_middleware` + `session_layer`):
        // the `require_instance_admin` gate refuses a signed-out / non-super-admin
        // caller with the SHIPPED non-enumerable uniform 404 (ADR-002). Drives the
        // SHIPPED grant path (`user_id_by_email` + idempotent `grant_instance_admin`,
        // the CLI's proven backend legs) and renders a non-committal confirmation
        // fragment (instance_admin.rs).
        .route(
            "/admin/instance/super-admins",
            post(instance_admin::submit_grant),
        )
        .route("/", get(signin::dashboard_root))
        // Non-enumerability (ADR-002, web-provisioning-flow): a path with NO
        // route is refused with the SAME uniform `resource_not_found_page()` the
        // tenancy/instance-admin gates return for a foreign-or-missing resource.
        // Without this, axum's default fallback returns a bare empty-body 404,
        // which DIFFERS from the gated-surface refusal — a body oracle that lets
        // an attacker distinguish "the admin surface exists but you can't reach
        // it" (styled 404 page) from "this path never existed" (empty 404). The
        // fallback collapses both to one byte-identical refusal, so the
        // `/admin/instance/…` signed-out refusal is indistinguishable from a
        // never-existed path (ADR-002 response-mapping contract).
        .fallback(uniform_not_found)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            csrf::csrf_middleware,
        ))
        .layer(session_layer)
        // Feature A (US-W05b) — the JSON `/api/v1` surface contributed by the
        // foundry-api driving adapter (api-contract.md §"Route surface"). It is
        // a PEER of the HTML routes over the shared `foundry-services` seam.
        // Slice 2 mounts it OUTSIDE the session + CSRF layers (auth.md
        // §Coexistence): a machine request carries a bearer JWT and NO cookie,
        // so CSRF-exemption is correct by construction and the foundry-api
        // `MachinePrincipal` extractor authenticates instead. The browser
        // cookie path's session/CSRF behaviour is byte-for-byte unchanged
        // (NFR-WEB-API-SEC-01) — those layers simply do not run on `/api/v1`.
        .merge(foundry_api::routes::<AppState>())
        // Slice 6 (ADR-010) — one tower layer per request emits
        // `http_requests_total{path,method,status}` + the matching
        // duration histogram. Sits at the same tower-stack position as
        // CSRF / session — composes naturally with all existing layers.
        // The layer applies to every routed request uniformly; zero
        // handler-signature changes.
        .layer(metrics_server::request_tracking_layer())
        .with_state(state)
}

/// Router fallback for any unmatched path: the SHIPPED uniform
/// `resource_not_found_page()` (ADR-002 non-enumerability). A never-existed path
/// is refused BYTE-IDENTICALLY to a gated surface's signed-out/non-member refusal,
/// so neither status nor body reveals whether a path corresponds to a real-but-
/// forbidden resource or to nothing at all.
async fn uniform_not_found() -> Response {
    bootstrap::resource_not_found_page()
}

async fn healthz() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

async fn readyz(State(state): State<AppState>) -> impl IntoResponse {
    // US-02 NFR-OBS-02: the acceptance harness can flip a test-only
    // flag to short-circuit /readyz without touching the real DB. This
    // simulates the "Postgres unreachable" condition for every replica
    // sharing the same flag, WITHOUT killing the shared testcontainers
    // Postgres (which would poison sibling scenarios). The seam is
    // compiled only under cfg(any(test, feature = "test-support")); the
    // release-feature path falls straight through to the probe below.
    #[cfg(any(test, feature = "test-support"))]
    {
        use std::sync::atomic::Ordering;
        if state.db_unreachable.load(Ordering::SeqCst) {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                r#"{"status":"not_ready","reason":"db_unreachable","detail":"injected"}"#,
            )
                .into_response();
        }
    }
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

/// US-02 test-only long-running endpoint. Holds the handler open for
/// ~3 seconds so the "long-running request that is being served by a
/// specific replica" + "SIGTERM-drain" scenarios can model a request
/// that overlaps replica shutdown.
///
/// Returns `200 OK` with the body `slow-done` once the sleep elapses.
/// The acceptance harness POSTs to this endpoint, then sends shutdown
/// to the same replica, then awaits the response.
///
/// Compiled only under `cfg(any(test, feature = "test-support"))`;
/// production binaries never carry this route.
#[cfg(any(test, feature = "test-support"))]
async fn test_slow() -> impl IntoResponse {
    tokio::time::sleep(std::time::Duration::from_millis(3000)).await;
    (StatusCode::OK, "slow-done")
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

    /// US-04 boot helper: if `state.test_migrations_dir` is Some, run
    /// `foundry_store::run_migrations_from_dir(pool, dir)` against the
    /// shared pool under the advisory lock. Mirrors the slice-1
    /// production boot semantics (which uses the compile-time
    /// `migrate!`) but lets the US-04 acceptance suite stage runtime
    /// migrations into a `tempfile::TempDir` per scenario.
    ///
    /// On success, records the per-invocation [`MigrationReport`] into
    /// `state.applied_migrations` so per-replica assertions can read
    /// what THIS replica applied vs observed as already-applied. On
    /// failure (e.g. the deliberately-broken 0099), returns the
    /// underlying error and leaves `applied_migrations` untouched.
    ///
    /// Callers that do NOT set `test_migrations_dir` get a no-op
    /// (the caller-built pool's migrations are assumed pre-applied,
    /// which matches the slice-1 harness contract).
    pub async fn boot_test_migrations(state: &AppState) -> Result<(), foundry_store::StoreError> {
        if let Some(dir) = &state.test_migrations_dir {
            let report = foundry_store::run_migrations_from_dir_with_delay(
                state.store.pool(),
                dir.as_path(),
                state.test_migration_delay_ms,
            )
            .await?;
            if let Ok(mut slot) = state.applied_migrations.lock() {
                *slot = report;
            }
        }
        Ok(())
    }
}
