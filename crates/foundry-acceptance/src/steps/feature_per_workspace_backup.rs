//! per-workspace-backup (US-PWB-01/02/03) step definitions: a self-hosting
//! operator EXPORTS exactly one workspace's data to a portable, verifiable tar
//! archive via the operator CLI, and proves — from the archive path alone — that
//! it is complete (all 10 tenant tables) and isolation-clean.
//!
//! This is the FIRST per-workspace-backup module and the WALKING SKELETON: it
//! stands up the feature's World glue + the thinnest end-to-end export vertical.
//! Step 01-01 implements ONLY the `@walking_skeleton` scenario ("An operator
//! exports one workspace to a verifiable archive reporting all ten tables").
//! Later steps extend this glue with list-workspaces, the id-selector, the
//! isolation crux + falsifiability, atomic-write, and the 0/2/3/4/5 exit-code
//! contract.
//!
//! Driving surface (feature header §"Driving adapter"): the operator CLI
//! `foundry doctor export-workspace <id|name-selector> <out-path>`, invoked as a
//! REAL subprocess (`assert_cmd::Command::cargo_bin("foundry")`) with
//! `DATABASE_URL` pinned to the per-scenario testcontainers schema (reusing the
//! `run_provision_workspace` scaffold). The archive is written to a real
//! filesystem path (`tempfile::TempDir`).
//!
//! LAYER 3 (real adapter + real subprocess, @real-io @wiring_e2e): real Postgres
//! via testcontainers + a per-scenario schema seeded with TWO real coexisting
//! workspaces ("Acme Corp" + "Globex LLC"), each holding its own rows; the real
//! `Store::export_workspace` scoped reader; the real tar archive on disk.
//! Example-based (Mandates 9 + 11) — no PBT machinery at this layer. Assertions
//! are traditional, over port-exposed observables: the CLI exit code + stdout
//! (per-table row counts, `status: OK`), and the archive file's well-formedness.

use crate::support::harness::{ensure_postgres, InProcHarness};
use crate::world::FoundryWorld;
use assert_cmd::Command as AssertCommand;
use cucumber::{given, then, when};
use secrecy::SecretString;
use sqlx::PgPool;

/// The two coexisting workspaces the Background seeds. The friendly selector
/// tokens ("globex"/"acme") in the scenarios map to these full names (DRIFT-1:
/// the CLI selector is an exact case-insensitive NAME or an id — no slug column —
/// so the step translates the friendly token to the seeded full name).
const ACME: &str = "Acme Corp";
const GLOBEX: &str = "Globex LLC";

/// Translate a friendly scenario token ("globex"/"acme") to the seeded full
/// workspace name the CLI selector resolves by (exact, case-insensitive).
fn token_to_workspace_name(token: &str) -> Option<&'static str> {
    match token.to_ascii_lowercase().as_str() {
        "globex" => Some(GLOBEX),
        "acme" => Some(ACME),
        _ => None,
    }
}

async fn ensure_harness(world: &mut FoundryWorld) -> &InProcHarness {
    if world.pwb_harness.is_none() {
        let harness = InProcHarness::spawn(time::OffsetDateTime::now_utc()).await;
        world.pwb_harness = Some(harness);
    }
    world.pwb_harness.as_ref().expect("pwb harness")
}

fn harness_pool(world: &FoundryWorld) -> PgPool {
    world
        .pwb_harness
        .as_ref()
        .expect("pwb harness")
        .app
        .state
        .store
        .pool()
        .clone()
}

// ---------------------------------------------------------------------------
// Background — two coexisting workspaces, each with its own data
// ---------------------------------------------------------------------------

/// `Given an instance with workspaces "Acme Corp" and "Globex LLC"` — create both
/// workspaces (just the `workspaces` rows; members/teams/etc. are layered by the
/// subsequent Background steps). Uses the SHIPPED `create_initial_workspace` for
/// the first and a direct insert for the second so the two coexist as real
/// tenants the export must isolate between.
#[given(regex = r#"^an instance with workspaces "([^"]+)" and "([^"]+)"$"#)]
async fn instance_with_two_workspaces(world: &mut FoundryWorld, first: String, second: String) {
    ensure_harness(world).await;
    let pool = harness_pool(world);
    for name in [first, second] {
        let id = uuid::Uuid::now_v7();
        sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
            .bind(id)
            .bind(&name)
            .execute(&pool)
            .await
            .unwrap_or_else(|e| panic!("insert workspace {name:?}: {e}"));
        world.pwb_workspace_ids.insert(name, id);
    }
}

