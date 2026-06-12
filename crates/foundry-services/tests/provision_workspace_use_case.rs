//! Integration-style unit tests for the `provisioning::provision_workspace`
//! use-case (multi-workspace-provisioning, US-MWT07, ADR-002/003), driven through
//! the `Services` driving port against a REAL Postgres harness (@real-io). The
//! use-case was previously exercised only by the bin-driven slice-06 acceptance
//! lane; these pin its two observable behaviours directly at the service seam.
//!
//! The genuinely-new logic is the FAIL-CLOSED instance-super-admin gate
//! (`if !is_admin { return Err(Forbidden) }`) composed with the atomic provision
//! transaction — single-example @real-io (no domain invariant warrants proptest;
//! the contract is "the gate refuses non-admins and the admin path commits").
//!
//! Two distinct behaviours (budget = 2 × 2 = 4; 2 written):
//!   1. A super-admin actor provisions a NEW workspace → `Ok(Provisioned)` whose
//!      ids resolve to real committed rows (kills the no-op `Ok(Default::default())`
//!      mutants on both the `Services` wrapper and the use-case).
//!   2. A NON-super-admin actor is refused FAIL-CLOSED with `ServiceError::Forbidden`
//!      and NO workspace is created (kills the `delete !` gate-inversion mutant —
//!      the security-critical one — and re-confirms the no-op mutants).

use foundry_services::{provisioning, ServiceError, Services};
use foundry_store::Store;
use secrecy::SecretString;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::ConnectOptions;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use testcontainers_modules::postgres::Postgres;
use testcontainers_modules::testcontainers::runners::AsyncRunner;
use testcontainers_modules::testcontainers::ImageExt;

struct Harness {
    _container: testcontainers_modules::testcontainers::ContainerAsync<Postgres>,
    services: Services,
    store: Arc<Store>,
    /// The bootstrap operator — workspace 1's admin AND the first instance
    /// super-admin (seeded by `create_initial_workspace`).
    super_admin_id: uuid::Uuid,
    /// A workspace member who is NOT an instance super-admin.
    non_admin_id: uuid::Uuid,
}

/// Spin a real Postgres, migrate it, claim the instance (seeding workspace 1 +
/// its admin who is the first super-admin), and add a second, non-super-admin
/// user via a provisioning call made BY the super-admin (so the non-admin is a
/// real user but holds no instance authority).
async fn seeded_harness() -> Harness {
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

    let opts = PgConnectOptions::from_str(&base)
        .expect("parse base url")
        .disable_statement_logging();
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(opts)
        .await
        .expect("connect pool");
    foundry_store::run_migrations(&pool)
        .await
        .expect("run migrations");
    let store = Arc::new(Store::from_pool(pool));

    // Claim the instance: workspace 1 + its admin, who is the first super-admin.
    let super_admin_id = uuid::Uuid::now_v7();
    store
        .create_initial_workspace(
            uuid::Uuid::now_v7(),
            "Acme",
            super_admin_id,
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
        .expect("bootstrap claim");

    // A plain user with no instance authority (inserted directly — the simplest
    // way to get a non-super-admin users row to act as the unauthorized actor).
    let non_admin_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(non_admin_id)
    .bind("mallory@acme.com")
    .bind("mallory@acme.com")
    .bind("Mallory")
    .bind("phc$dummy")
    .execute(store.pool())
    .await
    .expect("insert non-admin user");

    let services = Services::new(Arc::clone(&store));
    Harness {
        _container: container,
        services,
        store,
        super_admin_id,
        non_admin_id,
    }
}

fn request_for(
    acting_user_id: uuid::Uuid,
    workspace_name: &'static str,
    admin_email: &'static str,
) -> provisioning::ProvisionRequest<'static> {
    provisioning::ProvisionRequest {
        acting_user_id,
        workspace_name,
        admin_email,
        admin_password: SecretString::new("initial-credential-123".into()),
        invite_expires_at: time::OffsetDateTime::now_utc() + time::Duration::days(7),
    }
}

