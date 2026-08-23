//! `foundry doctor add-test-user` / `reset-password` — store-level contract.
//!
//! Pins the two store writes the operator CLI composes:
//!
//! - `create_user` inserts a bare `users` row (no memberships).
//! - `grant_all_memberships` enrols the user as a `member` of EVERY workspace
//!   and EVERY team, idempotently: rerunning after new workspaces/teams appear
//!   tops up only the missing rows (ON CONFLICT DO NOTHING), never duplicates,
//!   and never touches existing rows' roles.
//! - `update_user_password` (shipped) is the force-reset write; re-pinned here
//!   for the by-email CLI path (resolve via `user_id_by_email`, then update).
//!
//! Runs against a real Postgres (testcontainers, @real-io): PK conflicts and
//! INSERT..SELECT enumeration cannot be faked.

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

/// Seed one workspace with one team via the REAL bootstrap transaction, so the
/// grant sweep has genuine tenant rows to enumerate.
async fn seed_workspace(store: &Store, ws_name: &str, team_slug: &str) {
    store
        .create_initial_workspace(
            uuid::Uuid::now_v7(),
            ws_name,
            uuid::Uuid::now_v7(),
            &format!("admin-{team_slug}@example.com"),
            &format!("admin-{team_slug}@example.com"),
            "Ops",
            "phc$dummy",
            uuid::Uuid::now_v7(),
            "General",
            team_slug,
            uuid::Uuid::now_v7(),
            "Sandbox",
            &format!("sandbox-{team_slug}"),
            "GEN",
        )
        .await
        .expect("seed workspace");
}

/// The sweep enrols the user in every workspace and every team as 'member',
/// and a rerun after NEW tenants appear tops up exactly the missing rows.
#[tokio::test]
async fn grant_all_memberships_covers_everything_and_is_idempotent() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;

    seed_workspace(&store, "Acme", "general-a").await;
    seed_workspace(&store, "Beta", "general-b").await;

    let user_id = uuid::Uuid::now_v7();
    store
        .create_user(
            user_id,
            "tester@example.com",
            "tester@example.com",
            "Test User",
            "phc$dummy",
        )
        .await
        .expect("create bare test user");

    let (ws_added, teams_added) = store
        .grant_all_memberships(user_id)
        .await
        .expect("first sweep");
    assert_eq!((ws_added, teams_added), (2, 2), "joins both seeded tenants");

    // Rerun with nothing new: strictly zero rows added, none duplicated.
    let (ws_again, teams_again) = store
        .grant_all_memberships(user_id)
        .await
        .expect("idempotent rerun");
    assert_eq!((ws_again, teams_again), (0, 0), "rerun adds nothing");

    // A THIRD workspace appears later; a rerun tops up only the delta.
    seed_workspace(&store, "Gamma", "general-c").await;
    let (ws_delta, teams_delta) = store
        .grant_all_memberships(user_id)
        .await
        .expect("top-up sweep");
    assert_eq!((ws_delta, teams_delta), (1, 1), "only the new tenant added");

    // Every membership row the sweep wrote is role 'member' (never a
    // privilege escalation), and the counts on disk match the sweep total.
    let (ws_rows, member_ws_rows): (i64, i64) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE role = 'member')
           FROM workspace_memberships WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(store.pool())
    .await
    .expect("count workspace memberships");
    assert_eq!((ws_rows, member_ws_rows), (3, 3));

    let (team_rows, member_team_rows): (i64, i64) = sqlx::query_as(
        "SELECT count(*), count(*) FILTER (WHERE role = 'member')
           FROM team_memberships WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_one(store.pool())
    .await
    .expect("count team memberships");
    assert_eq!((team_rows, member_team_rows), (3, 3));
}

/// The sweep never demotes or duplicates an EXISTING membership: a user who is
/// already a workspace 'admin' / team 'lead' keeps those roles.
#[tokio::test]
async fn grant_all_memberships_leaves_existing_roles_untouched() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;

    // The bootstrap operator IS ws1's admin + the seeded team's lead.
    let operator_id = uuid::Uuid::now_v7();
    store
        .create_initial_workspace(
            uuid::Uuid::now_v7(),
            "Acme",
            operator_id,
            "ops@acme.com",
            "ops@acme.com",
            "Ops",
            "phc$dummy",
            uuid::Uuid::now_v7(),
            "General",
            "general",
            uuid::Uuid::now_v7(),
            "Sandbox",
            "sandbox",
            "GEN",
        )
        .await
        .expect("seed workspace with operator");

    let (ws_added, teams_added) = store
        .grant_all_memberships(operator_id)
        .await
        .expect("sweep over an already-enrolled user");
    assert_eq!(
        (ws_added, teams_added),
        (0, 0),
        "existing memberships are conflicts, not inserts"
    );

    let role: String =
        sqlx::query_scalar("SELECT role FROM workspace_memberships WHERE user_id = $1")
            .bind(operator_id)
            .fetch_one(store.pool())
            .await
            .expect("read operator's workspace role");
    assert_eq!(role, "admin", "the sweep never demotes an existing role");
}

/// The by-email force-reset path: resolve, update, and the old hash is gone.
/// (`update_user_password` is shipped; this pins the CLI's compose of it.)
#[tokio::test]
async fn force_reset_replaces_the_stored_hash_by_email() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    seed_workspace(&store, "Acme", "general").await;

    let user_id = store
        .user_id_by_email("admin-general@example.com")
        .await
        .expect("resolve query")
        .expect("seeded admin resolves by email");

    let rows = store
        .update_user_password(user_id, "phc$new-hash")
        .await
        .expect("force reset write");
    assert_eq!(rows, 1, "exactly the resolved user is updated");

    let hash: String = sqlx::query_scalar("SELECT password_hash FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(store.pool())
        .await
        .expect("read back hash");
    assert_eq!(hash, "phc$new-hash");
}
