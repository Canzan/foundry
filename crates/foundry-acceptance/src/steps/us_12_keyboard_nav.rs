//! US-12 step definitions — keyboard-nav server contracts.
//!
//! The 6 automated scenarios pin the data-attribute + endpoint contracts
//! the alpine.js client handlers depend on:
//!
//! - `GET /team/{team}/project/{project_slug}/issues/new` —
//!   modal-shaped htmx fragment when `HX-Request: true` is present.
//! - `GET /team/{team}/project/{project_slug}/search?q=...` —
//!   filtered issue list (matches exact key or title substring).
//! - `GET /keyboard-help` — shortcut-help overlay listing every shortcut.
//! - `GET /team/{team}/project/{project_slug}` (existing) — must render each
//!   card carrying a `data-issue-key` attribute, which is what the client
//!   keyboard layer resolves a selection to (ADR-004).
//!
//! Background phrases are inherited from US-06/07/08 (workspace,
//! membership, signed-in, project, issue range). Re-declaring them
//! would collide on cucumber-rs's globally-unique step-phrase rule.
//!
//! Implementation pattern (mirrors US-10):
//!   When step → sign in → GET → cache body in `world.us_12_last_get_body`
//!   Then step(s) → parse cached body with scraper-based helpers.

use crate::support::harness::InProcHarness;
use crate::support::html_assertions::assert_has;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::redirect::Policy;
use scraper::{Html, Selector};
use std::collections::HashMap;

const TEST_NOW: &str = "2026-01-15T12:00:00Z";
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
        other => panic!("no identity registered for {other:?}"),
    }
}

/// Project-name → URL slug. Mirrors the slugify in projects.rs (kept
/// inline so we don't reach across crates for a 12-line helper).
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

/// Sign in and capture the `foundry_session=...` cookie pair. Mirrors
/// the US-10 helper.
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
        .expect("post /sign-in");
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

// --- Given: seed extra titled issues -------------------------------

#[given(regex = r#"^the "([^"]+)" project already has an issue titled "([^"]+)"$"#)]
async fn project_already_has_issue_titled(
    world: &mut FoundryWorld,
    project_name: String,
    title: String,
) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    // Project lookup by name — slice-2 has one project per scenario in
    // these tests so the name is unique.
    let row: (uuid::Uuid, String, uuid::Uuid) = sqlx::query_as(
        "SELECT p.id, p.key_prefix, p.workspace_id
           FROM projects p
          WHERE p.name = $1",
    )
    .bind(&project_name)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|err| panic!("project {project_name:?} not found: {err}"));
    let (project_id, key_prefix, workspace_id) = row;

    // Pick the next available number for that project.
    let next: (Option<i32>,) =
        sqlx::query_as("SELECT MAX(number) FROM issues WHERE project_id = $1")
            .bind(project_id)
            .fetch_one(pool)
            .await
            .expect("max issue number");
    let number = next.0.unwrap_or(0) + 1;

    // We need an author_id. Pull the first workspace admin — these
    // scenarios don't care who authored the seeded issue, only that it
    // exists and is searchable.
    let author: (uuid::Uuid,) = sqlx::query_as(
        "SELECT user_id FROM workspace_memberships
          WHERE workspace_id = $1 ORDER BY user_id LIMIT 1",
    )
    .bind(workspace_id)
    .fetch_one(pool)
    .await
    .expect("workspace member for author");

    let issue_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO issues (id, workspace_id, project_id, number, title, state, priority, author_id)
              VALUES ($1, $2, $3, $4, $5, 'backlog', 'medium', $6)",
    )
    .bind(issue_id)
    .bind(workspace_id)
    .bind(project_id)
    .bind(number)
    .bind(&title)
    .bind(author.0)
    .execute(pool)
    .await
    .expect("seed extra issue");

    // Avoid an unused warning in the unused-prefix path.
    let _ = key_prefix;
}

// --- When: server-contract GETs ------------------------------------

#[when(regex = r#"^(\w+) opens the project board for "([^"]+)"$"#)]
async fn member_opens_project_board(world: &mut FoundryWorld, who: String, project_name: String) {
    ensure_harness(world).await;
    let (email, password) = identity_for(&who);
    let cookie = sign_in_and_capture_cookie(world, &email, &password).await;
    let base = world.harness.as_ref().expect("harness").base_url();
    let project_slug = slugify(&project_name);
    let team_slug = "backend";
    let url = format!("{base}/team/{team_slug}/project/{project_slug}");
    let http = world.http.as_ref().expect("http");
    let resp = http
        .get(&url)
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("get project board");
    world.last_status = Some(resp.status());
    world.us_12_last_get_body = Some(resp.text().await.unwrap_or_default());
}

