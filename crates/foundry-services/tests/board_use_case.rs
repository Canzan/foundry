//! Integration-style unit tests for the `board::list_board_issues` use-case.
//!
//! Per step 01-01: the board read use-case performs membership authz
//! (`Store::is_team_member`) THEN `Store::list_issues_by_project`, returning
//! neutral `BoardIssue` rows. These tests drive the use-case through its public
//! driving-port signature against a REAL Postgres harness (@real-io,
//! single-example — proptest NOT warranted, there is no domain invariant here).
//!
//! Two distinct behaviours (budget = 2 × 2 = 4 unit tests; 2 written):
//!   1. A team member reads the board and gets the project's issues as neutral
//!      rows carrying key/number/title/state (most-recent-first, no markup).
//!   2. A non-member is refused with `ServiceError::Forbidden` and gets NO rows.

use foundry_services::board::list_board_issues;
use foundry_services::{BoardIssue, Principal, ServiceError};
use foundry_store::Store;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::ConnectOptions;
use std::str::FromStr;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::ImageExt;

/// A seeded Postgres + the ids a test needs to act on it. Keeps the
/// `_container` alive for the duration of the test (drop = teardown).
struct Harness {
    _container: testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    store: Store,
    workspace_id: uuid::Uuid,
    admin_id: uuid::Uuid,
    outsider_id: uuid::Uuid,
}

/// Spin a real Postgres, migrate it, and seed a workspace with:
///   - admin "devansh" (team member of "Backend", lead)
///   - an outsider "mallory" (workspace member, NOT a team member)
///   - project "Auth v2" (key prefix "AUTH") in the "Backend" team
///   - two issues: AUTH-1 ("First", backlog) and AUTH-2 ("Second", in_progress)
async fn seeded_harness() -> Harness {
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

    let opts = PgConnectOptions::from_str(&base)
        .expect("parse base url")
        .disable_statement_logging();
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(opts)
        .await
        .expect("connect pool");
    foundry_store::run_migrations(&pool)
        .await
        .expect("run migrations");
    let store = Store::from_pool(pool);

    let workspace_id = uuid::Uuid::now_v7();
    let admin_id = uuid::Uuid::now_v7();
    let team_id = uuid::Uuid::now_v7();
    let project_id = uuid::Uuid::now_v7();
    store
        .create_initial_workspace(
            workspace_id,
            "Acme Eng",
            admin_id,
            "devansh@acme.com",
            "devansh@acme.com",
            "Devansh",
            "x", // password hash placeholder — never verified in these tests
            team_id,
            "Backend",
            "backend",
            project_id,
            "Auth v2",
            "auth-v2",
            "AUTH",
        )
        .await
        .expect("seed initial workspace");

    // An outsider: a workspace member who does NOT belong to the Backend team.
    let outsider_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(outsider_id)
    .bind("mallory@acme.com")
    .bind("mallory@acme.com")
    .bind("Mallory")
    .bind("x")
    .execute(store.pool())
    .await
    .expect("seed outsider user");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(workspace_id)
    .bind(outsider_id)
    .execute(store.pool())
    .await
    .expect("seed outsider membership");

    // Two issues so we can assert mapping + ordering. AUTH-1 stays in the
    // default `backlog` state; AUTH-2 is moved to `in_progress`.
    store
        .insert_issue_with_outbox(
            uuid::Uuid::now_v7(),
            workspace_id,
            project_id,
            "AUTH",
            admin_id,
            "First",
        )
        .await
        .expect("seed AUTH-1");
    store
        .insert_issue_with_outbox(
            uuid::Uuid::now_v7(),
            workspace_id,
            project_id,
            "AUTH",
            admin_id,
            "Second",
        )
        .await
        .expect("seed AUTH-2");
    store
        .update_issue_state_with_outbox("AUTH", 2, "in_progress", admin_id)
        .await
        .expect("move AUTH-2 to in_progress");

    Harness {
        _container: container,
        store,
        workspace_id,
        admin_id,
        outsider_id,
    }
}

/// Behaviour 1 — a team member reads the board and receives the project's
/// issues as neutral rows (key/number/title/state), most-recent-first, with no
/// markup. This is the literal core-neutrality proof (NFR-WEB-BND-05).
#[tokio::test]
async fn member_reads_board_issues_as_neutral_rows() {
    let h = seeded_harness().await;
    let principal = Principal::Human {
        user_id: h.admin_id,
        workspace_id: h.workspace_id,
    };

    let rows: Vec<BoardIssue> = list_board_issues(&h.store, &principal, "backend", "auth-v2")
        .await
        .expect("a member must be able to read the board");

    // Ordering mirrors `Store::list_issues_by_project`: number DESC.
    let observed: Vec<(String, i32, String, String)> = rows
        .iter()
        .map(|r| (r.key.clone(), r.number, r.title.clone(), r.state.clone()))
        .collect();
    assert_eq!(
        observed,
        vec![
            (
                "AUTH-2".to_string(),
                2,
                "Second".to_string(),
                "in_progress".to_string()
            ),
            (
                "AUTH-1".to_string(),
                1,
                "First".to_string(),
                "backlog".to_string()
            ),
        ],
        "the use-case must return neutral rows carrying key/number/title/state, newest first"
    );
}

/// Behaviour 2 — a workspace member who is NOT a member of the team is refused
/// with `ServiceError::Forbidden`, and NO issue rows are returned (authz runs
/// BEFORE the fetch).
#[tokio::test]
async fn non_member_is_refused_forbidden() {
    let h = seeded_harness().await;
    let principal = Principal::Human {
        user_id: h.outsider_id,
        workspace_id: h.workspace_id,
    };

    let err = list_board_issues(&h.store, &principal, "backend", "auth-v2")
        .await
        .expect_err("a non-member must be refused, never handed the board's issues");

    assert!(
        matches!(err, ServiceError::Forbidden),
        "non-member must get Forbidden, got {err:?}"
    );
}
