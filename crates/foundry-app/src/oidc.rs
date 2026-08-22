//! Federated sign-in: `GET /auth/oidc/start` and `GET /auth/oidc/callback`.
//!
//! ADDITIVE. The password door in `signin.rs` is untouched and remains the
//! break-glass route — Keycloak, LLDAP and foundry share a cluster, so an
//! SSO-only tracker is unreachable exactly when the operator most needs the issue
//! describing how to fix it.
//!
//! Both handlers end in `signin::establish_session`, the SAME seam the password
//! flow uses, so a federated session is indistinguishable downstream and the
//! fail-closed no-workspace branch cannot drift between the two doors.
//!
//! NON-ENUMERABLE. Every refusal returns exactly what a wrong password returns
//! (`GENERIC_SIGNIN_ERROR`, 401). The callback is publicly reachable, so a
//! specific message would make foundry an account-existence oracle for the whole
//! Keycloak realm.
//!
//! THE ONE-TIME CHALLENGE rides in a signed cookie, not a pre-auth session row:
//! `/auth/oidc/start` is reachable signed-out, so a session row per click would be
//! an unauthenticated unbounded INSERT on a public endpoint. The cookie is
//! stateless, short-lived, and signed with the shipped `foundry_auth::sign` HMAC
//! over SESSION_SECRET — the same primitive `InviteToken` and `UnsubscribeToken`
//! already use (ADR-OIDC-002).

use crate::signin::{
    ensure_csrf_cookie, establish_session, render_signin_form, response_with_optional_cookie,
    GENERIC_SIGNIN_ERROR,
};
use crate::AppState;
use axum::extract::{Query, State};
use axum::http::header::{HeaderMap, SET_COOKIE};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use base64::Engine as _;
use foundry_oidc::AuthRequest;
use serde::Deserialize;
use tower_sessions::Session;

/// Where the sign-in page's control points. Moving this moves the redirect URI
/// registered with Keycloak, in the same change.
pub const START_PATH: &str = "/auth/oidc/start";
pub const CALLBACK_PATH: &str = "/auth/oidc/callback";

const CHALLENGE_COOKIE: &str = "foundry_oidc";
/// Long enough for a human to authenticate, short enough that an outstanding
/// challenge is not a durable credential. It cannot be revoked server-side (the
/// cost of statelessness), so it expires quickly instead.
const CHALLENGE_TTL_SECONDS: i64 = 600;

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub state: String,
}

/// The generic refusal. One function, so the federated and password paths
/// physically cannot diverge on refusal shape.
fn refuse(state: &AppState, headers: &HeaderMap, why: &str) -> Response {
    tracing::info!(reason = %why, "oidc sign-in refused");
    let (token, set_cookie) = ensure_csrf_cookie(state, headers);
    let body = render_signin_form(&token, Some(GENERIC_SIGNIN_ERROR));
    let mut resp = response_with_optional_cookie(
        StatusCode::UNAUTHORIZED,
        Html(body).into_response(),
        set_cookie,
    );
    // Clear the challenge on every refusal as well as on success: single-use.
    if let Ok(v) = clear_cookie(state).parse() {
        resp.headers_mut().append(SET_COOKIE, v);
    }
    resp
}

fn cookie_attrs(state: &AppState) -> String {
    let secure = if state.session_cookie_secure {
        "; Secure"
    } else {
        ""
    };
    format!("; Path=/; HttpOnly; SameSite=Lax{secure}")
}

fn clear_cookie(state: &AppState) -> String {
    format!(
        "{CHALLENGE_COOKIE}={}",
        format_args!("; Max-Age=0{}", cookie_attrs(state))
    )
}

/// `<base64url(json)>.<hmac>` — the payload is not secret (it is the client's own
/// challenge), but it must not be client-CHOSEN, which is what the signature buys.
fn seal(state: &AppState, req: &AuthRequest) -> Option<String> {
    let json = serde_json::json!({
        "state": req.state,
        "nonce": req.nonce,
        "verifier": req.code_verifier,
    })
    .to_string();
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(json.as_bytes());
    let sig = foundry_auth::sign(&state.session_secret, payload.as_bytes()).ok()?;
    Some(format!("{payload}.{sig}"))
}

