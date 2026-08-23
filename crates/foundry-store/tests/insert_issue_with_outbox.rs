//! new-issue-dialog-description (step 01-01) — the store seam.
//!
//! `insert_issue_with_outbox` gains a `description: &str` param and widens its
//! existing in-transaction INSERT to carry `description_md`. This is the walking
//! skeleton's FIRST failing test (bottom-up): prove the store persists a supplied
//! description BEFORE the vertical thread (step 01-02+) rides the seam.
//!
//! Three contracts, pinned directly at the boundary against a real Postgres
//! (testcontainers, @real-io) — the INSERT and the workspace scoping can't be
//! faked. Each test runs the full production migration set on a fresh container.
//!
//! - a SUPPLIED description is persisted verbatim (`description_md` reads back equal);
//! - passing `""` writes `description_md = ""` byte-identically to today;
//! - the new param does NOT loosen workspace scoping — a create lands in the
//!   acting workspace exactly as before, and a foreign workspace stays empty.

use foundry_store::{run_migrations, Store};
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::ImageExt;

async fn fresh_postgres() -> (
    String,
    testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
) {
    let container = Postgres::default()
        .with_tag("16-alpine")
        .start()
        .await
        .expect("start postgres container");
    let host = container.get_host().await.expect("container host");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("container port");
    let base = format!("postgres://postgres:postgres@{host}:{port}/postgres");
    (base, container)
}

async fn migrated_store(base: &str) -> Store {
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(10))
        .connect(base)
        .await
        .expect("connect pool");
    run_migrations(&pool).await.expect("run migrations");
    Store::from_pool(pool)
}

/// Seed a workspace + team + member + project and return
/// `(workspace_id, project_id, author_id)`. The member authors the issues
/// (issues.author_id is NOT NULL).
async fn seed_project(
    store: &Store,
    workspace_name: &str,
    key_prefix: &str,
) -> (uuid::Uuid, uuid::Uuid, uuid::Uuid) {
    let workspace_id = uuid::Uuid::now_v7();
    let user_id = uuid::Uuid::now_v7();
    let team_id = uuid::Uuid::now_v7();
    let project_id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(workspace_id)
        .bind(workspace_name)
        .execute(store.pool())
        .await
        .expect("insert workspace");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, 'x')",
    )
    .bind(user_id)
    .bind(format!("author@{key_prefix}.test"))
    .bind(format!("author@{key_prefix}.test"))
    .bind("Author")
    .execute(store.pool())
    .await
    .expect("insert user");
    sqlx::query(
        "INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'General', 'general')",
    )
    .bind(team_id)
    .bind(workspace_id)
    .execute(store.pool())
    .await
    .expect("insert team");
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, 'Sandbox', 'sandbox', $4)",
    )
    .bind(project_id)
    .bind(team_id)
    .bind(workspace_id)
    .bind(key_prefix)
    .execute(store.pool())
    .await
    .expect("insert project");
    seed_lanes(store, project_id, workspace_id).await;
    (workspace_id, project_id, user_id)
}

/// board-lane-management sweep: raw-SQL project fixtures need lane rows —
/// post-0015 the composite FK `fk_issues_lane` refuses a laneless landing.
async fn seed_lanes(store: &Store, project_id: uuid::Uuid, workspace_id: uuid::Uuid) {
    sqlx::query(
        "INSERT INTO lanes (id, project_id, workspace_id, slug, label, position)
         SELECT gen_random_uuid(), $1, $2, v.slug, v.label, v.position
           FROM (VALUES ('backlog', 'Backlog', 0), ('todo', 'Todo', 1),
                        ('in_progress', 'In-Progress', 2), ('done', 'Done', 3))
                AS v (slug, label, position)
             ON CONFLICT (project_id, slug) DO NOTHING",
    )
    .bind(project_id)
    .bind(workspace_id)
    .execute(store.pool())
    .await
    .expect("seed lanes for raw-SQL project fixture");
}

async fn read_description(store: &Store, project_id: uuid::Uuid, number: i32) -> String {
    sqlx::query_scalar("SELECT description_md FROM issues WHERE project_id = $1 AND number = $2")
        .bind(project_id)
        .bind(number)
        .fetch_one(store.pool())
        .await
        .expect("read description_md")
}

/// A SUPPLIED description is persisted verbatim: read the freshly-minted row
/// back and `description_md` equals what was passed.
#[tokio::test]
async fn insert_issue_persists_supplied_description() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;

    let (workspace_id, project_id, author_id) = seed_project(&store, "Acme", "GEN").await;
    let body = "A **bold** ask\n\n- one\n- two";

    let number = store
        .insert_issue_with_outbox(
            uuid::Uuid::now_v7(),
            workspace_id,
            project_id,
            "GEN",
            author_id,
            "Ship it",
            body,
        )
        .await
        .expect("insert succeeds");

    assert_eq!(
        read_description(&store, project_id, number).await,
        body,
        "description_md must persist the supplied value verbatim"
    );
}

/// The empty-description path writes `description_md = ""` byte-identically to
/// today's behaviour (every caller passes `""` this step).
#[tokio::test]
async fn insert_issue_persists_empty_description_byte_identically() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;

    let (workspace_id, project_id, author_id) = seed_project(&store, "Acme", "GEN").await;

    let number = store
        .insert_issue_with_outbox(
            uuid::Uuid::now_v7(),
            workspace_id,
            project_id,
            "GEN",
            author_id,
            "Ship it",
            "",
        )
        .await
        .expect("insert succeeds");

    assert_eq!(
        read_description(&store, project_id, number).await,
        "",
        "an empty description must persist as the empty string"
    );
}

/// The new param does NOT loosen workspace scoping: a create lands in the acting
/// workspace exactly as before, and a foreign workspace sees no issue.
#[tokio::test]
async fn insert_issue_is_scoped_to_the_acting_workspace() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;

    let (acme_ws, acme_project, acme_author) = seed_project(&store, "Acme", "GEN").await;
    let (globex_ws, _globex_project, _globex_author) = seed_project(&store, "Globex", "FGN").await;

    let number = store
        .insert_issue_with_outbox(
            uuid::Uuid::now_v7(),
            acme_ws,
            acme_project,
            "GEN",
            acme_author,
            "Ship it",
            "body",
        )
        .await
        .expect("insert succeeds");

    let row_workspace: uuid::Uuid =
        sqlx::query_scalar("SELECT workspace_id FROM issues WHERE project_id = $1 AND number = $2")
            .bind(acme_project)
            .bind(number)
            .fetch_one(store.pool())
            .await
            .expect("read workspace_id");
    assert_eq!(
        row_workspace, acme_ws,
        "the created issue must belong to the acting workspace"
    );

    let foreign_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM issues WHERE workspace_id = $1")
            .bind(globex_ws)
            .fetch_one(store.pool())
            .await
            .expect("count foreign issues");
    assert_eq!(
        foreign_count, 0,
        "a create scoped to Acme must never appear under a foreign workspace"
    );
}
