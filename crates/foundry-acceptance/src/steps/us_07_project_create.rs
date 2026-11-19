//! US-07 step definitions — project create + view + invariant outline.
//!
//! Scenarios drive HTTP through the in-process axum harness reused from
//! US-05 / US-06. The Background creates a workspace + a "Backend" team
//! with member "mei@acme.com"; the user is signed in via the shared
//! `signed_in_post` helper on each scenario's first `When`.
//!
//! The shared `Then` phrases ("the response status is 403 Forbidden",
//! "the response body contains ...") are intentionally NOT redeclared
//! here — cucumber-rs requires globally-unique step phrases. They are
//! reused from `us_05_bootstrap.rs` (status_409 lives there) and
//! `us_06_signin.rs` (body_contains).

use crate::support::harness::{signed_in_post, InProcHarness, PostOutcome};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use secrecy::SecretString;

const TEST_NOW: &str = "2026-01-15T12:00:00Z";
const MEMBER_PASSWORD: &str = "mei-correct-horse-battery-staple";
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

/// Insert a workspace + admin + named team + team-membership for the
/// admin. Returns `(workspace_id, team_id, admin_user_id)`.
async fn seed_workspace_with_team(
    harness: &InProcHarness,
    workspace_name: &str,
    admin_email: &str,
    team_name: &str,
    team_slug: &str,
) -> (uuid::Uuid, uuid::Uuid, uuid::Uuid) {
    let pool = harness.app.state.store.pool();
    let workspace_id = uuid::Uuid::now_v7();
    let admin_id = uuid::Uuid::now_v7();
    let team_id = uuid::Uuid::now_v7();
    let admin_lower = admin_email.to_ascii_lowercase();
    let admin_hash = foundry_auth::hash_password(&SecretString::new(
        "admin-correct-horse-battery-staple".to_string().into(),
    ))
    .await
    .expect("hash admin pw");
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(workspace_id)
        .bind(workspace_name)
        .execute(pool)
        .await
        .expect("insert workspace");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(admin_id)
    .bind(&admin_lower)
    .bind(admin_email)
    .bind("Admin")
    .bind(&admin_hash)
    .execute(pool)
    .await
    .expect("insert admin user");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'admin')",
    )
    .bind(workspace_id)
    .bind(admin_id)
    .execute(pool)
    .await
    .expect("insert admin membership");
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)")
        .bind(team_id)
        .bind(workspace_id)
        .bind(team_name)
        .bind(team_slug)
        .execute(pool)
        .await
        .expect("insert team");
    sqlx::query("INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'lead')")
        .bind(team_id)
        .bind(admin_id)
        .execute(pool)
        .await
        .expect("insert admin team membership");
    (workspace_id, team_id, admin_id)
}

async fn insert_user_with_team_membership(
    harness: &InProcHarness,
    workspace_id: uuid::Uuid,
    team_id: Option<uuid::Uuid>,
    email: &str,
    display: &str,
    password: &str,
) -> uuid::Uuid {
    let pool = harness.app.state.store.pool();
    let user_id = uuid::Uuid::now_v7();
    let lower = email.to_ascii_lowercase();
    let hash = foundry_auth::hash_password(&SecretString::new(password.to_string().into()))
        .await
        .expect("hash user pw");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(&lower)
    .bind(email)
    .bind(display)
    .bind(&hash)
    .execute(pool)
    .await
    .expect("insert user");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role)
              VALUES ($1, $2, 'member')
              ON CONFLICT DO NOTHING",
    )
    .bind(workspace_id)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("insert workspace membership");
    if let Some(tid) = team_id {
        sqlx::query(
            "INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'member')
                  ON CONFLICT DO NOTHING",
        )
        .bind(tid)
        .bind(user_id)
        .execute(pool)
        .await
        .expect("insert team membership");
    }
    user_id
}

// ----- Background --------------------------------------------------------

// "a workspace 'Acme Eng' exists with admin 'devansh@acme.com'" is the
// US-06 phrase. It already seeds the workspace + admin row. We extend
// the slice-1 fixture here by creating the "Backend" team in the
// "member belongs to team" step below.

