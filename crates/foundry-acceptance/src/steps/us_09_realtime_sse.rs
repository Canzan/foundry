//! US-09 step definitions — realtime issue events via SSE.
//!
//! Re-uses background phrases from US-06 (`workspace_with_admin`,
//! `is signed in`, `is signed out`) and US-07 (`member_belongs_to_team`,
//! `project_exists_in_team` from US-08). New step phrases here cover:
//!
//! - opening / refusing SSE subscriptions through the real SSE handler,
//! - asserting events arrive (or do NOT arrive) within latency budgets,
//! - heartbeats from the SSE handler's keepalive timer,
//! - the @nfr-perf-03 sequential-creations fan-out scenario.
//!
//! All steps drive HTTP through the in-process axum harness; the
//! pg_listener task and broadcast channel live inside `InProcHarness`,
//! mirroring the slice-2 production wiring.

use crate::support::harness::InProcHarness;
use crate::support::heartbeat_env;
use crate::support::sse_client::{
    open_sse_subscription, open_sse_subscription_unauthenticated, SseEvent,
};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::redirect::Policy;
use secrecy::SecretString;
use std::collections::HashMap;
use std::time::{Duration, Instant};

const TEST_NOW: &str = "2026-01-15T12:00:00Z";
// All US-09 personas are seeded via the shared US-07
// `member_belongs_to_team` step, which uses MEMBER_PASSWORD for every
// inserted user. Keep the constant in sync with us_07_project_create.rs.
const MEMBER_PASSWORD: &str = "mei-correct-horse-battery-staple";

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

fn identity_for(who: &str) -> (String, String) {
    match who {
        "Mei" => ("mei@acme.com".to_string(), MEMBER_PASSWORD.to_string()),
        "Hiroshi" => ("hiroshi@acme.com".to_string(), MEMBER_PASSWORD.to_string()),
        "Rita" => (
            "rita@partners.acme.com".to_string(),
            MEMBER_PASSWORD.to_string(),
        ),
        other => panic!("no identity registered for {other:?}"),
    }
}

fn team_slug_for(_project_name: &str) -> &'static str {
    // Slice-2 fixture: every project the US-09 scenarios touch lives in
    // the "Backend" team. The "Partners" team for the @error scenario
    // never gets a project; Rita subscribes to a Backend-team project
    // and is rejected by the membership check, not by the team slug.
    // Project-to-team resolution happens via the DB lookup at SSE-open
    // time; this helper just owns the URL-composition concern.
    "backend"
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

// ---- session cookie helper ----------------------------------------------
//
// We re-implement a thin sign-in helper rather than reuse `signed_in_post`
// because that helper bakes in a POST as its terminal action. Here we
// need just the session cookie so SSE GETs can ride with it.

async fn sign_in_and_capture_cookie(
    world: &mut FoundryWorld,
    email: &str,
    password: &str,
) -> String {
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let base = harness.base_url();

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

    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("email", email.to_string());
    form.insert("password", password.to_string());
    form.insert("_csrf", csrf_token);
    let signin_resp = http
        .post(format!("{base}/sign-in"))
        .header(reqwest::header::COOKIE, csrf_pair)
        .form(&form)
        .send()
        .await
        .expect("post /sign-in for sse");
    let session_cookie = signin_resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_session="))
        .map(|s| s.to_string())
        .expect("session cookie from sign-in");
    session_cookie
        .split(';')
        .next()
        .unwrap_or(&session_cookie)
        .to_string()
}

// ---- Background extras -----------------------------------------------

#[given(regex = r"^Mei is signed out$")]
async fn mei_signed_out(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    // The other background steps may have stashed a session cookie;
    // discard it so the @anonymous scenario does not present credentials.
    world.session_cookie_header = None;
    world.us_09_mei_cookie = None;
}

