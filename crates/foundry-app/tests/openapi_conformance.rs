//! api-openapi-document — router-level conformance of the served description
//! (DESIGN DDD-9 behavioural layer, [REF] Test Seams "foundry-app integration").
//!
//! Authored RED in DISTILL 2026-10-19 as pending panic stubs (the
//! project-lifecycle `project_lifecycle_copy.rs` idiom). DELIVER replaces each
//! stub with real requests through `foundry_app::build_router` on a
//! testcontainers Postgres (the `api_comment_scoping.rs` harness), judges every
//! answer against the document's entry for that operation and status, and
//! removes the `#[ignore]`. Step 01-01 lands the five that the served
//! description alone makes true; the three that need every operation
//! described stay pending for 02-01 and 02-02. The acceptance matrix
//! (`api-openapi-document-02-every-answer.feature`) produces the same answers
//! through the HTTP lane; these tests hold them below it, inside
//! `cargo test --workspace` (so `smoke` and `ci` run them unconditionally).

use axum::body::Body;
use axum::http::{header, HeaderMap, Method, Request, StatusCode};
use axum::Router;
use foundry_app::{build_router, AppState, Notifier, SystemClock};
use foundry_auth::{MachineTokenClaims, MachineTokenSigner, MachineTokenVerifier};
use foundry_store::{run_migrations, Store};
use secrecy::{ExposeSecret, SecretString};
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};
use tower::ServiceExt;

/// The fixed test Ed25519 keypair (`foundry_auth::test_keys`).
const SIGNING_KEY_PEM: &str = "-----BEGIN PRIVATE KEY-----\nMC4CAQAwBQYDK2VwBCIEIK82aJdks5jZYdUK6HN6tiWmq7nU4yrILbQSibZdvMYK\n-----END PRIVATE KEY-----\n";
const PUBLIC_KEY_PEM: &str = "-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEAwtFPs8Jcuncc+E7dXqG/oolI3P6Hamrpd8zVKPvRmg0=\n-----END PUBLIC KEY-----\n";
const CSRF: &str = "openapi-conformance-csrf-token";
const PASSWORD: &str = "openapi-conformance-password";
const MEI: &str = "mei@acme.com";
const DOC_PATH: &str = "/api/v1/openapi.json";
/// An address that was never part of the API: the catch-all's answer.
const NEVER_API_PATH: &str = "/api/v1/no-such-address";

/// The committed description, read from disk (not foundry-api's constant).
fn committed() -> Vec<u8> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../foundry-api/openapi.json");
    std::fs::read(path).unwrap_or_else(|e| panic!("read the committed description {path}: {e}"))
}

fn document() -> Value {
    serde_json::from_slice(&committed()).expect("the committed description parses as JSON")
}

/// One workspace (`Acme`), team `backend` with project `auth`; Mei is on the
/// team, can sign in, and holds machine tokens.
struct World {
    router: Router,
    store: Arc<Store>,
    workspace_id: uuid::Uuid,
    mei: uuid::Uuid,
    _pg: ContainerAsync<Postgres>,
}

fn app_state(store: Arc<Store>) -> AppState {
    AppState {
        oidc: None,
        store,
        session_secret: Arc::new(SecretString::new(
            "openapi-conformance-secret-thirty-two-bytes-or-more".into(),
        )),
        machine_token_verifier: Arc::new(
            MachineTokenVerifier::from_public_keys(&[PUBLIC_KEY_PEM.to_string()])
                .expect("fixed test public key is valid"),
        ),
        machine_token_signer: None,
        session_cookie_secure: false,
        db_schema: "public".into(),
        public_url: "http://foundry.test".into(),
        clock: Arc::new(SystemClock),
        notifier: Arc::new(Notifier::new(vec![])),
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
    }
}

async fn exec(store: &Store, sql: &str, binds: &[uuid::Uuid], what: &str) {
    let mut q = sqlx::query(sql);
    for b in binds {
        q = q.bind(*b);
    }
    q.execute(store.pool()).await.expect(what);
}