#[given(regex = r#"^a member "([^"]+)" belongs to the team "([^"]+)"$"#)]
async fn member_belongs_to_team(world: &mut FoundryWorld, email: String, team_name: String) {
    // Reset per-scenario state. The US-06 workspace_with_admin background
    // step runs BEFORE this one and creates the workspace; we just need
    // to add the named team and the user → team membership.
    if world.harness.is_none() {
        // Some scenarios (the property outline) don't include the
        // "workspace exists with admin" Background line — bootstrap a
        // minimal workspace ourselves.
        world.us_06_unknown_latencies_ms.clear();
        world.us_06_wrong_pw_latencies_ms.clear();
        world.session_cookie_header = None;
        world.last_status = None;
        world.last_body = None;
        world.last_headers = None;
        ensure_harness(world).await;
        let harness = world.harness.as_ref().expect("harness");
        seed_workspace_with_team(
            harness,
            "Acme Eng",
            "devansh@acme.com",
            &team_name,
            &slugify(&team_name),
        )
        .await;
    } else {
        // Reuse the workspace seeded by US-06's workspace_with_admin
        // step; add the named team.
        let harness = world.harness.as_ref().expect("harness");
        let pool = harness.app.state.store.pool();
        let ws_id: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM workspaces LIMIT 1")
            .fetch_one(pool)
            .await
            .expect("fetch workspace");
        let team_slug = slugify(&team_name);
        // Idempotent — admin might re-run the line in a re-run scenario.
        let team_id = uuid::Uuid::now_v7();
        sqlx::query(
            "INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)
                  ON CONFLICT (workspace_id, slug) DO NOTHING",
        )
        .bind(team_id)
        .bind(ws_id.0)
        .bind(&team_name)
        .bind(&team_slug)
        .execute(pool)
        .await
        .expect("insert team");
    }

    // Now insert the named member + team membership.
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let ws_id: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM workspaces LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("fetch workspace");
    let team_id: (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM teams WHERE workspace_id = $1 AND name = $2")
            .bind(ws_id.0)
            .bind(&team_name)
            .fetch_one(pool)
            .await
            .expect("fetch team");
    insert_user_with_team_membership(
        harness,
        ws_id.0,
        Some(team_id.0),
        &email,
        "Member",
        MEMBER_PASSWORD,
    )
    .await;
    world.us_07_signed_in_email = None;
    world.us_07_signed_in_password = None;
}

#[given(regex = r"^(\w+) is signed in$")]
async fn user_signed_in(world: &mut FoundryWorld, who: String) {
    ensure_harness(world).await;
    // Map first-name "Mei" → mei@acme.com (the only member registered in
    // the Background). Other personas (Hiroshi) get their own clause.
    let (email, password) = identity_for(&who);
    world.us_07_signed_in_email = Some(email);
    world.us_07_signed_in_password = Some(password);
}

// ----- Happy path --------------------------------------------------------

#[when(
    regex = r#"^(\w+) creates a project under "([^"]+)" with name "([^"]+)" and key prefix "([^"]*)"$"#
)]
async fn create_project(
    world: &mut FoundryWorld,
    who: String,
    team_name: String,
    project_name: String,
    key: String,
) {
    perform_create(world, &who, &team_name, &project_name, &key).await;
}

#[then(regex = r#"^the response redirects to "([^"]+)"$"#)]
async fn response_redirects_url(world: &mut FoundryWorld, expected: String) {
    let status = world.last_status.expect("status captured");
    assert!(
        status == StatusCode::SEE_OTHER || status == StatusCode::FOUND,
        "expected 303/302 redirect, got {status}"
    );
    let headers = world.last_headers.as_ref().expect("headers captured");
    let location = headers
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        location, expected,
        "expected Location {expected:?}, got {location:?}"
    );
}

