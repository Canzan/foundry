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
use axum::http::{header, HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use base64::Engine;
use rand::RngCore;
use subtle::ConstantTimeEq;

pub const CSRF_COOKIE_NAME: &str = "foundry_csrf";
pub const CSRF_FORM_FIELD: &str = "_csrf";
pub const CSRF_HEADER: &str = "x-csrf-token";
pub const CSRF_HX_HEADER: &str = "hx-csrf";

/// Ceiling on the urlencoded form body this middleware buffers to read the
/// `_csrf` field. This is a GLOBAL cap that applies to EVERY form POST: the
/// CSRF middleware sits OUTSIDE the routes and their per-route
/// `DefaultBodyLimit` (it wraps the router in lib.rs, under only the session +
/// metrics layers), so it reads the body here — via
/// `to_bytes(.., CSRF_BODY_BUFFER_MAX_BYTES)` — BEFORE any per-route
/// `DefaultBodyLimit` or handler extractor runs. So this constant is the
/// per-request buffering ceiling for ALL form routes (sign-in, issue create/
/// edit, comments, …), not only the ones that need a large body.
///
/// The buffering only happens on requests that carry a CSRF cookie: a form POST
/// with no `foundry_csrf` cookie is refused with 403 BEFORE the body is read
/// (see `csrf_middleware`), so an unauthenticated caller cannot force a 2 MiB
/// allocation on a doomed request.
///
/// It must cover the LARGEST legitimate form body. The issue create/edit forms
/// carry a `description` up to `DESCRIPTION_MAX_LEN` (262144) CHARACTERS —
/// counted with `chars().count()` in `foundry-services`, so the limit is in
/// CHARACTERS, not bytes, and the worst case is the WIDEST character. Urlencoded
/// per-char byte cost:
///   - ASCII        1 B  → 256 KB at the limit
///   - 2-byte 'é'   6 B  (`%C3%A9`)        → ~1.5 MB
///   - 3-byte '中'  9 B  (`%E4%B8%AD`)     → ~2.25 MB
///   - 4-byte '😀' 12 B  (`%F0%9F%98%80`)  → ~3.0 MB   ← worst case
///
/// A 4-byte description at the limit is ~3.0 MB; plus title + `_csrf` overhead,
/// 4 MiB covers it with margin. (The earlier 2 MiB cap only reasoned about
/// 2-byte chars and silently 403'd CJK/emoji descriptions at the char limit.)
///
/// SECURITY (DoS): this is the per-form-POST buffering ceiling for ALL form
/// routes, NOT scoped to the issue endpoints. Two things bound the exposure:
/// (1) since the F1 gate, the body is only buffered on requests that already
/// carry a valid-shaped CSRF cookie — a cookie-less caller is refused before any
/// allocation; and (2) 4 MiB is a small multiple of axum's 2 MiB default
/// `DefaultBodyLimit`, applied once per request. The per-route
/// `DefaultBodyLimit::max(2 MiB)` on issue create/edit (lib.rs) DECLARES the
/// intended large payload there — it does NOT reduce this global buffer, and is
/// itself sized to the same DESCRIPTION_MAX_LEN contract. A tighter future
/// design could make this cap per-route (reading the route's own limit) to
/// restore a small ceiling on small forms.
const CSRF_BODY_BUFFER_MAX_BYTES: usize = 4 * 1024 * 1024;

/// Generate a fresh 32-byte URL-safe CSRF token.
pub fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Build the Set-Cookie header for the CSRF token. Note `HttpOnly` is
/// FALSE so a client-side script can read it and mirror it into a request
/// header (see the multipart note in `csrf_middleware`). `static/js/csrf-upload.js`
/// is the live consumer.
pub fn build_csrf_cookie(value: &str, secure: bool) -> String {
    let secure_attr = if secure { "; Secure" } else { "" };
    // Path=/; SameSite=Lax; Max-Age=86400 (1 day).
    format!("{CSRF_COOKIE_NAME}={value}; Path=/; SameSite=Lax; Max-Age=86400{secure_attr}")
}

/// Reuse the request's CSRF cookie if present, else mint a fresh one. The GET
/// write-form handlers (new-issue modal, edit dialog, issue-detail add-comment)
/// render the returned token into the form's hidden `_csrf` field; the matching
/// write POST (under [`csrf_middleware`]) double-submits it against this same
/// cookie. Returns `(token, Some(set_cookie))` when a fresh cookie must be
/// attached, or `(token, None)` when the request already carried one. This is
/// the single issuance seam shared by every write-form page (previously
/// duplicated in `issues.rs` and `keyboard.rs`).
pub(crate) fn ensure_csrf_cookie(
    state: &AppState,
    headers: &HeaderMap,
) -> (String, Option<String>) {
    let existing = headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(extract_csrf_cookie);
    if let Some(token) = existing {
        return (token, None);
    }
    let token = generate_token();
    let cookie = build_csrf_cookie(&token, state.session_cookie_secure);
    (token, Some(cookie))
}

/// Attach an optional freshly-minted CSRF `Set-Cookie` to a response and set its
/// status. Paired with [`ensure_csrf_cookie`] on the GET write-form pages.
pub(crate) fn response_with_optional_cookie(
    status: StatusCode,
    body: Response,
    set_cookie: Option<String>,
) -> Response {
    let (mut parts, body) = body.into_parts();
    parts.status = status;
    if let Some(cookie) = set_cookie {
        if let Ok(v) = HeaderValue::from_str(&cookie) {
            parts.headers.insert(header::SET_COOKIE, v);
        }
    }
    Response::from_parts(parts, body)
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

/// The single fail-closed refusal. Byte-identical across every rejection branch
/// (missing cookie, missing token, mismatch, multipart-without-header) so no
/// response oracle can distinguish *why* CSRF failed.
fn csrf_refusal() -> Response {
    (
        StatusCode::FORBIDDEN,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )],
        "CSRF token missing or mismatched",
    )
        .into_response()
}

