//! US-10 step definitions — markdown comments on an issue.
//!
//! Background phrases are reused unchanged from slice 1 / US-09 (the
//! workspace + member + signed-in + open-subscription lines).
//!
//! New phrases here cover:
//!   - posting a comment with markdown body
//!   - asserting the issue-page render contains specific HTML elements
//!   - asserting the author was recorded on the comment
//!   - asserting the realtime event payload carries `author_email`
//!   - asserting no comment was recorded on the @error paths
//!
//! Each Then step that inspects rendered HTML lazily GETs the issue
//! page once per scenario and caches the body in
//! `world.us_10_last_issue_body` so multiple assertions on the same
//! comment block don't fan out into multiple HTTP requests.

use crate::support::harness::InProcHarness;
use crate::support::html_assertions::{
    assert_comment_has_element_with_text, assert_comment_has_no_element,
    assert_comment_link_with_rel,
};
use crate::world::FoundryWorld;
use cucumber::{then, when};
use reqwest::redirect::Policy;
use reqwest::StatusCode;
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
        "Rita" => (
            "rita@partners.acme.com".to_string(),
            MEMBER_PASSWORD.to_string(),
        ),
        other => panic!("no identity registered for {other:?}"),
    }
}

fn email_for(who: &str) -> String {
    identity_for(who).0
}

/// Sign in as `email` / `password` and capture the session cookie pair
/// (`foundry_session=...`). Same shape as the US-09 helper — we don't
/// re-export across modules because cucumber-rs step modules are
/// independently linked; the helper is small.
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

// ---- Comment POST ----------------------------------------------------

#[when(regex = r#"^(\w+) comments on "(\w+)-(\d+)" with body "([\s\S]*)"$"#)]
async fn member_comments_on_issue(
    world: &mut FoundryWorld,
    who: String,
    prefix: String,
    number: i32,
    body: String,
) {
    // Gherkin does not interpret `\n` or `\t` inside double-quoted
    // strings — they arrive here as 2-character escape sequences.
    // The whitespace-only-body scenario clearly intends real whitespace,
    // so we unescape both here before sending. Any test that genuinely
    // wants the literal backslash-n in a comment body would write it
    // as a Gherkin docstring instead.
    let unescaped = body.replace("\\n", "\n").replace("\\t", "\t");
    ensure_harness(world).await;
    let (email, password) = identity_for(&who);
    let cookie = sign_in_and_capture_cookie(world, &email, &password).await;
    let base = world.harness.as_ref().expect("harness").base_url();
    let http = world.http.as_ref().expect("http");

    // Mint a CSRF cookie / token pair tied to this session.
    let csrf_get = http
        .get(format!("{base}/sign-in"))
        .header(reqwest::header::COOKIE, cookie.clone())
        .send()
        .await
        .expect("csrf for comment");
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
    // Slice-2 fixture: all comment scenarios target the "Backend" team.
    let team_slug = "backend";
    let url = format!("{base}/team/{team_slug}/project/{project_slug}/issues/{number}/comments");

    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("body", unescaped);
    form.insert("_csrf", csrf_token);
    let started = std::time::Instant::now();
    let resp = http
        .post(&url)
        .header(reqwest::header::COOKIE, combined)
        .form(&form)
        .send()
        .await
        .expect("post comment");
    world.us_09_last_action_started_at = Some(started);
    world.last_status = Some(resp.status());
    world.last_headers = Some(resp.headers().clone());
    let body_text = resp.text().await.unwrap_or_default();
    world.last_body = Some(body_text);
    world.us_10_last_issue_key = Some(format!("{prefix}-{number}"));
    // Clear cached page body so the first Then-step assertion re-GETs.
    world.us_10_last_issue_body = None;
}

