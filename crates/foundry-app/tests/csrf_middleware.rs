//! Per the acceptance-designer-reviewer Q2 resolution: a single
//! middleware-level integration test exercises the CSRF double-submit
//! contract across three input cases — missing token, mismatched
//! token, and matching token — rather than scattering CSRF scenarios
//! across each .feature file.
//!
//! We mount the real `csrf_middleware` on a tiny in-process router
//! whose handler always returns 200 OK. If the middleware lets a
//! request through, the test sees 200; if the middleware rejects, it
//! sees 403. The handler is never reached for the rejection cases —
//! that *is* the contract.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::middleware;
use axum::routing::post;
use axum::Router;
use foundry_app::csrf::csrf_middleware;
use foundry_app::{AppState, NoopEmailSender, SystemClock};
use foundry_store::Store;
use secrecy::SecretString;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tower::ServiceExt;

/// Build a router with ONLY the CSRF middleware mounted and a 200-OK
/// terminal handler. The AppState is required by the middleware signature
/// but the middleware never reads the store, so a "dead" sqlx pool that
/// never connects is sufficient.
///
/// `PgPoolOptions::min_connections(0)` + `lazy_connect` would still
/// try to validate a URL; instead we use `connect_lazy` on a
/// syntactically valid URL.
async fn build_test_router() -> Router {
    let pool = PgPoolOptions::new()
        .min_connections(0)
        .max_connections(1)
        .connect_lazy("postgres://noone:nopass@127.0.0.1:1/none")
        .expect("build lazy pool");
    let store = Arc::new(Store::from_pool(pool));
    let state = AppState {
        store,
        session_secret: Arc::new(SecretString::new(
            "csrf-test-secret-please-thirty-two-bytes-yes-yes-yes-yes".into(),
        )),
        session_cookie_secure: false,
        db_schema: "public".into(),
        public_url: "http://localhost".into(),
        clock: Arc::new(SystemClock),
        email: Arc::new(NoopEmailSender),
        realtime_tx: foundry_realtime::build_broadcast(),
        sse_heartbeat_ms: foundry_app::DEFAULT_SSE_HEARTBEAT_MS,
        file_upload_max_mb: foundry_app::DEFAULT_FILE_UPLOAD_MAX_MB,
        db_unreachable: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        test_migrations_dir: None,
        applied_migrations: Arc::new(std::sync::Mutex::new(
            foundry_store::MigrationReport::default(),
        )),
        test_migration_delay_ms: 0,
    };
    Router::new()
        .route("/csrf-probe", post(|| async { "OK" }))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            csrf_middleware,
        ))
        .with_state(state)
}

#[tokio::test]
async fn csrf_middleware_enforces_double_submit_contract() {
    let router = build_test_router().await;

    // Case 1: missing token (no cookie, no form field) -> 403
    let req = Request::builder()
        .method("POST")
        .uri("/csrf-probe")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(""))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "missing CSRF token must return 403"
    );

    // Case 2: mismatched token (cookie A, form B) -> 403
    let body = "_csrf=mismatched-token-value";
    let req = Request::builder()
        .method("POST")
        .uri("/csrf-probe")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, "foundry_csrf=cookie-token-A")
        .body(Body::from(body))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::FORBIDDEN,
        "mismatched CSRF token must return 403"
    );

    // Case 3: matching token in cookie + form -> 200 OK
    let token = "matched-token-value";
    let body = format!("_csrf={token}");
    let req = Request::builder()
        .method("POST")
        .uri("/csrf-probe")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, format!("foundry_csrf={token}"))
        .body(Body::from(body))
        .unwrap();
    let resp = router.oneshot(req).await.unwrap();
    assert_eq!(
        resp.status(),
        StatusCode::OK,
        "matching CSRF token must let the request through"
    );
}