#[when(regex = r#"^(\w+) requests the new-issue modal for "([^"]+)" as an htmx request$"#)]
async fn member_requests_new_issue_modal(
    world: &mut FoundryWorld,
    who: String,
    project_name: String,
) {
    ensure_harness(world).await;
    let (email, password) = identity_for(&who);
    let cookie = sign_in_and_capture_cookie(world, &email, &password).await;
    let base = world.harness.as_ref().expect("harness").base_url();
    let project_slug = slugify(&project_name);
    let team_slug = "backend";
    let url = format!("{base}/team/{team_slug}/project/{project_slug}/issues/new");
    let http = world.http.as_ref().expect("http");
    let resp = http
        .get(&url)
        .header(reqwest::header::COOKIE, cookie)
        .header("hx-request", "true")
        .send()
        .await
        .expect("get new-issue modal");
    world.last_status = Some(resp.status());
    world.us_12_last_get_body = Some(resp.text().await.unwrap_or_default());
}

#[when(regex = r#"^(\w+) searches "([^"]+)" for the query "([^"]+)"$"#)]
async fn member_searches_project(
    world: &mut FoundryWorld,
    who: String,
    project_name: String,
    query: String,
) {
    ensure_harness(world).await;
    let (email, password) = identity_for(&who);
    let cookie = sign_in_and_capture_cookie(world, &email, &password).await;
    let base = world.harness.as_ref().expect("harness").base_url();
    let project_slug = slugify(&project_name);
    let team_slug = "backend";
    let encoded = urlencoding_minimal(&query);
    let url = format!("{base}/team/{team_slug}/project/{project_slug}/search?q={encoded}");
    let http = world.http.as_ref().expect("http");
    let resp = http
        .get(&url)
        .header(reqwest::header::COOKIE, cookie)
        .header("hx-request", "true")
        .send()
        .await
        .expect("get search");
    world.last_status = Some(resp.status());
    world.us_12_last_get_body = Some(resp.text().await.unwrap_or_default());
}

