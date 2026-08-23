//! instance-admin-project-rename step definitions
//! (`tests/features/instance-admin-project-rename.feature`, 21 scenarios).
//!
//! The production seams these steps drive are the DESIGN port signatures
//! (`foundry_core::slugify`, the four `foundry-store` queries,
//! `foundry_services::projects::rename_project`, and the mounted
//! `instance_admin::submit_project_rename` handler).
//!
//! THE SLUG-CAPTURE RULE (D2 / ADR-PROJECT-RENAME-001): every board/report URL
//! asserted after a rename is built from the STORED slugs, read back from the
//! database BEFORE the rename (`note_where_board_lives`, `stored_slugs_of`).
//! This module deliberately has NO test-local `fn slugify` — re-deriving the
//! slug from the NEW name would assert the wrong URL and go green over the
//! exact render-time re-derivation defect the D2 fix removes. The ~20
//! test-local slugify copies elsewhere in this crate are creation-time-only
//! and unaffected.
//!
//! LAYER 3 (real adapter + real HTTP, @real-io): real Postgres via the shared
//! testcontainer + per-scenario schema; the real tower-sessions store; the real
//! double-submit CSRF middleware; the in-process axum router. Example-based
//! (Mandates 9 + 11) — no PBT machinery at this layer. State-mutation
//! assertions follow the state-delta discipline via [`assert_project_delta`]:
//! snapshot the full projects row before the write, snapshot after, and assert
//! the declared universe — (name, slug, key_prefix, next_issue_number) — where
//! ONLY `name` may move; anything else moving fails closed.
//!
//! The two `@needs-browser` scenarios drive a REAL headless Chrome (fantoccini,
//! `support::browser_harness`) because the HTTP lane is byte-blind to the htmx
//! row swap and to `form-errors.js` routing the 422 fragment into the
//! submitting row's `[data-error-slot]` (the form-errors RCA).

use crate::support::browser_harness;
use crate::support::harness::{
    establish_session, post_with_cookie, signed_in_get, signed_in_post, InProcHarness, PostOutcome,
};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use fantoccini::Locator;
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use secrecy::SecretString;
use sqlx::PgPool;
use std::time::Duration;

const TEST_NOW: &str = "2026-01-15T12:00:00Z";
const PRIYA_EMAIL: &str = "priya@canzan.test";
const PRIYA_PASSWORD: &str = "priya-correct-horse-battery-staple";
const MARCO_EMAIL: &str = "marco@canzan.test";
const MARCO_PASSWORD: &str = "marco-correct-horse-battery-staple";

/// DESIGN-pinned seams (component-boundaries.md). If DELIVER moves these, the
/// row partial and this module move in the same change.
fn rename_url(project_id: &str) -> String {
    format!("/admin/instance/projects/{project_id}/rename")
}
const DASHBOARD_PATH: &str = "/admin/instance/workspaces";
const ERROR_MARKER: &str = "project-rename-error";
const ROW_MARKER: &str = "data-project-row";

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
        world.harness = Some(InProcHarness::spawn(now_anchor()).await);
    }
    if world.http.is_none() {
        world.http = Some(client());
    }
}

fn harness(world: &FoundryWorld) -> &InProcHarness {
    world.harness.as_ref().expect("harness spawned by a Given")
}

fn pool(world: &FoundryWorld) -> PgPool {
    harness(world).app.state.store.pool().clone()
}

async fn record_outcome(world: &mut FoundryWorld, outcome: PostOutcome) {
    world.last_status = Some(outcome.status);
    world.last_headers = Some(outcome.headers);
    world.last_body = Some(outcome.body);
}

/// A full `projects`-row snapshot: the declared observable universe for a
/// rename — `(name, slug, key_prefix, next_issue_number)`.
type ProjectSnapshot = (String, String, String, i32);

async fn snapshot_project(world: &FoundryWorld, project_id: uuid::Uuid) -> ProjectSnapshot {
    sqlx::query_as("SELECT name, slug, key_prefix, next_issue_number FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_one(&pool(world))
        .await
        .expect("snapshot project row")
}

/// State-delta over the project universe: ONLY `name` is allowed to move (to
/// `expected_name`); `slug`, `key_prefix`, and `next_issue_number` must be
/// byte-identical to the pre-write snapshot. Fail-closed: any undeclared drift
/// in the declared universe is a violation.
fn assert_project_delta(before: &ProjectSnapshot, after: &ProjectSnapshot, expected_name: &str) {
    assert_eq!(
        after.0, expected_name,
        "projects.name must be {expected_name:?}; got {:?}",
        after.0
    );
    assert_eq!(
        after.1, before.1,
        "projects.slug must be byte-identical across a rename (D1); {:?} -> {:?}",
        before.1, after.1
    );
    assert_eq!(
        after.2, before.2,
        "projects.key_prefix must be byte-identical across a rename (D1); {:?} -> {:?}",
        before.2, after.2
    );
    assert_eq!(
        after.3, before.3,
        "projects.next_issue_number must not move on a rename; {} -> {}",
        before.3, after.3
    );
}

fn project_id_of(world: &FoundryWorld, seeded_name: &str) -> uuid::Uuid {
    *world
        .iapr_project_ids
        .get(seeded_name)
        .unwrap_or_else(|| panic!("project {seeded_name:?} must be seeded by the Background"))
}

/// The STORED `(team_slug, project_slug)` pair captured from the database at
/// seed time — never re-derived from a (possibly renamed) name.
fn stored_slugs_of(world: &FoundryWorld, seeded_name: &str) -> (String, String) {
    world
        .iapr_stored_slugs
        .get(seeded_name)
        .unwrap_or_else(|| panic!("stored slugs for {seeded_name:?} must have been captured"))
        .clone()
}

async fn seed_user(world: &FoundryWorld, email: &str, display: &str, password: &str) -> uuid::Uuid {
    let pool = pool(world);
    let email_lower = email.to_ascii_lowercase();
    let hash = foundry_auth::hash_password(&SecretString::new(password.to_string().into()))
        .await
        .expect("hash password");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5) ON CONFLICT (email_lower) DO NOTHING",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(&email_lower)
    .bind(email)
    .bind(display)
    .bind(&hash)
    .execute(&pool)
    .await
    .expect("insert user");
    let (id,): (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(&email_lower)
        .fetch_one(&pool)
        .await
        .expect("resolve user id");
    id
}

async fn seed_membership(
    world: &FoundryWorld,
    workspace_id: uuid::Uuid,
    user_id: uuid::Uuid,
    role: &str,
) {
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role)
              VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(workspace_id)
    .bind(user_id)
    .bind(role)
    .execute(&pool(world))
    .await
    .expect("insert membership");
}

