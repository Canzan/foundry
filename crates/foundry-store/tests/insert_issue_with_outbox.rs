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
/// (issues.author_id is NOT NULL). Seeds the grandfather lane rows too.
async fn seed_project(
    store: &Store,
    workspace_name: &str,
    key_prefix: &str,
) -> (uuid::Uuid, uuid::Uuid, uuid::Uuid) {
    let ids = seed_bare_project(store, workspace_name, key_prefix).await;
    seed_lanes(store, ids.1, ids.0).await;
    ids
}

/// Seed a workspace + team + member + project WITHOUT lane rows — the
/// leftmost-landing property supplies its own arbitrary lane set
/// (board-lane-management 02-01).
async fn seed_bare_project(
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
        .expect("insert succeeds")
        .number;

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
        .expect("insert succeeds")
        .number;

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
        .expect("insert succeeds")
        .number;

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

// ===========================================================================
// board-lane-management 02-01 — the D6 leftmost-landing rule at the store
// driving port. Test Budget: 2 behaviors (leftmost landing + echo; honest
// failure on an unresolvable lane) × 2 = 4 max; 2 written.
// ===========================================================================

use proptest::prelude::*;
use proptest::test_runner::{Config as ProptestConfig, TestCaseError, TestRunner};
use std::sync::atomic::{AtomicU32, Ordering};

/// A non-empty arbitrary lane set: unique slugs, unique positions, the
/// slug↔position pairing AND the insertion order both shuffled — the landing
/// lane must depend on nothing but the unique minimum position.
fn lane_set_strategy() -> impl Strategy<Value = Vec<(String, i32)>> {
    (1usize..=5)
        .prop_flat_map(|n| {
            (
                proptest::collection::btree_set("[a-z]{3,10}", n..=n),
                proptest::collection::btree_set(0i32..1000, n..=n)
                    .prop_map(|set| set.into_iter().collect::<Vec<_>>())
                    .prop_shuffle(),
            )
        })
        .prop_map(|(slugs, positions)| slugs.into_iter().zip(positions).collect::<Vec<_>>())
        .prop_shuffle()
}

/// A unique `^[A-Z]{2,6}$` key prefix per proptest case (I-P3 CHECK).
fn case_prefix(n: u32) -> String {
    let first = char::from(b'A' + (n / 26 % 26) as u8);
    let second = char::from(b'A' + (n % 26) as u8);
    format!("Q{first}{second}")
}

async fn seed_lane_row(
    store: &Store,
    project_id: uuid::Uuid,
    workspace_id: uuid::Uuid,
    slug: &str,
    position: i32,
) {
    sqlx::query(
        "INSERT INTO lanes (id, project_id, workspace_id, slug, label, position)
              VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(project_id)
    .bind(workspace_id)
    .bind(slug)
    .bind(slug)
    .bind(position)
    .execute(store.pool())
    .await
    .expect("seed arbitrary lane row");
}

/// PROPERTY (D6): for ANY non-empty lane set, the freshly-inserted issue
/// lands in the lane with the UNIQUE MINIMUM position — independent of
/// insertion order and of what the lanes are called — and the returned
/// [`foundry_store::InsertedIssue`] echoes that PERSISTED landing slug.
#[test]
fn issue_lands_in_the_unique_minimum_position_lane() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");
    let (base, guard) = rt.block_on(fresh_postgres());
    let store = rt.block_on(migrated_store(&base));

    let case = AtomicU32::new(0);
    let mut runner = TestRunner::new(ProptestConfig {
        cases: 16,
        failure_persistence: None,
        ..ProptestConfig::default()
    });
    let outcome = runner.run(&lane_set_strategy(), |lanes| {
        let n = case.fetch_add(1, Ordering::Relaxed);
        let prefix = case_prefix(n);
        rt.block_on(async {
            let (workspace_id, project_id, author_id) =
                seed_bare_project(&store, &format!("Lanes {n}"), &prefix).await;
            for (slug, position) in &lanes {
                seed_lane_row(&store, project_id, workspace_id, slug, *position).await;
            }
            let expected = lanes
                .iter()
                .min_by_key(|(_, position)| *position)
                .map(|(slug, _)| slug.clone())
                .expect("lane set is non-empty");

            let inserted = store
                .insert_issue_with_outbox(
                    uuid::Uuid::now_v7(),
                    workspace_id,
                    project_id,
                    &prefix,
                    author_id,
                    "Land me leftmost",
                    "",
                )
                .await
                .map_err(|err| {
                    TestCaseError::fail(format!(
                        "the insert must land in the leftmost lane of {lanes:?}, \
                             not fail: {err:?}"
                    ))
                })?;

            prop_assert_eq!(
                &inserted.state,
                &expected,
                "InsertedIssue.state must echo the unique-minimum-position lane of {:?}",
                lanes
            );
            let persisted: String = sqlx::query_scalar(
                "SELECT state FROM issues WHERE project_id = $1 AND number = $2",
            )
            .bind(project_id)
            .bind(inserted.number)
            .fetch_one(store.pool())
            .await
            .expect("read persisted state");
            prop_assert_eq!(
                persisted,
                expected,
                "the persisted state must be the unique-minimum-position lane of {:?}",
                lanes
            );
            Ok(())
        })
    });
    // The container's async Drop needs a live reactor — drop it explicitly
    // INSIDE the runtime before any failure panic unwinds this thread.
    rt.block_on(async move { drop(guard) });
    outcome.unwrap_or_else(|err| panic!("leftmost-landing property failed: {err}"));
}

/// Criterion 5's honest-error arm: when NO lane can be resolved (the lane set
/// vanished concurrently — modelled as a laneless project), the insert fails
/// with a store error and is FULLY rolled back: no issue row, no outbox row,
/// and `next_issue_number` unchanged.
#[tokio::test]
async fn insert_into_a_laneless_project_fails_honestly_and_rolls_back() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;

    let (_workspace_id, project_id, author_id) =
        seed_bare_project(&store, "Laneless", "GONE").await;

    let next_before: i32 =
        sqlx::query_scalar("SELECT next_issue_number FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_one(store.pool())
            .await
            .expect("read next_issue_number");

    let result = store
        .insert_issue_with_outbox(
            uuid::Uuid::now_v7(),
            uuid::Uuid::now_v7(),
            project_id,
            "GONE",
            author_id,
            "Nowhere to land",
            "",
        )
        .await;
    assert!(
        matches!(result, Err(foundry_store::IssueInsertError::Store(_))),
        "an unresolvable landing lane must surface an honest store error, got {result:?}"
    );

    let issue_count: i64 = sqlx::query_scalar("SELECT count(*) FROM issues")
        .fetch_one(store.pool())
        .await
        .expect("count issues");
    assert_eq!(issue_count, 0, "the failed insert must leave no issue row");
    let outbox_count: i64 = sqlx::query_scalar("SELECT count(*) FROM outbox")
        .fetch_one(store.pool())
        .await
        .expect("count outbox");
    assert_eq!(
        outbox_count, 0,
        "the failed insert must leave no outbox row"
    );
    let next_after: i32 =
        sqlx::query_scalar("SELECT next_issue_number FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_one(store.pool())
            .await
            .expect("re-read next_issue_number");
    assert_eq!(
        next_after, next_before,
        "the number allocation must roll back with the failed insert"
    );
}