/// Minimal URL-encoder for the query string. The fixture queries are
/// short identifiers + words so we only need to escape space + a handful
/// of reserved characters; pulling in `urlencoding` for this is overkill.
fn urlencoding_minimal(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

#[when(regex = r"^(\w+) requests the keyboard-help overlay$")]
async fn member_requests_keyboard_help(world: &mut FoundryWorld, _who: String) {
    ensure_harness(world).await;
    let base = world.harness.as_ref().expect("harness").base_url();
    let http = world.http.as_ref().expect("http");
    // /keyboard-help is intentionally public (no session required) so
    // the alpine.js bootstrap can request it before the user opens any
    // team-scoped page.
    let resp = http
        .get(format!("{base}/keyboard-help"))
        .send()
        .await
        .expect("get keyboard-help");
    world.last_status = Some(resp.status());
    world.us_12_last_get_body = Some(resp.text().await.unwrap_or_default());
}

// --- Then: data attributes + fragment shape ------------------------

#[then(
    regex = r#"^the rendered page contains an element with attribute data-issue-key="(\w+)-(\d+)"$"#
)]
async fn page_contains_data_issue_key(world: &mut FoundryWorld, prefix: String, number: i32) {
    let body = world
        .us_12_last_get_body
        .as_ref()
        .expect("body captured by When");
    let css = format!(r#"[data-issue-key="{prefix}-{number}"]"#);
    assert_has(body, &css);
}

#[then(regex = r#"^the response is an htmx fragment containing a form posting to "([^"]+)"$"#)]
async fn fragment_contains_form_posting_to(world: &mut FoundryWorld, action: String) {
    let body = world
        .us_12_last_get_body
        .as_ref()
        .expect("body captured by When");
    let css = format!(r#"form[action="{action}"]"#);
    assert_has(body, &css);
}

#[then(regex = r#"^the response contains an input named "([^"]+)"$"#)]
async fn response_contains_input_named(world: &mut FoundryWorld, name: String) {
    let body = world
        .us_12_last_get_body
        .as_ref()
        .expect("body captured by When");
    let css = format!(r#"input[name="{name}"]"#);
    assert_has(body, &css);
}

#[then(regex = r"^the response marks the title input as autofocused$")]
async fn response_marks_title_autofocused(world: &mut FoundryWorld) {
    let body = world
        .us_12_last_get_body
        .as_ref()
        .expect("body captured by When");
    assert_has(body, r#"input[name="title"][autofocus]"#);
}

#[then(regex = r"^the response is an htmx fragment$")]
async fn response_is_htmx_fragment(world: &mut FoundryWorld) {
    let body = world
        .us_12_last_get_body
        .as_ref()
        .expect("body captured by When");
    // An htmx fragment is identified by the absence of a top-level
    // <html>/<!doctype html> wrapper. Same convention as the slice-1
    // `response_not_full_page` helper.
    let lower = body.to_ascii_lowercase();
    assert!(
        !lower.contains("<!doctype html") && !lower.contains("<html"),
        "expected an htmx fragment (no <html> wrapper), got:\n{body}"
    );
}

#[then(regex = r#"^the response lists exactly one matching issue whose title contains "([^"]+)"$"#)]
async fn lists_one_issue_title_contains(world: &mut FoundryWorld, fragment: String) {
    let body = world
        .us_12_last_get_body
        .as_ref()
        .expect("body captured by When");
    // Search-result fragment markup: each match wrapped in
    // <li class="search-result" data-issue-key="...">...<span class="title">TITLE</span></li>.
    let doc = Html::parse_fragment(body);
    let sel = Selector::parse(".search-result").expect(".search-result");
    let matches: Vec<_> = doc.select(&sel).collect();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one .search-result; got {n}; body was:\n{body}",
        n = matches.len()
    );
    let title_sel = Selector::parse(".title").expect(".title");
    let title_el = matches[0]
        .select(&title_sel)
        .next()
        .unwrap_or_else(|| panic!("search result missing .title in:\n{body}"));
    let title_text: String = title_el.text().collect();
    assert!(
        title_text.contains(&fragment),
        "expected .title text to contain {fragment:?}; got {title_text:?}"
    );
}

#[then(regex = r#"^the response does NOT list the issue titled "([^"]+)"$"#)]
async fn response_does_not_list_title(world: &mut FoundryWorld, title: String) {
    let body = world
        .us_12_last_get_body
        .as_ref()
        .expect("body captured by When");
    assert!(
        !body.contains(&title),
        "expected body NOT to contain {title:?}; body was:\n{body}"
    );
}

#[then(regex = r#"^the response lists exactly one matching issue whose key is "(\w+)-(\d+)"$"#)]
async fn lists_one_issue_with_key(world: &mut FoundryWorld, prefix: String, number: i32) {
    let body = world
        .us_12_last_get_body
        .as_ref()
        .expect("body captured by When");
    let doc = Html::parse_fragment(body);
    let sel = Selector::parse(".search-result").expect(".search-result");
    let matches: Vec<_> = doc.select(&sel).collect();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one .search-result; got {n}; body was:\n{body}",
        n = matches.len()
    );
    let key = matches[0]
        .value()
        .attr("data-issue-key")
        .unwrap_or_default();
    assert_eq!(
        key,
        format!("{prefix}-{number}"),
        "expected data-issue-key={prefix}-{number}, got {key:?}"
    );
}

#[then(regex = r"^the response is a valid HTML fragment$")]
async fn response_is_valid_html_fragment(world: &mut FoundryWorld) {
    let body = world
        .us_12_last_get_body
        .as_ref()
        .expect("body captured by When");
    // "Valid HTML fragment" = non-empty, scraper-parseable, and contains
    // at least one element node. scraper accepts almost anything, so the
    // real signal is "did the endpoint return something with structure".
    assert!(!body.trim().is_empty(), "expected non-empty body");
    let doc = Html::parse_fragment(body);
    let any = Selector::parse("*").expect("*");
    assert!(
        doc.select(&any).next().is_some(),
        "expected at least one HTML element in body:\n{body}"
    );
}

#[then(regex = r#"^the response describes the "([^"]+)" shortcut as "([^"]+)"$"#)]
async fn response_describes_shortcut(world: &mut FoundryWorld, shortcut: String, label: String) {
    let body = world
        .us_12_last_get_body
        .as_ref()
        .expect("body captured by When");
    // Shortcut-help markup: <dt data-shortcut="c">c</dt><dd>Create issue</dd>.
    // We assert the dt+dd pairing by selecting the dt with the matching
    // data-shortcut and walking to its sibling dd.
    let doc = Html::parse_fragment(body);
    let dt_sel_str = format!(r#"dt[data-shortcut="{shortcut}"]"#);
    let dt_sel = Selector::parse(&dt_sel_str)
        .unwrap_or_else(|err| panic!("bad selector {dt_sel_str:?}: {err:?}"));
    let dt = doc
        .select(&dt_sel)
        .next()
        .unwrap_or_else(|| panic!("no {dt_sel_str:?} in body:\n{body}"));
    // The next element sibling should be the dd describing it. scraper
    // exposes element children via `next_sibling_element()`-style
    // walking but we go via the dt's parent and find dd by index.
    // Simpler: find any dd inside the same dl whose text equals `label`
    // immediately after this dt.
    let dt_id = dt.id();
    let parent = dt.parent().expect("dt has parent (dl)");
    let mut found = false;
    let mut after = false;
    for node in parent.children() {
        if node.id() == dt_id {
            after = true;
            continue;
        }
        if !after {
            continue;
        }
        if let Some(el) = scraper::ElementRef::wrap(node) {
            if el.value().name() == "dd" {
                let text: String = el.text().collect();
                assert_eq!(
                    text.trim(),
                    label,
                    "shortcut {shortcut:?}: expected description {label:?}, got {text:?}"
                );
                found = true;
                break;
            }
        }
    }
    assert!(
        found,
        "no <dd> sibling found after <dt data-shortcut={shortcut:?}> in body:\n{body}"
    );
}
