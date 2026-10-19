//! api-openapi-document slice 03 — the API page at `/docs/api` (DESIGN DDD-15,
//! DDD-16, DDD-19; ADR-API-OPENAPI-003 and -004).
//!
//! A pure render: the release version in, the page out. It reads no session and
//! no store and sets no cookie, so it is the same for everyone. It is mounted
//! outside the session and CSRF layers and inside the request-tracking layer
//! (`build_router`). Its own content policy and companion headers are set by
//! [`api_docs_headers`], layered on this route only: no other answer changes
//! (D12, SC-3). Foundry sends no site-wide policy, so this one is the page's
//! whole policy; the layer `insert`s, so the page never carries two.

use askama::Template;
use axum::extract::Request;
use axum::http::{header, HeaderName, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;

/// The page's address (DDD-15).
pub const API_DOCS_PATH: &str = "/docs/api";

/// The page's content policy, exactly (DDD-19). No `'unsafe-eval'`, and
/// `script-src` never gains `'unsafe-inline'`: swagger-ui's React output needs
/// inline styles only. The seed of the site-wide follow-up (ADR-004 decision 3).
pub const API_DOCS_CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'; object-src 'none'";

/// Every header the page's answer carries besides its body's own.
/// `Cache-Control: no-cache` because the heading carries the release version.
const API_DOCS_HEADERS: [(HeaderName, &str); 5] = [
    (header::CONTENT_SECURITY_POLICY, API_DOCS_CSP),
    (header::X_FRAME_OPTIONS, "DENY"),
    (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
    (header::REFERRER_POLICY, "no-referrer"),
    (header::CACHE_CONTROL, "no-cache"),
];

/// The standalone page template (DDD-16): it does not extend `base.html`, so it
/// loads no htmx and no app script, only its three files.
#[derive(Template)]
#[template(path = "api_docs.html")]
struct ApiDocsPage<'a> {
    version: &'a str,
}

/// `GET /docs/api`.
pub async fn show() -> Response {
    match (ApiDocsPage {
        version: crate::views::RELEASE_VERSION,
    })
    .render()
    {
        Ok(body) => Html(body).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Stamps the page's policy and companions onto its answer, replacing any
/// value an inner layer set.
pub async fn api_docs_headers(req: Request, next: Next) -> Response {
    let mut resp = next.run(req).await;
    let headers = resp.headers_mut();
    for (name, value) in API_DOCS_HEADERS {
        headers.insert(name, HeaderValue::from_static(value));
    }
    resp
}

/// The page's route, its headers layered on it alone.
pub fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
    Router::new().route(
        API_DOCS_PATH,
        get(show).layer(middleware::from_fn(api_docs_headers)),
    )
}
