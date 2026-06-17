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
    // Scenario 15 (DB unreachable): point the CLI at a deliberately bad DATABASE_URL
    // (a closed port on localhost) so the real `Store::connect` fails and the export
    // maps the connect error to exit 3 — a real connect failure, not a mock.
    let database_url = if world.pwb_db_unreachable {
        "postgres://foundry:foundry@127.0.0.1:1/foundry".to_string()
    } else {
        format!("{base}?options=-csearch_path%3D{schema}")
    };

    // Snapshot every tenant table BEFORE the export so the read-only proof
    // (scenario "Exporting a workspace removes nothing") can assert the source
    // instance is byte-for-byte unchanged afterwards. Harmless for the other
    // scenarios that reuse this When step — they simply ignore the baseline.
    let pool = harness_pool(world);
    world.pwb_snapshot_before_export = snapshot_tenant_tables(&pool).await;

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
    // Failure messages (exit 2 unknown-selector guidance, exit 3 DB-unreachable)
    // go to stderr, so capture it for the failure-path Then steps (scenarios 11, 15).
    world.pwb_cli_stderr = Some(String::from_utf8_lossy(&output.stderr).into_owned());
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

/// `And the command exits with code <n>` — assert the CLI's port-exposed exit code
/// is exactly the expected one. Covers the 0 happy path AND the failure-path codes
/// (2 unknown selector, 3 DB unreachable) the exit-code contract mirrors from
/// `admin_cli.rs`. Falsifiability: any other exit code REDs, surfacing stdout +
/// stderr so the wrong code's cause is visible.
#[then(regex = r#"^the command exits with code (\d+)$"#)]
async fn command_exits_with_code(world: &mut FoundryWorld, expected: i32) {
    assert_eq!(
        world.pwb_cli_exit,
        Some(expected),
        "the command must exit {expected}; stdout={:?}, stderr={:?}",
        world.pwb_cli_stdout,
        world.pwb_cli_stderr,
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

// ---------------------------------------------------------------------------
// Scenario 4 (step 01-03) — the export is read-only: it removes nothing
// ---------------------------------------------------------------------------

/// `Then "<workspace>" and all its data still exist on the instance unchanged` —
/// the read-only proof. The When step snapshotted every tenant table before the
/// export; here we re-snapshot the live instance and assert it is byte-for-byte
/// identical to that baseline (so NO row was deleted, inserted, or updated by the
/// export), AND that the named workspace's own row is still present. Falsifiability:
/// an export that deleted or mutated any tenant row makes the before/after equality
/// RED; a vanished workspace row makes the presence check RED.
#[then(regex = r#"^"([^"]+)" and all its data still exist on the instance unchanged$"#)]
async fn workspace_data_unchanged(world: &mut FoundryWorld, ws_name: String) {
    let pool = harness_pool(world);
    let after = snapshot_tenant_tables(&pool).await;
    let before = world.pwb_snapshot_before_export.clone();
    assert!(
        !before.is_empty(),
        "the before-export snapshot must have been captured in the When step"
    );

    // The named workspace's own row must still be present after the export.
    let workspace_id = *world
        .pwb_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} must be seeded first"));
    let still_present =
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM workspaces WHERE id = $1")
            .bind(workspace_id)
            .fetch_one(&pool)
            .await
            .expect("count workspace row after export");
    assert_eq!(
        still_present, 1,
        "workspace {ws_name:?} ({workspace_id}) must still exist on the instance after a read-only export"
    );

    // Every tenant table must be byte-for-byte identical to the pre-export baseline:
    // the export removed, added, and changed NOTHING.
    for table in foundry_store::TENANT_TABLES {
        assert_eq!(
            after.get(table),
            before.get(table),
            "tenant table {table:?} must be byte-for-byte unchanged by a read-only export"
        );
    }
}

// ---------------------------------------------------------------------------
// Scenario 5 (step 02-01) — the archive contains every target row and no sibling
// row. Offline inspection of the written archive proves the isolation crux
// directly from the bytes (not via the CLI): every archived row resolves to the
// target workspace, no row resolves to the sibling, and the member set is exactly
// the target's members.
// ---------------------------------------------------------------------------

/// `Then every row in the archive belongs to "<workspace>"` — read the archive
/// offline and re-apply the §5 scope predicate to every row across the ten tenant
/// tables: each row resolves to the target workspace's id. Falsifiability: a row
/// resolving to any other workspace REDs.
#[then(regex = r#"^every row in the archive belongs to "([^"]+)"$"#)]
async fn every_row_belongs_to(world: &mut FoundryWorld, ws_name: String) {
    let archive = read_archive_for_isolation(world);
    let target_id = *world
        .pwb_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} must be seeded"));
    let report = foundry_store::verify_workspace_export(&archive);
    assert_eq!(
        archive.declared_workspace_id, target_id,
        "the archive must declare the target workspace {ws_name:?} ({target_id})"
    );
    assert!(
        report.is_isolation_clean(),
        "every archived row must belong to {ws_name:?}; isolation violations={:?}",
        report.isolation_violations
    );
}

/// `And no row in the archive belongs to "<sibling>"` — no archived row resolves
/// to the sibling workspace's id. Falsifiability: a planted sibling row REDs. This
/// is checked directly against the sibling's real id, complementing the
/// declared-workspace isolation pass.
#[then(regex = r#"^no row in the archive belongs to "([^"]+)"$"#)]
async fn no_row_belongs_to_sibling(world: &mut FoundryWorld, sibling: String) {
    let archive = read_archive_for_isolation(world);
    let sibling_id = *world
        .pwb_workspace_ids
        .get(&sibling)
        .unwrap_or_else(|| panic!("sibling workspace {sibling:?} must be seeded"));
    let sibling_str = sibling_id.to_string();
    for table in &archive.tables {
        for row in &table.rows {
            let mentions_sibling = row
                .get("workspace_id")
                .and_then(serde_json::Value::as_str)
                .map(|w| w == sibling_str)
                .unwrap_or(false);
            assert!(
                !mentions_sibling,
                "no archived row may belong to the sibling {sibling:?} ({sibling_id}); \
                 found one in table {:?}: {row}",
                table.name
            );
        }
    }
}

