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
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use secrecy::SecretString;
use sqlx::{PgPool, Row};

/// The password the Background seeds every existing-workspace member with
/// (`workspace_has_member_with_issues`). The web sign-in path re-authenticates
/// per request (no cookie jar), so the cross-tenant probe needs it.
const MEMBER_PASSWORD: &str = "member-password";

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
// Scenario 3 — the provisioned workspace is a real coexisting tenant that sees
//              only its own data (US-MWT08 isolation leg, NFR-MWT-SEC-01).
//
// Green-by-inheritance: this scenario adds NO new isolation code. It provisions
// Globex through the REAL CLI (as in the walking skeleton), seeds Globex with
// its OWN team/project/issues using the SAME slugs Acme uses (so the only thing
// distinguishing the two tenants' reads is the acting workspace), then drives
// the SHIPPED scoped-read seam exactly as `list_board_issues` does —
// `resolve_active_workspace(priya)` (the same membership-resolution seam ws1
// uses) → `find_team_by_slug(acting_ws, slug)` → `find_project_by_slug` →
// `list_issues_by_project`. Because the acting workspace is resolved through the
// shipped seam and the team lookup is workspace-scoped, Priya sees only Globex's
// issues and never Acme's. Falsifiability: resolving Priya to Acme's workspace
// (ignoring the acting workspace) would surface Acme's "Existing issue" —
// demonstrated in the unit isolation assertion below.
// ---------------------------------------------------------------------------

/// `Given the super-admin has provisioned workspace "Globex" with first admin …`
/// — drive the REAL provisioning CLI subprocess (same driving port as the
/// walking skeleton) and capture the new workspace's id, so the isolation leg
/// reads against the very rows the subprocess wrote.
#[given(
    regex = r#"^the super-admin has provisioned workspace "([^"]+)" with first admin "([^"]+)"$"#
)]
async fn super_admin_has_provisioned(
    world: &mut FoundryWorld,
    ws_name: String,
    admin_email: String,
) {
    run_provision_cli(world, &ws_name, &admin_email).await;
    assert_eq!(
        world.mwt6_cli_exit,
        Some(0),
        "provisioning must succeed before proving isolation; stdout={:?}",
        world.mwt6_cli_stdout
    );
    let pool = harness_pool(world);
    let (id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM workspaces WHERE name = $1")
        .bind(&ws_name)
        .fetch_one(&pool)
        .await
        .unwrap_or_else(|e| panic!("provisioned workspace {ws_name:?} must exist: {e}"));
    world.mwt6_provisioned_workspace_id = Some(id);
    world.mwt6_workspace_ids.insert(ws_name, id);
    world.mwt6_first_admin_email = Some(admin_email);
}

/// `And "Globex" has issues that belong to "Globex"` — seed the provisioned
/// tenant with its OWN team/project/issue, AND make the seeded first admin a
/// member of that team so the shipped membership-gated scoped read returns her
/// board. The team/project slugs DELIBERATELY match Acme's ("core"/"apollo") so
/// the only variable distinguishing the two tenants' reads is the acting
/// workspace — a scope leak would surface Acme's issue under Globex's slugs.
#[given(regex = r#"^"([^"]+)" has issues that belong to "([^"]+)"$"#)]
async fn provisioned_workspace_has_own_issues(
    world: &mut FoundryWorld,
    ws_name: String,
    _ws_name_again: String,
) {
    let pool = harness_pool(world);
    let workspace_id = *world
        .mwt6_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("provisioned workspace {ws_name:?} must exist first"));
    let admin_email = world
        .mwt6_first_admin_email
        .clone()
        .expect("provisioned first admin recorded");

    let admin_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email_lower = $1")
        .bind(admin_email.to_ascii_lowercase())
        .fetch_one(&pool)
        .await
        .expect("provisioned first admin exists");

    // Team + project scoped to the provisioned workspace, same slugs as Acme.
    let team_id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'Core', 'core')")
        .bind(team_id)
        .bind(workspace_id)
        .execute(&pool)
        .await
        .expect("insert provisioned-tenant team");
    sqlx::query("INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'lead')")
        .bind(team_id)
        .bind(admin_id)
        .execute(&pool)
        .await
        .expect("first admin joins her workspace's team");
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
    .expect("insert provisioned-tenant project");
    let issue_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, author_id)
              VALUES ($1, $2, $3, 1, 'Globex-only issue', $4)",
    )
    .bind(issue_id)
    .bind(project_id)
    .bind(workspace_id)
    .bind(admin_id)
    .execute(&pool)
    .await
    .expect("insert provisioned-tenant issue");

    world
        .mwt6_provisioned_issue_titles
        .insert(ws_name, vec!["Globex-only issue".to_string()]);
}

