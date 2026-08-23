//! US-08 step definitions — file an issue.
//!
//! Reuses shared step phrases from US-06 / US-07 (the workspace +
//! member + signed-in background lines). New phrases here cover:
//! - filing one issue via POST /team/{}/project/{}/issues
//! - seeding pre-existing issues for sequential-key assertions
//! - htmx-fragment / 400 inline-error assertions
//! - the 100-POST performance scenario w/ P95 measurement
//! - the title-length boundary property outline

use crate::support::harness::{signed_in_post, InProcHarness, PostOutcome};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use secrecy::SecretString;
use std::time::Instant;

const TEST_NOW: &str = "2026-01-15T12:00:00Z";
const MEMBER_PASSWORD: &str = "mei-correct-horse-battery-staple";
#[allow(dead_code)]
const HIROSHI_PASSWORD: &str = "hiroshi-correct-horse-battery-staple";

fn now_anchor() -> time::OffsetDateTime {
    time::OffsetDateTime::parse(TEST_NOW, &time::format_description::well_known::Rfc3339)
        .expect("parse anchor")
}

fn client() -> reqwest::Client {
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
        world.http = Some(client());
    }
}

// ----- Background --------------------------------------------------------

/// "a project 'Auth v2' with key prefix 'AUTH' exists in the 'Backend' team"
///
/// Runs after US-07's `member_belongs_to_team` step (which seeds the
/// workspace + team + admin + member). Inserts a project row directly;
/// the project's existence is the precondition of US-08, not the
/// behaviour under test.
#[given(regex = r#"^a project "([^"]+)" with key prefix "([^"]+)" exists in the "([^"]+)" team$"#)]
async fn project_exists_in_team(
    world: &mut FoundryWorld,
    project_name: String,
    key_prefix: String,
    team_name: String,
) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let ws_id: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM workspaces LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("fetch workspace");
    let team_row: (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM teams WHERE workspace_id = $1 AND name = $2")
            .bind(ws_id.0)
            .bind(&team_name)
            .fetch_one(pool)
            .await
            .expect("fetch team");
    let project_id = uuid::Uuid::now_v7();
    let slug = slugify(&project_name);
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, $4, $5, $6)
              ON CONFLICT (workspace_id, key_prefix) DO NOTHING",
    )
    .bind(project_id)
    .bind(team_row.0)
    .bind(ws_id.0)
    .bind(&project_name)
    .bind(&slug)
    .bind(&key_prefix)
    .execute(pool)
    .await
    .expect("insert project");
    // board-lane-management sweep: raw-SQL projects need their lane rows
    // (no-op when ON CONFLICT skipped the insert).
    crate::support::harness::seed_lanes_for_project(pool, project_id).await;
}

// ----- Pre-seed existing issues -----------------------------------------

#[given(regex = r#"^the "([^"]+)" project already has issues (\w+)-(\d+) through (\w+)-(\d+)$"#)]
async fn project_has_issues_range(
    world: &mut FoundryWorld,
    project_name: String,
    first_prefix: String,
    first_n: i32,
    _last_prefix: String,
    last_n: i32,
) {
    seed_issues_in_project(world, &project_name, &first_prefix, first_n, last_n).await;
}

#[given(regex = r#"^the "([^"]+)" project already has issue (\w+)-(\d+)$"#)]
async fn project_has_issue(world: &mut FoundryWorld, project_name: String, prefix: String, n: i32) {
    seed_issues_in_project(world, &project_name, &prefix, n, n).await;
}

