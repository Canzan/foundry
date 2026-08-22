//! US-03 step definitions — operator backs up and restores.
//!
//! Strategy C, per `distill/driver.md` §2c: real `pg_dump` + `pg_restore`
//! shelled out as subprocesses against the slice-1 shared Postgres
//! (the dump source) plus a per-scenario second Postgres container
//! (the restore target — restore is destructive and cannot share
//! state with sibling scenarios).
//!
//! The CLI scenarios drive `foundry doctor backup-verify <file>` via
//! `assert_cmd::Command::cargo_bin("foundry")` — this is the slice-3
//! driving-adapter coverage for the CLI entry point (P1-RCA fix).
//!
//! Step modules per the contributor convention reuse the Background
//! lines from US-05 / US-07 / US-08 (workspace, member-belongs-to-team,
//! signed-in, project-has-issue). New step phrases here:
//!
//! - seed N issues / attachments / comments / sessions into the
//!   slice-1 source database
//! - dump (with optional truncate) and restore the database
//! - point a foundry-app replica at the restored DB
//! - assert restored row counts, attachment byte-identity, sequential
//!   key continuity, and session-cookie validity
//! - invoke `foundry doctor backup-verify` as a subprocess and assert
//!   on its stdout / stderr / exit-code contract

use crate::support::file_upload_env;
use crate::support::harness::InProcHarness;
use crate::support::notify_recorder::{notifier_from_recorder, DeliveryRecorder};
use crate::support::pg_backup::{
    dump_schema_to_file, fresh_dump_path, restore_file_to_schema, spawn_restore_target,
    truncate_dump,
};
use crate::world::FoundryWorld;
use assert_cmd::Command as AssertCommand;
use cucumber::{given, then, when};
use foundry_app::clock::MockClock;
use foundry_app::test_support::spawn_app_with_listener;
use foundry_app::ProviderKind;
use foundry_app::{AppState, DEFAULT_FILE_UPLOAD_MAX_MB, DEFAULT_SSE_HEARTBEAT_MS};
use foundry_store::Store;
use reqwest::header::HeaderMap;
use reqwest::multipart::{Form, Part};
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use secrecy::SecretString;
use sha2::{Digest, Sha256};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

const TEST_NOW: &str = "2026-01-15T12:00:00Z";
const MEMBER_PASSWORD: &str = "mei-correct-horse-battery-staple";
/// Password the slice-1 us_06 Background step seeds for `devansh@acme.com`.
/// US-03's WS scenario asserts "signing in with the same password
/// succeeds against the restored instance" — same as in source DB.
const ADMIN_PASSWORD: &str = "admin-password-from-bootstrap";

fn now_anchor() -> time::OffsetDateTime {
    time::OffsetDateTime::parse(TEST_NOW, &time::format_description::well_known::Rfc3339)
        .expect("parse anchor")
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(Policy::none())
        .cookie_store(false)
        .build()
        .expect("build reqwest client")
}

async fn ensure_harness(world: &mut FoundryWorld) {
    if world.harness.is_none() {
        let harness = InProcHarness::spawn(now_anchor()).await;
        world.harness = Some(harness);
    }
    if world.http.is_none() {
        world.http = Some(http_client());
    }
}

fn identity_for(who: &str) -> (String, String) {
    match who {
        "Mei" => ("mei@acme.com".to_string(), MEMBER_PASSWORD.to_string()),
        "Hiroshi" => ("hiroshi@acme.com".to_string(), MEMBER_PASSWORD.to_string()),
        "Devansh" => ("devansh@acme.com".to_string(), ADMIN_PASSWORD.to_string()),
        other => panic!("no identity registered for {other:?}"),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().iter().map(|b| format!("{:02x}", b)).collect()
}

/// Deterministic synthetic byte pattern (same scheme as US-11).
fn synthetic_bytes(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i & 0xff) as u8).collect()
}

async fn lookup_project_slug_by_prefix(world: &FoundryWorld, prefix: &str) -> String {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let row: (String,) = sqlx::query_as("SELECT slug FROM projects WHERE key_prefix = $1")
        .bind(prefix)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|err| panic!("project with prefix {prefix:?} not found: {err}"));
    row.0
}

// ============== Given: seed source state ===============================

/// "the workspace contains 5 issues with titles \"AUTH-1\" through \"AUTH-5\""
///
/// The feature file titles refer to issue keys, not to free-form
/// titles; we seed both the per-project number AND a deterministic
/// title so downstream assertions can grep by key.
#[given(
    regex = r#"^the workspace contains (\d+) issues with titles "(\w+)-(\d+)" through "(\w+)-(\d+)"$"#
)]
async fn seed_issues_by_key_range(
    world: &mut FoundryWorld,
    count: u32,
    prefix_lo: String,
    lo: i32,
    _prefix_hi: String,
    hi: i32,
) {
    ensure_harness(world).await;
    assert_eq!(
        (hi - lo + 1) as u32,
        count,
        "feature range {lo}..={hi} does not match count {count}"
    );
    seed_issue_range(world, &prefix_lo, lo, hi).await;
}

/// "the workspace contains issues \"AUTH-1\" through \"AUTH-5\""
#[given(regex = r#"^the workspace contains issues "(\w+)-(\d+)" through "(\w+)-(\d+)"$"#)]
async fn seed_issue_key_range(
    world: &mut FoundryWorld,
    prefix_lo: String,
    lo: i32,
    _prefix_hi: String,
    hi: i32,
) {
    ensure_harness(world).await;
    seed_issue_range(world, &prefix_lo, lo, hi).await;
}

