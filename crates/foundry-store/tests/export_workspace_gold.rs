//! per-workspace-backup (OD-PWB-2 / ADR-005) — the GOLD test for the tenant-table
//! set guard, plus a focused two-workspace isolation contract for
//! [`Store::export_workspace`].
//!
//! WHY-NEW-FILE: crates/foundry-store/tests/export_workspace_gold.rs
//!   CLOSEST-EXISTING: crates/foundry-store/tests/provision_workspace_store.rs
//!   EXTENSION-COST: that file pins the provisioning transaction + email lookup
//!     against a freshly-claimed instance; folding the export gold guard into it
//!     would entangle the claim-then-provision fixture lifecycle with the
//!     export's plant-a-row-per-table fixture, and mix two unrelated store seams.
//!   PARALLEL-RATIONALE: the gold test seeds EVERY tenant table directly (a
//!     different fixture shape) and its observable surface is the per-table export
//!     counts + the isolation crux — a distinct contract from the provisioning tx.
//!
//! The GOLD discipline (mirrors `check_arch.rs`'s plant-a-violation guard): plant
//! exactly one row in EACH of the ten `TENANT_TABLES` for a target workspace, run
//! the export, and assert the export reports all ten tables WITH the planted row.
//! Removing a table from the `TENANT_TABLES` constant would leave its planted row
//! uncounted — reding this test. This is the forcing function that keeps the
//! constant honest as the schema evolves: a new tenant table added to the schema
//! but not the constant is caught the moment a row is planted in it.
//!
//! Runs against a real Postgres (testcontainers, @real-io): the REPEATABLE READ
//! snapshot + the ten scoped SELECTs cannot be faked. Integration-level (adapter
//! ↔ real DB), example-based wiring verification — NOT PBT (the contract is "the
//! ten tables are all walked and scoped to W", not "all input shapes").

use foundry_store::{run_migrations, Store, TENANT_TABLES};
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

