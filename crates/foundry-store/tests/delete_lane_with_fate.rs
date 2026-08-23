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
//! Test budget: 1 integration test for the concurrency behaviour of AC-5;
//! the five acceptance scenarios own the remaining behaviours. Three adapter
//! integration tests added at DELIVER Phase 5 (Mandate 4 — adapters are
//! tested against real infrastructure): mutation testing showed the
//! delete-fate batch DELETE, the move-fate position append math, and the
//! bounded-retry envelope were unreachable from the acceptance lane's
//! observable surface (the @real-io trap).

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

/// Delete fate at the store boundary: the batch `DELETE … WHERE id = ANY`
/// really removes the captured members (the lane-row delete would otherwise
/// abort on the composite-FK strand-guard), and the outcome reports the TRUE
/// deleted count. Kills `delete_cards_permanently` → `Ok(0)`/`Ok(1)` (with
/// the batch DELETE gone, the transaction can only fail or lie about counts).
#[tokio::test]
async fn delete_fate_removes_the_captured_cards_and_reports_the_true_count() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let (project, operator) = seed_project(&store).await;
    seed_issue_at(&store, project, operator, 10, "todo", 0).await;
    seed_issue_at(&store, project, operator, 20, "todo", 1).await;
    seed_issue_at(&store, project, operator, 30, "done", 0).await;

    let outcome = store
        .delete_lane_with_fate(project, "todo", LaneDeleteFate::DeleteCards, operator)
        .await
        .expect("a delete-fate confirm on a populated lane must not be a store error");

    assert!(
        matches!(
            outcome,
            LaneDeleteOutcome::Deleted {
                moved: 0,
                deleted: 2
            }
        ),
        "the outcome must report 0 moved / 2 deleted; got {outcome:?}"
    );
    let survivors: Vec<(i32, String)> =
        sqlx::query_as("SELECT number, state FROM issues WHERE project_id = $1 ORDER BY number")
            .bind(project)
            .fetch_all(store.pool())
            .await
            .expect("read issues");
    assert_eq!(
        survivors,
        vec![(30, "done".to_string())],
        "the dying lane's cards must be gone; other lanes' cards untouched"
    );
    let (lane_rows,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM lanes WHERE project_id = $1 AND slug = 'todo'")
            .bind(project)
            .fetch_one(store.pool())
            .await
            .expect("count dying lane");
    assert_eq!(lane_rows, 0, "the lane row itself must be deleted");
}

/// Move fate position math (data-models.md §3-§4): the dying lane's cards
/// append at the destination's BOTTOM — positions `C..C+N-1` in the captured
/// `(position ASC, number DESC)` order, `C` the destination's occupied count —
/// with one outbox row per moved card. Kills the `occupied + index` → `-`/`*`
/// arithmetic mutants (either collides with the destination's own card at
/// position 0, breaking the 0012 contiguity permutation).
#[tokio::test]
async fn move_fate_appends_at_the_destination_bottom_in_captured_order() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let (project, operator) = seed_project(&store).await;
    // Destination already occupied: in_progress holds GEN-30 at position 0.
    seed_issue_at(&store, project, operator, 30, "in_progress", 0).await;
    // Dying lane todo: captured order (position ASC, number DESC) = 10, 20.
    seed_issue_at(&store, project, operator, 10, "todo", 0).await;
    seed_issue_at(&store, project, operator, 20, "todo", 1).await;

    let outcome = store
        .delete_lane_with_fate(
            project,
            "todo",
            LaneDeleteFate::MoveTo {
                destination_slug: "in_progress",
            },
            operator,
        )
        .await
        .expect("a move-fate confirm to a live destination must not be a store error");

    assert!(
        matches!(
            outcome,
            LaneDeleteOutcome::Deleted {
                moved: 2,
                deleted: 0
            }
        ),
        "the outcome must report 2 moved / 0 deleted; got {outcome:?}"
    );
    let placed: Vec<(i32, String, i32)> = sqlx::query_as(
        "SELECT number, state, position FROM issues WHERE project_id = $1 ORDER BY number",
    )
    .bind(project)
    .fetch_all(store.pool())
    .await
    .expect("read issues");
    assert_eq!(
        placed,
        vec![
            (10, "in_progress".to_string(), 1),
            (20, "in_progress".to_string(), 2),
            (30, "in_progress".to_string(), 0),
        ],
        "moved cards must APPEND at the destination bottom (positions C..C+N-1 \
         in captured order) below the destination's own card at position 0"
    );
    let (outbox_rows,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM outbox WHERE event_type = 'IssueUpdated'")
            .fetch_one(store.pool())
            .await
            .expect("count outbox rows");
    assert_eq!(outbox_rows, 2, "one IssueUpdated outbox row per MOVED card");
}

/// The bounded-retry envelope's honest-error arm: a PERSISTENT store failure
/// (modelled as the issues table vanishing) surfaces promptly as an error —
/// never an endless retry, never a fabricated outcome. Kills the retry-guard
/// → `true` mutant (which retries every error forever; under it this test
/// hangs into the mutation timeout).
#[tokio::test]
async fn persistent_store_failure_is_an_honest_error_not_an_endless_retry() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let (project, operator) = seed_project(&store).await;
    sqlx::query("DROP TABLE issues CASCADE")
        .execute(store.pool())
        .await
        .expect("model a persistent store failure");

    let outcome = store
        .delete_lane_with_fate(project, "todo", LaneDeleteFate::DeleteCards, operator)
        .await;

    assert!(
        outcome.is_err(),
        "a persistent failure must surface as an honest store error; got {outcome:?}"
    );
}

/// The dialog's advisory-count read at the store boundary: the LIVE count of
/// the cards a lane holds — 2 for the seeded lane, 0 for an empty one (never
/// a canned value). Added at DELIVER Phase 5: the services-layer killer for
/// `count_issues_in_lane` cannot run for foundry-store mutants (cargo-mutants
/// 25.3.1 tests the mutated package only).
#[tokio::test]
async fn count_issues_in_lane_reports_the_live_per_lane_count() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let (project, operator) = seed_project(&store).await;
    seed_issue_at(&store, project, operator, 10, "todo", 0).await;
    seed_issue_at(&store, project, operator, 20, "todo", 1).await;

    let todo = store
        .count_issues_in_lane(project, "todo")
        .await
        .expect("count a populated lane");
    let done = store
        .count_issues_in_lane(project, "done")
        .await
        .expect("count an empty lane");
    assert_eq!(
        (todo, done),
        (2, 0),
        "the advisory count must be the LIVE per-lane card count"
    );
}
