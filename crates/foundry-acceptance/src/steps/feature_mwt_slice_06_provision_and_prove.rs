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
use sqlx::PgPool;

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

/// Seed an instance CLAIMED by a super-admin: a workspace + its admin user +
/// admin membership, AND the `instance_admins` row that makes the admin the
/// first super-admin (the bootstrap-claim path, ADR-001 / D1 — the seed CLI is
/// step 02-02; this Background uses a direct insert fixture per the task brief).
#[given(regex = r#"^an instance claimed by super-admin "([^"]+)" with workspace "([^"]+)"$"#)]
async fn instance_claimed_by_superadmin(world: &mut FoundryWorld, admin: String, ws_name: String) {
    ensure_harness(world).await;
    let pool = harness_pool(world);

    let workspace_id = uuid::Uuid::now_v7();
    let admin_id = uuid::Uuid::now_v7();
    let admin_lower = admin.to_ascii_lowercase();
    let admin_hash =
        foundry_auth::hash_password(&SecretString::new("ops-password".to_string().into()))
            .await
            .expect("hash super-admin pw");

    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(workspace_id)
        .bind(&ws_name)
        .execute(&pool)
        .await
        .expect("insert claimed workspace");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, 'Ops', $4) ON CONFLICT (email_lower) DO NOTHING",
    )
    .bind(admin_id)
    .bind(&admin_lower)
    .bind(&admin)
    .bind(&admin_hash)
    .execute(&pool)
    .await
    .expect("insert super-admin user");
    let (admin_id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(&admin_lower)
        .fetch_one(&pool)
        .await
        .expect("resolve super-admin user id");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role)
              VALUES ($1, $2, 'admin')",
    )
    .bind(workspace_id)
    .bind(admin_id)
    .execute(&pool)
    .await
    .expect("insert super-admin membership");
    // The bootstrap-claiming operator IS the first instance super-admin (D1).
    sqlx::query("INSERT INTO instance_admins (user_id) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(admin_id)
        .execute(&pool)
        .await
        .expect("seed first instance super-admin");

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

    let name = ws_name.clone();
    let email = admin_email.clone();
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
