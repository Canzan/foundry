//! US-05 step definitions.
//!
//! All scenarios use the in-process harness from
//! `support::harness::InProcHarness`. Per `driver.md`, scenarios spin
//! a fresh per-scenario PG schema and an axum app on an ephemeral port.

use crate::support::harness::InProcHarness;
use crate::world::FoundryWorld;
use cucumber::gherkin::Step;
use cucumber::{given, then, when};
use foundry_app::Clock;
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

const TEST_NOW: &str = "2026-01-15T12:00:00Z";

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

fn sha256(s: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(s.as_bytes());
    h.finalize().into()
}

// Background ---------------------------------------------------------------

#[given(regex = r"^a fresh Foundry instance with no workspace and no users$")]
async fn fresh_instance(world: &mut FoundryWorld) {
    // Force a brand-new harness for each scenario.
    world.harness = None;
    world.http = None;
    world.minted_tokens.clear();
    world.last_status = None;
    world.last_body = None;
    world.last_headers = None;
    world.last_invite_id = None;
    world.session_cookie_header = None;
    ensure_harness(world).await;
}

#[given(
    regex = r#"^the bootstrap token "([^"]+)" was minted (\d+) minutes? ago with a (\d+)-minute TTL$"#
)]
async fn bootstrap_token_minted(
    world: &mut FoundryWorld,
    name: String,
    minted_ago_min: i64,
    ttl_min: i64,
) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let now = harness.fake_clock.now();
    // The token was minted minted_ago_min ago and expires at minted_at + ttl.
    let minted_at = now - time::Duration::minutes(minted_ago_min);
    let expires_at = minted_at + time::Duration::minutes(ttl_min);

    // Use the scenario-supplied name as the raw token value. Real
    // production tokens are 32 random bytes URL-base64 — the harness
    // doesn't care, only the hash matters at the store boundary.
    let raw = name.clone();
    let hash = sha256(&raw);
    harness
        .app
        .state
        .store
        .insert_bootstrap_token(uuid::Uuid::now_v7(), &hash, expires_at)
        .await
        .expect("insert bootstrap token");
    world.minted_tokens.insert(name, raw);
}

// Scenario 1 — claim happy path -------------------------------------------

#[when(regex = r#"^the admin submits the bootstrap claim form via "([^"]+)" with$"#)]
async fn submit_bootstrap_form(world: &mut FoundryWorld, url: String, step: &Step) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");

    let mut form: HashMap<String, String> = HashMap::new();
    if let Some(table) = step.table.as_ref() {
        for row in &table.rows {
            if row.len() >= 2 {
                form.insert(row[0].clone(), row[1].clone());
            }
        }
    }

    let absolute = format!("{}{}", harness.base_url(), url);
    let resp = http
        .post(&absolute)
        .form(&form)
        .send()
        .await
        .expect("submit bootstrap form");
    capture_response(world, resp).await;
}

#[then(regex = r"^the response redirects the admin to the workspace dashboard$")]
async fn redirected_to_dashboard(world: &mut FoundryWorld) {
    let status = world.last_status.expect("status captured");
    assert!(
        status == StatusCode::SEE_OTHER || status == StatusCode::FOUND,
        "expected redirect, got {status}"
    );
    let headers = world.last_headers.as_ref().expect("headers captured");
    let location = headers
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        location == "/dashboard" || location.ends_with("/dashboard"),
        "expected Location header to /dashboard, got {location:?}"
    );
}

#[then(regex = r#"^the response sets a session cookie named "([^"]+)"$"#)]
async fn session_cookie_set(world: &mut FoundryWorld, cookie_name: String) {
    let headers = world.last_headers.as_ref().expect("headers captured");
    let set_cookies: Vec<String> = headers
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok().map(|s| s.to_string()))
        .collect();
    let found = set_cookies
        .iter()
        .find(|sc| sc.starts_with(&format!("{cookie_name}=")));
    assert!(
        found.is_some(),
        "no Set-Cookie named {cookie_name:?} in headers: {set_cookies:?}"
    );
    world.session_cookie_header = found.cloned();
}