/// `And the archive's member set is exactly the members of "<workspace>"` — the
/// archived `users` set equals exactly the user ids that are members of the target
/// workspace (per the seeded `workspace_memberships`). Falsifiability: a missing
/// target member or an extra non-member user REDs.
#[then(regex = r#"^the archive's member set is exactly the members of "([^"]+)"$"#)]
async fn member_set_is_exactly(world: &mut FoundryWorld, ws_name: String) {
    let archive = read_archive_for_isolation(world);
    let pool = harness_pool(world);
    let target_id = *world
        .pwb_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} must be seeded"));

    let expected: std::collections::BTreeSet<String> = sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT user_id FROM workspace_memberships WHERE workspace_id = $1",
    )
    .bind(target_id)
    .fetch_all(&pool)
    .await
    .expect("read expected members")
    .into_iter()
    .map(|u| u.to_string())
    .collect();

    let archived: std::collections::BTreeSet<String> = archive
        .tables
        .iter()
        .find(|t| t.name == "users")
        .expect("archive has users table")
        .rows
        .iter()
        .filter_map(|r| {
            r.get("id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .collect();

    assert_eq!(
        archived, expected,
        "the archive's member (users) set must be exactly the members of {ws_name:?}"
    );
}

/// Read the archive written by the most recent export into a
/// `foundry_store::ArchiveContents` (the offline verifier's input): parse
/// `manifest.json` for the declared id + per-table counts, parse each
/// `tables/<table>.jsonl` into whole-row JSON. Mirrors the CLI reader so the step
/// asserts against the SAME parsed shape the production verifier consumes.
fn read_archive_for_isolation(world: &FoundryWorld) -> foundry_store::ArchiveContents {
    use std::io::Read;
    let path = world
        .pwb_out_path
        .clone()
        .expect("export path captured in the When step");
    let manifest = read_tar_manifest(&path);
    let declared_workspace_id = manifest
        .get("declared_workspace_id")
        .and_then(serde_json::Value::as_str)
        .and_then(|s| uuid::Uuid::parse_str(s).ok())
        .expect("manifest declared_workspace_id");
    let declared_counts = manifest
        .get("row_counts")
        .and_then(serde_json::Value::as_object);

    let file = std::fs::File::open(&path).expect("open archive");
    let mut tar_archive = tar::Archive::new(file);
    let mut jsonl_by_table: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    for entry in tar_archive.entries().expect("tar entries") {
        let mut entry = entry.expect("tar entry");
        let name = entry
            .path()
            .expect("entry path")
            .to_string_lossy()
            .into_owned();
        if let Some(table) = name
            .strip_prefix("tables/")
            .and_then(|n| n.strip_suffix(".jsonl"))
            .map(str::to_string)
        {
            let mut buf = String::new();
            entry.read_to_string(&mut buf).expect("read jsonl");
            jsonl_by_table.insert(table, buf);
        }
    }

    let tables = foundry_store::TENANT_TABLES
        .iter()
        .map(|table| {
            let jsonl = jsonl_by_table.get(*table).cloned().unwrap_or_default();
            let rows: Vec<serde_json::Value> = jsonl
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| serde_json::from_str(l).expect("parse jsonl row"))
                .collect();
            let declared_count = declared_counts
                .and_then(|m| m.get(*table))
                .and_then(serde_json::Value::as_u64)
                .map_or(rows.len(), |c| c as usize);
            foundry_store::ArchivedTable {
                name: (*table).to_string(),
                declared_count,
                rows,
            }
        })
        .collect();

    foundry_store::ArchiveContents {
        declared_workspace_id,
        tables,
    }
}

// ---------------------------------------------------------------------------
// Scenario 6 (step 02-01) — verify-export confirms completeness + isolation from
// the path alone and exits 0 on a clean archive.
// ---------------------------------------------------------------------------

/// `Given Devansh has exported "<selector>" to a backup path` — same as the export
/// When step, used as a precondition for the verify scenarios. Drives the REAL
/// export CLI so a genuine archive exists on disk for verify-export to read.
#[given(regex = r#"^Devansh has exported "([^"]+)" to a backup path$"#)]
async fn devansh_has_exported(world: &mut FoundryWorld, selector: String) {
    devansh_exports_to_backup_path(world, selector).await;
}

/// `When Devansh runs "foundry doctor verify-export" on that archive` — drive the
/// REAL operator CLI `verify-export` subprocess on the exported archive path. It is
/// PATH-ONLY (NFR-PWB-INT-01): no workspace argument is passed; the declared
/// workspace is read from the archive's manifest header. Stash exit + stdout.
#[when(regex = r#"^Devansh runs "foundry doctor verify-export" on that archive$"#)]
async fn devansh_runs_verify_export(world: &mut FoundryWorld) {
    let path = world
        .pwb_out_path
        .clone()
        .expect("an archive must have been exported first");
    let path_arg = path.clone();
    let output = tokio::task::spawn_blocking(move || {
        AssertCommand::cargo_bin("foundry")
            .expect("cargo-bin foundry")
            .args(["doctor", "verify-export"])
            .arg(&path_arg)
            .output()
            .expect("invoke foundry doctor verify-export")
    })
    .await
    .expect("join blocking cli");

    world.pwb_cli_exit = Some(output.status.code().unwrap_or(-1));
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    // Surface stderr in the captured diagnostics: verify-export writes failure
    // messages (exit 4 completeness / exit 6 isolation) to stderr, so a bare
    // empty-stdout assertion would hide WHY it failed. Stash stderr too.
    world.pwb_cli_stderr = Some(String::from_utf8_lossy(&output.stderr).into_owned());
    world.pwb_cli_stdout = Some(stdout);
}

/// `Then the report confirms all 10 tenant tables are present` — verify-export's
/// port-exposed stdout reports the completeness confirmation. Falsifiability: a
/// missing table makes the CLI exit 4 with no completeness-OK line.
#[then(regex = r#"^the report confirms all 10 tenant tables are present$"#)]
async fn report_confirms_completeness(world: &mut FoundryWorld) {
    let stdout = world
        .pwb_cli_stdout
        .as_deref()
        .expect("verify-export stdout captured");
    let lower = stdout.to_ascii_lowercase();
    let stderr = world.pwb_cli_stderr.as_deref().unwrap_or("");
    assert!(
        lower.contains("completeness: ok") && lower.contains("tenant tables are present"),
        "verify-export must confirm all 10 tenant tables are present; \
         exit={:?}, stdout={stdout:?}, stderr={stderr:?}",
        world.pwb_cli_exit,
    );
}

/// `And the report confirms every row belongs to the declared workspace`.
#[then(regex = r#"^the report confirms every row belongs to the declared workspace$"#)]
async fn report_confirms_rows_belong(world: &mut FoundryWorld) {
    let stdout = world
        .pwb_cli_stdout
        .as_deref()
        .expect("verify-export stdout captured");
    let lower = stdout.to_ascii_lowercase();
    assert!(
        lower.contains("every row belongs to the declared workspace"),
        "verify-export must confirm every row belongs to the declared workspace; got {stdout:?}"
    );
}

/// `And the report confirms no row references a sibling workspace`.
#[then(regex = r#"^the report confirms no row references a sibling workspace$"#)]
async fn report_confirms_no_sibling(world: &mut FoundryWorld) {
    let stdout = world
        .pwb_cli_stdout
        .as_deref()
        .expect("verify-export stdout captured");
    let lower = stdout.to_ascii_lowercase();
    assert!(
        lower.contains("no row references a sibling workspace"),
        "verify-export must confirm no row references a sibling workspace; got {stdout:?}"
    );
}

// ---------------------------------------------------------------------------
// Scenario 7 (step 02-02) — transitively-scoped rows are isolation-checked
// through the FK chain. team_memberships reaches the workspace ONLY via team_id;
// comments are cross-checked via comment.issue_id -> issues.workspace_id (DRIFT-2).
// verify-export's port-exposed stdout confirms each chain check ran, and that
// every transitively-scoped row belongs to the declared workspace.
// ---------------------------------------------------------------------------

/// `Then each team membership is resolved to its owning workspace through its
/// team` — verify-export reports that the team_memberships rows (which carry no
/// direct workspace_id) were resolved THROUGH their team_id to the declared
/// workspace. Falsifiability: a team_membership whose team_id does not resolve to
/// an archived team makes verify-export exit 6 with no such confirmation line.
#[then(regex = r#"^each team membership is resolved to its owning workspace through its team$"#)]
async fn each_team_membership_resolved_through_team(world: &mut FoundryWorld) {
    let stdout = world
        .pwb_cli_stdout
        .as_deref()
        .expect("verify-export stdout captured");
    let lower = stdout.to_ascii_lowercase();
    let stderr = world.pwb_cli_stderr.as_deref().unwrap_or("");
    assert!(
        lower.contains("team membership") && lower.contains("through their team"),
        "verify-export must confirm each team membership was resolved to its owning \
         workspace through its team; exit={:?}, stdout={stdout:?}, stderr={stderr:?}",
        world.pwb_cli_exit,
    );
}

/// `And each comment is cross-checked against its issue's owning workspace` —
/// verify-export reports the DRIFT-2 cross-check ran: each comment's issue_id was
/// resolved to an archived issue (whose workspace_id is the declared workspace),
/// so a comment whose denormalized workspace_id disagreed with its issue's would
/// be caught. Falsifiability: a comment whose issue_id dangles reds verify (exit 6).
#[then(regex = r#"^each comment is cross-checked against its issue's owning workspace$"#)]
async fn each_comment_cross_checked(world: &mut FoundryWorld) {
    let stdout = world
        .pwb_cli_stdout
        .as_deref()
        .expect("verify-export stdout captured");
    let lower = stdout.to_ascii_lowercase();
    assert!(
        lower.contains("comment") && lower.contains("cross-checked"),
        "verify-export must confirm each comment was cross-checked against its issue's \
         owning workspace; got {stdout:?}"
    );
}

/// `And every transitively-scoped row is confirmed to belong to "<workspace>"` —
/// the FK-chain check passed for every transitively-scoped row and verify-export
/// exited 0, declaring the target workspace. Falsifiability: any unresolved
/// transitive reference reds verify (non-zero) so this OK confirmation is absent.
#[then(regex = r#"^every transitively-scoped row is confirmed to belong to "([^"]+)"$"#)]
async fn every_transitive_row_belongs_to(world: &mut FoundryWorld, ws_name: String) {
    let stdout = world
        .pwb_cli_stdout
        .as_deref()
        .expect("verify-export stdout captured");
    let lower = stdout.to_ascii_lowercase();
    assert!(
        lower.contains("every transitively-scoped row belongs to the declared workspace"),
        "verify-export must confirm every transitively-scoped row belongs to the declared \
         workspace; got {stdout:?}"
    );
    // The archive the verify ran on must declare the named target workspace, so the
    // "belongs to the declared workspace" confirmation is genuinely about <ws_name>.
    let target_id = *world
        .pwb_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} must be seeded"));
    assert!(
        stdout.contains(&target_id.to_string()),
        "verify-export must declare the target workspace {ws_name:?} ({target_id}); got {stdout:?}"
    );
    assert_eq!(
        world.pwb_cli_exit,
        Some(0),
        "verify-export must exit 0 on an isolation-clean archive; stdout={stdout:?}"
    );
}

