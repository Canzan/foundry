//! `/reset-password` — redeeming the link `/forgot-password` emails.
//!
//! `signin::submit_forgot` has always minted a single-use token, stored its
//! SHA-256 in `reset_tokens`, and emailed `{public_url}/reset-password?token=…`.
//! Nothing served that path: the route did not exist, the store had an insert
//! and no read, and every emailed link 404'd. The flow was half a feature — the
//! half that cannot be noticed from the sending side, because `/forgot-password`
//! answers "Check your email" whether or not anything downstream works.
//!
//! Mounted on the PUBLIC layer (the user is by definition signed out) UNDER the
//! shipped `session_layer` + `csrf::csrf_middleware`, exactly like `/sign-in`,
//! `/bootstrap` and `/invites/accept`.
//!
//! Deliberately does NOT auto-sign-in on success, where `invites_accept` does.
//! An invite is a first credential and signing in completes the journey; a reset
//! is a recovery, often triggered because control of the account is in doubt, so
//! the new password is proved once at `/sign-in` before any session exists.

use crate::bootstrap::invalid_page;
use crate::csrf::{build_csrf_cookie, extract_csrf_cookie, generate_token};
use crate::views::ResetPasswordPage;
use crate::AppState;
use askama::Template;
use axum::extract::{Form, Query, State};
use axum::http::header::{HeaderMap, HeaderValue, COOKIE, LOCATION, SET_COOKIE};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use foundry_store::ResetOutcome;
use secrecy::SecretString;
use serde::Deserialize;
use sha2::{Digest, Sha256};

const MISMATCH_ERROR: &str = "The passwords do not match.";

