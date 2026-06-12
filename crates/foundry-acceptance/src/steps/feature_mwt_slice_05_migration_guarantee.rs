//! multi-workspace-provisioning — Slice 5 (US-MWT06) step definitions:
//! the existing-install upgrade-safety guarantee.
//!
//! This is the FIRST slice-05 module; step 01-01 implements ONLY the
//! idempotent-re-upgrade scenario ("Re-running the upgrade does not duplicate
//! or alter anything"). Later slice-05 steps (04-xx) extend this World glue
//! with the row-equality, carried-session/token, and no-backfill scenarios.
//!
//! Driving surface (feature header §"Driving surface"): the upgrade is
//! migration-shaped — the "actor" is the operator upgrading the binary. The
//! scenario drives the SHIPPED migration runner: it stages the PRE-feature
//! migration history (`0001`..`0008`) into a `tempfile::TempDir` (the
//! `support::test_migration` precedent), seeds representative tenant data via
//! raw inserts, then applies the canonical forward-only migrations
//! (`0009`, `0010`, `0011`) via the SAME `run_migrations_from_dir` the
//! production boot path uses — TWICE — and asserts the second apply neither
//! duplicates the workspace nor alters any tenant row.
//!
//! LAYER 3 (real adapter, @real-io): real Postgres via testcontainers + a
//! per-scenario schema; the real migration runner under its advisory-lock
//! guard. Example-based (Mandates 9 + 11) — no PBT machinery at this layer.
//! Assertions are traditional, over port-exposed observables: the workspace
//! row count + the row-level before/after snapshot equality of every tenant
//! table (matching the slices 1-4 convention; no Rust state-delta port exists).

use crate::support::harness::fresh_schema_pool_no_migrations;
use crate::support::test_migration;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use foundry_store::run_migrations_from_dir;
use sqlx::{PgPool, Row};
use std::collections::HashMap;

/// The tenant tables whose rows the guarantee promises to leave byte-for-byte
/// unchanged across the upgrade (ADR-004 §Decision step 2).
const TENANT_TABLES: &[&str] = &[
    "workspaces",
    "users",
    "workspace_memberships",
    "teams",
    "team_memberships",
    "projects",
    "issues",
    "invites",
];

/// Snapshot every tenant table as an ordered list of whole-row JSON strings,
/// keyed by table name. `to_jsonb(t.*)` renders the entire row deterministically;
/// ordering by the row text makes the comparison insertion-order independent.
async fn snapshot_tenant_tables(pool: &PgPool) -> HashMap<String, Vec<String>> {
    let mut out = HashMap::new();
    for table in TENANT_TABLES {
        let sql =
            format!("SELECT to_jsonb(t.*)::text AS row_json FROM {table} t ORDER BY row_json");
        let rows = sqlx::query(&sql)
            .fetch_all(pool)
            .await
            .unwrap_or_else(|e| panic!("snapshot {table}: {e}"));
        let row_jsons = rows
            .into_iter()
            .map(|r| r.get::<String, _>("row_json"))
            .collect();
        out.insert((*table).to_string(), row_jsons);
    }
    out
}

// ---------------------------------------------------------------------------
// Background
// ---------------------------------------------------------------------------

/// Stand up a PRE-feature single-workspace install: a fresh schema migrated to
/// the pre-feature history (`0001`..`0008`) ONLY, with one workspace + its admin.
#[given(regex = r#"^a pre-feature single-workspace install of "([^"]+)" with admin "([^"]+)"$"#)]
async fn pre_feature_install(world: &mut FoundryWorld, ws_name: String, admin: String) {
    let (schema, pool, _url) = fresh_schema_pool_no_migrations().await;

    // Stage ONLY the pre-feature migration history (0001..0008) on disk and
    // apply it via the real runner — reconstructing a real pre-0009 schema.
    let staged = test_migration::stage_subset(8).expect("stage pre-feature migrations 0001..0008");
    run_migrations_from_dir(&pool, staged.path())
        .await
        .expect("apply pre-feature migration history");

    let workspace_id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(workspace_id)
        .bind(&ws_name)
        .execute(&pool)
        .await
        .expect("insert pre-feature workspace");

    let admin_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, 'Ops Admin', 'phc$dummy')",
    )
    .bind(admin_id)
    .bind(&admin)
    .execute(&pool)
    .await
    .expect("insert pre-feature admin user");

    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'admin')",
    )
    .bind(workspace_id)
    .bind(admin_id)
    .execute(&pool)
    .await
    .expect("insert pre-feature admin membership");

    world.mwt5_schema = Some(schema);
    world.mwt5_pool = Some(pool);
    world.mwt5_staged = Some(staged);
    world.mwt5_workspace_id = Some(workspace_id);
}

