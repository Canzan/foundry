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
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};
use tower::ServiceExt;

/// The API page (DESIGN DDD-15) and the description it points to (D1).
const PAGE_PATH: &str = "/docs/api";
const DOC_PATH: &str = "/api/v1/openapi.json";
/// DDD-19 / ADR-API-OPENAPI-004 decision 1, exactly.
const PAGE_CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'; object-src 'none'";
const CSRF: &str = "api-docs-page-csrf-token";
const PASSWORD: &str = "api-docs-page-password";
const MEMBER: &str = "priya@canzan.dev";

/// The fixed test Ed25519 public key (`foundry_auth::test_keys`).
const PUBLIC_KEY_PEM: &str = "-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEAwtFPs8Jcuncc+E7dXqG/oolI3P6Hamrpd8zVKPvRmg0=\n-----END PUBLIC KEY-----\n";

/// `GET /docs/api` with no cookie and with a signed-in session: 200, no
/// `Set-Cookie`, the exact `API_DOCS_CSP` string, `X-Frame-Options: DENY`,
/// `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`,
/// `Cache-Control: no-cache`.
#[tokio::test]
async fn the_api_page_carries_its_policy_and_sets_no_cookie() {
    let (router, _pg) = router_over_postgres().await;
    let session = sign_in(&router).await;
    assert!(
        session.contains("foundry_session"),
        "the sign-in left no session cookie: {session:?}"
    );

    let mut pages = Vec::new();
    for (who, cookie) in [("a visitor", None), ("a signed-in member", Some(&session))] {
        let response = get(&router, PAGE_PATH, cookie).await;
        assert_eq!(response.status(), StatusCode::OK, "{who}");
        assert!(
            response.headers().get(header::SET_COOKIE).is_none(),
            "{who}: the API page set a cookie"
        );
        let headers = response.headers();
        for (name, expected) in [
            (header::CONTENT_SECURITY_POLICY, PAGE_CSP),
            (header::X_FRAME_OPTIONS, "DENY"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (header::REFERRER_POLICY, "no-referrer"),
            (header::CACHE_CONTROL, "no-cache"),
        ] {
            let values: Vec<&str> = headers
                .get_all(&name)
                .iter()
                .filter_map(|v| v.to_str().ok())
                .collect();
            assert_eq!(values, [expected], "{who}: {name}");
        }
        pages.push(body_text(response).await);
    }
    assert_eq!(pages[0], pages[1], "the API page differs once signed in");
}

/// D14 copy, byte-exact: the title "Foundry API", the heading line with
/// `RELEASE_VERSION`, the `<noscript>` copy and its link to `/api/v1/openapi.json`.
#[tokio::test]
async fn the_api_page_copy_is_byte_exact() {
    let page = page_html().await;
    let version = foundry_app::views::RELEASE_VERSION;
    let doc_link = format!("<a href=\"{DOC_PATH}\">{DOC_PATH}</a>");
    for copy in [
        "<title>Foundry API</title>".to_string(),
        "<h1>Foundry API</h1>".to_string(),
        format!("<p>Foundry API v{version}. The OpenAPI document is at {doc_link}.</p>"),
        format!(
            "<noscript><p>This page needs JavaScript. The OpenAPI document is at {doc_link}.</p></noscript>"
        ),
        "<div id=\"swagger-ui\"></div>".to_string(),
    ] {
        assert!(page.contains(&copy), "the API page lacks {copy:?}:\n{page}");
    }
    assert!(page.contains("<html lang=\"en\">"), "the page's language");
}

/// Exactly three subresource references, all same-origin `/static/…`: the
/// bundle, the stylesheet and `api-docs.js`; no inline script, handler or style.
#[tokio::test]
async fn the_api_page_references_exactly_its_three_files() {
    let page = page_html().await;
    let mut found = std::collections::BTreeMap::new();
    for attr in ["src=\"", "href=\""] {
        for (at, _) in page.match_indices(attr) {
            let rest = &page[at + attr.len()..];
            found.insert(at, rest[..rest.find('"').unwrap_or(0)].to_string());
        }
    }
    let references: Vec<String> = found
        .into_values()
        .filter(|target| target != DOC_PATH)
        .collect();
    assert_eq!(
        references,
        [
            "/static/vendor/swagger-ui-5.33.3/swagger-ui-bundle.js",
            "/static/vendor/swagger-ui-5.33.3/swagger-ui.css",
            "/static/js/api-docs.js",
        ],
        "the API page's subresources"
    );
    assert_eq!(
        page.matches("<script").count(),
        page.matches("<script defer src=\"/static/").count(),
        "an inline or non-deferred script on the API page:\n{page}"
    );
    let lower = page.to_ascii_lowercase();
    for inline in [
        "<style",
        " style=",
        " onclick=",
        " onload=",
        " onerror=",
        "javascript:",
    ] {
        assert!(!lower.contains(inline), "the API page carries {inline:?}");
    }
}

/// The policy is the page's only: `GET /`, `GET /static/…` and
/// `GET /api/v1/openapi.json` carry no `Content-Security-Policy` (D12, SC-3).
#[tokio::test]
async fn no_other_answer_carries_the_page_policy() {
    let router = static_router();
    let page = get(&router, PAGE_PATH, None).await;
    assert_eq!(
        page.headers().get(header::CONTENT_SECURITY_POLICY),
        Some(&axum::http::HeaderValue::from_static(PAGE_CSP)),
        "control: the API page itself carries the policy"
    );
    for path in [
        "/",
        "/sign-in",
        "/static/js/api-docs.js",
        "/static/vendor/swagger-ui-5.33.3/swagger-ui-bundle.js",
        DOC_PATH,
        "/docs/api/",
        "/docs/no-such-page",
    ] {
        let response = get(&router, path, None).await;
        for name in [header::CONTENT_SECURITY_POLICY, header::X_FRAME_OPTIONS] {
            assert!(
                response.headers().get(&name).is_none(),
                "{path} carries the API page's {name}"
            );
        }
    }
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
    router_over(pool)
}

/// The real router over a real, migrated Postgres holding one workspace and
/// one member who signs in with a password.
async fn router_over_postgres() -> (axum::Router, ContainerAsync<Postgres>) {
    let pg = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("start postgres container");
    let host = pg.get_host().await.expect("host");
    let port = pg.get_host_port_ipv4(5432).await.expect("port");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(&format!(
            "postgres://postgres:postgres@{host}:{port}/postgres"
        ))
        .await
        .expect("connect pool");
    foundry_store::run_migrations(&pool)
        .await
        .expect("run migrations");
    let workspace_id = uuid::Uuid::now_v7();
    let user_id = uuid::Uuid::now_v7();
    let hash = foundry_auth::hash_password(&SecretString::new(PASSWORD.into()))
        .await
        .expect("hash");
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'Canzan Labs')")
        .bind(workspace_id)
        .execute(&pool)
        .await
        .expect("seed workspace");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, $2, $3)",
    )
    .bind(user_id)
    .bind(MEMBER)
    .bind(&hash)
    .execute(&pool)
    .await
    .expect("seed user");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(workspace_id)
    .bind(user_id)
    .execute(&pool)
    .await
    .expect("seed workspace membership");
    (router_over(pool), pg)
}