async fn seed_workspace(world: &mut FoundryWorld, name: &str) -> uuid::Uuid {
    if let Some(id) = world.iapr_workspace_ids.get(name) {
        return *id;
    }
    let id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(id)
        .bind(name)
        .execute(&pool(world))
        .await
        .expect("insert workspace");
    world.iapr_workspace_ids.insert(name.to_string(), id);
    id
}

async fn seed_team(
    world: &mut FoundryWorld,
    workspace_id: uuid::Uuid,
    name: &str,
    slug: &str,
) -> uuid::Uuid {
    if let Some(id) = world.iapr_team_ids.get(name) {
        return *id;
    }
    let id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)")
        .bind(id)
        .bind(workspace_id)
        .bind(name)
        .bind(slug)
        .execute(&pool(world))
        .await
        .expect("insert team");
    world.iapr_team_ids.insert(name.to_string(), id);
    // Board/report reads re-validate TEAM membership (`Store::is_team_member`
    // via `board::list_board_issues`), so the persona who verifies the
    // board-survival and name-propagation oracles must belong to the team.
    // This is precondition seeding for the READ oracles, not the behaviour
    // under test (the rename itself is instance-admin-gated, not team-gated).
    let priya = world.iapr_priya_id.expect("Priya seeded first");
    sqlx::query(
        "INSERT INTO team_memberships (team_id, user_id, role)
              VALUES ($1, $2, 'lead') ON CONFLICT DO NOTHING",
    )
    .bind(id)
    .bind(priya)
    .execute(&pool(world))
    .await
    .expect("insert Priya's team membership");
    id
}

/// Seed one project and CAPTURE its stored `(team_slug, project_slug)` by
/// reading the rows back — the slug-capture rule (module header).
async fn seed_project(
    world: &mut FoundryWorld,
    workspace_id: uuid::Uuid,
    team_id: uuid::Uuid,
    name: &str,
    slug: &str,
    key_prefix: &str,
) {
    let id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(id)
    .bind(team_id)
    .bind(workspace_id)
    .bind(name)
    .bind(slug)
    .bind(key_prefix)
    .execute(&pool(world))
    .await
    .expect("insert project");
    world.iapr_project_ids.insert(name.to_string(), id);
    let (stored_project_slug, stored_team_slug): (String, String) = sqlx::query_as(
        "SELECT p.slug, t.slug FROM projects p JOIN teams t ON p.team_id = t.id WHERE p.id = $1",
    )
    .bind(id)
    .fetch_one(&pool(world))
    .await
    .expect("read back stored slugs");
    world
        .iapr_stored_slugs
        .insert(name.to_string(), (stored_team_slug, stored_project_slug));
}

/// Kebab-case the SEED name once, at creation time only (the intended
/// derivation point) — mirrors what the production create path mints. This is
/// input to the INSERT, never an assertion oracle; assertions use the
/// read-back stored slugs.
fn creation_slug(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

// ===========================================================================
// Background
// ===========================================================================

#[given(regex = r"^Priya is the instance super-admin$")]
async fn priya_is_instance_super_admin(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    let priya = seed_user(world, PRIYA_EMAIL, "Priya Raman", PRIYA_PASSWORD).await;
    sqlx::query("INSERT INTO instance_admins (user_id) VALUES ($1) ON CONFLICT DO NOTHING")
        .bind(priya)
        .execute(&pool(world))
        .await
        .expect("grant instance admin");
    world.iapr_priya_id = Some(priya);
}

#[given(
    regex = r#"^workspace "([^"]+)" has a team "([^"]+)" with projects "([^"]+)" \(([A-Z]+)\) and "([^"]+)" \(([A-Z]+)\)$"#
)]
async fn workspace_with_two_projects(
    world: &mut FoundryWorld,
    ws_name: String,
    team_name: String,
    p1_name: String,
    p1_prefix: String,
    p2_name: String,
    p2_prefix: String,
) {
    let ws = seed_workspace(world, &ws_name).await;
    let team = seed_team(world, ws, &team_name, &creation_slug(&team_name)).await;
    seed_project(
        world,
        ws,
        team,
        &p1_name,
        &creation_slug(&p1_name),
        &p1_prefix,
    )
    .await;
    seed_project(
        world,
        ws,
        team,
        &p2_name,
        &creation_slug(&p2_name),
        &p2_prefix,
    )
    .await;
    // Priya's sign-in resolves an active workspace from her memberships; anchor
    // her to the first seeded workspace (the bootstrap-super-admin shape).
    let priya = world.iapr_priya_id.expect("Priya seeded first");
    seed_membership(world, ws, priya, "admin").await;
}

