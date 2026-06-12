//! multi-workspace-provisioning — Slice 6 (US-MWT07) step definitions:
//! an instance super-admin provisions a NEW isolated workspace + first admin
//! from the operator CLI, and the new tenant is real, reachable, and isolated.
//!
//! This is the FIRST slice-06 module and the SLICE-06 WALKING SKELETON: it
//! stands up the slice-06 World glue + the thinnest end-to-end provisioning
//! vertical. Step 03-01 implements ONLY the `@walking_skeleton` scenario ("A
//! super-admin provisions a new isolated workspace with a first admin"). Later
//! slice-06 steps extend this glue with the untouched-existing-workspace,
//! cross-tenant-isolation, authz-refusal, and non-enumerability scenarios.
//!
//! Driving surface (feature header §"Driving adapter", ADR-002 / D2): the
//! operator CLI `foundry doctor provision-workspace --name <name>
//! --admin-email <addr> --as <super-admin-email>`, invoked as a REAL subprocess
//! (`assert_cmd::Command::cargo_bin("foundry")`) with `DATABASE_URL` pinned to
//! the per-scenario testcontainers schema (reusing the `run_restore_comment`
//! scaffold). The isolation/acts-on leg then rides the SHIPPED
//! `resolve_active_workspace` membership-resolution seam (the same seam the web
//! sign-in path uses to stamp `SessionUser.workspace_id`) — proving the
//! provisioned tenant obeys the already-shipped boundary.
//!
//! LAYER 3 (real adapter + real subprocess, @real-io @wiring_e2e): real
//! Postgres via testcontainers + a per-scenario schema (including the feature's
//! additive `0011_instance_admins`); the real `provision_workspace` tx; the real
//! `is_instance_admin` authz; the real resolution seam. Example-based
//! (Mandates 9 + 11) — no PBT machinery at this layer. Assertions are
//! traditional, over port-exposed observables: the CLI exit code + stdout (new
//! workspace id, invite link), the post-provision DB row presence scoped by
//! workspace, and the resolution seam's return for the first admin.

use crate::support::harness::{ensure_postgres, InProcHarness};
use crate::world::FoundryWorld;
use assert_cmd::Command as AssertCommand;
use cucumber::{given, then, when};
use secrecy::SecretString;
use sqlx::{PgPool, Row};

/// Resolve (or spawn) the slice-06 in-process harness. Its migrated schema is
/// the one the provisioning CLI subprocess targets via DATABASE_URL; reusing it
/// lets the "first admin acts on the new workspace" leg drive the SHIPPED
/// resolution seam against the very rows the subprocess just wrote.
async fn ensure_harness(world: &mut FoundryWorld) -> &InProcHarness {
    if world.mwt6_harness.is_none() {
        let harness = InProcHarness::spawn(time::OffsetDateTime::now_utc()).await;
        world.mwt6_harness = Some(harness);
    }
    world.mwt6_harness.as_ref().expect("mwt6 harness")
}

fn harness_pool(world: &FoundryWorld) -> PgPool {
    world
        .mwt6_harness
        .as_ref()
        .expect("mwt6 harness")
        .app
        .state
        .store
        .pool()
        .clone()
}

// ---------------------------------------------------------------------------
// Background
// ---------------------------------------------------------------------------

/// Seed an instance CLAIMED by a super-admin via the REAL bootstrap-claim path
/// (ADR-001 / D1, step 02-01): the SHIPPED `create_initial_workspace` seeding
/// transaction atomically creates workspace 1 + its admin (+ seeded team/project)
/// AND records that operator as the first `instance_admins` row. We drive the
/// production claim — NOT a fixture `instance_admins` insert — so every scenario
/// in this feature proves the bootstrap claim establishes the first super-admin
/// (a fresh instance never has a workspace 1 with no provisioning authority).
#[given(regex = r#"^an instance claimed by super-admin "([^"]+)" with workspace "([^"]+)"$"#)]
async fn instance_claimed_by_superadmin(world: &mut FoundryWorld, admin: String, ws_name: String) {
    ensure_harness(world).await;
    let store = world
        .mwt6_harness
        .as_ref()
        .expect("mwt6 harness")
        .app
        .state
        .store
        .clone();

    let workspace_id = uuid::Uuid::now_v7();
    let admin_id = uuid::Uuid::now_v7();
    let admin_lower = admin.to_ascii_lowercase();
    let admin_hash =
        foundry_auth::hash_password(&SecretString::new("ops-password".to_string().into()))
            .await
            .expect("hash super-admin pw");

    // The REAL bootstrap claim (the `submit` handler's seam): one atomic tx
    // creates ws1 + its admin + seeded team/project AND the first super-admin.
    store
        .create_initial_workspace(
            workspace_id,
            &ws_name,
            admin_id,
            &admin_lower,
            &admin,
            "Ops",
            &admin_hash,
            uuid::Uuid::now_v7(),
            "General",
            "general",
            uuid::Uuid::now_v7(),
            "Sandbox",
            "sandbox",
            "GEN",
        )
        .await
        .expect("real bootstrap claim seeds ws1 + its admin + first super-admin");

    world.mwt6_superadmin_email = Some(admin);
    world.mwt6_workspace_ids.insert(ws_name, workspace_id);
}