// board-lane-management 02-01: a freshly created project seeds the THREE
// defaults (D4) — the walking-skeleton assertion narrows from four columns.
#[then(regex = r#"^the response body lists the columns "([^"]+)", "([^"]+)", "([^"]+)"$"#)]
async fn body_lists_columns(world: &mut FoundryWorld, c1: String, c2: String, c3: String) {
    let body = fetch_redirected_board_body(world).await;
    for col in [&c1, &c2, &c3] {
        assert!(
            body.contains(col.as_str()),
            "board body missing column {col:?}: {body}"
        );
    }
    // Cache the board body so subsequent shared steps (e.g. the
    // `body_contains` from us_06_signin.rs asserting "New issue") see
    // the board content rather than the empty 303 redirect body.
    world.last_body = Some(body);
}

#[then(
    regex = r#"^the project "([^"]+)" is recorded in the "([^"]+)" team with key prefix "([^"]+)"$"#
)]
async fn project_recorded(world: &mut FoundryWorld, name: String, team: String, key: String) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let row: (String,) = sqlx::query_as(
        "SELECT p.key_prefix FROM projects p
           JOIN teams t ON t.id = p.team_id
          WHERE p.name = $1 AND t.name = $2",
    )
    .bind(&name)
    .bind(&team)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|err| panic!("project {name:?} in team {team:?} not found: {err}"));
    assert_eq!(
        row.0,
        key,
        "project {name:?} has key {actual:?}, expected {key:?}",
        actual = row.0
    );
}

// ----- Duplicate-key & duplicate-name ------------------------------------

#[given(
    regex = r#"^a project named "([^"]+)" with key prefix "([^"]+)" already exists in "([^"]+)"$"#
)]
async fn project_already_exists(
    world: &mut FoundryWorld,
    name: String,
    key: String,
    team_name: String,
) {
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
    let slug = slugify(&name);
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(project_id)
    .bind(team_row.0)
    .bind(ws_id.0)
    .bind(&name)
    .bind(&slug)
    .bind(&key)
    .execute(pool)
    .await
    .expect("seed existing project");
    // board-lane-management sweep: raw-SQL projects need their lane rows.
    crate::support::harness::seed_lanes_for_project(pool, project_id).await;
}

#[when(
    regex = r#"^(\w+) attempts to create a project under "([^"]+)" with name "([^"]+)" and key prefix "([^"]*)"$"#
)]
async fn attempt_create_project(
    world: &mut FoundryWorld,
    who: String,
    team_name: String,
    project_name: String,
    key: String,
) {
    perform_create(world, &who, &team_name, &project_name, &key).await;
}

#[then(regex = r"^the response body explains the project key is already in use$")]
async fn body_explains_dup_key(world: &mut FoundryWorld) {
    let body = world.last_body.as_deref().unwrap_or("");
    let lower = body.to_ascii_lowercase();
    assert!(
        lower.contains("project key") && lower.contains("already"),
        "body did not explain duplicate key: {body}"
    );
}

#[then(
    regex = r"^the response shows an inline error explaining the name must be unique within the team$"
)]
async fn body_explains_dup_name(world: &mut FoundryWorld) {
    let body = world.last_body.as_deref().unwrap_or("");
    let lower = body.to_ascii_lowercase();
    assert!(
        lower.contains("name") && lower.contains("unique"),
        "body did not explain duplicate name: {body}"
    );
}

#[then(regex = r"^no second project is created$")]
async fn no_second_project(world: &mut FoundryWorld) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let count: (i64,) = sqlx::query_as("SELECT count(*) FROM projects WHERE name = $1")
        .bind(world.us_07_last_attempted_name.as_deref().unwrap_or(""))
        .fetch_one(pool)
        .await
        .expect("count projects");
    assert!(
        count.0 <= 1,
        "expected at most one project named {name:?}, got {n}",
        name = world.us_07_last_attempted_name.as_deref().unwrap_or(""),
        n = count.0
    );
}

// ----- Non-team-member ---------------------------------------------------