#[given(regex = r#"^workspace "([^"]+)" exists with no projects$"#)]
async fn workspace_with_no_projects(world: &mut FoundryWorld, ws_name: String) {
    seed_workspace(world, &ws_name).await;
}

#[given(
    regex = r#"^workspace "([^"]+)" has a team "([^"]+)" with project "([^"]+)" \(([A-Z]+)\)$"#
)]
async fn workspace_with_one_project(
    world: &mut FoundryWorld,
    ws_name: String,
    team_name: String,
    p_name: String,
    p_prefix: String,
) {
    let ws = seed_workspace(world, &ws_name).await;
    let team = seed_team(world, ws, &team_name, &creation_slug(&team_name)).await;
    seed_project(world, ws, team, &p_name, &creation_slug(&p_name), &p_prefix).await;
}

#[given(regex = r"^Marco is a signed-in member who is not an instance admin$")]
async fn marco_member_not_instance_admin(world: &mut FoundryWorld) {
    let marco = seed_user(world, MARCO_EMAIL, "Marco", MARCO_PASSWORD).await;
    let ws = *world
        .iapr_workspace_ids
        .get("Canzan Labs")
        .expect("Canzan Labs seeded by the Background");
    seed_membership(world, ws, marco, "member").await;
    let (is_admin,): (bool,) =
        sqlx::query_as("SELECT EXISTS (SELECT 1 FROM instance_admins WHERE user_id = $1)")
            .bind(marco)
            .fetch_one(&pool(world))
            .await
            .expect("probe instance_admins");
    assert!(!is_admin, "Marco must NOT be an instance admin");
}

#[given(regex = r#"^issue ([A-Z]+)-(\d+) "([^"]+)" exists on the "([^"]+)" board$"#)]
async fn issue_exists_on_board(
    world: &mut FoundryWorld,
    prefix: String,
    number: i32,
    title: String,
    project_name: String,
) {
    let project_id = project_id_of(world, &project_name);
    let (stored_prefix, workspace_id): (String, uuid::Uuid) =
        sqlx::query_as("SELECT key_prefix, workspace_id FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_one(&pool(world))
            .await
            .expect("project row");
    assert_eq!(stored_prefix, prefix, "seeded key prefix mismatch");
    let author = world.iapr_priya_id.expect("Priya seeded");
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, author_id)
              VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(project_id)
    .bind(workspace_id)
    .bind(number)
    .bind(&title)
    .bind(author)
    .execute(&pool(world))
    .await
    .expect("insert issue");
    sqlx::query("UPDATE projects SET next_issue_number = $1 WHERE id = $2")
        .bind(number + 1)
        .bind(project_id)
        .execute(&pool(world))
        .await
        .expect("bump next_issue_number");
}

/// Capture the STORED board address from the database BEFORE the rename — the
/// board-survival oracle's anchor. Never re-derived from a name.
#[given(regex = r#"^Priya has noted where the "([^"]+)" board lives$"#)]
async fn note_where_board_lives(world: &mut FoundryWorld, project_name: String) {
    let project_id = project_id_of(world, &project_name);
    let (project_slug, team_slug): (String, String) = sqlx::query_as(
        "SELECT p.slug, t.slug FROM projects p JOIN teams t ON p.team_id = t.id WHERE p.id = $1",
    )
    .bind(project_id)
    .fetch_one(&pool(world))
    .await
    .expect("read stored slugs pre-rename");
    world.iapr_noted_board = Some((team_slug, project_slug));
}

// ===========================================================================
// When — dashboard reads
// ===========================================================================

#[when(regex = r"^Priya opens the instance dashboard$")]
async fn priya_opens_dashboard(world: &mut FoundryWorld) {
    let http = world.http.as_ref().expect("http client").clone();
    let outcome = signed_in_get(
        harness(world),
        &http,
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        DASHBOARD_PATH,
    )
    .await;
    record_outcome(world, outcome).await;
}

#[when(regex = r"^Marco requests the instance dashboard$")]
async fn marco_requests_dashboard(world: &mut FoundryWorld) {
    let http = world.http.as_ref().expect("http client").clone();
    let outcome = signed_in_get(
        harness(world),
        &http,
        MARCO_EMAIL,
        MARCO_PASSWORD,
        DASHBOARD_PATH,
    )
    .await;
    record_outcome(world, outcome).await;
}

// ===========================================================================
// When — renames
// ===========================================================================

/// Snapshot the target row, then drive the rename POST as Priya (real session
/// cookie + fresh double-submit `_csrf` via `signed_in_post`).
async fn priya_renames_project(world: &mut FoundryWorld, project_name: &str, new_name: &str) {
    let project_id = project_id_of(world, project_name);
    world.iapr_before_row = Some(snapshot_project(world, project_id).await);
    world.iapr_expected_name = Some(new_name.to_string());
    let http = world.http.as_ref().expect("http client").clone();
    let outcome = signed_in_post(
        harness(world),
        &http,
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &rename_url(&project_id.to_string()),
        &[("name", new_name)],
    )
    .await;
    record_outcome(world, outcome).await;
}