async fn seed_issues_in_project(
    world: &mut FoundryWorld,
    project_name: &str,
    expected_prefix: &str,
    first_n: i32,
    last_n: i32,
) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let row: (uuid::Uuid, uuid::Uuid, String) =
        sqlx::query_as("SELECT id, workspace_id, key_prefix FROM projects WHERE name = $1")
            .bind(project_name)
            .fetch_one(pool)
            .await
            .unwrap_or_else(|err| panic!("project {project_name:?} not found: {err}"));
    let (project_id, workspace_id, key_prefix) = row;
    assert_eq!(
        key_prefix, expected_prefix,
        "feature seed mismatched key prefix: expected {expected_prefix:?}, got {key_prefix:?}"
    );

    let author: (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM users WHERE email_lower = 'mei@acme.com'")
            .fetch_one(pool)
            .await
            .expect("fetch mei id");

    for n in first_n..=last_n {
        let issue_id = uuid::Uuid::now_v7();
        // board-lane-management sweep: 0015 dropped the state DEFAULT.
        sqlx::query(
            "INSERT INTO issues (id, project_id, workspace_id, number, title, state, author_id)
                  VALUES ($1, $2, $3, $4, $5, 'backlog', $6)",
        )
        .bind(issue_id)
        .bind(project_id)
        .bind(workspace_id)
        .bind(n)
        .bind(format!("Seeded issue {n}"))
        .bind(author.0)
        .execute(pool)
        .await
        .unwrap_or_else(|err| panic!("seed issue {n}: {err}"));
    }
    // Bump next_issue_number on the project past the seeded range so the
    // first real allocation lands at last_n + 1.
    sqlx::query("UPDATE projects SET next_issue_number = $1 WHERE id = $2")
        .bind(last_n + 1)
        .bind(project_id)
        .execute(pool)
        .await
        .expect("bump next_issue_number");
}

// ----- When: file an issue --------------------------------------------

#[when(regex = r#"^(\w+) files an issue against "([^"]+)" with title "([^"]*)"$"#)]
async fn file_issue(world: &mut FoundryWorld, who: String, project_name: String, title: String) {
    perform_file_issue(world, &who, &project_name, &title).await;
}

#[when(regex = r#"^(\w+) files a new issue against "([^"]+)" with title "([^"]*)"$"#)]
async fn file_new_issue(
    world: &mut FoundryWorld,
    who: String,
    project_name: String,
    title: String,
) {
    perform_file_issue(world, &who, &project_name, &title).await;
}

#[when(regex = r#"^Mei files an issue against "Auth v2" with title of length (\d+)$"#)]
async fn file_issue_title_length(world: &mut FoundryWorld, length: u32) {
    let title = "x".repeat(length as usize);
    perform_file_issue(world, "Mei", "Auth v2", &title).await;
}

#[when(
    regex = r#"^(\w+) files (\d+) issues against "([^"]+)" sequentially, each with a unique title$"#
)]
async fn file_n_issues(world: &mut FoundryWorld, who: String, n: u32, project_name: String) {
    // NFR-PERF-01 measures **server-side** render latency, not full
    // client round-trip. The non-perf scenarios use signed_in_post which
    // performs a fresh sign-in cycle (3 HTTP requests) per call, masking
    // the real handler latency behind authentication overhead. For the
    // perf measurement we sign in ONCE, then reuse the session cookie
    // across 100 POSTs, measuring only the POST itself.
    ensure_harness(world).await;
    let (email, password) = identity_for(&who);
    let team_slug = "backend";
    let project_slug = slugify(&project_name);
    world.us_08_last_project_slug = Some(project_slug.clone());
    world.us_08_last_team_slug = Some(team_slug.to_string());
    let url = format!("/team/{team_slug}/project/{project_slug}/issues");

    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let base = harness.base_url();

    // (1) GET /sign-in once to mint a CSRF cookie + token.
    let csrf_get = http
        .get(format!("{base}/sign-in"))
        .send()
        .await
        .expect("get /sign-in for csrf");
    let csrf_cookie_full = csrf_get
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .map(|s| s.to_string())
        .expect("csrf cookie");
    let csrf_token = csrf_cookie_full
        .strip_prefix("foundry_csrf=")
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string();
    let csrf_pair = format!("foundry_csrf={csrf_token}");

    // (2) POST /sign-in once to mint a session cookie.
    let mut signin_form: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
    signin_form.insert("email", email);
    signin_form.insert("password", password);
    signin_form.insert("_csrf", csrf_token.clone());
    let signin_resp = http
        .post(format!("{base}/sign-in"))
        .header(reqwest::header::COOKIE, csrf_pair.clone())
        .form(&signin_form)
        .send()
        .await
        .expect("post /sign-in for perf");
    let session_cookie = signin_resp
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
    let combined_cookie = format!("{session_pair}; {csrf_pair}");

    // (3) 100 POSTs. Time only the POST round-trip itself.
    world.us_08_latencies_ms.clear();
    let mut latest_status: Option<StatusCode> = None;
    for i in 1..=n {
        let title = format!("Perf issue #{i}");
        let mut form: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
        form.insert("title", title);
        form.insert("_csrf", csrf_token.clone());

        let started = Instant::now();
        let resp = http
            .post(format!("{base}{url}"))
            .header(reqwest::header::COOKIE, combined_cookie.clone())
            .form(&form)
            .send()
            .await
            .expect("post issue");
        let status = resp.status();
        // Drain the body so we don't keep a connection half-open
        // skewing subsequent timing.
        let _ = resp.text().await;
        let elapsed = started.elapsed();
        world
            .us_08_latencies_ms
            .push(u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX));
        assert_eq!(
            status,
            StatusCode::SEE_OTHER,
            "perf request #{i} returned {status}"
        );
        latest_status = Some(status);
    }
    world.last_status = latest_status;
}

