//! Operator CLI roster, workspace-scoped: `foundry doctor list-users
//! --workspace <id|name>` reads `Store::list_users_in_workspace(workspace_id)`,
//! which returns `(id, email_lower, display_name, is_super_admin, role)` for
//! every member of ONE workspace, ordered by email.
//!
//! - members come back ordered by email, each with their workspace role and
//!   the instance super-admin flag;
//! - a user of a SECOND workspace never appears (tenant isolation);
//! - a user in both workspaces appears in each, with that workspace's role;
//! - a workspace with no members yields an empty vec.
//!
//! Runs against a real Postgres (testcontainers, @real-io): the JOINs and the
//! `WHERE workspace_id` scoping can't be faked.

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

async fn make_super_admin(store: &Store, user_id: uuid::Uuid) {
    sqlx::query("INSERT INTO instance_admins (user_id) VALUES ($1)")
        .bind(user_id)
        .execute(store.pool())
        .await
        .expect("insert instance admin");
}

#[tokio::test]
async fn lists_one_workspaces_members_by_email_with_role_and_isolated() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;

    let acme = seed_workspace(&store, "Acme").await;
    let globex = seed_workspace(&store, "Globex").await;

    // Seeded out of email order, so a missing ORDER BY surfaces insertion order.
    let zoe = seed_user(&store, "zoe@acme.com", "Zoe").await;
    let amy = seed_user(&store, "amy@acme.com", "Amy").await;
    let both = seed_user(&store, "kim@both.com", "Kim").await;
    let foreign = seed_user(&store, "gus@globex.com", "Gus").await;

    join(&store, acme, zoe, "member").await;
    join(&store, acme, amy, "admin").await;
    join(&store, acme, both, "member").await;
    join(&store, globex, both, "admin").await;
    join(&store, globex, foreign, "member").await;
    make_super_admin(&store, amy).await;

    let acme_users = store
        .list_users_in_workspace(acme)
        .await
        .expect("list acme members");
    let summary: Vec<(uuid::Uuid, &str, &str, bool, &str)> = acme_users
        .iter()
        .map(|(id, email, name, admin, role)| {
            (*id, email.as_str(), name.as_str(), *admin, role.as_str())
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            (amy, "amy@acme.com", "Amy", true, "admin"),
            (both, "kim@both.com", "Kim", false, "member"),
            (zoe, "zoe@acme.com", "Zoe", false, "member"),
        ],
        "Acme's members, ordered by email, with their Acme role and the super-admin flag; \
         Globex's own user must not appear"
    );

    let globex_users = store
        .list_users_in_workspace(globex)
        .await
        .expect("list globex members");
    let globex_summary: Vec<(&str, &str)> = globex_users
        .iter()
        .map(|(_, email, _, _, role)| (email.as_str(), role.as_str()))
        .collect();
    assert_eq!(
        globex_summary,
        vec![("gus@globex.com", "member"), ("kim@both.com", "admin")],
        "a user in two workspaces carries each workspace's own role"
    );
}

#[tokio::test]
async fn a_workspace_with_no_members_lists_no_users() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;

    let empty = seed_workspace(&store, "Empty").await;
    let other = seed_workspace(&store, "Other").await;
    let someone = seed_user(&store, "someone@other.com", "Someone").await;
    join(&store, other, someone, "member").await;

    let users = store
        .list_users_in_workspace(empty)
        .await
        .expect("list members of an empty workspace");
    assert!(users.is_empty(), "no members, no rows: {users:?}");
}