/// comment-add-csrf 01-01 regression — acquire the CSRF double-submit pair from
/// the REAL issue-detail page (not `/sign-in`), scrape the add-comment form's
/// hidden `_csrf` field, and POST using ONLY that cookie + token. Proves
/// `show_issue` mints the same issuance seam every other write-form page uses.
/// Against a `show_issue` that omits the seam this fails loudly: no
/// `foundry_csrf` Set-Cookie and no hidden `_csrf` field to scrape.
#[when(
    regex = r#"^(\w+) posts a comment on "(\w+)-(\d+)" with body "([^"]*)" using only the CSRF cookie and token minted by the issue page$"#
)]
async fn member_comments_via_issue_page_csrf(
    world: &mut FoundryWorld,
    who: String,
    prefix: String,
    number: i32,
    body: String,
) {
    ensure_harness(world).await;
    let (email, password) = identity_for(&who);
    let session = sign_in_and_capture_cookie(world, &email, &password).await;
    let project_slug = lookup_project_slug_by_prefix(world, &prefix).await;
    let team_slug = "backend";
    let base = world.harness.as_ref().expect("harness").base_url();
    let http = world.http.as_ref().expect("http");

    // (a) GET the REAL issue page as the signed-in member.
    let issue_url = format!("{base}/team/{team_slug}/project/{project_slug}/issues/{number}");
    let page = http
        .get(&issue_url)
        .header(reqwest::header::COOKIE, session.clone())
        .send()
        .await
        .expect("get issue page");

    // (b) Capture `foundry_csrf` from the issue page's OWN Set-Cookie. The bug:
    // `show_issue` never mints it, so this is absent pre-fix.
    let cookie_token = page
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .and_then(|s| s.strip_prefix("foundry_csrf="))
        .and_then(|rest| rest.split(';').next())
        .map(str::to_string)
        .expect("the issue page must mint a foundry_csrf cookie (double-submit issuance seam)");

    let page_body = page.text().await.unwrap_or_default();

    // (c) Scrape the hidden `_csrf` field from the add-comment form. The bug:
    // the form has no hidden field pre-fix, so this is absent too.
    let field_token = {
        let doc = scraper::Html::parse_document(&page_body);
        let sel = scraper::Selector::parse(
            r#"form[action$="/comments"] input[type="hidden"][name="_csrf"]"#,
        )
        .expect("valid selector");
        doc.select(&sel)
            .next()
            .and_then(|el| el.value().attr("value"))
            .map(str::to_string)
            .expect("the add-comment form must carry a hidden _csrf field")
    };

    // The double-submit contract requires the cookie value and the rendered
    // form field to be the SAME token.
    assert_eq!(
        cookie_token, field_token,
        "issue-page cookie token and hidden-field token must be the same double-submit value"
    );

    // (d) POST using ONLY the session + the issue-page-minted cookie/token.
    let combined = format!("{session}; foundry_csrf={cookie_token}");
    let post_url =
        format!("{base}/team/{team_slug}/project/{project_slug}/issues/{number}/comments");
    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("body", body);
    form.insert("_csrf", field_token);
    let resp = http
        .post(&post_url)
        .header(reqwest::header::COOKIE, combined)
        .form(&form)
        .send()
        .await
        .expect("post comment");
    world.last_status = Some(resp.status());
    world.last_headers = Some(resp.headers().clone());
    let body_text = resp.text().await.unwrap_or_default();
    world.last_body = Some(body_text);
    world.us_10_last_issue_key = Some(format!("{prefix}-{number}"));
    world.us_10_last_issue_body = None;
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

// ---- Then: issue-page render ---------------------------------------

/// Fetch the issue page as the most recently signed-in commenter (Mei
/// by default for the US-10 scenarios). Caches the body in the world so
/// subsequent assertions on the same scenario reuse it.
async fn ensure_issue_page_body(world: &mut FoundryWorld, prefix: &str, n: i32) -> String {
    if let Some(b) = world.us_10_last_issue_body.clone() {
        return b;
    }
    let project_slug = lookup_project_slug_by_prefix(world, prefix).await;
    let team_slug = "backend";
    let base = world.harness.as_ref().expect("harness").base_url();
    // Sign in as Mei (the only persona who can read Backend issues in
    // this slice; both Hiroshi and Mei work — Mei is the scenarios'
    // primary author).
    let cookie = sign_in_and_capture_cookie(world, "mei@acme.com", MEMBER_PASSWORD).await;
    let http = world.http.as_ref().expect("http");
    let url = format!("{base}/team/{team_slug}/project/{project_slug}/issues/{n}");
    let resp = http
        .get(&url)
        .header(reqwest::header::COOKIE, cookie)
        .send()
        .await
        .expect("get issue page");
    let body = resp.text().await.unwrap_or_default();
    world.us_10_last_issue_body = Some(body.clone());
    body
}

#[then(
    regex = r#"^the issue page for "(\w+)-(\d+)" shows a comment by (\w+) containing an? <(\w+)> element with text "([^"]+)"$"#
)]
async fn issue_page_comment_contains_element_with_text(
    world: &mut FoundryWorld,
    prefix: String,
    n: i32,
    who: String,
    tag: String,
    text: String,
) {
    let body = ensure_issue_page_body(world, &prefix, n).await;
    let author = email_for(&who);
    assert_comment_has_element_with_text(&body, &author, &tag, &text);
}