/// "the workspace contains 4 issues and 2 attachments"
#[given(regex = r"^the workspace contains (\d+) issues and (\d+) attachments$")]
async fn seed_issues_and_attachments_counts(
    world: &mut FoundryWorld,
    issue_count: u32,
    attachment_count: u32,
) {
    ensure_harness(world).await;
    // Seed N issues (AUTH-1..AUTH-N).
    seed_issue_range(world, "AUTH", 1, issue_count as i32).await;
    // Distribute the attachments across the first attachment_count issues
    // (one per issue, deterministic 1 KiB body each so we can assert
    // counts independent of attachment size).
    for n in 1..=(attachment_count.min(issue_count) as i32) {
        let filename = format!("seed-attachment-{n}.bin");
        let bytes = synthetic_bytes(1024);
        upload_attachment_via_http(
            world,
            "Mei",
            "AUTH",
            n,
            &filename,
            "application/octet-stream",
            bytes,
        )
        .await;
    }
}

/// "issue \"AUTH-3\" has an attachment \"screenshot.png\" of 256 kilobytes"
#[given(regex = r#"^issue "(\w+)-(\d+)" has an attachment "([^"]+)" of (\d+) kilobytes$"#)]
async fn issue_has_attachment_kb(
    world: &mut FoundryWorld,
    prefix: String,
    number: i32,
    filename: String,
    kb: u32,
) {
    ensure_harness(world).await;
    let bytes = synthetic_bytes(kb as usize * 1024);
    upload_attachment_via_http(world, "Mei", &prefix, number, &filename, "image/png", bytes).await;
}

/// "issue \"AUTH-1\" has 3 attachments of 100, 2000, and 8000 kilobytes respectively"
///
/// Seeds the issue row first if it doesn't exist — the feature file's
/// Background pre-seeds workspace/team/project but leaves issue
/// creation to the per-scenario Given lines.
#[given(
    regex = r#"^issue "(\w+)-(\d+)" has (\d+) attachments of (\d+), (\d+), and (\d+) kilobytes respectively$"#
)]
async fn issue_has_three_attachments(
    world: &mut FoundryWorld,
    prefix: String,
    number: i32,
    count: u32,
    kb1: u32,
    kb2: u32,
    kb3: u32,
) {
    ensure_harness(world).await;
    assert_eq!(
        count, 3,
        "feature line said {count} attachments but listed 3 sizes"
    );
    seed_issue_range(world, &prefix, number, number).await;
    let plans: [(u32, &str, &str); 3] = [
        (kb1, "evidence-a.bin", "application/octet-stream"),
        (kb2, "evidence-b.bin", "application/pdf"),
        (kb3, "evidence-c.bin", "application/zip"),
    ];
    for (kb, fname, ct) in plans {
        let bytes = synthetic_bytes(kb as usize * 1024);
        upload_attachment_via_http(world, "Mei", &prefix, number, fname, ct, bytes).await;
    }
}

/// "the workspace contains 3 issues, 2 comments, 1 attachment, and 1 active session for Mei"
#[given(
    regex = r"^the workspace contains (\d+) issues, (\d+) comments, (\d+) attachment, and (\d+) active session for (\w+)$"
)]
async fn seed_mixed_state(
    world: &mut FoundryWorld,
    issues: u32,
    comments: u32,
    attachments: u32,
    sessions: u32,
    who: String,
) {
    ensure_harness(world).await;
    // Seed N issues (AUTH-1..AUTH-N).
    seed_issue_range(world, "AUTH", 1, issues as i32).await;
    // Add `comments` comments, all on AUTH-1, written by Mei.
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let lookup: (uuid::Uuid, uuid::Uuid, uuid::Uuid) = sqlx::query_as(
        "SELECT i.id, i.project_id, i.workspace_id
           FROM issues i
           JOIN projects p ON p.id = i.project_id
          WHERE p.key_prefix = 'AUTH' AND i.number = 1",
    )
    .fetch_one(pool)
    .await
    .expect("AUTH-1 lookup");
    let (issue_id, _project_id, workspace_id) = lookup;
    let author: (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM users WHERE email_lower = 'mei@acme.com'")
            .fetch_one(pool)
            .await
            .expect("mei user lookup");
    for i in 1..=comments {
        let comment_id = uuid::Uuid::now_v7();
        let body = format!("Seed comment #{i} before backup");
        sqlx::query(
            "INSERT INTO comments
                  (id, workspace_id, issue_id, author_id, body_markdown, body_html)
              VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(comment_id)
        .bind(workspace_id)
        .bind(issue_id)
        .bind(author.0)
        .bind(&body)
        .bind(format!("<p>{body}</p>"))
        .execute(pool)
        .await
        .expect("insert seed comment");
    }
    // Attach `attachments` files to AUTH-1 via the real HTTP path so
    // the bytea + sha256 columns mirror production semantics.
    for n in 1..=(attachments as i32) {
        let filename = format!("seed-mixed-{n}.bin");
        upload_attachment_via_http(
            world,
            "Mei",
            "AUTH",
            1,
            &filename,
            "application/octet-stream",
            synthetic_bytes(2048),
        )
        .await;
    }
    // Capture an active session for the named user. The real
    // tower-sessions store inserts a row into the `session` table
    // when the cookie is minted via /sign-in; we drive that flow over
    // HTTP and stash the cookie value for the restore-side assertion.
    assert_eq!(sessions, 1, "slice-3 only supports 1 active-session seed");
    let (email, password) = identity_for(&who);
    let cookie = mint_session_for(world, &email, &password).await;
    world.us_03_pre_backup_session_cookie = Some(cookie);
}

/// "the operator has captured a backup of the database to a file"
#[given(regex = r"^the operator has captured a backup of the database to a file$")]
async fn operator_has_captured_backup(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    capture_backup(world).await;
}

/// "the backup file has been truncated to its first 1024 bytes"
#[given(regex = r"^the backup file has been truncated to its first (\d+) bytes$")]
async fn backup_truncated_to_bytes(world: &mut FoundryWorld, keep_bytes: u64) {
    let path = world
        .us_03_backup_file
        .clone()
        .expect("backup file captured before truncation");
    truncate_dump(&path, keep_bytes).expect("truncate dump");
}

// ============== When: backup, restore, replica re-point, CLI ===========

/// "the operator captures a complete backup of the running Foundry instance into a single backup file"
#[when(
    regex = r"^the operator captures a complete backup of the running Foundry instance into a single backup file$"
)]
async fn operator_captures_full_backup(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    capture_backup(world).await;
}

