//! Integration-style unit tests for the `projects::rename_project` use-case
//! (instance-admin-project-rename, ADR-PROJECT-RENAME-002), driven through the
//! `Services` driving port against a REAL Postgres harness (@real-io) — the
//! `provision_workspace_use_case` idiom.
//!
//! The PURE ordered classification is proptest-pinned in
//! `src/projects.rs::classify_rename_properties`; what only a real store can
//! exercise is the composition AROUND it: the fail-closed `if !is_admin` gate
//! and the write path's `rows_affected == 0` race guard. DELIVER Phase 5
//! mutation testing showed both survived when covered only by the @iapr
//! acceptance lane (the @real-io trap): the `delete !` gate-inversion mutant
//! and the `==` → `!=` rows-affected inversion. These two tests kill them at
//! the service seam.
//!
//! Two distinct behaviours (budget = 2 × 2 = 4; 2 written):
//!   1. A super-admin actor renames the project → `Ok(Renamed)` carrying the
//!      trimmed name, the row is committed, and the slug/key are UNTOUCHED
//!      (D1 — a rename never moves a URL).
//!   2. A NON-super-admin actor is refused FAIL-CLOSED with `Forbidden` and
//!      the stored name is byte-unchanged.

use foundry_services::projects::{RenameOutcome, RenameProjectError, RenameProjectRequest};
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
    /// The bootstrap operator — the first instance super-admin.
    super_admin_id: uuid::Uuid,
    /// A workspace member who is NOT an instance super-admin.
    non_admin_id: uuid::Uuid,
    /// The seeded "Sandbox" project (slug `sandbox`, key `GEN`).
    project_id: uuid::Uuid,
}

/// Spin a real Postgres, migrate it, claim the instance (seeding workspace 1,
/// team "General", project "Sandbox"/`sandbox`/`GEN`, and the first super-admin),
/// then insert a plain non-super-admin user as the unauthorized actor.
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

    let super_admin_id = uuid::Uuid::now_v7();
    let project_id = uuid::Uuid::now_v7();
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
            project_id,
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
        super_admin_id,
        non_admin_id,
        project_id,
    }
}

async fn stored_name_slug_key(h: &Harness) -> (String, String, String) {
    sqlx::query_as("SELECT name, slug, key_prefix FROM projects WHERE id = $1")
        .bind(h.project_id)
        .fetch_one(h.store.pool())
        .await
        .expect("query project row")
}

/// Behaviour 1: a super-admin renames the project; the outcome carries the
/// trimmed name, the row is committed, and slug + key_prefix are byte-unchanged
/// (D1). Kills the `rows_affected == 0` → `!= 0` inversion (which would turn
/// the successful single-row UPDATE into the race-guard `NotFound`).
#[tokio::test]
async fn super_admin_renames_and_the_new_name_is_committed_with_slug_untouched() {
    let h = seeded_harness().await;

    let outcome = h
        .services
        .rename_project(RenameProjectRequest {
            acting_user_id: h.super_admin_id,
            project_id: h.project_id,
            new_name: "  Identity Platform  ",
        })
        .await;

    match outcome {
        Ok(RenameOutcome::Renamed { name }) => assert_eq!(
            name, "Identity Platform",
            "the outcome must carry the TRIMMED stored name for the fragment"
        ),
        _ => panic!("a super-admin's valid rename must succeed as Renamed"),
    }

    let (name, slug, key_prefix) = stored_name_slug_key(&h).await;
    assert_eq!(
        name, "Identity Platform",
        "the new display name must be committed"
    );
    assert_eq!(slug, "sandbox", "the slug must NEVER move on rename (D1)");
    assert_eq!(
        key_prefix, "GEN",
        "the key prefix must NEVER move on rename (D1)"
    );
}

/// Behaviour 2: a NON-super-admin actor is refused FAIL-CLOSED with `Forbidden`
/// and the stored name is unchanged. Kills the `delete !` mutant on the
/// `if !is_admin` gate — the security-critical inversion that would let any
/// signed-in member rename cross-tenant projects.
#[tokio::test]
async fn non_super_admin_is_refused_fail_closed_and_the_name_is_unchanged() {
    let h = seeded_harness().await;

    let outcome = h
        .services
        .rename_project(RenameProjectRequest {
            acting_user_id: h.non_admin_id,
            project_id: h.project_id,
            new_name: "Hijacked",
        })
        .await;

    assert!(
        matches!(outcome, Err(RenameProjectError::Forbidden)),
        "a non-super-admin must be refused Forbidden, fail-closed"
    );

    let (name, _, _) = stored_name_slug_key(&h).await;
    assert_eq!(name, "Sandbox", "a refused rename must write NOTHING");
}
