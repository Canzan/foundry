//! card-ranking-within-status (ADR-001/002) — the net-new
//! `reposition_issue_with_outbox` write path, pinned directly at the store
//! boundary for the two invariants the HTTP acceptance suite CANNOT observe:
//!
//! - **R1 contiguity**: after ANY move, BOTH the source `(project, old_state)`
//!   and the target `(project, new_state)` columns are a contiguous `0..N-1`
//!   permutation — reindexed in the SAME transaction.
//! - **R2 conditional emit (ODD-4)**: a within-status reorder writes `position`
//!   with NO `IssueUpdated` outbox row (state unchanged); a cross-status drop
//!   emits EXACTLY ONE (state changed). Broadcasting a pure reorder would shove
//!   other viewers' cards to the column end, so the emit is gated on the state
//!   actually changing.
//!
//! Runs against a real Postgres (testcontainers, @real-io): the reindex SQL, the
//! ordered read, and the outbox row can't be faked. Two behaviours (the two emit
//! branches); budget = 2 × 2 = 4, 2 written.

use foundry_store::{run_migrations, RepositionOutcome, Store};
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

/// Seed a workspace + team + member + project; return `(project_id, author_id)`.
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
              VALUES ($1, $2, $3, 'Author', 'x')",
    )
    .bind(user_id)
    .bind(format!("author@{key_prefix}.test"))
    .bind(format!("author@{key_prefix}.test"))
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

/// Insert an issue at an explicit `(state, position)` so the pre-move order is
/// deterministic (independent of the number-DESC default).
async fn seed_issue_at(
    store: &Store,
    project_id: uuid::Uuid,
    author_id: uuid::Uuid,
    number: i32,
    state: &str,
    position: i32,
) {
    let workspace_id: (uuid::Uuid,) =
        sqlx::query_as("SELECT workspace_id FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_one(store.pool())
            .await
            .expect("fetch project workspace");
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, description_md, state, position, author_id)
              VALUES ($1, $2, $3, $4, 'seed', '', $5, $6, $7)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(project_id)
    .bind(workspace_id.0)
    .bind(number)
    .bind(state)
    .bind(position)
    .bind(author_id)
    .execute(store.pool())
    .await
    .expect("insert issue");
}

/// The `(number, position)` pairs of a column, ordered by `position`.
async fn column_order(store: &Store, project_id: uuid::Uuid, state: &str) -> Vec<(i32, i32)> {
    sqlx::query_as(
        "SELECT number, position FROM issues
          WHERE project_id = $1 AND state = $2
          ORDER BY position ASC",
    )
    .bind(project_id)
    .bind(state)
    .fetch_all(store.pool())
    .await
    .expect("read column order")
}

async fn issue_updated_count(store: &Store) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM outbox WHERE event_type = 'IssueUpdated'")
        .fetch_one(store.pool())
        .await
        .expect("count outbox rows")
}

/// R1 + R2 — a WITHIN-status reorder reindexes the column to a contiguous
/// `0..N-1` in the new order and emits NO outbox row (state unchanged).
#[tokio::test]
async fn reposition_within_status_reindexes_contiguously_without_emit() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let (project, author) = seed_project(&store, "Acme", "GEN").await;

    // todo: [10, 20, 30] at positions 0,1,2.
    seed_issue_at(&store, project, author, 10, "todo", 0).await;
    seed_issue_at(&store, project, author, 20, "todo", 1).await;
    seed_issue_at(&store, project, author, 30, "todo", 2).await;

    // Drop GEN-30 immediately after GEN-10 → new order [10, 30, 20].
    let outcome = store
        .reposition_issue_with_outbox("GEN", 30, "todo", Some(10), author)
        .await
        .expect("reposition query succeeds");
    assert_eq!(outcome, RepositionOutcome::Repositioned);

    assert_eq!(
        column_order(&store, project, "todo").await,
        vec![(10, 0), (30, 1), (20, 2)],
        "the todo column must be a contiguous 0..N-1 permutation in the new order"
    );
    assert_eq!(
        issue_updated_count(&store).await,
        0,
        "a pure within-status reorder must emit NO IssueUpdated outbox row (ODD-4)"
    );
}

/// R1 + R2 — a CROSS-status drop moves the card (state + rank), reindexes BOTH
/// the source and target columns to contiguous `0..N-1`, and emits EXACTLY ONE
/// outbox row (state changed).
#[tokio::test]
async fn reposition_across_status_reindexes_both_columns_and_emits_once() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let (project, author) = seed_project(&store, "Acme", "GEN").await;

    // todo: [10, 20]; backlog: [5, 6].
    seed_issue_at(&store, project, author, 10, "todo", 0).await;
    seed_issue_at(&store, project, author, 20, "todo", 1).await;
    seed_issue_at(&store, project, author, 5, "backlog", 0).await;
    seed_issue_at(&store, project, author, 6, "backlog", 1).await;

    // Drop GEN-5 from backlog INTO todo, after GEN-10 → todo [10, 5, 20].
    let outcome = store
        .reposition_issue_with_outbox("GEN", 5, "todo", Some(10), author)
        .await
        .expect("reposition query succeeds");
    assert_eq!(outcome, RepositionOutcome::Repositioned);

    assert_eq!(
        column_order(&store, project, "todo").await,
        vec![(10, 0), (5, 1), (20, 2)],
        "the target column must splice the moved card at neighbour+1, contiguous 0..N-1"
    );
    assert_eq!(
        column_order(&store, project, "backlog").await,
        vec![(6, 0)],
        "the source column must close the gap, contiguous 0..N-1"
    );
    assert_eq!(
        issue_updated_count(&store).await,
        1,
        "a cross-status drop (state changed) must emit EXACTLY ONE IssueUpdated row"
    );
}
