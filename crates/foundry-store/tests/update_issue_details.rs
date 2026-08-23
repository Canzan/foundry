//! issue-edit-dialog (ADR-002) — the net-new `update_issue_details` write path.
//!
//! Pins the store contract the edit dialog rests on directly at the boundary:
//!
//! - `update_issue_details` persists BOTH the new title and the new
//!   description_md (and bumps `updated_at`) for an issue addressed by
//!   `key_prefix + number` — last-write-wins, no outbox row (ODD-3/ODD-4);
//! - a scoped call NEVER updates a same-numbered issue that lives under a
//!   DIFFERENT project/workspace (tenant isolation — the crux of ADR-002/003);
//! - an absent issue resolves to `Ok(None)` (the service maps this to the
//!   uniform non-enumerable NotFound).
//!
//! Runs against a real Postgres (testcontainers, @real-io): the JOIN scoping by
//! `key_prefix`, the UPDATE, and the tenant boundary can't be faked. Each test
//! runs the full production migration set on a fresh container.

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

/// Seed a workspace + team + member + project and return `(project_id, author_id)`.
/// The member authors the issues (issues.author_id is NOT NULL).
async fn seed_project(
    store: &Store,
    workspace_name: &str,
    key_prefix: &str,
) -> (uuid::Uuid, uuid::Uuid) {
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
    // board-lane-management sweep: raw-SQL project fixtures need lane rows
    // (post-0015 fk_issues_lane refuses a laneless issue INSERT).
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
    (project_id, user_id)
}

/// Insert an issue numbered `number` under `project_id`, authored by `author_id`.
async fn seed_issue(
    store: &Store,
    project_id: uuid::Uuid,
    author_id: uuid::Uuid,
    number: i32,
    title: &str,
    description_md: &str,
) {
    let workspace_id: (uuid::Uuid,) =
        sqlx::query_as("SELECT workspace_id FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_one(store.pool())
            .await
            .expect("fetch project workspace");
    // board-lane-management sweep: 0015 dropped the state DEFAULT — INSERT it.
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, description_md, state, author_id)
              VALUES ($1, $2, $3, $4, $5, $6, 'backlog', $7)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(project_id)
    .bind(workspace_id.0)
    .bind(number)
    .bind(title)
    .bind(description_md)
    .bind(author_id)
    .execute(store.pool())
    .await
    .expect("insert issue");
}

async fn read_issue(store: &Store, project_id: uuid::Uuid, number: i32) -> (String, String) {
    sqlx::query_as("SELECT title, description_md FROM issues WHERE project_id = $1 AND number = $2")
        .bind(project_id)
        .bind(number)
        .fetch_one(store.pool())
        .await
        .expect("read issue")
}

/// A scoped `update_issue_details` persists BOTH fields for the addressed issue
/// and touches NO same-numbered issue under a foreign project key — the
/// write-side proof of ADR-002 last-write-wins + the key-scoped isolation the
/// store JOIN provides (mirrors `reposition_issue_with_outbox`).
#[tokio::test]
async fn update_issue_details_persists_both_fields_and_is_tenant_isolated() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;

    // Acme "GEN-1" is the target. Globex has its OWN "FGN-1" (a foreign
    // workspace + a distinct key_prefix) numbered identically — the isolation
    // trap: a call scoped to "GEN" must never reach across to "FGN".
    let (acme_project, acme_author) = seed_project(&store, "Acme", "GEN").await;
    seed_issue(
        &store,
        acme_project,
        acme_author,
        1,
        "Old title",
        "old body",
    )
    .await;
    let (globex_project, globex_author) = seed_project(&store, "Globex", "FGN").await;
    seed_issue(
        &store,
        globex_project,
        globex_author,
        1,
        "Globex title",
        "globex body",
    )
    .await;

    let before_updated_at: time::OffsetDateTime =
        sqlx::query_scalar("SELECT updated_at FROM issues WHERE project_id = $1 AND number = 1")
            .bind(acme_project)
            .fetch_one(store.pool())
            .await
            .expect("read updated_at");

    let outcome = store
        .update_issue_details("GEN", 1, "New title", "new body", acme_author)
        .await
        .expect("update succeeds");
    assert_eq!(
        outcome,
        Some(()),
        "an existing issue update reports Some(())"
    );

    // Both fields persisted for the target.
    let (title, description) = read_issue(&store, acme_project, 1).await;
    assert_eq!(title, "New title", "title must be persisted");
    assert_eq!(description, "new body", "description_md must be persisted");

    // updated_at bumped.
    let after_updated_at: time::OffsetDateTime =
        sqlx::query_scalar("SELECT updated_at FROM issues WHERE project_id = $1 AND number = 1")
            .bind(acme_project)
            .fetch_one(store.pool())
            .await
            .expect("read updated_at");
    assert!(
        after_updated_at >= before_updated_at,
        "updated_at must be bumped by the edit"
    );

    // The foreign FGN-1 (different workspace + key_prefix) must be byte-for-byte
    // untouched by a call scoped to "GEN": the JOIN's `key_prefix = 'GEN'` clause
    // never selects it.
    let globex_after = read_issue(&store, globex_project, 1).await;
    assert_eq!(
        globex_after,
        ("Globex title".to_string(), "globex body".to_string()),
        "a foreign-key issue must be untouched by a GEN-scoped call (isolation)"
    );
}

/// An absent issue resolves to `Ok(None)` — the service maps this to the
/// uniform non-enumerable NotFound, and nothing is written.
#[tokio::test]
async fn update_issue_details_reports_none_for_absent_issue() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;

    let (_project, author) = seed_project(&store, "Acme", "GEN").await;

    let outcome = store
        .update_issue_details("GEN", 999, "Whatever", "body", author)
        .await
        .expect("update query succeeds");
    assert_eq!(
        outcome, None,
        "an absent issue reports None, writes nothing"
    );
}