/// `When "priya@globex.com" lists her issues` — drive the SHIPPED scoped-read
/// seam through the resolution seam (the same one ws1 uses): resolve her active
/// workspace, then read her board via the workspace-scoped team→project→issues
/// chain `list_board_issues` walks. No new isolation code — green by inheritance.
#[when(regex = r#"^"([^"]+)" lists her issues$"#)]
async fn first_admin_lists_her_issues(world: &mut FoundryWorld, admin_email: String) {
    let titles = read_board_titles_via_resolution(world, &admin_email).await;
    world.mwt6_listed_issue_titles = titles;
}

/// Resolve `admin_email`'s acting workspace through the SHIPPED
/// `resolve_active_workspace` seam, then read the `core`/`apollo` board scoped to
/// THAT acting workspace — exactly the chain the shipped `list_board_issues`
/// application port walks (`find_team_by_slug(acting_ws, …)` →
/// `find_project_by_slug` → `list_issues_by_project`). Returns the issue titles
/// the caller is permitted to see. Enforces the shipped membership gate.
async fn read_board_titles_via_resolution(
    world: &mut FoundryWorld,
    admin_email: &str,
) -> Vec<String> {
    let store = world
        .mwt6_harness
        .as_ref()
        .expect("mwt6 harness")
        .app
        .state
        .store
        .clone();
    let user_id = store
        .user_id_by_email(&admin_email.to_ascii_lowercase())
        .await
        .expect("query user")
        .unwrap_or_else(|| panic!("{admin_email:?} must exist"));

    // SHIPPED resolution seam — the SAME seam workspace 1 uses to stamp the
    // session's acting workspace (AC5: the provisioned tenant is resolved
    // through the same seam as workspace 1).
    let (acting_workspace_id, _name) = store
        .resolve_active_workspace(user_id)
        .await
        .expect("resolve active workspace")
        .unwrap_or_else(|| panic!("{admin_email:?} must resolve to an acting workspace"));

    board_titles_scoped(&store, acting_workspace_id, user_id).await
}

/// The SHIPPED scoped-read chain, extracted so the falsifiability mutation can
/// drive it with a DIFFERENT acting workspace and observe the leak. Membership-
/// gated (a non-member sees nothing), workspace-scoped at the team lookup.
async fn board_titles_scoped(
    store: &foundry_store::Store,
    acting_workspace_id: uuid::Uuid,
    user_id: uuid::Uuid,
) -> Vec<String> {
    let Some(team) = store
        .find_team_by_slug(acting_workspace_id, "core")
        .await
        .expect("find team by slug scoped to acting workspace")
    else {
        return Vec::new();
    };
    if !store
        .is_team_member(team.id, user_id)
        .await
        .expect("team membership gate")
    {
        return Vec::new();
    }
    let Some(project) = store
        .find_project_by_slug(team.id, "apollo")
        .await
        .expect("find project by slug")
    else {
        return Vec::new();
    };
    store
        .list_issues_by_project(project.id)
        .await
        .expect("scoped issue read")
        .into_iter()
        .map(|row| row.title)
        .collect()
}

/// `Then she sees only "Globex" issues` — the scoped read returns EXACTLY the
/// provisioned tenant's own issues (AC1).
#[then(regex = r#"^she sees only "([^"]+)" issues$"#)]
async fn sees_only_own_issues(world: &mut FoundryWorld, ws_name: String) {
    let expected = world
        .mwt6_provisioned_issue_titles
        .get(&ws_name)
        .cloned()
        .unwrap_or_else(|| panic!("provisioned tenant {ws_name:?} issues seeded"));
    let mut listed = world.mwt6_listed_issue_titles.clone();
    let mut expected_sorted = expected.clone();
    listed.sort();
    expected_sorted.sort();
    assert_eq!(
        listed, expected_sorted,
        "the provisioned tenant's admin must see ONLY {ws_name:?}'s own issues \
         (expected={expected_sorted:?}, got={listed:?})"
    );
}

