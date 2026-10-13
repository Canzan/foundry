//! fix-issue-attachment-scoping step 01-02 (RCA §4, Defect 2): an attachment is
//! served only through the address of the issue it belongs to.
//!
//! The download URL names a team, a project, an issue and an attachment id. The
//! handler must resolve the issue from the first three and serve the attachment
//! only when it belongs to THAT issue. Any other combination (another team's
//! attachment through your own existing issue, a project or issue that does not
//! exist, the attachment's sibling issue) is the uniform not-found page, byte
//! for byte, with no file bytes.
//!
//! Driven through the real router (driving port) over a real Postgres. No
//! doubles: the notifier has no providers and nothing here sends mail.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::Router;
use foundry_app::{build_router, AppState, Notifier, SystemClock};
use foundry_store::{run_migrations, Store};
use secrecy::SecretString;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::{ContainerAsync, ImageExt};
use tower::ServiceExt;

const CSRF: &str = "attachment-scoping-csrf-token";
const PASSWORD: &str = "attachment-scoping-password";
/// Rita is on team Partners only.
const RITA: &str = "rita@example.com";
/// Yuki is on team Backend only, which owns the attachment.
const YUKI: &str = "yuki@example.com";
const SCREENSHOT: &[u8] = b"\x89PNG\r\n\x1a\nnot-really-a-png-but-distinct-bytes";

struct Harness {
    router: Router,
    _pg: ContainerAsync<Postgres>,
}

/// One workspace, two teams:
/// - Partners (`partners`), Rita's team: project `portal` with issue 1 and no
///   attachments.
/// - Backend (`backend`), Yuki's team: project `api` with issues 1 and 2;
///   `screenshot.png` is attached to issue 1.
struct World {
    h: Harness,
    attachment_id: uuid::Uuid,
}

async fn harness() -> (Harness, Arc<Store>) {
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
    let state = AppState {
        oidc: None,
        store: store.clone(),
        session_secret: Arc::new(SecretString::new(
            "attachment-scoping-secret-thirty-two-bytes-or-more-yes".into(),
        )),
        machine_token_verifier: Arc::new(
            foundry_auth::MachineTokenVerifier::from_public_keys(&[
                "-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEAwtFPs8Jcuncc+E7dXqG/oolI3P6Hamrpd8zVKPvRmg0=\n-----END PUBLIC KEY-----\n"
                    .to_string(),
            ])
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
    };
    (
        Harness {
            router: build_router(state),
            _pg: pg,
        },
        store,
    )
}

async fn exec(store: &Store, sql: &str, binds: &[uuid::Uuid], what: &str) {
    let mut q = sqlx::query(sql);
    for b in binds {
        q = q.bind(*b);
    }
    q.execute(store.pool()).await.expect(what);
}

async fn seed_member(store: &Store, workspace_id: uuid::Uuid, team_id: uuid::Uuid, email: &str) {
    let user_id = uuid::Uuid::now_v7();
    let hash = foundry_auth::hash_password(&SecretString::new(PASSWORD.into()))
        .await
        .expect("hash");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, $2, $3)",
    )
    .bind(user_id)
    .bind(email)
    .bind(&hash)
    .execute(store.pool())
    .await
    .expect("seed user");
    exec(
        store,
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
        &[workspace_id, user_id],
        "seed workspace membership",
    )
    .await;
    exec(
        store,
        "INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'member')",
        &[team_id, user_id],
        "seed team membership",
    )
    .await;
}

/// A team with one project (and its default lanes); returns the project id.
async fn seed_team_project(
    store: &Store,
    workspace_id: uuid::Uuid,
    team_id: uuid::Uuid,
    names: (&str, &str, &str),
) -> uuid::Uuid {
    let (team_slug, project_slug, key_prefix) = names;
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $3)")
        .bind(team_id)
        .bind(workspace_id)
        .bind(team_slug)
        .execute(store.pool())
        .await
        .expect("seed team");
    let project_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, $4, $4, $5)",
    )
    .bind(project_id)
    .bind(team_id)
    .bind(workspace_id)
    .bind(project_slug)
    .bind(key_prefix)
    .execute(store.pool())
    .await
    .expect("seed project");
    exec(
        store,
        "INSERT INTO lanes (id, project_id, workspace_id, slug, label, position)
         SELECT gen_random_uuid(), $1, $2, v.slug, v.label, v.position
           FROM (VALUES ('backlog', 'Backlog', 0), ('todo', 'Todo', 1),
                        ('in_progress', 'In-Progress', 2), ('done', 'Done', 3))
                AS v (slug, label, position)",
        &[project_id, workspace_id],
        "seed lanes",
    )
    .await;
    project_id
}

async fn seed_issue(
    store: &Store,
    workspace_id: uuid::Uuid,
    project_id: uuid::Uuid,
    number: i32,
) -> uuid::Uuid {
    let issue_id = uuid::Uuid::now_v7();
    let author: uuid::Uuid = sqlx::query_scalar("SELECT id FROM users LIMIT 1")
        .fetch_one(store.pool())
        .await
        .expect("an author");
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, description_md,
                             state, position, author_id)
              VALUES ($1, $2, $3, $4, 'seed', '', 'todo', $4, $5)",
    )
    .bind(issue_id)
    .bind(project_id)
    .bind(workspace_id)
    .bind(number)
    .bind(author)
    .execute(store.pool())
    .await
    .expect("seed issue");
    issue_id
}