#[when(regex = r#"^Priya renames "([^"]*)" to "([^"]*)"$"#)]
async fn priya_renames(world: &mut FoundryWorld, project_name: String, new_name: String) {
    priya_renames_project(world, &project_name, &new_name).await;
}

#[when(regex = r#"^Priya renames "([^"]+)" to a (\d+)-character name$"#)]
async fn priya_renames_generated(world: &mut FoundryWorld, project_name: String, length: usize) {
    let new_name = "x".repeat(length);
    world.iapr_generated_name = Some(new_name.clone());
    priya_renames_project(world, &project_name, &new_name).await;
}

#[when(regex = r#"^Marco sends the rename for "([^"]+)" to "([^"]+)"$"#)]
async fn marco_sends_rename(world: &mut FoundryWorld, project_name: String, new_name: String) {
    let project_id = project_id_of(world, &project_name);
    world.iapr_before_row = Some(snapshot_project(world, project_id).await);
    let http = world.http.as_ref().expect("http client").clone();
    let outcome = signed_in_post(
        harness(world),
        &http,
        MARCO_EMAIL,
        MARCO_PASSWORD,
        &rename_url(&project_id.to_string()),
        &[("name", &new_name)],
    )
    .await;
    record_outcome(world, outcome).await;
}

/// A POST from Priya's real session but WITHOUT the `_csrf` field/cookie pair —
/// the double-submit middleware must refuse it before the handler runs (D5).
#[when(regex = r#"^a rename for "([^"]+)" is submitted without the dashboard's matching token$"#)]
async fn rename_without_csrf(world: &mut FoundryWorld, project_name: String) {
    let project_id = project_id_of(world, &project_name);
    world.iapr_before_row = Some(snapshot_project(world, project_id).await);
    let http = world.http.as_ref().expect("http client").clone();
    let session_pair = establish_session(harness(world), &http, PRIYA_EMAIL, PRIYA_PASSWORD).await;
    let outcome = post_with_cookie(
        harness(world),
        &http,
        &rename_url(&project_id.to_string()),
        &session_pair, // session only — deliberately NO foundry_csrf cookie, NO _csrf field
        &[("name", "Identity Platform")],
    )
    .await;
    record_outcome(world, outcome).await;
}

/// A signed-out POST carrying a VALID double-submit pair but no session — the
/// session gate must answer with the uniform non-enumerable 404 (D5).
#[when(regex = r#"^a signed-out visitor sends a rename for "([^"]+)"$"#)]
async fn signed_out_rename(world: &mut FoundryWorld, project_name: String) {
    let project_id = project_id_of(world, &project_name);
    world.iapr_before_row = Some(snapshot_project(world, project_id).await);
    let http = world.http.as_ref().expect("http client").clone();
    let base = harness(world).base_url();
    // Mint a real CSRF pair from the public sign-in page (no session involved).
    let signin_get = http
        .get(format!("{base}/sign-in"))
        .send()
        .await
        .expect("get /sign-in for csrf");
    let csrf_token = signin_get
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .and_then(|s| s.strip_prefix("foundry_csrf="))
        .and_then(|rest| rest.split(';').next())
        .expect("/sign-in must mint foundry_csrf cookie")
        .to_string();
    let outcome = post_with_cookie(
        harness(world),
        &http,
        &rename_url(&project_id.to_string()),
        &format!("foundry_csrf={csrf_token}"),
        &[("name", "Identity Platform"), ("_csrf", &csrf_token)],
    )
    .await;
    record_outcome(world, outcome).await;
}

#[when(regex = r#"^Priya sends a rename aimed at the project id "([^"]+)"$"#)]
async fn rename_garbled_id(world: &mut FoundryWorld, raw_id: String) {
    let http = world.http.as_ref().expect("http client").clone();
    let outcome = signed_in_post(
        harness(world),
        &http,
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &rename_url(&raw_id),
        &[("name", "Identity Platform")],
    )
    .await;
    record_outcome(world, outcome).await;
}

#[when(regex = r"^Priya sends a rename aimed at a project id that matches nothing$")]
async fn rename_unknown_id(world: &mut FoundryWorld) {
    let unknown = uuid::Uuid::now_v7().to_string();
    let http = world.http.as_ref().expect("http client").clone();
    let outcome = signed_in_post(
        harness(world),
        &http,
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &rename_url(&unknown),
        &[("name", "Identity Platform")],
    )
    .await;
    record_outcome(world, outcome).await;
}

// ===========================================================================
// Then — the dashboard listing (US-IAPR-01)
// ===========================================================================

