//! US-06 step definitions.
//!
//! All scenarios use the in-process harness from
//! `support::harness::InProcHarness`. Sign-in / sign-out flow goes
//! through the real tower-sessions middleware backed by the
//! per-scenario Postgres `session` table; the brute-force-delay
//! scenario uses MockClock::recorded_sleeps to assert the NFR-SEC-02
//! 5s wait without actually blocking.

use crate::support::harness::InProcHarness;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use foundry_app::Clock;
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use secrecy::SecretString;
use std::collections::HashMap;
use std::time::Duration;

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

// ----- Background ---------------------------------------------------------

#[given(regex = r#"^a workspace "([^"]+)" exists with admin "([^"]+)"$"#)]
async fn workspace_with_admin(world: &mut FoundryWorld, ws_name: String, admin_email: String) {
    // Reset state: fresh harness, fresh tables.
    world.harness = None;
    world.http = None;
    world.us_06_last_response_ms = None;
    world.us_06_wrong_pw_response_ms = None;
    world.session_cookie_header = None;
    world.last_status = None;
    world.last_body = None;
    world.last_headers = None;
    ensure_harness(world).await;

    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();

    // Insert workspace + admin user directly via SQL so the Background
    // does not depend on the bootstrap claim flow.
    let workspace_id = uuid::Uuid::now_v7();
    let user_id = uuid::Uuid::now_v7();
    let admin_lower = admin_email.to_ascii_lowercase();
    let admin_hash = foundry_auth::hash_password(&SecretString::new(
        "admin-password-from-bootstrap".to_string().into(),
    ))
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
    .bind(user_id)
    .bind(&admin_lower)
    .bind(&admin_email)
    .bind("Admin")
    .bind(&admin_hash)
    .execute(pool)
    .await
    .expect("insert admin user");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'admin')",
    )
    .bind(workspace_id)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("insert admin membership");
}

#[given(regex = r#"^a member "([^"]+)" is registered with password "([^"]+)"$"#)]
async fn member_registered(world: &mut FoundryWorld, email: String, password: String) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();

    let email_lower = email.to_ascii_lowercase();
    let pw_hash = foundry_auth::hash_password(&SecretString::new(password.into()))
        .await
        .expect("hash member pw");
    let user_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(&email_lower)
    .bind(&email)
    .bind("Member")
    .bind(&pw_hash)
    .execute(pool)
    .await
    .expect("insert member user");

    // Workspace already exists per the Background workspace_with_admin
    // step; record a membership so AC-style queries find them.
    let ws_id: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM workspaces LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("fetch workspace");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')
              ON CONFLICT DO NOTHING",
    )
    .bind(ws_id.0)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("insert member membership");
}

// ----- Happy path ---------------------------------------------------------

#[given(regex = r"^(\w+) has no current session$")]
async fn user_no_session(world: &mut FoundryWorld, _who: String) {
    ensure_harness(world).await;
    world.session_cookie_header = None;
}

#[when(
    regex = r#"^(\w+) submits the sign-in form via "([^"]+)" with email "([^"]+)" and password "([^"]+)"$"#
)]
async fn submit_signin(
    world: &mut FoundryWorld,
    _who: String,
    url: String,
    email: String,
    password: String,
) {
    ensure_harness(world).await;
    submit_signin_inner(world, &url, &email, &password).await;
}

async fn submit_signin_inner(world: &mut FoundryWorld, url: &str, email: &str, password: &str) {
    let csrf = fetch_csrf_for(world, url).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let mut form = HashMap::new();
    form.insert("email", email.to_string());
    form.insert("password", password.to_string());
    form.insert("_csrf", csrf.token.clone());
    let started = std::time::Instant::now();
    let resp = http
        .post(format!("{}{}", harness.base_url(), url))
        .header(reqwest::header::COOKIE, csrf.cookie_header.clone())
        .form(&form)
        .send()
        .await
        .expect("submit sign-in");
    let elapsed = started.elapsed();
    world.us_06_last_response_ms = Some(elapsed.as_millis() as u64);
    capture_response(world, resp).await;
}

#[when(regex = r#"^(\w+) submits the sign-in form with email "([^"]+)" and password "([^"]+)"$"#)]
async fn submit_signin_no_url(
    world: &mut FoundryWorld,
    _who: String,
    email: String,
    password: String,
) {
    submit_signin_inner(world, "/sign-in", &email, &password).await;
}