/// `And "<workspace>" has its own members, teams, projects, issues, and comments`
/// — seed one full tenant data set scoped to the named workspace: a member user
/// + membership, a team + team membership, a project, an issue, a comment, and a
/// machine token. This populates the tenant tables the export walks so the
/// per-table report has non-trivial counts and the isolation crux has real
/// sibling rows to NOT leak.
#[given(regex = r#"^"([^"]+)" has its own members, teams, projects, issues, and comments$"#)]
async fn workspace_has_full_data(world: &mut FoundryWorld, ws_name: String) {
    let pool = harness_pool(world);
    let workspace_id = *world
        .pwb_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} must be seeded first"));
    seed_tenant_data(&pool, workspace_id, &ws_name).await;
}

/// Seed exactly one row in each scoped tenant table for `workspace_id`, so the
/// export's per-table report is non-empty and the two workspaces hold distinct,
/// real rows. Reused by the gold discipline at the acceptance layer (every tenant
/// table is populated, so a silently-omitted table would show a missing count).
async fn seed_tenant_data(pool: &PgPool, workspace_id: uuid::Uuid, ws_name: &str) {
    // A member user (global identity) + membership edge.
    let member_id = uuid::Uuid::now_v7();
    let member_email = format!("member-{}@example.com", workspace_id.simple());
    let pw = foundry_auth::hash_password(&SecretString::new("member-password".to_string().into()))
        .await
        .expect("hash member pw");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(member_id)
    .bind(&member_email)
    .bind(&member_email)
    .bind(format!("Member of {ws_name}"))
    .bind(&pw)
    .execute(pool)
    .await
    .expect("insert member user");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'admin')",
    )
    .bind(workspace_id)
    .bind(member_id)
    .execute(pool)
    .await
    .expect("insert membership");

    // Team + team membership.
    let team_id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'Core', 'core')")
        .bind(team_id)
        .bind(workspace_id)
        .execute(pool)
        .await
        .expect("insert team");
    sqlx::query("INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'lead')")
        .bind(team_id)
        .bind(member_id)
        .execute(pool)
        .await
        .expect("insert team membership");

    // Project + issue.
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
    .expect("insert project");
    let issue_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, author_id)
              VALUES ($1, $2, $3, 1, 'An issue', $4)",
    )
    .bind(issue_id)
    .bind(project_id)
    .bind(workspace_id)
    .bind(member_id)
    .execute(pool)
    .await
    .expect("insert issue");

    // Comment (carries a denormalized workspace_id + issue_id).
    let comment_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO comments (id, workspace_id, issue_id, author_id, body_markdown, body_html)
              VALUES ($1, $2, $3, $4, 'A comment', '<p>A comment</p>')",
    )
    .bind(comment_id)
    .bind(workspace_id)
    .bind(issue_id)
    .bind(member_id)
    .execute(pool)
    .await
    .expect("insert comment");

    // Invite.
    let invite_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO invites (id, workspace_id, invitee_email, created_by, expires_at)
              VALUES ($1, $2, 'invitee@example.com', $3, now() + interval '7 days')",
    )
    .bind(invite_id)
    .bind(workspace_id)
    .bind(member_id)
    .execute(pool)
    .await
    .expect("insert invite");

    // Machine token (PK is `jti`; `label` not `name`; NOT NULL `expires_at`).
    let token_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO machine_tokens (jti, user_id, workspace_id, expires_at, label, created_by)
              VALUES ($1, $2, $3, now() + interval '30 days', 'ci-token', $2)",
    )
    .bind(token_id)
    .bind(member_id)
    .bind(workspace_id)
    .execute(pool)
    .await
    .expect("insert machine token");
}

// ---------------------------------------------------------------------------
// Scenario 1 (walking skeleton)
// ---------------------------------------------------------------------------