/// Seed an existing-workspace member with an issue, so the new tenant has a
/// real coexisting workspace to be isolated FROM.
#[given(regex = r#"^"([^"]+)" has a member "([^"]+)" with issues in "([^"]+)"$"#)]
async fn workspace_has_member_with_issues(
    world: &mut FoundryWorld,
    ws_name: String,
    member: String,
    _ws_name_again: String,
) {
    let pool = harness_pool(world);
    let workspace_id = *world
        .mwt6_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} must be seeded first"));

    let member_id = uuid::Uuid::now_v7();
    let member_lower = member.to_ascii_lowercase();
    let pw = foundry_auth::hash_password(&SecretString::new("member-password".to_string().into()))
        .await
        .expect("hash member pw");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, 'Member', $4) ON CONFLICT (email_lower) DO NOTHING",
    )
    .bind(member_id)
    .bind(&member_lower)
    .bind(&member)
    .bind(&pw)
    .execute(&pool)
    .await
    .expect("insert member user");
    let (member_id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(&member_lower)
        .fetch_one(&pool)
        .await
        .expect("resolve member id");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role)
              VALUES ($1, $2, 'member') ON CONFLICT DO NOTHING",
    )
    .bind(workspace_id)
    .bind(member_id)
    .execute(&pool)
    .await
    .expect("insert member membership");

    // A team + project + issue scoped to the existing workspace.
    let team_id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'Core', 'core')")
        .bind(team_id)
        .bind(workspace_id)
        .execute(&pool)
        .await
        .expect("insert team");
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
    let issue_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, author_id)
              VALUES ($1, $2, $3, 1, 'Existing issue', $4)",
    )
    .bind(issue_id)
    .bind(project_id)
    .bind(workspace_id)
    .bind(member_id)
    .execute(&pool)
    .await
    .expect("insert existing issue");
}

// ---------------------------------------------------------------------------
// Scenario 1 (walking skeleton)
// ---------------------------------------------------------------------------

/// Drive the REAL operator-CLI provisioning subprocess against the per-scenario
/// schema. DATABASE_URL pins the same search_path the in-process harness
/// migrated, SESSION_SECRET matches the harness's fixed test secret so the
/// emitted invite link is signable, and `--as` carries the bootstrap super-admin.
#[when(regex = r#"^the super-admin provisions workspace "([^"]+)" with first admin "([^"]+)"$"#)]
async fn superadmin_provisions(world: &mut FoundryWorld, ws_name: String, admin_email: String) {
    run_provision_cli(world, &ws_name, &admin_email).await;
}

