//! api-openapi-document — the API page below the browser lane (DESIGN DDD-15,
//! DDD-16, DDD-19, review H3; [REF] Test Seams "foundry-app integration,
//! slice 03").
//!
//! Authored RED in DISTILL 2026-10-19 as pending panic stubs (the
//! project-lifecycle `project_lifecycle_copy.rs` idiom): `/docs/api`,
//! `api_docs::show`, `API_DOCS_CSP` and the vendored files do not exist yet.
//! DELIVER step 03-02 replaces each stub with requests through
//! `foundry_app::build_router` (the `api_comment_scoping.rs` harness; Docker
//! required, as for every testcontainers test) and removes the `#[ignore]`.
//! These run in `smoke` and `ci` with no browser; the browser lane keeps only
//! what needs one (`api-openapi-document-03-api-page.feature`, `@needs-browser`).

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use foundry_app::{AppState, Notifier, SystemClock};
use secrecy::SecretString;
use std::sync::Arc;
use tower::ServiceExt;

/// The fixed test Ed25519 public key (`foundry_auth::test_keys`).
const PUBLIC_KEY_PEM: &str = "-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEAwtFPs8Jcuncc+E7dXqG/oolI3P6Hamrpd8zVKPvRmg0=\n-----END PUBLIC KEY-----\n";

/// `GET /docs/api` with no cookie and with a signed-in session: 200, no
/// `Set-Cookie`, the exact `API_DOCS_CSP` string, `X-Frame-Options: DENY`,
/// `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`,
/// `Cache-Control: no-cache`.
#[test]
#[ignore = "pending: api-openapi-document 03-02 — page headers, no Set-Cookie, the exact CSP"]
fn the_api_page_carries_its_policy_and_sets_no_cookie() {
    panic!("Not yet implemented -- RED scaffold (api-openapi-document 03-02: /docs/api headers)");
}

/// D14 copy, byte-exact: the title "Foundry API", the heading line with
/// `RELEASE_VERSION`, the `<noscript>` copy and its link to `/api/v1/openapi.json`.
#[test]
#[ignore = "pending: api-openapi-document 03-02 — the page's D14 copy, byte-exact"]
fn the_api_page_copy_is_byte_exact() {
    panic!("Not yet implemented -- RED scaffold (api-openapi-document 03-02: title, heading, noscript)");
}

/// Exactly three subresource references, all same-origin `/static/…`: the
/// bundle, the stylesheet and `api-docs.js`; no inline script, handler or style.
#[test]
#[ignore = "pending: api-openapi-document 03-02 — exactly three same-origin subresources"]
fn the_api_page_references_exactly_its_three_files() {
    panic!("Not yet implemented -- RED scaffold (api-openapi-document 03-02: subresources)");
}

/// The policy is the page's only: `GET /`, `GET /static/…` and
/// `GET /api/v1/openapi.json` carry no `Content-Security-Policy` (D12, SC-3).
#[test]
#[ignore = "pending: api-openapi-document 03-02 — no other answer carries the page's policy"]
fn no_other_answer_carries_the_page_policy() {
    panic!("Not yet implemented -- RED scaffold (api-openapi-document 03-02: page-only CSP)");
}

/// Every vendored swagger-ui file and `api-docs.js` answers 200 through the
/// real `ServeDir`, `/vendor/` immutable and `/js/` no-cache (ADR-B03).
///
/// Driven through `foundry_app::build_router` with a never-connecting pool (the
/// `csrf_middleware.rs` idiom): `/static` is a `ServeDir` and reads no store, so
/// this needs no Docker. The body must be the repository's bytes, verbatim.
#[tokio::test]
async fn every_page_file_is_served_with_its_cache_policy() {
    const IMMUTABLE: &str = "public, max-age=31536000, immutable";
    let files = [
        ("vendor/swagger-ui-5.33.3/swagger-ui-bundle.js", IMMUTABLE),
        ("vendor/swagger-ui-5.33.3/swagger-ui.css", IMMUTABLE),
        ("vendor/swagger-ui-5.33.3/LICENSE.txt", IMMUTABLE),
        ("vendor/swagger-ui-5.33.3/NOTICE.txt", IMMUTABLE),
        ("js/api-docs.js", "no-cache"),
    ];
    let router = static_router();
    let static_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("static");
    for (relative, cache_control) in files {
        let path = format!("/static/{relative}");
        let request = Request::builder()
            .uri(&path)
            .body(Body::empty())
            .expect("a valid request");
        let response = router.clone().oneshot(request).await.expect("an answer");
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        assert_eq!(
            response
                .headers()
                .get(header::CACHE_CONTROL)
                .and_then(|v| v.to_str().ok()),
            Some(cache_control),
            "{path}: its cache policy (ADR-B03)"
        );
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("a readable body");
        let on_disk = std::fs::read(static_root.join(relative))
            .unwrap_or_else(|e| panic!("the repository holds no {path}: {e}"));
        assert!(
            body.as_ref() == on_disk.as_slice(),
            "{path} is not served as the repository holds it"
        );
    }
}

/// The real router over a pool that never connects: enough for `/static`.
fn static_router() -> axum::Router {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .min_connections(0)
        .max_connections(1)
        .connect_lazy("postgres://noone:nopass@127.0.0.1:1/none")
        .expect("a lazy pool");
    foundry_app::build_router(AppState {
        oidc: None,
        store: Arc::new(foundry_store::Store::from_pool(pool)),
        session_secret: Arc::new(SecretString::new(
            "api-docs-page-secret-thirty-two-bytes-or-more".into(),
        )),
        machine_token_verifier: Arc::new(
            foundry_auth::MachineTokenVerifier::from_public_keys(&[PUBLIC_KEY_PEM.to_string()])
                .expect("fixed test public key is valid"),
        ),
        machine_token_signer: None,
        session_cookie_secure: false,
        db_schema: "public".into(),
        public_url: "http://foundry.test".into(),
        clock: Arc::new(SystemClock),
        notifier: Arc::new(Notifier::empty()),
        revoke_rate_limiter: Arc::new(foundry_app::rate_limit::RevokeRateLimiter::default()),
        realtime_tx: foundry_realtime::build_broadcast(),
        sse_heartbeat_ms: foundry_app::DEFAULT_SSE_HEARTBEAT_MS,
        file_upload_max_mb: foundry_app::DEFAULT_FILE_UPLOAD_MAX_MB,
        db_unreachable: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        force_board_render_failure: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        test_migrations_dir: None,
        applied_migrations: Arc::new(std::sync::Mutex::new(
            foundry_store::MigrationReport::default(),
        )),
        test_migration_delay_ms: 0,
    })
}
