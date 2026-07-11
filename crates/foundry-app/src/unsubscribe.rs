//! recipient-notification-preferences — the PUBLIC `/unsubscribe` confirm/mutate
//! vertical (ADR-001/002).
//!
//! A NEW PUBLIC web driving adapter turning the signed in-body unsubscribe link
//! into a live, prefetch-safe flow for account-less recipients:
//!   - `GET /unsubscribe?t=..&sig=..` — decode `t` (base64url of
//!     `email_lower|workspace_id`), verify `sig` (constant-time `UnsubscribeToken`).
//!     On failure → the uniform non-enumerable refusal (mirrors
//!     `invites_accept::invite_refusal_page`). On success → a STATE-AWARE confirm
//!     page (Unsubscribe when subscribed, Resubscribe when already muted) + the CSRF
//!     cookie. The GET is NON-DESTRUCTIVE (renders only — a scanner prefetch changes
//!     nothing, NFR-2).
//!   - `POST /unsubscribe` — CSRF-checked by the shipped `csrf_middleware`. Re-verify
//!     the token, then WRITE (`action=unsubscribe`) or CLEAR (`action=resubscribe`)
//!     the `(email_lower, workspace_id)` row (idempotent, BR-8). A bad token →
//!     uniform refusal, no state change.
//!
//! Mounted in `build_router` on the PUBLIC layer (the recipient is signed OUT)
//! UNDER the shipped `session_layer` + `csrf::csrf_middleware`, alongside
//! `/invites/accept` and `/forgot-password`.

use crate::bootstrap::{html_escape, invalid_page};
use crate::csrf::{build_csrf_cookie, extract_csrf_cookie, generate_token};
use crate::AppState;
use axum::extract::{Form, Query, State};
use axum::http::header::{HeaderMap, HeaderValue, COOKIE, SET_COOKIE};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use base64::Engine;
use serde::Deserialize;

const UNSUBSCRIBE_ACTION: &str = "unsubscribe";
const RESUBSCRIBE_ACTION: &str = "resubscribe";

