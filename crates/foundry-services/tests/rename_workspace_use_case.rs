//! Integration-style unit tests for the `workspaces::rename_workspace`
//! use-case (instance-admin-workspace-rename, DDD-1), driven through the
//! `Services` driving port against a REAL Postgres harness (@real-io) — the
//! `rename_project_use_case` idiom.
//!
//! The PURE ordered classification is proptest-pinned in
//! `src/workspaces.rs::classify_workspace_rename_properties`, and the atomic
//! audited write is pinned in `foundry-store/tests/workspace_rename_with_audit.rs`.
//! What only this seam can exercise is the composition AROUND them: the
//! defence-in-depth `if !is_admin` re-check and the non-locking pre-read
//! no-op. DELIVER mutation testing showed the admin re-check survived when
//! covered only by the acceptance lane — the HTTP handler's
//! `require_instance_admin` refuses non-admins first, so deleting the
//! service guard passed every suite. These tests kill it at the service seam.
//!
//! Universe per test: the workspace's stored name + every
//! `workspace_rename_events` row (actor, old name, new name).
//!
//! Three distinct behaviours (budget = 3 × 2 = 6; 3 written):
//!   1. A NON-instance-admin actor is refused FAIL-CLOSED with `Forbidden`;
//!      the name and the rename record are byte-unchanged.
//!   2. An instance admin renames the workspace → `Ok(Renamed)` carrying the
//!      trimmed name, the name is committed, and exactly ONE record is
//!      appended (actor · old → new).
//!   3. An instance admin resubmitting the current name (padded) gets a quiet
//!      `NoOp` and nothing is written (D4) — the pre-read short-circuit.

use foundry_services::workspaces::{
    RenameWorkspaceError, RenameWorkspaceRequest, WorkspaceRenameOutcome,
};
use foundry_services::Services;
use foundry_store::Store;
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
    /// The bootstrap operator — the first instance admin.
    instance_admin_id: uuid::Uuid,
    /// A signed-in user who is NOT an instance admin.
    non_admin_id: uuid::Uuid,
    /// The seeded "Acme" workspace.
    workspace_id: uuid::Uuid,
}

/// Spin a real Postgres, migrate it, claim the instance (seeding workspace
/// "Acme", team "General", project "Sandbox", and the first instance admin),
/// then insert a plain non-admin user as the unauthorized actor.
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

    let workspace_id = uuid::Uuid::now_v7();
    let instance_admin_id = uuid::Uuid::now_v7();
    store
        .create_initial_workspace(
            workspace_id,
            "Acme",
            instance_admin_id,
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
        instance_admin_id,
        non_admin_id,
        workspace_id,
    }
}

async fn stored_name(h: &Harness) -> String {
    let (name,): (String,) = sqlx::query_as("SELECT name FROM workspaces WHERE id = $1")
        .bind(h.workspace_id)
        .fetch_one(h.store.pool())
        .await
        .expect("query workspace row");
    name
}

/// Every rename record on file, oldest first: (workspace, actor, old, new).
async fn rename_records(h: &Harness) -> Vec<(uuid::Uuid, uuid::Uuid, String, String)> {
    sqlx::query_as(
        "SELECT workspace_id, actor_id, old_name, new_name
           FROM workspace_rename_events
          ORDER BY created_at, id",
    )
    .fetch_all(h.store.pool())
    .await
    .expect("query rename records")
}

/// Behaviour 1: a NON-instance-admin actor is refused FAIL-CLOSED with
/// `Forbidden`, and neither the name nor the rename record moves. Kills the
/// deleted / inverted `if !is_admin` defence-in-depth re-check (DDD-1) — the
/// guard the HTTP handler otherwise masks.
#[tokio::test]
async fn non_instance_admin_is_refused_fail_closed_and_nothing_is_written() {
    let h = seeded_harness().await;
    let name_before = stored_name(&h).await;
    let records_before = rename_records(&h).await;

    let outcome = h
        .services
        .rename_workspace(RenameWorkspaceRequest {
            acting_user_id: h.non_admin_id,
            workspace_id: h.workspace_id,
            new_name: "Hijacked",
        })
        .await;

    assert!(
        matches!(outcome, Err(RenameWorkspaceError::Forbidden)),
        "a non-instance-admin must be refused Forbidden, fail-closed; got {outcome:?}"
    );
    assert_eq!(
        stored_name(&h).await,
        name_before,
        "a refused rename must leave the workspace name unchanged"
    );
    assert_eq!(
        rename_records(&h).await,
        records_before,
        "a refused rename must put nothing on record"
    );
}

/// Behaviour 2 (the paired allow case): an instance admin renames the
/// workspace; the outcome carries the trimmed name, the name is committed,
/// and exactly one record names who renamed it from what to what.
#[tokio::test]
async fn instance_admin_renames_and_exactly_one_record_is_kept() {
    let h = seeded_harness().await;
    let records_before = rename_records(&h).await;

    let outcome = h
        .services
        .rename_workspace(RenameWorkspaceRequest {
            acting_user_id: h.instance_admin_id,
            workspace_id: h.workspace_id,
            new_name: "  Acme Labs  ",
        })
        .await;

    match outcome {
        Ok(WorkspaceRenameOutcome::Renamed { name }) => assert_eq!(
            name, "Acme Labs",
            "the outcome must carry the TRIMMED stored name for the fragment"
        ),
        other => panic!("an instance admin's valid rename must succeed as Renamed; got {other:?}"),
    }
    assert_eq!(
        stored_name(&h).await,
        "Acme Labs",
        "the new workspace name must be committed"
    );

    let mut expected = records_before;
    expected.push((
        h.workspace_id,
        h.instance_admin_id,
        "Acme".to_string(),
        "Acme Labs".to_string(),
    ));
    assert_eq!(
        rename_records(&h).await,
        expected,
        "exactly one record (actor · old → new) must be appended"
    );
}

/// Behaviour 3: an instance admin resubmitting the current name (padded, as
/// from the pre-filled form) gets a quiet `NoOp` and nothing is written (D4)
/// — the service's pre-read short-circuit, before the store is asked to write.
#[tokio::test]
async fn instance_admin_resubmitting_the_current_name_is_a_quiet_no_op() {
    let h = seeded_harness().await;
    let records_before = rename_records(&h).await;

    let outcome = h
        .services
        .rename_workspace(RenameWorkspaceRequest {
            acting_user_id: h.instance_admin_id,
            workspace_id: h.workspace_id,
            new_name: " Acme ",
        })
        .await;

    assert_eq!(
        outcome.ok(),
        Some(WorkspaceRenameOutcome::NoOp {
            name: "Acme".to_string()
        }),
        "the unchanged name must be a quiet no-op"
    );
    assert_eq!(
        stored_name(&h).await,
        "Acme",
        "a no-op must not touch the name"
    );
    assert_eq!(
        rename_records(&h).await,
        records_before,
        "a no-op must put nothing on record"
    );
}