// ---------------------------------------------------------------------------
// Scenario 8 (step 02-02) — a user who belongs to BOTH workspaces is legitimately
// included in the target archive and is NOT flagged as a sibling leak (OD-PWB-1 /
// ADR-001 membership-bounded isolation). The verifier sees only that the user is a
// member of the declared workspace; it has no sibling-membership column to trip on.
// ---------------------------------------------------------------------------

/// `Given a user is a member of both "<a>" and "<b>"` — promote one of the seeded
/// member users so it is a member of BOTH named workspaces: add a
/// workspace_memberships edge into the SECOND workspace for a user that already
/// belongs to the FIRST. This is the OD-PWB-1 dual-membership fixture — the shared
/// user must appear in either workspace's export as a legitimate member.
#[given(regex = r#"^a user is a member of both "([^"]+)" and "([^"]+)"$"#)]
async fn user_is_member_of_both(world: &mut FoundryWorld, first: String, second: String) {
    let pool = harness_pool(world);
    let first_id = *world
        .pwb_workspace_ids
        .get(&first)
        .unwrap_or_else(|| panic!("workspace {first:?} must be seeded"));
    let second_id = *world
        .pwb_workspace_ids
        .get(&second)
        .unwrap_or_else(|| panic!("workspace {second:?} must be seeded"));

    // Pick an existing member of the FIRST workspace and ALSO make them a member of
    // the SECOND, so the same global users row is reachable from both workspaces'
    // membership-bounded predicate.
    let shared_user: uuid::Uuid = sqlx::query_scalar(
        "SELECT user_id FROM workspace_memberships WHERE workspace_id = $1 LIMIT 1",
    )
    .bind(first_id)
    .fetch_one(&pool)
    .await
    .expect("first workspace must already have a member to share");

    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')
         ON CONFLICT DO NOTHING",
    )
    .bind(second_id)
    .bind(shared_user)
    .execute(&pool)
    .await
    .expect("add shared user to second workspace");

    world.pwb_shared_user_id = Some(shared_user);
}