#[derive(Debug, Deserialize)]
pub struct UnsubscribeQuery {
    pub t: Option<String>,
    pub sig: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UnsubscribeForm {
    pub t: String,
    pub sig: String,
    #[serde(default)]
    pub action: String,
    #[serde(rename = "_csrf", default)]
    pub _csrf: Option<String>,
}

// ------------------------------------------------------------- GET /unsubscribe

/// Verify the signed link and render the state-aware confirm page. NON-DESTRUCTIVE:
/// no row is ever written here (prefetch-safety, NFR-2). Any bad/tampered/unknown
/// token → the uniform non-enumerable refusal.
pub async fn show_confirm(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<UnsubscribeQuery>,
) -> Response {
    let (Some(t), Some(sig)) = (query.t.as_deref(), query.sig.as_deref()) else {
        return unsubscribe_refusal_page();
    };
    let Some((email_lower, workspace_id)) = decode_and_verify(&state, t, sig) else {
        return unsubscribe_refusal_page();
    };
    // Name the workspace the token authorizes. A missing workspace (e.g. deleted)
    // collapses to the same uniform refusal — no oracle on existence.
    let workspace_name = match state.store.workspace_name(workspace_id).await {
        Ok(Some(name)) => name,
        Ok(None) => return unsubscribe_refusal_page(),
        Err(err) => {
            tracing::error!(%err, "workspace_name failed on unsubscribe GET");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };
    // State-aware (ADR-006): offer Resubscribe when already muted, else Unsubscribe.
    let already_unsubscribed = match state
        .store
        .is_unsubscribed(&email_lower, workspace_id)
        .await
    {
        Ok(value) => value,
        Err(err) => {
            tracing::error!(%err, "is_unsubscribed failed on unsubscribe GET");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };
    let (token, set_cookie) = ensure_csrf_cookie(&state, &headers);
    let body = render_confirm_page(&token, t, sig, &workspace_name, already_unsubscribed);
    response_with_optional_cookie(StatusCode::OK, Html(body).into_response(), set_cookie)
}

// ------------------------------------------------------------ POST /unsubscribe

/// CSRF-checked by the shipped middleware. Re-verify the token, then write (opt out)
/// or clear (resubscribe) the row. Idempotent. A bad token → uniform refusal, no
/// state change.
pub async fn submit_confirm(
    State(state): State<AppState>,
    Form(form): Form<UnsubscribeForm>,
) -> Response {
    let Some((email_lower, workspace_id)) = decode_and_verify(&state, &form.t, &form.sig) else {
        return unsubscribe_refusal_page();
    };
    let workspace_name = match state.store.workspace_name(workspace_id).await {
        Ok(Some(name)) => name,
        Ok(None) => return unsubscribe_refusal_page(),
        Err(err) => {
            tracing::error!(%err, "workspace_name failed on unsubscribe POST");
            return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
        }
    };
    let resubscribe = form.action == RESUBSCRIBE_ACTION;
    let result = if resubscribe {
        state
            .store
            .delete_unsubscribe(&email_lower, workspace_id)
            .await
    } else {
        // Default to opting out (the confirm page's primary action). `ON CONFLICT
        // DO NOTHING` makes confirming twice a harmless no-op.
        state
            .store
            .insert_unsubscribe(&email_lower, workspace_id)
            .await
    };
    if let Err(err) = result {
        tracing::error!(%err, resubscribe, "unsubscribe write failed");
        return (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response();
    }
    Html(render_result_page(&workspace_name, resubscribe)).into_response()
}

// ----------------------------------------------------------------------- link

/// Build the trailing unsubscribe line appended to a SUPPRESSIBLE email body
/// (ADR-002): `\n\nTo stop these emails, unsubscribe here:\n{public_url}/unsubscribe?t=..&sig=..`.
/// Returns an EMPTY string on a signing failure so the invite still sends (the link
/// is best-effort, not load-bearing for the invite itself).
pub fn unsubscribe_link_line(
    state: &AppState,
    recipient: &str,
    workspace_id: uuid::Uuid,
) -> String {
    match unsubscribe_url(state, recipient, workspace_id) {
        Some(url) => format!("\n\nTo stop these emails, unsubscribe here:\n{url}"),
        None => String::new(),
    }
}

/// The full public unsubscribe URL for `(recipient, workspace)`. `None` on a
/// signing failure. Public so tests can mirror the exact link a recipient receives.
pub fn unsubscribe_url(
    state: &AppState,
    recipient: &str,
    workspace_id: uuid::Uuid,
) -> Option<String> {
    let email_lower = recipient.to_ascii_lowercase();
    let token =
        foundry_auth::UnsubscribeToken::new(&email_lower, workspace_id, &state.session_secret)
            .ok()?;
    let t = encode_t(&email_lower, workspace_id);
    Some(format!(
        "{}/unsubscribe?t={}&sig={}",
        state.public_url.trim_end_matches('/'),
        urlencoding::encode(&t),
        urlencoding::encode(&token.signature),
    ))
}

// -------------------------------------------------------------------- helpers

/// base64url(`email_lower|workspace_id`) — the opaque `t` param (log-hygiene
/// obfuscation, ADR-002; NOT confidentiality).
pub fn encode_t(email_lower: &str, workspace_id: uuid::Uuid) -> String {
    let payload = format!("{email_lower}|{workspace_id}");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload.as_bytes())
}

/// Decode `t` + verify `sig` against the recovered pair. Returns the
/// `(email_lower, workspace_id)` iff decode AND constant-time signature verify both
/// pass; `None` (→ uniform refusal) on any failure.
fn decode_and_verify(state: &AppState, t: &str, sig: &str) -> Option<(String, uuid::Uuid)> {
    let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(t.as_bytes())
        .ok()?;
    let decoded = String::from_utf8(raw).ok()?;
    let (email_lower, workspace_raw) = decoded.rsplit_once('|')?;
    let workspace_id = uuid::Uuid::parse_str(workspace_raw).ok()?;
    foundry_auth::UnsubscribeToken::verify(email_lower, workspace_id, sig, &state.session_secret)
        .ok()?;
    Some((email_lower.to_string(), workspace_id))
}

/// The UNIFORM non-enumerable unsubscribe refusal (ADR-002) — every invalid-link
/// reason (tampered / unknown / missing / deleted-workspace) collapses to THIS
/// byte-identical fixed-200 response. Mirrors `invites_accept::invite_refusal_page`.
fn unsubscribe_refusal_page() -> Response {
    invalid_page(
        StatusCode::OK,
        "This unsubscribe link is no longer valid",
        "It may have been mistyped or the link may be out of date. If you keep \
         receiving unwanted emails, follow the unsubscribe link in a more recent \
         message.",
    )
}

fn render_confirm_page(
    csrf_token: &str,
    t: &str,
    sig: &str,
    workspace_name: &str,
    already_unsubscribed: bool,
) -> String {
    let name = html_escape(workspace_name);
    let (action, heading, lead, button) = if already_unsubscribed {
        (
            RESUBSCRIBE_ACTION,
            format!("You are unsubscribed from “{name}”"),
            format!("You currently receive no invitation emails from “{name}”."),
            "Resubscribe".to_string(),
        )
    } else {
        (
            UNSUBSCRIBE_ACTION,
            format!("Stop invitation emails from “{name}”?"),
            format!(
                "Confirm to stop receiving invitation emails from “{name}”. Security \
                     emails such as password resets are always delivered."
            ),
            "Unsubscribe".to_string(),
        )
    };
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <title>{heading}</title></head><body>\
         <main><h1>{heading}</h1><p>{lead}</p>\
         <form method=\"post\" action=\"/unsubscribe\">\
         <input type=\"hidden\" name=\"_csrf\" value=\"{csrf}\">\
         <input type=\"hidden\" name=\"t\" value=\"{t}\">\
         <input type=\"hidden\" name=\"sig\" value=\"{sig}\">\
         <input type=\"hidden\" name=\"action\" value=\"{action}\">\
         <button type=\"submit\">{button}</button>\
         </form></main></body></html>",
        heading = heading,
        lead = lead,
        csrf = html_escape(csrf_token),
        t = html_escape(t),
        sig = html_escape(sig),
        action = action,
        button = button,
    )
}

fn render_result_page(workspace_name: &str, resubscribed: bool) -> String {
    let name = html_escape(workspace_name);
    let (heading, message) = if resubscribed {
        (
            format!("You are subscribed to “{name}” again"),
            format!("You will once again receive invitation emails from “{name}”."),
        )
    } else {
        (
            format!("“{name}” invitations are stopped"),
            format!(
                "You will no longer receive invitation emails from “{name}”. Security \
                     emails such as password resets are still delivered."
            ),
        )
    };
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
         <title>{heading}</title></head><body>\
         <main><h1>{heading}</h1><p>{message}</p></main></body></html>",
        heading = heading,
        message = message,
    )
}

/// Reuse the request's `foundry_csrf` cookie if present, else mint one (the
/// signed-out double-submit seam, mirrors `invites_accept::ensure_csrf_cookie`).
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