#[given(regex = r#"^(\w+) is a workspace member but not a member of the "([^"]+)" team$"#)]
async fn non_team_member(world: &mut FoundryWorld, who: String, _team_name: String) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let ws_id: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM workspaces LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("fetch workspace");
    let (email, _) = identity_for(&who);
    // Use the shared MEMBER_PASSWORD so `identity_for` (which is
    // single-password since slice-2 unified persona credentials)
    // can sign Hiroshi in. The HIROSHI_PASSWORD constant remains as
    // documentation of the original per-persona convention.
    let _ = HIROSHI_PASSWORD;
    insert_user_with_team_membership(harness, ws_id.0, None, &email, &who, MEMBER_PASSWORD).await;
}

#[then(regex = r"^the response status is 403 Forbidden$")]
async fn status_403(world: &mut FoundryWorld) {
    let status = world.last_status.expect("status captured");
    assert_eq!(status.as_u16(), 403, "expected 403, got {status}");
}

/// Sibling of `status_403` for the authz refusals that converged on the uniform
/// non-enumerable 404 (ADR-003). An authz refusal must not confirm that the
/// resource it is refusing exists, so the HTML adapter answers the same 404 a
/// never-existed team/project would. The 403 phrase above is retained for the
/// refusals that legitimately stay 403 — CSRF double-submit failures and the
/// non-author (not non-member) comment-edit refusal — which are a DIFFERENT
/// refusal from an authz-membership one.
#[then(regex = r"^the response status is 404 Not Found$")]
async fn status_404(world: &mut FoundryWorld) {
    let status = world.last_status.expect("status captured");
    assert_eq!(status.as_u16(), 404, "expected 404, got {status}");
}

#[then(regex = r#"^no project named "([^"]+)" exists in any team$"#)]
async fn no_project_named_anywhere(world: &mut FoundryWorld, name: String) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let count: (i64,) = sqlx::query_as("SELECT count(*) FROM projects WHERE name = $1")
        .bind(&name)
        .fetch_one(pool)
        .await
        .expect("count projects");
    assert_eq!(
        count.0, 0,
        "expected zero projects named {name:?}, got {}",
        count.0
    );
}

// ----- Property outline --------------------------------------------------

#[then(regex = r#"^the project-create outcome is "(accepted|rejected)"$"#)]
async fn project_create_outcome(world: &mut FoundryWorld, outcome: String) {
    let status = world.last_status.expect("status captured");
    match outcome.as_str() {
        "accepted" => {
            assert!(
                matches!(status.as_u16(), 302 | 303),
                "expected redirect on accepted key, got {status} body={body}",
                body = world.last_body.as_deref().unwrap_or("")
            );
        }
        "rejected" => {
            assert!(
                !matches!(status.as_u16(), 200..=299 | 302 | 303),
                "expected 4xx on rejected key, got {status}"
            );
        }
        other => panic!("unrecognised outcome {other:?}"),
    }
}

// ----- internals ---------------------------------------------------------

fn identity_for(who: &str) -> (String, String) {
    // All shared scenarios that seed users do so via the same step
    // (`member_belongs_to_team`), which inserts every user with
    // MEMBER_PASSWORD. Slice 2 added "Rita" as a Partners-team
    // persona; her credential follows the same convention.
    //
    // Slice 5 added "Devansh" — the workspace admin seeded by the
    // US-06 `workspace_with_admin` background step. Devansh's password
    // is the admin-only `admin-correct-horse-battery-staple` literal
    // hashed by `seed_workspace_with_team` above (line ~63). Keeping
    // this mapping co-located with the other personas so the globally-
    // unique `(\w+) is signed in` step resolves cleanly.
    let _ = HIROSHI_PASSWORD; // kept for future "different password per persona" scenarios
    match who {
        "Mei" => ("mei@acme.com".to_string(), MEMBER_PASSWORD.to_string()),
        "Hiroshi" => ("hiroshi@acme.com".to_string(), MEMBER_PASSWORD.to_string()),
        "Rita" => (
            "rita@partners.acme.com".to_string(),
            MEMBER_PASSWORD.to_string(),
        ),
        "Devansh" => (
            "devansh@acme.com".to_string(),
            // The Background workspace_with_admin step (us_06_signin.rs
            // line 69) seeds Devansh with `admin-password-from-bootstrap`.
            // Slice-3 US-03 uses the same literal as `ADMIN_PASSWORD`
            // (us_03_backup_restore.rs line 57). Keep in sync if either
            // moves.
            "admin-password-from-bootstrap".to_string(),
        ),
        "Ada" => (
            "ada@acme.com".to_string(),
            // dashboard-enhancements US-01: the Background `a workspace "Acme"
            // exists with admin "Ada" and display name "Ada Lovelace"` step
            // (feature_dashboard_enhancements.rs) seeds Ada with the
            // `ADA_PASSWORD` literal below. Keep in sync if either moves.
            "ada-correct-horse-battery-staple".to_string(),
        ),
        other => panic!("no identity registered for {other:?}"),
    }
}