/// `Then that shared user appears in the archive as a member of "<workspace>"` —
/// read the exported archive offline: the dual-membership user appears in the
/// archived `users` table AND in the archived `workspace_memberships` for the
/// target workspace (so it is included AS a member of the declared workspace, the
/// ADR-001 membership-bounded inclusion). Falsifiability: a missing shared user, or
/// a shared user with no membership edge into the target, reds.
#[then(regex = r#"^that shared user appears in the archive as a member of "([^"]+)"$"#)]
async fn shared_user_appears_as_member(world: &mut FoundryWorld, ws_name: String) {
    let shared = world
        .pwb_shared_user_id
        .expect("a shared dual-membership user must have been seeded")
        .to_string();
    let target_id = *world
        .pwb_workspace_ids
        .get(&ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} must be seeded"));
    let archive = read_archive_for_isolation(world);

    let in_users = archive
        .tables
        .iter()
        .find(|t| t.name == "users")
        .expect("archive has users table")
        .rows
        .iter()
        .any(|r| r.get("id").and_then(serde_json::Value::as_str) == Some(shared.as_str()));
    assert!(
        in_users,
        "the shared user {shared} must appear in the archived users set for {ws_name:?}"
    );

    let is_member_of_target = archive
        .tables
        .iter()
        .find(|t| t.name == "workspace_memberships")
        .expect("archive has workspace_memberships table")
        .rows
        .iter()
        .any(|r| {
            r.get("user_id").and_then(serde_json::Value::as_str) == Some(shared.as_str())
                && r.get("workspace_id").and_then(serde_json::Value::as_str)
                    == Some(target_id.to_string().as_str())
        });
    assert!(
        is_member_of_target,
        "the shared user {shared} must appear as a member of the target workspace \
         {ws_name:?} ({target_id}) in the archived memberships"
    );
}

/// `And verification does not flag that shared user as a sibling-workspace row` —
/// verify-export run on the archive exits 0 and reports isolation OK: the
/// dual-membership user is NOT a sibling leak (OD-PWB-1 / ADR-001). Falsifiability:
/// were the user wrongly flagged, verify would exit 6 with an isolation violation.
#[then(regex = r#"^verification does not flag that shared user as a sibling-workspace row$"#)]
async fn verification_does_not_flag_shared_user(world: &mut FoundryWorld) {
    let stdout = world
        .pwb_cli_stdout
        .as_deref()
        .expect("verify-export stdout captured");
    let stderr = world.pwb_cli_stderr.as_deref().unwrap_or("");
    let lower = stdout.to_ascii_lowercase();
    assert!(
        lower.contains("isolation: ok"),
        "verify-export must report isolation OK (the shared member is not a leak); \
         exit={:?}, stdout={stdout:?}, stderr={stderr:?}",
        world.pwb_cli_exit,
    );
    assert!(
        !stderr
            .to_ascii_lowercase()
            .contains("isolation check failed"),
        "verify-export must NOT flag the shared member as a sibling-workspace row; \
         stderr={stderr:?}"
    );
}

// ---------------------------------------------------------------------------
// Scenario 9 (step 02-03) — the falsifiability crux (NFR-PWB-ISO-01, AC-02.4).
// A planted sibling (Acme) row inside the Globex archive must make verify-export
// RED: the isolation check fails, the command exits non-zero, and the message
// NAMES a row resolving to a workspace other than the declared one. We tamper with
// the archive ON DISK (re-write the tar with one extra sibling-workspace row in an
// existing table's JSONL, bumping that table's manifest row_count so completeness
// still passes and isolation is the check that bites), then drive the REAL
// verify-export subprocess on the contaminated archive.
// ---------------------------------------------------------------------------

/// `And one row belonging to "<sibling>" is planted into that archive` — contaminate
/// the just-exported archive on disk: read its tar, append one fabricated row whose
/// `workspace_id` is the SIBLING workspace's real id to an existing tenant table
/// (`issues`), increment that table's manifest `row_counts` so the completeness
/// tripwire still passes, and re-write the tar at the same path. This is the planted
/// leak the isolation pass must catch.
#[given(regex = r#"^one row belonging to "([^"]+)" is planted into that archive$"#)]
async fn plant_sibling_row(world: &mut FoundryWorld, sibling: String) {
    let path = world
        .pwb_out_path
        .clone()
        .expect("an archive must have been exported first");
    let sibling_id = *world
        .pwb_workspace_ids
        .get(&sibling)
        .unwrap_or_else(|| panic!("sibling workspace {sibling:?} must be seeded"));
    plant_sibling_row_into_archive(&path, sibling_id);
}

/// `Then the isolation check fails` — verify-export's port-exposed stderr reports the
/// isolation check failed on the contaminated archive. Falsifiability: a verifier
/// that accepted the planted row would print no such failure line.
#[then(regex = r#"^the isolation check fails$"#)]
async fn isolation_check_fails(world: &mut FoundryWorld) {
    let stderr = world
        .pwb_cli_stderr
        .as_deref()
        .expect("verify-export stderr captured");
    let lower = stderr.to_ascii_lowercase();
    assert!(
        lower.contains("isolation check failed"),
        "verify-export must report the isolation check failed on a planted sibling row; \
         exit={:?}, stdout={:?}, stderr={stderr:?}",
        world.pwb_cli_exit,
        world.pwb_cli_stdout,
    );
}