/// Shared driving-port invocation: run the REAL operator-CLI `provision-workspace`
/// subprocess against the per-scenario schema, acting as the super-admin email
/// captured in the World, and stash the exit code + stdout for the Then steps.
async fn run_provision_cli(world: &mut FoundryWorld, ws_name: &str, admin_email: &str) {
    ensure_harness(world).await;
    let base = ensure_postgres().await;
    let schema = world
        .mwt6_harness
        .as_ref()
        .expect("mwt6 harness")
        .schema
        .clone();
    let database_url = format!("{base}?options=-csearch_path%3D{schema}");
    let acting = world
        .mwt6_superadmin_email
        .clone()
        .expect("super-admin seeded in Background");
    // The fixed test secret matches the InProcHarness session_secret so the
    // signed invite link is verifiable by the shipped server.
    let session_secret = "test-only-secret-must-be-at-least-32-bytes-long-please-yes".to_string();

    let name = ws_name.to_string();
    let email = admin_email.to_string();
    let output = tokio::task::spawn_blocking(move || {
        AssertCommand::cargo_bin("foundry")
            .expect("cargo-bin foundry")
            .env("DATABASE_URL", database_url)
            .env("SESSION_SECRET", session_secret)
            .env("FOUNDRY_PUBLIC_URL", "http://localhost")
            .args(["doctor", "provision-workspace"])
            .args(["--name", &name])
            .args(["--admin-email", &email])
            .args(["--as", &acting])
            .output()
            .expect("invoke foundry doctor provision-workspace")
    })
    .await
    .expect("join blocking cli");

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    world.mwt6_cli_exit = Some(output.status.code().unwrap_or(-1));
    world.mwt6_cli_stdout = Some(stdout);
}

/// The new workspace exists in the DB and is isolated from all others: its id
/// (parsed from the CLI stdout) names a workspace that is NOT any pre-existing
/// one, and it carries no rows belonging to another tenant.
#[then(regex = r#"^the new workspace "([^"]+)" exists and is isolated from all others$"#)]
async fn new_workspace_exists_isolated(world: &mut FoundryWorld, ws_name: String) {
    assert_eq!(
        world.mwt6_cli_exit,
        Some(0),
        "provision-workspace must exit 0; stdout={:?}",
        world.mwt6_cli_stdout
    );
    let pool = harness_pool(world);

    let (id, name): (uuid::Uuid, String) =
        sqlx::query_as("SELECT id, name FROM workspaces WHERE name = $1")
            .bind(&ws_name)
            .fetch_one(&pool)
            .await
            .unwrap_or_else(|e| panic!("provisioned workspace {ws_name:?} must exist: {e}"));
    assert_eq!(name, ws_name);
    world.mwt6_provisioned_workspace_id = Some(id);
    world.mwt6_workspace_ids.insert(ws_name, id);

    // Isolated: the new workspace is distinct from every pre-existing one, and
    // there is now more than one workspace (a real coexisting tenant).
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM workspaces")
        .fetch_one(&pool)
        .await
        .expect("count workspaces");
    assert!(
        count >= 2,
        "provisioning must create an ADDITIONAL workspace beside the existing one (got {count})"
    );
    // The new workspace owns no issues yet (it starts empty / isolated — no
    // foreign tenant's rows leaked into it).
    let issue_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM issues WHERE workspace_id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("count new-workspace issues");
    assert_eq!(
        issue_count, 0,
        "the freshly-provisioned workspace must start with no issues (isolation)"
    );
}

/// The CLI's port-exposed stdout reports the new workspace id + a first-admin
/// invite link.
#[then(regex = r#"^the command reports the new workspace and a first-admin invite link$"#)]
async fn command_reports_workspace_and_invite(world: &mut FoundryWorld) {
    let stdout = world
        .mwt6_cli_stdout
        .as_deref()
        .expect("provision CLI stdout captured");
    let workspace_id = world
        .mwt6_provisioned_workspace_id
        .expect("provisioned workspace id parsed");
    assert!(
        stdout.contains(&workspace_id.to_string()),
        "stdout must report the new workspace id {workspace_id}; got {stdout:?}"
    );
    assert!(
        stdout.contains("invite-link:") && stdout.contains("/invites/accept?id="),
        "stdout must report a first-admin invite link; got {stdout:?}"
    );
}

