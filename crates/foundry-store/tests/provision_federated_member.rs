//! keycloak-sso D3a (DDD-16/DDD-17): the focused store-seam contract for
//! `Store::provision_federated_member` — the single transaction that gives a
//! federated newcomer a password-less `member` account in the instance's ORIGINAL
//! workspace.
//!
//! WHY-NEW-FILE: crates/foundry-store/tests/provision_federated_member.rs
//!   CLOSEST-EXISTING: crates/foundry-store/tests/create_member_and_consume_store.rs
//!   EXTENSION-COST: that file pins the invite-consuming member-accept tx (guarded
//!     invite UPDATE + 23505 collision → EmailCollision rollback). Federated
//!     provisioning consumes nothing, picks its workspace itself, and resolves the
//!     email collision by RE-READING the winner (both callers sign in) rather than
//!     refusing — a different transaction with a different observable surface.
//!
//! Runs against a real Postgres (testcontainers, @real-io): ON CONFLICT under a
//! genuine concurrent race, the NULL password hash and the created_at ordering
//! cannot be faked. Integration-level, example-based wiring verification.

use foundry_store::{run_migrations, FederatedProvisionOutcome, Store};
use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::ImageExt;

async fn migrated_store() -> (
    Store,
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
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(10))
        .connect(&format!(
            "postgres://postgres:postgres@{host}:{port}/postgres"
        ))
        .await
        .expect("connect pool");
    run_migrations(&pool).await.expect("run migrations");
    (Store::from_pool(pool), container)
}

/// Seed a workspace with an explicit `created_at` so "original" is unambiguous.
async fn seed_workspace(store: &Store, name: &str, created_at: time::OffsetDateTime) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name, created_at) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(name)
        .bind(created_at)
        .execute(store.pool())
        .await
        .expect("seed workspace");
    id
}

async fn users_with_email(store: &Store, email_lower: &str) -> i64 {
    let (n,): (i64,) = sqlx::query_as("SELECT count(*) FROM users WHERE email_lower = $1")
        .bind(email_lower)
        .fetch_one(store.pool())
        .await
        .expect("count users");
    n
}