/// `And the command exits with a non-zero code` — a contaminated archive must NOT
/// pass verification: verify-export exits non-zero. Falsifiability: an exit 0 here
/// would mean a sibling leak slipped through.
#[then(regex = r#"^the command exits with a non-zero code$"#)]
async fn command_exits_non_zero(world: &mut FoundryWorld) {
    assert!(
        matches!(world.pwb_cli_exit, Some(code) if code != 0),
        "verify-export must exit non-zero on a planted sibling row; exit={:?}, stderr={:?}",
        world.pwb_cli_exit,
        world.pwb_cli_stderr,
    );
}

/// `And the message identifies a row resolving to a workspace other than the declared
/// one` — the failure message NAMES the offending row: it carries the planted
/// sibling's real id and identifies it as a workspace other than the declared one.
/// Falsifiability: a generic "verification failed" with no resolved-workspace id REDs.
#[then(
    regex = r#"^the message identifies a row resolving to a workspace other than the declared one$"#
)]
async fn message_identifies_foreign_row(world: &mut FoundryWorld) {
    let stderr = world
        .pwb_cli_stderr
        .as_deref()
        .expect("verify-export stderr captured");
    let sibling_id = world
        .pwb_workspace_ids
        .get("Acme Corp")
        .expect("Acme Corp must be seeded")
        .to_string();
    assert!(
        stderr.contains(&sibling_id),
        "the failure message must name the planted sibling row's resolved workspace id \
         {sibling_id}; stderr={stderr:?}"
    );
    let lower = stderr.to_ascii_lowercase();
    assert!(
        lower.contains("workspace other than the declared")
            || lower.contains("not the declared workspace")
            || lower.contains("sibling-workspace row"),
        "the failure message must identify the row as resolving to a workspace OTHER than \
         the declared one; stderr={stderr:?}"
    );
}

/// Plant one fabricated sibling-workspace row into the tar archive at `path`: read
/// every entry, append a JSONL row whose `workspace_id` is `sibling_id` to the
/// `tables/issues.jsonl` entry, bump the manifest `row_counts.issues` by one (so the
/// completeness tripwire still passes and the isolation pass is what catches the
/// leak), and write the modified entries back to a fresh tar at the same path.
fn plant_sibling_row_into_archive(path: &std::path::Path, sibling_id: uuid::Uuid) {
    use std::io::Read;

    // Read the whole archive into memory (entry name -> bytes).
    let file = std::fs::File::open(path).expect("open archive to contaminate");
    let mut archive = tar::Archive::new(file);
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    for entry in archive.entries().expect("read tar entries") {
        let mut entry = entry.expect("tar entry");
        let name = entry
            .path()
            .expect("entry path")
            .to_string_lossy()
            .into_owned();
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).expect("read entry bytes");
        entries.push((name, buf));
    }

    // Append a fabricated sibling-workspace issues row.
    let planted = serde_json::json!({
        "id": uuid::Uuid::now_v7().to_string(),
        "workspace_id": sibling_id.to_string(),
    });
    let planted_line = format!("{planted}\n");
    for (name, buf) in &mut entries {
        if name == "tables/issues.jsonl" {
            buf.extend_from_slice(planted_line.as_bytes());
        }
        if name == "manifest.json" {
            let mut manifest: serde_json::Value =
                serde_json::from_slice(buf).expect("parse manifest.json");
            let counts = manifest
                .get_mut("row_counts")
                .and_then(serde_json::Value::as_object_mut)
                .expect("manifest row_counts");
            let current = counts
                .get("issues")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            counts.insert("issues".to_string(), serde_json::json!(current + 1));
            *buf = serde_json::to_vec(&manifest).expect("re-serialize manifest");
        }
    }

    // Re-write the tar at the same path from the modified entries.
    let out = std::fs::File::create(path).expect("re-create contaminated archive");
    let mut builder = tar::Builder::new(out);
    for (name, buf) in &entries {
        let mut header = tar::Header::new_gnu();
        header.set_path(name).expect("set entry path");
        header.set_size(buf.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append(&header, buf.as_slice())
            .expect("append contaminated entry");
    }
    builder.finish().expect("finish contaminated tar");
}

// ---------------------------------------------------------------------------
// Scenario 10 (step 02-03) — the isolation invariant, example-pinned (@property).
// Exporting then verifying EITHER workspace ("globex" or "acme") confirms zero rows
// resolve to any workspace OTHER than the target, and verify-export exits 0. The
// export + verify-export When steps are reused; this Then asserts the clean-archive
// isolation confirmation for the named target.
// ---------------------------------------------------------------------------

/// `Then verification confirms zero rows resolve to any workspace other than
/// "<target>"` — on a clean single-workspace export, verify-export reports isolation
/// OK (no row references a sibling workspace) and the archive declares the named
/// target workspace, so the confirmation is genuinely about <target>. Falsifiability:
/// an isolation violation, or an archive declaring the wrong workspace, REDs.
#[then(
    regex = r#"^verification confirms zero rows resolve to any workspace other than "([^"]+)"$"#
)]
async fn verification_confirms_zero_foreign_rows(world: &mut FoundryWorld, target: String) {
    let stdout = world
        .pwb_cli_stdout
        .as_deref()
        .expect("verify-export stdout captured");
    let stderr = world.pwb_cli_stderr.as_deref().unwrap_or("");
    let lower = stdout.to_ascii_lowercase();
    assert!(
        lower.contains("no row references a sibling workspace"),
        "verify-export must confirm zero rows resolve to a sibling workspace; \
         exit={:?}, stdout={stdout:?}, stderr={stderr:?}",
        world.pwb_cli_exit,
    );
    assert!(
        !stderr
            .to_ascii_lowercase()
            .contains("isolation check failed"),
        "verify-export must not report any isolation failure for a clean {target:?} export; \
         stderr={stderr:?}"
    );
    // The archive verify ran on must declare the named target workspace, so the
    // "no sibling" confirmation is genuinely about <target>.
    let target_name = token_to_workspace_name(&target).unwrap_or(&target);
    let target_id = *world
        .pwb_workspace_ids
        .get(target_name)
        .unwrap_or_else(|| panic!("workspace {target_name:?} must be seeded"));
    assert!(
        stdout.contains(&target_id.to_string()),
        "verify-export must declare the target workspace {target_name:?} ({target_id}); \
         stdout={stdout:?}"
    );
}