// Rita's cookie is captured lazily inside `rita_subscribes` — the
// generic "(\\w+) is signed in" step from US-07 already matches
// "Rita is signed in" and stashes the persona; redeclaring it here
// would collide on cucumber-rs's globally-unique step-phrase rule.

#[given(regex = r"^the heartbeat interval is configured to (\d+) milliseconds for this scenario$")]
async fn heartbeat_interval_override(world: &mut FoundryWorld, ms: u64) {
    // Set the override BEFORE the harness spins up so AppState.sse_heartbeat_ms
    // observes the shortened interval. If the harness is already spawned
    // (some earlier step touched it), drop it and rebuild so the new
    // value lands in AppState.
    heartbeat_env::override_heartbeat_ms(ms);
    if world.harness.is_some() {
        // The Background step for the heartbeat scenario seeded a
        // workspace already; rebuilding the harness loses that seed.
        // Solution: tear down + reseed minimal context. The scenario
        // body only needs Mei signed in + the Auth v2 project; we
        // reuse the standard background steps via direct SQL replay.
        world.harness = None;
        ensure_harness(world).await;
        reseed_backend_workspace_with_mei_and_auth_v2(world).await;
    }
}

async fn reseed_backend_workspace_with_mei_and_auth_v2(world: &mut FoundryWorld) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();

    let workspace_id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'Acme Eng')")
        .bind(workspace_id)
        .execute(pool)
        .await
        .expect("seed workspace");

    let admin_id = uuid::Uuid::now_v7();
    let admin_hash = foundry_auth::hash_password(&SecretString::new(
        "admin-correct-horse-battery-staple".to_string().into(),
    ))
    .await
    .expect("hash admin pw");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, 'devansh@acme.com', 'devansh@acme.com', 'Devansh', $2)",
    )
    .bind(admin_id)
    .bind(&admin_hash)
    .execute(pool)
    .await
    .expect("seed admin");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'admin')",
    )
    .bind(workspace_id)
    .bind(admin_id)
    .execute(pool)
    .await
    .expect("seed admin membership");

    let team_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'Backend', 'backend')",
    )
    .bind(team_id)
    .bind(workspace_id)
    .execute(pool)
    .await
    .expect("seed team");

    let mei_id = uuid::Uuid::now_v7();
    let mei_hash =
        foundry_auth::hash_password(&SecretString::new(MEMBER_PASSWORD.to_string().into()))
            .await
            .expect("hash mei pw");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, 'mei@acme.com', 'mei@acme.com', 'Mei', $2)",
    )
    .bind(mei_id)
    .bind(&mei_hash)
    .execute(pool)
    .await
    .expect("seed mei");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(workspace_id)
    .bind(mei_id)
    .execute(pool)
    .await
    .expect("seed mei workspace membership");
    sqlx::query("INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'member')")
        .bind(team_id)
        .bind(mei_id)
        .execute(pool)
        .await
        .expect("seed mei team membership");

    let project_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, 'Auth v2', 'auth-v2', 'AUTH')",
    )
    .bind(project_id)
    .bind(team_id)
    .bind(workspace_id)
    .execute(pool)
    .await
    .expect("seed project");
}

// ---- When: open subscription ----------------------------------------

#[given(regex = r#"^(\w+) has an open subscription to events on "([^"]+)"$"#)]
async fn member_has_open_subscription(world: &mut FoundryWorld, who: String, project_name: String) {
    ensure_harness(world).await;
    let (email, password) = identity_for(&who);
    let cookie = sign_in_and_capture_cookie(world, &email, &password).await;
    let base = world.harness.as_ref().expect("harness").base_url();
    let project_slug = slugify(&project_name);
    let team_slug = team_slug_for(&project_name);

    let sub = open_sse_subscription(&base, &project_slug, team_slug, &cookie).await;
    assert!(
        sub.open_status.is_success(),
        "expected SSE 200, got {status}",
        status = sub.open_status
    );
    // Wait for the server's `:ready` handshake so the broadcast
    // Receiver is registered BEFORE the next When step fires a NOTIFY.
    // Without this, the NOTIFY can be sent into a zero-receiver
    // channel and lost.
    let ready = sub.wait_until_ready(Duration::from_secs(2)).await;
    assert!(ready, "SSE subscription never received :ready handshake");
    let key = (who, project_name);
    world.us_09_subscriptions.insert(key, sub);
    world.us_09_mei_cookie = Some(cookie);
}

