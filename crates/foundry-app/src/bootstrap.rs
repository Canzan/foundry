//! US-05 bootstrap handlers.
//!
//! GET  /bootstrap?token=...  → claim form OR 410 explanatory page
//! POST /bootstrap?token=...  → claim the workspace, set session, 303 → /dashboard
//! POST /invites              → mint a shareable invite link (admin only)
//! GET  /dashboard            → minimal "workspace dashboard" landing

use crate::session::SESSION_KEY_USER_ID;
use crate::views::{BootstrapClaim, BootstrapDashboard, BootstrapInvite, InvalidPage};
use crate::AppState;
use askama::Template;
use axum::extract::{Form, Query, State};
use axum::http::header::{HeaderMap, HeaderValue, LOCATION};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use foundry_store::BootstrapTokenStatus;
use secrecy::SecretString;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tower_sessions::Session;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct SessionUser {
    pub user_id: uuid::Uuid,
    pub workspace_id: uuid::Uuid,
}

impl SessionUser {
    /// The RESOLVED acting workspace for this signed-in session (ADR-002).
    ///
    /// `workspace_id` was stamped at sign-in by the resolution seam
    /// (`resolve_active_workspace`, ADR-005), so this is the single trusted
    /// origin of an [`crate::session::ActingWorkspace`]. Handlers call this and
    /// scope every tenant query by the result, never by a path/query/body id.
    pub(crate) fn acting_workspace(&self) -> crate::session::ActingWorkspace {
        crate::session::ActingWorkspace::from_resolved(self.workspace_id)
    }
}