/// `When Devansh exports "<selector>" to a backup path` — drive the REAL operator
/// CLI `export-workspace` subprocess against the per-scenario schema, writing the
/// archive into a per-scenario TempDir. Stash the exit code + stdout + path for
/// the Then steps. The selector ("globex") resolves by case-insensitive name.
#[when(regex = r#"^Devansh exports "([^"]+)" to a backup path$"#)]
async fn devansh_exports_to_backup_path(world: &mut FoundryWorld, selector: String) {
    ensure_harness(world).await;
    let base = ensure_postgres().await;
    let schema = world
        .pwb_harness
        .as_ref()
        .expect("pwb harness")
        .schema
        .clone();
    let database_url = format!("{base}?options=-csearch_path%3D{schema}");

    let tempdir = tempfile::TempDir::new().expect("create export tempdir");
    let out_path = tempdir.path().join("export.dump");
    world.pwb_tempdir = Some(tempdir);
    world.pwb_out_path = Some(out_path.clone());

    // Translate the friendly scenario token ("globex") to the seeded full name
    // the CLI resolves by exact, case-insensitive name (DRIFT-1). An unknown
    // token is passed through verbatim so the unknown-workspace path (exit 2) is
    // still exercised by later scenarios.
    let cli_selector =
        token_to_workspace_name(&selector).map_or_else(|| selector.clone(), str::to_string);

    let sel = cli_selector;
    let out = out_path.clone();
    let output = tokio::task::spawn_blocking(move || {
        AssertCommand::cargo_bin("foundry")
            .expect("cargo-bin foundry")
            .env("DATABASE_URL", database_url)
            .args(["doctor", "export-workspace"])
            .arg(&sel)
            .arg(&out)
            .output()
            .expect("invoke foundry doctor export-workspace")
    })
    .await
    .expect("join blocking cli");

    world.pwb_cli_exit = Some(output.status.code().unwrap_or(-1));
    world.pwb_cli_stdout = Some(String::from_utf8_lossy(&output.stdout).into_owned());
}

/// `Then an archive file exists at that path` — the export wrote a real,
/// well-formed tar archive (manifest.json + a `tables/<table>.jsonl` entry for
/// every one of the ten tenant tables) at the final path. Reading the tar offline
/// proves it is a genuine archive, not merely a touched file.
#[then(regex = r#"^an archive file exists at that path$"#)]
async fn archive_file_exists(world: &mut FoundryWorld) {
    let path = world
        .pwb_out_path
        .clone()
        .expect("export path captured in the When step");
    assert!(
        path.exists(),
        "an archive file must exist at {path:?}; CLI exit={:?}, stdout={:?}",
        world.pwb_cli_exit,
        world.pwb_cli_stdout,
    );

    let entries = read_tar_entry_names(&path);
    assert!(
        entries.iter().any(|n| n == "manifest.json"),
        "the archive must contain manifest.json; entries={entries:?}"
    );
    for table in foundry_store::TENANT_TABLES {
        let expected = format!("tables/{table}.jsonl");
        assert!(
            entries.contains(&expected),
            "the archive must contain {expected:?} for tenant table {table:?}; entries={entries:?}"
        );
    }
}

/// `And the output reports a row count for all 10 tenant tables` — the CLI's
/// port-exposed stdout carries a `<table>: <count>` line for every one of the ten
/// tenant tables (the completeness report).
#[then(regex = r#"^the output reports a row count for all 10 tenant tables$"#)]
async fn output_reports_all_ten_tables(world: &mut FoundryWorld) {
    let stdout = world
        .pwb_cli_stdout
        .as_deref()
        .expect("export CLI stdout captured");
    for table in foundry_store::TENANT_TABLES {
        assert!(
            stdout.lines().any(|line| {
                let line = line.trim();
                line.starts_with(&format!("{table}:"))
                    && line
                        .rsplit(':')
                        .next()
                        .map(|n| n.trim().parse::<u64>().is_ok())
                        .unwrap_or(false)
            }),
            "stdout must report a numeric row count for tenant table {table:?}; got {stdout:?}"
        );
    }
}

/// `And the output ends with "status: OK"`.
#[then(regex = r#"^the output ends with "status: OK"$"#)]
async fn output_ends_with_status_ok(world: &mut FoundryWorld) {
    let stdout = world
        .pwb_cli_stdout
        .as_deref()
        .expect("export CLI stdout captured");
    let last = stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .next_back()
        .unwrap_or("");
    assert_eq!(
        last.trim(),
        "status: OK",
        "the export output must end with `status: OK`; got {stdout:?}"
    );
}