/// "the operator restores that backup file onto a freshly-booted database"
#[when(regex = r"^the operator restores that backup file onto a freshly-booted database$")]
async fn operator_restores_onto_fresh(world: &mut FoundryWorld) {
    restore_backup_into_fresh_target(world).await;
}

/// "the operator points a foundry-app replica at the restored database"
#[when(regex = r"^the operator points a foundry-app replica at the restored database$")]
async fn operator_points_replica_at_restored(world: &mut FoundryWorld) {
    point_replica_at_restored(world).await;
}

/// "the operator backs up and restores the database"
///
/// Composite of capture + spawn + restore + re-point so siblings that
/// don't care about the intermediate states get one line in their
/// Gherkin. Always re-points the replica so subsequent Then steps
/// query the restored instance.
#[when(regex = r"^the operator backs up and restores the database$")]
async fn operator_backs_up_and_restores(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    capture_backup(world).await;
    restore_backup_into_fresh_target(world).await;
    point_replica_at_restored(world).await;
}

/// "the operator captures a backup and then drops every Foundry table from the source database"
#[when(
    regex = r"^the operator captures a backup and then drops every Foundry table from the source database$"
)]
async fn operator_captures_then_drops_source(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    capture_backup(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    // The migrations file `0001_init.sql` and siblings create these
    // tables in the per-scenario schema. Dropping CASCADE leaves the
    // schema empty so subsequent application-level reads against the
    // SOURCE harness would 500; the post-restore Then steps only ever
    // read from the RESTORED harness, so this is safe.
    for table in [
        "issue_attachments",
        "comments",
        "outbox",
        "issues",
        "projects",
        "team_memberships",
        "teams",
        "session",
        "signin_attempts",
        "reset_tokens",
        "invites",
        "workspace_memberships",
        "users",
        "bootstrap_tokens",
        "workspaces",
    ] {
        let _ = sqlx::query(&format!("DROP TABLE IF EXISTS {table} CASCADE"))
            .execute(pool)
            .await;
    }
}

/// "the operator restores the backup onto a clean database"
#[when(regex = r"^the operator restores the backup onto a clean database$")]
async fn operator_restores_onto_clean(world: &mut FoundryWorld) {
    restore_backup_into_fresh_target(world).await;
    point_replica_at_restored(world).await;
}

/// "Mei files a new issue against \"Auth v2\" with title \"Post-restore issue creation\" on the restored instance"
#[when(
    regex = r#"^(\w+) files a new issue against "([^"]+)" with title "([^"]*)" on the restored instance$"#
)]
async fn member_files_issue_on_restored(
    world: &mut FoundryWorld,
    who: String,
    project_name: String,
    title: String,
) {
    let restored = world
        .us_03_restored_harness
        .as_ref()
        .expect("restored harness — call the restore step first");
    let (email, password) = identity_for(&who);
    let project_slug = lookup_project_slug_on_restored(restored, &project_name).await;
    let team_slug = "backend";
    let url = format!("/team/{team_slug}/project/{project_slug}/issues");
    let outcome = crate::support::harness::signed_in_post(
        restored,
        world.http.as_ref().expect("http"),
        &email,
        &password,
        &url,
        &[("title", &title)],
    )
    .await;
    assert!(
        matches!(outcome.status.as_u16(), 200 | 303),
        "file-issue on restored returned {status} body={body}",
        status = outcome.status,
        body = outcome.body,
    );
}