async fn perform_create(
    world: &mut FoundryWorld,
    who: &str,
    team_name: &str,
    project_name: &str,
    key: &str,
) {
    ensure_harness(world).await;
    let (email, password) = identity_for(who);
    let team_slug = slugify(team_name);
    let url = format!("/team/{team_slug}/projects");
    world.us_07_last_attempted_name = Some(project_name.to_string());
    world.us_07_last_team_slug = Some(team_slug.clone());

    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let outcome: PostOutcome = signed_in_post(
        harness,
        http,
        &email,
        &password,
        &url,
        &[("name", project_name), ("key_prefix", key)],
    )
    .await;
    let _ = email;
    let _ = password;
    capture_outcome(world, outcome);
}

fn capture_outcome(world: &mut FoundryWorld, outcome: PostOutcome) {
    world.last_status = Some(outcome.status);
    world.last_headers = Some(outcome.headers);
    world.last_body = Some(outcome.body);
}

/// After a successful redirect from the project-create POST, follow the
/// Location header (with a fresh sign-in) to fetch the empty board view.
/// We need the board body because the redirect itself has no payload —
/// the columns live on the destination page.
async fn fetch_redirected_board_body(world: &mut FoundryWorld) -> String {
    let headers = world.last_headers.as_ref().expect("headers captured");
    let location = headers
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .expect("Location header present")
        .to_string();
    let (email, password) = (
        world
            .us_07_signed_in_email
            .clone()
            .unwrap_or_else(|| "mei@acme.com".into()),
        world
            .us_07_signed_in_password
            .clone()
            .unwrap_or_else(|| MEMBER_PASSWORD.into()),
    );
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    // signed_in_post does a fresh sign-in cycle; we want a GET, so build
    // an authenticated GET inline.
    let base = harness.base_url();
    // (1) GET /sign-in for CSRF cookie.
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
    // (2) POST /sign-in with this CSRF to mint a session cookie.
    let mut form: std::collections::HashMap<&str, String> = std::collections::HashMap::new();
    form.insert("email", email.clone());
    form.insert("password", password.clone());
    form.insert("_csrf", csrf_token.clone());
    let signin_resp = http
        .post(format!("{base}/sign-in"))
        .header(reqwest::header::COOKIE, csrf_pair.clone())
        .form(&form)
        .send()
        .await
        .expect("sign-in for board view");
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
    // (3) GET the board page with both cookies.
    let board_resp = http
        .get(format!("{base}{location}"))
        .header(reqwest::header::COOKIE, combined)
        .send()
        .await
        .expect("get board view");
    board_resp.text().await.unwrap_or_default()
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

// ----- The project key rule: 2 to 10 capital letters A to Z --------------
//
// Steps for law L1 (which keys the form accepts), L5 (what a refused key is
// answered with, and that it leaves nothing behind) and the project-key half
// of L6 (a project that carries a shorter key keeps it). The issue-key halves
// of L4 and L6 live with the file-issue steps.

/// The one sentence a refused key is answered with. The operator must be able
/// to read the whole rule out of it: the subject, both ends of the length and
/// the capital-letters-A-to-Z shape — and never the old six-letter end.
#[then(regex = r"^the refusal states that a project key must be 2 to 10 capital letters A to Z$")]
async fn refusal_states_the_key_rule(world: &mut FoundryWorld) {
    let status = world.last_status.expect("status captured");
    assert_eq!(
        status.as_u16(),
        422,
        "WHAT: the create answered {status} instead of 422 for a key outside the rule. \
         WHY: a key the rule refuses must come back to the operator as a refusal of the form, \
         not as a success or a server error. HOW: answer the create form again with 422."
    );
    let body = world.last_body.as_deref().unwrap_or("");
    let message = rendered_error_message(body).unwrap_or_else(|| {
        panic!(
            "WHAT: the refused create rendered no message for the operator to read. \
             WHY: every refused key must be answered with one sentence that states the whole \
             key rule. HOW: re-render the form with that sentence. Body was:\n{body}"
        )
    });
    let said = message.to_ascii_lowercase();
    let states_length = said.contains("2 to 10") || said.contains("2-10");
    let states_capitals = said.contains("capital letter") || said.contains("uppercase");
    let states_alphabet = said.contains("a to z") || said.contains("a-z");
    let names_subject = said.contains("project key") || said.contains("key prefix");
    let keeps_old_bound = said.contains("2 to 6") || said.contains("2-6");
    assert!(
        names_subject && states_length && states_capitals && states_alphabet && !keeps_old_bound,
        "WHAT: the refusal read {message:?}, which does not state that a project key must be \
         2 to 10 capital letters A to Z (subject={names_subject}, length={states_length}, \
         capitals={states_capitals}, alphabet={states_alphabet}, still states the old 2 to 6 \
         bound={keeps_old_bound}). WHY: the operator has to learn the whole rule from the one \
         sentence she is given, whichever part of the rule her key broke. HOW: answer every \
         refused key with one sentence naming 2 to 10 capital letters A to Z."
    );
}

/// A refusal leaves nothing behind — not the key the operator typed, and not a
/// quietly corrected version of it (upper-cased, or with the characters the
/// rule refuses dropped).
#[then(regex = r#"^no project was created from the key "([^"]*)", in that form or tidied up$"#)]
async fn no_project_from_key(world: &mut FoundryWorld, key: String) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let tidied: String = key
        .to_uppercase()
        .chars()
        .filter(|c| c.is_ascii_uppercase())
        .collect();
    let mut candidates = vec![key.clone(), key.trim().to_string(), key.to_uppercase()];
    if !tidied.is_empty() {
        candidates.push(tidied);
    }
    candidates.sort();
    candidates.dedup();
    for candidate in &candidates {
        let count: (i64,) = sqlx::query_as("SELECT count(*) FROM projects WHERE key_prefix = $1")
            .bind(candidate)
            .fetch_one(pool)
            .await
            .expect("count projects by key");
        assert_eq!(
            count.0, 0,
            "WHAT: a project carrying the key {candidate:?} exists after the refused create of \
             {key:?}. WHY: a refused key must create nothing, and must not be corrected into an \
             acceptable key behind the operator's back. HOW: refuse the key and write no row."
        );
    }
    let attempted = world
        .us_07_last_attempted_name
        .clone()
        .unwrap_or_else(|| "Probe".to_string());
    let by_name: (i64,) = sqlx::query_as("SELECT count(*) FROM projects WHERE name = $1")
        .bind(&attempted)
        .fetch_one(pool)
        .await
        .expect("count projects by name");
    assert_eq!(
        by_name.0, 0,
        "WHAT: a project named {attempted:?} exists after the create was refused for its key. \
         WHY: a refused create must leave no project behind at all. HOW: write no row when the \
         key is refused."
    );
}