/// `And no "Acme" issue appears` — none of the EXISTING workspace's issues leak
/// into the provisioned tenant's scoped read (AC2). Asserted by title against
/// the Background-seeded existing-workspace issue.
#[then(regex = r#"^no "([^"]+)" issue appears$"#)]
async fn no_existing_issue_appears(world: &mut FoundryWorld, _existing_ws: String) {
    assert!(
        !world
            .mwt6_listed_issue_titles
            .iter()
            .any(|t| t == "Existing issue"),
        "no existing-workspace issue may appear in the provisioned tenant's scoped \
         read; got={:?}",
        world.mwt6_listed_issue_titles
    );
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

// ---------------------------------------------------------------------------
// Scenario 4 — a member of the EXISTING workspace cannot reach the provisioned
//              one non-enumerably (US-MWT08 / NFR-MWT-SEC-02, evil-user).
//
// Green-by-inheritance: this scenario adds NO new isolation code. It provisions
// Globex through the REAL CLI (reusing scenario-3's `Given the super-admin has
// provisioned …`), seeds Globex with its OWN team/project/issue under a slug
// DISTINCT from Acme's (so the foreign address genuinely does not resolve inside
// Acme's acting workspace), then drives the SHIPPED web issue-detail handler
// (`show_issue`, comments.rs:58) over real HTTP as the existing-workspace member
// Marco — signed in and resolved (ADR-005) to Acme. The handler scopes by the
// RESOLVED acting workspace: a FOREIGN team/project/issue resolves to `None`
// through `find_team_by_slug(acting, …)` EXACTLY as a never-existed one does, and
// BOTH render the SINGLE uniform `resource_not_found_page()` with no slug/number
// echoed (the SHIPPED uniform-404 idiom, ADR-003). So the two requests are
// byte-identical — no 403-vs-404 oracle, no body that reveals the Globex issue
// exists.
//
// Falsifiability (demonstrated at RED, then restored): make the foreign-resource
// path 403 (or echo the foreign slug into the refusal body) and the two responses
// DIFFER → this scenario reds. The shipped handler does neither, so it greens.
// ---------------------------------------------------------------------------

fn http_client(world: &mut FoundryWorld) -> reqwest::Client {
    if world.http.is_none() {
        world.http = Some(
            reqwest::Client::builder()
                .redirect(Policy::none())
                .cookie_store(false)
                .build()
                .expect("build reqwest client"),
        );
    }
    world.http.as_ref().expect("http client").clone()
}

/// `And an issue belongs to "Globex"` — seed the provisioned tenant with its OWN
/// team/project/issue under a slug DELIBERATELY distinct from the existing
/// workspace's (`globex-team`/`globex-core`), and record its real address. The
/// existing member will reach THIS address (foreign) and a never-existed one;
/// because the address does not resolve inside the member's acting workspace, the
/// SHIPPED scoping returns the SAME uniform-404 for both.
#[given(regex = r#"^an issue belongs to "([^"]+)"$"#)]
async fn an_issue_belongs_to(world: &mut FoundryWorld, ws_name: String) {
    let pool = harness_pool(world);
    let workspace_id = *world
        .mwt6_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("provisioned workspace {ws_name:?} must exist first"));

    let team_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'Globex Team', 'globex-team')",
    )
    .bind(team_id)
    .bind(workspace_id)
    .execute(&pool)
    .await
    .expect("insert provisioned-tenant team");
    let project_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, 'Globex Core', 'globex-core', 'GBX')",
    )
    .bind(project_id)
    .bind(team_id)
    .bind(workspace_id)
    .execute(&pool)
    .await
    .expect("insert provisioned-tenant project");
    // Resolve any member of the provisioned workspace as the issue author.
    let author_id: uuid::Uuid = sqlx::query_scalar(
        "SELECT user_id FROM workspace_memberships WHERE workspace_id = $1 LIMIT 1",
    )
    .bind(workspace_id)
    .fetch_one(&pool)
    .await
    .expect("provisioned workspace has at least its first admin as a member");
    let issue_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, author_id)
              VALUES ($1, $2, $3, 1, 'Globex-secret issue', $4)",
    )
    .bind(issue_id)
    .bind(project_id)
    .bind(workspace_id)
    .bind(author_id)
    .execute(&pool)
    .await
    .expect("insert provisioned-tenant issue");

    world.mwt6_foreign_issue_address =
        Some(("globex-team".to_string(), "globex-core".to_string(), 1));
}

