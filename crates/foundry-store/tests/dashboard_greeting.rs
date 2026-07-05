//! dashboard-enhancements US-01 (AC-05.4) — the greeting store query.
//!
//! The signed-in dashboard greets the user by name and names the acting
//! workspace via ONE tenant-scoped read: `Store::dashboard_greeting(user_id,
//! workspace_id)` returns `(display_name, workspace_name)` for the SESSION pair.
//! This pins that query's contract directly at the store boundary:
//!
//! - the valid session pair yields EXACTLY `(display_name, workspace_name)`;
//! - a mismatched id (a workspace/user with no row) yields `None`, so the
//!   handler degrades to a neutral fallback greeting rather than 500 (D1).
//!
//! Runs against a real Postgres (testcontainers, @real-io): the two-relation
//! read + its `None` fallback can't be faked. Each test runs the full
//! production migration set on a fresh container.

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

async fn seed_user(store: &Store, email: &str, display_name: &str) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, $3, 'phc$dummy')",
    )
    .bind(id)
    .bind(email)
    .bind(display_name)
    .execute(store.pool())
    .await
    .expect("insert user");
    id
}

/// The valid session pair yields EXACTLY the user's display name and the acting
/// workspace's name — even with a SECOND coexisting workspace/user seeded, so a
/// non-scoped read could have surfaced the wrong pair.
#[tokio::test]
async fn valid_session_pair_yields_display_name_and_workspace_name() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;

    // Seed a FOREIGN workspace + user first so a mis-scoped read would be tempted
    // to return their names instead.
    let _globex = seed_workspace(&store, "Globex").await;
    let _hank = seed_user(&store, "hank@globex.com", "Hank Scorpio").await;

    let acme = seed_workspace(&store, "Acme").await;
    let ada = seed_user(&store, "ada@acme.com", "Ada Lovelace").await;

    let greeting = store
        .dashboard_greeting(ada, acme)
        .await
        .expect("greeting query succeeds");

    assert_eq!(
        greeting,
        Some(("Ada Lovelace".to_string(), "Acme".to_string())),
        "the session pair must resolve to EXACTLY (display_name, workspace_name)"
    );
}

/// A session referencing an id with no row (e.g. a stale session after the
/// user/workspace was deleted) resolves to `None`, so the handler degrades to a
/// neutral fallback greeting (AC-01.4 / D1) rather than failing.
#[tokio::test]
async fn missing_id_yields_none_for_neutral_fallback() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;

    let acme = seed_workspace(&store, "Acme").await;
    let ada = seed_user(&store, "ada@acme.com", "Ada Lovelace").await;

    // A workspace id that was never seeded → no row → None.
    let phantom_workspace = uuid::Uuid::now_v7();
    let greeting = store
        .dashboard_greeting(ada, phantom_workspace)
        .await
        .expect("greeting query succeeds");
    assert_eq!(
        greeting, None,
        "a session pointing at a non-existent workspace must resolve to None"
    );

    // Symmetrically, a user id that was never seeded → None.
    let phantom_user = uuid::Uuid::now_v7();
    let greeting = store
        .dashboard_greeting(phantom_user, acme)
        .await
        .expect("greeting query succeeds");
    assert_eq!(
        greeting, None,
        "a session pointing at a non-existent user must resolve to None"
    );
}
