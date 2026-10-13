//! fix-issue-attachment-scoping steps 01-03 and 01-04 (RCA §4, comment
//! sibling): a comment is shown, its edit form is served, and it is edited or
//! deleted only through the address of the issue it belongs to.
//!
//! The URL names a team, a project, an issue and a comment id. The handler
//! resolves the issue from the first three and fetches the comment by its id
//! plus that issue's id. Another team's comment through your own existing
//! issue, an unknown team, project or issue, and the comment's sibling issue
//! are all refused: the single-comment view with the uniform not-found page,
//! byte for byte and with no slug echoed; the edit form with the uniform page
//! for an unresolvable address and with the fragment a nonexistent comment id
//! gets for a comment that is not on the resolved issue. Edit (PATCH) and
//! delete (DELETE) answer the same way as the edit form, change nothing when
//! refused, and the update and soft-delete statements themselves carry the
//! issue predicate.
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

const CSRF: &str = "comment-scoping-csrf-token";
const PASSWORD: &str = "comment-scoping-password";
/// Rita is on team Partners only.
const RITA: &str = "rita@example.com";
/// Yuki is on team Backend only, and wrote the comment.
const YUKI: &str = "yuki@example.com";
/// Ada is a workspace admin on no team: she may moderate (delete) any comment.
const ADA: &str = "ada@example.com";
const COMMENT_BODY: &str = "Backend-only note: rotate the zebra-42 signing key";
const HIJACKED_BODY: &str = "edited through the wrong address";

struct Harness {
    router: Router,
    _pg: ContainerAsync<Postgres>,
}

/// One workspace, two teams:
/// - Partners (`partners`), Rita's team: project `portal` with issue 1 and no
///   comments.
/// - Backend (`backend`), Yuki's team: project `api` with issues 1 and 2;
///   Yuki's comment is on issue 1.
///
/// Ada is a workspace admin on neither team.
struct World {
    h: Harness,
    store: Arc<Store>,
    comment_id: uuid::Uuid,
    api_1: uuid::Uuid,
    api_2: uuid::Uuid,
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
            "comment-scoping-secret-thirty-two-bytes-or-more-yes-ok".into(),
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
    seed_user(&store, workspace_id, ADA, "admin").await;
    seed_issue(&store, workspace_id, portal, 1).await;
    let auth_1 = seed_issue(&store, workspace_id, api, 1).await;
    let auth_2 = seed_issue(&store, workspace_id, api, 2).await;

    let comment_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO comments (id, workspace_id, issue_id, author_id, body_markdown, body_html)
              SELECT $1, $2, $3, u.id, $4, $5
                FROM users u WHERE u.email_lower = $6",
    )
    .bind(comment_id)
    .bind(workspace_id)
    .bind(auth_1)
    .bind(COMMENT_BODY)
    .bind(format!("<p>{COMMENT_BODY}</p>"))
    .bind(YUKI)
    .execute(store.pool())
    .await
    .expect("seed comment");
    World {
        h,
        store,
        comment_id,
        api_1: auth_1,
        api_2: auth_2,
    }
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

/// A state-changing request (PATCH with a form body, or DELETE) carrying the
/// double-submit CSRF cookie and header.
async fn write(
    router: &Router,
    method: &str,
    uri: &str,
    session: &str,
    form: Option<&str>,
) -> (StatusCode, Vec<u8>) {
    let mut req = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::COOKIE, format!("foundry_csrf={CSRF}; {session}"))
        .header("x-csrf-token", CSRF);
    let body = match form {
        Some(body_markdown) => {
            req = req.header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
            Body::from(
                serde_urlencoded::to_string([("body_markdown", body_markdown)])
                    .expect("encode form"),
            )
        }
        None => Body::empty(),
    };
    let resp = router
        .clone()
        .oneshot(req.body(body).expect("request"))
        .await
        .expect("response");
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .expect("body");
    (status, bytes.to_vec())
}

async fn edit(router: &Router, uri: &str, session: &str) -> (StatusCode, Vec<u8>) {
    write(router, "PATCH", uri, session, Some(HIJACKED_BODY)).await
}

async fn delete(router: &Router, uri: &str, session: &str) -> (StatusCode, Vec<u8>) {
    write(router, "DELETE", uri, session, None).await
}

/// The stored comment: its markdown and whether it is tombstoned.
async fn stored_comment(store: &Store, comment_id: uuid::Uuid) -> (String, bool) {
    sqlx::query_as("SELECT body_markdown, deleted_at IS NOT NULL FROM comments WHERE id = $1")
        .bind(comment_id)
        .fetch_one(store.pool())
        .await
        .expect("the comment row")
}

