//! fix-hide-unreachable-boards — step definitions for the "never offer a board the
//! member cannot open" regression.
//!
//! THE DEFECT (user-approved RCA): the dashboard "Your projects" list
//! (`signin::dashboard_root`) and the rail Board link (`nav::resolve_board_href`) read
//! `Store::list_projects_for_workspace`, which carries no membership filter, while the
//! board gate (`resolve_board_project` -> `Store::is_team_member`) is team-scoped. A
//! member added by invite-accept or OIDC provisioning has a workspace membership but
//! NO team membership, so they are offered `/team/general/project/sandbox` and 404.
//!
//! Harness: the in-process member-invites harness (`mi_harness`), reusing the shipped
//! Gherkin steps "Dana Reyes is signed in as an admin of ...", "Dana invites ... to
//! ..." and "Sam opens his invite link and sets a password ..." so the no-team member
//! is created through the REAL issue + accept handlers (the production path that omits
//! `team_memberships`). Only the lead's team membership is seeded directly — the lead
//! is the guard, not the defect's precondition.
//!
//! Assertions are over port-exposed web observables only: HTTP status, listed project
//! names, `href` values, and empty-state copy.

use crate::support::harness::{seed_lanes_for_project, signed_in_get, InProcHarness};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::redirect::Policy;
use reqwest::StatusCode;

/// Dana's password as seeded by the shipped member-invites Background
/// (`feature_member_invites::DANA_PASSWORD`); the guard signs her in with it.
const DANA_PASSWORD: &str = "northwind-admin-secret";

fn harness(world: &FoundryWorld) -> &InProcHarness {
    world
        .mi_harness
        .as_ref()
        .expect("the Background must have spawned the member-invites harness")
}

fn http(world: &mut FoundryWorld) -> reqwest::Client {
    world
        .http
        .get_or_insert_with(|| {
            reqwest::Client::builder()
                .redirect(Policy::none())
                .cookie_store(false)
                .build()
                .expect("build reqwest client")
        })
        .clone()
}

fn workspace_id(world: &FoundryWorld, ws_name: &str) -> uuid::Uuid {
    *world
        .mi_workspace_ids
        .get(ws_name)
        .unwrap_or_else(|| panic!("workspace {ws_name:?} seeded in the Background"))
}

fn dashboard_body(world: &FoundryWorld) -> &str {
    world
        .last_body
        .as_deref()
        .expect("a dashboard GET captured a rendered body")
}

/// The `href` of the rail's Board item (`partials/sidebar.html`): the anchor whose
/// text is exactly `Board`.
fn rail_board_href(body: &str) -> Option<String> {
    let anchor_end = body.find(">Board</a>")?;
    let anchor_start = body[..anchor_end].rfind("<a ")?;
    let anchor = &body[anchor_start..anchor_end];
    let after = &anchor[anchor.find("href=\"")? + "href=\"".len()..];
    Some(after[..after.find('"')?].to_string())
}

/// The "Your projects" section of the dashboard (up to the next section heading).
fn your_projects_section(body: &str) -> &str {
    let start = body
        .find("<h2>Your projects</h2>")
        .expect("the dashboard renders a \"Your projects\" section");
    let rest = &body[start..];
    let end = rest[1..].find("<h2>").map_or(rest.len(), |i| i + 1);
    &rest[..end]
}

// ---------------------------------------------------------------------------
// Given
// ---------------------------------------------------------------------------

/// Seed team `general` in the workspace with Dana as its `lead`, and the `Sandbox`
/// project (with its lanes) under it — the team-general project the incident member
/// was wrongly offered.
#[given(
    regex = r#"^the "([^"]+)" workspace has a project "([^"]+)" on team "([^"]+)" led by Dana$"#
)]
async fn workspace_has_team_project_led_by_dana(
    world: &mut FoundryWorld,
    ws_name: String,
    project_name: String,
    team_slug: String,
) {
    let ws = workspace_id(world, &ws_name);
    let dana = world
        .mi_admin_user_id
        .expect("the Background seeded Dana's user id");
    let pool = harness(world).app.state.store.pool().clone();

    let team_id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $3)")
        .bind(team_id)
        .bind(ws)
        .bind(&team_slug)
        .execute(&pool)
        .await
        .expect("seed the team");
    sqlx::query("INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'lead')")
        .bind(team_id)
        .bind(dana)
        .execute(&pool)
        .await
        .expect("seed Dana as the team lead");

    let project_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, $4, $5, 'SBX')",
    )
    .bind(project_id)
    .bind(team_id)
    .bind(ws)
    .bind(&project_name)
    .bind(project_name.to_lowercase())
    .execute(&pool)
    .await
    .expect("seed the project");
    seed_lanes_for_project(&pool, project_id).await;
}