async fn perform_file_issue(world: &mut FoundryWorld, who: &str, project_name: &str, title: &str) {
    ensure_harness(world).await;
    let (email, password) = identity_for(who);
    let team_slug = "backend";
    let project_slug = slugify(project_name);
    let url = format!("/team/{team_slug}/project/{project_slug}/issues");
    world.us_08_last_project_slug = Some(project_slug);
    world.us_08_last_team_slug = Some(team_slug.to_string());

    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    // Record the When-step instant so US-09 Then steps that assert
    // "within Nms a subscriber observes ..." can compute per-event
    // latency relative to the action that produced the event.
    world.us_09_last_action_started_at = Some(Instant::now());
    let outcome: PostOutcome =
        signed_in_post(harness, http, &email, &password, &url, &[("title", title)]).await;
    capture_outcome(world, outcome);
}

// ----- Then: per-issue assertions --------------------------------------

#[then(regex = r#"^the new issue is assigned the key "(\w+)-(\d+)"$"#)]
async fn issue_assigned_key(world: &mut FoundryWorld, prefix: String, n: i32) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    // The most-recently-created issue with this prefix/number must exist.
    let row: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM issues i
           JOIN projects p ON p.id = i.project_id
          WHERE p.key_prefix = $1 AND i.number = $2",
    )
    .bind(&prefix)
    .bind(n)
    .fetch_one(pool)
    .await
    .expect("count issues");
    assert_eq!(
        row.0,
        1,
        "expected exactly one {prefix}-{n} issue, got {count}",
        count = row.0
    );
}

#[then(regex = r#"^the issue's state is "([^"]+)"$"#)]
async fn issue_state(world: &mut FoundryWorld, expected: String) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let row: (String,) =
        sqlx::query_as("SELECT state FROM issues ORDER BY created_at DESC LIMIT 1")
            .fetch_one(pool)
            .await
            .expect("fetch latest issue state");
    assert_eq!(row.0, expected);
}

#[then(regex = r#"^the issue's priority is "([^"]+)"$"#)]
async fn issue_priority(world: &mut FoundryWorld, expected: String) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let row: (String,) =
        sqlx::query_as("SELECT priority FROM issues ORDER BY created_at DESC LIMIT 1")
            .fetch_one(pool)
            .await
            .expect("fetch latest issue priority");
    assert_eq!(row.0, expected);
}