/// `And the command exits with code 0`.
#[then(regex = r#"^the command exits with code 0$"#)]
async fn command_exits_zero(world: &mut FoundryWorld) {
    assert_eq!(
        world.pwb_cli_exit,
        Some(0),
        "export-workspace must exit 0; stdout={:?}",
        world.pwb_cli_stdout
    );
}

// ---------------------------------------------------------------------------
// Scenario 2 (step 01-02) — list-workspaces shows each workspace's identity
// ---------------------------------------------------------------------------

/// `When Devansh runs "foundry doctor list-workspaces"` — drive the REAL operator
/// CLI `list-workspaces` subprocess against the per-scenario schema and stash the
/// exit code + stdout for the Then steps. This is the operator's discovery surface
/// (DRIFT-1: prints id + name; `workspaces` has no slug column) so the operator can
/// pick a target before exporting.
#[when(regex = r#"^Devansh runs "foundry doctor list-workspaces"$"#)]
async fn devansh_runs_list_workspaces(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    let base = ensure_postgres().await;
    let schema = world
        .pwb_harness
        .as_ref()
        .expect("pwb harness")
        .schema
        .clone();
    let database_url = format!("{base}?options=-csearch_path%3D{schema}");

    let output = tokio::task::spawn_blocking(move || {
        AssertCommand::cargo_bin("foundry")
            .expect("cargo-bin foundry")
            .env("DATABASE_URL", database_url)
            .args(["doctor", "list-workspaces"])
            .output()
            .expect("invoke foundry doctor list-workspaces")
    })
    .await
    .expect("join blocking cli");

    world.pwb_cli_exit = Some(output.status.code().unwrap_or(-1));
    world.pwb_cli_stdout = Some(String::from_utf8_lossy(&output.stdout).into_owned());
}

/// `Then the output lists each workspace's id and name` — the CLI's port-exposed
/// stdout carries, for EVERY seeded workspace, a row pairing its real id (UUID) and
/// its name. Falsifiability: a list that omits a seeded workspace's id REDs this
/// assertion.
#[then(regex = r#"^the output lists each workspace's id and name$"#)]
async fn output_lists_each_workspace_identity(world: &mut FoundryWorld) {
    let stdout = world
        .pwb_cli_stdout
        .as_deref()
        .expect("list-workspaces CLI stdout captured");
    for (name, id) in &world.pwb_workspace_ids {
        let id_str = id.to_string();
        assert!(
            stdout.contains(&id_str),
            "list-workspaces stdout must contain the id {id_str:?} of workspace {name:?}; got {stdout:?}"
        );
        assert!(
            stdout.contains(name.as_str()),
            "list-workspaces stdout must contain the name {name:?}; got {stdout:?}"
        );
    }
}