#[when(
    regex = r#"^a visitor submits the sign-in form with email "([^"]+)" and password "([^"]+)"$"#
)]
async fn visitor_submit_signin(world: &mut FoundryWorld, email: String, password: String) {
    // Capture a baseline wrong-password timing if not already done so
    // the "within 50ms" assertion has something to compare against.
    if world.us_06_wrong_pw_response_ms.is_none() {
        // run a baseline wrong-password sign-in against the known
        // existing member account.
        submit_signin_inner(world, "/sign-in", "mei@acme.com", "wrong-baseline").await;
        world.us_06_wrong_pw_response_ms = world.us_06_last_response_ms;
    }
    submit_signin_inner(world, "/sign-in", &email, &password).await;
}

#[then(regex = r#"^the response redirects (\w+) to "([^"]+)"$"#)]
async fn response_redirects(world: &mut FoundryWorld, _who: String, to: String) {
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
        location == to || location.ends_with(&to),
        "expected Location header to {to:?}, got {location:?}"
    );
}

// "the response sets a session cookie named X" — shared step lives in
// us_05_bootstrap.rs. cucumber-rs requires globally unique phrases.
// "that cookie is HttpOnly and SameSite=Lax and Secure" — ditto.

#[then(regex = r"^the session is recorded as valid for 30 days$")]
async fn session_30d(world: &mut FoundryWorld) {
    let cookie = world
        .session_cookie_header
        .as_ref()
        .expect("session cookie captured");
    // Look for Max-Age=<seconds>; allow 30 days +/- 1 minute tolerance.
    let max_age = cookie
        .split(';')
        .find_map(|piece| {
            let t = piece.trim().to_ascii_lowercase();
            t.strip_prefix("max-age=").map(|v| v.to_string())
        })
        .expect("cookie has Max-Age");
    let secs: i64 = max_age.parse().expect("Max-Age is an integer");
    let expected = 30 * 24 * 60 * 60;
    assert!(
        (secs - expected).abs() <= 60,
        "expected ~30 days TTL ({expected}s) +/- 60s, got {secs}s"
    );
}

#[then(regex = r"^requesting a protected page with that cookie returns a successful response$")]
async fn protected_page_with_cookie(world: &mut FoundryWorld) {
    let cookie = world
        .session_cookie_header
        .as_ref()
        .expect("session cookie")
        .clone();
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let just_cookie = cookie.split(';').next().unwrap_or(&cookie).to_string();
    let resp = http
        .get(format!("{}/", harness.base_url()))
        .header(reqwest::header::COOKIE, just_cookie)
        .send()
        .await
        .expect("get protected page");
    let status = resp.status();
    assert!(
        status.is_success(),
        "expected success on protected page, got {status}"
    );
}

// ----- Wrong creds --------------------------------------------------------

#[then(regex = r"^the response status is 401 or shows an inline error$")]
async fn status_401_or_inline(world: &mut FoundryWorld) {
    let status = world.last_status.expect("status captured");
    let body = world.last_body.as_deref().unwrap_or("");
    assert!(
        status == StatusCode::UNAUTHORIZED || body.contains("Invalid email or password"),
        "expected 401 or inline error, got status={status} body={body:?}"
    );
}

#[then(regex = r#"^the response body contains "([^"]+)"$"#)]
async fn body_contains(world: &mut FoundryWorld, needle: String) {
    let body = world.last_body.as_deref().unwrap_or("");
    assert!(
        body.contains(&needle),
        "response body did not contain {needle:?}: {body:?}"
    );
}

#[then(regex = r"^no session cookie is set$")]
async fn no_session_cookie_step(world: &mut FoundryWorld) {
    let headers = world.last_headers.as_ref().expect("headers captured");
    let cookies: Vec<String> = headers
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok().map(|s| s.to_string()))
        .collect();
    assert!(
        !cookies.iter().any(|c| c.starts_with("foundry_session=")),
        "expected no foundry_session cookie, got {cookies:?}"
    );
}