async fn world() -> World {
    let (h, store) = harness().await;
    let workspace_id = uuid::Uuid::now_v7();
    exec(
        &store,
        "INSERT INTO workspaces (id, name) VALUES ($1, 'Acme')",
        &[workspace_id],
        "seed workspace",
    )
    .await;
    let partners = uuid::Uuid::now_v7();
    let backend = uuid::Uuid::now_v7();
    let portal = seed_team_project(
        &store,
        workspace_id,
        partners,
        ("partners", "portal", "PRT"),
    )
    .await;
    let api = seed_team_project(&store, workspace_id, backend, ("backend", "api", "AUTH")).await;
    seed_member(&store, workspace_id, partners, RITA).await;
    seed_member(&store, workspace_id, backend, YUKI).await;
    seed_issue(&store, workspace_id, portal, 1).await;
    let auth_1 = seed_issue(&store, workspace_id, api, 1).await;
    seed_issue(&store, workspace_id, api, 2).await;

    let attachment_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO issue_attachments (id, issue_id, workspace_id, uploader_id, filename,
                                        content_type, size_bytes, sha256_hex, content)
              SELECT $1, $2, $3, u.id, 'screenshot.png', 'image/png', $4, $5, $6
                FROM users u WHERE u.email_lower = $7",
    )
    .bind(attachment_id)
    .bind(auth_1)
    .bind(workspace_id)
    .bind(SCREENSHOT.len() as i64)
    .bind("0".repeat(64))
    .bind(SCREENSHOT.to_vec())
    .bind(YUKI)
    .execute(store.pool())
    .await
    .expect("seed attachment");
    World { h, attachment_id }
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

async fn get(router: &Router, uri: &str, session: &str) -> (StatusCode, Vec<u8>) {
    let req = Request::builder()
        .method("GET")
        .uri(uri)
        .header(header::COOKIE, session)
        .body(Body::empty())
        .expect("request");
    let resp = router.clone().oneshot(req).await.expect("response");
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("body");
    (status, bytes.to_vec())
}

fn download_uri(team: &str, project: &str, issue: i32, attachment_id: uuid::Uuid) -> String {
    format!("/team/{team}/project/{project}/issues/{issue}/attachments/{attachment_id}")
}

/// The uniform not-found page, as a never-existed team renders it.
async fn uniform_404(router: &Router, session: &str) -> (StatusCode, Vec<u8>) {
    let page = get(
        router,
        &download_uri("no-such-team", "no-such-project", 1, uuid::Uuid::now_v7()),
        session,
    )
    .await;
    assert_eq!(page.0, StatusCode::NOT_FOUND, "the reference page is a 404");
    page
}

fn assert_refused(answer: &(StatusCode, Vec<u8>), uniform: &(StatusCode, Vec<u8>), what: &str) {
    assert_eq!(
        answer.0, uniform.0,
        "{what}: must be the uniform 404 status"
    );
    assert!(
        !answer.1.windows(SCREENSHOT.len()).any(|w| w == SCREENSHOT),
        "{what}: no file bytes may be served"
    );
    assert_eq!(
        answer.1, uniform.1,
        "{what}: must be the uniform not-found page byte for byte"
    );
}

/// T5: Partners' project has an issue 1 (with no attachments), so the address
/// resolves an issue; only the attachment/issue mismatch can refuse it.
#[tokio::test]
async fn another_teams_attachment_through_your_own_existing_issue_is_the_uniform_404() {
    let w = world().await;
    let rita = sign_in(&w.h.router, RITA).await;
    let uniform = uniform_404(&w.h.router, &rita).await;

    let answer = get(
        &w.h.router,
        &download_uri("partners", "portal", 1, w.attachment_id),
        &rita,
    )
    .await;
    assert_refused(
        &answer,
        &uniform,
        "Backend's attachment through Partners' issue 1",
    );
}

/// T6: the address names a project, or an issue, that does not exist.
#[tokio::test]
async fn an_attachment_through_a_project_that_does_not_exist_is_the_uniform_404() {
    let w = world().await;
    let rita = sign_in(&w.h.router, RITA).await;
    let uniform = uniform_404(&w.h.router, &rita).await;

    let no_project = get(
        &w.h.router,
        &download_uri("partners", "no-such-project", 1, w.attachment_id),
        &rita,
    )
    .await;
    assert_refused(&no_project, &uniform, "a project that does not exist");

    let no_issue = get(
        &w.h.router,
        &download_uri("partners", "portal", 99, w.attachment_id),
        &rita,
    )
    .await;
    assert_refused(&no_issue, &uniform, "an issue that does not exist");
}

/// T7: Yuki is on the owning team, but asks through issue 2, a sibling of the
/// issue the attachment belongs to.
#[tokio::test]
async fn an_attachment_through_the_wrong_issue_of_its_own_project_is_the_uniform_404() {
    let w = world().await;
    let yuki = sign_in(&w.h.router, YUKI).await;
    let uniform = uniform_404(&w.h.router, &yuki).await;

    let answer = get(
        &w.h.router,
        &download_uri("backend", "api", 2, w.attachment_id),
        &yuki,
    )
    .await;
    assert_refused(
        &answer,
        &uniform,
        "the attachment through its sibling issue 2",
    );
}

/// T8 (positive control): through its own issue the attachment downloads.
#[tokio::test]
async fn an_attachment_through_its_own_issue_downloads_byte_identical() {
    let w = world().await;
    let yuki = sign_in(&w.h.router, YUKI).await;

    let (status, bytes) = get(
        &w.h.router,
        &download_uri("backend", "api", 1, w.attachment_id),
        &yuki,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the owning issue serves its attachment"
    );
    assert_eq!(
        bytes, SCREENSHOT,
        "the downloaded bytes are the uploaded bytes"
    );
}