/// Sign `email` in over the SHIPPED web sign-in path, then GET an issue-detail
/// URL, returning the (status, body) the shipped `show_issue` handler produced.
/// The existing member is resolved (ADR-005) to their own workspace; a foreign or
/// never-existed address both collapse to the uniform `resource_not_found_page`.
async fn member_web_get_issue(
    world: &mut FoundryWorld,
    email: &str,
    team_slug: &str,
    project_slug: &str,
    issue_number: i32,
) -> (StatusCode, String) {
    let http = http_client(world);
    let base = world
        .mwt6_harness
        .as_ref()
        .expect("mwt6 harness")
        .base_url();

    // (1) GET /sign-in for a CSRF cookie + token.
    let csrf_resp = http
        .get(format!("{base}/sign-in"))
        .send()
        .await
        .expect("get /sign-in for csrf");
    let csrf_token = csrf_resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .and_then(|s| s.strip_prefix("foundry_csrf="))
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string();

    // (2) POST /sign-in to authenticate; capture the session cookie.
    let mut form: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
    form.insert("email", email.to_string());
    form.insert("password", MEMBER_PASSWORD.to_string());
    form.insert("_csrf", csrf_token.clone());
    let signin = http
        .post(format!("{base}/sign-in"))
        .header(
            reqwest::header::COOKIE,
            format!("foundry_csrf={csrf_token}"),
        )
        .form(&form)
        .send()
        .await
        .expect("post /sign-in");
    let session_pair = signin
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_session="))
        .and_then(|s| s.split(';').next())
        .map(|s| s.to_string())
        .expect("sign-in must issue a foundry_session cookie");

    // (3) GET the issue-detail URL with the authenticated session.
    let url = format!("{base}/team/{team_slug}/project/{project_slug}/issues/{issue_number}");
    let resp = http
        .get(&url)
        .header(
            reqwest::header::COOKIE,
            format!("{session_pair}; foundry_csrf={csrf_token}"),
        )
        .send()
        .await
        .expect("authenticated web GET issue detail");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    (status, body)
}

/// `When "<member>" requests that "<ws>" issue by its real address` — the
/// existing member reaches the provisioned tenant's REAL issue address. The
/// SHIPPED handler scopes by the member's resolved (Acme) workspace, so the
/// foreign address resolves to `None` and renders the uniform 404.
#[when(regex = r#"^"([^"]+)" requests that "([^"]+)" issue by its real address$"#)]
async fn member_requests_foreign_issue(world: &mut FoundryWorld, member: String, _ws: String) {
    let (team_slug, project_slug, number) = world
        .mwt6_foreign_issue_address
        .clone()
        .expect("the provisioned-tenant issue address must have been seeded");
    let refusal = member_web_get_issue(world, &member, &team_slug, &project_slug, number).await;
    world.mwt6_first_refusal = Some(refusal);
}

/// `And "<member>" requests an issue that never existed` — the comparator: a
/// never-existed address (a team slug that exists in no workspace). The SHIPPED
/// handler renders the SAME uniform 404 page as the foreign reach above.
#[when(regex = r#"^"([^"]+)" requests an issue that never existed$"#)]
async fn member_requests_missing_issue(world: &mut FoundryWorld, member: String) {
    let refusal = member_web_get_issue(
        world,
        &member,
        "never-existed-team",
        "never-existed-project",
        999_999,
    )
    .await;
    world.mwt6_second_refusal = Some(refusal);
}