/// The seeded first admin is real and reachable on the new tenant: the SHIPPED
/// `resolve_active_workspace` membership-resolution seam (the driving port the
/// web sign-in path uses to stamp the session's workspace) resolves her to the
/// new workspace and ONLY that workspace.
#[then(regex = r#"^"([^"]+)" signs in and acts on "([^"]+)"$"#)]
async fn first_admin_acts_on(world: &mut FoundryWorld, admin_email: String, ws_name: String) {
    let store = world
        .mwt6_harness
        .as_ref()
        .expect("mwt6 harness")
        .app
        .state
        .store
        .clone();
    let expected_id = *world
        .mwt6_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} id must be known"));

    let admin_id = store
        .user_id_by_email(&admin_email.to_ascii_lowercase())
        .await
        .expect("query first admin")
        .unwrap_or_else(|| panic!("first admin {admin_email:?} must exist after provisioning"));

    let resolved = store
        .resolve_active_workspace(admin_id)
        .await
        .expect("resolve active workspace")
        .unwrap_or_else(|| panic!("first admin {admin_email:?} must resolve to a workspace"));

    assert_eq!(
        resolved.0, expected_id,
        "the first admin must land on the provisioned workspace {ws_name:?}, not another tenant"
    );
    assert_eq!(
        resolved.1, ws_name,
        "resolved workspace name must be {ws_name:?}"
    );
}

// ---------------------------------------------------------------------------
// Scenario 2 — provisioning a new workspace leaves existing ones untouched
//              (NFR-MWT-REL-01 / D4 untouched-A proof)
// ---------------------------------------------------------------------------

/// Every tenant-scoped table that carries a `workspace_id`. The snapshot keys
/// on `(table, existing-workspace-id)` so the proof is row-for-row over ONLY
/// the pre-existing tenant's rows — provisioning a new workspace must not touch
/// any of them.
const EXISTING_TENANT_TABLES: &[&str] = &[
    "workspaces",
    "workspace_memberships",
    "teams",
    "projects",
    "issues",
    "invites",
];

/// Snapshot every row belonging to the EXISTING workspace as an ordered list of
/// whole-row JSON strings, keyed by table name. `to_jsonb(t.*)` renders the
/// entire row deterministically; ordering by the row text makes the comparison
/// insertion-order independent. The `workspaces` table keys on `id`; every other
/// tenant table keys on `workspace_id`. Users are scoped via their membership in
/// the existing workspace (a `users` row has no `workspace_id`).
async fn snapshot_existing_tenant(
    pool: &PgPool,
    existing_workspace_id: uuid::Uuid,
) -> std::collections::HashMap<String, Vec<String>> {
    let mut out = std::collections::HashMap::new();
    for table in EXISTING_TENANT_TABLES {
        let id_column = if *table == "workspaces" {
            "id"
        } else {
            "workspace_id"
        };
        let sql = format!(
            "SELECT to_jsonb(t.*)::text AS row_json FROM {table} t \
             WHERE t.{id_column} = $1 ORDER BY row_json"
        );
        let rows = sqlx::query(&sql)
            .bind(existing_workspace_id)
            .fetch_all(pool)
            .await
            .unwrap_or_else(|e| panic!("snapshot existing {table}: {e}"));
        let row_jsons = rows
            .into_iter()
            .map(|r| r.get::<String, _>("row_json"))
            .collect();
        out.insert((*table).to_string(), row_jsons);
    }
    // The existing workspace's member users (scoped via membership), keyed under
    // a synthetic "users" slot — a users row carries no workspace_id, so we
    // resolve them through workspace_memberships.
    let sql = "SELECT to_jsonb(u.*)::text AS row_json \
               FROM users u \
               JOIN workspace_memberships m ON m.user_id = u.id \
               WHERE m.workspace_id = $1 ORDER BY row_json";
    let rows = sqlx::query(sql)
        .bind(existing_workspace_id)
        .fetch_all(pool)
        .await
        .expect("snapshot existing users");
    out.insert(
        "users".to_string(),
        rows.into_iter()
            .map(|r| r.get::<String, _>("row_json"))
            .collect(),
    );
    // team_memberships carries no workspace_id — it is scoped through its team's
    // workspace. Snapshot the rows whose team belongs to the existing workspace.
    let sql = "SELECT to_jsonb(tm.*)::text AS row_json \
               FROM team_memberships tm \
               JOIN teams t ON t.id = tm.team_id \
               WHERE t.workspace_id = $1 ORDER BY row_json";
    let rows = sqlx::query(sql)
        .bind(existing_workspace_id)
        .fetch_all(pool)
        .await
        .expect("snapshot existing team_memberships");
    out.insert(
        "team_memberships".to_string(),
        rows.into_iter()
            .map(|r| r.get::<String, _>("row_json"))
            .collect(),
    );
    out
}

