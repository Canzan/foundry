//! CSRF — double-submit cookie pattern (NFR-SEC-04, design/auth.md §CSRF).
//!
//! - On GETs, the cookie `foundry_csrf` is set if absent. The handler
//!   templates render the same value as a hidden `_csrf` form field.
//! - On POST/PUT/PATCH/DELETE, this middleware compares the cookie
//!   value with either the form `_csrf` field OR the `HX-CSRF` header
//!   (htmx). Constant-time compare via `subtle`. Missing or mismatched
//!   token returns `403 Forbidden` — the rejected request never reaches
//!   the handler.
//!
//! Exempt POST paths:
//!   - `/bootstrap` — pre-session admin claim. The bootstrap token in
//!     the URL plus the SHA-256 single-use guard already prevent CSRF.
//!     (Pre-session forms cannot rely on a server-issued cookie.)

use crate::AppState;
use axum::body::{to_bytes, Body};
use axum::extract::{Request, State};
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use base64::Engine;
use rand::RngCore;
use subtle::ConstantTimeEq;

pub const CSRF_COOKIE_NAME: &str = "foundry_csrf";
pub const CSRF_FORM_FIELD: &str = "_csrf";
pub const CSRF_HEADER: &str = "x-csrf-token";
pub const CSRF_HX_HEADER: &str = "hx-csrf";

/// Generate a fresh 32-byte URL-safe CSRF token.
pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Build the Set-Cookie header for the CSRF token. Note `HttpOnly` is
/// FALSE so an htmx/alpine.js hook can read it client-side.
pub fn build_csrf_cookie(value: &str, secure: bool) -> String {
    let secure_attr = if secure { "; Secure" } else { "" };
    // Path=/; SameSite=Lax; Max-Age=86400 (1 day).
    format!("{CSRF_COOKIE_NAME}={value}; Path=/; SameSite=Lax; Max-Age=86400{secure_attr}")
}

/// Extract the CSRF cookie value from a Cookie header.
pub fn extract_csrf_cookie(cookie_header: &str) -> Option<String> {
    for piece in cookie_header.split(';') {
        let trimmed = piece.trim();
        if let Some(rest) = trimmed.strip_prefix(&format!("{CSRF_COOKIE_NAME}=")) {
            return Some(rest.to_string());
        }
    }
    None
}

fn is_safe_method(m: &Method) -> bool {
    matches!(*m, Method::GET | Method::HEAD | Method::OPTIONS)
}

fn is_exempt_path(path: &str) -> bool {
    // The bootstrap POST is pre-session (no cookie yet). The /bootstrap
    // single-use token in the URL already provides CSRF-equivalent
    // protection: an attacker would need the operator's secret token,
    // and the token is consumed on first POST.
    path == "/bootstrap"
}

fn constant_time_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    a.ct_eq(b).into()
}

/// Pull `_csrf` out of a urlencoded body without consuming the request.
async fn extract_form_token(body: Body) -> (Option<String>, Body) {
    // Body size cap — sign-in forms are tiny.
    match to_bytes(body, 64 * 1024).await {
        Ok(bytes) => {
            let token = serde_urlencoded::from_bytes::<Vec<(String, String)>>(&bytes)
                .ok()
                .and_then(|pairs| {
                    pairs
                        .into_iter()
                        .find_map(|(k, v)| (k == CSRF_FORM_FIELD).then_some(v))
                });
            (token, Body::from(bytes))
        }
        Err(_) => (None, Body::empty()),
    }
}

pub async fn csrf_middleware(State(_state): State<AppState>, req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    if is_safe_method(&method) || is_exempt_path(&path) {
        return next.run(req).await;
    }

    // Pull the cookie BEFORE we tear the request apart.
    let cookie_value = req
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(extract_csrf_cookie);
    let header_token = req
        .headers()
        .get(CSRF_HEADER)
        .or_else(|| req.headers().get(CSRF_HX_HEADER))
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    // Multipart bodies skip the urlencoded form-extraction path. The
    // existing 64 KB cap would otherwise truncate a 9 MB file upload
    // and the downstream multipart handler would see an empty body.
    // For multipart, the CSRF token MUST arrive in the `x-csrf-token`
    // (or `hx-csrf`) header — US-11's upload client sets this; a
    // browser multipart form post can rely on a small alpine.js/htmx
    // hook that mirrors the cookie value into the header.
    let is_multipart = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_ascii_lowercase().starts_with("multipart/form-data"))
        .unwrap_or(false);

    if is_multipart {
        let valid = match (cookie_value.as_deref(), header_token.as_deref()) {
            (Some(c), Some(s)) if !c.is_empty() && !s.is_empty() => constant_time_eq(c, s),
            _ => false,
        };
        if !valid {
            return (
                StatusCode::FORBIDDEN,
                [(
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("text/plain; charset=utf-8"),
                )],
                "CSRF token missing or mismatched",
            )
                .into_response();
        }
        return next.run(req).await;
    }

    let (parts, body) = req.into_parts();
    let (form_token, body) = extract_form_token(body).await;
    let req = Request::from_parts(parts, body);

    let supplied = header_token.or(form_token);
    let valid = match (cookie_value.as_deref(), supplied.as_deref()) {
        (Some(c), Some(s)) if !c.is_empty() && !s.is_empty() => constant_time_eq(c, s),
        _ => false,
    };

    if !valid {
        return (
            StatusCode::FORBIDDEN,
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/plain; charset=utf-8"),
            )],
            "CSRF token missing or mismatched",
        )
            .into_response();
    }

    next.run(req).await
}
