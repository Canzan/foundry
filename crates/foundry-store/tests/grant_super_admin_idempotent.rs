//! multi-workspace-provisioning — Slice 6 (step 02-02): an UPGRADED install
//! grants its first instance super-admin idempotently (ADR-001 / D1).
//!
//! An install that predates the super-admin role has a workspace + its admin but
//! NO `instance_admins` row. `grant_instance_admin` records that operator as the
//! first super-admin via an idempotent `INSERT … ON CONFLICT DO NOTHING`, so a
//! second grant for the SAME operator is a no-op (still exactly one row). After
//! the grant the operator passes `is_instance_admin` and can provision.
//!
//! Runs against a real Postgres (testcontainers, @real-io): the `ON CONFLICT`
//! idempotence and the row presence cannot be faked. Pins the store contract that
//! underpins the slice-06 "An upgraded install grants its first super-admin and
//! can then provision" acceptance scenario.

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

/// Seed an UPGRADED install: a workspace + its admin user, but NO
/// `instance_admins` row (the pre-super-admin-role world). Returns the admin's id.
async fn seed_upgraded_install(store: &Store, operator_email: &str) -> uuid::Uuid {
    let workspace_id = uuid::Uuid::now_v7();
    let admin_id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'Acme')")
        .bind(workspace_id)
        .execute(store.pool())
        .await
        .expect("seed workspace");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, 'Ops', 'phc$dummy')",
    )
    .bind(admin_id)
    .bind(operator_email.to_ascii_lowercase())
    .bind(operator_email)
    .execute(store.pool())
    .await
    .expect("seed admin user");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role)
              VALUES ($1, $2, 'admin')",
    )
    .bind(workspace_id)
    .bind(admin_id)
    .execute(store.pool())
    .await
    .expect("seed admin membership");
    admin_id
}

/// AC 1/3: granting an upgraded install's operator records exactly one
/// `instance_admins` row and the operator then passes `is_instance_admin`.
#[tokio::test]
async fn grant_records_operator_as_super_admin() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let operator_id = seed_upgraded_install(&store, "ops@acme.com").await;

    assert!(
        !store
            .is_instance_admin(operator_id)
            .await
            .expect("is_instance_admin before grant"),
        "an upgraded install has no super-admin before the grant"
    );

    store
        .grant_instance_admin(operator_id)
        .await
        .expect("grant super-admin");

    assert!(
        store
            .is_instance_admin(operator_id)
            .await
            .expect("is_instance_admin after grant"),
        "the granted operator must pass is_instance_admin"
    );
    let rows: Vec<uuid::Uuid> = sqlx::query_scalar("SELECT user_id FROM instance_admins")
        .fetch_all(store.pool())
        .await
        .expect("read instance_admins rows");
    assert_eq!(
        rows,
        vec![operator_id],
        "the grant records EXACTLY the operator as the single super-admin"
    );
}

/// AC 2/5: a second grant for the SAME operator is an idempotent no-op — still
/// exactly one row, and no other tenant data is altered (only the one insert).
#[tokio::test]
async fn second_grant_is_idempotent_no_op() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let operator_id = seed_upgraded_install(&store, "ops@acme.com").await;

    // Capture the surrounding tenant data so we can prove the grant touches
    // nothing beyond the single instance_admins insert.
    let users_before: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(store.pool())
        .await
        .expect("count users before");
    let memberships_before: i64 = sqlx::query_scalar("SELECT count(*) FROM workspace_memberships")
        .fetch_one(store.pool())
        .await
        .expect("count memberships before");

    store
        .grant_instance_admin(operator_id)
        .await
        .expect("first grant");
    store
        .grant_instance_admin(operator_id)
        .await
        .expect("second grant (idempotent)");

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM instance_admins")
        .fetch_one(store.pool())
        .await
        .expect("count instance_admins after two grants");
    assert_eq!(
        count, 1,
        "granting twice records the operator exactly once (ON CONFLICT DO NOTHING)"
    );

    let users_after: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
        .fetch_one(store.pool())
        .await
        .expect("count users after");
    let memberships_after: i64 = sqlx::query_scalar("SELECT count(*) FROM workspace_memberships")
        .fetch_one(store.pool())
        .await
        .expect("count memberships after");
    assert_eq!(
        (users_after, memberships_after),
        (users_before, memberships_before),
        "granting super-admin alters no tenant data beyond the instance_admins insert"
    );
}