/// `(password_hash, display_name, email_display)` of the user.
async fn account_of(store: &Store, user_id: uuid::Uuid) -> (Option<String>, String, String) {
    sqlx::query_as("SELECT password_hash, display_name, email_display FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(store.pool())
        .await
        .expect("read user")
}

/// `(workspace_id, role)` for every membership the user holds.
async fn memberships_of(store: &Store, user_id: uuid::Uuid) -> Vec<(uuid::Uuid, String)> {
    sqlx::query_as("SELECT workspace_id, role FROM workspace_memberships WHERE user_id = $1")
        .bind(user_id)
        .fetch_all(store.pool())
        .await
        .expect("read memberships")
}

/// Behaviour 1 — a newcomer gets ONE password-less account holding exactly one
/// `member` membership in the ORIGINAL workspace (oldest `created_at`), even when a
/// newer workspace exists, and is never made an instance admin.
#[tokio::test]
async fn provisions_a_password_less_member_into_the_original_workspace() {
    let (store, _pg) = migrated_store().await;
    let now = time::OffsetDateTime::now_utc();
    let original = seed_workspace(&store, "Northwind", now - time::Duration::days(30)).await;
    let _newer = seed_workspace(&store, "Side Project", now).await;

    let outcome = store
        .provision_federated_member(
            "nia.newcomer@example.test",
            "Nia.Newcomer@example.test",
            "Nia Newcomer",
            now,
        )
        .await
        .expect("provision");

    let FederatedProvisionOutcome::Created {
        user_id,
        workspace_id,
    } = outcome
    else {
        panic!("expected a newly created account, got {outcome:?}");
    };
    assert_eq!(
        workspace_id, original,
        "provisioned into the original workspace"
    );
    let (hash, display, email_display) = account_of(&store, user_id).await;
    assert_eq!(hash, None, "a provisioned account has no password");
    assert_eq!(display, "Nia Newcomer");
    assert_eq!(email_display, "Nia.Newcomer@example.test");
    assert_eq!(
        memberships_of(&store, user_id).await,
        vec![(original, "member".to_string())]
    );
    let (admins,): (i64,) =
        sqlx::query_as("SELECT count(*) FROM instance_admins WHERE user_id = $1")
            .bind(user_id)
            .fetch_one(store.pool())
            .await
            .expect("count instance admins");
    assert_eq!(admins, 0, "provisioning never grants instance admin");
}

/// Behaviour 2 — an unclaimed instance (no workspace) provisions nothing, so
/// provisioning can never pre-empt the bootstrap claim (DDD-17 / D5).
#[tokio::test]
async fn an_unclaimed_instance_provisions_nothing() {
    let (store, _pg) = migrated_store().await;

    let outcome = store
        .provision_federated_member(
            "nia.newcomer@example.test",
            "nia.newcomer@example.test",
            "Nia",
            time::OffsetDateTime::now_utc(),
        )
        .await
        .expect("provision");

    assert_eq!(outcome, FederatedProvisionOutcome::NoWorkspace);
    assert_eq!(
        users_with_email(&store, "nia.newcomer@example.test").await,
        0
    );
}

/// Behaviour 3 — an address that already has an account is re-read, never
/// touched: its password, name and memberships survive unchanged.
#[tokio::test]
async fn an_existing_account_is_returned_untouched() {
    let (store, _pg) = migrated_store().await;
    let workspace = seed_workspace(&store, "Northwind", time::OffsetDateTime::now_utc()).await;
    let existing = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, 'pat@example.test', 'pat@example.test', 'Pat Operator', 'phc$kept')",
    )
    .bind(existing)
    .execute(store.pool())
    .await
    .expect("seed existing user");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'admin')",
    )
    .bind(workspace)
    .bind(existing)
    .execute(store.pool())
    .await
    .expect("seed existing membership");

    let outcome = store
        .provision_federated_member(
            "pat@example.test",
            "PAT@example.test",
            "Patricia Operator",
            time::OffsetDateTime::now_utc(),
        )
        .await
        .expect("provision");

    assert_eq!(
        outcome,
        FederatedProvisionOutcome::Existing { user_id: existing }
    );
    assert_eq!(
        account_of(&store, existing).await,
        (
            Some("phc$kept".to_string()),
            "Pat Operator".to_string(),
            "pat@example.test".to_string()
        )
    );
    assert_eq!(
        memberships_of(&store, existing).await,
        vec![(workspace, "admin".to_string())]
    );
}

/// Behaviour 4 — two concurrent first sign-ins for one address: both succeed with
/// the SAME user, and exactly one account + one member membership exist.
#[tokio::test]
async fn concurrent_first_sign_ins_share_one_account() {
    let (store, _pg) = migrated_store().await;
    let now = time::OffsetDateTime::now_utc();
    let workspace = seed_workspace(&store, "Northwind", now).await;

    let email = "nia.newcomer@example.test";
    let (a, b) = tokio::join!(
        store.provision_federated_member(email, email, "Nia", now),
        store.provision_federated_member(email, email, "Nia", now),
    );
    let user_of = |o: FederatedProvisionOutcome| match o {
        FederatedProvisionOutcome::Created { user_id, .. } => user_id,
        FederatedProvisionOutcome::Existing { user_id } => user_id,
        FederatedProvisionOutcome::NoWorkspace => panic!("a workspace exists"),
    };
    let (a, b) = (user_of(a.expect("first")), user_of(b.expect("second")));

    assert_eq!(a, b, "both sign-ins resolve to the same account");
    assert_eq!(users_with_email(&store, email).await, 1);
    assert_eq!(
        memberships_of(&store, a).await,
        vec![(workspace, "member".to_string())]
    );
}