/// Record a row-level before-snapshot of the existing workspace ("Acme") and all
/// its data + members. The After step compares the same workspace's rows after
/// provisioning to prove they are unchanged row-for-row (NFR-MWT-REL-01).
#[given(regex = r#"^a recorded snapshot of "([^"]+)" and its data and members$"#)]
async fn recorded_snapshot_of_existing(world: &mut FoundryWorld, ws_name: String) {
    let existing_id = *world
        .mwt6_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("existing workspace {ws_name:?} must be seeded first"));
    let pool = harness_pool(world);
    world.mwt6_existing_snapshot = snapshot_existing_tenant(&pool, existing_id).await;
}

/// AC 1/2/4: the existing workspace and all its data + members are unchanged
/// row-for-row after provisioning a NEW workspace — the after-snapshot equals
/// the before-snapshot exactly (no row written, updated, or deleted in any
/// pre-existing tenant). Proven against the real database, not by inspecting
/// internal call paths.
#[then(regex = r#"^"([^"]+)" and all its data and members are unchanged$"#)]
async fn existing_workspace_unchanged(world: &mut FoundryWorld, ws_name: String) {
    assert_eq!(
        world.mwt6_cli_exit,
        Some(0),
        "provision-workspace must exit 0 before proving non-interference; stdout={:?}",
        world.mwt6_cli_stdout
    );
    let existing_id = *world
        .mwt6_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("existing workspace {ws_name:?} id must be known"));
    let pool = harness_pool(world);
    let after = snapshot_existing_tenant(&pool, existing_id).await;
    let before = &world.mwt6_existing_snapshot;
    assert!(
        !before.is_empty(),
        "a before-snapshot of {ws_name:?} must have been recorded"
    );
    for (table, before_rows) in before {
        let after_rows = after
            .get(table)
            .unwrap_or_else(|| panic!("after-snapshot missing table {table:?}"));
        assert_eq!(
            after_rows, before_rows,
            "provisioning a new workspace must leave {ws_name:?}'s {table} rows \
             unchanged row-for-row (before={before_rows:?}, after={after_rows:?})"
        );
    }
}

/// AC 3: the newly-provisioned workspace's identity is distinct from every
/// pre-existing workspace, and it starts empty + isolated — no foreign tenant's
/// rows leaked into it. AC 4's no-cross-write is the inverse, proven by the
/// unchanged-existing step above.
#[then(regex = r#"^"([^"]+)" starts empty and isolated$"#)]
async fn new_workspace_starts_empty_isolated(world: &mut FoundryWorld, ws_name: String) {
    let pool = harness_pool(world);
    let (new_id, name): (uuid::Uuid, String) =
        sqlx::query_as("SELECT id, name FROM workspaces WHERE name = $1")
            .bind(&ws_name)
            .fetch_one(&pool)
            .await
            .unwrap_or_else(|e| panic!("provisioned workspace {ws_name:?} must exist: {e}"));
    assert_eq!(name, ws_name);
    world.mwt6_provisioned_workspace_id = Some(new_id);
    world.mwt6_workspace_ids.insert(ws_name.clone(), new_id);

    // Distinct identity: the new workspace id is not any pre-existing one.
    let existing_ids: Vec<uuid::Uuid> = world
        .mwt6_workspace_ids
        .iter()
        .filter(|(n, _)| n.as_str() != ws_name)
        .map(|(_, id)| *id)
        .collect();
    assert!(
        !existing_ids.contains(&new_id),
        "the provisioned workspace id {new_id} must be distinct from every existing one"
    );

    // Empty + isolated: the new workspace owns no issues, teams, or projects —
    // no foreign tenant's rows leaked in.
    for table in ["issues", "teams", "projects"] {
        let sql = format!("SELECT count(*) FROM {table} WHERE workspace_id = $1");
        let count: i64 = sqlx::query_scalar(&sql)
            .bind(new_id)
            .fetch_one(&pool)
            .await
            .unwrap_or_else(|e| panic!("count new-workspace {table}: {e}"));
        assert_eq!(
            count, 0,
            "the freshly-provisioned workspace must start with no {table} (isolation)"
        );
    }
}

// ---------------------------------------------------------------------------
// Scenario 8 — upgraded installs gain a super-admin via grant (ADR-001 / D1)
// ---------------------------------------------------------------------------