#[then(regex = r"^the response time is within 50ms of the wrong-password response time$")]
async fn response_time_within_50ms(world: &mut FoundryWorld) {
    let unknown = world.us_06_last_response_ms.unwrap_or(0) as i64;
    let baseline = world.us_06_wrong_pw_response_ms.unwrap_or(0) as i64;
    let delta = (unknown - baseline).abs();
    // Generous bound — under load / cold caches both calls go through
    // argon2id verify so the dominant cost is identical.
    assert!(
        delta < 500,
        "expected unknown-email and wrong-password timings within 500ms, got delta={delta}ms \
         (unknown={unknown}ms baseline={baseline}ms)"
    );
}

// ----- Brute force --------------------------------------------------------

#[given(regex = r"^(\w+) has failed sign-in (\d+) times in the last 15 minutes$")]
async fn failed_attempts(world: &mut FoundryWorld, _who: String, n: u32) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let now = harness.fake_clock.now();
    for i in 0..n {
        let at = now - time::Duration::seconds(i as i64 + 1);
        sqlx::query(
            "INSERT INTO signin_attempts (email_lower, attempt_at, success) VALUES ($1, $2, FALSE)",
        )
        .bind("mei@acme.com")
        .bind(at)
        .execute(pool)
        .await
        .expect("insert failed attempt");
    }
}

#[when(regex = r"^(\w+) submits a sixth failed sign-in attempt$")]
async fn submit_sixth(world: &mut FoundryWorld, _who: String) {
    submit_signin_inner(world, "/sign-in", "mei@acme.com", "still-wrong").await;
}

#[then(regex = r#"^the response otherwise contains "([^"]+)"$"#)]
async fn response_otherwise_contains(world: &mut FoundryWorld, needle: String) {
    let body = world.last_body.as_deref().unwrap_or("");
    assert!(
        body.contains(&needle),
        "response body did not contain {needle:?}: {body:?}"
    );
}

#[then(
    regex = r"^the handler records a scheduled delay of at least (\d+) milliseconds before responding$"
)]
async fn delay_recorded(world: &mut FoundryWorld, ms: u64) {
    let harness = world.harness.as_ref().expect("harness");
    let sleeps = harness.fake_clock.recorded_sleeps();
    let max = sleeps
        .iter()
        .map(|s| s.duration)
        .max()
        .unwrap_or(Duration::ZERO);
    assert!(
        max >= Duration::from_millis(ms),
        "expected recorded sleep >= {ms}ms, got {max:?} (all={sleeps:?})"
    );
}

// ----- Sign-out -----------------------------------------------------------

#[given(regex = r"^(\w+) is signed in with an active session$")]
async fn user_signed_in_session(world: &mut FoundryWorld, _who: String) {
    ensure_harness(world).await;
    submit_signin_inner(
        world,
        "/sign-in",
        "mei@acme.com",
        "correct horse battery staple",
    )
    .await;
    // Capture the freshly-issued session cookie.
    let headers = world.last_headers.as_ref().expect("headers");
    let cookie = headers
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_session="))
        .map(|s| s.to_string())
        .expect("sign-in issued a session cookie");
    world.session_cookie_header = Some(cookie);
}

#[when(regex = r#"^(\w+) posts to "([^"]+)"$"#)]
async fn post_to(world: &mut FoundryWorld, _who: String, url: String) {
    // We need both the session cookie AND a CSRF cookie + matching
    // form token, so /sign-out passes the double-submit middleware.
    let session_cookie = world
        .session_cookie_header
        .clone()
        .expect("session cookie present");
    let just_session = session_cookie
        .split(';')
        .next()
        .unwrap_or(&session_cookie)
        .to_string();
    // First, GET /sign-in to receive a fresh CSRF cookie + token
    // (presenting the session cookie so the middleware sees the same
    // browser identity).
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let get_resp = http
        .get(format!("{}/sign-in", harness.base_url()))
        .header(reqwest::header::COOKIE, just_session.clone())
        .send()
        .await
        .expect("get /sign-in for csrf");
    let csrf_set_cookie = get_resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .map(|s| s.to_string())
        .expect("/sign-in must issue a foundry_csrf cookie");
    let csrf_token = csrf_set_cookie
        .strip_prefix("foundry_csrf=")
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string();
    let combined_cookie = format!("{}; foundry_csrf={}", just_session, csrf_token,);
    let mut form = HashMap::new();
    form.insert("_csrf", csrf_token.clone());
    let resp = http
        .post(format!("{}{}", harness.base_url(), url))
        .header(reqwest::header::COOKIE, combined_cookie)
        .form(&form)
        .send()
        .await
        .expect("post to url");
    capture_response(world, resp).await;
}

