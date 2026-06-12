//! multi-workspace-provisioning — Slice 6 (step 01-02): focused contract for the
//! `is_instance_admin(user_id)` INSTANCE-level super-admin authz predicate (D3,
//! ADR-003). This pins the predicate's behaviour directly, independently of the
//! provisioning use-case that consumes it, so a regression in the gate is caught
//! at the store seam — not only obliquely through the slice-06 acceptance run.
//!
//! WHY-NEW-FILE: crates/foundry-store/tests/is_instance_admin_authz.rs
//!   CLOSEST-EXISTING: crates/foundry-store/tests/grant_super_admin_idempotent.rs
//!   EXTENSION-COST: that file's scenarios are GRANT-idempotence assertions over a
//!     seeded UPGRADED install; folding the read-only authz-predicate contract in
//!     would entangle the predicate's fail-closed/empty-table/non-conflation
//!     contract with the grant transaction's lifecycle and obscure both.
//!   PARALLEL-RATIONALE: this file is a read-only predicate contract (no grant, no
//!     mutation) with a different fixture lifecycle — it asserts the EXISTS query's
//!     instance-scoping and fail-closed semantics, a distinct behavioural surface
//!     from the grant's ON CONFLICT idempotence.
//!
//! Runs against a real Postgres (testcontainers, @real-io): the EXISTS query and
//! its instance-scoping cannot be faked. Integration-level (adapter ↔ real DB),
//! example-based wiring verification — NOT PBT (the contract is "the query reads
//! instance_admins, scoped to the instance, fail-closed", not "all input shapes").
//!
//! Acceptance criteria pinned (ADR-003 / D3):
//!   1. present in instance_admins        → true
//!   2. absent                            → false (fail-closed: absence denies)
//!   3. takes ONLY a user identity        → no workspace argument (the signature)
//!   4. mirrors is_workspace_admin EXISTS → single existence query
//!   5. empty instance_admins table       → denies EVERY user (no implicit super-admin)

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

/// Insert a bare user row (the global identity `instance_admins` references) and
/// return its id. No workspace, no membership — so the only authority a user can
/// hold here is an explicit `instance_admins` row.
async fn seed_user(store: &Store, email: &str) -> uuid::Uuid {
    let id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, 'User', 'phc$dummy')",
    )
    .bind(id)
    .bind(email.to_ascii_lowercase())
    .bind(email)
    .execute(store.pool())
    .await
    .expect("seed user");
    id
}

/// AC 1/2/5: the predicate is true EXACTLY for users with an `instance_admins`
/// row, and false otherwise — including on the empty table (no implicit
/// super-admin). One real DB, three observable states (empty-denies,
/// present-allows, absent-denies) asserted over the predicate's return value.
#[tokio::test]
async fn is_instance_admin_is_true_exactly_for_recorded_super_admins() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;

    let super_admin = seed_user(&store, "super@acme.com").await;
    let regular = seed_user(&store, "regular@acme.com").await;

    // AC 5: empty table denies EVERY user — no implicit super-admin exists.
    for (who, id) in [("super-admin-elect", super_admin), ("regular", regular)] {
        assert!(
            !store
                .is_instance_admin(id)
                .await
                .expect("is_instance_admin on empty table"),
            "an empty instance_admins table must deny {who} (no implicit super-admin)"
        );
    }

    // Record exactly one super-admin.
    store
        .grant_instance_admin(super_admin)
        .await
        .expect("record the super-admin");

    // AC 1: the recorded user is now a super-admin; AC 2: the other is still
    // denied (absence denies, fail-closed) — the predicate is true EXACTLY for
    // the recorded set, not "anyone once any row exists".
    assert!(
        store
            .is_instance_admin(super_admin)
            .await
            .expect("is_instance_admin for recorded user"),
        "a user present in instance_admins must be a super-admin"
    );
    assert!(
        !store
            .is_instance_admin(regular)
            .await
            .expect("is_instance_admin for absent user"),
        "a user absent from instance_admins must be refused (fail-closed)"
    );
}

/// AC 3/4: the authority is INSTANCE-scoped, never tenant-scoped — a user who is
/// a workspace ADMIN (the strongest per-workspace role) but has no
/// `instance_admins` row is NOT a super-admin. This proves `is_instance_admin`
/// does not conflate workspace authority with instance authority (it reads only
/// `instance_admins`, takes no workspace argument), the exact separation ADR-003
/// keeps off the LAYER-1e tenant guard.
#[tokio::test]
async fn workspace_admin_is_not_an_instance_admin_unless_recorded() {
    let (base, _guard) = fresh_postgres().await;
    let store = migrated_store(&base).await;

    let ws_admin = seed_user(&store, "wsadmin@acme.com").await;
    let workspace_id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'Acme')")
        .bind(workspace_id)
        .execute(store.pool())
        .await
        .expect("seed workspace");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role)
              VALUES ($1, $2, 'admin')",
    )
    .bind(workspace_id)
    .bind(ws_admin)
    .execute(store.pool())
    .await
    .expect("seed workspace admin membership");

    // Sanity: the user genuinely holds the strongest per-workspace authority.
    assert!(
        store
            .is_workspace_admin(workspace_id, ws_admin)
            .await
            .expect("is_workspace_admin"),
        "the seeded user must be a workspace admin"
    );

    // Yet instance authority is a SEPARATE grant: a workspace admin with no
    // instance_admins row is refused provisioning authority (instance-scoped,
    // no workspace argument — cannot be satisfied by any membership).
    assert!(
        !store
            .is_instance_admin(ws_admin)
            .await
            .expect("is_instance_admin for a workspace admin"),
        "a workspace admin is NOT an instance super-admin unless explicitly recorded"
    );
}
