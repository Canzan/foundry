//! multi-workspace-provisioning — Slice 6 (step 02-01): the bootstrap CLAIM
//! seeds the claiming operator as the first instance super-admin (ADR-001 / D1).
//!
//! The SHIPPED `create_initial_workspace` seeding transaction atomically creates
//! workspace 1 + its admin (+ a seeded team/project). This step EXTENDS that SAME
//! atomic transaction so the claiming operator is ALSO recorded as the first
//! `instance_admins` row — so a fresh instance never exists with a workspace 1
//! but no provisioning authority (AC 4). The operator is therefore both ws1's
//! admin AND the first super-admin, with no separate instance identity (AC 2),
//! and after the claim passes `is_instance_admin` (AC 3).
//!
//! Runs against a real Postgres (testcontainers, @real-io): the transaction's
//! atomicity and the `instance_admins` row cannot be faked. Pins the step-02-01
//! contract that underpins the slice-06 "the bootstrap-claiming operator is the
//! first super-admin and can provision" acceptance scenario.

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

/// Drive the REAL bootstrap claim once, seeding workspace 1 + its admin operator.
async fn claim_instance(store: &Store, operator_email: &str) -> uuid::Uuid {
    let workspace_id = uuid::Uuid::now_v7();
    let operator_id = uuid::Uuid::now_v7();
    store
        .create_initial_workspace(
            workspace_id,
            "Acme",
            operator_id,
            &operator_email.to_ascii_lowercase(),
            operator_email,
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
        .expect("bootstrap claim seeds workspace 1 + its admin");
    operator_id
}

/// AC 1/2/3: the operator who claims the instance is recorded as the first
/// `instance_admins` row in the SAME claim, and afterwards passes
/// `is_instance_admin` — they are both ws1's admin and the first super-admin.
#[tokio::test]
async fn bootstrap_claim_records_operator_as_first_super_admin() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;

    let operator_id = claim_instance(&store, "ops@acme.com").await;

    assert!(
        store
            .is_instance_admin(operator_id)
            .await
            .expect("is_instance_admin query"),
        "the bootstrap-claiming operator must be the first instance super-admin (D1)"
    );

    // The operator is ALSO ws1's admin (same human, no separate identity).
    let admin_membership: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM workspace_memberships WHERE user_id = $1 AND role = 'admin'",
    )
    .bind(operator_id)
    .fetch_one(store.pool())
    .await
    .expect("count admin memberships");
    assert_eq!(
        admin_membership, 1,
        "the operator is also workspace 1's admin (no separate instance identity)"
    );
}

/// AC 4/5: a fresh instance never has a workspace 1 with no provisioning
/// authority — exactly one `instance_admins` row exists after a single claim,
/// keyed on the claiming operator (the seed touches no other tenant data).
#[tokio::test]
async fn claim_seeds_exactly_one_super_admin_and_no_more() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;

    // Before any claim there is no provisioning authority.
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM instance_admins")
        .fetch_one(store.pool())
        .await
        .expect("count instance_admins before claim");
    assert_eq!(before, 0, "a never-claimed instance has no super-admin");

    let operator_id = claim_instance(&store, "ops@acme.com").await;

    let rows: Vec<uuid::Uuid> = sqlx::query_scalar("SELECT user_id FROM instance_admins")
        .fetch_all(store.pool())
        .await
        .expect("read instance_admins rows");
    assert_eq!(
        rows,
        vec![operator_id],
        "the claim seeds EXACTLY the claiming operator as the single first super-admin"
    );
}