/// "the operator runs `foundry doctor backup-verify <backup-file>` as a subprocess"
#[when(regex = r"^the operator runs `foundry doctor backup-verify <backup-file>` as a subprocess$")]
async fn operator_runs_doctor_backup_verify(world: &mut FoundryWorld) {
    let path = world
        .us_03_backup_file
        .clone()
        .expect("backup file captured");
    // The CLI needs a writable Postgres to restore-and-count into. We
    // reuse the per-scenario restore target (the same one the dump+
    // restore step booted) if present; otherwise spawn one just for
    // the CLI invocation. Either way the CLI's row counts come from
    // an actual `pg_restore` round-trip.
    let probe_url = ensure_probe_url(world).await;
    let path_clone = path.clone();
    // assert_cmd uses std::process internally; run on a blocking
    // pool so the cucumber-rs async runtime isn't starved.
    let output = tokio::task::spawn_blocking(move || {
        AssertCommand::cargo_bin("foundry")
            .expect("cargo-bin foundry")
            .env("FOUNDRY_DOCTOR_PROBE_URL", probe_url)
            .args(["doctor", "backup-verify"])
            .arg(&path_clone)
            .output()
            .expect("invoke foundry doctor backup-verify")
    })
    .await
    .expect("join blocking cli");
    world.us_03_cli_stdout = Some(String::from_utf8_lossy(&output.stdout).into_owned());
    world.us_03_cli_stderr = Some(String::from_utf8_lossy(&output.stderr).into_owned());
    world.us_03_cli_exit_code = Some(output.status.code().unwrap_or(-1));
}

// ============== Then: restored state assertions ========================

/// "signing in as \"devansh@acme.com\" with the same password succeeds against the restored instance"
#[then(
    regex = r#"^signing in as "([^"]+)" with the same password succeeds against the restored instance$"#
)]
async fn signin_succeeds_on_restored(world: &mut FoundryWorld, email: String) {
    let restored = world
        .us_03_restored_harness
        .as_ref()
        .expect("restored harness");
    let http = world.http.as_ref().expect("http");
    // Pick the same password the user was seeded with. Devansh is the
    // admin; everyone else uses MEMBER_PASSWORD.
    let password = if email.eq_ignore_ascii_case("devansh@acme.com") {
        ADMIN_PASSWORD
    } else {
        MEMBER_PASSWORD
    };
    let cookie = sign_in_against(restored, http, &email, password).await;
    assert!(
        cookie.is_some(),
        "sign-in as {email:?} against restored instance failed (no session cookie issued)",
    );
}

/// "the workspace \"Acme Eng\" contains the same 5 issues \"AUTH-1\" through \"AUTH-5\""
#[then(
    regex = r#"^the workspace "([^"]+)" contains the same (\d+) issues "(\w+)-(\d+)" through "(\w+)-(\d+)"$"#
)]
async fn restored_workspace_has_issues(
    world: &mut FoundryWorld,
    _workspace: String,
    count: u32,
    prefix_lo: String,
    lo: i32,
    _prefix_hi: String,
    hi: i32,
) {
    assert_eq!(
        (hi - lo + 1) as u32,
        count,
        "feature range {lo}..={hi} does not match count {count}"
    );
    let restored = world
        .us_03_restored_harness
        .as_ref()
        .expect("restored harness");
    let pool = restored.app.state.store.pool();
    let rows: Vec<(i32,)> = sqlx::query_as(
        "SELECT i.number FROM issues i
           JOIN projects p ON p.id = i.project_id
          WHERE p.key_prefix = $1
          ORDER BY i.number ASC",
    )
    .bind(&prefix_lo)
    .fetch_all(pool)
    .await
    .expect("query issues on restored");
    let got: Vec<i32> = rows.into_iter().map(|(n,)| n).collect();
    let expected: Vec<i32> = (lo..=hi).collect();
    assert_eq!(
        got, expected,
        "restored issues for prefix {prefix_lo:?} do not match expected range",
    );
}

/// "the attachment \"screenshot.png\" on \"AUTH-3\" downloads byte-identical to the original"
#[then(
    regex = r#"^the attachment "([^"]+)" on "(\w+)-(\d+)" downloads byte-identical to the original$"#
)]
async fn attachment_byte_identical(
    world: &mut FoundryWorld,
    filename: String,
    prefix: String,
    number: i32,
) {
    let restored = world
        .us_03_restored_harness
        .as_ref()
        .expect("restored harness");
    let bytes = download_attachment_from_restored(world, &prefix, number, &filename).await;
    let recomputed = sha256_hex(&bytes);
    let original = world
        .us_03_uploaded_sha
        .get(&filename)
        .cloned()
        .unwrap_or_else(|| panic!("no captured sha256 for {filename:?}"));
    assert_eq!(
        recomputed,
        original,
        "attachment {filename:?} sha256 differs across restore: \
         post-restore={recomputed} original={original} \
         (restored instance: {addr})",
        addr = restored.app.addr,
    );
}

/// "each of the 3 attachments on \"AUTH-1\" downloads from the restored instance byte-identical to the original"
#[then(
    regex = r#"^each of the (\d+) attachments on "(\w+)-(\d+)" downloads from the restored instance byte-identical to the original$"#
)]
async fn each_attachment_byte_identical(
    world: &mut FoundryWorld,
    count: u32,
    prefix: String,
    number: i32,
) {
    // Walk the captured upload set for this issue and re-download
    // each one from the restored instance, comparing sha256.
    let originals: Vec<(String, String)> = world
        .us_03_uploaded_sha
        .iter()
        .map(|(f, s)| (f.clone(), s.clone()))
        .collect();
    assert_eq!(
        originals.len() as u32,
        count,
        "captured uploads ({}) do not match expected attachment count {count}",
        originals.len(),
    );
    for (filename, original_sha) in originals {
        let bytes = download_attachment_from_restored(world, &prefix, number, &filename).await;
        let recomputed = sha256_hex(&bytes);
        assert_eq!(
            recomputed, original_sha,
            "attachment {filename:?} sha256 differs across restore",
        );
    }
}

