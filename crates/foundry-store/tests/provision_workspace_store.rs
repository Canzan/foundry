//! multi-workspace-provisioning — Slice 6: focused store-seam contracts for the
//! two provisioning store functions that were previously exercised only
//! indirectly (via the foundry-services use-case and the bin-driven acceptance
//! lane): [`Store::user_id_by_email`] and [`Store::provision_workspace`] (ADR-002
//! / ADR-003). This pins their behaviour directly at the store seam, so a
//! regression is caught here — not only obliquely through the slice-06 run.
//!
//! WHY-NEW-FILE: crates/foundry-store/tests/provision_workspace_store.rs
//!   CLOSEST-EXISTING: crates/foundry-store/tests/bootstrap_claim_seeds_superadmin.rs
//!   EXTENSION-COST: that file's scenarios pin the bootstrap CLAIM seeding tx
//!     (`create_initial_workspace` + the first-super-admin seed) against a fresh
//!     instance; folding the ADDITIONAL-workspace provisioning tx + the email→id
//!     lookup contract into it would entangle two distinct store transactions
//!     (initial-claim vs additional-provision) with different preconditions.
//!   PARALLEL-RATIONALE: `provision_workspace` operates on a RUNNING instance
//!     (an additional workspace, NOT workspace 1) and `user_id_by_email` is a
//!     read-only lookup — a different fixture lifecycle (claim-then-provision)
//!     and a different observable surface (the 4 provisioned rows + the resolved
//!     id) from the bootstrap-claim seed contract.
//!
//! Runs against a real Postgres (testcontainers, @real-io): the atomic
//! multi-row transaction and the case-insensitive email lookup cannot be faked.
//! Integration-level (adapter ↔ real DB), example-based wiring verification —
//! NOT PBT (the contract is "the lookup resolves the right id" and "the four
//! provisioned rows commit together", not "all input shapes").

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

/// Drive the REAL bootstrap claim once, seeding workspace 1 + its admin operator,
/// who is also the first instance super-admin. Returns the operator id.
async fn claim_instance(store: &Store, operator_email: &str) -> uuid::Uuid {
    let operator_id = uuid::Uuid::now_v7();
    store
        .create_initial_workspace(
            uuid::Uuid::now_v7(),
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

/// `user_id_by_email` resolves a stored user to EXACTLY their id (not a default,
/// not None) — the lookup the provisioning + grant CLIs use to resolve the actor.
///
/// Asserting the returned id EQUALS the known operator id kills both the
/// `Ok(None)` mutant (which would lose the user entirely) and the
/// `Ok(Some(Default::default()))` mutant (which would return the nil UUID instead
/// of the real row's id). The case-insensitive arm (mixed-case query against the
/// lowercased column) pins the `email_lower` contract the callers rely on.
#[tokio::test]
async fn user_id_by_email_resolves_a_stored_user_to_their_exact_id() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;

    let operator_id = claim_instance(&store, "Ops@Acme.com").await;

    // Exact-id resolution: the lookup returns THIS user's id, not nil/default.
    let resolved = store
        .user_id_by_email("ops@acme.com")
        .await
        .expect("user_id_by_email query");
    assert_eq!(
        resolved,
        Some(operator_id),
        "user_id_by_email must resolve the stored email to the user's EXACT id \
         (not None, not the nil/default UUID)"
    );
    assert_ne!(
        resolved,
        Some(uuid::Uuid::nil()),
        "the resolved id must be the real row's id, never the default/nil UUID"
    );

    // An email with no matching user resolves to None (the CLI maps this to a
    // fail-closed refusal, never a panic).
    let absent = store
        .user_id_by_email("nobody@acme.com")
        .await
        .expect("user_id_by_email query for absent email");
    assert_eq!(
        absent, None,
        "an email with no users row must resolve to None"
    );
}

/// `provision_workspace` atomically creates a NEW workspace + its first admin +
/// the admin's membership + a first-admin invite — every row commits together.
///
/// Asserting all FOUR rows exist with the exact ids passed in kills the
/// `Ok(())` no-op mutant (which would skip the whole transaction, leaving every
/// row absent). It also confirms the provision does NOT touch workspace 1 (the
/// running instance is left intact — an ADDITIONAL workspace).
#[tokio::test]
async fn provision_workspace_atomically_creates_workspace_admin_membership_and_invite() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;

    // A running instance with workspace 1 already claimed.
    claim_instance(&store, "ops@acme.com").await;
    let workspaces_before: i64 = sqlx::query_scalar("SELECT count(*) FROM workspaces")
        .fetch_one(store.pool())
        .await
        .expect("count workspaces before provision");

    let workspace_id = uuid::Uuid::now_v7();
    let admin_user_id = uuid::Uuid::now_v7();
    let invite_id = uuid::Uuid::now_v7();
    let expires_at = time::OffsetDateTime::now_utc() + time::Duration::days(7);

    store
        .provision_workspace(
            workspace_id,
            "Beta Workspace",
            admin_user_id,
            "admin@beta.com",
            "Admin@Beta.com",
            "Workspace Admin",
            "phc$dummy-hash",
            invite_id,
            expires_at,
        )
        .await
        .expect("provision_workspace commits the new workspace + admin + invite");

    // (1) the new workspace exists with the given id + name.
    let ws_name: Option<String> = sqlx::query_scalar("SELECT name FROM workspaces WHERE id = $1")
        .bind(workspace_id)
        .fetch_optional(store.pool())
        .await
        .expect("query provisioned workspace");
    assert_eq!(
        ws_name.as_deref(),
        Some("Beta Workspace"),
        "the new workspace row must exist with the provisioned name"
    );

    // (2) the first admin user exists with the given id + lowercased email.
    let admin_email_lower: Option<String> =
        sqlx::query_scalar("SELECT email_lower FROM users WHERE id = $1")
            .bind(admin_user_id)
            .fetch_optional(store.pool())
            .await
            .expect("query provisioned admin user");
    assert_eq!(
        admin_email_lower.as_deref(),
        Some("admin@beta.com"),
        "the first admin user row must exist, keyed on the provisioned id"
    );

    // (3) the admin's membership exists with role 'admin' in the new workspace.
    let admin_memberships: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM workspace_memberships \
         WHERE workspace_id = $1 AND user_id = $2 AND role = 'admin'",
    )
    .bind(workspace_id)
    .bind(admin_user_id)
    .fetch_one(store.pool())
    .await
    .expect("count provisioned admin membership");
    assert_eq!(
        admin_memberships, 1,
        "the first admin must be an 'admin' member of the new workspace"
    );

    // (4) the first-admin invite exists for the new workspace + admin email.
    let invite_workspace: Option<uuid::Uuid> =
        sqlx::query_scalar("SELECT workspace_id FROM invites WHERE id = $1")
            .bind(invite_id)
            .fetch_optional(store.pool())
            .await
            .expect("query provisioned invite");
    assert_eq!(
        invite_workspace,
        Some(workspace_id),
        "the first-admin invite must exist, bound to the new workspace"
    );

    // The provision created EXACTLY one additional workspace — workspace 1 (and
    // every existing tenant row) is untouched.
    let workspaces_after: i64 = sqlx::query_scalar("SELECT count(*) FROM workspaces")
        .fetch_one(store.pool())
        .await
        .expect("count workspaces after provision");
    assert_eq!(
        workspaces_after,
        workspaces_before + 1,
        "provision_workspace adds exactly one workspace, leaving the running instance intact"
    );
}
