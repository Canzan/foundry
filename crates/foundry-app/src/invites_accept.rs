//! invite-accept-flow — the PUBLIC `/invites/accept` claim-your-account vertical.
//!
//! A NEW PUBLIC web driving adapter (architecture.md C4-L3) turning the emitted
//! (today DEAD) `/invites/accept?id=…&sig=…` link into a live flow: verify the
//! signed `InviteToken` → render a set-password form naming the workspace →
//! atomically consume the single-use invite + write the argon2id password in ONE
//! tx (the NEW `Store::set_first_admin_password_and_consume`, ADR-001) → establish
//! a session → 303 onto the workspace (auto sign-in, no separate login step).
//!
//! Mounted in `build_router` on the PUBLIC layer (the invitee is NOT signed in
//! yet) UNDER the SHIPPED `session_layer` + `csrf::csrf_middleware` — alongside
//! `/sign-in` and `/bootstrap`, NOT behind the instance-admin gate.
//!
//! Step 01-01 implements the WALKING SKELETON only: GET renders the form for a
//! live invite; POST runs the min-12 policy (ADR-004), the one-TX consume+write,
//! the session establish, and the 303. The uniform non-enumerable refusal
//! (expired / used / tampered / unknown — ADR-002) and the inline recovery paths
//! (weak / mismatch — US-03) are LATER steps; this step renders the SHIPPED
//! `resource_not_found_page()` for any non-live invite (the thinnest refusal that
//! keeps the GET non-committal) and re-renders the form inline on a policy/confirm
//! failure WITHOUT opening the consume TX.
//!
//! LAYER-1e (ADR-005 / D7): this handler scopes nothing by a request-parsed
//! workspace id — it consumes by invite id and lands via the SHIPPED
//! `resolve_active_workspace` membership seam (like `signin`), so it must NOT trip
//! the tenant-scoping detector (confirmed at DELIVER via `cargo xtask check-arch`).

use crate::bootstrap::{resource_not_found_page, SessionUser};
use crate::csrf::{build_csrf_cookie, extract_csrf_cookie, generate_token};
use crate::session::SESSION_KEY_USER_ID;
use crate::views::InviteAcceptPage;
use crate::AppState;
use askama::Template;
use axum::extract::{Form, Query, State};
use axum::http::header::{HeaderMap, HeaderValue, COOKIE, LOCATION, SET_COOKIE};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use foundry_store::ConsumeOutcome;
use secrecy::SecretString;
use serde::Deserialize;
use tower_sessions::Session;

const MISMATCH_ERROR: &str = "The passwords do not match.";