/// Sign in through the password form; returns the cookies it set.
async fn sign_in(router: &axum::Router) -> String {
    let body =
        serde_urlencoded::to_string([("_csrf", CSRF), ("email", MEMBER), ("password", PASSWORD)])
            .expect("encode form");
    let request = Request::builder()
        .method("POST")
        .uri("/sign-in")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, format!("foundry_csrf={CSRF}"))
        .body(Body::from(body))
        .expect("a valid request");
    let response = router.clone().oneshot(request).await.expect("an answer");
    assert_eq!(
        response.status(),
        StatusCode::SEE_OTHER,
        "the member signs in"
    );
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok()?.split(';').next().map(str::to_string))
        .collect::<Vec<_>>()
        .join("; ")
}

async fn get(
    router: &axum::Router,
    path: &str,
    cookie: Option<&String>,
) -> axum::response::Response {
    let mut request = Request::builder().uri(path);
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie.as_str());
    }
    let request = request.body(Body::empty()).expect("a valid request");
    router.clone().oneshot(request).await.expect("an answer")
}

async fn body_text(response: axum::response::Response) -> String {
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("a readable body");
    String::from_utf8(bytes.to_vec()).expect("a UTF-8 page")
}

/// The API page's HTML, as anyone gets it.
async fn page_html() -> String {
    let response = get(&static_router(), PAGE_PATH, None).await;
    assert_eq!(response.status(), StatusCode::OK, "{PAGE_PATH}");
    body_text(response).await
}

fn router_over(pool: sqlx::PgPool) -> axum::Router {
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