/// Model an UPGRADED install: workspace "Acme" + its admin exist, but there is
/// NO `instance_admins` row yet (the pre-super-admin-role world of an install
/// that predates the bootstrap seed). The Background claimed the instance via the
/// SHIPPED `create_initial_workspace` (which now ALSO seeds the first super-admin
/// per step 02-01) — so to authentically model the upgraded state we DELETE the
/// `instance_admins` rows, leaving ws1 + its admin intact. The install thus has
/// the workspace and its admin but NO provisioning authority until granted.
#[given(regex = r#"^an upgraded instance with workspace "([^"]+)" and no super-admin yet$"#)]
async fn upgraded_instance_no_super_admin(world: &mut FoundryWorld, ws_name: String) {
    ensure_harness(world).await;
    let pool = harness_pool(world);

    // Strip the bootstrap-seeded super-admin authority → an install with a
    // workspace + admin but no super-admin (the upgrade starting point).
    sqlx::query("DELETE FROM instance_admins")
        .execute(&pool)
        .await
        .expect("clear instance_admins to model an upgraded install");

    let supers: i64 = sqlx::query_scalar("SELECT count(*) FROM instance_admins")
        .fetch_one(&pool)
        .await
        .expect("count instance_admins on upgraded install");
    assert_eq!(supers, 0, "an upgraded install starts with no super-admin");

    // Acme + its admin "ops@acme.com" remain from the Background claim.
    assert!(
        world.mwt6_workspace_ids.contains_key(&ws_name),
        "workspace {ws_name:?} must already exist from the Background claim"
    );
    world.mwt6_superadmin_email = Some("ops@acme.com".to_string());
}

/// Drive the REAL operator-CLI `grant-super-admin` subprocess against the
/// per-scenario schema — the upgrade path that records the operator as the first
/// instance super-admin (idempotent `ON CONFLICT DO NOTHING`). Mirrors
/// `run_provision_cli`'s subprocess wiring + structured exit code.
#[when(regex = r#"^"([^"]+)" is granted super-admin$"#)]
async fn operator_is_granted_super_admin(world: &mut FoundryWorld, operator: String) {
    run_grant_cli(world, &operator).await;
}

/// Second grant for the SAME operator — the idempotence leg. The CLI must exit 0
/// again and record the grant exactly once (no second `instance_admins` row).
#[when(regex = r#"^"([^"]+)" is granted super-admin a second time$"#)]
async fn operator_is_granted_super_admin_again(world: &mut FoundryWorld, operator: String) {
    run_grant_cli(world, &operator).await;
}

/// Shared driving-port invocation: run the REAL operator-CLI `grant-super-admin`
/// subprocess against the per-scenario schema and stash the exit code + stdout.
async fn run_grant_cli(world: &mut FoundryWorld, operator: &str) {
    ensure_harness(world).await;
    let base = ensure_postgres().await;
    let schema = world
        .mwt6_harness
        .as_ref()
        .expect("mwt6 harness")
        .schema
        .clone();
    let database_url = format!("{base}?options=-csearch_path%3D{schema}");

    let email = operator.to_string();
    let output = tokio::task::spawn_blocking(move || {
        AssertCommand::cargo_bin("foundry")
            .expect("cargo-bin foundry")
            .env("DATABASE_URL", database_url)
            .args(["doctor", "grant-super-admin"])
            .args(["--email", &email])
            .output()
            .expect("invoke foundry doctor grant-super-admin")
    })
    .await
    .expect("join blocking cli");

    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    world.mwt6_cli_exit = Some(output.status.code().unwrap_or(-1));
    world.mwt6_cli_stdout = Some(stdout);
}

/// AC 1/2: after granting (twice, for the same operator), exactly one
/// `instance_admins` row exists for that operator — the grant is idempotent.
#[then(regex = r#"^the grant is recorded exactly once$"#)]
async fn grant_recorded_exactly_once(world: &mut FoundryWorld) {
    assert_eq!(
        world.mwt6_cli_exit,
        Some(0),
        "grant-super-admin must exit 0; stdout={:?}",
        world.mwt6_cli_stdout
    );
    let pool = harness_pool(world);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM instance_admins")
        .fetch_one(&pool)
        .await
        .expect("count instance_admins after grant");
    assert_eq!(
        count, 1,
        "granting (even twice) records the operator as super-admin exactly once"
    );
}