#[derive(Debug, Deserialize)]
pub struct AcceptQuery {
    pub id: Option<String>,
    pub sig: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AcceptForm {
    pub id: String,
    pub sig: String,
    pub password: String,
    pub confirm: String,
    #[serde(rename = "_csrf", default)]
    pub _csrf: Option<String>,
}

// ------------------------------------------------------------- GET /invites/accept

/// Verify the signature + advisory liveness (NON-COMMITTAL, D6), mint the CSRF
/// cookie, and render the set-password form naming the workspace. Any failed
/// check renders the uniform refusal — no mutation on GET.
pub async fn show_accept_form(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AcceptQuery>,
) -> Response {
    let (Some(id_raw), Some(sig)) = (query.id, query.sig) else {
        return resource_not_found_page();
    };
    let Ok(invite_id) = uuid::Uuid::parse_str(id_raw.trim()) else {
        return resource_not_found_page();
    };

    let now = state.clock.now();
    let view = match state.store.invite_accept_view(invite_id).await {
        Ok(Some(v)) => v,
        Ok(None) => return resource_not_found_page(),
        Err(err) => {
            tracing::error!(%err, %invite_id, "invite_accept_view failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };

    // Verify the HMAC against the recovered expires_at (the tamper oracle), then
    // the advisory liveness (unused + not expired). Any failure → uniform refusal.
    if !invite_is_acceptable(&state, invite_id, view.expires_at, &sig, view.used_at, now) {
        return resource_not_found_page();
    }

    let (token, set_cookie) = ensure_csrf_cookie(&state, &headers);
    let body = render_form(&token, invite_id, &sig, &view.workspace_name, None);
    response_with_optional_cookie(StatusCode::OK, Html(body).into_response(), set_cookie)
}

// ------------------------------------------------------------ POST /invites/accept

/// Re-verify the token, run the min-12 policy + confirm match (BEFORE any consume
/// — a rejected password leaves the invite live), hash the password, run the
/// one-TX consume+write, establish a session, and 303 → `/`. A 0-rows consume
/// (unknown / used / expired) → uniform refusal.
pub async fn submit_accept(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<AcceptForm>,
) -> Response {
    let Ok(invite_id) = uuid::Uuid::parse_str(form.id.trim()) else {
        return resource_not_found_page();
    };

    let now = state.clock.now();
    let view = match state.store.invite_accept_view(invite_id).await {
        Ok(Some(v)) => v,
        Ok(None) => return resource_not_found_page(),
        Err(err) => {
            tracing::error!(%err, %invite_id, "invite_accept_view failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };

    // Re-verify the HMAC + advisory liveness (defense-in-depth; the consume guard
    // is still authoritative). A tampered / dead link → uniform refusal.
    if !invite_is_acceptable(
        &state,
        invite_id,
        view.expires_at,
        &form.sig,
        view.used_at,
        now,
    ) {
        return resource_not_found_page();
    }

    let password = SecretString::new(form.password.clone().into());

    // Policy + confirm match run BEFORE the consume TX opens (US-03): a rejected
    // password re-renders the form inline with the invite UNTOUCHED.
    if form.password != form.confirm {
        return re_render_with_error(
            &state,
            &headers,
            invite_id,
            &form.sig,
            &view.workspace_name,
            MISMATCH_ERROR,
        );
    }
    if let Err(err) = foundry_auth::check_password_policy(&password) {
        return re_render_with_error(
            &state,
            &headers,
            invite_id,
            &form.sig,
            &view.workspace_name,
            &err.to_string(),
        );
    }

    let password_hash = match foundry_auth::hash_password(&password).await {
        Ok(h) => h,
        Err(err) => {
            tracing::error!(%err, %invite_id, "hash_password failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };

    let outcome = match state
        .store
        .set_first_admin_password_and_consume(invite_id, &password_hash, now)
        .await
    {
        Ok(o) => o,
        Err(err) => {
            tracing::error!(%err, %invite_id, "set_first_admin_password_and_consume failed");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };

    let ConsumeOutcome::Consumed {
        workspace_id,
        user_id,
    } = outcome
    else {
        // 0 rows — lost the race / already used / expired in the GET→POST window.
        return resource_not_found_page();
    };

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
        tracing::error!(%err, %invite_id, "session.insert failed");
        return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
    }

    let mut hdrs = HeaderMap::new();
    hdrs.insert(LOCATION, HeaderValue::from_static("/"));
    (StatusCode::SEE_OTHER, hdrs, "").into_response()
}

// ----------------------------------------------------------------------- helpers

/// True iff the invite's signature verifies against the recovered `expires_at`
/// AND the invite is advisory-live (unused + not expired). The HMAC is the tamper
/// oracle (rejects tampered/extended links); the liveness is non-committal on GET
/// and re-checked (authoritatively) by the consume guard on POST.
fn invite_is_acceptable(
    state: &AppState,
    invite_id: uuid::Uuid,
    expires_at: time::OffsetDateTime,
    sig: &str,
    used_at: Option<time::OffsetDateTime>,
    now: time::OffsetDateTime,
) -> bool {
    if foundry_auth::InviteToken::verify(invite_id, expires_at, sig, &state.session_secret).is_err()
    {
        return false;
    }
    used_at.is_none() && expires_at > now
}

fn render_form(
    csrf_token: &str,
    invite_id: uuid::Uuid,
    sig: &str,
    workspace_name: &str,
    error: Option<&str>,
) -> String {
    InviteAcceptPage {
        csrf_token: csrf_token.to_string(),
        invite_id: invite_id.to_string(),
        sig: sig.to_string(),
        workspace_name: workspace_name.to_string(),
        error: error.map(str::to_string),
    }
    .render()
    .expect("invite_accept.html renders")
}

/// Re-render the set-password form inline with an error (US-03 recovery). The
/// invite is left UNTOUCHED (the consume TX never opened). 200 OK so the form is
/// re-presented in place.
fn re_render_with_error(
    state: &AppState,
    headers: &HeaderMap,
    invite_id: uuid::Uuid,
    sig: &str,
    workspace_name: &str,
    error: &str,
) -> Response {
    let (token, set_cookie) = ensure_csrf_cookie(state, headers);
    let body = render_form(&token, invite_id, sig, workspace_name, Some(error));
    response_with_optional_cookie(StatusCode::OK, Html(body).into_response(), set_cookie)
}

/// Reuse the request's existing double-submit `foundry_csrf` cookie when present,
/// else mint one (mirrors `signin::ensure_csrf_cookie` — the public-route seam,
/// D4/adr-003). The token rides into the form's hidden `_csrf` field; the optional
/// cookie is attached so a fresh visitor's first POST has a matching pair.
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