#[when(regex = r#"^an anonymous request attempts to subscribe to events on "([^"]+)"$"#)]
async fn anonymous_subscribes(world: &mut FoundryWorld, project_name: String) {
    let base = world.harness.as_ref().expect("harness").base_url();
    let project_slug = slugify(&project_name);
    let team_slug = team_slug_for(&project_name);
    let attempt = open_sse_subscription_unauthenticated(&base, &project_slug, team_slug).await;
    world.us_09_last_open_attempt = Some(attempt);
}

#[when(regex = r#"^Rita attempts to subscribe to events on "([^"]+)"$"#)]
async fn rita_subscribes(world: &mut FoundryWorld, project_name: String) {
    ensure_harness(world).await;
    // Lazily sign Rita in here — the upstream "Rita is signed in"
    // step from US-07 only stashes the persona; it doesn't authenticate.
    let cookie = if let Some(c) = world.us_09_rita_cookie.clone() {
        c
    } else {
        let c = sign_in_and_capture_cookie(world, "rita@partners.acme.com", MEMBER_PASSWORD).await;
        world.us_09_rita_cookie = Some(c.clone());
        c
    };
    let base = world.harness.as_ref().expect("harness").base_url();
    let project_slug = slugify(&project_name);
    let team_slug = team_slug_for(&project_name);
    let sub = open_sse_subscription(&base, &project_slug, team_slug, &cookie).await;
    // For the 403 path, body wasn't captured by open_sse_subscription
    // (it discards the body on non-2xx). Re-issue a vanilla GET to
    // capture body content if a later step needs it.
    world.us_09_last_open_status = Some(sub.open_status);
    // Hold the subscription so the "no events on a closed stream"
    // assertion can call `.drain()`.
    world
        .us_09_subscriptions
        .insert(("Rita".to_string(), project_name), sub);
}

