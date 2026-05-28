//! US-06 — sign-in / sign-out / forgot-password handlers.
//!
//! GET  /sign-in           → HTML form (CSRF token in cookie + hidden field)
//! POST /sign-in           → verify password, set session, redirect
//! POST /sign-out          → invalidate server-side session row, redirect
//! GET  /forgot-password   → HTML form
//! POST /forgot-password   → if SMTP configured + user exists, send reset email
//! GET  /                  → minimal protected landing page

use crate::bootstrap::{html_escape, SessionUser};
use crate::csrf::{build_csrf_cookie, generate_token, CSRF_COOKIE_NAME, CSRF_FORM_FIELD};
use crate::session::SESSION_KEY_USER_ID;
use crate::AppState;
use axum::extract::{Form, State};
use axum::http::header::{HeaderMap, HeaderValue, COOKIE, LOCATION, SET_COOKIE};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use secrecy::SecretString;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::time::Duration;
use tokio::sync::OnceCell;
use tower_sessions::Session;

/// NFR-SEC-02: failed-attempt threshold + window + delay.
const BRUTE_FORCE_THRESHOLD: i64 = 5;
const BRUTE_FORCE_WINDOW_MINUTES: i64 = 15;
const BRUTE_FORCE_DELAY: Duration = Duration::from_secs(5);

const GENERIC_SIGNIN_ERROR: &str = "Invalid email or password";