/// Seed representative tenant data: a member, a team + membership, a project,
/// two issues, and an invite — so the upgrade has real rows to leave untouched.
#[given(regex = r#"^"([^"]+)" has members, teams, projects, issues, and invites$"#)]
async fn install_has_tenant_data(world: &mut FoundryWorld, _ws_name: String) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let workspace_id = world.mwt5_workspace_id.expect("workspace seeded");

    let member_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $2, 'Member', 'phc$dummy')",
    )
    .bind(member_id)
    .bind("member@acme.com")
    .execute(&pool)
    .await
    .expect("insert member user");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(workspace_id)
    .bind(member_id)
    .execute(&pool)
    .await
    .expect("insert member membership");

    let team_id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'Core', 'core')")
        .bind(team_id)
        .bind(workspace_id)
        .execute(&pool)
        .await
        .expect("insert team");
    sqlx::query("INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'member')")
        .bind(team_id)
        .bind(member_id)
        .execute(&pool)
        .await
        .expect("insert team membership");

    let project_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, 'Apollo', 'apollo', 'APL')",
    )
    .bind(project_id)
    .bind(team_id)
    .bind(workspace_id)
    .execute(&pool)
    .await
    .expect("insert project");

    for (number, title) in [(1_i32, "First issue"), (2_i32, "Second issue")] {
        let issue_id = uuid::Uuid::now_v7();
        sqlx::query(
            "INSERT INTO issues (id, project_id, workspace_id, number, title, author_id)
                  VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(issue_id)
        .bind(project_id)
        .bind(workspace_id)
        .bind(number)
        .bind(title)
        .bind(member_id)
        .execute(&pool)
        .await
        .expect("insert issue");
    }

    let invite_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO invites (id, workspace_id, invitee_email, created_by, expires_at)
              VALUES ($1, $2, 'newcomer@acme.com', $3, now() + interval '7 days')",
    )
    .bind(invite_id)
    .bind(workspace_id)
    .bind(member_id)
    .execute(&pool)
    .await
    .expect("insert invite");
}

/// A live session + machine token from before the upgrade. Step 01-01's scenario
/// does not OBSERVE these (it asserts tenant-row idempotence only); seeding them
/// is a no-op placeholder so the Background step is satisfied. Later slice-05
/// steps (carried session/token resolution) replace this with real seeds.
#[given(regex = r#"^"([^"]+)" has a live signed-in session and a valid machine token$"#)]
async fn install_has_session_and_token(_world: &mut FoundryWorld, _ws_name: String) {
    // No observable contribution to the idempotent-re-upgrade scenario.
}

// ---------------------------------------------------------------------------
// Scenario 5: Re-running the upgrade does not duplicate or alter anything
// ---------------------------------------------------------------------------

/// Apply the canonical forward-only migration set (`0009`, `0010`, `0011`) ONCE
/// — the first upgrade — then snapshot every tenant table for later comparison.
#[given(regex = r#"^the install has already been upgraded to multi-workspace support$"#)]
async fn already_upgraded(world: &mut FoundryWorld) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let staged = world.mwt5_staged.as_ref().expect("staged dir present");

    // Add the forward-only migrations (0009, 0010, 0011) to the staged dir, then
    // apply the now-canonical set. The pre-feature history is already applied, so
    // only 0009/0010/0011 run.
    test_migration::add_forward_only_to(staged.path())
        .expect("stage forward-only migrations 0009/0010/0011");
    run_migrations_from_dir(&pool, staged.path())
        .await
        .expect("apply the forward-only upgrade");

    world.mwt5_snapshot_after_first = snapshot_tenant_tables(&pool).await;
}

/// Apply the SAME canonical migration set a second time. Idempotent
/// (`IF EXISTS` / `IF NOT EXISTS`): no SQL re-runs, nothing changes.
#[when(regex = r#"^the upgrade is applied a second time$"#)]
async fn upgrade_applied_again(world: &mut FoundryWorld) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let staged = world.mwt5_staged.as_ref().expect("staged dir present");

    run_migrations_from_dir(&pool, staged.path())
        .await
        .expect("re-applying the upgrade must not error");
}

/// The workspace is neither duplicated (still exactly one) nor altered (its id
/// unchanged from seed time).
#[then(regex = r#"^the workspace is neither duplicated nor altered$"#)]
async fn workspace_unchanged(world: &mut FoundryWorld) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let expected_id = world.mwt5_workspace_id.expect("workspace id captured");

    let count = sqlx::query_scalar::<_, i64>("SELECT count(*) FROM workspaces")
        .fetch_one(&pool)
        .await
        .expect("count workspaces");
    assert_eq!(
        count, 1,
        "the upgrade must NOT duplicate the workspace — exactly one must remain"
    );

    let id = sqlx::query_scalar::<_, uuid::Uuid>("SELECT id FROM workspaces")
        .fetch_one(&pool)
        .await
        .expect("read workspace id");
    assert_eq!(
        id, expected_id,
        "the existing workspace's identity must be unchanged by the re-upgrade"
    );
}

/// Every tenant row is byte-for-byte identical to the post-first-upgrade
/// snapshot — the re-upgrade rewrote nothing.
#[then(regex = r#"^every tenant row remains exactly as it was after the first upgrade$"#)]
async fn tenant_rows_unchanged(world: &mut FoundryWorld) {
    let pool = world.mwt5_pool.clone().expect("pre-feature pool seeded");
    let after_second = snapshot_tenant_tables(&pool).await;
    let after_first = &world.mwt5_snapshot_after_first;

    for table in TENANT_TABLES {
        assert_eq!(
            after_second.get(*table),
            after_first.get(*table),
            "re-applying the upgrade must leave every row of `{table}` exactly as it was"
        );
    }
}