async fn assert_comment_unchanged(w: &World, what: &str) {
    assert_eq!(
        stored_comment(&w.store, w.comment_id).await,
        (COMMENT_BODY.to_string(), false),
        "{what}: the comment must be unchanged and not deleted"
    );
}

fn comment_uri(team: &str, project: &str, issue: i32, comment_id: uuid::Uuid) -> String {
    format!("/team/{team}/project/{project}/issues/{issue}/comments/{comment_id}")
}

fn edit_form_uri(team: &str, project: &str, issue: i32, comment_id: uuid::Uuid) -> String {
    format!("{}/edit", comment_uri(team, project, issue, comment_id))
}

/// The uniform not-found page, as the issue page renders it for a team that
/// never existed.
async fn uniform_404(router: &Router, session: &str) -> (StatusCode, Vec<u8>) {
    let page = get(
        router,
        "/team/no-such-team/project/no-such-project/issues/1",
        session,
    )
    .await;
    assert_eq!(page.0, StatusCode::NOT_FOUND, "the reference page is a 404");
    page
}

fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack
        .windows(needle.len())
        .any(|w| w == needle.as_bytes())
}

fn assert_refused(answer: &(StatusCode, Vec<u8>), expected: &(StatusCode, Vec<u8>), what: &str) {
    assert_eq!(answer.0, expected.0, "{what}: must be the refusal status");
    assert!(
        !contains(&answer.1, COMMENT_BODY),
        "{what}: the comment body must not be shown"
    );
    assert!(
        !contains(&answer.1, YUKI),
        "{what}: the comment author's email must not be shown"
    );
    assert_eq!(
        answer.1, expected.1,
        "{what}: must be the expected refusal byte for byte"
    );
}

/// T9: Partners' project has an issue 1 (with no comments), so the address
/// resolves an issue; only the comment/issue mismatch can refuse it.
#[tokio::test]
async fn another_teams_comment_through_your_own_existing_issue_is_the_uniform_404() {
    let w = world().await;
    let rita = sign_in(&w.h.router, RITA).await;
    let uniform = uniform_404(&w.h.router, &rita).await;

    let answer = get(
        &w.h.router,
        &comment_uri("partners", "portal", 1, w.comment_id),
        &rita,
    )
    .await;
    assert_refused(
        &answer,
        &uniform,
        "Backend's comment through Partners' issue 1",
    );
}

/// The address names a team, a project or an issue that does not exist. The
/// refusal never echoes the requested slug.
#[tokio::test]
async fn a_comment_through_an_unknown_team_project_or_issue_is_the_slug_free_uniform_404() {
    let w = world().await;
    let yuki = sign_in(&w.h.router, YUKI).await;
    let uniform = uniform_404(&w.h.router, &yuki).await;

    for (uri, what) in [
        (
            comment_uri("ghost-team", "api", 1, w.comment_id),
            "a team that does not exist",
        ),
        (
            comment_uri("backend", "ghost-project", 1, w.comment_id),
            "a project that does not exist",
        ),
        (
            comment_uri("backend", "api", 99, w.comment_id),
            "an issue that does not exist",
        ),
    ] {
        let answer = get(&w.h.router, &uri, &yuki).await;
        assert!(
            !contains(&answer.1, "ghost-"),
            "{what}: the requested slug must not be echoed"
        );
        assert_refused(&answer, &uniform, what);
    }
}

/// Yuki is on the owning team and wrote the comment, but asks through issue 2,
/// a sibling of the issue the comment belongs to.
#[tokio::test]
async fn a_comment_through_its_sibling_issue_is_the_uniform_404() {
    let w = world().await;
    let yuki = sign_in(&w.h.router, YUKI).await;
    let uniform = uniform_404(&w.h.router, &yuki).await;

    let answer = get(
        &w.h.router,
        &comment_uri("backend", "api", 2, w.comment_id),
        &yuki,
    )
    .await;
    assert_refused(&answer, &uniform, "the comment through its sibling issue 2");
}