#[when(regex = r#"^(\w+) changes the state of "(\w+)-(\d+)" to "([^"]+)"$"#)]
async fn member_changes_issue_state(
    world: &mut FoundryWorld,
    who: String,
    prefix: String,
    number: i32,
    new_state: String,
) {
    ensure_harness(world).await;
    let (email, password) = identity_for(&who);
    // Sign the actor in (they may differ from the subscriber).
    let cookie = sign_in_and_capture_cookie(world, &email, &password).await;
    let base = world.harness.as_ref().expect("harness").base_url();

    // We need a CSRF cookie + token for the POST. Mint a fresh pair.
    let http = world.http.as_ref().expect("http");
    let csrf_get = http
        .get(format!("{base}/sign-in"))
        .header(reqwest::header::COOKIE, cookie.clone())
        .send()
        .await
        .expect("csrf for state change");
    let csrf_full = csrf_get
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .map(|s| s.to_string())
        .unwrap_or_default();
    let csrf_token = csrf_full
        .strip_prefix("foundry_csrf=")
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string();
    let combined = format!("{cookie}; foundry_csrf={csrf_token}");

    let project_slug = lookup_project_slug_by_prefix(world, &prefix).await;
    let team_slug = "backend";
    let url = format!("{base}/team/{team_slug}/project/{project_slug}/issues/{number}/state");
    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("state", new_state);
    form.insert("_csrf", csrf_token);
    let started = Instant::now();
    let _resp = http
        .post(&url)
        .header(reqwest::header::COOKIE, combined)
        .form(&form)
        .send()
        .await
        .expect("post state change");
    world.us_09_last_action_started_at = Some(started);
    let _ = (prefix, number);
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

#[when(regex = r#"^(\d+) milliseconds pass with no issue activity on "([^"]+)"$"#)]
async fn quiet_window(world: &mut FoundryWorld, ms: u64, _project_name: String) {
    let started = Instant::now();
    tokio::time::sleep(Duration::from_millis(ms)).await;
    world.us_09_last_action_started_at = Some(started);
}

#[when(
    regex = r#"^(\w+) files (\d+) issues against "([^"]+)" sequentially, each with a unique title, pausing (\d+) milliseconds between$"#
)]
async fn files_n_with_pause(
    world: &mut FoundryWorld,
    who: String,
    count: u32,
    project_name: String,
    pause_ms: u64,
) {
    ensure_harness(world).await;
    let (email, password) = identity_for(&who);
    let cookie = sign_in_and_capture_cookie(world, &email, &password).await;
    let base = world.harness.as_ref().expect("harness").base_url();
    let http = world.http.as_ref().expect("http");

    // Mint a CSRF pair once.
    let csrf_get = http
        .get(format!("{base}/sign-in"))
        .header(reqwest::header::COOKIE, cookie.clone())
        .send()
        .await
        .expect("csrf for perf");
    let csrf_full = csrf_get
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .map(|s| s.to_string())
        .unwrap_or_default();
    let csrf_token = csrf_full
        .strip_prefix("foundry_csrf=")
        .and_then(|rest| rest.split(';').next())
        .unwrap_or("")
        .to_string();
    let combined = format!("{cookie}; foundry_csrf={csrf_token}");
    let project_slug = slugify(&project_name);
    let url = format!("{base}/team/backend/project/{project_slug}/issues");

    let started = Instant::now();
    for i in 1..=count {
        let title = format!("Realtime perf issue #{i}");
        let mut form: HashMap<&str, String> = HashMap::new();
        form.insert("title", title);
        form.insert("_csrf", csrf_token.clone());
        let _ = http
            .post(&url)
            .header(reqwest::header::COOKIE, combined.clone())
            .form(&form)
            .send()
            .await
            .expect("post issue");
        if i < count {
            tokio::time::sleep(Duration::from_millis(pause_ms)).await;
        }
    }
    world.us_09_last_action_started_at = Some(started);
}

// ---- Then: assertions -----------------------------------------------

#[then(
    regex = r#"^within (\d+) milliseconds (\w+) observes an? "([^"]+)" event for "(\w+)-(\d+)" on "([^"]+)"$"#
)]
async fn member_observes_event_within(
    world: &mut FoundryWorld,
    timeout_ms: u64,
    who: String,
    event_type: String,
    prefix: String,
    number: i32,
    project_name: String,
) {
    let started_at = world
        .us_09_last_action_started_at
        .expect("When step captured started_at");
    let key = (who.clone(), project_name.clone());
    let sub = world
        .us_09_subscriptions
        .get(&key)
        .unwrap_or_else(|| panic!("no subscription for {who} on {project_name}"));

    let expected_key = format!("{prefix}-{number}");
    let result = sub
        .wait_for(Duration::from_millis(timeout_ms), started_at, |evt| {
            evt.event_type == event_type
                && evt
                    .payload_json
                    .as_ref()
                    .and_then(|v| v.get("key").and_then(|k| k.as_str()))
                    == Some(expected_key.as_str())
        })
        .await;
    let (evt, latency) = result.unwrap_or_else(|| {
        let drained = sub.drain();
        panic!(
            "{who} did not observe {event_type} for {expected_key} within {timeout_ms}ms; seen events: {drained:?}"
        )
    });
    assert!(
        latency <= Duration::from_millis(timeout_ms),
        "{who} observed {event_type} for {expected_key} after {latency:?} (budget {timeout_ms}ms)"
    );
    world.us_09_last_event = Some(evt);
}