fn unseal(state: &AppState, raw: &str) -> Option<AuthRequest> {
    let (payload, sig) = raw.split_once('.')?;
    foundry_auth::verify(&state.session_secret, payload.as_bytes(), sig).ok()?;
    let json = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()?;
    let v: serde_json::Value = serde_json::from_slice(&json).ok()?;
    Some(AuthRequest {
        state: v.get("state")?.as_str()?.to_string(),
        nonce: v.get("nonce")?.as_str()?.to_string(),
        code_verifier: v.get("verifier")?.as_str()?.to_string(),
    })
}

fn read_challenge(state: &AppState, headers: &HeaderMap) -> Option<AuthRequest> {
    let raw = headers
        .get(axum::http::header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .filter_map(|c| c.trim().split_once('='))
        .find(|(k, _)| *k == CHALLENGE_COOKIE)
        .map(|(_, v)| v.to_string())?;
    unseal(state, &raw)
}

/// Begin: mint a fresh challenge, remember it in the cookie, hand off to Keycloak.
pub async fn start(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let Some(provider) = state.oidc.clone() else {
        // Not configured: refuse exactly as any other failed sign-in would, never
        // a 500 and never a stack trace (AC-5.3).
        return refuse(&state, &headers, "oidc is not configured");
    };
    let req = AuthRequest::generate();
    let url = match provider.authorization_url(&req).await {
        Ok(u) => u,
        Err(err) => return refuse(&state, &headers, &format!("authorization_url: {err}")),
    };
    let Some(sealed) = seal(&state, &req) else {
        return refuse(&state, &headers, "could not seal the challenge");
    };

    let cookie = format!(
        "{CHALLENGE_COOKIE}={sealed}; Max-Age={CHALLENGE_TTL_SECONDS}{}",
        cookie_attrs(&state)
    );
    let mut resp = Response::builder()
        .status(StatusCode::FOUND)
        .header(axum::http::header::LOCATION, url)
        .body(axum::body::Body::empty())
        .expect("redirect response builds");
    if let Ok(v) = cookie.parse() {
        resp.headers_mut().append(SET_COOKIE, v);
    }
    resp
}

/// Finish: verify the challenge, exchange the code, link the identity, sign in.
pub async fn callback(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Query(q): Query<CallbackQuery>,
) -> Response {
    let Some(provider) = state.oidc.clone() else {
        return refuse(&state, &headers, "oidc is not configured");
    };
    // No challenge means this callback was never started here.
    let Some(req) = read_challenge(&state, &headers) else {
        return refuse(&state, &headers, "no challenge cookie");
    };
    if q.state.is_empty() || q.state != req.state {
        return refuse(&state, &headers, "state mismatch");
    }
    if q.code.is_empty() {
        return refuse(&state, &headers, "no authorization code");
    }

    // Exchanges the code (single-use AT the provider — this is what actually
    // refuses a replay), validates the ID token RS256-pinned against the
    // published JWKS, and checks the nonce.
    let identity = match provider.exchange_code(&q.code, &req).await {
        Ok(c) => c,
        Err(err) => return refuse(&state, &headers, &format!("exchange: {err}")),
    };

    if !identity.email_verified {
        return refuse(&state, &headers, "provider has not confirmed the email");
    }

    // LINK, never provision (D3). users.email_lower is UNIQUE, so the match is
    // unambiguous; an identity with no foundry account is refused, which is what
    // keeps invites the only way into the tracker.
    let email_lower = identity.email.trim().to_lowercase();
    let user = match state.store.find_user_by_email(&email_lower).await {
        Ok(Some(u)) => u,
        Ok(None) => return refuse(&state, &headers, "no foundry account for this identity"),
        Err(err) => {
            tracing::error!(%err, "find_user_by_email failed during oidc callback");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };

    let mut resp = establish_session(&state, &session, &headers, user.id).await;
    if let Ok(v) = clear_cookie(&state).parse() {
        resp.headers_mut().append(SET_COOKIE, v);
    }
    resp
}