/// "the Content-Type recorded for each attachment is preserved through the restore"
#[then(regex = r"^the Content-Type recorded for each attachment is preserved through the restore$")]
async fn content_type_preserved_post_restore(world: &mut FoundryWorld) {
    let restored = world
        .us_03_restored_harness
        .as_ref()
        .expect("restored harness");
    let pool = restored.app.state.store.pool();
    for (filename, expected_ct) in world.us_03_uploaded_content_type.iter() {
        let row: (String,) =
            sqlx::query_as("SELECT content_type FROM issue_attachments WHERE filename = $1")
                .bind(filename)
                .fetch_one(pool)
                .await
                .unwrap_or_else(|err| panic!("content_type lookup for {filename:?}: {err}"));
        assert_eq!(
            &row.0, expected_ct,
            "Content-Type for {filename:?} not preserved through restore",
        );
    }
}

/// "the new issue's key is \"AUTH-6\""
#[then(regex = r#"^the new issue's key is "(\w+)-(\d+)"$"#)]
async fn new_issue_key_is(world: &mut FoundryWorld, prefix: String, number: i32) {
    let restored = world
        .us_03_restored_harness
        .as_ref()
        .expect("restored harness");
    let pool = restored.app.state.store.pool();
    let row: (i32,) = sqlx::query_as(
        "SELECT i.number FROM issues i
           JOIN projects p ON p.id = i.project_id
          WHERE p.key_prefix = $1
          ORDER BY i.number DESC LIMIT 1",
    )
    .bind(&prefix)
    .fetch_one(pool)
    .await
    .expect("most-recent issue number on restored");
    assert_eq!(
        row.0, number,
        "expected most-recent issue key {prefix}-{number}, got {prefix}-{}",
        row.0,
    );
}

/// "the restored instance contains all 3 issues, all 2 comments, and the 1 attachment downloads byte-identical to the original"
#[then(
    regex = r"^the restored instance contains all (\d+) issues, all (\d+) comments, and the (\d+) attachment downloads byte-identical to the original$"
)]
async fn restored_contains_all_mixed(
    world: &mut FoundryWorld,
    issues: u32,
    comments: u32,
    attachments: u32,
) {
    let restored = world
        .us_03_restored_harness
        .as_ref()
        .expect("restored harness");
    let pool = restored.app.state.store.pool();
    let issue_count: (i64,) = sqlx::query_as("SELECT count(*) FROM issues")
        .fetch_one(pool)
        .await
        .expect("count issues");
    let comment_count: (i64,) = sqlx::query_as("SELECT count(*) FROM comments")
        .fetch_one(pool)
        .await
        .expect("count comments");
    let attachment_count: (i64,) = sqlx::query_as("SELECT count(*) FROM issue_attachments")
        .fetch_one(pool)
        .await
        .expect("count attachments");
    assert_eq!(issue_count.0 as u32, issues, "restored issue count");
    assert_eq!(comment_count.0 as u32, comments, "restored comment count");
    assert_eq!(
        attachment_count.0 as u32, attachments,
        "restored attachment count"
    );
    // Verify byte-identity for the single seeded attachment.
    let originals: Vec<(String, String)> = world
        .us_03_uploaded_sha
        .iter()
        .map(|(f, s)| (f.clone(), s.clone()))
        .collect();
    for (filename, original_sha) in originals {
        let bytes = download_attachment_from_restored(world, "AUTH", 1, &filename).await;
        assert_eq!(
            sha256_hex(&bytes),
            original_sha,
            "attachment {filename:?} not byte-identical after restore",
        );
    }
}

/// "Mei's session from before the backup is still recognised by the restored instance"
#[then(
    regex = r"^(\w+)'s session from before the backup is still recognised by the restored instance$"
)]
async fn pre_backup_session_still_recognised(world: &mut FoundryWorld, _who: String) {
    let restored = world
        .us_03_restored_harness
        .as_ref()
        .expect("restored harness");
    let cookie = world
        .us_03_pre_backup_session_cookie
        .clone()
        .expect("pre-backup session cookie captured");
    let http = world.http.as_ref().expect("http");
    // Probe an authenticated page. /dashboard requires a session; if
    // tower-sessions doesn't recognise the cookie we get a 303 →
    // /sign-in. The session table content rode through pg_dump alongside
    // the rest of the schema, so the restored instance should accept
    // the same cookie ID.
    let url = format!("http://{}/dashboard", restored.app.addr);
    let resp = http
        .get(&url)
        .header(reqwest::header::COOKIE, &cookie)
        .send()
        .await
        .expect("get /dashboard on restored");
    let status = resp.status();
    let location = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    assert!(
        status == StatusCode::OK
            || (status == StatusCode::SEE_OTHER && !location.contains("/sign-in")),
        "expected restored /dashboard to recognise pre-backup session cookie; \
         got status={status} location={location:?}",
    );
}

// ============== Then: CLI subcommand assertions =========================

/// "the exit code is 0"
#[then(regex = r"^the exit code is (\d+)$")]
async fn cli_exit_code_is(world: &mut FoundryWorld, expected: i32) {
    let got = world.us_03_cli_exit_code.expect("cli exit code captured");
    assert_eq!(
        got,
        expected,
        "CLI exit code mismatch (stdout={stdout:?} stderr={stderr:?})",
        stdout = world.us_03_cli_stdout.as_deref().unwrap_or(""),
        stderr = world.us_03_cli_stderr.as_deref().unwrap_or(""),
    );
}