#[then(regex = r#"^the event's project key is "([^"]+)"$"#)]
async fn event_project_key(world: &mut FoundryWorld, key_prefix: String) {
    let evt = world
        .us_09_last_event
        .as_ref()
        .expect("last event captured");
    let observed = evt
        .payload_json
        .as_ref()
        .and_then(|v| v.get("key").and_then(|k| k.as_str()))
        .unwrap_or("");
    assert!(
        observed.starts_with(&format!("{key_prefix}-")),
        "expected event key prefix {key_prefix:?}, got {observed:?}"
    );
}

#[then(regex = r#"^the event payload reports state "([^"]+)"$"#)]
async fn event_payload_state(world: &mut FoundryWorld, expected: String) {
    let evt = world
        .us_09_last_event
        .as_ref()
        .expect("last event captured");
    let observed_raw = evt
        .payload_json
        .as_ref()
        .and_then(|v| v.get("state").and_then(|s| s.as_str()))
        .unwrap_or("");
    // The handler stores `in_progress` (DB-friendly), the feature uses
    // `in-progress`. Normalize both sides before comparing.
    let normalize = |s: &str| s.replace('-', "_");
    assert_eq!(
        normalize(observed_raw),
        normalize(&expected),
        "expected state {expected:?}, got {observed_raw:?}"
    );
}

#[then(
    regex = r#"^within (\d+) milliseconds (\w+) has received zero events on her "([^"]+)" subscription$"#
)]
async fn member_received_zero_events(
    world: &mut FoundryWorld,
    wait_ms: u64,
    who: String,
    project_name: String,
) {
    tokio::time::sleep(Duration::from_millis(wait_ms)).await;
    let key = (who.clone(), project_name.clone());
    let sub = world
        .us_09_subscriptions
        .get(&key)
        .unwrap_or_else(|| panic!("no subscription for {who} on {project_name}"));
    let drained = sub.drain();
    assert!(
        drained.is_empty(),
        "{who} expected zero events on {project_name}, got {drained:?}"
    );
}

#[then(regex = r"^the subscription is refused with status (\d+)$")]
async fn subscription_refused_with_status(world: &mut FoundryWorld, expected_status: u16) {
    let status = world
        .us_09_last_open_attempt
        .as_ref()
        .map(|a| a.status)
        .or(world.us_09_last_open_status)
        .expect("an open attempt was captured");
    assert_eq!(
        status.as_u16(),
        expected_status,
        "expected SSE open status {expected_status}, got {status}"
    );
}

#[then(regex = r#"^(\w+) receives no events on a closed stream$"#)]
async fn member_receives_no_events_closed_stream(world: &mut FoundryWorld, who: String) {
    // For Rita the subscription was added under ("Rita", project_name);
    // pick the most recently inserted one for this persona.
    let found = world
        .us_09_subscriptions
        .iter()
        .find(|((person, _), _)| person == &who)
        .map(|(_, sub)| sub.drain())
        .unwrap_or_default();
    assert!(
        found.is_empty(),
        "{who} received unexpected events on a closed stream: {found:?}"
    );
}

#[then(regex = r"^the response body contains a sign-in prompt$")]
async fn response_contains_signin_prompt(world: &mut FoundryWorld) {
    let body = world
        .us_09_last_open_attempt
        .as_ref()
        .map(|a| a.body.as_str())
        .unwrap_or("");
    let lower = body.to_ascii_lowercase();
    assert!(
        lower.contains("sign in") || lower.contains("sign-in"),
        "expected sign-in prompt in body, got {body:?}"
    );
}

