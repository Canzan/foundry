//! Operator CLI workspace role change: `foundry doctor set-role --workspace
//! <id|name> --email <addr> --role admin|member` calls
//! `Store::set_workspace_role(workspace_id, user_id, role)`.
//!
//! - a member is promoted to admin, and an admin with company is demoted;
//! - asking for the role they hold writes nothing (`Unchanged`);
//! - a non-member is refused (`NotMember`) and no membership is added;
//! - the last admin is never demoted (`LastAdmin`), even when the two admins
//!   of a workspace demote themselves at the same moment;
//! - the change touches only the named workspace.
//!
//! Runs against a real Postgres (testcontainers, @real-io): the row lock and
//! the exact read of the other admins can't be faked.

use foundry_core::WorkspaceRole;
use foundry_store::{run_migrations, SetWorkspaceRoleOutcome, Store};
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

async fn seed_user(store: &Store, email: &str, name: &str) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, $3, 'not-a-real-hash')",
    )
    .bind(id)
    .bind(email)
    .bind(name)
    .execute(store.pool())
    .await
    .expect("insert user");
    id
}

async fn join(store: &Store, workspace_id: uuid::Uuid, user_id: uuid::Uuid, role: &str) {
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, $3)",
    )
    .bind(workspace_id)
    .bind(user_id)
    .bind(role)
    .execute(store.pool())
    .await
    .expect("insert membership");
}

async fn role_of(store: &Store, workspace_id: uuid::Uuid, user_id: uuid::Uuid) -> Option<String> {
    sqlx::query_scalar(
        "SELECT role FROM workspace_memberships WHERE workspace_id = $1 AND user_id = $2",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_optional(store.pool())
    .await
    .expect("read role")
}

#[tokio::test]
async fn promotes_a_member_and_demotes_an_admin_who_is_not_the_last() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let ws = seed_workspace(&store, "Acme").await;
    let ada = seed_user(&store, "ada@example.com", "Ada").await;
    let bob = seed_user(&store, "bob@example.com", "Bob").await;
    join(&store, ws, ada, "admin").await;
    join(&store, ws, bob, "member").await;

    let promoted = store
        .set_workspace_role(ws, bob, WorkspaceRole::Admin)
        .await
        .expect("promote");
    assert_eq!(promoted, SetWorkspaceRoleOutcome::Changed);
    assert_eq!(role_of(&store, ws, bob).await.as_deref(), Some("admin"));

    let demoted = store
        .set_workspace_role(ws, ada, WorkspaceRole::Member)
        .await
        .expect("demote");
    assert_eq!(demoted, SetWorkspaceRoleOutcome::Changed);
    assert_eq!(role_of(&store, ws, ada).await.as_deref(), Some("member"));
}

#[tokio::test]
async fn the_role_they_already_hold_is_unchanged() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let ws = seed_workspace(&store, "Acme").await;
    let ada = seed_user(&store, "ada@example.com", "Ada").await;
    let bob = seed_user(&store, "bob@example.com", "Bob").await;
    join(&store, ws, ada, "admin").await;
    join(&store, ws, bob, "member").await;

    for (user, role, stored) in [
        (ada, WorkspaceRole::Admin, "admin"),
        (bob, WorkspaceRole::Member, "member"),
    ] {
        let outcome = store
            .set_workspace_role(ws, user, role)
            .await
            .expect("set role");
        assert_eq!(outcome, SetWorkspaceRoleOutcome::Unchanged);
        assert_eq!(role_of(&store, ws, user).await.as_deref(), Some(stored));
    }
}

#[tokio::test]
async fn a_non_member_is_refused_and_not_added() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let acme = seed_workspace(&store, "Acme").await;
    let other = seed_workspace(&store, "Other").await;
    let ada = seed_user(&store, "ada@example.com", "Ada").await;
    let eve = seed_user(&store, "eve@example.com", "Eve").await;
    join(&store, acme, ada, "admin").await;
    join(&store, other, eve, "admin").await;

    for role in [WorkspaceRole::Admin, WorkspaceRole::Member] {
        let outcome = store
            .set_workspace_role(acme, eve, role)
            .await
            .expect("set role");
        assert_eq!(outcome, SetWorkspaceRoleOutcome::NotMember);
        assert_eq!(role_of(&store, acme, eve).await, None);
    }
}

#[tokio::test]
async fn the_last_admin_is_not_demoted() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let ws = seed_workspace(&store, "Acme").await;
    let ada = seed_user(&store, "ada@example.com", "Ada").await;
    let bob = seed_user(&store, "bob@example.com", "Bob").await;
    join(&store, ws, ada, "admin").await;
    join(&store, ws, bob, "member").await;

    let outcome = store
        .set_workspace_role(ws, ada, WorkspaceRole::Member)
        .await
        .expect("demote");
    assert_eq!(outcome, SetWorkspaceRoleOutcome::LastAdmin);
    assert_eq!(role_of(&store, ws, ada).await.as_deref(), Some("admin"));
}

#[tokio::test]
async fn an_admin_of_another_workspace_does_not_count_and_is_not_touched() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let acme = seed_workspace(&store, "Acme").await;
    let other = seed_workspace(&store, "Other").await;
    let ada = seed_user(&store, "ada@example.com", "Ada").await;
    let eve = seed_user(&store, "eve@example.com", "Eve").await;
    join(&store, acme, ada, "admin").await;
    join(&store, other, ada, "admin").await;
    join(&store, other, eve, "admin").await;

    // Eve admins Other, not Acme: Ada is still Acme's only admin.
    let refused = store
        .set_workspace_role(acme, ada, WorkspaceRole::Member)
        .await
        .expect("demote in acme");
    assert_eq!(refused, SetWorkspaceRoleOutcome::LastAdmin);

    let demoted = store
        .set_workspace_role(other, ada, WorkspaceRole::Member)
        .await
        .expect("demote in other");
    assert_eq!(demoted, SetWorkspaceRoleOutcome::Changed);
    assert_eq!(role_of(&store, other, ada).await.as_deref(), Some("member"));
    assert_eq!(role_of(&store, acme, ada).await.as_deref(), Some("admin"));
}

#[tokio::test]
async fn two_admins_demoting_at_once_leave_one_admin() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;

    for round in 0..10 {
        let ws = seed_workspace(&store, &format!("Race {round}")).await;
        let ada = seed_user(&store, &format!("ada{round}@example.com"), "Ada").await;
        let bob = seed_user(&store, &format!("bob{round}@example.com"), "Bob").await;
        join(&store, ws, ada, "admin").await;
        join(&store, ws, bob, "admin").await;

        let (a, b) = tokio::join!(
            store.set_workspace_role(ws, ada, WorkspaceRole::Member),
            store.set_workspace_role(ws, bob, WorkspaceRole::Member),
        );
        let mut outcomes = [a.expect("ada"), b.expect("bob")];
        outcomes.sort_by_key(|o| *o == SetWorkspaceRoleOutcome::LastAdmin);
        assert_eq!(
            outcomes,
            [
                SetWorkspaceRoleOutcome::Changed,
                SetWorkspaceRoleOutcome::LastAdmin
            ],
            "round {round}"
        );
        let admins: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM workspace_memberships WHERE workspace_id = $1 AND role = 'admin'",
        )
        .bind(ws)
        .fetch_one(store.pool())
        .await
        .expect("count admins");
        assert_eq!(admins, 1, "round {round}");
    }
}