/// Precondition sanity check: the invite-accepted member is a workspace member with
/// ZERO team memberships — exactly the shape invite-accept / OIDC provisioning leaves.
#[then(regex = r#"^Sam is a member of "([^"]+)" on no team$"#)]
async fn sam_belongs_to_no_team(world: &mut FoundryWorld, ws_name: String) {
    assert_eq!(
        world.mi_post_status,
        Some(StatusCode::SEE_OTHER),
        "Sam's invite accept must have auto-signed him in (303)"
    );
    let ws = workspace_id(world, &ws_name);
    let pool = harness(world).app.state.store.pool().clone();
    let (workspace_memberships, team_memberships): (i64, i64) = sqlx::query_as(
        "SELECT
            (SELECT count(*) FROM workspace_memberships wm JOIN users u ON u.id = wm.user_id
              WHERE wm.workspace_id = $1 AND u.email_lower = 'sam.okafor@northwind.example'),
            (SELECT count(*) FROM team_memberships tm JOIN users u ON u.id = tm.user_id
              WHERE u.email_lower = 'sam.okafor@northwind.example')",
    )
    .bind(ws)
    .fetch_one(&pool)
    .await
    .expect("count Sam's memberships");
    assert_eq!(workspace_memberships, 1, "Sam is a member of {ws_name:?}");
    assert_eq!(team_memberships, 0, "Sam is on no team");
}

// ---------------------------------------------------------------------------
// When
// ---------------------------------------------------------------------------

/// GET `/` with the session cookie the invite accept issued (the auto-sign-in).
#[when(regex = r#"^Sam opens his dashboard$"#)]
async fn sam_opens_dashboard(world: &mut FoundryWorld) {
    let session = world
        .mi_session_cookie
        .clone()
        .expect("the invite accept issued Sam a session cookie");
    let base = harness(world).base_url();
    let resp = http(world)
        .get(format!("{base}/"))
        .header(reqwest::header::COOKIE, session)
        .send()
        .await
        .expect("GET / as Sam");
    world.last_status = Some(resp.status());
    world.last_body = Some(resp.text().await.unwrap_or_default());
}

#[when(regex = r#"^Dana opens her dashboard$"#)]
async fn dana_opens_dashboard(world: &mut FoundryWorld) {
    let email = world
        .mi_admin_email
        .clone()
        .expect("the Background seeded Dana's email");
    let client = http(world);
    let outcome = signed_in_get(harness(world), &client, &email, DANA_PASSWORD, "/").await;
    world.last_status = Some(outcome.status);
    world.last_body = Some(outcome.body);
}

// ---------------------------------------------------------------------------
// Then
// ---------------------------------------------------------------------------

#[then(regex = r#"^the dashboard renders 200 with a "([^"]+)" greeting$"#)]
async fn dashboard_renders_greeting(world: &mut FoundryWorld, greeting: String) {
    assert_eq!(world.last_status, Some(StatusCode::OK), "dashboard status");
    let body = dashboard_body(world);
    assert!(
        body.contains(&format!("{greeting}, ")),
        "the dashboard must greet the user with {greeting:?}; body = {body}"
    );
}

#[then(regex = r#"^the "Your projects" list does not link to "([^"]+)"$"#)]
async fn your_projects_does_not_link(world: &mut FoundryWorld, href: String) {
    let section = your_projects_section(dashboard_body(world));
    assert!(
        !section.contains(&format!("href=\"{href}\"")),
        "a member on no team must not be offered {href:?} (the board gate 404s them); \
         \"Your projects\" rendered: {section}"
    );
}

#[then(regex = r#"^the "Your projects" list shows "([^"]+)" linking to "([^"]+)"$"#)]
async fn your_projects_shows(world: &mut FoundryWorld, name: String, href: String) {
    let section = your_projects_section(dashboard_body(world));
    assert!(
        section.contains(&format!("href=\"{href}\"")) && section.contains(&name),
        "the lead must see {name:?} linking to {href:?}; \"Your projects\" rendered: {section}"
    );
}

#[then(regex = r#"^the rail Board link does not point to "([^"]+)"$"#)]
async fn rail_board_not(world: &mut FoundryWorld, href: String) {
    let board = rail_board_href(dashboard_body(world));
    assert_ne!(
        board.as_deref(),
        Some(href.as_str()),
        "the rail Board link must not target a board the member cannot open"
    );
}

#[then(regex = r#"^the rail Board link points to "([^"]+)"$"#)]
async fn rail_board_is(world: &mut FoundryWorld, href: String) {
    let board = rail_board_href(dashboard_body(world));
    assert_eq!(board.as_deref(), Some(href.as_str()), "rail Board href");
}

#[then(regex = r#"^the no-team empty state is shown$"#)]
async fn no_team_empty_state(world: &mut FoundryWorld) {
    let section = your_projects_section(dashboard_body(world));
    assert!(
        section.contains("not on any team yet") && section.contains("ask a workspace admin"),
        "a member on no team must see the no-team empty state; \
         \"Your projects\" rendered: {section}"
    );
}

#[then(regex = r#"^opening the rail Board link as Dana returns 200$"#)]
async fn dana_opens_board(world: &mut FoundryWorld) {
    let href = rail_board_href(dashboard_body(world)).expect("the rail renders a Board link");
    let email = world
        .mi_admin_email
        .clone()
        .expect("the Background seeded Dana's email");
    let client = http(world);
    let outcome = signed_in_get(harness(world), &client, &email, DANA_PASSWORD, &href).await;
    assert_eq!(
        outcome.status,
        StatusCode::OK,
        "the lead's Board link {href:?} must open; body = {}",
        outcome.body
    );
}