async fn world() -> World {
    let pg = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("start postgres container");
    let host = pg.get_host().await.expect("host");
    let port = pg.get_host_port_ipv4(5432).await.expect("port");
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&format!(
            "postgres://postgres:postgres@{host}:{port}/postgres"
        ))
        .await
        .expect("connect pool");
    run_migrations(&pool).await.expect("run migrations");
    let store = Arc::new(Store::from_pool(pool));

    let workspace_id = uuid::Uuid::now_v7();
    let team_id = uuid::Uuid::now_v7();
    let project_id = uuid::Uuid::now_v7();
    let mei = uuid::Uuid::now_v7();
    exec(
        &store,
        "INSERT INTO workspaces (id, name) VALUES ($1, 'Acme')",
        &[workspace_id],
        "seed workspace",
    )
    .await;
    exec(
        &store,
        "INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'Backend', 'backend')",
        &[team_id, workspace_id],
        "seed team",
    )
    .await;
    exec(
        &store,
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, 'auth', 'auth', 'AUTH')",
        &[project_id, team_id, workspace_id],
        "seed project",
    )
    .await;
    let hash = foundry_auth::hash_password(&SecretString::new(PASSWORD.into()))
        .await
        .expect("hash");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, 'Mei', $3)",
    )
    .bind(mei)
    .bind(MEI)
    .bind(&hash)
    .execute(store.pool())
    .await
    .expect("seed user");
    exec(
        &store,
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
        &[workspace_id, mei],
        "seed workspace membership",
    )
    .await;
    exec(
        &store,
        "INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'member')",
        &[team_id, mei],
        "seed team membership",
    )
    .await;

    World {
        router: build_router(app_state(store.clone())),
        store,
        workspace_id,
        mei,
        _pg: pg,
    }
}

/// Register and sign a machine token for Mei that expires `ttl` from now.
async fn mint(w: &World, label: &str, ttl: time::Duration) -> (uuid::Uuid, String) {
    let jti = uuid::Uuid::now_v7();
    let now = time::OffsetDateTime::now_utc();
    let expires_at = now + ttl;
    w.store
        .insert_machine_token(jti, w.mei, w.workspace_id, None, expires_at, label, w.mei)
        .await
        .expect("register machine token");
    let signer =
        MachineTokenSigner::from_pkcs8_pem(&SecretString::new(SIGNING_KEY_PEM.to_string().into()))
            .expect("fixed test signing key is valid");
    let token = signer
        .mint(&MachineTokenClaims {
            sub: w.mei,
            scope: None,
            iat: now.unix_timestamp(),
            exp: expires_at.unix_timestamp(),
            jti,
            iss: String::new(),
            aud: String::new(),
        })
        .expect("mint machine token")
        .expose_secret()
        .to_string();
    (jti, token)
}

/// Sign in through the password form; returns the session cookies.
async fn sign_in(router: &Router, email: &str) -> String {
    let body =
        serde_urlencoded::to_string([("_csrf", CSRF), ("email", email), ("password", PASSWORD)])
            .expect("encode form");
    let req = Request::builder()
        .method("POST")
        .uri("/sign-in")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header(header::COOKIE, format!("foundry_csrf={CSRF}"))
        .body(Body::from(body))
        .expect("request");
    let resp = router.clone().oneshot(req).await.expect("response");
    assert_eq!(resp.status(), StatusCode::SEE_OTHER, "{email} signs in");
    resp.headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok()?.split(';').next().map(str::to_string))
        .collect::<Vec<_>>()
        .join("; ")
}