/// The edit form: an unresolvable address is the uniform page; a resolvable
/// issue that does not carry the comment answers exactly as a comment id that
/// never existed does.
#[tokio::test]
async fn the_edit_form_through_an_unknown_team_or_a_sibling_issue_is_refused_byte_exact() {
    let w = world().await;
    let yuki = sign_in(&w.h.router, YUKI).await;
    let uniform = uniform_404(&w.h.router, &yuki).await;

    for (uri, what) in [
        (
            edit_form_uri("ghost-team", "api", 1, w.comment_id),
            "the edit form through a team that does not exist",
        ),
        (
            edit_form_uri("backend", "ghost-project", 1, w.comment_id),
            "the edit form through a project that does not exist",
        ),
        (
            edit_form_uri("backend", "api", 99, w.comment_id),
            "the edit form through an issue that does not exist",
        ),
    ] {
        let answer = get(&w.h.router, &uri, &yuki).await;
        assert!(
            !contains(&answer.1, "ghost-"),
            "{what}: the requested slug must not be echoed"
        );
        assert_refused(&answer, &uniform, what);
    }

    let nonexistent_comment = get(
        &w.h.router,
        &edit_form_uri("backend", "api", 1, uuid::Uuid::now_v7()),
        &yuki,
    )
    .await;
    assert_eq!(
        nonexistent_comment.0,
        StatusCode::NOT_FOUND,
        "the reference fragment is a 404"
    );
    let sibling = get(
        &w.h.router,
        &edit_form_uri("backend", "api", 2, w.comment_id),
        &yuki,
    )
    .await;
    assert_refused(
        &sibling,
        &nonexistent_comment,
        "the edit form through the comment's sibling issue 2",
    );
}

/// Positive control: through its own issue the comment renders, its edit form
/// renders, and the edit form's cancel (a GET of the comment through the same
/// issue path) renders the card again.
#[tokio::test]
async fn a_comment_and_its_edit_form_through_its_own_issue_still_render() {
    let w = world().await;
    let yuki = sign_in(&w.h.router, YUKI).await;
    let own = comment_uri("backend", "api", 1, w.comment_id);

    let (status, card) = get(&w.h.router, &own, &yuki).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the comment renders through its own issue"
    );
    assert!(
        contains(&card, COMMENT_BODY),
        "the card shows the comment body"
    );
    assert!(contains(&card, YUKI), "the card shows the author");

    let (status, form) = get(
        &w.h.router,
        &edit_form_uri("backend", "api", 1, w.comment_id),
        &yuki,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the edit form renders through its own issue"
    );
    assert!(
        contains(&form, COMMENT_BODY),
        "the edit form carries the comment's markdown"
    );
    assert!(
        contains(&form, &own),
        "the edit form's cancel goes back through the comment's own issue path"
    );

    let (status, cancelled) = get(&w.h.router, &own, &yuki).await;
    assert_eq!(status, StatusCode::OK, "cancel re-renders the card");
    assert_eq!(cancelled, card, "cancel renders the same card");
}

/// Yuki wrote the comment and is on its team, but edits and deletes it
/// through issue 2, its sibling. Each answers exactly as a comment id that
/// never existed does on a resolvable issue, and the comment is untouched.
#[tokio::test]
async fn editing_or_deleting_your_comment_through_a_sibling_issue_is_refused_and_changes_nothing() {
    let w = world().await;
    let yuki = sign_in(&w.h.router, YUKI).await;
    let nonexistent = comment_uri("backend", "api", 1, uuid::Uuid::now_v7());
    let sibling = comment_uri("backend", "api", 2, w.comment_id);

    let reference_edit = edit(&w.h.router, &nonexistent, &yuki).await;
    assert_eq!(
        reference_edit.0,
        StatusCode::NOT_FOUND,
        "the reference edit is a 404"
    );
    let answer = edit(&w.h.router, &sibling, &yuki).await;
    assert_refused(&answer, &reference_edit, "editing through sibling issue 2");
    assert_comment_unchanged(&w, "editing through sibling issue 2").await;

    let reference_delete = delete(&w.h.router, &nonexistent, &yuki).await;
    assert_eq!(
        reference_delete.0,
        StatusCode::NOT_FOUND,
        "the reference delete is a 404"
    );
    let answer = delete(&w.h.router, &sibling, &yuki).await;
    assert_refused(
        &answer,
        &reference_delete,
        "deleting through sibling issue 2",
    );
    assert_comment_unchanged(&w, "deleting through sibling issue 2").await;
}

/// Edit and delete through an address that does not resolve an issue are the
/// uniform not-found page, with no slug echoed and nothing changed.
#[tokio::test]
async fn editing_or_deleting_through_an_unknown_team_is_the_slug_free_uniform_404() {
    let w = world().await;
    let yuki = sign_in(&w.h.router, YUKI).await;
    let uniform = uniform_404(&w.h.router, &yuki).await;

    for (uri, what) in [
        (
            comment_uri("ghost-team", "api", 1, w.comment_id),
            "a team that does not exist",
        ),
        (
            comment_uri("backend", "ghost-project", 1, w.comment_id),
            "a project that does not exist",
        ),
        (
            comment_uri("backend", "api", 99, w.comment_id),
            "an issue that does not exist",
        ),
    ] {
        for (answer, verb) in [
            (edit(&w.h.router, &uri, &yuki).await, "edit"),
            (delete(&w.h.router, &uri, &yuki).await, "delete"),
        ] {
            let what = format!("{verb} through {what}");
            assert!(
                !contains(&answer.1, "ghost-"),
                "{what}: the requested slug must not be echoed"
            );
            assert_refused(&answer, &uniform, &what);
            assert_comment_unchanged(&w, &what).await;
        }
    }
}