#[derive(Debug, Deserialize)]
pub struct ResetQuery {
    pub token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResetForm {
    pub token: String,
    pub password: String,
    pub confirm: String,
    #[serde(rename = "_csrf", default)]
    pub _csrf: Option<String>,
}

// ------------------------------------------------------------ GET /reset-password

/// Advisory-check the token (NON-COMMITTAL — a GET never burns it), mint the CSRF
/// cookie, and render the set-password form. Any dead link renders the uniform
/// refusal.
pub async fn show_reset_form(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ResetQuery>,
) -> Response {
    let Some(raw) = query.token.filter(|t| !t.trim().is_empty()) else {
        return reset_refusal_page();
    };

    match state
        .store
        .find_live_reset_token(&sha256(raw.trim()), state.clock.now())
        .await
    {
        Ok(Some(_)) => {}
        Ok(None) => return reset_refusal_page(),
        Err(err) => {
            // Keyed on nothing the caller supplied: the token is a credential and
            // never reaches a log line, here or anywhere below.
            tracing::error!(%err, "find_live_reset_token failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    }

    let (csrf, set_cookie) = ensure_csrf_cookie(&state, &headers);
    let body = render_form(&csrf, raw.trim(), None);
    response_with_optional_cookie(StatusCode::OK, Html(body).into_response(), set_cookie)
}

// ----------------------------------------------------------- POST /reset-password

/// Validate the new password FIRST (mismatch, then the min-12 policy), so a
/// rejected password re-renders the form with the token still live; then consume
/// the token and write the credential in one tx; then notify and 303 → `/sign-in`.
pub async fn submit_reset(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<ResetForm>,
) -> Response {
    let raw = form.token.trim().to_string();
    if raw.is_empty() {
        return reset_refusal_page();
    }
    let now = state.clock.now();

    // Step 1 — policy + confirm match BEFORE the consume tx opens. A user who
    // fumbles the confirm field must not lose the link and have to request a new
    // one; nothing is written and the token stays live.
    let password = SecretString::new(form.password.clone().into());
    if form.password != form.confirm {
        return re_render_with_error(&state, &headers, &raw, MISMATCH_ERROR);
    }
    if let Err(err) = foundry_auth::check_password_policy(&password) {
        return re_render_with_error(&state, &headers, &raw, &err.to_string());
    }
    let password_hash = match foundry_auth::hash_password(&password).await {
        Ok(hash) => hash,
        Err(err) => {
            tracing::error!(%err, "hash_password failed during password reset");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };

    // Step 2 — the authoritative single-use consume + credential write, one tx.
    let user_id = match state
        .store
        .reset_password_and_consume(&sha256(&raw), &password_hash, now)
        .await
    {
        Ok(ResetOutcome::Consumed { user_id }) => user_id,
        Ok(ResetOutcome::Refused) => return reset_refusal_page(),
        Err(err) => {
            tracing::error!(%err, "reset_password_and_consume failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };

    // Step 3 — tell the account holder their password changed. MANDATORY event
    // (never suppressed): this is the message that surfaces a reset the owner did
    // not perform, so it is the one notification that must not be opt-out.
    // Best-effort and non-fatal (NFR-5): the password IS already changed, and
    // failing the response would leave the user believing it was not.
    if let Ok(Some(email)) = state.store.find_user_email_by_id(user_id).await {
        let notification = crate::notify::Notification {
            event: crate::notify::NotificationEvent::PasswordChanged,
            recipient: email,
            subject: "Your Foundry password was changed".to_string(),
            body: "The password for your Foundry account was just reset. If this was \
                   not you, contact your instance administrator immediately."
                .to_string(),
            workspace_id: None,
        };
        state.notifier.notify(&notification).await;
    }

    let mut hdrs = HeaderMap::new();
    hdrs.insert(LOCATION, HeaderValue::from_static("/sign-in"));
    (StatusCode::SEE_OTHER, hdrs, "").into_response()
}

// ----------------------------------------------------------------------- helpers

/// The UNIFORM non-enumerable reset refusal. Unknown, already-used, expired,
/// mistyped, or lost-the-race all collapse to THIS byte-identical response.
///
/// 200 OK, not 404, for the same reason `invite_refusal_page` chose it: a status
/// that varies by reason is itself an oracle, and "this page exists, your link is
/// dead" is the honest reading. It also fixes what the operator actually hit — a
/// bare 404 that gave no hint the link had simply aged out, and was
/// indistinguishable from the route being missing, which is what it was.
fn reset_refusal_page() -> Response {
    invalid_page(
        StatusCode::OK,
        "This reset link is no longer valid",
        "It may have expired, already been used, or been mistyped. \
         Request a new one from the sign-in page.",
    )
}

fn sha256(s: &str) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    h.finalize().to_vec()
}

fn render_form(csrf_token: &str, token: &str, error: Option<&str>) -> String {
    ResetPasswordPage {
        csrf_token: csrf_token.to_string(),
        token: token.to_string(),
        error: error.map(str::to_string),
    }
    .render()
    .expect("reset_password.html renders")
}

/// Re-render inline with an error. The token is left UNTOUCHED — the consume tx
/// never opened — so the same link still works on the next attempt.
fn re_render_with_error(
    state: &AppState,
    headers: &HeaderMap,
    token: &str,
    error: &str,
) -> Response {
    let (csrf, set_cookie) = ensure_csrf_cookie(state, headers);
    let body = render_form(&csrf, token, Some(error));
    response_with_optional_cookie(StatusCode::OK, Html(body).into_response(), set_cookie)
}

/// Reuse the request's existing double-submit `foundry_csrf` cookie when present,
/// else mint one (mirrors `invites_accept::ensure_csrf_cookie` — the public-route
/// seam), so a fresh visitor's first POST has a matching pair.
fn ensure_csrf_cookie(state: &AppState, headers: &HeaderMap) -> (String, Option<String>) {
    let existing = headers
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(extract_csrf_cookie);
    if let Some(token) = existing {
        (token, None)
    } else {
        let token = generate_token();
        let cookie = build_csrf_cookie(&token, state.session_cookie_secure);
        (token, Some(cookie))
    }
}

fn response_with_optional_cookie(
    status: StatusCode,
    body: Response,
    set_cookie: Option<String>,
) -> Response {
    let (mut parts, body) = body.into_parts();
    parts.status = status;
    if let Some(cookie) = set_cookie {
        if let Ok(v) = HeaderValue::from_str(&cookie) {
            parts.headers.insert(SET_COOKIE, v);
        }
    }
    Response::from_parts(parts, body)
}