/// "the exit code is non-zero"
#[then(regex = r"^the exit code is non-zero$")]
async fn cli_exit_code_nonzero(world: &mut FoundryWorld) {
    let got = world.us_03_cli_exit_code.expect("cli exit code captured");
    assert!(
        got != 0,
        "expected non-zero exit, got 0 (stdout={stdout:?} stderr={stderr:?})",
        stdout = world.us_03_cli_stdout.as_deref().unwrap_or(""),
        stderr = world.us_03_cli_stderr.as_deref().unwrap_or(""),
    );
}

/// "the stdout contains a row-count entry for the \"issues\" table with the value 4"
#[then(
    regex = r#"^the stdout contains a row-count entry for the "([^"]+)" table with the value (\d+)$"#
)]
async fn stdout_row_count_entry(world: &mut FoundryWorld, table: String, value: u32) {
    let stdout = world
        .us_03_cli_stdout
        .as_deref()
        .expect("cli stdout captured");
    let needle = format!("{table}: {value}");
    assert!(
        stdout.contains(&needle),
        "stdout missing row-count line {needle:?}; got:\n{stdout}",
    );
}

/// "the stdout contains a \"status: OK\" line"
#[then(regex = r#"^the stdout contains a "([^"]+)" line$"#)]
async fn stdout_contains_line(world: &mut FoundryWorld, line: String) {
    let stdout = world
        .us_03_cli_stdout
        .as_deref()
        .expect("cli stdout captured");
    assert!(
        stdout.contains(&line),
        "stdout missing line {line:?}; got:\n{stdout}",
    );
}

/// "the stdout or stderr identifies the backup file as unreadable or truncated"
#[then(regex = r"^the stdout or stderr identifies the backup file as unreadable or truncated$")]
async fn stdout_or_stderr_identifies_corrupt(world: &mut FoundryWorld) {
    let stdout = world.us_03_cli_stdout.as_deref().unwrap_or("");
    let stderr = world.us_03_cli_stderr.as_deref().unwrap_or("");
    let combined = format!("{stdout}\n{stderr}").to_ascii_lowercase();
    let signals = [
        "truncat",
        "unreadable",
        "corrupt",
        "invalid",
        "could not read",
    ];
    assert!(
        signals.iter().any(|s| combined.contains(s)),
        "expected a corruption/truncation diagnostic in stdout or stderr; \
         got stdout={stdout:?} stderr={stderr:?}",
    );
}

// ============== Internals: backup + restore plumbing ====================

async fn seed_issue_range(world: &mut FoundryWorld, prefix: &str, lo: i32, hi: i32) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let row: (uuid::Uuid, uuid::Uuid) =
        sqlx::query_as("SELECT id, workspace_id FROM projects WHERE key_prefix = $1")
            .bind(prefix)
            .fetch_one(pool)
            .await
            .unwrap_or_else(|err| panic!("project {prefix:?} lookup: {err}"));
    let (project_id, workspace_id) = row;
    let author: (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM users WHERE email_lower = 'mei@acme.com'")
            .fetch_one(pool)
            .await
            .expect("mei lookup");
    for n in lo..=hi {
        let issue_id = uuid::Uuid::now_v7();
        sqlx::query(
            "INSERT INTO issues (id, project_id, workspace_id, number, title, author_id)
                  VALUES ($1, $2, $3, $4, $5, $6)
                  ON CONFLICT DO NOTHING",
        )
        .bind(issue_id)
        .bind(project_id)
        .bind(workspace_id)
        .bind(n)
        .bind(format!("{prefix}-{n} seeded"))
        .bind(author.0)
        .execute(pool)
        .await
        .unwrap_or_else(|err| panic!("seed issue {prefix}-{n}: {err}"));
    }
    sqlx::query("UPDATE projects SET next_issue_number = $1 WHERE id = $2")
        .bind(hi + 1)
        .bind(project_id)
        .execute(pool)
        .await
        .expect("bump next_issue_number");
}

async fn upload_attachment_via_http(
    world: &mut FoundryWorld,
    who: &str,
    prefix: &str,
    issue_number: i32,
    filename: &str,
    content_type: &str,
    bytes: Vec<u8>,
) {
    ensure_harness(world).await;
    let (email, password) = identity_for(who);
    let project_slug = lookup_project_slug_by_prefix(world, prefix).await;
    let team_slug = "backend";
    let url =
        format!("/team/{team_slug}/project/{project_slug}/issues/{issue_number}/attachments",);
    // Sign in FIRST — both reads (harness, http) and the cookie mint
    // path borrow `world` mutably under the hood; doing it here keeps
    // the upload path's borrow window narrow.
    let (cookie, csrf_token) = sign_in_capture_cookies(world, &email, &password).await;
    let sha = sha256_hex(&bytes);
    world.us_03_uploaded_sha.insert(filename.to_string(), sha);
    world
        .us_03_uploaded_content_type
        .insert(filename.to_string(), content_type.to_string());
    world
        .us_03_uploaded_bytes
        .insert(filename.to_string(), bytes.clone());
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let base = format!("http://{}", harness.app.addr);
    let form = Form::new().part(
        "file",
        Part::bytes(bytes)
            .file_name(filename.to_string())
            .mime_str(content_type)
            .expect("multipart mime"),
    );
    let resp = http
        .post(format!("{base}{url}"))
        .header(reqwest::header::COOKIE, cookie)
        .header("x-csrf-token", csrf_token)
        .multipart(form)
        .send()
        .await
        .expect("upload POST");
    let status = resp.status();
    assert!(
        matches!(status.as_u16(), 200 | 303),
        "seed upload of {filename:?} returned {status}",
    );
}