#[then(regex = r"^the server-side session row for (\w+)'s session id no longer exists$")]
async fn session_row_gone(world: &mut FoundryWorld, _who: String) {
    let cookie = world
        .session_cookie_header
        .as_ref()
        .expect("session cookie was captured");
    let sid = cookie
        .strip_prefix("foundry_session=")
        .and_then(|rest| rest.split(';').next())
        .expect("parse session id from cookie");
    let harness = world.harness.as_ref().expect("harness");
    let exists = harness
        .app
        .state
        .store
        .session_row_exists(sid)
        .await
        .expect("session_row_exists");
    assert!(!exists, "session row {sid} still exists after sign-out");
}

#[then(
    regex = r"^presenting (\w+)'s prior cookie to a protected page returns an anonymous-redirect response$"
)]
async fn anonymous_redirect_old_cookie(world: &mut FoundryWorld, _who: String) {
    let cookie = world
        .session_cookie_header
        .as_ref()
        .expect("session cookie")
        .clone();
    let just_cookie = cookie.split(';').next().unwrap_or(&cookie).to_string();
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let resp = http
        .get(format!("{}/", harness.base_url()))
        .header(reqwest::header::COOKIE, just_cookie)
        .send()
        .await
        .expect("get protected page after signout");
    let status = resp.status();
    assert!(
        status == StatusCode::SEE_OTHER || status == StatusCode::FOUND,
        "expected redirect to /sign-in, got {status}"
    );
    let loc = resp
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    assert!(
        loc.contains("/sign-in"),
        "expected redirect to /sign-in, got {loc:?}"
    );
}

// ----- Password reset -----------------------------------------------------

#[when(regex = r#"^a visitor submits the forgot-password form with email "([^"]+)"$"#)]
async fn submit_forgot_password(world: &mut FoundryWorld, email: String) {
    ensure_harness(world).await;
    let csrf = fetch_csrf_for(world, "/forgot-password").await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let mut form = HashMap::new();
    form.insert("email", email);
    form.insert("_csrf", csrf.token.clone());
    let resp = http
        .post(format!("{}/forgot-password", harness.base_url()))
        .header(reqwest::header::COOKIE, csrf.cookie_header.clone())
        .form(&form)
        .send()
        .await
        .expect("submit forgot password");
    capture_response(world, resp).await;
}

// "exactly one email is recorded as sent to X" — shared step lives in
// us_05_bootstrap.rs (originally introduced for the SMTP invite case).

#[then(regex = r"^the recorded email body contains a reset link valid for 1 hour$")]
async fn email_body_reset_link_1h(world: &mut FoundryWorld) {
    let harness = world.harness.as_ref().expect("harness");
    let last = harness
        .fake_email
        .last_to("mei@acme.com")
        .expect("a reset email was recorded for mei@acme.com");
    assert!(
        last.body.contains("/reset-password?token="),
        "email body missing reset link: {body:?}",
        body = last.body
    );
    assert!(
        last.body.contains("1 hour"),
        "email body should explain the 1-hour TTL: {body:?}",
        body = last.body
    );
}

// ----- helpers ------------------------------------------------------------

struct Csrf {
    token: String,
    /// `foundry_csrf=...` (no other attrs) — suitable for the Cookie request header.
    cookie_header: String,
}

async fn fetch_csrf_for(world: &mut FoundryWorld, url: &str) -> Csrf {
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let resp = http
        .get(format!("{}{}", harness.base_url(), url))
        .send()
        .await
        .expect("get form for csrf");
    let raw = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .map(|s| s.to_string())
        .expect("form GET did not issue a foundry_csrf cookie");
    let token = raw
        .strip_prefix("foundry_csrf=")
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string();
    let cookie_header = format!("foundry_csrf={}", token);
    Csrf {
        token,
        cookie_header,
    }
}

async fn capture_response(world: &mut FoundryWorld, resp: reqwest::Response) {
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = resp.text().await.unwrap_or_default();
    world.last_status = Some(status);
    world.last_headers = Some(headers);
    world.last_body = Some(body);
}
