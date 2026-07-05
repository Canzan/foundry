//! dashboard-enhancements — Slice 01 (US-01) step definitions.
//!
//! The signed-in landing (`GET /` → `signin::dashboard_root`) greets the user by
//! name and names the acting workspace. These scenarios drive the real HTTP
//! surface through the in-process axum harness (`support::harness::InProcHarness`)
//! with a real per-scenario Postgres (testcontainers, `@real-io`), mirroring
//! `us_06_signin` / `us_07_project_create`.
//!
//! Reuses, per the globally-unique-phrase rule (cucumber-rs):
//!   - `(\w+) is signed in`                        (us_07_project_create)
//!   - `the response body contains "..."`          (us_06_signin)
//!   - `support::harness::{InProcHarness, signed_in_get}`
//!
//! The Background's `Ada is signed in` routes through us_07's `(\w+) is signed
//! in`, which resolves personas via `identity_for` — "Ada" is registered there
//! and seeded here with the matching `ADA_PASSWORD`.
//!
//! Slice-01 covers the two drivable US-01 scenarios (greets-by-name,
//! markup-inert). The third scenario ("greeting degrades to 200 if identity
//! cannot be loaded") stays `@pending`: the in-process harness has no clean seam
//! to force the greeting query to fail mid-request, so the degradation contract
//! (D1 / AC-01.4) is pinned by a Rust unit test on the handler fallback seam
//! (`signin::tests::greeting_degrades_to_neutral_when_identity_absent`).

use crate::support::harness::{signed_in_get, InProcHarness};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::redirect::Policy;
use secrecy::SecretString;

const TEST_NOW: &str = "2026-01-15T12:00:00Z";
/// The password Ada (and the markup-name member) are seeded with. The `"Ada"`
/// arm of `us_07_project_create::identity_for` returns this SAME literal — keep
/// in sync if either moves.
const ADA_PASSWORD: &str = "ada-correct-horse-battery-staple";

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

/// Lowercased `<first-name>@acme.com` — the derivation `identity_for` uses for
/// the personas this feature signs in (Ada → `ada@acme.com`).
fn email_for(first_name: &str) -> String {
    format!("{}@acme.com", first_name.to_ascii_lowercase())
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

/// Askama's default HTML escaping, reproduced for the markup-inert assertion:
/// `<b>pwn</b>` → `&lt;b&gt;pwn&lt;/b&gt;`.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ----- Background ---------------------------------------------------------

#[given(
    regex = r#"^a workspace "([^"]+)" exists with admin "([^"]+)" and display name "([^"]+)"$"#
)]
async fn workspace_with_named_admin(
    world: &mut FoundryWorld,
    ws_name: String,
    admin_first_name: String,
    display_name: String,
) {
    // Reset per-scenario state: fresh harness + fresh schema.
    world.harness = None;
    world.http = None;
    world.us_06_unknown_latencies_ms.clear();
    world.us_06_wrong_pw_latencies_ms.clear();
    world.session_cookie_header = None;
    world.last_status = None;
    world.last_body = None;
    world.last_headers = None;
    world.dash_last_display_name = None;
    world.us_07_signed_in_email = None;
    world.us_07_signed_in_password = None;
    ensure_harness(world).await;

    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();

    let workspace_id = uuid::Uuid::now_v7();
    let admin_id = uuid::Uuid::now_v7();
    let admin_email = email_for(&admin_first_name);
    let admin_hash =
        foundry_auth::hash_password(&SecretString::new(ADA_PASSWORD.to_string().into()))
            .await
            .expect("hash admin pw");
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(workspace_id)
        .bind(&ws_name)
        .execute(pool)
        .await
        .expect("insert workspace");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(admin_id)
    .bind(&admin_email)
    .bind(&admin_email)
    .bind(&display_name)
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
}