#[when(regex = r#"^(\w+) opens the board of "([^"]+)"$"#)]
async fn opens_the_board(world: &mut FoundryWorld, who: String, project_name: String) {
    let body = fetch_board_of(world, &who, &project_name).await;
    world.last_body = Some(body);
}

#[then(regex = r#"^the board she opened shows the project key "([^"]+)"$"#)]
async fn opened_board_shows_key(world: &mut FoundryWorld, key: String) {
    let body = world.last_body.clone().expect("a board was opened");
    assert_board_shows_key(&body, &key);
}

#[then(regex = r#"^opening the board of "([^"]+)" shows the project key "([^"]+)"$"#)]
async fn board_shows_key(world: &mut FoundryWorld, project_name: String, key: String) {
    let body = fetch_board_of(world, "Mei", &project_name).await;
    assert_board_shows_key(&body, &key);
}

#[then(regex = r#"^the project list on her landing page shows the project key "([^"]+)"$"#)]
async fn landing_page_shows_key(world: &mut FoundryWorld, key: String) {
    let body = fetch_page_as(world, "Mei", "/").await;
    let shown = card_keys(&body);
    assert!(
        shown.contains(&key),
        "WHAT: the project list shows the keys {shown:?}, not {key:?}. WHY: the key the operator \
         gave her project must come back to her whole wherever the project is listed, not \
         shortened or re-lettered. HOW: render the stored key as it is."
    );
}