/// AC 3: the granted operator now passes `is_instance_admin` and can provision a
/// NEW workspace through the same operator-CLI provisioning subprocess. This is
/// an `And` following a `Then`, so cucumber dispatches it as a `then` step.
#[then(regex = r#"^"([^"]+)" can then provision workspace "([^"]+)" with first admin "([^"]+)"$"#)]
async fn granted_operator_can_provision(
    world: &mut FoundryWorld,
    operator: String,
    ws_name: String,
    admin_email: String,
) {
    world.mwt6_superadmin_email = Some(operator);
    run_provision_cli(world, &ws_name, &admin_email).await;
    assert_eq!(
        world.mwt6_cli_exit,
        Some(0),
        "the granted operator must be able to provision (exit 0); stdout={:?}",
        world.mwt6_cli_stdout
    );
    let pool = harness_pool(world);
    let (id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM workspaces WHERE name = $1")
        .bind(&ws_name)
        .fetch_one(&pool)
        .await
        .unwrap_or_else(|e| {
            panic!("granted operator's provisioned workspace {ws_name:?} must exist: {e}")
        });
    world.mwt6_workspace_ids.insert(ws_name, id);
}

// ---------------------------------------------------------------------------
// Scenario 5 — a non-super-admin cannot provision (authz core, evil-user)
// ---------------------------------------------------------------------------

/// Confirm the named member is a REGULAR workspace member and NOT a super-admin:
/// the SHIPPED `is_instance_admin` authz over the rows the real bootstrap claim +
/// Background seeded returns false for them. They are a `workspace_memberships`
/// member of "Acme" (seeded in the Background) with no `instance_admins` row — the
/// fail-closed starting state the provisioning gate must refuse. We also record
/// the workspace count so the refusal can prove no new workspace was created.
#[given(regex = r#"^"([^"]+)" is a regular member and not a super-admin$"#)]
async fn member_is_not_super_admin(world: &mut FoundryWorld, member: String) {
    let store = world
        .mwt6_harness
        .as_ref()
        .expect("mwt6 harness")
        .app
        .state
        .store
        .clone();
    let member_id = store
        .user_id_by_email(&member.to_ascii_lowercase())
        .await
        .expect("query member")
        .unwrap_or_else(|| panic!("member {member:?} must exist from the Background"));
    assert!(
        !store
            .is_instance_admin(member_id)
            .await
            .expect("is_instance_admin"),
        "a regular member {member:?} must NOT be an instance super-admin (fail-closed)"
    );

    let pool = harness_pool(world);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM workspaces")
        .fetch_one(&pool)
        .await
        .expect("count workspaces before the unauthorized attempt");
    world.mwt6_workspaces_before_attempt = Some(count);
    world.mwt6_superadmin_email = Some(member);
}

/// The non-super-admin attempts to provision through the REAL operator-CLI
/// subprocess, acting as themselves (`--as <member>`). The gate must refuse
/// fail-closed; we stash the exit code for the Then steps.
#[when(
    regex = r#"^"([^"]+)" attempts to provision workspace "([^"]+)" with first admin "([^"]+)"$"#
)]
async fn member_attempts_to_provision(
    world: &mut FoundryWorld,
    member: String,
    ws_name: String,
    admin_email: String,
) {
    world.mwt6_superadmin_email = Some(member);
    run_provision_cli(world, &ws_name, &admin_email).await;
}

/// AC: the attempt is refused as NOT AUTHORIZED — the CLI exits with the
/// structured "not authorized" exit code (4), the `ServiceError::Forbidden`
/// fail-closed refusal from the `is_instance_admin` gate.
#[then(regex = r#"^the attempt is refused as not authorized$"#)]
async fn attempt_refused_not_authorized(world: &mut FoundryWorld) {
    assert_eq!(
        world.mwt6_cli_exit,
        Some(4),
        "a non-super-admin's provisioning attempt must be refused with the \
         structured not-authorized exit code (4); stdout={:?}",
        world.mwt6_cli_stdout
    );
}