/// Positive control for admin moderation: Ada, a workspace admin on no team,
/// deletes Yuki's comment through the comment's own issue.
#[tokio::test]
async fn an_admin_not_on_the_team_deletes_through_the_correct_url() {
    let w = world().await;
    let ada = sign_in(&w.h.router, ADA).await;

    let (status, _) = delete(
        &w.h.router,
        &comment_uri("backend", "api", 1, w.comment_id),
        &ada,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "the admin deletes through the comment's own issue"
    );
    assert_eq!(
        stored_comment(&w.store, w.comment_id).await,
        (COMMENT_BODY.to_string(), true),
        "the comment is tombstoned"
    );
}

/// Ada's moderation right does not let her reach the comment through its
/// sibling issue: the answer is a nonexistent comment's and the comment
/// survives.
#[tokio::test]
async fn an_admin_deleting_through_a_sibling_issue_is_refused_and_the_comment_survives() {
    let w = world().await;
    let ada = sign_in(&w.h.router, ADA).await;

    let reference = delete(
        &w.h.router,
        &comment_uri("backend", "api", 1, uuid::Uuid::now_v7()),
        &ada,
    )
    .await;
    assert_eq!(reference.0, StatusCode::NOT_FOUND, "the reference is a 404");
    let answer = delete(
        &w.h.router,
        &comment_uri("backend", "api", 2, w.comment_id),
        &ada,
    )
    .await;
    assert_refused(
        &answer,
        &reference,
        "the admin deleting through sibling issue 2",
    );
    assert_comment_unchanged(&w, "the admin deleting through sibling issue 2").await;
}

/// The update statement itself carries the issue predicate: asked to edit the
/// comment under its sibling issue, it touches no row.
#[tokio::test]
async fn the_update_statement_changes_no_comment_of_another_issue() {
    let w = world().await;
    let (workspace_id, author_id): (uuid::Uuid, uuid::Uuid) =
        sqlx::query_as("SELECT workspace_id, author_id FROM comments WHERE id = $1")
            .bind(w.comment_id)
            .fetch_one(w.store.pool())
            .await
            .expect("the comment row");

    let updated = w
        .store
        .update_comment_with_outbox(
            workspace_id,
            w.api_2,
            w.comment_id,
            HIJACKED_BODY,
            HIJACKED_BODY,
            author_id,
            YUKI,
        )
        .await
        .expect("the update runs");
    assert!(!updated, "no comment of issue 2 matches");
    assert_comment_unchanged(&w, "the update under sibling issue 2").await;

    let updated = w
        .store
        .update_comment_with_outbox(
            workspace_id,
            w.api_1,
            w.comment_id,
            HIJACKED_BODY,
            HIJACKED_BODY,
            author_id,
            YUKI,
        )
        .await
        .expect("the update runs");
    assert!(updated, "positive control: the comment's own issue matches");
}

/// The soft-delete statement itself carries the issue predicate: asked to
/// delete the comment under its sibling issue, it tombstones no row.
#[tokio::test]
async fn the_soft_delete_statement_deletes_no_comment_of_another_issue() {
    let w = world().await;
    let (workspace_id, author_id): (uuid::Uuid, uuid::Uuid) =
        sqlx::query_as("SELECT workspace_id, author_id FROM comments WHERE id = $1")
            .bind(w.comment_id)
            .fetch_one(w.store.pool())
            .await
            .expect("the comment row");

    let deleted = w
        .store
        .soft_delete_comment_with_outbox(workspace_id, w.api_2, w.comment_id, author_id)
        .await
        .expect("the soft-delete runs");
    assert!(!deleted, "no comment of issue 2 matches");
    assert_comment_unchanged(&w, "the soft-delete under sibling issue 2").await;

    let deleted = w
        .store
        .soft_delete_comment_with_outbox(workspace_id, w.api_1, w.comment_id, author_id)
        .await
        .expect("the soft-delete runs");
    assert!(deleted, "positive control: the comment's own issue matches");
}