/// Behaviour 1: a super-admin actor provisions a new workspace, and the returned
/// identity resolves to real committed rows. Kills the `Ok(Default::default())`
/// no-op mutants on the `Services` wrapper AND the use-case (a no-op would return
/// a nil-id `Provisioned` whose workspace row does not exist).
#[tokio::test]
async fn super_admin_provisions_a_new_workspace_and_its_ids_resolve_to_real_rows() {
    let h = seeded_harness().await;

    let provisioned = h
        .services
        .provision_workspace(request_for(h.super_admin_id, "Globex", "admin@globex.com"))
        .await
        .expect("a super-admin's provision must succeed");

    // The returned ids are real, not the nil/default UUID a no-op would yield.
    assert_ne!(
        provisioned.workspace_id,
        uuid::Uuid::nil(),
        "the provisioned workspace id must be a real id, never the default/nil UUID"
    );

    // The workspace row actually exists with the requested name.
    let ws_name: Option<String> = sqlx::query_scalar("SELECT name FROM workspaces WHERE id = $1")
        .bind(provisioned.workspace_id)
        .fetch_optional(h.store.pool())
        .await
        .expect("query provisioned workspace");
    assert_eq!(
        ws_name.as_deref(),
        Some("Globex"),
        "the provisioned workspace must be committed with the requested name"
    );

    // The first admin + their invite were committed under the returned ids.
    let admin_email: Option<String> =
        sqlx::query_scalar("SELECT email_lower FROM users WHERE id = $1")
            .bind(provisioned.admin_user_id)
            .fetch_optional(h.store.pool())
            .await
            .expect("query provisioned admin");
    assert_eq!(
        admin_email.as_deref(),
        Some("admin@globex.com"),
        "the provisioned first-admin user must be committed under the returned id"
    );
    let invite_ws: Option<uuid::Uuid> =
        sqlx::query_scalar("SELECT workspace_id FROM invites WHERE id = $1")
            .bind(provisioned.invite_id)
            .fetch_optional(h.store.pool())
            .await
            .expect("query provisioned invite");
    assert_eq!(
        invite_ws,
        Some(provisioned.workspace_id),
        "the first-admin invite must be committed, bound to the new workspace"
    );
}

/// Behaviour 2: a NON-super-admin actor is refused FAIL-CLOSED and NO workspace
/// is created. Kills the `delete !` mutant on the `if !is_admin` gate (which
/// would INVERT the gate, letting a non-admin provision) — the security-critical
/// mutant — and re-confirms the no-op mutants (which would return Ok, not Err).
#[tokio::test]
async fn non_super_admin_is_refused_fail_closed_and_no_workspace_is_created() {
    let h = seeded_harness().await;

    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM workspaces")
        .fetch_one(h.store.pool())
        .await
        .expect("count workspaces before");

    let result = h
        .services
        .provision_workspace(request_for(h.non_admin_id, "Evil Corp", "evil@corp.com"))
        .await;

    // Map to a Debug-able marker (Provisioned has no Debug; we only need to
    // assert the error variant, not format the Ok value).
    let outcome: Result<(), ServiceError> = result.map(|_| ());
    assert!(
        matches!(outcome, Err(ServiceError::Forbidden)),
        "a non-super-admin's provision must be refused fail-closed with Forbidden, got {outcome:?}"
    );

    // The gate refused BEFORE any write — no workspace was created.
    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM workspaces")
        .fetch_one(h.store.pool())
        .await
        .expect("count workspaces after");
    assert_eq!(
        before, after,
        "a refused provision must create no workspace (the fail-closed gate blocks the write)"
    );
    let evil_ws: i64 = sqlx::query_scalar("SELECT count(*) FROM workspaces WHERE name = $1")
        .bind("Evil Corp")
        .fetch_one(h.store.pool())
        .await
        .expect("count evil workspace");
    assert_eq!(
        evil_ws, 0,
        "the unauthorized actor's target workspace must never exist"
    );
}