#[then(regex = r#"^(\w+)'s stream has received at least (\d+) keepalive heartbeats$"#)]
async fn stream_received_n_heartbeats(world: &mut FoundryWorld, who: String, n: u32) {
    let sub = world
        .us_09_subscriptions
        .iter()
        .find(|((person, _), _)| person == &who)
        .map(|(_, sub)| sub)
        .unwrap_or_else(|| panic!("no subscription for {who}"));
    let count = sub.heartbeat_count();
    assert!(
        count >= n,
        "{who} observed {count} heartbeats; expected at least {n}"
    );
}

#[then(
    regex = r#"^(\w+) receives (\d+) "([^"]+)" events whose keys are "(\w+)-(\d+)" through "(\w+)-(\d+)"$"#
)]
#[allow(clippy::too_many_arguments)]
async fn member_receives_n_events_with_keys(
    world: &mut FoundryWorld,
    who: String,
    count: u32,
    event_type: String,
    prefix1: String,
    first: i32,
    _prefix2: String,
    last: i32,
) {
    assert_eq!(
        (last - first + 1) as u32,
        count,
        "feature keys range {first}..={last} does not match N={count}"
    );
    let started_at = world
        .us_09_last_action_started_at
        .expect("action started_at captured");
    // The events should arrive within (count * pause + budget). Use 5s.
    let timeout = Duration::from_millis(5_000);
    let key = (who.clone(), "Auth v2".to_string());
    let sub = world
        .us_09_subscriptions
        .get(&key)
        .unwrap_or_else(|| panic!("no subscription for {who} on Auth v2"));

    // Spin until either we have `count` matching events or the deadline.
    let deadline = Instant::now() + timeout;
    loop {
        let evts: Vec<SseEvent> = sub
            .drain()
            .into_iter()
            .filter(|e| e.event_type == event_type)
            .collect();
        if evts.len() >= count as usize {
            // Validate the keys.
            for (i, evt) in evts.iter().take(count as usize).enumerate() {
                let expected_key = format!("{prefix1}-{}", first + i as i32);
                let observed = evt
                    .payload_json
                    .as_ref()
                    .and_then(|v| v.get("key").and_then(|k| k.as_str()))
                    .unwrap_or("");
                assert_eq!(
                    observed, expected_key,
                    "event #{i} key mismatch: got {observed:?}, expected {expected_key:?}"
                );
            }
            break;
        }
        if Instant::now() >= deadline {
            panic!(
                "expected {count} {event_type} events, got {got}",
                got = evts.len()
            );
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let _ = started_at;
}

#[then(regex = r"^every per-event arrival latency is at most (\d+) milliseconds$")]
async fn every_arrival_latency_at_most(world: &mut FoundryWorld, ms: u64) {
    let started_at = world
        .us_09_last_action_started_at
        .expect("action started_at captured");
    let sub = world
        .us_09_subscriptions
        .iter()
        .find(|((person, _), _)| person == "Mei")
        .map(|(_, sub)| sub)
        .expect("subscription for Mei");
    let latencies = sub.latencies_relative_to(started_at);
    let budget = Duration::from_millis(ms);
    for (i, l) in latencies.iter().enumerate() {
        assert!(
            *l <= budget,
            "event #{i} latency {l:?} exceeded budget {budget:?}"
        );
    }
}

#[then(regex = r"^the median per-event arrival latency is at most (\d+) milliseconds$")]
async fn median_arrival_latency_at_most(world: &mut FoundryWorld, ms: u64) {
    let started_at = world
        .us_09_last_action_started_at
        .expect("action started_at captured");
    let sub = world
        .us_09_subscriptions
        .iter()
        .find(|((person, _), _)| person == "Mei")
        .map(|(_, sub)| sub)
        .expect("subscription for Mei");
    let mut latencies = sub.latencies_relative_to(started_at);
    latencies.sort();
    let median = latencies
        .get(latencies.len() / 2)
        .copied()
        .unwrap_or_default();
    let budget = Duration::from_millis(ms);
    assert!(
        median <= budget,
        "median latency {median:?} exceeded budget {budget:?}"
    );
}