/// `Then the two responses are refused identically` — the foreign-resource reach
/// and the never-existed reach are observationally indistinguishable: the SAME
/// non-enumerable 404 status AND a byte-identical body (no 403-vs-404 oracle, no
/// shape difference). This is the SHIPPED uniform-404 idiom proven for the
/// freshly-provisioned tenant.
#[then(regex = r#"^the two responses are refused identically$"#)]
async fn two_responses_refused_identically(world: &mut FoundryWorld) {
    let (foreign_status, foreign_body) = world
        .mwt6_first_refusal
        .clone()
        .expect("the foreign-resource refusal was captured");
    let (missing_status, missing_body) = world
        .mwt6_second_refusal
        .clone()
        .expect("the never-existed refusal was captured");
    assert_eq!(
        foreign_status,
        StatusCode::NOT_FOUND,
        "the cross-tenant reach into the provisioned workspace must be a non-enumerable \
         404 (ADR-003), got {foreign_status}"
    );
    assert_eq!(
        foreign_status, missing_status,
        "the foreign-resource and never-existed reaches must share the SAME status \
         (a 403-vs-404 difference would be an existence oracle)"
    );
    assert_eq!(
        foreign_body, missing_body,
        "the foreign-resource and never-existed reaches must be byte-identical \
         (any body difference would reveal the provisioned issue exists)"
    );
}