/// Byte range of one workspace's section in the rendered page: from its name to
/// the next workspace name (or the end). Grouping is asserted positionally so
/// the test does not depend on the workspaces' render order.
///
/// Anchored INSIDE the `data-workspace-list` section (step-def defect fixed at
/// 01-01 GREEN): the app-shell sidebar renders the acting workspace's name in
/// the page chrome BEFORE the list, so a whole-body `find` starts the acting
/// workspace's section at the sidebar occurrence — a span that can never
/// contain its dashboard projects when another workspace's row renders first
/// (the shipped newest-first order). The assertion is about the dashboard
/// list, not the sidebar; the anchor restores that intent without weakening
/// the grouping check.
fn section_of<'a>(body: &'a str, ws_name: &str, all_ws: &[&str]) -> &'a str {
    let list = match body.find("data-workspace-list") {
        Some(pos) => &body[pos..],
        None => body,
    };
    let start = list
        .find(ws_name)
        .unwrap_or_else(|| panic!("dashboard must render workspace {ws_name:?}"));
    let end = all_ws
        .iter()
        .filter_map(|other| list.find(other))
        .filter(|&pos| pos > start)
        .min()
        .unwrap_or(list.len());
    &list[start..end]
}

#[then(
    regex = r#"^she sees "([^"]+)" and "([^"]+)" under "([^"]+)" and "([^"]+)" under "([^"]+)"$"#
)]
async fn sees_projects_grouped(
    world: &mut FoundryWorld,
    p1: String,
    p2: String,
    ws_a: String,
    p3: String,
    ws_b: String,
) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "the dashboard GET must render a 200 page; body = {:?}",
        world.last_body
    );
    let body = world.last_body.as_deref().expect("dashboard captured");
    let all_ws = [ws_a.as_str(), ws_b.as_str()];
    let section_a = section_of(body, &ws_a, &all_ws);
    assert!(
        section_a.contains(&p1) && section_a.contains(&p2),
        "{p1:?} and {p2:?} must be listed under {ws_a:?}; section = {section_a:?}"
    );
    let section_b = section_of(body, &ws_b, &all_ws);
    assert!(
        section_b.contains(&p3),
        "{p3:?} must be listed under {ws_b:?}; section = {section_b:?}"
    );
}

#[then(regex = r"^each project row shows its name, key prefix, and owning team$")]
async fn rows_show_name_prefix_team(world: &mut FoundryWorld) {
    let body = world.last_body.as_deref().expect("dashboard captured");
    assert!(
        body.contains(ROW_MARKER),
        "each listed project must render a [{ROW_MARKER}] element; got {body:?}"
    );
    for (prefix, team) in [("AUTH", "Backend"), ("SBX", "Backend"), ("CHR", "Home")] {
        assert!(
            body.contains(prefix),
            "a project row must show key prefix {prefix:?}"
        );
        assert!(
            body.contains(team),
            "a project row must show the owning team {team:?}"
        );
    }
}

#[then(regex = r"^within a workspace the projects are ordered by name$")]
async fn projects_ordered_by_name(world: &mut FoundryWorld) {
    let body = world.last_body.as_deref().expect("dashboard captured");
    let auth = body.find("Auth v2").expect("Auth v2 rendered");
    let sandbox = body.find("Sandbox").expect("Sandbox rendered");
    assert!(
        auth < sandbox,
        "\"Auth v2\" must be listed before \"Sandbox\" (name order within the workspace)"
    );
}

#[then(regex = r#"^the "([^"]+)" section says "No projects yet\."$"#)]
async fn section_says_no_projects(world: &mut FoundryWorld, ws_name: String) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "the dashboard GET must render a 200 page; body = {:?}",
        world.last_body
    );
    let body = world.last_body.as_deref().expect("dashboard captured");
    let all_ws = [ws_name.as_str(), "Canzan Labs"];
    let section = section_of(body, &ws_name, &all_ws);
    assert!(
        section.contains("No projects yet."),
        "the {ws_name:?} section must render the explicit empty state; section = {section:?}"
    );
}

// ===========================================================================
// Then — non-enumerable refusals
// ===========================================================================

/// Fetch the canonical never-existed answer: a signed-out GET to a path no
/// route has ever served. The fallback and every instance-admin refusal must be
/// byte-identical (ADR-002 idiom, D5).
async fn never_existed_answer(world: &mut FoundryWorld) -> (StatusCode, String) {
    let http = world.http.as_ref().expect("http client").clone();
    let base = harness(world).base_url();
    let resp = http
        .get(format!("{base}/never-existed-{}", uuid::Uuid::now_v7()))
        .send()
        .await
        .expect("get never-existed path");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    (status, body)
}

#[then(regex = r"^the answer is byte-identical to a never-existed address$")]
async fn answer_is_uniform_404(world: &mut FoundryWorld) {
    let (canon_status, canon_body) = never_existed_answer(world).await;
    assert_eq!(canon_status, StatusCode::NOT_FOUND, "fallback must be 404");
    assert_eq!(
        world.last_status,
        Some(canon_status),
        "the refusal status must match the never-existed answer; body = {:?}",
        world.last_body
    );
    assert_eq!(
        world.last_body.as_deref(),
        Some(canon_body.as_str()),
        "the refusal body must be BYTE-IDENTICAL to the never-existed answer (no enumeration oracle)"
    );
}

#[then(regex = r"^a signed-out visitor requesting the instance dashboard is answered identically$")]
async fn signed_out_dashboard_identical(world: &mut FoundryWorld) {
    let http = world.http.as_ref().expect("http client").clone();
    let base = harness(world).base_url();
    let resp = http
        .get(format!("{base}{DASHBOARD_PATH}"))
        .send()
        .await
        .expect("signed-out dashboard GET");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    assert_eq!(
        Some(status),
        world.last_status,
        "signed-out and non-admin refusals must share one status"
    );
    assert_eq!(
        Some(body.as_str()),
        world.last_body.as_deref(),
        "signed-out and non-admin refusals must be byte-identical"
    );
}