#[derive(Debug, Deserialize)]
pub struct TokenQuery {
    pub token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BootstrapForm {
    pub email: String,
    pub password: String,
    pub display_name: String,
    pub workspace_name: String,
}

#[derive(Debug, Deserialize)]
pub struct InviteForm {
    #[serde(default)]
    pub email: Option<String>,
}

// ---------------------------------------------------------------- GET /bootstrap

pub async fn show_form(State(state): State<AppState>, Query(q): Query<TokenQuery>) -> Response {
    let Some(token) = q.token else {
        return invalid_page(
            StatusCode::BAD_REQUEST,
            "Missing token",
            "The bootstrap URL is missing a token parameter.",
        );
    };
    let hash = sha256(&token);
    let now = state.clock.now();
    match state.store.bootstrap_token_status(&hash, now).await {
        Ok(BootstrapTokenStatus::Valid) => Html(render_claim_form(&token)).into_response(),
        // SECURITY (enumeration oracle): every NON-valid reason — already-used,
        // expired, unknown — collapses to ONE byte-identical refusal so a prober
        // cannot tell why the link is dead. The precise reason goes ONLY to
        // tracing, never the response body/status.
        Ok(reason) => {
            tracing::info!(?reason, "bootstrap claim link refused (non-enumerable)");
            bootstrap_refusal_page()
        }
        Err(err) => {
            tracing::error!(%err, "bootstrap_token_status failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
        }
    }
}

// --------------------------------------------------------------- POST /bootstrap

pub async fn submit(
    State(state): State<AppState>,
    session: Session,
    Query(q): Query<TokenQuery>,
    Form(form): Form<BootstrapForm>,
) -> Response {
    let Some(token) = q.token else {
        return invalid_page(
            StatusCode::BAD_REQUEST,
            "Missing token",
            "The bootstrap URL is missing a token parameter.",
        );
    };
    let hash = sha256(&token);
    let now = state.clock.now();

    // Atomic single-use claim. If this returns None, the token is
    // unknown/used/expired — SECURITY (enumeration oracle): all three reasons
    // render the SAME byte-identical refusal (status + body) so a prober cannot
    // distinguish them. We deliberately do NOT re-query bootstrap_token_status on
    // this path: knowing the precise reason would only let us leak it. The reason
    // is recoverable from tracing on the failed-claim path if ever needed.
    let token_row_id = match state.store.claim_bootstrap_token(&hash, now).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            tracing::info!("bootstrap claim refused: token not claimable (non-enumerable)");
            return bootstrap_refusal_page();
        }
        Err(err) => {
            tracing::error!(%err, "claim_bootstrap_token failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };
    let _ = token_row_id;

    // Hash the password.
    let pwd = SecretString::new(form.password.into());
    let password_hash = match foundry_auth::hash_password(&pwd).await {
        Ok(h) => h,
        Err(err) => {
            tracing::error!(%err, "hash_password failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };

    let workspace_id = uuid::Uuid::now_v7();
    let user_id = uuid::Uuid::now_v7();
    let team_id = uuid::Uuid::now_v7();
    let project_id = uuid::Uuid::now_v7();
    let email_lower = form.email.trim().to_lowercase();
    let email_display = form.email.trim();

    if let Err(err) = state
        .store
        .create_initial_workspace(
            workspace_id,
            form.workspace_name.trim(),
            user_id,
            &email_lower,
            email_display,
            form.display_name.trim(),
            &password_hash,
            team_id,
            "General",
            "general",
            project_id,
            "Sandbox",
            "sandbox",
            "GEN",
        )
        .await
    {
        tracing::error!(%err, "create_initial_workspace failed");
        return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
    }

    // tower-sessions: insert the signed-in user id and let the
    // SessionManagerLayer emit the Set-Cookie on response.
    if let Err(err) = session
        .insert(
            SESSION_KEY_USER_ID,
            SessionUser {
                user_id,
                workspace_id,
            },
        )
        .await
    {
        tracing::error!(%err, "session.insert failed");
        return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
    }

    let mut headers = HeaderMap::new();
    headers.insert(LOCATION, HeaderValue::from_static("/dashboard"));
    (StatusCode::SEE_OTHER, headers, "").into_response()
}

// --------------------------------------------------------------- GET /dashboard

pub async fn dashboard(State(_state): State<AppState>, session: Session) -> Response {
    let signed_in = session
        .get::<SessionUser>(SESSION_KEY_USER_ID)
        .await
        .ok()
        .flatten()
        .is_some();
    let body = BootstrapDashboard { signed_in }
        .render()
        .expect("bootstrap_dashboard.html renders");
    Html(body).into_response()
}

// --------------------------------------------------------------- POST /invites

pub async fn create_invite(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<InviteForm>,
) -> Response {
    let Some(user) = session
        .get::<SessionUser>(SESSION_KEY_USER_ID)
        .await
        .ok()
        .flatten()
    else {
        return (StatusCode::UNAUTHORIZED, "sign-in required").into_response();
    };

    let invite_id = uuid::Uuid::now_v7();
    let now = state.clock.now();
    let expires_at = now + time::Duration::days(7);

    if let Err(err) = state
        .store
        .insert_invite(
            invite_id,
            user.workspace_id,
            form.email.as_deref(),
            user.user_id,
            expires_at,
        )
        .await
    {
        tracing::error!(%err, "insert_invite failed");
        return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
    }

    let token = match foundry_auth::InviteToken::new(invite_id, expires_at, &state.session_secret) {
        Ok(t) => t,
        Err(err) => {
            tracing::error!(%err, "InviteToken::new failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };

    let invite_url = format!(
        "{}/invites/accept?id={}&sig={}",
        state.public_url.trim_end_matches('/'),
        invite_id,
        urlencoding::encode(&token.signature),
    );

    // If the form carried an `email`, also send an email invite.
    if let Some(addr) = form.email.as_deref().filter(|s| !s.is_empty()) {
        let subject = "You have been invited to a Foundry workspace";
        let body = format!(
            "You have been invited to join the workspace. Accept the invite here:\n\n{invite_url}\n\nThis link is valid for 7 days.",
        );
        // Best-effort delivery — a failure is non-fatal; the invite link is still
        // rendered below.
        let notification = crate::notify::Notification {
            event: crate::notify::NotificationEvent::WorkspaceInvite,
            recipient: addr.to_string(),
            subject: subject.to_string(),
            body,
        };
        state.notifier.notify(&notification).await;
    }

    let body = BootstrapInvite { invite_url }
        .render()
        .expect("bootstrap_invite.html renders");
    Html(body).into_response()
}

// ----------------------------------------------------------------------- helpers

fn sha256(input: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hasher.finalize().into()
}

fn render_claim_form(token: &str) -> String {
    BootstrapClaim {
        token: token.to_string(),
    }
    .render()
    .expect("bootstrap_claim.html renders")
}

/// The UNIFORM non-enumerable bootstrap-claim refusal (the security crux,
/// mirroring `invites_accept::invite_refusal_page`). EVERY non-valid token
/// reason — already-used, expired, unknown/forged — collapses to THIS
/// byte-identical response (status AND full body), on BOTH the GET claim-form
/// path and the POST claim path. It leaks NONE of: whether a token id ever
/// existed, nor its state (used vs expired vs never-existed). The precise reason
/// lives ONLY in internal `tracing`, never in the body or status.
///
/// Status is 200 OK to match the ratified invite-accept-flow convention
/// (`invite_refusal_page`, OD-3 2026-06-14): a uniform status avoids even a
/// status-code oracle, and "this page exists, the link is dead" is the honest
/// UX. Supersedes the prior three distinct 410 GONE pages, which were an
/// enumeration oracle (the leak recorded in invite-accept-flow's
/// upstream-changes.md Finding 2).
pub(crate) fn bootstrap_refusal_page() -> Response {
    invalid_page(
        StatusCode::OK,
        "This bootstrap link is no longer valid",
        "It may have expired, already been used, or been mistyped. \
         Ask the operator to generate a new bootstrap link.",
    )
}

pub(crate) fn invalid_page(status: StatusCode, heading: &str, message: &str) -> Response {
    // US-R06: the ~17 callers across 7 modules stay UNCHANGED — only this helper
    // body switches to the SHARED `invalid_page.html` (extends `base.html`, links
    // the vendored `/static` stylesheet). Restyles every not-found/error path at
    // once. The `<h1>{heading}</h1><p>{message}</p>` shape is byte-stable; both
    // fields are auto-escaped (matching the previous `html_escape`).
    let body = InvalidPage {
        heading: heading.to_string(),
        message: message.to_string(),
    }
    .render()
    .expect("invalid_page.html renders");
    (status, Html(body)).into_response()
}

/// The UNIFORM cross-tenant / missing-resource refusal page (ADR-003,
/// multi-workspace-tenancy).
///
/// Every tenant-scoped web resource (board, issue, project write) that resolves
/// to `None` — whether because the id never existed OR because it belongs to a
/// FOREIGN workspace — renders THIS exact response: a fixed 404 whose body
/// carries NO requested identifier (no team/project slug, no issue number). A
/// foreign-id reach and a never-existed reach are therefore byte-identical
/// (same status, same body), so there is no status/body/shape oracle that could
/// confirm a foreign resource exists (NFR-MWT-SEC-02 / DM2). Generalises the
/// shipped `find_attachment_in_workspace → None → 404` idiom to the board/issue/
/// write web paths.
///
/// Distinct from [`invalid_page`], whose callers (intra-workspace authz,
/// validation, the single-workspace guard) legitimately echo the requested
/// slug — those are NOT cross-tenant concerns (ADR-003's boundary clause).
pub(crate) fn resource_not_found_page() -> Response {
    invalid_page(
        StatusCode::NOT_FOUND,
        "Not found",
        "The requested resource does not exist or is not available.",
    )
}

pub(crate) fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