// ---------------------------------------------------------------------------
// Scenario 16 (step 01-03) — sole-workspace export is valid and removes nothing
// ---------------------------------------------------------------------------

/// `Given a single-tenant instance whose only workspace is "<name>"` — stand up the
/// harness with EXACTLY one workspace (and its full tenant data set), so the export
/// exercises the sole-workspace path. Unlike the two-workspace Background, no sibling
/// exists, so the CLI should note it is the only workspace on the instance.
#[given(regex = r#"^a single-tenant instance whose only workspace is "([^"]+)"$"#)]
async fn single_tenant_instance(world: &mut FoundryWorld, ws_name: String) {
    ensure_harness(world).await;
    let pool = harness_pool(world);

    // The Feature Background seeds TWO coexisting workspaces (Acme + Globex). This
    // scenario asserts the genuinely single-tenant install path, so first clear
    // every tenant table (the Background's rows) before seeding the sole workspace.
    // TRUNCATE ... CASCADE clears the FK-linked tenant rows in one shot.
    let tables = foundry_store::TENANT_TABLES.join(", ");
    sqlx::query(&format!("TRUNCATE TABLE {tables} CASCADE"))
        .execute(&pool)
        .await
        .unwrap_or_else(|e| panic!("truncate tenant tables for sole-workspace fixture: {e}"));
    world.pwb_workspace_ids.clear();

    let id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(id)
        .bind(&ws_name)
        .execute(&pool)
        .await
        .unwrap_or_else(|e| panic!("insert sole workspace {ws_name:?}: {e}"));
    world.pwb_workspace_ids.insert(ws_name.clone(), id);
    seed_tenant_data(&pool, id, &ws_name).await;
}

/// `Then the output notes that this is the only workspace on the instance` — on a
/// single-workspace instance the CLI's port-exposed stdout carries a note that this
/// is the only workspace. Falsifiability: removing that note from the export output
/// REDs this assertion.
#[then(regex = r#"^the output notes that this is the only workspace on the instance$"#)]
async fn output_notes_sole_workspace(world: &mut FoundryWorld) {
    let stdout = world
        .pwb_cli_stdout
        .as_deref()
        .expect("export CLI stdout captured");
    let lower = stdout.to_ascii_lowercase();
    assert!(
        lower.contains("only workspace"),
        "the export output must note this is the only workspace on the instance; got {stdout:?}"
    );
}

// ---------------------------------------------------------------------------
// Scenario 17 (step 01-03) — at-rest sensitivity disclosure on success
// ---------------------------------------------------------------------------

/// `Then the output prints a note that the archive contains password hashes and
/// machine-token rows` (NFR-PWB-SEC-01) — a successful export discloses that the
/// archive holds the two sensitive row kinds. Falsifiability: removing the
/// disclosure line from the export output REDs this assertion.
#[then(
    regex = r#"^the output prints a note that the archive contains password hashes and machine-token rows$"#
)]
async fn output_discloses_sensitive_contents(world: &mut FoundryWorld) {
    let stdout = world
        .pwb_cli_stdout
        .as_deref()
        .expect("export CLI stdout captured");
    let lower = stdout.to_ascii_lowercase();
    assert!(
        lower.contains("password_hash") || lower.contains("password hash"),
        "the export output must disclose the archive contains password hashes; got {stdout:?}"
    );
    assert!(
        lower.contains("machine_tokens")
            || lower.contains("machine-token")
            || lower.contains("machine token"),
        "the export output must disclose the archive contains machine-token rows; got {stdout:?}"
    );
}

/// `And the note advises treating the archive as sensitive at rest` — the disclosure
/// is actionable: it tells the operator to treat the archive as sensitive at rest.
/// Falsifiability: dropping the "sensitive at rest" advice REDs this assertion.
#[then(regex = r#"^the note advises treating the archive as sensitive at rest$"#)]
async fn note_advises_sensitive_at_rest(world: &mut FoundryWorld) {
    let stdout = world
        .pwb_cli_stdout
        .as_deref()
        .expect("export CLI stdout captured");
    let lower = stdout.to_ascii_lowercase();
    assert!(
        lower.contains("sensitive at rest"),
        "the export output must advise treating the archive as sensitive at rest; got {stdout:?}"
    );
}

// ---------------------------------------------------------------------------
// Scenario 11 (step 03-01) — unknown workspace refused with guidance (exit 2).
// The selector matches neither an id nor a name; the export resolve stage exits 2,
// the message redirects the operator to `list-workspaces`, and NO archive is
// written at the output path (AC-03.1).
// ---------------------------------------------------------------------------

/// `And the message tells Devansh to run "<command>"` — the unknown-selector
/// failure message (port-exposed on stderr) redirects the operator to the named
/// discovery command so they can find a valid id or name. Falsifiability: a bare
/// "not found" with no redirect REDs.
#[then(regex = r#"^the message tells Devansh to run "([^"]+)"$"#)]
async fn message_tells_to_run(world: &mut FoundryWorld, command: String) {
    let stderr = world
        .pwb_cli_stderr
        .as_deref()
        .expect("export CLI stderr captured");
    assert!(
        stderr.contains(&command),
        "the refusal message must tell Devansh to run {command:?}; \
         exit={:?}, stderr={stderr:?}",
        world.pwb_cli_exit,
    );
}

/// `And no archive file is created at that path` — an unknown-selector export is
/// refused at the resolve stage BEFORE any archive is written, so the output path
/// holds no file. Falsifiability: a stray (even partial) archive at the path REDs.
#[then(regex = r#"^no archive file is created at that path$"#)]
async fn no_archive_created(world: &mut FoundryWorld) {
    let path = world
        .pwb_out_path
        .clone()
        .expect("export path captured in the When step");
    assert!(
        !path.exists(),
        "no archive file may be created at {path:?} when the workspace is unknown; \
         exit={:?}, stderr={:?}",
        world.pwb_cli_exit,
        world.pwb_cli_stderr,
    );
}

// ---------------------------------------------------------------------------
// Scenario 15 (step 03-01) — DB unreachable reports a clear error (exit 3). The
// export When step is pointed at a deliberately bad DATABASE_URL so the real
// `Store::connect` fails; the export maps the connect error to exit 3 with an
// actionable message (AC-01.4), mirroring `admin_cli.rs`'s DB/infra failure code.
// ---------------------------------------------------------------------------