async fn capture_backup(world: &mut FoundryWorld) {
    let harness = world.harness.as_ref().expect("harness");
    let schema = harness.schema.clone();
    let source_url = crate::support::harness::ensure_postgres().await.to_string();
    let path = fresh_dump_path(&format!("scenario-{schema}"));
    let _bytes = dump_schema_to_file(&source_url, &schema, &path)
        .await
        .expect("dump source schema to file");
    world.us_03_backup_file = Some(path);
}

async fn restore_backup_into_fresh_target(world: &mut FoundryWorld) {
    let path = world
        .us_03_backup_file
        .clone()
        .expect("backup file before restore");
    if world.us_03_restore_target.is_none() {
        let target = spawn_restore_target().await;
        // Acquire the process-wide restore-serialisation lock before
        // the destructive `pg_restore --clean`. Hold it until the
        // FoundryWorld is dropped at scenario teardown so sibling
        // scenarios cannot clobber the restored state mid-assertion.
        let guard = target.lock_restore().await;
        world.us_03_restore_target = Some(target);
        world.us_03_restore_guard = Some(guard);
    }
    let target_url = world
        .us_03_restore_target
        .as_ref()
        .expect("restore target")
        .admin_url()
        .to_string();
    restore_file_to_schema(&target_url, &path)
        .await
        .expect("pg_restore into target");
}

async fn point_replica_at_restored(world: &mut FoundryWorld) {
    let harness_source = world.harness.as_ref().expect("source harness");
    let schema = harness_source.schema.clone();
    let target_admin_url = world
        .us_03_restore_target
        .as_ref()
        .expect("restore target")
        .admin_url()
        .to_string();

    // Build a per-schema pool against the restored target. The dump
    // preserves the schema name so the restored DB exposes the same
    // search_path-pinned namespace.
    let options = PgConnectOptions::from_str(&target_admin_url)
        .expect("parse target url")
        .options([("search_path", schema.as_str())]);
    let pool: PgPool = PgPoolOptions::new()
        .min_connections(1)
        .max_connections(4)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect_with(options)
        .await
        .expect("build restored pool");

    // Build an AppState mirroring the source harness's settings so the
    // restored instance speaks the same wire shape (cookie secrets,
    // file upload cap, etc.). We DO NOT re-run migrations: the dump
    // already carries the post-migration schema.
    let realtime_tx = foundry_realtime::build_broadcast();
    let fake_clock = MockClock::new(now_anchor());
    let fake_email = DeliveryRecorder::new();
    let file_upload_max_mb =
        file_upload_env::current_file_upload_max_mb().unwrap_or(DEFAULT_FILE_UPLOAD_MAX_MB);
    let store = Arc::new(Store::from_pool(pool));
    let state = AppState {
        oidc: None,
        store,
        session_secret: Arc::new(SecretString::new(
            "test-only-secret-must-be-at-least-32-bytes-long-please-yes".into(),
        )),
        machine_token_verifier: Arc::new(foundry_auth::test_keys::verifier()),
        // machine-token-admin-ux: US-03 restore scenarios do not mint tokens.
        machine_token_signer: None,
        session_cookie_secure: true,
        db_schema: schema.clone(),
        public_url: "http://localhost".into(),
        clock: fake_clock.clone(),
        notifier: notifier_from_recorder(&fake_email, &[ProviderKind::Log]),
        revoke_rate_limiter: Arc::new(foundry_app::rate_limit::RevokeRateLimiter::default()),
        realtime_tx,
        sse_heartbeat_ms: DEFAULT_SSE_HEARTBEAT_MS,
        file_upload_max_mb,
        db_unreachable: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        force_board_render_failure: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        test_migrations_dir: None,
        applied_migrations: Arc::new(std::sync::Mutex::new(
            foundry_store::MigrationReport::default(),
        )),
        test_migration_delay_ms: 0,
    };
    let listen_url = format!(
        "{base}?options=-csearch_path%3D{schema}",
        base = target_admin_url,
        schema = schema,
    );
    let app = spawn_app_with_listener(state, listen_url)
        .await
        .expect("spawn restored axum + listener");
    let restored = InProcHarness {
        app,
        fake_clock,
        fake_email,
        schema,
        // US-03 restore harness wires the log channel only — no webhook / email_api
        // vendor receiver.
        webhook_receiver: None,
        email_api_receiver: None,
        // recipient-notification-preferences: US-03 restore scenarios assert no
        // suppression; a fresh recorder satisfies the field (the notifier here uses
        // the default AllowAllSuppression, so nothing is recorded).
        suppressions: crate::support::notify_recorder::SuppressionRecorder::new(),
        // recipient-notification-preferences (fail-open edges): the restore harness
        // wires the default AllowAllSuppression (no fault switch is exercised here);
        // a fresh, unfaulted switch satisfies the field.
        suppression_faults: crate::support::notify_recorder::SuppressionFaults::new(),
    };
    world.us_03_restored_harness = Some(restored);
}

