//! US-05 bootstrap handlers.
//!
//! GET  /bootstrap?token=...  → claim form OR 410 explanatory page
//! POST /bootstrap?token=...  → claim the workspace, set session, 303 → /dashboard
//! POST /invites              → mint a shareable invite link (admin only)
//! POST /workspaces           → 409 Conflict (single-workspace MVP)
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
pub struct WorkspaceForm {
    pub name: String,
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
        Ok(BootstrapTokenStatus::AlreadyUsed) => invalid_page(
            StatusCode::GONE,
            "Link already used",
            "This bootstrap link has already been used to claim the workspace.",
        ),
        Ok(BootstrapTokenStatus::Expired) => invalid_page(
            StatusCode::GONE,
            "Link expired",
            "This bootstrap link has expired. Ask the operator to generate a new one.",
        ),
        Ok(BootstrapTokenStatus::Unknown) => invalid_page(
            StatusCode::GONE,
            "Link not found",
            "This bootstrap link is not recognised. It may have already been used or never existed.",
        ),
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
    // unknown/used/expired — report the same explanatory page.
    let token_row_id = match state.store.claim_bootstrap_token(&hash, now).await {
        Ok(Some(id)) => id,
        Ok(None) => {
            // Report the precise reason for clarity.
            let status = state
                .store
                .bootstrap_token_status(&hash, now)
                .await
                .unwrap_or(BootstrapTokenStatus::Unknown);
            return match status {
                BootstrapTokenStatus::AlreadyUsed => invalid_page(
                    StatusCode::GONE,
                    "Link already used",
                    "This bootstrap link has already been used to claim the workspace.",
                ),
                BootstrapTokenStatus::Expired => invalid_page(
                    StatusCode::GONE,
                    "Link expired",
                    "This bootstrap link has expired. Ask the operator to generate a new one.",
                ),
                _ => invalid_page(
                    StatusCode::GONE,
                    "Link not found",
                    "This bootstrap link is not recognised.",
                ),
            };
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
        if let Err(err) = state.email.send(addr, subject, &body).await {
            tracing::warn!(%err, "email send failed (invite link is still valid)");
        }
    }

    let body = BootstrapInvite { invite_url }
        .render()
        .expect("bootstrap_invite.html renders");
    Html(body).into_response()
}

// --------------------------------------------------------------- POST /workspaces

pub async fn create_workspace(
    State(state): State<AppState>,
    _session: Session,
    Form(_form): Form<WorkspaceForm>,
) -> Response {
    // Slice-1 MVP supports exactly one workspace per instance — the
    // answer is the same regardless of the requester's identity. We
    // keep the single-workspace guard even after the unique index
    // makes a second INSERT impossible, as a defence-in-depth /
    // boring-monolith taste filter (cheap human-readable 409 instead
    // of an opaque DB constraint violation).
    match state.store.workspace_count().await {
        Ok(0) => {
            // No workspace exists — caller should use /bootstrap instead.
            invalid_page(
                StatusCode::BAD_REQUEST,
                "No workspace claimed",
                "Use the bootstrap link to create the initial workspace.",
            )
        }
        Ok(_) => invalid_page(
            StatusCode::CONFLICT,
            "Only one workspace per instance",
            "This Foundry instance already has a workspace. Multi-workspace per \
             instance is not supported in this release; only one workspace per \
             Foundry instance is available.",
        ),
        Err(err) => {
            tracing::error!(%err, "workspace_count failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
        }
    }
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

pub(crate) fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
