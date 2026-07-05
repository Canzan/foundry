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
//!   - `the response redirects (\w+) to "..."`     (us_06_signin — sign-out 303)
//!
//! Slice-03 (US-02 sign out) holds ONE session across the visit → sign-out →
//! re-visit sequence via `support::harness::{establish_session, get_with_cookie,
//! post_with_cookie}` — the per-call re-authenticating `signed_in_get`/`_post`
//! cannot express session lifecycle (sign-out must destroy the SAME session a
//! later step then observes invalid).
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

use crate::support::harness::{
    establish_session, get_with_cookie, post_with_cookie, InProcHarness,
};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::redirect::Policy;
use reqwest::StatusCode;
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
    world.dash_session_cookie = None;
    world.dash_csrf_token = None;
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

// "Ada is an instance super-admin" grants the (Background-seeded, already
// signed-in) Ada the INSTANCE-level super-admin authority via the shipped
// `Store::grant_instance_admin` (lib.rs:1584) — the same upgrade path a real
// operator takes. Ada's session identity is unchanged (Background's `Ada is
// signed in` already recorded it); this only flips the `is_instance_admin`
// predicate the dashboard reads.
#[given(regex = r"^Ada is an instance super-admin$")]
async fn ada_is_instance_super_admin(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let ada_email = email_for("Ada");
    let ada_id: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(&ada_email)
        .fetch_one(pool)
        .await
        .expect("fetch Ada's user id");
    harness
        .app
        .state
        .store
        .grant_instance_admin(ada_id.0)
        .await
        .expect("grant Ada instance super-admin");
}

// "a member Mei who is not an instance admin is signed in" seeds Mei as a plain
// workspace member (NO `instance_admins` row) attached to the Background
// workspace, then records her as the signed-in persona. Mirrors
// `member_with_display_name_signed_in` (same seeding + sign-in seam), minus the
// markup display name and minus any super-admin grant — the negative case for
// the role-conditional link.
#[given(regex = r#"^a member "([^"]+)" who is not an instance admin is signed in$"#)]
async fn member_not_instance_admin_signed_in(world: &mut FoundryWorld, member_first_name: String) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();

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
    .bind(&member_first_name)
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
    // Hold ONE session for the whole scenario so the sign-out flow (US-02) can
    // destroy the SAME session it visited under and then observe it invalid. The
    // GET carries ONLY the session cookie (no `foundry_csrf`), so `dashboard_root`
    // MINTS a fresh CSRF cookie + renders the matching hidden `_csrf` (D2).
    let session = establish_session(harness, http, &email, &password).await;
    let outcome = get_with_cookie(harness, http, "/", &session).await;
    world.dash_session_cookie = Some(session);
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

/// US-03: the instance-admin link is an `<a href="…">` targeting the shipped
/// instance surface — asserting on the `href` (not a bare substring) so it is a
/// LINK, not incidental copy.
fn instance_admin_href(path: &str) -> String {
    format!("href=\"{path}\"")
}

#[then(regex = r#"^the response body contains a link to "([^"]+)"$"#)]
async fn body_contains_link_to(world: &mut FoundryWorld, path: String) {
    let needle = instance_admin_href(&path);
    let body = world.last_body.as_deref().unwrap_or("");
    assert!(
        body.contains(&needle),
        "response body missing a link to {path:?} (expected {needle:?}): {body:?}"
    );
}

#[then(regex = r#"^the response body does not contain a link to "([^"]+)"$"#)]
async fn body_does_not_contain_link_to(world: &mut FoundryWorld, path: String) {
    let needle = instance_admin_href(&path);
    let body = world.last_body.as_deref().unwrap_or("");
    assert!(
        !body.contains(&needle),
        "response body contained a link to {path:?} it must NOT expose (found {needle:?}): {body:?}"
    );
}

// ----- US-02 sign out -----------------------------------------------------

