//! Workspace member-invite issuance web surface — `/workspace/invites`
//! (workspace-member-invites US-01).
//!
//! WHY-NEW-FILE: crates/foundry-app/src/member_invites.rs
//!   CLOSEST-EXISTING: crates/foundry-app/src/bootstrap.rs (`create_invite`)
//!   EXTENSION-COST: `create_invite` is an UNGATED POST-only handler that returns a
//!     401 for a missing session and trusts `SessionUser.workspace_id` blindly; the
//!     member-invite surface is an ADMIN-GATED GET+POST pair that must refuse a
//!     non-admin / signed-out caller with the SHIPPED non-enumerable uniform 404
//!     (NFR-1) and render an admin form — folding both into `create_invite` would
//!     entangle the bootstrap claim-flow's invite minting with a distinct
//!     admin-gated workspace surface (different authz, different GET render, different
//!     refusal posture).
//!   PARALLEL-RATIONALE: this surface is gated INSIDE the handler by the SHIPPED
//!     `is_workspace_admin` (a different authz boundary from `create_invite`'s bare
//!     session check) and serves a GET admin form with a non-enumerable 404 for
//!     non-admins — a different request shape (GET+POST), authz boundary, and refusal
//!     contract from the bootstrap invite minter; it mirrors the `instance_admin`
//!     issuance idiom (gated GET+POST + fragment) rather than `create_invite`.
//!
//! A NEW admin-gated web driving adapter mirroring the shipped `bootstrap::create_invite`
//! (insert_invite + InviteToken::new + emit link + best-effort email) and the
//! `instance_admin` issuance idiom (gated GET form + POST fragment):
//!   GET  /workspace/invites — admin → one-email-field form + CSRF cookie;
//!                             non-admin / signed-out → non-enumerable uniform 404.
//!   POST /workspace/invites — email + `_csrf` → `is_workspace_admin` gate → resolve
//!                             the admin's workspace from `SessionUser` → `insert_invite`
//!                             (`created_by = the inviter`, `invitee_email = the typed
//!                             email`) → `InviteToken::new` → emit the signed
//!                             `/invites/accept` link → best-effort email → render the
//!                             "invite sent" fragment. A blank email re-renders the
//!                             form inline with an error (NO invite). A non-admin /
//!                             signed-out caller gets the uniform 404; CSRF is enforced
//!                             by the surrounding `csrf_middleware`.
//!
//! Mounted in `build_router` on the SHARED layer (UNDER `csrf_middleware` +
//! `session_layer`) alongside `/admin/tokens` + `/workspace/switch` — a real
//! signed-in `foundry_session` cookie + double-submit `_csrf` apply.
//!
//! LAYER-1e (D7): this handler scopes nothing by a request-parsed workspace id — it
//! resolves the acting workspace from the SESSION (`SessionUser.workspace_id`, stamped
//! by `resolve_active_workspace` at sign-in) and gates on the SHIPPED
//! `is_workspace_admin`, so it does NOT trip the tenant-scoping detector (no
//! check_arch allow-list line needed).

use crate::bootstrap::{resource_not_found_page, SessionUser};
use crate::csrf::{build_csrf_cookie, extract_csrf_cookie, generate_token};
use crate::session::SESSION_KEY_USER_ID;
use crate::views::{MemberInviteForm, MemberInviteSent};
use crate::AppState;
use askama::Template;
use axum::extract::{Form, State};
use axum::http::header::{HeaderMap, HeaderValue, COOKIE, SET_COOKIE};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use serde::Deserialize;
use tower_sessions::Session;

/// How long a freshly-issued member invite is valid (mirrors `bootstrap::create_invite`).
const INVITE_TTL_DAYS: i64 = 7;
const BLANK_EMAIL_ERROR: &str = "Enter an email address to invite a member.";

#[derive(Debug, Deserialize)]
pub struct IssueForm {
    #[serde(default)]
    pub email: Option<String>,
    #[serde(rename = "_csrf", default)]
    _csrf: Option<String>,
}

// ------------------------------------------------------- GET /workspace/invites

/// Render the one-email-field member-invite form for a signed-in WORKSPACE ADMIN,
/// naming the workspace + minting the double-submit CSRF cookie. A non-admin /
/// signed-out caller gets the SHIPPED non-enumerable uniform 404 (NFR-1) —
/// byte-identical to a never-existed path, no oracle the surface exists.
pub async fn show_invite_form(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let Some((_admin, workspace_name)) = require_workspace_admin(&state, &session).await else {
        return resource_not_found_page();
    };
    let (csrf_token, set_cookie) = ensure_csrf_cookie(&state, &headers);
    render_form(&csrf_token, &workspace_name, None, set_cookie)
}

// ------------------------------------------------------ POST /workspace/invites