fn assert_board_shows_key(body: &str, key: &str) {
    let shown = board_key(body).unwrap_or_else(|| {
        panic!(
            "WHAT: the board shows no project key. WHY: the operator reads her project's key \
                off its board. HOW: render the key in the board header. Body was:\n{body}"
        )
    });
    assert_eq!(
        shown.trim(),
        key,
        "WHAT: the board shows the key {shown:?} where the project carries {key:?}. WHY: a key \
         that is shortened, padded or re-lettered on the page is a different key to the operator \
         and to anyone she sends it to. HOW: render the stored key unchanged."
    );
}

/// Text of the board header's key, from `Key prefix: AUTH`.
fn board_key(body: &str) -> Option<String> {
    let marker = "Key prefix:";
    let start = body.find(marker)? + marker.len();
    let rest = &body[start..];
    let end = rest.find('<')?;
    Some(rest[..end].trim().to_string())
}

/// Text of each project-card key on the landing page.
fn card_keys(body: &str) -> Vec<String> {
    let marker = "card__key\">";
    let mut out = Vec::new();
    let mut rest = body;
    while let Some(at) = rest.find(marker) {
        rest = &rest[at + marker.len()..];
        if let Some(end) = rest.find('<') {
            out.push(rest[..end].trim().to_string());
            rest = &rest[end..];
        }
    }
    out
}

/// The single `<p class="error">...</p>` message a refused form renders.
fn rendered_error_message(body: &str) -> Option<String> {
    let anchor = body.find("class=\"error\"")?;
    let rest = &body[anchor..];
    let open = rest.find('>')? + 1;
    let rest = &rest[open..];
    let end = rest.find('<')?;
    let text = rest[..end].trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

async fn fetch_board_of(world: &mut FoundryWorld, who: &str, project_name: &str) -> String {
    let team_slug = world
        .us_07_last_team_slug
        .clone()
        .unwrap_or_else(|| "backend".to_string());
    let url = format!(
        "/team/{team_slug}/project/{slug}",
        slug = slugify(project_name)
    );
    fetch_page_as(world, who, &url).await
}

async fn fetch_page_as(world: &mut FoundryWorld, who: &str, url: &str) -> String {
    ensure_harness(world).await;
    let (email, password) = identity_for(who);
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let outcome =
        crate::support::harness::signed_in_get(harness, http, &email, &password, url).await;
    assert_eq!(
        outcome.status.as_u16(),
        200,
        "WHAT: {url} answered {status} instead of opening. WHY: the operator must be able to \
         open her project and her project list. HOW: serve the page.",
        status = outcome.status
    );
    outcome.body
}

/// The operator's own earlier create, kept as a precondition of a later
/// scenario (the chained narrative: the Given of one scenario is the Given
/// plus When of the one before it). It posts the create form exactly as the
/// When step does and asserts nothing, so the scenario's own Then steps are
/// what report a key the form will not take.
#[given(
    regex = r#"^(\w+) has created a project under "([^"]+)" with name "([^"]+)" and key prefix "([^"]*)"$"#
)]
async fn has_created_project(
    world: &mut FoundryWorld,
    who: String,
    team_name: String,
    project_name: String,
    key: String,
) {
    perform_create(world, &who, &team_name, &project_name, &key).await;
}