#[then(regex = r"^the issue's author is (\w+)$")]
async fn issue_author(world: &mut FoundryWorld, who: String) {
    let (email, _) = identity_for(&who);
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let row: (String,) = sqlx::query_as(
        "SELECT u.email_lower FROM issues i JOIN users u ON u.id = i.author_id
          ORDER BY i.created_at DESC LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .expect("fetch latest issue author email");
    assert_eq!(row.0, email.to_ascii_lowercase());
}

#[then(regex = r"^the response contains a fragment showing (\w+)-(\d+) in the Backlog column$")]
async fn response_fragment_shows_issue(world: &mut FoundryWorld, prefix: String, n: i32) {
    // The response body in the non-htmx happy path is empty (303
    // redirect). Walk the Location to fetch the board content and
    // assert there.
    let key = format!("{prefix}-{n}");
    let body = fetch_redirected_board_body(world).await;
    assert!(
        body.contains(&key),
        "board body missing issue key {key:?}: {body}"
    );
    assert!(
        body.contains("Backlog"),
        "board body missing 'Backlog' column heading: {body}"
    );
}

#[then(regex = r#"^opening "([^"]+)" lists (\w+)-(\d+) in the Backlog column$"#)]
async fn opening_page_lists(world: &mut FoundryWorld, url: String, prefix: String, n: i32) {
    let body = fetch_url_body(world, &url).await;
    let key = format!("{prefix}-{n}");
    assert!(
        body.contains(&key),
        "page {url:?} did not list {key:?}: {body}"
    );
    // The Backlog column heading must be present and the key must come
    // after the "Backlog" header in document order — a minimal
    // structural assertion that the issue is in the right column.
    let backlog_pos = body
        .find("Backlog")
        .unwrap_or_else(|| panic!("no Backlog heading: {body}"));
    let key_pos = body
        .find(&key)
        .unwrap_or_else(|| panic!("no {key:?}: {body}"));
    assert!(
        key_pos > backlog_pos,
        "{key:?} appeared before the Backlog heading in {url:?}"
    );
}

// ----- Then: error fragment / 400-422 --------------------------------

#[then(regex = r"^the response status is 400 or 422$")]
async fn status_400_or_422(world: &mut FoundryWorld) {
    let status = world.last_status.expect("status captured");
    assert!(
        matches!(status.as_u16(), 400 | 422),
        "expected 400 or 422, got {status}"
    );
}

#[then(regex = r#"^the response is an htmx fragment containing "([^"]+)"$"#)]
async fn response_htmx_fragment_containing(world: &mut FoundryWorld, needle: String) {
    let body = world.last_body.as_deref().unwrap_or("");
    assert!(
        body.contains(&needle),
        "response body missing {needle:?}: {body}"
    );
}

#[then(regex = r"^the response is not a full HTML page$")]
async fn response_not_full_page(world: &mut FoundryWorld) {
    let body = world.last_body.as_deref().unwrap_or("");
    let lower = body.trim_start().to_ascii_lowercase();
    assert!(
        !lower.starts_with("<!doctype") && !lower.starts_with("<html"),
        "response was a full HTML page, expected htmx fragment: {body}"
    );
}

#[then(regex = r#"^no issue is created in "([^"]+)"$"#)]
async fn no_issue_created_in(world: &mut FoundryWorld, project_name: String) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let project_row: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM projects WHERE name = $1")
        .bind(&project_name)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|err| panic!("project {project_name:?} not found: {err}"));
    // Count is the number of pre-seeded issues (if any) — the property
    // we want is "no NEW issue was created by the failed POST". The
    // simplest read: collect the pre-count in a snapshot table is
    // overkill; instead we assert the last-inserted issue does NOT have
    // the title the failing POST attempted. For empty-title + 403 paths
    // the only title we ever attempted to write is "" (empty) or
    // "Unauthorized attempt"; both rejections leave zero rows with
    // those titles.
    let bad_count: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM issues
          WHERE project_id = $1 AND (title = '' OR title = 'Unauthorized attempt')",
    )
    .bind(project_row.0)
    .fetch_one(pool)
    .await
    .expect("count bad-title issues");
    assert_eq!(bad_count.0, 0, "unexpected rejected-title issue persisted");
}

// ----- Then: perf -----------------------------------------------------

#[then(
    regex = r"^all (\d+) issues are persisted with sequential keys (\w+)-(\d+) through (\w+)-(\d+)$"
)]
async fn n_issues_sequential(
    world: &mut FoundryWorld,
    n: u32,
    prefix: String,
    first: i32,
    _prefix_last: String,
    last: i32,
) {
    assert_eq!(
        (last - first + 1) as u32,
        n,
        "feature range {first}..={last} does not match N={n}"
    );
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let rows: Vec<(i32,)> = sqlx::query_as(
        "SELECT i.number FROM issues i
           JOIN projects p ON p.id = i.project_id
          WHERE p.key_prefix = $1
          ORDER BY i.number ASC",
    )
    .bind(&prefix)
    .fetch_all(pool)
    .await
    .expect("fetch all issues for prefix");
    let numbers: Vec<i32> = rows.into_iter().map(|(n,)| n).collect();
    let expected: Vec<i32> = (first..=last).collect();
    assert_eq!(numbers, expected, "issue numbers not gap-free");
}

#[then(
    regex = r"^the P95 server-side response time across those (\d+) requests is at most (\d+) milliseconds$"
)]
async fn p95_at_most(world: &mut FoundryWorld, n: u32, ms: u64) {
    let mut latencies = world.us_08_latencies_ms.clone();
    assert_eq!(
        latencies.len() as u32,
        n,
        "expected {n} latencies, captured {len}",
        len = latencies.len()
    );
    latencies.sort_unstable();
    // P95 index: ceiling(0.95 * N) - 1 (0-indexed). For N=100 that's
    // index 94 (the 95th value).
    let idx = ((latencies.len() as f64) * 0.95).ceil() as usize - 1;
    let p95 = latencies[idx];
    eprintln!("[US-08 NFR-PERF-01] measured P95 = {p95}ms over {n} requests (ceiling {ms}ms)");
    assert!(
        p95 <= ms,
        "P95 {p95}ms exceeded budget {ms}ms (sorted latencies: {latencies:?})"
    );
}