// ===========================================================================
// Then — rename outcomes (US-IAPR-02)
// ===========================================================================

/// The success fragment is the BARE row partial (one-partial rule): 200, a
/// `[data-project-row]` root, the new name + prefix, and no full-page wrapper.
fn assert_row_fragment(world: &FoundryWorld, name: &str, prefix: Option<&str>) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "the rename must answer 200 with the re-rendered row; body = {:?}",
        world.last_body
    );
    let body = world.last_body.as_deref().expect("fragment captured");
    assert!(
        body.contains(ROW_MARKER),
        "the success fragment must be the row partial ([{ROW_MARKER}]); got {body:?}"
    );
    assert!(
        body.contains(name),
        "the row must show the name {name:?}; got {body:?}"
    );
    if let Some(prefix) = prefix {
        assert!(
            body.contains(prefix),
            "the row must keep showing key prefix {prefix:?}; got {body:?}"
        );
    }
    assert!(
        !body.contains("<html"),
        "the fragment must be BARE (no base.html double-wrap); got {body:?}"
    );
}

#[then(regex = r#"^the row she gets back shows "([^"]+)" with key prefix "([^"]+)"$"#)]
async fn row_shows_name_and_prefix(world: &mut FoundryWorld, name: String, prefix: String) {
    assert_row_fragment(world, &name, Some(&prefix));
}

#[then(
    regex = r#"^the row she gets back shows "([^"]+)" with key prefix "([^"]+)" and carries no error$"#
)]
async fn row_shows_name_no_error(world: &mut FoundryWorld, name: String, prefix: String) {
    assert_row_fragment(world, &name, Some(&prefix));
    let body = world.last_body.as_deref().expect("fragment captured");
    assert!(
        !body.contains(ERROR_MARKER),
        "a quiet no-op success must carry no error marker; got {body:?}"
    );
}

#[then(regex = r"^the row she gets back shows that exact name$")]
async fn row_shows_generated_name(world: &mut FoundryWorld) {
    let name = world
        .iapr_generated_name
        .clone()
        .expect("a generated name was submitted");
    assert_row_fragment(world, &name, None);
}

#[then(regex = r#"^reopening the instance dashboard shows "([^"]+)" and no longer "([^"]+)"$"#)]
async fn reopened_dashboard_shows(world: &mut FoundryWorld, new_name: String, old_name: String) {
    let http = world.http.as_ref().expect("http client").clone();
    let outcome = signed_in_get(
        harness(world),
        &http,
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        DASHBOARD_PATH,
    )
    .await;
    assert_eq!(outcome.status, StatusCode::OK, "dashboard must render");
    assert!(
        outcome.body.contains(&new_name),
        "the reloaded dashboard must show {new_name:?}"
    );
    assert!(
        !outcome.body.contains(&old_name),
        "the reloaded dashboard must no longer show {old_name:?}"
    );
}

#[then(regex = r#"^the board still answers at its old address, now titled "([^"]+)"$"#)]
async fn board_survives_at_old_address(world: &mut FoundryWorld, new_name: String) {
    let (team_slug, project_slug) = world
        .iapr_noted_board
        .clone()
        .expect("the board address was noted BEFORE the rename");
    let http = world.http.as_ref().expect("http client").clone();
    let outcome = signed_in_get(
        harness(world),
        &http,
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &format!("/team/{team_slug}/project/{project_slug}"),
    )
    .await;
    assert_eq!(
        outcome.status,
        StatusCode::OK,
        "the board must still serve at /team/{team_slug}/project/{project_slug} after the rename"
    );
    assert!(
        outcome.body.contains(&new_name),
        "the surviving board must be titled {new_name:?}"
    );
    // Keep the board body for the issue-card assertion that follows.
    world.last_status = Some(outcome.status);
    world.last_body = Some(outcome.body);
}

#[then(
    regex = r"^issue ([A-Z]+)-(\d+) keeps its key and its card actions still answer at the old address$"
)]
async fn issue_card_actions_survive(world: &mut FoundryWorld, prefix: String, number: i32) {
    let (team_slug, project_slug) = world
        .iapr_noted_board
        .clone()
        .expect("the board address was noted BEFORE the rename");
    let board = world.last_body.as_deref().expect("board body captured");
    let key = format!("{prefix}-{number}");
    assert!(
        board.contains(&key),
        "the issue must keep its key {key:?} after the rename"
    );
    // D2 oracle: the card's edit/state actions must point at the STORED slug —
    // a render-time slugify(new_name) would emit a different, dead address.
    let edit_url = format!("/team/{team_slug}/project/{project_slug}/issues/{number}/edit");
    let state_url = format!("/team/{team_slug}/project/{project_slug}/issues/{number}/state");
    assert!(
        board.contains(&edit_url),
        "the card's edit action must still point at the OLD address {edit_url:?} (D2); board = {board:?}"
    );
    assert!(
        board.contains(&state_url),
        "the card's state action must still point at the OLD address {state_url:?} (D2)"
    );
    // And the old address must actually ANSWER — not merely be printed.
    let http = world.http.as_ref().expect("http client").clone();
    let outcome = signed_in_get(
        harness(world),
        &http,
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &edit_url,
    )
    .await;
    assert_eq!(
        outcome.status,
        StatusCode::OK,
        "the card's edit dialog must still open at the old address after the rename"
    );
}

