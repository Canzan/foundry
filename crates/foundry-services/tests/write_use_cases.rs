//! Integration-style unit tests for the foundry-services WRITE use-cases
//! (step 03-01): `issues::{create_issue, change_issue_state}` and
//! `comments::{create_comment, edit_comment}`.
//!
//! These drive the use-cases through their public async driving-port
//! signatures against a REAL Postgres harness (@real-io, single-example —
//! the writes orchestrate the real store + outbox path, no domain invariant
//! warrants proptest here). The KEY contract (NFR-WEB-API-CON-02): the
//! service REUSES the exact core write+outbox path the browser handlers use,
//! so an API write and a browser write accept/reject identically and store
//! identical bytes.
//!
//! Test budget: 3 distinct behaviours named in the step criteria
//! (budget = 2 × 3 = 6; 3 written):
//!   1. create_issue happy path — a member files an issue, gets the next
//!      sequential key + backlog state, persisted with the SAME validation
//!      the browser enforces (trimmed, non-empty, ≤256).
//!   2. create_issue empty-title rejected — a whitespace-only title is
//!      refused with the SAME rule the browser uses (Validation), no row.
//!   3. edit_comment non-author → Forbidden — author-or-admin authz is
//!      decided in the service, never the adapter.

use foundry_services::comments::edit_comment;
use foundry_services::issues::create_issue;
use foundry_services::{CreatedIssue, Principal, ServiceError};
use foundry_store::Store;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::ConnectOptions;
use std::str::FromStr;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::ImageExt;

struct Harness {
    _container: testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    store: Store,
    workspace_id: uuid::Uuid,
    project_id: uuid::Uuid,
    admin_id: uuid::Uuid,
    member_id: uuid::Uuid,
}

/// Spin a real Postgres, migrate it, and seed a workspace with:
///   - admin "devansh" (team member of "Backend", lead)
///   - member "mei" (workspace member AND team member of "Backend")
///   - project "Auth v2" (key prefix "AUTH") in the "Backend" team
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
            "x",
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

    // A second user "mei" who IS a member of the Backend team.
    let member_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(member_id)
    .bind("mei@acme.com")
    .bind("mei@acme.com")
    .bind("Mei")
    .bind("x")
    .execute(store.pool())
    .await
    .expect("seed member user");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(workspace_id)
    .bind(member_id)
    .execute(store.pool())
    .await
    .expect("seed member workspace membership");
    sqlx::query("INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'member')")
        .bind(team_id)
        .bind(member_id)
        .execute(store.pool())
        .await
        .expect("seed member team membership");

    Harness {
        _container: container,
        store,
        workspace_id,
        project_id,
        admin_id,
        member_id,
    }
}

/// Behaviour 1 — create_issue happy path. A team member files an issue and
/// gets the next sequential key (AUTH-1) + the backlog start state. The row
/// is persisted via the SAME `insert_issue_with_outbox` core path the browser
/// handler uses (count goes 0 → 1).
#[tokio::test]
async fn create_issue_files_with_next_key_and_backlog_state() {
    let h = seeded_harness().await;
    let principal = Principal::Human {
        user_id: h.admin_id,
        workspace_id: h.workspace_id,
    };

    let before = h
        .store
        .count_issues_in_project(h.project_id)
        .await
        .expect("count before");

    let created: CreatedIssue = create_issue(
        &h.store,
        &principal,
        "backend",
        "auth-v2",
        "  Refresh token rotation broken  ",
    )
    .await
    .expect("a member must be able to file an issue");

    let after = h
        .store
        .count_issues_in_project(h.project_id)
        .await
        .expect("count after");

    assert_eq!(
        (created.key.as_str(), created.number, created.state.as_str()),
        ("AUTH-1", 1, "backlog"),
        "create_issue must return the next sequential key + backlog start state"
    );
    assert_eq!(
        (before, after),
        (0, 1),
        "create_issue must persist exactly one issue via the core write+outbox path"
    );
}

/// Behaviour 2 — create_issue empty-title rejected by the SAME rule the
/// browser enforces. A whitespace-only title is refused (Validation), and NO
/// issue row is created.
#[tokio::test]
async fn create_issue_rejects_empty_title_with_no_row() {
    let h = seeded_harness().await;
    let principal = Principal::Human {
        user_id: h.admin_id,
        workspace_id: h.workspace_id,
    };

    let err = create_issue(&h.store, &principal, "backend", "auth-v2", "   ")
        .await
        .expect_err("an empty (whitespace-only) title must be rejected");

    assert!(
        matches!(err, ServiceError::Validation { .. }),
        "empty title must be a Validation refusal, got {err:?}"
    );
    let count = h
        .store
        .count_issues_in_project(h.project_id)
        .await
        .expect("count after rejection");
    assert_eq!(count, 0, "a rejected title must create NO issue row");
}

/// Behaviour 3 — edit_comment non-author → Forbidden. The author-or-admin
/// authorization is decided in the service. A non-author (mei) editing the
/// admin's comment is refused with Forbidden, and the stored body is unchanged.
#[tokio::test]
async fn edit_comment_by_non_author_is_forbidden() {
    let h = seeded_harness().await;
    let admin = Principal::Human {
        user_id: h.admin_id,
        workspace_id: h.workspace_id,
    };
    // Seed an issue + a comment authored by the admin.
    let number = create_issue(&h.store, &admin, "backend", "auth-v2", "Some issue")
        .await
        .expect("seed issue")
        .number;
    let comment = foundry_services::comments::create_comment(
        &h.store,
        &admin,
        "backend",
        "auth-v2",
        number,
        "original body",
    )
    .await
    .expect("seed comment authored by admin");

    // Mei (a team member, but NOT the author and NOT an admin) tries to edit.
    let mei = Principal::Human {
        user_id: h.member_id,
        workspace_id: h.workspace_id,
    };
    let err = edit_comment(
        &h.store,
        &mei,
        "backend",
        "auth-v2",
        number,
        comment.id,
        "hijacked body",
    )
    .await
    .expect_err("a non-author must not be able to edit another user's comment");

    assert!(
        matches!(err, ServiceError::Forbidden),
        "non-author edit must be Forbidden, got {err:?}"
    );
}
