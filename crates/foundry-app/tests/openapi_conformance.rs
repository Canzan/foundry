//! api-openapi-document — router-level conformance of the served description
//! (DESIGN DDD-9 behavioural layer, [REF] Test Seams "foundry-app integration").
//!
//! Each test sends real requests through `foundry_app::build_router` on a
//! testcontainers Postgres (the `api_comment_scoping.rs` harness) and judges
//! every answer against the document's entry for that operation and status.
//! The acceptance matrix
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
    // The default lanes, so an issue can be created (the
    // `api_comment_scoping.rs` idiom).
    exec(
        &store,
        "INSERT INTO lanes (id, project_id, workspace_id, slug, label, position)
         SELECT gen_random_uuid(), $1, $2, v.slug, v.label, v.position
           FROM (VALUES ('backlog', 'Backlog', 0), ('todo', 'Todo', 1),
                        ('in_progress', 'In-Progress', 2), ('done', 'Done', 3))
                AS v (slug, label, position)",
        &[project_id, workspace_id],
        "seed lanes",
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
#[tokio::test]
async fn every_documented_path_answers_405_with_the_documented_methods() {
    let w = world().await;
    let doc = document();
    let asked = [
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::PATCH,
        Method::DELETE,
    ];
    let mut refused = 0;
    for (template, item) in doc["paths"].as_object().expect("the description has paths") {
        if template == DOC_PATH {
            // The description's own path answers the catch-all instead (DDD-4).
            continue;
        }
        let documented: Vec<Method> = asked
            .iter()
            .filter(|m| item.get(m.as_str().to_ascii_lowercase()).is_some())
            .cloned()
            .collect();
        let mut allowed: std::collections::BTreeSet<String> =
            documented.iter().map(|m| m.as_str().to_string()).collect();
        if documented.contains(&Method::GET) {
            allowed.insert("HEAD".to_string());
        }
        for method in &documented {
            let op = &item[method.as_str().to_ascii_lowercase()];
            let response = resolve(&doc, &op["responses"]["405"]);
            assert!(
                response["headers"].get("Allow").is_some(),
                "{method} {template} documents no 405 with an Allow header"
            );
            assert!(
                response.get("content").is_none(),
                "{method} {template} documents a body on its 405"
            );
        }
        for method in asked.iter().filter(|m| !documented.contains(m)) {
            let answer = call(&w.router, method.clone(), &concrete(template), &[]).await;
            assert_eq!(
                answer.status,
                StatusCode::METHOD_NOT_ALLOWED,
                "{method} {template} is not documented, so it answers 405"
            );
            assert!(
                answer.body.is_empty(),
                "{method} {template}'s 405 has no body"
            );
            let named: std::collections::BTreeSet<String> = answer
                .header(header::ALLOW)
                .unwrap_or_default()
                .split(',')
                .map(|m| m.trim().to_ascii_uppercase())
                .filter(|m| !m.is_empty())
                .collect();
            assert_eq!(
                named, allowed,
                "{method} {template}'s Allow names the documented methods"
            );
            refused += 1;
        }
    }
    assert!(
        refused >= 1,
        "the description documents no path a wrong method can be tried on"
    );
}

/// With a real token on a seeded project: a bad `{number}` / `{jti}` /
/// `{comment_id}` is 400 text/plain; no Content-Type 415; bad JSON 400; a
/// missing field 422 text/plain; a 2 MB + 1 byte body 413 (C4, C12); each
/// status and media type equals the document's entry for that operation.
#[tokio::test]
async fn the_extractor_refusals_answer_as_documented() {
    let w = world().await;
    let doc = document();
    let (_, token) = mint(&w, "refused", time::Duration::hours(1)).await;
    let bearer = (header::AUTHORIZATION, format!("Bearer {token}"));
    let json = (header::CONTENT_TYPE, "application/json".to_string());
    let mut refusals = Vec::new();

    // A path segment that does not parse: 400 text/plain.
    for (method, tail, param, body) in [
        (
            Method::PATCH,
            "/issues/{number}",
            "{number}",
            r#"{"state":"todo"}"#,
        ),
        (Method::GET, "/issues/{number}/history", "{number}", ""),
        (
            Method::POST,
            "/issues/{number}/comments",
            "{number}",
            r#"{"body":"hi"}"#,
        ),
        (
            Method::PATCH,
            "/issues/{number}/comments/{comment_id}",
            "{number}",
            r#"{"body":"hi"}"#,
        ),
        (
            Method::PATCH,
            "/issues/{number}/comments/{comment_id}",
            "{comment_id}",
            r#"{"body":"hi"}"#,
        ),
        (Method::DELETE, "/tokens/{jti}", "{jti}", ""),
    ] {
        let template = format!("{PROJECT}{tail}");
        let headers = if body.is_empty() {
            vec![bearer.clone()]
        } else {
            vec![bearer.clone(), json.clone()]
        };
        refusals.push(Refusal {
            uri: concrete(&template.replace(param, NOT_AN_ID)),
            method,
            template,
            headers,
            body: body.as_bytes().to_vec(),
            status: StatusCode::BAD_REQUEST,
            what: "a path segment that does not parse",
        });
    }

    // The four body-taking operations, each with every body refusal.
    let too_large = format!(r#"{{"title":"{}"}}"#, "x".repeat(2 * 1024 * 1024 + 1));
    for (method, tail) in [
        (Method::POST, "/issues"),
        (Method::PATCH, "/issues/{number}"),
        (Method::POST, "/issues/{number}/comments"),
        (Method::PATCH, "/issues/{number}/comments/{comment_id}"),
    ] {
        let template = format!("{PROJECT}{tail}");
        let uri = concrete(&template);
        let with_json = vec![bearer.clone(), json.clone()];
        for (headers, body, status, what) in [
            (
                vec![bearer.clone()],
                b"{}".to_vec(),
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "no Content-Type",
            ),
            (
                with_json.clone(),
                b"{".to_vec(),
                StatusCode::BAD_REQUEST,
                "a body that is not JSON",
            ),
            (
                with_json.clone(),
                b"{}".to_vec(),
                StatusCode::UNPROCESSABLE_ENTITY,
                "a body missing its required field",
            ),
            (
                with_json.clone(),
                too_large.clone().into_bytes(),
                StatusCode::PAYLOAD_TOO_LARGE,
                "a body over 2 MB",
            ),
        ] {
            refusals.push(Refusal {
                method: method.clone(),
                template: template.clone(),
                uri: uri.clone(),
                headers,
                body,
                status,
                what,
            });
        }
    }

    for refusal in refusals {
        refusal.answers_as_documented(&w.router, &doc).await;
    }
}

const PROJECT: &str = "/api/v1/teams/{team_slug}/projects/{project_slug}";
const NOT_AN_ID: &str = "not-an-id";

/// One request the framework refuses before the handler runs, and the
/// status it must answer with a plain-text body.
struct Refusal {
    method: Method,
    template: String,
    uri: String,
    headers: Vec<(header::HeaderName, String)>,
    body: Vec<u8>,
    status: StatusCode,
    what: &'static str,
}

impl Refusal {
    async fn answers_as_documented(self, router: &Router, doc: &Value) {
        let Refusal {
            method,
            template,
            uri,
            headers,
            body,
            status,
            what,
        } = self;
        let op = method.as_str().to_ascii_lowercase();
        let documented = resolve(
            doc,
            &doc["paths"][&template][&op]["responses"][status.as_str()],
        );
        assert!(
            documented["content"].get("text/plain").is_some(),
            "{method} {template} documents no text/plain {status} for {what}"
        );
        let mut req = Request::builder().method(method.clone()).uri(&uri);
        for (name, value) in &headers {
            req = req.header(name, value);
        }
        let resp = router
            .clone()
            .oneshot(req.body(Body::from(body)).expect("request"))
            .await
            .expect("response");
        assert_eq!(resp.status(), status, "{method} {uri} with {what}");
        let media = resp
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(';').next())
            .map(str::trim)
            .map(str::to_string);
        assert_eq!(
            media.as_deref(),
            Some("text/plain"),
            "{method} {uri} with {what} answers plain text, as documented"
        );
    }
}

/// A valid create answers 201 with a `Location` of the documented form.
#[tokio::test]
async fn a_created_issue_carries_the_documented_location() {
    let w = world().await;
    let doc = document();
    let collection = "/api/v1/teams/{team_slug}/projects/{project_slug}/issues";
    let one_issue = "/api/v1/teams/{team_slug}/projects/{project_slug}/issues/{number}";
    let created = resolve(&doc, &doc["paths"][collection]["post"]["responses"]["201"]);
    assert!(
        created["headers"].get("Location").is_some(),
        "POST {collection} documents no Location header on its 201"
    );
    assert!(
        doc["paths"].get(one_issue).is_some(),
        "the description has no path for one issue for a Location to name"
    );
    let (_, token) = mint(&w, "creator", time::Duration::hours(1)).await;
    let req = Request::builder()
        .method(Method::POST)
        .uri(concrete(collection))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"title":"Sign-in times out behind the proxy"}"#,
        ))
        .expect("request");
    let resp = w.router.clone().oneshot(req).await.expect("response");
    assert_eq!(
        resp.status(),
        StatusCode::CREATED,
        "a valid create answers 201"
    );
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .expect("the 201 carries a Location");
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("body");
    let issue: Value = serde_json::from_slice(&body).expect("the 201 body parses");
    let number = issue["number"]
        .as_i64()
        .expect("the created issue has a number");
    let expected = one_issue
        .replace("{team_slug}", "backend")
        .replace("{project_slug}", "auth")
        .replace("{number}", &number.to_string());
    assert_eq!(
        location, expected,
        "the Location is the documented one-issue address of the created issue"
    );
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