/// `And nothing reveals that the "<ws>" issue exists` — the refusal body echoes
/// none of the provisioned tenant's identifiers (slug, project key, title, name).
/// A leaked identifier would be an enumeration oracle even if the status matched.
#[then(regex = r#"^nothing reveals that the "([^"]+)" issue exists$"#)]
async fn nothing_reveals_provisioned_issue(world: &mut FoundryWorld, ws_name: String) {
    let (_status, body) = world
        .mwt6_first_refusal
        .clone()
        .expect("the foreign-resource refusal was captured");
    for forbidden in [
        ws_name.as_str(),
        "globex-team",
        "globex-core",
        "GBX",
        "Globex-secret issue",
    ] {
        assert!(
            !body.contains(forbidden),
            "the cross-tenant refusal body echoed a provisioned-tenant identifier \
             {forbidden:?} (enumeration oracle): {body:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Scenario 6 — an unauthorized provisioning attempt does not reveal whether the
//              target exists (non-enumerable authz, NFR-MWT-SEC-02 applied to
//              provisioning).
//
// Green-by-inheritance from the exit-4 fail-closed path: the SHIPPED service
// `provision_workspace` checks `is_instance_admin(acting_user_id)` and returns
// `ServiceError::Forbidden` BEFORE any workspace-name lookup (services/lib.rs);
// the CLI maps `Forbidden` → exit 4 with a FIXED stderr message
// (`admin_cli.rs:525`), regardless of the target name. So a non-super-admin
// attempting an EXISTING name ("Acme", seeded in the Background) and a
// NEVER-existed name are refused with the SAME exit code AND the SAME output —
// the refusal is observationally independent of target existence.
//
// Falsifiability (demonstrated at RED, then restored): make the gate look up the
// workspace FIRST and diverge on existence — e.g. return exit 4 only when the
// name does not exist, or echo the name into the refusal — and the two attempts
// DIFFER → this scenario reds. The shipped gate denies before any lookup, so it
// greens.
// ---------------------------------------------------------------------------

/// Drive the REAL operator-CLI `provision-workspace` subprocess as a
/// non-super-admin and capture the FULL observable refusal surface
/// (exit code + stdout + stderr) — the refusal message goes to stderr, so the
/// non-enumerability comparison must include it. Acts as the member captured in
/// the World (`--as <member>`), exactly as the shipped CLI is invoked.
async fn run_provision_cli_capture_all(
    world: &mut FoundryWorld,
    ws_name: &str,
    admin_email: &str,
) -> (i32, String, String) {
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
        .expect("acting user recorded in the World");
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

    let exit = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    (exit, stdout, stderr)
}

/// `When "<member>" attempts to provision a workspace named like an existing one`
/// — the non-super-admin attempts to provision under the EXISTING workspace's
/// name ("Acme", seeded in the Background). Captures the full refusal surface.
#[when(regex = r#"^"([^"]+)" attempts to provision a workspace named like an existing one$"#)]
async fn member_attempts_provision_existing_name(world: &mut FoundryWorld, member: String) {
    // The Background seeds the existing workspace "Acme"; reuse its name as the
    // target so the gate would have an EXISTING workspace to find IF it looked.
    let existing_name = world
        .mwt6_workspace_ids
        .keys()
        .find(|n| n.as_str() == "Acme")
        .cloned()
        .unwrap_or_else(|| "Acme".to_string());
    world.mwt6_superadmin_email = Some(member);
    let refusal = run_provision_cli_capture_all(world, &existing_name, "intruder@acme.com").await;
    world.mwt6_authz_refusal_existing = Some(refusal);
}

/// `And "<member>" attempts to provision a workspace named like one that never
/// existed` — the comparator: the SAME non-super-admin attempts a name that
/// matches no workspace. Captures the full refusal surface for the identity
/// comparison.
#[when(
    regex = r#"^"([^"]+)" attempts to provision a workspace named like one that never existed$"#
)]
async fn member_attempts_provision_never_existed_name(world: &mut FoundryWorld, member: String) {
    world.mwt6_superadmin_email = Some(member);
    let refusal =
        run_provision_cli_capture_all(world, "NeverExistedWorkspace", "intruder@nowhere.test")
            .await;
    world.mwt6_authz_refusal_never_existed = Some(refusal);
}

/// `Then the two attempts are refused identically as not authorized` — both
/// attempts exit with the structured not-authorized code (4) AND produce
/// byte-identical output (stdout + stderr). A differing exit code or message
/// would be an existence oracle. The shipped gate denies before any workspace
/// lookup, so the refusal is independent of whether the target exists.
#[then(regex = r#"^the two attempts are refused identically as not authorized$"#)]
async fn two_attempts_refused_identically_not_authorized(world: &mut FoundryWorld) {
    let (existing_exit, existing_stdout, existing_stderr) = world
        .mwt6_authz_refusal_existing
        .clone()
        .expect("the existing-name unauthorized attempt was captured");
    let (never_exit, never_stdout, never_stderr) = world
        .mwt6_authz_refusal_never_existed
        .clone()
        .expect("the never-existed-name unauthorized attempt was captured");

    assert_eq!(
        existing_exit, 4,
        "an unauthorized attempt against an EXISTING name must be refused with the \
         structured not-authorized exit code (4); stdout={existing_stdout:?} \
         stderr={existing_stderr:?}"
    );
    assert_eq!(
        never_exit, 4,
        "an unauthorized attempt against a NEVER-existed name must be refused with the \
         structured not-authorized exit code (4); stdout={never_stdout:?} \
         stderr={never_stderr:?}"
    );
    assert_eq!(
        existing_exit, never_exit,
        "both unauthorized attempts must share the SAME exit code (a differing code \
         would be an existence oracle)"
    );
    assert_eq!(
        existing_stdout, never_stdout,
        "both unauthorized attempts must produce identical stdout (any difference \
         would reveal whether the target exists)"
    );
    assert_eq!(
        existing_stderr, never_stderr,
        "both unauthorized attempts must produce identical stderr (the refusal \
         message must not echo the target name or otherwise diverge on existence)"
    );
}

/// `And neither refusal reveals whether the target already exists` — neither
/// refusal's output echoes the target name, and both are observationally
/// indistinguishable. The authz gate denies BEFORE any workspace lookup, so the
/// refusal carries no information about target state.
#[then(regex = r#"^neither refusal reveals whether the target already exists$"#)]
async fn neither_refusal_reveals_existence(world: &mut FoundryWorld) {
    let (_existing_exit, existing_stdout, existing_stderr) = world
        .mwt6_authz_refusal_existing
        .clone()
        .expect("the existing-name unauthorized attempt was captured");
    let (_never_exit, never_stdout, never_stderr) = world
        .mwt6_authz_refusal_never_existed
        .clone()
        .expect("the never-existed-name unauthorized attempt was captured");

    // The existing target name must not be echoed into either refusal — an
    // echoed name would let the caller distinguish the two attempts and so leak
    // whether the target exists.
    for (channel, text) in [
        ("existing.stdout", existing_stdout.as_str()),
        ("existing.stderr", existing_stderr.as_str()),
        ("never.stdout", never_stdout.as_str()),
        ("never.stderr", never_stderr.as_str()),
    ] {
        assert!(
            !text.contains("Acme") && !text.contains("NeverExistedWorkspace"),
            "the refusal on {channel} echoed the target name (existence oracle): {text:?}"
        );
    }
    // And the two refusals are observationally identical — the strongest
    // statement of non-enumerability: nothing in the output distinguishes the
    // existing-target attempt from the never-existed one.
    assert_eq!(
        (existing_stdout.as_str(), existing_stderr.as_str()),
        (never_stdout.as_str(), never_stderr.as_str()),
        "the two refusals must be observationally indistinguishable"
    );
}