/// Pull the `value="…"` of the FIRST `<input … name="{field}" …>` in `body`.
/// Attribute order in the rendered form is `type="hidden" name="_csrf"
/// value="…"`, so `value=` follows `name=`.
fn hidden_input_value(body: &str, field: &str) -> Option<String> {
    let marker = format!("name=\"{field}\"");
    let after = &body[body.find(&marker)? + marker.len()..];
    let value_start = after.find("value=\"")? + "value=\"".len();
    let rest = &after[value_start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// The `foundry_csrf` value from the response's `Set-Cookie` header(s).
fn set_cookie_csrf(headers: &reqwest::header::HeaderMap) -> Option<String> {
    headers
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .and_then(|s| s.strip_prefix("foundry_csrf="))
        .and_then(|rest| rest.split(';').next())
        .map(str::to_string)
}

#[then(regex = r#"^the response body contains a sign-out form posting to "([^"]+)"$"#)]
async fn body_contains_signout_form(world: &mut FoundryWorld, action: String) {
    let body = world.last_body.as_deref().unwrap_or("");
    let needle = format!("<form method=\"post\" action=\"{action}\">");
    assert!(
        body.contains(&needle),
        "response body missing a sign-out form {needle:?}: {body:?}"
    );
}

#[then(regex = r#"^the sign-out form carries a "_csrf" token matching the "foundry_csrf" cookie$"#)]
async fn signout_form_csrf_matches_cookie(world: &mut FoundryWorld) {
    let body = world.last_body.as_deref().unwrap_or("");
    let form_token =
        hidden_input_value(body, "_csrf").expect("sign-out form must carry a hidden _csrf field");
    let headers = world.last_headers.as_ref().expect("headers captured");
    let cookie_token = set_cookie_csrf(headers)
        .expect("the dashboard visit must mint a foundry_csrf Set-Cookie (D2)");
    assert_eq!(
        form_token, cookie_token,
        "the form _csrf token must match the foundry_csrf cookie on the SAME response \
         (double-submit): form={form_token:?} cookie={cookie_token:?}"
    );
    // Replay the matched token on the sign-out POST (form field + cookie).
    world.dash_csrf_token = Some(form_token);
}

#[when(regex = r#"^(\w+) submits the sign-out form$"#)]
async fn submits_signout_form(world: &mut FoundryWorld, _who: String) {
    let session = world
        .dash_session_cookie
        .clone()
        .expect("a session was established by the dashboard visit");
    let token = world
        .dash_csrf_token
        .clone()
        .expect("the sign-out form's _csrf token was captured");
    let cookie = format!("{session}; foundry_csrf={token}");
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let outcome = post_with_cookie(harness, http, "/sign-out", &cookie, &[("_csrf", &token)]).await;
    world.last_status = Some(outcome.status);
    world.last_headers = Some(outcome.headers);
    world.last_body = Some(outcome.body);
}

#[then(regex = r#"^requesting "/" with the old session redirects to "([^"]+)"$"#)]
async fn old_session_redirects(world: &mut FoundryWorld, location: String) {
    let session = world
        .dash_session_cookie
        .clone()
        .expect("a session was established by the dashboard visit");
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let outcome = get_with_cookie(harness, http, "/", &session).await;
    assert_eq!(
        outcome.status,
        StatusCode::SEE_OTHER,
        "the destroyed session must no longer reach the dashboard (expected 303): {outcome:?}"
    );
    let loc = outcome
        .headers
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert_eq!(
        loc, location,
        "the old session must redirect to {location:?}, got {loc:?}"
    );
}

#[when(regex = r#"^(\w+) posts to "/sign-out" with a "_csrf" that does not match the cookie$"#)]
async fn forged_signout_post(world: &mut FoundryWorld, _who: String) {
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
    let session = establish_session(harness, http, &email, &password).await;
    // A well-formed but MISMATCHED double-submit: the cookie token and the form
    // token are both non-empty and differ, so `csrf_middleware` refuses (403)
    // BEFORE the request reaches `submit_signout` — the session is never flushed.
    let cookie = format!("{session}; foundry_csrf=cookie-side-token-aaaa");
    let outcome = post_with_cookie(
        harness,
        http,
        "/sign-out",
        &cookie,
        &[("_csrf", "form-side-token-bbbb")],
    )
    .await;
    world.dash_session_cookie = Some(session);
    world.last_status = Some(outcome.status);
    world.last_headers = Some(outcome.headers);
    world.last_body = Some(outcome.body);
}

#[then(regex = r"^the request is refused by CSRF middleware$")]
async fn refused_by_csrf(world: &mut FoundryWorld) {
    let status = world.last_status.expect("status captured");
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a mismatched double-submit token must be refused 403 by csrf_middleware"
    );
    let body = world.last_body.as_deref().unwrap_or("");
    assert!(
        body.contains("CSRF"),
        "the refusal body must name the CSRF failure, got {body:?}"
    );
}

#[then(regex = r#"^(\w+)'s session is still valid$"#)]
async fn session_still_valid(world: &mut FoundryWorld, _who: String) {
    let session = world
        .dash_session_cookie
        .clone()
        .expect("a session was established before the forged post");
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let outcome = get_with_cookie(harness, http, "/", &session).await;
    assert_eq!(
        outcome.status,
        StatusCode::OK,
        "the refused sign-out must NOT have destroyed the session (expected 200): {outcome:?}"
    );
    assert!(
        outcome.body.contains("<h1>Foundry</h1>"),
        "the still-valid session must render the dashboard: {:?}",
        outcome.body
    );
}

// ----- US-05 project list (AC-05.3) ---------------------------------------

/// A project card is the `<span class="card__key">KEY</span>` +
/// `<span class="card__title">NAME</span>` pair `dashboard_root.html` renders per
/// project — asserting on BOTH spans (not a bare substring) so it is the project
/// card, not incidental copy.
#[then(regex = r#"^the response body contains a project card "([^"]+)" for "([^"]+)"$"#)]
async fn body_contains_project_card(world: &mut FoundryWorld, key_prefix: String, name: String) {
    let body = world.last_body.as_deref().unwrap_or("");
    let key_span = format!("<span class=\"card__key\">{key_prefix}</span>");
    let title_span = format!("<span class=\"card__title\">{name}</span>");
    assert!(
        body.contains(&key_span),
        "response body missing the project card key {key_span:?}: {body:?}"
    );
    assert!(
        body.contains(&title_span),
        "response body missing the project card title {title_span:?}: {body:?}"
    );
}

/// The project card's `<a>` targets the project board — asserted on the `href`
/// so it is a real navigable link, not incidental text.
#[then(regex = r#"^that card links to "([^"]+)"$"#)]
async fn card_links_to(world: &mut FoundryWorld, path: String) {
    let body = world.last_body.as_deref().unwrap_or("");
    let needle = format!("href=\"{path}\"");
    assert!(
        body.contains(&needle),
        "response body missing the project card link {needle:?}: {body:?}"
    );
}

// ----- US-04 styles promoted to the vendored stylesheet (AC-04.1–.4) ------

/// Extract the FIRST `/static/css/foundry.<hash>.css` path the layout links, from
/// `<link rel="stylesheet" href="…">`. Used to prove the layout references a
/// hashed stylesheet AND to fetch it — WITHOUT hard-coding the (D3 hand-bumped)
/// hash into the step, so a future re-bump doesn't touch this glue.
fn linked_stylesheet_href(body: &str) -> Option<String> {
    let prefix = "/static/css/foundry.";
    let start = body.find(prefix)?;
    let rest = &body[start..];
    let end = rest.find(".css")? + ".css".len();
    Some(rest[..end].to_string())
}

#[then(regex = r#"^the response body contains no inline "<style>" block$"#)]
async fn body_has_no_inline_style(world: &mut FoundryWorld) {
    let body = world.last_body.as_deref().unwrap_or("");
    assert!(
        !body.contains("<style>"),
        "the dashboard must carry NO inline <style> block (styles are vendored): {body:?}"
    );
}

#[then(regex = r#"^the base layout links a hashed "/static/css/foundry\.\*\.css" stylesheet$"#)]
async fn layout_links_hashed_stylesheet(world: &mut FoundryWorld) {
    let body = world.last_body.as_deref().unwrap_or("");
    let href = linked_stylesheet_href(body).unwrap_or_else(|| {
        panic!("base layout must link a /static/css/foundry.<hash>.css: {body:?}")
    });
    // A hashed name: `foundry.<hash>.css` with a non-empty hash segment.
    let hash = href
        .strip_prefix("/static/css/foundry.")
        .and_then(|rest| rest.strip_suffix(".css"))
        .unwrap_or("");
    assert!(
        !hash.is_empty(),
        "the linked stylesheet must carry a non-empty content hash, got href {href:?}"
    );
}

#[then(regex = r#"^fetching that stylesheet returns 200 and contains the "\.dash" rules$"#)]
async fn fetching_stylesheet_serves_dash_rules(world: &mut FoundryWorld) {
    let body = world.last_body.as_deref().unwrap_or("");
    let href =
        linked_stylesheet_href(body).expect("the layout must link a hashed stylesheet to fetch");
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    // `/static` is public GET-only content (mounted outside the auth layers), so
    // no cookie is needed to fetch the promoted stylesheet.
    let outcome = get_with_cookie(harness, http, &href, "").await;
    assert_eq!(
        outcome.status,
        StatusCode::OK,
        "the linked stylesheet {href:?} must be served 200, got {outcome:?}"
    );
    assert!(
        outcome.body.contains(".dash"),
        "the promoted stylesheet must contain the .dash dashboard rules: {:?}",
        outcome.body
    );
}