#[then(regex = r"^that cookie is HttpOnly and SameSite=Lax and Secure$")]
async fn cookie_security_flags(world: &mut FoundryWorld) {
    let cookie = world
        .session_cookie_header
        .as_ref()
        .expect("session cookie captured");
    let lower = cookie.to_ascii_lowercase();
    assert!(
        lower.contains("httponly"),
        "cookie missing HttpOnly: {cookie}"
    );
    assert!(
        lower.contains("samesite=lax"),
        "cookie missing SameSite=Lax: {cookie}"
    );
    assert!(lower.contains("secure"), "cookie missing Secure: {cookie}");
}

#[then(regex = r#"^the workspace "([^"]+)" exists with (\w+) as its only admin$"#)]
async fn workspace_exists_with_admin(
    world: &mut FoundryWorld,
    ws_name: String,
    admin_display: String,
) {
    let pool = world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .pool();
    let count: (i64,) = sqlx::query_as("SELECT count(*) FROM workspaces WHERE name = $1")
        .bind(&ws_name)
        .fetch_one(pool)
        .await
        .expect("count workspace");
    assert_eq!(count.0, 1, "workspace {ws_name:?} not found exactly once");

    let admins: Vec<(String, String)> = sqlx::query_as(
        "SELECT u.display_name, wm.role
           FROM users u
           JOIN workspace_memberships wm ON wm.user_id = u.id
           JOIN workspaces w ON w.id = wm.workspace_id
          WHERE w.name = $1",
    )
    .bind(&ws_name)
    .fetch_all(pool)
    .await
    .expect("fetch admins");
    let only_admins: Vec<_> = admins.iter().filter(|(_, role)| role == "admin").collect();
    assert_eq!(
        only_admins.len(),
        1,
        "expected exactly one admin in {ws_name:?}, got {only_admins:?}"
    );
    assert_eq!(
        only_admins[0].0, admin_display,
        "admin display_name mismatch"
    );
}

#[then(regex = r#"^a default team named "([^"]+)" exists in that workspace$"#)]
async fn default_team_exists(world: &mut FoundryWorld, team_name: String) {
    let pool = world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .pool();
    let count: (i64,) = sqlx::query_as("SELECT count(*) FROM teams WHERE name = $1")
        .bind(&team_name)
        .fetch_one(pool)
        .await
        .expect("count team");
    assert!(count.0 >= 1, "team {team_name:?} not found");
}

#[then(regex = r#"^a default project named "([^"]+)" exists in the (\w+) team$"#)]
async fn default_project_exists(world: &mut FoundryWorld, project_name: String, team_name: String) {
    let pool = world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .pool();
    let count: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM projects p
           JOIN teams t ON t.id = p.team_id
          WHERE p.name = $1 AND t.name = $2",
    )
    .bind(&project_name)
    .bind(&team_name)
    .fetch_one(pool)
    .await
    .expect("count project");
    assert_eq!(
        count.0, 1,
        "project {project_name:?} in team {team_name:?} not found exactly once"
    );
}

// Scenario 2 — replayed token --------------------------------------------

#[given(regex = r#"^the admin has already claimed the workspace using "([^"]+)"$"#)]
async fn admin_already_claimed_workspace(world: &mut FoundryWorld, token_name: String) {
    ensure_harness(world).await;
    let raw = world
        .minted_tokens
        .get(&token_name)
        .cloned()
        .expect("token was minted in Background");
    let url = format!("/bootstrap?token={}", urlencoding::encode(&raw));
    let mut form = HashMap::new();
    form.insert("email", "devansh@acme.com");
    form.insert("password", "correct horse battery staple");
    form.insert("display_name", "Devansh");
    form.insert("workspace_name", "Acme Eng");
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let resp = http
        .post(format!("{}{url}", harness.base_url()))
        .form(&form)
        .send()
        .await
        .expect("first claim succeeds");
    assert!(
        resp.status() == StatusCode::SEE_OTHER || resp.status() == StatusCode::FOUND,
        "first claim should redirect, got {}",
        resp.status()
    );
}