/// Issue a member invite for the typed email (US-01). CSRF is enforced by the
/// surrounding `csrf_middleware`. Gates on `is_workspace_admin`; a non-admin /
/// signed-out caller gets the uniform 404. A blank email re-renders the form inline
/// (NO invite). On a valid email: `insert_invite` (`created_by = the inviter`,
/// `invitee_email = the typed email`), `InviteToken::new`, emit the signed accept
/// link, best-effort email (a send failure is non-fatal — the link is still
/// rendered), and render the "invite sent" fragment.
pub async fn submit_invite(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<IssueForm>,
) -> Response {
    let Some((admin, workspace_name)) = require_workspace_admin(&state, &session).await else {
        return resource_not_found_page();
    };

    let email = form.email.as_deref().map(str::trim).unwrap_or("");
    if email.is_empty() {
        let (csrf_token, set_cookie) = ensure_csrf_cookie(&state, &headers);
        return render_form(
            &csrf_token,
            &workspace_name,
            Some(BLANK_EMAIL_ERROR),
            set_cookie,
        );
    }

    let invite_id = uuid::Uuid::now_v7();
    let now = state.clock.now();
    let expires_at = now + time::Duration::days(INVITE_TTL_DAYS);

    if let Err(err) = state
        .store
        .insert_invite(
            invite_id,
            admin.workspace_id,
            Some(email),
            admin.user_id,
            expires_at,
        )
        .await
    {
        tracing::error!(%err, "insert_invite (member) failed");
        return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
    }

    let token = match foundry_auth::InviteToken::new(invite_id, expires_at, &state.session_secret) {
        Ok(t) => t,
        Err(err) => {
            tracing::error!(%err, "InviteToken::new (member) failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };
    let invite_url = format!(
        "{}/invites/accept?id={}&sig={}",
        state.public_url.trim_end_matches('/'),
        invite_id,
        urlencoding::encode(&token.signature),
    );

    // Best-effort email — a send failure is non-fatal; the link is still rendered.
    let subject = "You have been invited to a Foundry workspace";
    let body = format!(
        "You have been invited to join the {workspace_name} workspace. Accept the \
         invitation here:\n\n{invite_url}\n\nThis link is valid for 7 days.",
    );
    if let Err(err) = state.email.send(email, subject, &body).await {
        tracing::warn!(%err, "member invite email send failed (link is still valid)");
    }

    let fragment = MemberInviteSent {
        invitee_email: email.to_string(),
        invite_url,
    };
    match fragment.render() {
        Ok(html) => Html(html).into_response(),
        Err(err) => {
            tracing::error!(%err, "render member_invite_sent failed");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
        }
    }
}

// ----------------------------------------------------------------- internals

/// Resolve the signed-in WORKSPACE ADMIN from the session (the issuance authz gate).
/// Returns `Some((user, workspace_name))` only when there IS a session AND the user
/// is an `admin`-role member of their acting workspace; otherwise `None` (→ the
/// SHIPPED non-enumerable uniform 404). Fails closed on any store error.
async fn require_workspace_admin(
    state: &AppState,
    session: &Session,
) -> Option<(SessionUser, String)> {
    let user = match session.get::<SessionUser>(SESSION_KEY_USER_ID).await {
        Ok(maybe_user) => maybe_user?,
        Err(err) => {
            tracing::warn!(error = %err, "session store error resolving workspace admin for /workspace/invites; failing closed (404)");
            return None;
        }
    };
    match state
        .store
        .is_workspace_admin(user.workspace_id, user.user_id)
        .await
    {
        Ok(true) => {}
        Ok(false) => return None,
        Err(err) => {
            tracing::warn!(error = %err, "is_workspace_admin probe failed; failing closed (404)");
            return None;
        }
    }
    let workspace_name = match state.store.resolve_active_workspace(user.user_id).await {
        Ok(Some((_id, name))) => name,
        _ => return None,
    };
    Some((user, workspace_name))
}

/// Render the member-invite form (with an optional inline error), attaching the
/// optional freshly-minted CSRF cookie so a first visitor's POST has a matching pair.
fn render_form(
    csrf_token: &str,
    workspace_name: &str,
    error: Option<&str>,
    set_cookie: Option<String>,
) -> Response {
    let body = MemberInviteForm {
        csrf_token: csrf_token.to_string(),
        workspace_name: workspace_name.to_string(),
        error: error.map(str::to_string),
    }
    .render()
    .expect("member_invite_form.html renders");
    let mut resp = Html(body).into_response();
    if let Some(cookie) = set_cookie {
        if let Ok(value) = HeaderValue::from_str(&cookie) {
            resp.headers_mut().insert(SET_COOKIE, value);
        }
    }
    resp
}

/// Reuse the request's existing double-submit `foundry_csrf` cookie when present,
/// else mint one (mirrors `instance_admin::ensure_csrf_cookie`).
fn ensure_csrf_cookie(state: &AppState, headers: &HeaderMap) -> (String, Option<String>) {
    let existing = headers
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(extract_csrf_cookie);
    if let Some(token) = existing {
        return (token, None);
    }
    let token = generate_token();
    let cookie = build_csrf_cookie(&token, state.session_cookie_secure);
    (token, Some(cookie))
}