struct Answer {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl Answer {
    fn header(&self, name: header::HeaderName) -> Option<&str> {
        self.headers.get(name).and_then(|v| v.to_str().ok())
    }
}

async fn call(
    router: &Router,
    method: Method,
    uri: &str,
    headers: &[(header::HeaderName, String)],
) -> Answer {
    let mut req = Request::builder().method(method).uri(uri);
    for (name, value) in headers {
        req = req.header(name, value);
    }
    let resp = router
        .clone()
        .oneshot(req.body(Body::empty()).expect("request"))
        .await
        .expect("response");
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("body")
        .to_vec();
    Answer {
        status,
        headers,
        body,
    }
}

/// A documented path template with every `{param}` filled in.
fn concrete(template: &str) -> String {
    let mut out = template.to_string();
    for (param, value) in [
        ("{team_slug}", "backend".to_string()),
        ("{project_slug}", "auth".to_string()),
        ("{number}", "1".to_string()),
        ("{comment_id}", uuid::Uuid::now_v7().to_string()),
        ("{jti}", uuid::Uuid::now_v7().to_string()),
    ] {
        out = out.replace(param, &value);
    }
    assert!(
        !out.contains('{'),
        "{template} has a path parameter this test cannot fill"
    );
    out
}

fn resolve<'a>(doc: &'a Value, node: &'a Value) -> &'a Value {
    node.get("$ref")
        .and_then(Value::as_str)
        .and_then(|r| r.strip_prefix('#'))
        .and_then(|pointer| doc.pointer(pointer))
        .unwrap_or(node)
}

/// D1, D8, OQ-6: GET with no header, a garbage bearer, an expired token, a
/// revoked token and a session cookie each answer 200 `application/json`,
/// `Cache-Control: no-cache`, and bytes equal to the committed file.
#[tokio::test]
async fn the_description_is_the_same_bytes_for_every_credential() {
    let w = world().await;
    let (_, expired) = mint(&w, "expired", time::Duration::hours(-1)).await;
    let (revoked_jti, revoked) = mint(&w, "revoked", time::Duration::hours(1)).await;
    w.store
        .revoke_machine_token(revoked_jti)
        .await
        .expect("revoke the token");
    let session = sign_in(&w.router, MEI).await;
    let committed = committed();
    assert_eq!(
        committed,
        foundry_api::openapi::DOCUMENT,
        "foundry-api compiles in the committed file"
    );
    let credentials: [(&str, Option<(header::HeaderName, String)>); 5] = [
        ("no credential", None),
        (
            "a garbage bearer",
            Some((header::AUTHORIZATION, "Bearer garbage".to_string())),
        ),
        (
            "an expired token",
            Some((header::AUTHORIZATION, format!("Bearer {expired}"))),
        ),
        (
            "a revoked token",
            Some((header::AUTHORIZATION, format!("Bearer {revoked}"))),
        ),
        ("a session cookie", Some((header::COOKIE, session))),
    ];
    for (what, credential) in credentials {
        let answer = call(&w.router, Method::GET, DOC_PATH, credential.as_slice()).await;
        assert_eq!(
            answer.status,
            StatusCode::OK,
            "{what}: the description answers 200"
        );
        assert_eq!(
            answer.header(header::CONTENT_TYPE),
            Some("application/json"),
            "{what}: the description is JSON"
        );
        assert_eq!(
            answer.header(header::CACHE_CONTROL),
            Some("no-cache"),
            "{what}: the description is revalidated"
        );
        assert!(
            answer.body == committed,
            "{what}: the served bytes ({}) differ from the committed file ({})",
            answer.body.len(),
            committed.len()
        );
    }
}

/// Changed Assumption 1 (user, 2026-10-18): POST, PUT, PATCH and DELETE on
/// `/api/v1/openapi.json` answer the 404 envelope byte-identical to
/// `/api/v1/openapi.yaml` and to the catch-all, with no `Allow` header.
#[tokio::test]
async fn writing_to_the_description_answers_the_catch_all() {
    let w = world().await;
    let never = call(&w.router, Method::GET, NEVER_API_PATH, &[]).await;
    let beside = call(&w.router, Method::GET, "/api/v1/openapi.yaml", &[]).await;
    assert_eq!(
        never.status,
        StatusCode::NOT_FOUND,
        "the catch-all answers 404"
    );
    assert_eq!(
        (beside.status, &beside.body),
        (never.status, &never.body),
        "/api/v1/openapi.yaml answers the catch-all"
    );
    for method in [Method::POST, Method::PUT, Method::PATCH, Method::DELETE] {
        let answer = call(&w.router, method.clone(), DOC_PATH, &[]).await;
        assert_eq!(
            (
                answer.status,
                &answer.body,
                answer.header(header::CONTENT_TYPE)
            ),
            (
                never.status,
                &never.body,
                never.header(header::CONTENT_TYPE)
            ),
            "{method} {DOC_PATH} answers the catch-all's 404 byte for byte"
        );
        assert_eq!(
            answer.header(header::ALLOW),
            None,
            "{method} {DOC_PATH} names no allowed methods"
        );
    }
}

/// DESIGN Test Seams: `HEAD /api/v1/openapi.json` answers 200 with no body.
#[tokio::test]
async fn asking_only_for_the_headers_answers_200_with_no_body() {
    let w = world().await;
    let answer = call(&w.router, Method::HEAD, DOC_PATH, &[]).await;
    assert_eq!(answer.status, StatusCode::OK, "HEAD {DOC_PATH} answers 200");
    assert_eq!(
        answer.header(header::CONTENT_TYPE),
        Some("application/json"),
        "HEAD carries GET's media type"
    );
    assert!(answer.body.is_empty(), "HEAD answers no body");
}

/// For each documented operation, no token answers the documented 401 envelope.
#[tokio::test]
async fn every_documented_operation_refuses_no_token_as_documented() {
    let w = world().await;
    let doc = document();
    let mut refused = 0;
    for (template, item) in doc["paths"].as_object().expect("the description has paths") {
        for (method, op) in item.as_object().into_iter().flatten() {
            let security = op.get("security").unwrap_or(&doc["security"]);
            if security == &serde_json::json!([]) {
                // The description's own route asks for no credential.
                continue;
            }
            let documented = resolve(&doc, &op["responses"]["401"]);
            let example =
                &documented["content"]["application/json"]["examples"]["unauthorized"]["value"];
            assert!(
                example.is_object(),
                "{method} {template} documents no 401 `unauthorized` envelope"
            );
            let verb =
                Method::from_bytes(method.to_ascii_uppercase().as_bytes()).expect("a method");
            let answer = call(
                &w.router,
                verb,
                &concrete(template),
                &[(header::CONTENT_TYPE, "application/json".to_string())],
            )
            .await;
            assert_eq!(
                answer.status,
                StatusCode::UNAUTHORIZED,
                "{method} {template} with no token answers 401"
            );
            assert_eq!(
                answer.header(header::CONTENT_TYPE),
                Some("application/json"),
                "{method} {template}'s 401 is JSON"
            );
            let body: Value = serde_json::from_slice(&answer.body).expect("the 401 parses");
            assert_eq!(
                &body, example,
                "{method} {template}'s 401 is the documented envelope"
            );
            refused += 1;
        }
    }
    assert!(
        refused >= 1,
        "the description documents no operation that asks for a token"
    );
}

/// For each documented path except the description's own, each undocumented
/// method answers 405 with an `Allow` header equal to the documented methods
/// (plus HEAD where GET).
#[test]
#[ignore = "pending: api-openapi-document 01-04 — 405 with an Allow header equal to the documented methods"]
fn every_documented_path_answers_405_with_the_documented_methods() {
    panic!("Not yet implemented -- RED scaffold (api-openapi-document 01-04: 405 + Allow)");
}

/// With a real token on a seeded project: a bad `{number}` / `{jti}` /
/// `{comment_id}` is 400 text/plain; no Content-Type 415; bad JSON 400; a
/// missing field 422 text/plain; a 2 MB + 1 byte body 413 (C4, C12); each
/// status and media type equals the document's entry for that operation.
#[test]
#[ignore = "pending: api-openapi-document 01-04 — the text/plain extractor refusals match the document"]
fn the_extractor_refusals_answer_as_documented() {
    panic!("Not yet implemented -- RED scaffold (api-openapi-document 01-04: 400/413/415/422 text/plain)");
}

/// A valid create answers 201 with a `Location` of the documented form.
#[test]
#[ignore = "pending: api-openapi-document 01-04 — a created issue carries the documented Location"]
fn a_created_issue_carries_the_documented_location() {
    panic!("Not yet implemented -- RED scaffold (api-openapi-document 01-04: 201 Location)");
}

/// Review L1: the document's `info.version` equals
/// `foundry_app::views::RELEASE_VERSION`, the footer's source.
#[test]
fn the_description_version_is_the_footer_version() {
    assert_eq!(
        document()["info"]["version"].as_str(),
        Some(foundry_app::views::RELEASE_VERSION),
        "the description's info.version is the footer's release; run `cargo xtask openapi`"
    )
}
