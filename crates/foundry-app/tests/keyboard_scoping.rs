//! fix-issue-attachment-scoping (adversarial review M2): the keyboard routes —
//! the new-issue modal and the project search — refuse an unknown team and an
//! unknown project with the uniform not-found page, byte for byte the page a
//! signed-in non-member of an existing team gets, and with no slug echoed. A
//! page that names the slug, or differs from the non-member's, tells an outsider
//! which teams and projects exist.
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

const CSRF: &str = "keyboard-scoping-csrf-token";
const PASSWORD: &str = "keyboard-scoping-password";
/// Rita is on team Partners only.
const RITA: &str = "rita@example.com";
const UNKNOWN_TEAM: &str = "zebra-team-42";
const UNKNOWN_PROJECT: &str = "zebra-project-42";

struct Harness {
    router: Router,
    _pg: ContainerAsync<Postgres>,
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
            "keyboard-scoping-secret-thirty-two-bytes-or-more-yes-ok".into(),
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

async fn seed_user(store: &Store, workspace_id: uuid::Uuid, email: &str, role: &str) -> uuid::Uuid {
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
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, $3)",
    )
    .bind(workspace_id)
    .bind(user_id)
    .bind(role)
    .execute(store.pool())
    .await
    .expect("seed workspace membership");
    user_id
}

async fn seed_member(store: &Store, workspace_id: uuid::Uuid, team_id: uuid::Uuid, email: &str) {
    let user_id = seed_user(store, workspace_id, email, "member").await;
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

fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|w| w == needle.as_bytes())
}

/// One workspace, two teams: Partners (`partners`, project `portal`), Rita's
/// team; Backend (`backend`, project `api`), which Rita is not on.
async fn world() -> (Harness, String) {
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
    seed_team_project(
        &store,
        workspace_id,
        partners,
        ("partners", "portal", "PRT"),
    )
    .await;
    seed_team_project(&store, workspace_id, backend, ("backend", "api", "AUTH")).await;
    seed_member(&store, workspace_id, partners, RITA).await;
    let rita = sign_in(&h.router, RITA).await;
    (h, rita)
}

/// For `route` (`search?q=a` or `issues/new`): an unknown team and an unknown
/// project in Rita's own team are refused exactly as Backend's existing
/// project, which Rita may not see, is refused.
async fn assert_unknown_addresses_are_the_non_member_page(route: &str) {
    let (h, rita) = world().await;
    let non_member = get(
        &h.router,
        &format!("/team/backend/project/api/{route}"),
        &rita,
    )
    .await;
    assert_eq!(
        non_member.0,
        StatusCode::NOT_FOUND,
        "{route}: a non-member is refused with a 404"
    );
    for (what, uri) in [
        (
            "an unknown team",
            format!("/team/{UNKNOWN_TEAM}/project/api/{route}"),
        ),
        (
            "an unknown project",
            format!("/team/partners/project/{UNKNOWN_PROJECT}/{route}"),
        ),
    ] {
        let answer = get(&h.router, &uri, &rita).await;
        assert_eq!(answer.0, StatusCode::NOT_FOUND, "{route}, {what}: a 404");
        for slug in [UNKNOWN_TEAM, UNKNOWN_PROJECT] {
            assert!(
                !contains(&answer.1, slug),
                "{route}, {what}: the slug {slug} must not be echoed"
            );
        }
        assert_eq!(
            answer.1, non_member.1,
            "{route}, {what}: must be the non-member's page byte for byte"
        );
    }
}

#[tokio::test]
async fn search_refuses_an_unknown_team_or_project_with_the_non_members_page() {
    assert_unknown_addresses_are_the_non_member_page("search?q=a").await;
}

#[tokio::test]
async fn the_new_issue_modal_refuses_an_unknown_team_or_project_with_the_non_members_page() {
    assert_unknown_addresses_are_the_non_member_page("issues/new").await;
}