#[derive(Debug, Deserialize)]
pub struct SigninForm {
    pub email: String,
    pub password: String,
    #[serde(rename = "_csrf", default)]
    pub _csrf: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ForgotForm {
    pub email: String,
    #[serde(rename = "_csrf", default)]
    pub _csrf: Option<String>,
}

// ------------------------------------------------------------------ GET /sign-in

pub async fn show_form(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let (token, set_cookie) = ensure_csrf_cookie(&state, &headers);
    let body = render_signin_form(&token, None);
    response_with_optional_cookie(StatusCode::OK, Html(body).into_response(), set_cookie)
}

pub async fn show_forgot_form(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let (token, set_cookie) = ensure_csrf_cookie(&state, &headers);
    let body = render_forgot_form(&token, None);
    response_with_optional_cookie(StatusCode::OK, Html(body).into_response(), set_cookie)
}

// ----------------------------------------------------------------- POST /sign-in

pub async fn submit_signin(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<SigninForm>,
) -> Response {
    let email_lower = form.email.trim().to_lowercase();
    let pwd = SecretString::new(form.password.into());
    let now = state.clock.now();

    let window_start = now - time::Duration::minutes(BRUTE_FORCE_WINDOW_MINUTES);
    let recent_failures = match state
        .store
        .count_recent_failed_signin_attempts(&email_lower, window_start)
        .await
    {
        Ok(c) => c,
        Err(err) => {
            tracing::error!(%err, "count_recent_failed_signin_attempts failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };

    if recent_failures >= BRUTE_FORCE_THRESHOLD {
        // NFR-SEC-02: artificial delay before answering. The mock clock
        // in tests records the request and returns immediately.
        state.clock.sleep(BRUTE_FORCE_DELAY).await;
    }

    // Look up the user and verify the password. We always run the
    // verify step (against a known-bad hash if the user doesn't exist)
    // so timing does not leak whether the email is registered.
    let user_row = match state.store.find_user_by_email(&email_lower).await {
        Ok(u) => u,
        Err(err) => {
            tracing::error!(%err, "find_user_by_email failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };

    let (verified, user) = match user_row {
        Some(u) => {
            let ok = foundry_auth::verify_password(&pwd, &u.password_hash)
                .await
                .unwrap_or(false);
            (ok, Some(u))
        }
        None => {
            // Run verify against a known-bad hash to keep wall-clock
            // similar to the real-user path (constant-time email
            // check per design/auth.md).
            let _ = foundry_auth::verify_password(&pwd, known_bad_hash().await).await;
            (false, None)
        }
    };

    // Record the attempt regardless of outcome.
    if let Err(err) = state
        .store
        .record_signin_attempt(&email_lower, verified, now)
        .await
    {
        tracing::warn!(%err, "record_signin_attempt failed");
    }

    if !verified {
        let (token, set_cookie) = ensure_csrf_cookie(&state, &headers);
        let body = render_signin_form(&token, Some(GENERIC_SIGNIN_ERROR));
        return response_with_optional_cookie(
            StatusCode::UNAUTHORIZED,
            Html(body).into_response(),
            set_cookie,
        );
    }

    let user = user.expect("verified implies user row found");
    let workspace_id = match state.store.first_workspace().await {
        Ok(Some((id, _))) => id,
        Ok(None) => {
            tracing::error!("no workspace exists at sign-in time");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
        Err(err) => {
            tracing::error!(%err, "first_workspace failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };

    if let Err(err) = session
        .insert(
            SESSION_KEY_USER_ID,
            SessionUser {
                user_id: user.id,
                workspace_id,
            },
        )
        .await
    {
        tracing::error!(%err, "session.insert failed");
        return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
    }

    let mut hdrs = HeaderMap::new();
    hdrs.insert(LOCATION, HeaderValue::from_static("/"));
    (StatusCode::SEE_OTHER, hdrs, "").into_response()
}

// ---------------------------------------------------------------- POST /sign-out

pub async fn submit_signout(State(_state): State<AppState>, session: Session) -> Response {
    // tower-sessions `flush()` deletes the server-side row AND removes
    // the cookie on the response.
    if let Err(err) = session.flush().await {
        tracing::warn!(%err, "session.flush failed during sign-out");
    }
    let mut headers = HeaderMap::new();
    headers.insert(LOCATION, HeaderValue::from_static("/sign-in"));
    (StatusCode::SEE_OTHER, headers, "").into_response()
}

// ----------------------------------------------------------- POST /forgot-password

pub async fn submit_forgot(
    State(state): State<AppState>,
    Form(form): Form<ForgotForm>,
) -> Response {
    let email_lower = form.email.trim().to_lowercase();
    let now = state.clock.now();
    let expires_at = now + time::Duration::hours(1);

    if let Ok(Some(user)) = state.store.find_user_by_email(&email_lower).await {
        let raw = generate_token();
        let token_hash = sha256(&raw);
        let token_id = uuid::Uuid::now_v7();
        if let Err(err) = state
            .store
            .insert_reset_token(token_id, user.id, &token_hash, expires_at)
            .await
        {
            tracing::warn!(%err, "insert_reset_token failed");
        }
        let reset_url = format!(
            "{}/reset-password?token={}",
            state.public_url.trim_end_matches('/'),
            urlencoding::encode(&raw),
        );
        let subject = "Reset your Foundry password";
        let body = format!(
            "Someone (hopefully you) asked to reset the password for {email}.\n\n\
             To choose a new password, follow this link (valid for 1 hour):\n\n\
             {reset_url}\n\n\
             If you did not request this, ignore this email.",
            email = email_lower,
        );
        if let Err(err) = state.email.send(&email_lower, subject, &body).await {
            tracing::warn!(%err, "email send for password reset failed");
        }
    }

    // Always respond the same so we don't leak which emails are on file.
    let body = "<!doctype html><html><body>\
                <h1>Check your email</h1>\
                <p>If that email is on file, a reset link has been sent.</p>\
                </body></html>";
    Html(body).into_response()
}

// ------------------------------------------------------------------------ GET /

pub async fn dashboard_root(State(_state): State<AppState>, session: Session) -> Response {
    let user = session
        .get::<SessionUser>(SESSION_KEY_USER_ID)
        .await
        .ok()
        .flatten();
    match user {
        Some(_) => {
            let body = "<!doctype html><html><body>\
                        <h1>Foundry</h1>\
                        <p>You are signed in. Welcome back.</p>\
                        </body></html>";
            Html(body).into_response()
        }
        None => {
            let mut hdrs = HeaderMap::new();
            hdrs.insert(LOCATION, HeaderValue::from_static("/sign-in"));
            (StatusCode::SEE_OTHER, hdrs, "").into_response()
        }
    }
}

// ----------------------------------------------------------------------- helpers

fn sha256(s: &str) -> Vec<u8> {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    h.finalize().to_vec()
}

/// Read the existing CSRF cookie if present; otherwise mint a fresh
/// one. Returns `(token, optional Set-Cookie header value)`.
fn ensure_csrf_cookie(state: &AppState, headers: &HeaderMap) -> (String, Option<String>) {
    let existing = headers
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(crate::csrf::extract_csrf_cookie);
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

fn render_signin_form(csrf_token: &str, error: Option<&str>) -> String {
    let err_html = error
        .map(|e| format!("<p class=\"error\">{}</p>", html_escape(e)))
        .unwrap_or_default();
    let token_html = html_escape(csrf_token);
    format!(
        r#"<!doctype html>
<html><head><title>Sign in to Foundry</title></head>
<body>
<h1>Sign in</h1>
{err_html}
<form method="post" action="/sign-in">
  <input type="hidden" name="{CSRF_FORM_FIELD}" value="{token_html}">
  <label>Email <input type="email" name="email" required></label>
  <label>Password <input type="password" name="password" required></label>
  <button type="submit">Sign in</button>
</form>
<p><a href="/forgot-password">Forgot password?</a></p>
</body></html>"#,
    )
}

fn render_forgot_form(csrf_token: &str, _error: Option<&str>) -> String {
    let token_html = html_escape(csrf_token);
    format!(
        r#"<!doctype html>
<html><head><title>Forgot password</title></head>
<body>
<h1>Forgot password</h1>
<form method="post" action="/forgot-password">
  <input type="hidden" name="{CSRF_FORM_FIELD}" value="{token_html}">
  <label>Email <input type="email" name="email" required></label>
  <button type="submit">Send reset link</button>
</form>
</body></html>"#,
    )
}

/// A PHC-encoded argon2id hash of a process-unique throwaway password.
/// Generated once per process so the verify path on an unknown email
/// burns the same CPU/wall-clock as the real-user path (constant-time
/// email check per design/auth.md). The hash matches the same OWASP
/// parameters production uses, because [`foundry_auth::hash_password`]
/// hardcodes them.
///
/// `tokio::sync::OnceCell` (not `std::sync::OnceLock`) because
/// `hash_password` is `async` — the initializer needs to `.await`. The
/// cache still pays the ~80–300ms argon2 cost exactly once per process.
async fn known_bad_hash() -> &'static str {
    static CACHE: OnceCell<String> = OnceCell::const_new();
    CACHE
        .get_or_init(|| async {
            let throwaway = SecretString::new(
                "this-password-never-matches-a-real-user-account"
                    .to_string()
                    .into(),
            );
            foundry_auth::hash_password(&throwaway)
                .await
                .expect("hash_password for known-bad cache")
        })
        .await
}

// Suppress unused-import lint when CSRF cookie name not referenced elsewhere.
#[allow(dead_code)]
const _: &str = CSRF_COOKIE_NAME;
