//! fix-issue-attachment-scoping step 01-04 (RCA §4, comment sibling): the JSON
//! API edits a comment only through the address of the issue it belongs to.
//!
//! `PATCH /api/v1/teams/{team}/projects/{project}/issues/{n}/comments/{id}`
//! through the comment's sibling issue answers exactly as a comment id that
//! never existed does, and changes nothing. Driven through the real router
//! (driving port) with a real machine token over a real Postgres. No doubles.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use foundry_app::{build_router, AppState, Notifier, SystemClock};
use foundry_auth::{MachineTokenClaims, MachineTokenSigner, MachineTokenVerifier};
use foundry_store::{run_migrations, Store};
use secrecy::{ExposeSecret, SecretString};
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
const COMMENT_BODY: &str = "Backend-only note: rotate the zebra-42 signing key";
const NEW_BODY: &str = "edited through the API";

/// One workspace; team Backend (`backend`) with project `api`, issues 1 and 2.
/// Yuki is on Backend, wrote the comment on issue 1, and holds a machine token.
struct World {
    router: Router,
    store: Arc<Store>,
    token: String,
    comment_id: uuid::Uuid,
    _pg: ContainerAsync<Postgres>,
}

fn app_state(store: Arc<Store>) -> AppState {
    AppState {
        oidc: None,
        store,
        session_secret: Arc::new(SecretString::new(
            "api-comment-scoping-secret-thirty-two-bytes-or-more".into(),
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

async fn seed_issue(store: &Store, ids: [uuid::Uuid; 3], number: i32) -> uuid::Uuid {
    let [workspace_id, project_id, author_id] = ids;
    let issue_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, description_md,
                             state, position, author_id)
              VALUES ($1, $2, $3, $4, 'seed', '', 'todo', $4, $5)",
    )
    .bind(issue_id)
    .bind(project_id)
    .bind(workspace_id)
    .bind(number)
    .bind(author_id)
    .execute(store.pool())
    .await
    .expect("seed issue");
    issue_id
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
    let yuki = uuid::Uuid::now_v7();
    exec(
        &store,
        "INSERT INTO workspaces (id, name) VALUES ($1, 'Acme')",
        &[workspace_id],
        "seed workspace",
    )
    .await;
    exec(
        &store,
        "INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'backend', 'backend')",
        &[team_id, workspace_id],
        "seed team",
    )
    .await;
    exec(
        &store,
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, 'api', 'api', 'AUTH')",
        &[project_id, team_id, workspace_id],
        "seed project",
    )
    .await;
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
    exec(
        &store,
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, 'yuki@example.com', 'yuki@example.com', 'Yuki', 'x')",
        &[yuki],
        "seed user",
    )
    .await;
    exec(
        &store,
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
        &[workspace_id, yuki],
        "seed workspace membership",
    )
    .await;
    exec(
        &store,
        "INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'member')",
        &[team_id, yuki],
        "seed team membership",
    )
    .await;
    let api_1 = seed_issue(&store, [workspace_id, project_id, yuki], 1).await;
    seed_issue(&store, [workspace_id, project_id, yuki], 2).await;

    let comment_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO comments (id, workspace_id, issue_id, author_id, body_markdown, body_html)
              VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(comment_id)
    .bind(workspace_id)
    .bind(api_1)
    .bind(yuki)
    .bind(COMMENT_BODY)
    .bind(format!("<p>{COMMENT_BODY}</p>"))
    .execute(store.pool())
    .await
    .expect("seed comment");

    let jti = uuid::Uuid::now_v7();
    let now = time::OffsetDateTime::now_utc();
    let expires_at = now + time::Duration::hours(1);
    store
        .insert_machine_token(jti, yuki, workspace_id, None, expires_at, "ci", yuki)
        .await
        .expect("register machine token");
    let signer =
        MachineTokenSigner::from_pkcs8_pem(&SecretString::new(SIGNING_KEY_PEM.to_string().into()))
            .expect("fixed test signing key is valid");
    let token = signer
        .mint(&MachineTokenClaims {
            sub: yuki,
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

    World {
        router: build_router(app_state(store.clone())),
        store,
        token,
        comment_id,
        _pg: pg,
    }
}

async fn patch(w: &World, issue: i32, comment_id: uuid::Uuid) -> (StatusCode, Vec<u8>) {
    let req = Request::builder()
        .method("PATCH")
        .uri(format!(
            "/api/v1/teams/backend/projects/api/issues/{issue}/comments/{comment_id}"
        ))
        .header(header::AUTHORIZATION, format!("Bearer {}", w.token))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            serde_json::json!({ "body": NEW_BODY }).to_string(),
        ))
        .expect("request");
    let resp = w.router.clone().oneshot(req).await.expect("response");
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("body");
    (status, bytes.to_vec())
}

async fn stored_body(w: &World) -> String {
    sqlx::query_scalar("SELECT body_markdown FROM comments WHERE id = $1")
        .bind(w.comment_id)
        .fetch_one(w.store.pool())
        .await
        .expect("the comment row")
}

#[tokio::test]
async fn api_patch_through_a_sibling_issue_path_is_not_found_and_changes_nothing() {
    let w = world().await;

    let reference = patch(&w, 1, uuid::Uuid::now_v7()).await;
    assert_eq!(
        reference.0,
        StatusCode::NOT_FOUND,
        "the reference (a comment id that never existed) is the API's 404"
    );
    let answer = patch(&w, 2, w.comment_id).await;
    assert_eq!(
        (answer.0, &answer.1),
        (reference.0, &reference.1),
        "PATCH through sibling issue 2 must be the API's not-found JSON byte for byte"
    );
    assert_eq!(
        stored_body(&w).await,
        COMMENT_BODY,
        "the comment must be unchanged"
    );

    let (status, _) = patch(&w, 1, w.comment_id).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "positive control: PATCH through the comment's own issue edits it"
    );
    assert_eq!(stored_body(&w).await, NEW_BODY, "the edit is stored");
}