#[when(regex = r#"^a second visitor opens the bootstrap URL "([^"]+)"$"#)]
async fn second_visit_bootstrap(world: &mut FoundryWorld, url: String) {
    visit_url(world, &url).await;
}

#[when(regex = r#"^a visitor opens the bootstrap URL "([^"]+)"$"#)]
async fn visit_bootstrap(world: &mut FoundryWorld, url: String) {
    visit_url(world, &url).await;
}

async fn visit_url(world: &mut FoundryWorld, url: &str) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");

    // Translate the Gherkin "valid-token-001" path back into the raw
    // token captured by the Background step.
    let url_real = rewrite_token(world, url);

    let resp = http
        .get(format!("{}{url_real}", harness.base_url()))
        .send()
        .await
        .expect("visit bootstrap url");
    capture_response(world, resp).await;
}

fn rewrite_token(world: &FoundryWorld, url: &str) -> String {
    if let Some(idx) = url.find("token=") {
        let (lhs, rhs) = url.split_at(idx + "token=".len());
        let raw_token_name = rhs.split('&').next().unwrap_or(rhs);
        if let Some(real) = world.minted_tokens.get(raw_token_name) {
            return format!(
                "{lhs}{}{}",
                urlencoding::encode(real),
                &rhs[raw_token_name.len()..]
            );
        }
    }
    url.to_string()
}

#[then(regex = r"^the response status is (\d+) Gone$")]
async fn response_status_gone(world: &mut FoundryWorld, code: u16) {
    let status = world.last_status.expect("status captured");
    assert_eq!(status.as_u16(), code, "expected {code} Gone, got {status}");
}

#[then(regex = r"^the page body explains the link has already been used$")]
async fn body_explains_used(world: &mut FoundryWorld) {
    let body = world.last_body.as_deref().unwrap_or("");
    assert!(
        body.to_ascii_lowercase().contains("already been used")
            || body.to_ascii_lowercase().contains("already used"),
        "page body did not explain link was already used: {body:?}"
    );
}

#[then(regex = r"^the page body explains the link has expired$")]
async fn body_explains_expired(world: &mut FoundryWorld) {
    let body = world.last_body.as_deref().unwrap_or("");
    assert!(
        body.to_ascii_lowercase().contains("expired"),
        "page body did not explain link has expired: {body:?}"
    );
}

#[then(regex = r"^no second workspace is created$")]
async fn no_second_workspace(world: &mut FoundryWorld) {
    let pool = world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .pool();
    let count: (i64,) = sqlx::query_as("SELECT count(*) FROM workspaces")
        .fetch_one(pool)
        .await
        .expect("count workspaces");
    assert_eq!(
        count.0, 1,
        "expected exactly one workspace, got {}",
        count.0
    );
}