/// `And both "<first>" and "<second>" appear` — both named workspaces are listed,
/// so the operator sees the full instance roster, not a truncated view.
#[then(regex = r#"^both "([^"]+)" and "([^"]+)" appear$"#)]
async fn both_workspaces_appear(world: &mut FoundryWorld, first: String, second: String) {
    let stdout = world
        .pwb_cli_stdout
        .as_deref()
        .expect("list-workspaces CLI stdout captured");
    for name in [first, second] {
        assert!(
            stdout.contains(&name),
            "list-workspaces stdout must list workspace {name:?}; got {stdout:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Scenario 3 (step 01-02) — export a workspace selected by its id
// ---------------------------------------------------------------------------

/// `When Devansh exports the workspace whose id is <ws>'s to a backup path` — drive
/// the REAL operator CLI `export-workspace` subprocess with the SELECTED workspace's
/// real UUID (not its name) as the selector. Proves the id branch of the id-or-name
/// resolver (DRIFT-1) feeds the archive header. Stash exit/stdout/path + the
/// expected name for the Then steps.
#[when(regex = r#"^Devansh exports the workspace whose id is (.+)'s to a backup path$"#)]
async fn devansh_exports_by_id(world: &mut FoundryWorld, ws_name: String) {
    ensure_harness(world).await;
    let base = ensure_postgres().await;
    let schema = world
        .pwb_harness
        .as_ref()
        .expect("pwb harness")
        .schema
        .clone();
    let database_url = format!("{base}?options=-csearch_path%3D{schema}");

    let workspace_id = *world
        .pwb_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} must be seeded first"));
    world.pwb_expected_name = Some(ws_name);

    let tempdir = tempfile::TempDir::new().expect("create export tempdir");
    let out_path = tempdir.path().join("export-by-id.dump");
    world.pwb_tempdir = Some(tempdir);
    world.pwb_out_path = Some(out_path.clone());

    let selector = workspace_id.to_string();
    let out = out_path.clone();
    let output = tokio::task::spawn_blocking(move || {
        AssertCommand::cargo_bin("foundry")
            .expect("cargo-bin foundry")
            .env("DATABASE_URL", database_url)
            .args(["doctor", "export-workspace"])
            .arg(&selector)
            .arg(&out)
            .output()
            .expect("invoke foundry doctor export-workspace")
    })
    .await
    .expect("join blocking cli");

    world.pwb_cli_exit = Some(output.status.code().unwrap_or(-1));
    world.pwb_cli_stdout = Some(String::from_utf8_lossy(&output.stdout).into_owned());
}

/// `Then the selector resolves to "<name>"` — the id selector resolved to the right
/// workspace: the CLI's port-exposed `workspace-name:` line names the expected
/// workspace. Falsifiability: resolving the id to the WRONG workspace's name REDs.
#[then(regex = r#"^the selector resolves to "([^"]+)"$"#)]
async fn selector_resolves_to(world: &mut FoundryWorld, expected: String) {
    let stdout = world
        .pwb_cli_stdout
        .as_deref()
        .expect("export CLI stdout captured");
    let resolved = stdout
        .lines()
        .find_map(|l| l.trim().strip_prefix("workspace-name:"))
        .map(str::trim);
    assert_eq!(
        resolved,
        Some(expected.as_str()),
        "the id selector must resolve to workspace {expected:?}; stdout={stdout:?}, exit={:?}",
        world.pwb_cli_exit,
    );
}

/// `And an archive of "<name>" exists at that path` — a real, well-formed tar
/// archive whose manifest declares the expected workspace was written at the final
/// path. Reading the tar offline proves it is a genuine archive of the SELECTED
/// workspace, not a touched file or the wrong tenant.
#[then(regex = r#"^an archive of "([^"]+)" exists at that path$"#)]
async fn archive_of_workspace_exists(world: &mut FoundryWorld, expected: String) {
    let path = world
        .pwb_out_path
        .clone()
        .expect("export path captured in the When step");
    assert!(
        path.exists(),
        "an archive must exist at {path:?}; CLI exit={:?}, stdout={:?}",
        world.pwb_cli_exit,
        world.pwb_cli_stdout,
    );

    let manifest = read_tar_manifest(&path);
    let declared = manifest
        .get("declared_workspace_name")
        .and_then(|v| v.as_str());
    assert_eq!(
        declared,
        Some(expected.as_str()),
        "the archive manifest must declare workspace {expected:?}; manifest={manifest:?}"
    );
}

/// Read and parse the `manifest.json` entry from a tar archive at `path`, offline.
fn read_tar_manifest(path: &std::path::Path) -> serde_json::Value {
    use std::io::Read;
    let file = std::fs::File::open(path).expect("open export archive");
    let mut archive = tar::Archive::new(file);
    for entry in archive.entries().expect("read tar entries") {
        let mut entry = entry.expect("tar entry");
        let name = entry
            .path()
            .expect("entry path")
            .to_string_lossy()
            .into_owned();
        if name == "manifest.json" {
            let mut buf = String::new();
            entry.read_to_string(&mut buf).expect("read manifest.json");
            return serde_json::from_str(&buf).expect("parse manifest.json");
        }
    }
    panic!("archive at {path:?} has no manifest.json entry");
}

/// Read the entry names of a tar archive at `path`, offline. Used to verify the
/// exported archive is well-formed (the ten table JSONL files + manifest).
fn read_tar_entry_names(path: &std::path::Path) -> Vec<String> {
    let file = std::fs::File::open(path).expect("open export archive");
    let mut archive = tar::Archive::new(file);
    archive
        .entries()
        .expect("read tar entries")
        .map(|e| {
            e.expect("tar entry")
                .path()
                .expect("entry path")
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}