#[then(regex = r"^the change report at the old address shows the new name$")]
async fn report_shows_new_name(world: &mut FoundryWorld) {
    let (team_slug, project_slug) = world
        .iapr_noted_board
        .clone()
        .expect("the board address was noted BEFORE the rename");
    let new_name = world
        .iapr_expected_name
        .clone()
        .expect("a rename was submitted");
    let http = world.http.as_ref().expect("http client").clone();
    let outcome = signed_in_get(
        harness(world),
        &http,
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        &format!("/team/{team_slug}/project/{project_slug}/report"),
    )
    .await;
    assert_eq!(
        outcome.status,
        StatusCode::OK,
        "the change report must still serve at its old address"
    );
    assert!(
        outcome.body.contains(&new_name),
        "the change report must show the new name {new_name:?}"
    );
}

#[then(regex = r"^the project's stored address and key prefix are byte-identical to before$")]
async fn stored_identity_unchanged(world: &mut FoundryWorld) {
    let before = world.iapr_before_row.clone().expect("pre-rename snapshot");
    let new_name = world
        .iapr_expected_name
        .clone()
        .expect("a rename was submitted");
    let renamed_id = *world
        .iapr_project_ids
        .get("Auth v2")
        .expect("the renamed project was seeded as Auth v2");
    let after = snapshot_project(world, renamed_id).await;
    assert_project_delta(&before, &after, &new_name);
}

#[then(regex = r"^the stored project record is untouched$")]
async fn stored_record_untouched(world: &mut FoundryWorld) {
    let before = world.iapr_before_row.clone().expect("pre-write snapshot");
    let renamed_id = *world
        .iapr_project_ids
        .get("Sandbox")
        .expect("the no-op target was seeded as Sandbox");
    let after = snapshot_project(world, renamed_id).await;
    assert_eq!(
        after, before,
        "a no-op rename must leave the projects row byte-identical"
    );
}

#[then(regex = r"^the project's stored address is unchanged$")]
async fn stored_slug_unchanged(world: &mut FoundryWorld) {
    let before = world.iapr_before_row.clone().expect("pre-write snapshot");
    let expected = world
        .iapr_expected_name
        .clone()
        .expect("a rename was submitted");
    let renamed_id = *world
        .iapr_project_ids
        .get("Sandbox")
        .expect("the re-case target was seeded as Sandbox");
    let after = snapshot_project(world, renamed_id).await;
    assert_project_delta(&before, &after, &expected);
}

// ===========================================================================
// Then — refusals leave the name alone
// ===========================================================================

#[then(regex = r#"^the project is still named "([^"]+)" everywhere$"#)]
async fn still_named_everywhere(world: &mut FoundryWorld, name: String) {
    // The persisted row.
    let project_id = project_id_of(world, &name);
    let (stored_name,): (String,) = sqlx::query_as("SELECT name FROM projects WHERE id = $1")
        .bind(project_id)
        .fetch_one(&pool(world))
        .await
        .expect("project row");
    assert_eq!(stored_name, name, "the persisted name must be unchanged");
    // The dashboard.
    let http = world.http.as_ref().expect("http client").clone();
    let dashboard = signed_in_get(
        harness(world),
        &http,
        PRIYA_EMAIL,
        PRIYA_PASSWORD,
        DASHBOARD_PATH,
    )
    .await;
    assert!(
        dashboard.body.contains(&name),
        "the dashboard must still show {name:?}"
    );
    // The board and the report, at the STORED address.
    let (team_slug, project_slug) = stored_slugs_of(world, &name);
    for path in [
        format!("/team/{team_slug}/project/{project_slug}"),
        format!("/team/{team_slug}/project/{project_slug}/report"),
    ] {
        let outcome =
            signed_in_get(harness(world), &http, PRIYA_EMAIL, PRIYA_PASSWORD, &path).await;
        assert_eq!(outcome.status, StatusCode::OK, "{path} must still serve");
        assert!(
            outcome.body.contains(&name),
            "{path} must still show {name:?}"
        );
    }
}

#[then(regex = r"^the rename is refused before any change is made$")]
async fn refused_before_handler(world: &mut FoundryWorld) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::FORBIDDEN),
        "a POST without its double-submit pair must be refused by the CSRF middleware (403); body = {:?}",
        world.last_body
    );
    let before = world.iapr_before_row.clone().expect("pre-write snapshot");
    let renamed_id = *world
        .iapr_project_ids
        .get("Auth v2")
        .expect("target seeded as Auth v2");
    let after = snapshot_project(world, renamed_id).await;
    assert_eq!(after, before, "the refused POST must have written nothing");
}

#[then(regex = r#"^the rename is refused saying "([^"]+)"$"#)]
async fn refused_with_message(world: &mut FoundryWorld, message: String) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::UNPROCESSABLE_ENTITY),
        "a validation refusal must be 422 (D6); body = {:?}",
        world.last_body
    );
    let body = world.last_body.as_deref().expect("fragment captured");
    assert!(
        body.contains(&message),
        "the error fragment must state {message:?}; got {body:?}"
    );
    assert!(
        body.contains(ERROR_MARKER),
        "the error fragment must carry the {ERROR_MARKER:?} marker; got {body:?}"
    );
    assert!(
        !body.contains("<html"),
        "the error fragment must be BARE (no base.html double-wrap); got {body:?}"
    );
}