#[then(regex = r"^no workspace, user, or session is created$")]
async fn no_state_created(world: &mut FoundryWorld) {
    let pool = world
        .harness
        .as_ref()
        .expect("harness")
        .app
        .state
        .store
        .pool();
    let ws_count: (i64,) = sqlx::query_as("SELECT count(*) FROM workspaces")
        .fetch_one(pool)
        .await
        .expect("count ws");
    let user_count: (i64,) = sqlx::query_as("SELECT count(*) FROM users")
        .fetch_one(pool)
        .await
        .expect("count users");
    assert_eq!(ws_count.0, 0, "workspaces should be empty");
    assert_eq!(user_count.0, 0, "users should be empty");
    // No session header should have been captured.
    let cookies: Vec<String> = world
        .last_headers
        .as_ref()
        .map(|h| {
            h.get_all(reqwest::header::SET_COOKIE)
                .iter()
                .filter_map(|v| v.to_str().ok().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        !cookies.iter().any(|c| c.starts_with("foundry_session=")),
        "no foundry_session cookie should be set on the error response, got {cookies:?}"
    );
}

// Invite link -------------------------------------------------------------

#[given(regex = r#"^the admin has claimed "([^"]+)" and is signed in$"#)]
async fn admin_claimed_and_signed_in(world: &mut FoundryWorld, ws_name: String) {
    ensure_harness(world).await;
    // Ensure a valid bootstrap token exists.
    if world.minted_tokens.is_empty() {
        bootstrap_token_minted(world, "valid-token-claim".into(), 1, 30).await;
    }
    let token_name = world
        .minted_tokens
        .keys()
        .next()
        .cloned()
        .expect("at least one token");
    let raw = world.minted_tokens.get(&token_name).cloned().unwrap();
    let url = format!("/bootstrap?token={}", urlencoding::encode(&raw));
    let mut form = HashMap::new();
    form.insert("email", "devansh@acme.com");
    form.insert("password", "correct horse battery staple");
    form.insert("display_name", "Devansh");
    let ws_owned = ws_name.clone();
    form.insert("workspace_name", &ws_owned);

    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let resp = http
        .post(format!("{}{url}", harness.base_url()))
        .form(&form)
        .send()
        .await
        .expect("claim succeeds");
    let headers = resp.headers().clone();
    capture_response_from_parts(world, resp).await;
    // Capture the session cookie so subsequent steps can present it.
    for v in headers.get_all(reqwest::header::SET_COOKIE) {
        if let Ok(s) = v.to_str() {
            if s.starts_with("foundry_session=") {
                world.session_cookie_header = Some(s.to_string());
            }
        }
    }
}

#[when(regex = r"^the admin opens the invite-teammates panel and requests a shareable link$")]
async fn request_invite_link(world: &mut FoundryWorld) {
    let cookie = session_cookie_value(world).expect("session cookie captured");
    let session_pair = format!("foundry_session={cookie}");
    let (csrf_token, combined) = ensure_csrf_for(world, &session_pair).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let mut form = HashMap::new();
    form.insert("_csrf", csrf_token);
    let resp = http
        .post(format!("{}/invites", harness.base_url()))
        .header(reqwest::header::COOKIE, combined)
        .form(&form)
        .send()
        .await
        .expect("request invite link");
    capture_response(world, resp).await;
}

#[when(regex = r#"^the admin sends an email invite to "([^"]+)"$"#)]
async fn send_email_invite(world: &mut FoundryWorld, email: String) {
    let cookie = session_cookie_value(world).expect("session cookie captured");
    let session_pair = format!("foundry_session={cookie}");
    let (csrf_token, combined) = ensure_csrf_for(world, &session_pair).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let mut form = HashMap::new();
    form.insert("email", email);
    form.insert("_csrf", csrf_token);
    let resp = http
        .post(format!("{}/invites", harness.base_url()))
        .header(reqwest::header::COOKIE, combined)
        .form(&form)
        .send()
        .await
        .expect("send email invite");
    capture_response(world, resp).await;
}

#[given(regex = r"^the SMTP transport is configured$")]
async fn smtp_configured(_world: &mut FoundryWorld) {
    // No-op: harness always wires FakeEmailSender, which behaves as a
    // "configured" SMTP transport for assertion purposes.
}

#[then(regex = r#"^exactly one email is recorded as sent to "([^"]+)"$"#)]
async fn one_email_sent(world: &mut FoundryWorld, email: String) {
    let harness = world.harness.as_ref().expect("harness");
    let count = harness.fake_email.count_to(&email);
    assert_eq!(count, 1, "expected exactly 1 email to {email}, got {count}");
}

#[then(regex = r"^the recorded email body contains a signed invite link$")]
async fn email_body_contains_link(world: &mut FoundryWorld) {
    let harness = world.harness.as_ref().expect("harness");
    let last = harness
        .fake_email
        .last_to("mei@acme.com")
        .expect("last email present");
    assert!(
        last.body.contains("/invites/accept?id="),
        "email body missing invite link: {body:?}",
        body = last.body
    );
    assert!(
        last.body.contains("sig="),
        "email body missing signature param: {body:?}",
        body = last.body
    );
}

#[then(regex = r"^the response contains an invite URL$")]
async fn response_contains_invite_url(world: &mut FoundryWorld) {
    let body = world.last_body.as_deref().unwrap_or("");
    let id = extract_invite_id(body).expect("invite id present in response body");
    world.last_invite_id = Some(id);
    assert!(
        body.contains("/invites/accept?id="),
        "response body missing /invites/accept link"
    );
}

#[then(regex = r"^the invite URL carries a signed token parameter$")]
async fn invite_url_signed_token(world: &mut FoundryWorld) {
    let body = world.last_body.as_deref().unwrap_or("");
    assert!(
        body.contains("sig="),
        "response body missing signature param: {body:?}"
    );
}

#[then(regex = r"^the invite is recorded as valid for 7 days$")]
async fn invite_valid_7d(world: &mut FoundryWorld) {
    let harness = world.harness.as_ref().expect("harness");
    let invite_id = world.last_invite_id.expect("invite id captured");
    let expires_at = harness
        .app
        .state
        .store
        .invite_expires_at(invite_id)
        .await
        .expect("query invite")
        .expect("invite row present");
    let now = harness.fake_clock.now();
    let delta: time::Duration = expires_at - now;
    let days = delta.whole_seconds() as f64 / 86_400.0;
    assert!(
        (days - 7.0).abs() < 0.01,
        "expected invite TTL ~7 days, got {days}"
    );
}

// Shared HTTP-status assertion (used by the us-07 duplicate-project-key 409
// scenario; the legacy single-workspace 409 scenario that also used it was
// retired by ADR-003 / step 03-01).
#[then(regex = r"^the response status is 409 Conflict$")]
async fn status_409(world: &mut FoundryWorld) {
    let status = world.last_status.expect("status captured");
    assert_eq!(status.as_u16(), 409, "expected 409, got {status}");
}

// helpers -----------------------------------------------------------------

async fn capture_response(world: &mut FoundryWorld, resp: reqwest::Response) {
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = resp.text().await.unwrap_or_default();
    world.last_status = Some(status);
    world.last_headers = Some(headers);
    world.last_body = Some(body);
}

async fn capture_response_from_parts(world: &mut FoundryWorld, resp: reqwest::Response) {
    capture_response(world, resp).await
}

fn session_cookie_value(world: &FoundryWorld) -> Option<String> {
    let raw = world.session_cookie_header.as_deref()?;
    let after_eq = raw.strip_prefix("foundry_session=")?;
    Some(after_eq.split(';').next().unwrap_or("").to_string())
}

/// Hit a GET form page presenting `session_cookie_pair` (e.g.
/// `"foundry_session=...")` so the CSRF cookie is issued. Return the
/// extracted token plus a `Cookie` header that combines the session
/// cookie with the new CSRF cookie, ready to attach to a POST.
async fn ensure_csrf_for(world: &mut FoundryWorld, session_cookie_pair: &str) -> (String, String) {
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    // /sign-in is a public GET that mints a CSRF cookie. The form
    // exists in slice 1 and does not require a session.
    let resp = http
        .get(format!("{}/sign-in", harness.base_url()))
        .header(reqwest::header::COOKIE, session_cookie_pair.to_string())
        .send()
        .await
        .expect("get /sign-in for csrf");
    let raw = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .map(|s| s.to_string())
        .expect("GET /sign-in must mint foundry_csrf cookie");
    let token = raw
        .strip_prefix("foundry_csrf=")
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string();
    let combined = format!("{session_cookie_pair}; foundry_csrf={token}");
    (token, combined)
}

fn extract_invite_id(body: &str) -> Option<uuid::Uuid> {
    let needle = "/invites/accept?id=";
    let start = body.find(needle)? + needle.len();
    let rest = &body[start..];
    let end = rest.find(['&', '"', '<', ' ']).unwrap_or(rest.len());
    uuid::Uuid::parse_str(&rest[..end]).ok()
}