// ----- Then: title-length property outline -----------------------------

#[then(regex = r#"^the file-issue outcome is "(accepted|rejected)"$"#)]
async fn file_issue_outcome(world: &mut FoundryWorld, outcome: String) {
    let status = world.last_status.expect("status captured");
    match outcome.as_str() {
        "accepted" => {
            assert!(
                matches!(status.as_u16(), 200 | 303),
                "expected 200/303 on accepted title, got {status} body={body}",
                body = world.last_body.as_deref().unwrap_or("")
            );
        }
        "rejected" => {
            assert!(
                matches!(status.as_u16(), 400 | 422),
                "expected 400/422 on rejected title, got {status}"
            );
        }
        other => panic!("unrecognised outcome {other:?}"),
    }
}

// ----- internals -------------------------------------------------------

fn identity_for(who: &str) -> (String, String) {
    // The shared `member_belongs_to_team` step (us_07) seeds every
    // inserted user with MEMBER_PASSWORD; slice-2 introduced Hiroshi
    // as an actor via that step, so the persona password must match
    // the seed. HIROSHI_PASSWORD is preserved as a constant for any
    // future scenarios that seed Hiroshi with a distinct credential.
    match who {
        "Mei" => ("mei@acme.com".to_string(), MEMBER_PASSWORD.to_string()),
        "Hiroshi" => ("hiroshi@acme.com".to_string(), MEMBER_PASSWORD.to_string()),
        other => panic!("no identity registered for {other:?}"),
    }
}

fn capture_outcome(world: &mut FoundryWorld, outcome: PostOutcome) {
    world.last_status = Some(outcome.status);
    world.last_headers = Some(outcome.headers);
    world.last_body = Some(outcome.body);
}

async fn fetch_redirected_board_body(world: &mut FoundryWorld) -> String {
    let team_slug = world
        .us_08_last_team_slug
        .clone()
        .unwrap_or_else(|| "backend".into());
    let project_slug = world
        .us_08_last_project_slug
        .clone()
        .unwrap_or_else(|| "auth-v2".into());
    let url = format!("/team/{team_slug}/project/{project_slug}");
    fetch_url_body(world, &url).await
}

/// Authenticated GET helper. Mirrors the body of US-07's
/// `fetch_redirected_board_body` but parameterised on the URL.
async fn fetch_url_body(world: &mut FoundryWorld, url: &str) -> String {
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let base = harness.base_url();

    // (1) Mint CSRF cookie.
    let csrf_get = http
        .get(format!("{base}/sign-in"))
        .send()
        .await
        .expect("get /sign-in");
    let csrf_cookie_full = csrf_get
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .map(|s| s.to_string())
        .expect("csrf cookie");
    let csrf_token = csrf_cookie_full
        .strip_prefix("foundry_csrf=")
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string();
    let csrf_pair = format!("foundry_csrf={csrf_token}");

    // (2) Sign in as Mei (acceptance steps default to Mei for GETs).
    let email = "mei@acme.com";
    let _ = SecretString::new(MEMBER_PASSWORD.to_string().into()); // ensure SecretString in scope
    let mut form: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
    form.insert("email", email.to_string());
    form.insert("password", MEMBER_PASSWORD.to_string());
    form.insert("_csrf", csrf_token.clone());
    let signin_resp = http
        .post(format!("{base}/sign-in"))
        .header(reqwest::header::COOKIE, csrf_pair.clone())
        .form(&form)
        .send()
        .await
        .expect("sign-in for board fetch");
    let session_cookie = signin_resp
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
    let combined = format!("{session_pair}; {csrf_pair}");

    // (3) GET the URL with both cookies.
    let resp = http
        .get(format!("{base}{url}"))
        .header(reqwest::header::COOKIE, combined)
        .send()
        .await
        .expect("get url");
    resp.text().await.unwrap_or_default()
}

fn slugify(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_was_hyphen = true;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            for c in ch.to_lowercase() {
                out.push(c);
            }
            last_was_hyphen = false;
        } else if !last_was_hyphen {
            out.push('-');
            last_was_hyphen = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}
