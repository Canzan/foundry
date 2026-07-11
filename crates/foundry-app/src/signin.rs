//! US-06 — sign-in / sign-out / forgot-password handlers.
//!
//! GET  /sign-in           → HTML form (CSRF token in cookie + hidden field)
//! POST /sign-in           → verify password, set session, redirect
//! POST /sign-out          → invalidate server-side session row, redirect
//! GET  /forgot-password   → HTML form
//! POST /forgot-password   → if SMTP configured + user exists, send reset email
//! GET  /                  → minimal protected landing page

use crate::bootstrap::SessionUser;
use crate::csrf::{build_csrf_cookie, generate_token, CSRF_COOKIE_NAME};
use crate::session::SESSION_KEY_USER_ID;
use crate::AppState;
use askama::Template;
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

#[derive(Debug, Deserialize)]
pub struct ChangePasswordForm {
    /// The signed-in owner's CURRENT password — reauthenticates the request so a
    /// hijacked session alone cannot rotate the credential (D2).
    #[serde(default)]
    pub current_password: String,
    pub new_password: String,
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
    // ADR-005: the session's ACTIVE workspace is resolved by the member's
    // `workspace_memberships`, NOT by the global `first_workspace()`. Under two
    // coexisting workspaces (slice 1), `first_workspace()`'s unordered `LIMIT 1`
    // would scope a member of one tenant to an arbitrary other; membership
    // resolution scopes them to their own. A single-membership user
    // auto-resolves to their one workspace; a user with NO membership FAILS
    // CLOSED (we refuse — never default to a tenant they do not belong to). The
    // multi-membership selector + switcher (step 02-05) layer onto this same
    // seam later.
    let workspace_id = match state.store.resolve_active_workspace(user.id).await {
        Ok(Some((id, _))) => id,
        Ok(None) => {
            // Fail closed: a verified user who belongs to no workspace cannot be
            // given an active tenant. Refuse rather than default.
            tracing::warn!(user_id = %user.id, "sign-in: user belongs to no workspace; refusing");
            let (token, set_cookie) = ensure_csrf_cookie(&state, &headers);
            let body = render_signin_form(&token, Some(GENERIC_SIGNIN_ERROR));
            return response_with_optional_cookie(
                StatusCode::UNAUTHORIZED,
                Html(body).into_response(),
                set_cookie,
            );
        }
        Err(err) => {
            tracing::error!(%err, "resolve_active_workspace failed");
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
        // Best-effort, non-fatal delivery through the config-selected providers
        // (NFR-5). `notify()` is infallible: a delivery problem is contained and
        // never fails this request (we always respond the same below).
        let notification = crate::notify::Notification {
            event: crate::notify::NotificationEvent::PasswordReset,
            recipient: email_lower.clone(),
            subject: subject.to_string(),
            body,
        };
        state.notifier.notify(&notification).await;
    }

    // Always respond the same so we don't leak which emails are on file.
    // Templated via the shared base layout (Phase-4 FIX 3) so it links the
    // vendored /static stylesheet like every other surface; the confirmation
    // copy is preserved byte-identically (NFR-WEBB-COMPAT-02).
    let body = crate::views::ForgotSentPage
        .render()
        .expect("forgot_sent.html renders");
    Html(body).into_response()
}

// ---------------------------------------------------------- POST /account/password

/// Change the signed-in account owner's password (notification-delivery-providers
/// US-06 — the `password_changed` trigger). Mirrors the `submit_forgot` emit idiom:
/// perform the real state change (write the new `password_hash`), then emit ONE
/// structured [`crate::notify::Notification`] best-effort through the config-selected
/// providers (`notify()` is infallible — a delivery problem is contained and never
/// fails this request, NFR-5). CSRF is enforced by the surrounding `csrf_middleware`.
/// A signed-out caller gets the SHIPPED non-enumerable uniform 404.
pub async fn submit_change_password(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<ChangePasswordForm>,
) -> Response {
    let user = match session.get::<SessionUser>(SESSION_KEY_USER_ID).await {
        Ok(Some(user)) => user,
        _ => return crate::bootstrap::resource_not_found_page(),
    };

    // D3: enforce the SHIPPED length-first password policy (ADR-004 / NFR-4,
    // min-12 — the same policy the invite-accept set-password flow uses) on the
    // NEW password BEFORE any state change or notification. A too-weak (or empty)
    // password is rejected inline; nothing is written and nobody is notified.
    let new_password = SecretString::new(form.new_password.trim().to_string().into());
    if let Err(err) = foundry_auth::check_password_policy(&new_password) {
        return (StatusCode::BAD_REQUEST, err.to_string()).into_response();
    }

    // D2: reauthenticate — verify the CURRENT password against the stored hash
    // using the SAME verifier the sign-in flow uses, so a hijacked session alone
    // cannot rotate the credential. A valid session implies the user row exists;
    // a missing/failed hash lookup is a data-consistency fault ⇒ 500 (D8 posture).
    let stored_hash = match state
        .store
        .find_user_password_hash_by_id(user.user_id)
        .await
    {
        Ok(Some(hash)) => hash,
        Ok(None) => {
            tracing::error!(user_id = %user.user_id, "change-password: no user row for a valid session");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
        Err(err) => {
            tracing::error!(%err, "find_user_password_hash_by_id failed during change-password");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };
    let current = SecretString::new(form.current_password.clone().into());
    let reauthenticated = foundry_auth::verify_password(&current, &stored_hash)
        .await
        .unwrap_or(false);
    if !reauthenticated {
        // Wrong current password: refuse without changing anything and WITHOUT
        // emitting `password_changed`.
        return (StatusCode::UNAUTHORIZED, "current password is incorrect").into_response();
    }

    // D8: the session is valid, so the user EXISTS. A missing/failed email lookup
    // is therefore a data-consistency fault (500), not a "not found" (404).
    let owner_email = match state.store.find_user_email_by_id(user.user_id).await {
        Ok(Some(email)) => email,
        Ok(None) => {
            tracing::error!(user_id = %user.user_id, "change-password: no email for a valid session");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
        Err(err) => {
            tracing::error!(%err, "find_user_email_by_id failed during change-password");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };

    let hash = match foundry_auth::hash_password(&new_password).await {
        Ok(hash) => hash,
        Err(err) => {
            tracing::error!(%err, "hash_password failed during change-password");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };
    if let Err(err) = state.store.update_user_password(user.user_id, &hash).await {
        tracing::error!(%err, "update_user_password failed");
        return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
    }

    // Best-effort, non-fatal delivery through the config-selected providers (NFR-5).
    let notification = crate::notify::Notification {
        event: crate::notify::NotificationEvent::PasswordChanged,
        recipient: owner_email,
        subject: "Your Foundry password was changed".to_string(),
        body: "The password for your Foundry account was just changed. If this was \
             not you, contact your workspace administrator immediately."
            .to_string(),
    };
    state.notifier.notify(&notification).await;

    (StatusCode::OK, "password changed").into_response()
}

// ------------------------------------------------------------------------ GET /

pub async fn dashboard_root(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
) -> Response {
    let user = session
        .get::<SessionUser>(SESSION_KEY_USER_ID)
        .await
        .ok()
        .flatten();
    match user {
        Some(u) => {
            // US-R04 / US-01: the signed-in landing renders through the shared
            // base layout. Keeps `<h1>Foundry</h1>`, greets the user by name +
            // names the acting workspace (US-01), and lists the acting
            // workspace's projects — all scoped by the SESSION user_id /
            // workspace_id (never a path/query id — ADR-002).
            //
            // D1 (AC-01.4): the greeting degrades to a NEUTRAL fallback and the
            // page still renders 200 if the identity lookup errors or the row is
            // gone — it never 500s. Same graceful-degradation posture as the
            // project-list load below.
            let greeting = state
                .store
                .dashboard_greeting(u.user_id, u.workspace_id)
                .await
                .unwrap_or_else(|err| {
                    tracing::error!(%err, "dashboard: dashboard_greeting failed");
                    None
                });
            let (display_name, workspace_name) = greeting_or_neutral(greeting);
            let projects: Vec<crate::views::ProjectLink> = state
                .store
                .list_projects_for_workspace(u.workspace_id)
                .await
                .unwrap_or_else(|err| {
                    tracing::error!(%err, "dashboard: list_projects_for_workspace failed");
                    Vec::new()
                })
                .into_iter()
                .map(
                    |(team_slug, project_slug, name, key_prefix)| crate::views::ProjectLink {
                        team_slug,
                        project_slug,
                        name,
                        key_prefix,
                    },
                )
                .collect();
            // US-03: the instance-admin link renders only for an instance
            // super-admin, scoped by the SESSION user_id (never a path/query id —
            // ADR-002). Fail-closed: on lookup error we default to `false` so the
            // link is ABSENT (never surface an admin affordance we could not
            // verify) — same graceful-degradation posture as the loads above.
            let is_instance_admin = state
                .store
                .is_instance_admin(u.user_id)
                .await
                .unwrap_or_else(|err| {
                    tracing::error!(%err, "dashboard: is_instance_admin failed");
                    false
                });
            // US-02 (D2): the sign-out control POSTs to `/sign-out` with a valid
            // double-submit token, so `/` must mint a CSRF cookie and render the
            // matching hidden `_csrf` — the response becomes `(SET_COOKIE, Html)`,
            // mirroring `admin_tokens::show_index`. An existing cookie is reused;
            // otherwise a fresh one is minted + Set-Cookie'd.
            let (csrf, set_cookie) = ensure_csrf_cookie(&state, &headers);
            // navigation-bar-linear-ui (US-01): assemble the shared sidebar
            // carrier ONCE from the identity values already resolved above. The
            // dashboard IS the `Home` section; `board_href` is the ADR-003
            // first-project deep-link (step 04-02) — resolved by REUSING the
            // `projects` list already loaded above (its `ORDER BY p.name` first row
            // is the deterministic default board), so no extra query is issued.
            // Zero projects → `/` (this very dashboard, whose empty-state hosts the
            // create-first-project affordance).
            let board_href = crate::nav::board_href_for_first_project(projects.first().map(
                |p: &crate::views::ProjectLink| (p.team_slug.as_str(), p.project_slug.as_str()),
            ));
            let nav = crate::nav::NavContext::for_page(
                workspace_name.clone(),
                display_name.clone(),
                is_instance_admin,
                csrf.clone(),
                crate::nav::NavSection::Home,
                board_href,
            );
            let body = crate::views::DashboardRoot {
                display_name,
                workspace_name,
                projects,
                is_instance_admin,
                csrf,
                nav,
            }
            .render()
            .expect("dashboard_root.html renders");
            response_with_optional_cookie(StatusCode::OK, Html(body).into_response(), set_cookie)
        }
        None => {
            let mut hdrs = HeaderMap::new();
            hdrs.insert(LOCATION, HeaderValue::from_static("/sign-in"));
            (StatusCode::SEE_OTHER, hdrs, "").into_response()
        }
    }
}

// ----------------------------------------------------------------------- helpers

/// US-01 / AC-01.4 (D1): the neutral fallback greeting shown when the identity
/// lookup yields no row (a stale session) or errors. Rendered into the same
/// "Welcome back, {name}" / "Workspace: {name}" slots so the page still reads
/// 200 with a sensible, non-personalized greeting instead of 500.
const NEUTRAL_DISPLAY_NAME: &str = "there";
const NEUTRAL_WORKSPACE_NAME: &str = "your workspace";

/// Map the greeting store read to the `(display_name, workspace_name)` the view
/// renders: the resolved pair when present, else the neutral fallback (D1). The
/// handler pre-maps a query `Err` to `None`, so this covers both the missing-row
/// and the failed-lookup degradation paths.
fn greeting_or_neutral(loaded: Option<(String, String)>) -> (String, String) {
    match loaded {
        Some(pair) => pair,
        None => (
            NEUTRAL_DISPLAY_NAME.to_string(),
            NEUTRAL_WORKSPACE_NAME.to_string(),
        ),
    }
}

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

/// Render the sign-in page from the shared base layout (US-B04). Selector-and-
/// substring-identical to the previous bare-`<head>` `format!`: same hidden
/// `_csrf` field, `method="post"` `action="/sign-in"`, and the non-enumerable
/// `GENERIC_SIGNIN_ERROR` copy in the `.error` slot — now wrapped by `base.html`
/// so it links the vendored `/static` stylesheet. Auth logic UNCHANGED.
fn render_signin_form(csrf_token: &str, error: Option<&str>) -> String {
    crate::views::SigninPage {
        csrf_token: csrf_token.to_string(),
        error: error.map(str::to_string),
    }
    .render()
    .expect("signin.html renders")
}

/// Render the forgot-password page from the shared base layout (US-B04).
/// Selector-and-substring-identical to the previous form (hidden `_csrf`,
/// `method="post"` `action="/forgot-password"`); now linked to the vendored
/// `/static` stylesheet via `base.html`.
fn render_forgot_form(csrf_token: &str, _error: Option<&str>) -> String {
    crate::views::ForgotPage {
        csrf_token: csrf_token.to_string(),
    }
    .render()
    .expect("forgot.html renders")
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

#[cfg(test)]
mod tests {
    use super::*;

    /// US-01 / AC-01.4 (D1): when the greeting lookup yields nothing — the
    /// missing-row case AND (via the handler's `Err`→`None` pre-map) the
    /// failed-lookup case — the dashboard renders a NEUTRAL greeting rather than
    /// personalized copy, so the page stays 200 instead of 500.
    ///
    /// This covers the "greeting degrades to 200 if identity cannot be loaded"
    /// acceptance scenario, which stays `@pending`: the in-process harness has no
    /// clean seam to force the greeting query to fail mid-request, so the
    /// degradation contract is pinned here at the handler's fallback seam.
    #[test]
    fn greeting_degrades_to_neutral_when_identity_absent() {
        assert_eq!(
            greeting_or_neutral(None),
            ("there".to_string(), "your workspace".to_string()),
            "an absent/failed identity lookup must yield the neutral fallback greeting"
        );
    }

    /// The happy path: a resolved pair is rendered verbatim (auto-escaping in the
    /// template handles markup — AC-01.3).
    #[test]
    fn greeting_uses_resolved_pair_when_present() {
        assert_eq!(
            greeting_or_neutral(Some(("Ada Lovelace".to_string(), "Acme".to_string()))),
            ("Ada Lovelace".to_string(), "Acme".to_string()),
            "a resolved identity must be greeted by its own name + workspace"
        );
    }
}