/// `Given the database is unreachable` — arm the export When step to point the CLI
/// at a bad DATABASE_URL (a closed port), so the next export drives a REAL connect
/// failure (not a mock) and exercises the exit-3 mapping.
#[given(regex = r#"^the database is unreachable$"#)]
async fn database_is_unreachable(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    world.pwb_db_unreachable = true;
}

/// `And the message says it could not connect to the database` — the DB-unreachable
/// failure message (port-exposed on stderr) tells the operator the connection
/// failed. Falsifiability: a generic failure with no connect-to-database phrasing REDs.
#[then(regex = r#"^the message says it could not connect to the database$"#)]
async fn message_says_could_not_connect(world: &mut FoundryWorld) {
    let stderr = world
        .pwb_cli_stderr
        .as_deref()
        .expect("export CLI stderr captured");
    let lower = stderr.to_ascii_lowercase();
    assert!(
        lower.contains("could not connect"),
        "the failure message must say it could not connect to the database; \
         exit={:?}, stderr={stderr:?}",
        world.pwb_cli_exit,
    );
}

// ---------------------------------------------------------------------------
// Scenario 12 (step 03-02) — output-path error fails BEFORE any DB read (exit 5).
// The parent directory of the output path does not exist; the export's pre-flight
// path stage catches it and exits 5 BEFORE opening any DB read snapshot, leaving NO
// file at the path (AC-03.2, NFR-PWB-ATOM-01). To PROVE the path stage precedes the
// DB read we also point the CLI at a deliberately unreachable DATABASE_URL: were the
// DB read attempted first, the export would exit 3 (connect failure); exit 5 instead
// proves the pre-flight path check ran first, before any tenant data was read.
// ---------------------------------------------------------------------------

/// `When Devansh exports "<selector>" to a path whose parent directory does not
/// exist` — drive the REAL export CLI with an out-path under a NON-EXISTENT parent
/// directory, AND with DATABASE_URL pointed at a closed port. The pre-flight path
/// stage must reject the unwritable path (exit 5) BEFORE the export ever tries to
/// connect to the DB — so the exit code distinguishes path-first (5) from
/// DB-first (3) ordering. Stash exit + stderr + the (never-created) path.
#[when(regex = r#"^Devansh exports "([^"]+)" to a path whose parent directory does not exist$"#)]
async fn devansh_exports_to_unwritable_path(world: &mut FoundryWorld, selector: String) {
    ensure_harness(world).await;

    // An out-path under a parent directory that does not exist on disk. The export's
    // pre-flight path stage must reject it (exit 5) without writing anything.
    let tempdir = tempfile::TempDir::new().expect("create export tempdir");
    let out_path = tempdir.path().join("does-not-exist").join("export.dump");
    world.pwb_tempdir = Some(tempdir);
    world.pwb_out_path = Some(out_path.clone());

    // Point the CLI at a deliberately unreachable DATABASE_URL (a closed port). If the
    // export read the DB before checking the path it would exit 3; the pre-flight path
    // stage must run FIRST and exit 5, proving no tenant data was read.
    let database_url = "postgres://foundry:foundry@127.0.0.1:1/foundry".to_string();
    let cli_selector =
        token_to_workspace_name(&selector).map_or_else(|| selector.clone(), str::to_string);

    let out = out_path.clone();
    let output = tokio::task::spawn_blocking(move || {
        AssertCommand::cargo_bin("foundry")
            .expect("cargo-bin foundry")
            .env("DATABASE_URL", database_url)
            .args(["doctor", "export-workspace"])
            .arg(&cli_selector)
            .arg(&out)
            .output()
            .expect("invoke foundry doctor export-workspace")
    })
    .await
    .expect("join blocking cli");

    world.pwb_cli_exit = Some(output.status.code().unwrap_or(-1));
    world.pwb_cli_stdout = Some(String::from_utf8_lossy(&output.stdout).into_owned());
    world.pwb_cli_stderr = Some(String::from_utf8_lossy(&output.stderr).into_owned());
}

/// `Then no file exists at that path` — a failed export leaves NO file at the output
/// path: neither a complete archive nor a discardable `.partial`. Falsifiability: a
/// stray file at the path REDs.
#[then(regex = r#"^no file exists at that path$"#)]
async fn no_file_exists_at_path(world: &mut FoundryWorld) {
    let path = world
        .pwb_out_path
        .clone()
        .expect("export path captured in the When step");
    assert!(
        !path.exists(),
        "no file may exist at {path:?} after a failed export; exit={:?}, stderr={:?}",
        world.pwb_cli_exit,
        world.pwb_cli_stderr,
    );
}

/// `And the failure happened before any tenant data was read` — the export's
/// pre-flight path stage rejected the unwritable path with exit 5 BEFORE any DB read.
/// The When step pointed the CLI at an unreachable DATABASE_URL, so a DB-first export
/// would exit 3 (connect failure); exit 5 proves the path check ran first and no
/// tenant data was read. Falsifiability: exit 3 (DB read attempted first) REDs.
#[then(regex = r#"^the failure happened before any tenant data was read$"#)]
async fn failure_before_any_db_read(world: &mut FoundryWorld) {
    assert_eq!(
        world.pwb_cli_exit,
        Some(5),
        "the export must fail at the pre-flight path stage (exit 5) BEFORE any DB read; \
         an exit 3 would mean the DB read was attempted first against the unreachable \
         DATABASE_URL. exit={:?}, stderr={:?}",
        world.pwb_cli_exit,
        world.pwb_cli_stderr,
    );
}

// ---------------------------------------------------------------------------
// Scenario 13 (step 03-02) — a disk-full / killed export leaves no complete-looking
// archive at the final path (NFR-PWB-ATOM-01: <out>.partial → fsync → rename). We
// simulate the disk filling mid-write by making the output's parent directory
// read-only, so the atomic write fails: the final <out> never appears (at most a
// discardable .partial may), and a later verify-export on the final path finds no
// archive to accept.
// ---------------------------------------------------------------------------