/// Pull `_csrf` out of a urlencoded body without consuming the request.
async fn extract_form_token(body: Body) -> (Option<String>, Body) {
    // Body size cap (CSRF_BODY_BUFFER_MAX_BYTES). GLOBAL: this middleware wraps
    // the router, so EVERY form POST that reaches here is buffered up to this
    // ceiling before any per-route DefaultBodyLimit runs. Only requests that
    // already passed the cookie-presence gate reach this point. See
    // CSRF_BODY_BUFFER_MAX_BYTES for the DoS rationale (2 MiB == axum's default
    // body limit).
    match to_bytes(body, CSRF_BODY_BUFFER_MAX_BYTES).await {
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

    // Fail closed BEFORE buffering the body. Every valid branch below requires a
    // non-empty CSRF cookie (double-submit), so a request without one is already
    // a guaranteed 403 — refuse it here rather than allocate up to
    // CSRF_BODY_BUFFER_MAX_BYTES (2 MiB) for a body that cannot change the
    // verdict. Without this gate an UNAUTHENTICATED caller (no cookie, any path,
    // including a non-existent one that never reaches a route) could force a
    // 2 MiB per-request allocation. The refusal is byte-identical to the token-
    // mismatch refusal, so no new oracle is introduced.
    let cookie_present = cookie_value.as_deref().is_some_and(|c| !c.is_empty());
    if !cookie_present {
        return csrf_refusal();
    }

    // Multipart bodies skip the urlencoded form-extraction path. The CSRF
    // buffer cap would otherwise truncate a large file upload
    // and the downstream multipart handler would see an empty body.
    // For multipart, the CSRF token MUST arrive in the `x-csrf-token`
    // (or `hx-csrf`) header — US-11's upload client sets this; a
    // browser multipart form post relies on `static/js/csrf-upload.js`,
    // which mirrors the cookie value into the header.
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
            return csrf_refusal();
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
        return csrf_refusal();
    }

    next.run(req).await
}