#[then(
    regex = r#"^the issue page for "(\w+)-(\d+)" shows a comment by (\w+) containing an <a> element whose href is "([^"]+)" and whose rel attribute contains "([^"]+)"$"#
)]
async fn issue_page_comment_link_with_rel(
    world: &mut FoundryWorld,
    prefix: String,
    n: i32,
    who: String,
    href: String,
    rel_fragment: String,
) {
    let body = ensure_issue_page_body(world, &prefix, n).await;
    let author = email_for(&who);
    assert_comment_link_with_rel(&body, &author, &href, &rel_fragment);
}

#[then(
    regex = r#"^the issue page for "(\w+)-(\d+)" shows a comment by (\w+) that does NOT contain any <(\w+)> element$"#
)]
async fn issue_page_comment_no_element(
    world: &mut FoundryWorld,
    prefix: String,
    n: i32,
    who: String,
    tag: String,
) {
    let body = ensure_issue_page_body(world, &prefix, n).await;
    let author = email_for(&who);
    assert_comment_has_no_element(&body, &author, &tag);
}

#[then(regex = r"^the comment is recorded as authored by (\w+)$")]
async fn comment_recorded_authored_by(world: &mut FoundryWorld, who: String) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let email = email_for(&who);
    let row: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM comments c
           JOIN users u ON u.id = c.author_id
          WHERE u.email_lower = $1",
    )
    .bind(&email)
    .fetch_one(pool)
    .await
    .expect("count comments by author");
    assert!(
        row.0 >= 1,
        "expected ≥1 comment authored by {who} ({email}), got {}",
        row.0
    );
}

#[then(regex = r#"^no comment is recorded on "(\w+)-(\d+)"$"#)]
async fn no_comment_recorded(world: &mut FoundryWorld, prefix: String, n: i32) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    // Resolve the issue row via (key_prefix, number) then count comments.
    let row: Option<(uuid::Uuid,)> = sqlx::query_as(
        "SELECT i.id FROM issues i
           JOIN projects p ON p.id = i.project_id
          WHERE p.key_prefix = $1 AND i.number = $2",
    )
    .bind(&prefix)
    .bind(n)
    .fetch_optional(pool)
    .await
    .expect("lookup issue id");
    let issue_id = row
        .unwrap_or_else(|| panic!("issue {prefix}-{n} not seeded"))
        .0;
    let count: (i64,) = sqlx::query_as("SELECT count(*) FROM comments WHERE issue_id = $1")
        .bind(issue_id)
        .fetch_one(pool)
        .await
        .expect("count comments");
    assert_eq!(
        count.0, 0,
        "expected zero comments on {prefix}-{n}, got {}",
        count.0
    );
}

#[then(regex = r#"^the event payload's author email is "([^"]+)"$"#)]
async fn event_author_email(world: &mut FoundryWorld, expected: String) {
    let evt = world
        .us_09_last_event
        .as_ref()
        .expect("a realtime event was captured by the prior Then step");
    let observed = evt
        .payload_json
        .as_ref()
        .and_then(|v| v.get("author_email").and_then(|a| a.as_str()))
        .unwrap_or("");
    assert_eq!(
        observed, expected,
        "expected author_email {expected:?}, got {observed:?} in payload {:?}",
        evt.payload_json
    );
}

#[then(regex = r"^the response status is 403$")]
async fn status_403(world: &mut FoundryWorld) {
    let status = world.last_status.expect("status captured");
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "expected 403, got {status} body={body}",
        body = world.last_body.as_deref().unwrap_or("")
    );
}

// ---- Background extension: Partners team + Rita seeding -------------
//
// US-09 already owns the regex r#"^a member "([^"]+)" belongs to the team "([^"]+)"$"#
// (via the US-07 module). That step seeds the team if missing — including
// "Partners" — so we DO NOT redeclare it here (it would collide).
//
// "Mei is signed in" / "Rita is signed in" / "Hiroshi is signed in" are
// matched by the US-07 step `(\w+) is signed in`. We don't redeclare
// them — but the US-10 scenarios that use Rita rely on her being seeded
// AND on her cookie being captured at first POST time, which happens
// inside `member_comments_on_issue` via `sign_in_and_capture_cookie`.
