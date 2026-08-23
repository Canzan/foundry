//! board-lane-management 04-01 — `delete_lane_with_fate`, pinned at the store
//! boundary for the ONE interleaving the HTTP acceptance lane cannot drive:
//! two operators deleting two lanes INTO each other concurrently (Earned
//! Trust gold test 2, architecture-design.md §8 / ADR-BOARD-LANE-002).
//!
//! The pinned contract (data-models.md §5 race matrix, READ COMMITTED): the
//! crossing deletes resolve CLEANLY — exactly one wins (`Deleted`), the loser
//! is refused whole (`DestinationNotFound`: its chosen destination died under
//! it) or, on a broken deadlock, retried by the bounded loop and then refused.
//! NEVER a partial apply: zero laneless cards, zero vanished cards, every
//! card in the one surviving lane. Runs against a real Postgres
//! (testcontainers, @real-io): `FOR UPDATE` ordering, the composite-FK
//! strand-guard, and deadlock detection cannot be faked in memory.
//!
//! Test budget: this is 1 integration test for the concurrency behaviour of
//! AC-5; the five acceptance scenarios own the remaining behaviours.

use foundry_store::{LaneDeleteFate, LaneDeleteOutcome, Store};
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
    foundry_store::run_migrations(&pool)
        .await
        .expect("run migrations");
    Store::from_pool(pool)
}

/// Seed a workspace + team + project with the four grandfathered lanes;
/// return `(project_id, operator_id)`.
async fn seed_project(store: &Store) -> (uuid::Uuid, uuid::Uuid) {
    let workspace_id = uuid::Uuid::now_v7();
    let user_id = uuid::Uuid::now_v7();
    let team_id = uuid::Uuid::now_v7();
    let project_id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'Acme')")
        .bind(workspace_id)
        .execute(store.pool())
        .await
        .expect("insert workspace");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, 'op@fate.test', 'op@fate.test', 'Operator', 'x')",
    )
    .bind(user_id)
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
              VALUES ($1, $2, $3, 'Sandbox', 'sandbox', 'GEN')",
    )
    .bind(project_id)
    .bind(team_id)
    .bind(workspace_id)
    .execute(store.pool())
    .await
    .expect("insert project");
    sqlx::query(
        "INSERT INTO lanes (id, project_id, workspace_id, slug, label, position)
         SELECT gen_random_uuid(), $1, $2, v.slug, v.label, v.position
           FROM (VALUES ('backlog', 'Backlog', 0), ('todo', 'Todo', 1),
                        ('in_progress', 'In-Progress', 2), ('done', 'Done', 3))
                AS v (slug, label, position)",
    )
    .bind(project_id)
    .bind(workspace_id)
    .execute(store.pool())
    .await
    .expect("seed lanes");
    (project_id, user_id)
}

async fn seed_issue_at(
    store: &Store,
    project_id: uuid::Uuid,
    author_id: uuid::Uuid,
    number: i32,
    state: &str,
    position: i32,
) {
    let (workspace_id,): (uuid::Uuid,) =
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
    .bind(workspace_id)
    .bind(number)
    .bind(state)
    .bind(position)
    .bind(author_id)
    .execute(store.pool())
    .await
    .expect("insert issue");
}

/// Earned Trust gold test 2 — crossing A↔B concurrent deletes resolve
/// cleanly: exactly one `Deleted`, the loser refused whole; zero laneless
/// cards, zero lost cards, all cards in the one surviving lane. Never partial.
#[tokio::test]
async fn crossing_concurrent_lane_deletes_resolve_cleanly_never_partially() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let (project, operator) = seed_project(&store).await;

    // todo: [10, 20]; in_progress: [30].
    seed_issue_at(&store, project, operator, 10, "todo", 0).await;
    seed_issue_at(&store, project, operator, 20, "todo", 1).await;
    seed_issue_at(&store, project, operator, 30, "in_progress", 0).await;

    // Two operators, two crossing move-fate deletes, launched concurrently.
    let delete_todo_into_in_progress = store.delete_lane_with_fate(
        project,
        "todo",
        LaneDeleteFate::MoveTo {
            destination_slug: "in_progress",
        },
        operator,
    );
    let delete_in_progress_into_todo = store.delete_lane_with_fate(
        project,
        "in_progress",
        LaneDeleteFate::MoveTo {
            destination_slug: "todo",
        },
        operator,
    );
    let (a, b) = tokio::join!(delete_todo_into_in_progress, delete_in_progress_into_todo);
    let a = a.expect("crossing delete A must resolve without a store error (never a 500)");
    let b = b.expect("crossing delete B must resolve without a store error (never a 500)");

    // Exactly one wins; the loser is refused WHOLE — its destination died
    // under it (DestinationNotFound) — never half-applied.
    let wins = [&a, &b]
        .iter()
        .filter(|o| matches!(o, LaneDeleteOutcome::Deleted { .. }))
        .count();
    assert_eq!(
        wins, 1,
        "exactly one crossing delete must win; outcomes: A={a:?}, B={b:?}"
    );
    assert!(
        [&a, &b]
            .iter()
            .any(|o| matches!(o, LaneDeleteOutcome::DestinationNotFound)),
        "the loser must be refused cleanly (its destination is gone); A={a:?}, B={b:?}"
    );

    // Never partial: all 3 cards survive, none laneless, all in the survivor.
    let (laneless,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM issues i
          WHERE NOT EXISTS (SELECT 1 FROM lanes l
                             WHERE l.project_id = i.project_id AND l.slug = i.state)",
    )
    .fetch_one(store.pool())
    .await
    .expect("laneless probe");
    assert_eq!(laneless, 0, "zero-laneless guard must hold after the race");

    let states: Vec<(i32, String)> =
        sqlx::query_as("SELECT number, state FROM issues WHERE project_id = $1 ORDER BY number")
            .bind(project)
            .fetch_all(store.pool())
            .await
            .expect("read issues");
    assert_eq!(states.len(), 3, "a move-fate race may never lose a card");
    let survivor = if matches!(a, LaneDeleteOutcome::Deleted { .. }) {
        "in_progress" // A won: todo died, its cards moved into in_progress
    } else {
        "todo" // B won: in_progress died, its card moved into todo
    };
    for (number, state) in &states {
        assert_eq!(
            state, survivor,
            "GEN-{number} must sit in the surviving lane {survivor}; got {state}"
        );
    }
    let (dead_lanes,): (i64,) = sqlx::query_as(
        "SELECT count(*) FROM lanes WHERE project_id = $1 AND slug IN ('todo', 'in_progress')",
    )
    .bind(project)
    .fetch_one(store.pool())
    .await
    .expect("count contested lanes");
    assert_eq!(
        dead_lanes, 1,
        "exactly one of the two contested lanes must remain"
    );
}