async fn lookup_project_slug_on_restored(harness: &InProcHarness, project_name: &str) -> String {
    let row: (String,) = sqlx::query_as("SELECT slug FROM projects WHERE name = $1")
        .bind(project_name)
        .fetch_one(harness.app.state.store.pool())
        .await
        .unwrap_or_else(|err| panic!("project {project_name:?} on restored: {err}"));
    row.0
}

async fn sign_in_against(
    harness: &InProcHarness,
    http: &reqwest::Client,
    email: &str,
    password: &str,
) -> Option<String> {
    let base = format!("http://{}", harness.app.addr);
    let csrf_get = http
        .get(format!("{base}/sign-in"))
        .send()
        .await
        .expect("csrf for restored sign-in");
    let csrf_full = csrf_get
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .map(|s| s.to_string())?;
    let csrf_token = csrf_full
        .strip_prefix("foundry_csrf=")
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string();
    let csrf_cookie = format!("foundry_csrf={csrf_token}");
    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("email", email.to_string());
    form.insert("password", password.to_string());
    form.insert("_csrf", csrf_token);
    let resp = http
        .post(format!("{base}/sign-in"))
        .header(reqwest::header::COOKIE, csrf_cookie)
        .form(&form)
        .send()
        .await
        .expect("post /sign-in on restored");
    resp.headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_session="))
        .map(|s| s.split(';').next().unwrap_or(s).to_string())
}

async fn mint_session_for(world: &mut FoundryWorld, email: &str, password: &str) -> String {
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    sign_in_against(harness, http, email, password)
        .await
        .unwrap_or_else(|| panic!("mint session for {email:?}: no Set-Cookie"))
}

async fn sign_in_capture_cookies(
    world: &mut FoundryWorld,
    email: &str,
    password: &str,
) -> (String, String) {
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let base = format!("http://{}", harness.app.addr);
    let csrf_get = http
        .get(format!("{base}/sign-in"))
        .send()
        .await
        .expect("csrf for sign-in");
    let csrf_full = csrf_get
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .map(|s| s.to_string())
        .expect("csrf cookie minted by /sign-in");
    let csrf_token = csrf_full
        .strip_prefix("foundry_csrf=")
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string();
    let csrf_pair = format!("foundry_csrf={csrf_token}");
    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("email", email.to_string());
    form.insert("password", password.to_string());
    form.insert("_csrf", csrf_token.clone());
    let signin = http
        .post(format!("{base}/sign-in"))
        .header(reqwest::header::COOKIE, csrf_pair)
        .form(&form)
        .send()
        .await
        .expect("post /sign-in for upload");
    let session_cookie = signin
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_session="))
        .map(|s| s.to_string())
        .expect("session cookie from sign-in");
    let session_pair = session_cookie
        .split(';')
        .next()
        .unwrap_or(&session_cookie)
        .to_string();
    let combined = format!("{session_pair}; foundry_csrf={csrf_token}");
    (combined, csrf_token)
}

async fn download_attachment_from_restored(
    world: &FoundryWorld,
    prefix: &str,
    issue_number: i32,
    filename: &str,
) -> Vec<u8> {
    let restored = world
        .us_03_restored_harness
        .as_ref()
        .expect("restored harness");
    let pool = restored.app.state.store.pool();
    let row: (uuid::Uuid, String) = sqlx::query_as(
        "SELECT a.id, p.slug FROM issue_attachments a
           JOIN issues i ON i.id = a.issue_id
           JOIN projects p ON p.id = i.project_id
          WHERE p.key_prefix = $1 AND i.number = $2 AND a.filename = $3
          ORDER BY a.created_at DESC LIMIT 1",
    )
    .bind(prefix)
    .bind(issue_number)
    .bind(filename)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|err| panic!("restored attachment {filename:?} lookup: {err}"));
    let (attachment_id, project_slug) = row;
    let http = world.http.as_ref().expect("http");
    let cookie = sign_in_against(restored, http, "mei@acme.com", MEMBER_PASSWORD)
        .await
        .expect("sign in on restored to download");
    let url = format!(
        "http://{addr}/team/backend/project/{project_slug}/issues/{issue_number}/attachments/{attachment_id}",
        addr = restored.app.addr,
    );
    let resp = http
        .get(&url)
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("download from restored");
    let status = resp.status();
    let bytes = resp.bytes().await.expect("download bytes").to_vec();
    assert_eq!(status, StatusCode::OK, "download status on restored");
    bytes
}

/// Ensure there is a Postgres URL the `foundry doctor backup-verify`
/// CLI can use as its probe target. The CLI restores the dump into
/// that DB to count rows. We use the process-wide restore target so
/// the daemon doesn't have to boot a third container per scenario.
///
/// The restore-mutex is acquired here too — the CLI invocation runs
/// `pg_restore --clean` against the same shared target so sibling
/// scenarios must not interleave.
async fn ensure_probe_url(world: &mut FoundryWorld) -> String {
    if world.us_03_restore_target.is_none() {
        let target = spawn_restore_target().await;
        let guard = target.lock_restore().await;
        world.us_03_restore_target = Some(target);
        world.us_03_restore_guard = Some(guard);
    }
    world
        .us_03_restore_target
        .as_ref()
        .expect("restore target")
        .admin_url()
        .to_string()
}

#[allow(dead_code)]
fn _imports_silencer() {
    // HeaderMap referenced from reqwest::header::HeaderMap above so we
    // keep the explicit import lints quiet across the migration.
    let _ = HeaderMap::new();
}