#[given(regex = r#"^a project "([^"]+)" with key prefix "([^"]+)" exists in "([^"]+)"$"#)]
async fn project_exists_in_workspace(
    world: &mut FoundryWorld,
    project_name: String,
    key_prefix: String,
    ws_name: String,
) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let ws_id: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM workspaces WHERE name = $1")
        .bind(&ws_name)
        .fetch_one(pool)
        .await
        .expect("fetch workspace");

    // Ensure a default "General" team to hang the project on.
    let team_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'General', 'general')
              ON CONFLICT (workspace_id, slug) DO NOTHING",
    )
    .bind(team_id)
    .bind(ws_id.0)
    .execute(pool)
    .await
    .expect("insert team");
    let team_row: (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM teams WHERE workspace_id = $1 AND slug = 'general'")
            .bind(ws_id.0)
            .fetch_one(pool)
            .await
            .expect("fetch team");

    let project_id = uuid::Uuid::now_v7();
    let slug = slugify(&project_name);
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, $4, $5, $6)",
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
}

// "Ada is signed in" reuses us_07_project_create's `(\w+) is signed in` Given,
// which records the persona's identity via `identity_for`.

#[given(regex = r#"^a member "([^"]+)" whose display name is "([^"]+)" is signed in$"#)]
async fn member_with_display_name_signed_in(
    world: &mut FoundryWorld,
    member_first_name: String,
    display_name: String,
) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();

    // Attach the member to the (Background-seeded) workspace so sign-in resolves
    // an active workspace (ADR-005 fail-closed otherwise).
    let ws_id: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM workspaces LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("fetch workspace");
    let email = email_for(&member_first_name);
    let hash = foundry_auth::hash_password(&SecretString::new(ADA_PASSWORD.to_string().into()))
        .await
        .expect("hash member pw");
    let user_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(&email)
    .bind(&email)
    .bind(&display_name)
    .bind(&hash)
    .execute(pool)
    .await
    .expect("insert member user");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')
              ON CONFLICT DO NOTHING",
    )
    .bind(ws_id.0)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("insert member membership");

    world.us_07_signed_in_email = Some(email);
    world.us_07_signed_in_password = Some(ADA_PASSWORD.to_string());
    world.dash_last_display_name = Some(display_name);
}

// ----- When ---------------------------------------------------------------

#[when(regex = r#"^(\w+) visits "/"$"#)]
async fn visits_root(world: &mut FoundryWorld, _who: String) {
    ensure_harness(world).await;
    let email = world
        .us_07_signed_in_email
        .clone()
        .expect("a persona is signed in");
    let password = world
        .us_07_signed_in_password
        .clone()
        .expect("signed-in password recorded");
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let outcome = signed_in_get(harness, http, &email, &password, "/").await;
    world.last_status = Some(outcome.status);
    world.last_headers = Some(outcome.headers);
    world.last_body = Some(outcome.body);
}

// ----- Then ---------------------------------------------------------------

// "the response body contains \"...\"" reuses us_06_signin's shared Then.

#[then(regex = r#"^the response body contains the heading "([^"]+)"$"#)]
async fn body_contains_heading(world: &mut FoundryWorld, heading: String) {
    let body = world.last_body.as_deref().unwrap_or("");
    let needle = format!("<h1>{heading}</h1>");
    assert!(
        body.contains(&needle),
        "response body missing heading {needle:?}: {body:?}"
    );
}

#[then(regex = r"^the response body contains the escaped display name$")]
async fn body_contains_escaped_display_name(world: &mut FoundryWorld) {
    let raw = world
        .dash_last_display_name
        .clone()
        .expect("a markup display name was seeded");
    let escaped = html_escape(&raw);
    let body = world.last_body.as_deref().unwrap_or("");
    assert!(
        body.contains(&escaped),
        "response body missing the escaped display name {escaped:?}: {body:?}"
    );
}

#[then(regex = r#"^the response body does not contain a live "<b>" element$"#)]
async fn body_no_live_element(world: &mut FoundryWorld) {
    let raw = world
        .dash_last_display_name
        .clone()
        .expect("a markup display name was seeded");
    let body = world.last_body.as_deref().unwrap_or("");
    assert!(
        !body.contains(&raw),
        "response body contained the LIVE markup {raw:?} (must be HTML-escaped): {body:?}"
    );
}