#[then(regex = r"^both projects keep their names$")]
async fn both_projects_keep_names(world: &mut FoundryWorld) {
    for name in ["Auth v2", "Sandbox"] {
        let project_id = project_id_of(world, name);
        let (stored_name,): (String,) = sqlx::query_as("SELECT name FROM projects WHERE id = $1")
            .bind(project_id)
            .fetch_one(&pool(world))
            .await
            .expect("project row");
        assert_eq!(
            stored_name, name,
            "the refused rename must leave {name:?} untouched"
        );
    }
}

// ===========================================================================
// @needs-browser — the DOM oracle
// ===========================================================================

/// A page-lifetime marker: set once after load, gone if the page fully
/// navigates. The "without the page reloading" assertions read it back.
const PAGE_MARKER_SET: &str = "window.__iapr_page_marker = 'alive';";
const PAGE_MARKER_GET: &str = "return window.__iapr_page_marker || null;";

#[given(regex = r"^Priya has the instance dashboard open in her browser$")]
async fn dashboard_open_in_browser(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    let browser = browser_harness::new_session().await;
    {
        let harness = world.harness.as_ref().expect("harness");
        browser_harness::sign_in_through_browser(&browser, harness, PRIYA_EMAIL, PRIYA_PASSWORD)
            .await;
        browser
            .goto(&format!("{}{DASHBOARD_PATH}", harness.base_url()))
            .await
            .expect("open the instance dashboard in the browser");
    }
    browser
        .execute(PAGE_MARKER_SET, vec![])
        .await
        .expect("plant the page-lifetime marker");
    world.browser = Some(browser);
}

/// XPath to the `[data-project-row]` element whose text mentions the project.
fn row_xpath(project_name: &str) -> String {
    format!("//*[@data-project-row][contains(., \"{project_name}\")]")
}

async fn submit_rename_in_row(world: &mut FoundryWorld, project_name: &str, typed: &str) {
    let browser = world.browser.as_ref().expect("browser session");
    let row = browser
        .wait()
        .at_most(Duration::from_secs(10))
        .for_element(Locator::XPath(&row_xpath(project_name)))
        .await
        .unwrap_or_else(|err| {
            panic!("the dashboard must render a [data-project-row] for {project_name:?}: {err}")
        });
    let input = row
        .find(Locator::Css("input[name='name']"))
        .await
        .expect("the row must carry a rename input");
    input.clear().await.expect("clear the rename input");
    input.send_keys(typed).await.expect("type the new name");
    row.find(Locator::Css("button[type='submit']"))
        .await
        .expect("the row must carry a submit button")
        .click()
        .await
        .expect("submit the rename form");
}

#[when(regex = r#"^she types "([^"]+)" into the "([^"]+)" row and submits it$"#)]
async fn types_and_submits(world: &mut FoundryWorld, new_name: String, project_name: String) {
    submit_rename_in_row(world, &project_name, &new_name).await;
}

/// One space: the browser's `required` sees a non-empty value and lets htmx
/// POST it; the server trims it to empty (D4) — the only "empty name" a real
/// browser will actually submit (the form-error-display precedent).
#[when(regex = r#"^she blanks the name in the "([^"]+)" row and submits it$"#)]
async fn blanks_and_submits(world: &mut FoundryWorld, project_name: String) {
    submit_rename_in_row(world, &project_name, " ").await;
}

#[then(regex = r#"^that row shows "([^"]+)" without the page reloading$"#)]
async fn row_swapped_in_place(world: &mut FoundryWorld, new_name: String) {
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .wait()
        .at_most(Duration::from_secs(10))
        .for_element(Locator::XPath(&row_xpath(&new_name)))
        .await
        .unwrap_or_else(|err| {
            panic!("the row must re-render in place showing {new_name:?}: {err}")
        });
    let marker = browser
        .execute(PAGE_MARKER_GET, vec![])
        .await
        .expect("read the page-lifetime marker");
    assert_eq!(
        marker.as_str(),
        Some("alive"),
        "the page-lifetime marker must survive — the row swap must NOT be a full reload"
    );
}

#[then(regex = r#"^"([^"]+)" appears inside that row's message area$"#)]
async fn error_appears_in_row_slot(world: &mut FoundryWorld, message: String) {
    let browser = world.browser.as_ref().expect("browser session");
    // The submitting row still names "Auth v2" (the refusal changed nothing);
    // the message must land inside THAT row's [data-error-slot], not a global
    // banner and not another row's slot.
    let slot_xpath = format!("{}//*[@data-error-slot]", row_xpath("Auth v2"));
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(slot) = browser.find(Locator::XPath(&slot_xpath)).await {
            if let Ok(text) = slot.text().await {
                if text.contains(&message) {
                    return;
                }
            }
        }
        if std::time::Instant::now() > deadline {
            let page = browser.source().await.unwrap_or_default();
            panic!(
                "{message:?} must appear inside the submitting row's [data-error-slot] \
                 (form-errors.js routing); page = {page:?}"
            );
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[then(regex = r"^the rename form is still there for her to correct$")]
async fn form_still_mounted(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let input_xpath = format!("{}//input[@name='name']", row_xpath("Auth v2"));
    browser
        .find(Locator::XPath(&input_xpath))
        .await
        .expect("the rename form must stay mounted and resubmittable after a refusal");
}
