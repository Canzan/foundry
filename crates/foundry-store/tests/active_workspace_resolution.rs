//! multi-workspace-tenancy (step 02-01) — membership-based active-workspace
//! resolution (ADR-005).
//!
//! The web sign-in path must resolve a member's ACTIVE workspace by their
//! `workspace_memberships`, NOT by the global `first_workspace()` (an unordered
//! `LIMIT 1` that returns an arbitrary tenant once two workspaces coexist). This
//! pins the resolution seam's contract directly at the store boundary:
//!
//! - a single-membership user auto-resolves to their ONE workspace;
//! - a user with NO membership resolves to `None` so the caller FAILS CLOSED
//!   (refuses) — it never defaults to an arbitrary tenant;
//! - the choice is DETERMINISTIC: a member of workspace B is never silently
//!   scoped to a coexisting workspace A by heap order.
//!
//! Runs against a real Postgres (testcontainers, @real-io): the JOIN + scoping
//! behaviour can't be faked. Each test runs the full production migration set on
//! a fresh container — including `0002_multi_workspace.sql` which drops the
//! single-workspace `uniq_one_workspace` guard so two workspaces can coexist.

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
        .with_tag("16-alpine") // match production Postgres
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

/// Insert a bare workspace row directly (mirrors the acceptance seeds; with
/// `0002` applied the `uniq_one_workspace` guard is gone so a 2nd row inserts).
async fn seed_workspace(store: &Store, name: &str) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(id)
        .bind(name)
        .execute(store.pool())
        .await
        .expect("insert workspace");
    id
}

async fn seed_user(store: &Store, email: &str) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, 'Member', 'phc$dummy')",
    )
    .bind(id)
    .bind(email)
    .execute(store.pool())
    .await
    .expect("insert user");
    id
}

async fn add_membership(store: &Store, workspace_id: uuid::Uuid, user_id: uuid::Uuid) {
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(workspace_id)
    .bind(user_id)
    .execute(store.pool())
    .await
    .expect("insert membership");
}

/// A single-membership member of one of two coexisting workspaces resolves to
/// EXACTLY their own workspace — even when the OTHER workspace was inserted
/// first (so an unordered `first_workspace()` could have returned the wrong
/// tenant). This is the walking-skeleton contract: Marco (Acme) sees Acme.
#[tokio::test]
async fn single_membership_resolves_to_that_one_workspace() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;

    // Seed the FOREIGN workspace FIRST so heap order would tempt an unordered
    // LIMIT-1 to return it.
    let _globex = seed_workspace(&store, "Globex").await;
    let acme = seed_workspace(&store, "Acme").await;

    let marco = seed_user(&store, "marco@acme.com").await;
    add_membership(&store, acme, marco).await;

    let resolved = store
        .resolve_active_workspace(marco)
        .await
        .expect("resolve query succeeds");

    assert_eq!(
        resolved.map(|(id, _)| id),
        Some(acme),
        "a single-membership member must resolve to their OWN workspace, not the heap-first one"
    );
}

/// A user with ZERO memberships resolves to `None` so the sign-in caller fails
/// closed (refuses) rather than defaulting to an arbitrary tenant (ADR-005).
#[tokio::test]
async fn zero_membership_resolves_to_none_fail_closed() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;

    let _acme = seed_workspace(&store, "Acme").await;
    let orphan = seed_user(&store, "orphan@nowhere.test").await;
    // Deliberately NO membership row.

    let resolved = store
        .resolve_active_workspace(orphan)
        .await
        .expect("resolve query succeeds");

    assert_eq!(
        resolved, None,
        "a member of no workspace must resolve to None so sign-in fails closed"
    );
}