/// `Given an export of "<selector>" fails mid-write because the disk fills` — drive
/// the REAL export CLI against the live per-scenario DB (so the DB read succeeds and
/// the write stage is genuinely reached), but with the output's parent directory made
/// READ-ONLY so the archive write fails like a disk-full. The atomic `.partial` →
/// rename discipline must leave NO complete-looking file at the final `<out>` path.
#[given(regex = r#"^an export of "([^"]+)" fails mid-write because the disk fills$"#)]
async fn export_fails_mid_write_disk_full(world: &mut FoundryWorld, selector: String) {
    ensure_harness(world).await;
    let base = ensure_postgres().await;
    let schema = world
        .pwb_harness
        .as_ref()
        .expect("pwb harness")
        .schema
        .clone();
    let database_url = format!("{base}?options=-csearch_path%3D{schema}");

    // A real, EXISTING parent directory the pre-flight path check accepts — then made
    // read-only so the actual archive write (which the DB read precedes) fails like a
    // full disk. This reaches the write stage, unlike scenario 12's missing-parent path.
    let tempdir = tempfile::TempDir::new().expect("create export tempdir");
    let parent = tempdir.path().join("readonly");
    std::fs::create_dir(&parent).expect("create read-only parent dir");
    let out_path = parent.join("export.dump");
    let mut perms = std::fs::metadata(&parent)
        .expect("read parent perms")
        .permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&parent, perms).expect("make parent read-only");

    world.pwb_tempdir = Some(tempdir);
    world.pwb_out_path = Some(out_path.clone());

    let cli_selector =
        token_to_workspace_name(&selector).map_or_else(|| selector.clone(), str::to_string);
    let out = out_path.clone();
    let output = tokio::task::spawn_blocking(move || {
        AssertCommand::cargo_bin("foundry")
            .expect("cargo-bin foundry")
            .env("DATABASE_URL", database_url)
            .args(["doctor", "export-workspace"])
            .arg(&cli_selector)
            .arg(&out)
            .output()
            .expect("invoke foundry doctor export-workspace")
    })
    .await
    .expect("join blocking cli");

    world.pwb_cli_exit = Some(output.status.code().unwrap_or(-1));
    world.pwb_cli_stdout = Some(String::from_utf8_lossy(&output.stdout).into_owned());
    world.pwb_cli_stderr = Some(String::from_utf8_lossy(&output.stderr).into_owned());

    // Restore write permission on the parent so the TempDir can be cleaned up and the
    // Then steps can stat the (absent) final path without permission noise.
    if let Some(parent) = out_path.parent() {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o755));
    }
}

/// `Then no file exists at the final output path` — the disk-full export left NO
/// complete-looking archive at the final `<out>` path: the atomic rename never ran.
/// Falsifiability: a half-written file at `<out>` REDs.
#[then(regex = r#"^no file exists at the final output path$"#)]
async fn no_file_at_final_path(world: &mut FoundryWorld) {
    let path = world
        .pwb_out_path
        .clone()
        .expect("export path captured in the Given step");
    assert!(
        !path.exists(),
        "no complete-looking archive may exist at the final path {path:?} after a \
         disk-full export; exit={:?}, stderr={:?}",
        world.pwb_cli_exit,
        world.pwb_cli_stderr,
    );
}

/// `And at most a discardable partial file remains` — the only artifact a failed
/// atomic write may leave is a discardable `<out>.partial`; nothing else (and no file
/// at the final `<out>`). Falsifiability: a complete archive at `<out>`, or any
/// non-`.partial` stray file, REDs.
#[then(regex = r#"^at most a discardable partial file remains$"#)]
async fn at_most_partial_remains(world: &mut FoundryWorld) {
    let path = world
        .pwb_out_path
        .clone()
        .expect("export path captured in the Given step");
    assert!(
        !path.exists(),
        "the final archive path {path:?} must hold no file; only a discardable \
         <out>.partial may remain"
    );
    let partial = path.with_extension("partial");
    if let Some(parent) = path.parent() {
        if let Ok(entries) = std::fs::read_dir(parent) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                assert!(
                    entry_path == partial,
                    "the only artifact a failed export may leave is the discardable \
                     partial {partial:?}; found a stray {entry_path:?}"
                );
            }
        }
    }
}

/// `And a later verify-export on the final path finds no archive to accept` — running
/// the REAL verify-export CLI on the (absent) final path must NOT accept it as a valid
/// archive: there is no file to read, so verify exits non-zero. Falsifiability: a
/// verify-export that exits 0 on the missing path (accepting a phantom archive) REDs.
#[then(regex = r#"^a later verify-export on the final path finds no archive to accept$"#)]
async fn verify_finds_no_archive(world: &mut FoundryWorld) {
    let path = world
        .pwb_out_path
        .clone()
        .expect("export path captured in the Given step");
    let path_arg = path.clone();
    let output = tokio::task::spawn_blocking(move || {
        AssertCommand::cargo_bin("foundry")
            .expect("cargo-bin foundry")
            .args(["doctor", "verify-export"])
            .arg(&path_arg)
            .output()
            .expect("invoke foundry doctor verify-export")
    })
    .await
    .expect("join blocking cli");

    let exit = output.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_ne!(
        exit, 0,
        "verify-export on the final path must find no archive to accept (non-zero exit); \
         a disk-full export left no complete-looking archive there. stderr={stderr:?}"
    );
}

/// Snapshot every tenant table as an ordered list of whole-row JSON strings, keyed
/// by table name (the slice-05 idiom). `to_jsonb(t.*)` renders the entire row
/// deterministically; ordering by the row text makes the comparison
/// insertion-order independent. Used by the read-only proof to assert the export
/// changed NOTHING.
async fn snapshot_tenant_tables(pool: &PgPool) -> std::collections::HashMap<String, Vec<String>> {
    let mut out = std::collections::HashMap::new();
    for table in foundry_store::TENANT_TABLES {
        let sql =
            format!("SELECT to_jsonb(t.*)::text AS row_json FROM {table} t ORDER BY row_json");
        let rows = sqlx::query(&sql)
            .fetch_all(pool)
            .await
            .unwrap_or_else(|e| panic!("snapshot {table}: {e}"));
        let row_jsons = rows
            .into_iter()
            .map(|r| sqlx::Row::get::<String, _>(&r, "row_json"))
            .collect();
        out.insert((*table).to_string(), row_jsons);
    }
    out
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