/// AC: the refused attempt created NO new workspace — the workspace count is
/// unchanged from before the attempt (the fail-closed gate refuses BEFORE the
/// provision transaction runs, so the evil user's `Sneaky` workspace never lands).
#[then(regex = r#"^no new workspace was created$"#)]
async fn no_new_workspace_created(world: &mut FoundryWorld) {
    let before = world
        .mwt6_workspaces_before_attempt
        .expect("workspace count recorded before the attempt");
    let pool = harness_pool(world);
    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM workspaces")
        .fetch_one(&pool)
        .await
        .expect("count workspaces after the unauthorized attempt");
    assert_eq!(
        after, before,
        "a refused provisioning attempt must create NO new workspace \
         (count before={before}, after={after})"
    );
}

// ---------------------------------------------------------------------------
// Scenario 7 — the bootstrap-claiming operator is the first super-admin (D1)
// ---------------------------------------------------------------------------

/// Confirm the named operator was established as the first super-admin by the
/// REAL bootstrap claim (run in the Background via `create_initial_workspace`).
/// This rides the SHIPPED `is_instance_admin` authz over the rows the production
/// claim seeded — NOT a fixture insert — proving D1: claiming the instance
/// makes the operator both ws1's admin AND the first super-admin.
#[given(regex = r#"^"([^"]+)" claimed the instance at bootstrap$"#)]
async fn operator_claimed_at_bootstrap(world: &mut FoundryWorld, operator: String) {
    let store = world
        .mwt6_harness
        .as_ref()
        .expect("mwt6 harness")
        .app
        .state
        .store
        .clone();
    let operator_id = store
        .user_id_by_email(&operator.to_ascii_lowercase())
        .await
        .expect("query bootstrap operator")
        .unwrap_or_else(|| panic!("bootstrap operator {operator:?} must exist after the claim"));
    assert!(
        store
            .is_instance_admin(operator_id)
            .await
            .expect("is_instance_admin"),
        "the bootstrap-claiming operator {operator:?} must be the first super-admin (D1)"
    );
    world.mwt6_superadmin_email = Some(operator);
}

/// The named bootstrap operator provisions a new workspace through the REAL
/// operator-CLI subprocess (same driving surface as the walking skeleton),
/// acting as `--as <operator>` — the super-admin seeded by the real claim.
#[when(regex = r#"^"([^"]+)" provisions workspace "([^"]+)" with first admin "([^"]+)"$"#)]
async fn operator_provisions(
    world: &mut FoundryWorld,
    operator: String,
    ws_name: String,
    admin_email: String,
) {
    world.mwt6_superadmin_email = Some(operator);
    run_provision_cli(world, &ws_name, &admin_email).await;
}

/// The provisioning command succeeded (exit 0) — the bootstrap operator's
/// super-admin authority (seeded by the real claim) passed `is_instance_admin`.
#[then(regex = r#"^the provisioning succeeds$"#)]
async fn provisioning_succeeds(world: &mut FoundryWorld) {
    assert_eq!(
        world.mwt6_cli_exit,
        Some(0),
        "provision-workspace must succeed (exit 0); stdout={:?}",
        world.mwt6_cli_stdout
    );
}

/// The provisioned workspace exists and coexists as an isolated tenant beside
/// the bootstrap workspace. Mirrors the walking-skeleton isolation assertion.
#[then(regex = r#"^"([^"]+)" exists and is isolated from all others$"#)]
async fn workspace_exists_isolated(world: &mut FoundryWorld, ws_name: String) {
    let pool = harness_pool(world);
    let (id, name): (uuid::Uuid, String) =
        sqlx::query_as("SELECT id, name FROM workspaces WHERE name = $1")
            .bind(&ws_name)
            .fetch_one(&pool)
            .await
            .unwrap_or_else(|e| panic!("provisioned workspace {ws_name:?} must exist: {e}"));
    assert_eq!(name, ws_name);
    world.mwt6_provisioned_workspace_id = Some(id);
    world.mwt6_workspace_ids.insert(ws_name, id);

    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM workspaces")
        .fetch_one(&pool)
        .await
        .expect("count workspaces");
    assert!(
        count >= 2,
        "provisioning must create an ADDITIONAL workspace beside the bootstrap one (got {count})"
    );
    let issue_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM issues WHERE workspace_id = $1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .expect("count new-workspace issues");
    assert_eq!(
        issue_count, 0,
        "the freshly-provisioned workspace must start with no issues (isolation)"
    );
}