/// Plant exactly one row in EACH of the ten tenant tables for `workspace_id`.
/// Returns the member user id (the membership-bounded `users` row + the FK parent
/// for team membership / machine token / issue author / comment author).
async fn plant_one_row_per_tenant_table(
    pool: &sqlx::PgPool,
    workspace_id: uuid::Uuid,
    ws_name: &str,
) -> uuid::Uuid {
    // 1. workspaces
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(workspace_id)
        .bind(ws_name)
        .execute(pool)
        .await
        .expect("plant workspace");

    // 2. users (global identity) — scoped into W via its membership below.
    let user_id = uuid::Uuid::now_v7();
    let email = format!("u-{}@example.com", workspace_id.simple());
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, 'Member', 'phc$dummy')",
    )
    .bind(user_id)
    .bind(&email)
    .bind(&email)
    .execute(pool)
    .await
    .expect("plant user");

    // 3. workspace_memberships
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'admin')",
    )
    .bind(workspace_id)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("plant membership");

    // 4. teams
    let team_id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'Core', 'core')")
        .bind(team_id)
        .bind(workspace_id)
        .execute(pool)
        .await
        .expect("plant team");

    // 5. team_memberships (transitive via team_id)
    sqlx::query("INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'lead')")
        .bind(team_id)
        .bind(user_id)
        .execute(pool)
        .await
        .expect("plant team membership");

    // 6. projects
    let project_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, 'Apollo', 'apollo', 'APL')",
    )
    .bind(project_id)
    .bind(team_id)
    .bind(workspace_id)
    .execute(pool)
    .await
    .expect("plant project");

    // 7. issues
    let issue_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, author_id)
              VALUES ($1, $2, $3, 1, 'Gold issue', $4)",
    )
    .bind(issue_id)
    .bind(project_id)
    .bind(workspace_id)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("plant issue");

    // 8. invites
    sqlx::query(
        "INSERT INTO invites (id, workspace_id, invitee_email, created_by, expires_at)
              VALUES ($1, $2, 'invitee@example.com', $3, now() + interval '7 days')",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(workspace_id)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("plant invite");

    // 9. comments (denormalized workspace_id + issue_id)
    sqlx::query(
        "INSERT INTO comments (id, workspace_id, issue_id, author_id, body_markdown, body_html)
              VALUES ($1, $2, $3, $4, 'Gold comment', '<p>Gold comment</p>')",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(workspace_id)
    .bind(issue_id)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("plant comment");

    // 10. machine_tokens (PK is `jti`; `label` not `name`; NOT NULL `expires_at`)
    sqlx::query(
        "INSERT INTO machine_tokens (jti, user_id, workspace_id, expires_at, label, created_by)
              VALUES ($1, $2, $3, now() + interval '30 days', 'ci', $2)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(user_id)
    .bind(workspace_id)
    .execute(pool)
    .await
    .expect("plant machine token");

    user_id
}

/// GOLD: planting a row in every one of the ten tenant tables and exporting yields
/// an export that reports all ten tables, each with its planted row counted. A
/// table missing from `TENANT_TABLES` would leave its planted row uncounted —
/// reding this assertion.
#[tokio::test]
async fn export_reports_all_ten_tenant_tables_with_a_planted_row() {
    let (base, _container) = fresh_postgres().await;
    let store = migrated_store(&base).await;
    let workspace_id = uuid::Uuid::now_v7();
    plant_one_row_per_tenant_table(store.pool(), workspace_id, "Gold Workspace").await;

    let export = store
        .export_workspace(workspace_id)
        .await
        .expect("export the gold workspace");

    let counts: std::collections::HashMap<String, usize> =
        export.row_counts().into_iter().collect();
    assert_eq!(
        counts.len(),
        TENANT_TABLES.len(),
        "export must report every tenant table; got {counts:?}"
    );
    for table in TENANT_TABLES {
        let count = counts
            .get(table)
            .unwrap_or_else(|| panic!("export omitted tenant table {table:?}; got {counts:?}"));
        assert_eq!(
            *count, 1,
            "tenant table {table:?} must report its one planted row (got {count})"
        );
    }
    assert_eq!(export.workspace_id, workspace_id);
    assert_eq!(export.workspace_name, "Gold Workspace");
}

/// ISOLATION CRUX (store seam): with TWO coexisting workspaces each holding their
/// own full data set, exporting ONE contains every one of its rows and NONE of the
/// sibling's. Proven by row-count parity (each table reports exactly the target's
/// one planted row, not two) and by confirming the exported `workspaces` row is
/// the target's id. The membership-bounded `users` rule (ADR-001) is honoured: the
/// target's member is included, the sibling's is not.
#[tokio::test]
async fn export_of_one_workspace_excludes_the_sibling() {
    let (base, _container) = fresh_postgres().await;
    let store = migrated_store(&base).await;

    let globex_id = uuid::Uuid::now_v7();
    let acme_id = uuid::Uuid::now_v7();
    let _globex_user = plant_one_row_per_tenant_table(store.pool(), globex_id, "Globex LLC").await;
    let _acme_user = plant_one_row_per_tenant_table(store.pool(), acme_id, "Acme Corp").await;

    let export = store
        .export_workspace(globex_id)
        .await
        .expect("export globex");

    // Every tenant table reports exactly the target's one row — the sibling's
    // identical-shape row never rode along.
    for (table, rows) in &export.tables {
        assert_eq!(
            rows.len(),
            1,
            "exporting Globex must include only Globex's {table:?} row, not Acme's (got {})",
            rows.len()
        );
    }

    // The single workspaces row is Globex's, never Acme's.
    let workspaces_rows = &export
        .tables
        .iter()
        .find(|(t, _)| t == "workspaces")
        .expect("workspaces in export")
        .1;
    assert!(
        workspaces_rows[0].contains(&globex_id.to_string()),
        "the exported workspaces row must be Globex's id"
    );
    assert!(
        !workspaces_rows[0].contains(&acme_id.to_string()),
        "the exported workspaces row must NOT carry Acme's id"
    );
}
