//! US-10 slice-5 step definitions — comment edit and delete and admin
//! delete, plus 410-Gone disambiguation, soft-delete invariant, and
//! realtime fan-out of CommentEdited / CommentDeleted.
//!
//! Slice-2 step file `us_10_comments.rs` is NOT modified — slice-5
//! work is additive. The phrases registered here are NEW; they do not
//! collide with the slice-2 phrases (cucumber-rs treats phrases as
//! globally unique).
//!
//! World additions used by these steps:
//!   - world.us_10_5_last_comment_id_by_author : HashMap<(prefix, n,
//!     who_email), comment_id>
//!   - world.us_10_5_last_comment_id_by_body : HashMap<(prefix, n,
//!     body_substring), comment_id>
//!   - world.us_10_5_last_edit_form_body : Option<String>
//!   - world.us_10_5_last_posted_body : HashMap<(prefix, n, email), String>

#![allow(unused_variables, dead_code)]

use crate::support::harness::InProcHarness;
use crate::support::html_assertions::{
    assert_comment_has_element_with_text, comment_section_by_author, parse, select_all, text_of,
};
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use std::collections::HashMap;

const TEST_NOW: &str = "2026-01-15T12:00:00Z";
const MEMBER_PASSWORD: &str = "mei-correct-horse-battery-staple";
/// Password the slice-1 us_06 Background `workspace_with_admin` step
/// seeds for `devansh@acme.com` (us_06_signin.rs line 69). Same literal
/// the slice-3 US-03 step file uses as its `ADMIN_PASSWORD` constant.
const ADMIN_PASSWORD: &str = "admin-password-from-bootstrap";

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

/// Resolve the (email, password) for a persona. Devansh is the admin
/// (different password from members per the slice-2 seeding convention).
fn identity_for(who: &str) -> (String, String) {
    match who {
        "Mei" => ("mei@acme.com".to_string(), MEMBER_PASSWORD.to_string()),
        "Hiroshi" => ("hiroshi@acme.com".to_string(), MEMBER_PASSWORD.to_string()),
        "Devansh" => ("devansh@acme.com".to_string(), ADMIN_PASSWORD.to_string()),
        other => panic!("no identity registered for {other:?}"),
    }
}

fn email_for(who: &str) -> String {
    identity_for(who).0
}

/// Sign-in dance (same shape as the slice-2 helper). Returns just the
/// session-cookie pair (`foundry_session=<id>`). The CSRF cookie is
/// re-minted at each state-mutating call site so this helper stays
/// minimal.
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

/// Mint a fresh CSRF cookie + token bound to the given session. Returns
/// `(combined_cookie_header, csrf_token)`. The combined header carries
/// both the session and CSRF cookies so the next state-mutating call can
/// satisfy the double-submit middleware.
async fn mint_csrf(world: &FoundryWorld, session_cookie: &str) -> (String, String) {
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let base = harness.base_url();
    let csrf_get = http
        .get(format!("{base}/sign-in"))
        .header(reqwest::header::COOKIE, session_cookie.to_string())
        .send()
        .await
        .expect("csrf mint");
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
    let combined = format!("{session_cookie}; foundry_csrf={csrf_token}");
    (combined, csrf_token)
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

/// Sign in as `who`, POST a comment via the existing slice-2 POST handler,
/// then look up the comment id in the DB and stash it in both world maps
/// (by-author and by-body) so subsequent When steps can address it.
async fn seed_comment_via_post_handler(
    world: &mut FoundryWorld,
    who: &str,
    prefix: &str,
    n: i32,
    body: &str,
) {
    ensure_harness(world).await;
    let (email, password) = identity_for(who);
    let session = sign_in_and_capture_cookie(world, &email, &password).await;
    let (combined, csrf_token) = mint_csrf(world, &session).await;
    let project_slug = lookup_project_slug_by_prefix(world, prefix).await;
    let base = world.harness.as_ref().expect("harness").base_url();
    let http = world.http.as_ref().expect("http");
    let url = format!("{base}/team/backend/project/{project_slug}/issues/{n}/comments");
    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("body", body.to_string());
    form.insert("_csrf", csrf_token);
    let resp = http
        .post(&url)
        .header(reqwest::header::COOKIE, combined)
        .form(&form)
        .send()
        .await
        .expect("post comment seed");
    assert!(
        resp.status().is_success() || resp.status().is_redirection(),
        "expected POST comment to succeed, got {} body={}",
        resp.status(),
        resp.text().await.unwrap_or_default()
    );
    // Look up the most-recently-posted comment for this (issue, author)
    // pair. Per-scenario PG schema rotation means this is the only
    // comment by that author on that issue with the seeded body.
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let row: (uuid::Uuid,) = sqlx::query_as(
        "SELECT c.id
           FROM comments c
           JOIN users u ON u.id = c.author_id
           JOIN issues i ON i.id = c.issue_id
           JOIN projects p ON p.id = i.project_id
          WHERE p.key_prefix = $1 AND i.number = $2
            AND u.email_lower = $3 AND c.body_markdown = $4
          ORDER BY c.created_at DESC LIMIT 1",
    )
    .bind(prefix)
    .bind(n)
    .bind(email.to_ascii_lowercase())
    .bind(body)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|err| panic!("lookup seeded comment failed: {err}"));
    let comment_id = row.0;
    world
        .us_10_5_last_comment_id_by_author
        .insert((prefix.to_string(), n, email.clone()), comment_id);
    // Also key by body — slice-9 ("Second comment — to be removed")
    // needs to address by body fragment.
    world
        .us_10_5_last_comment_id_by_body
        .insert((prefix.to_string(), n, body.to_string()), comment_id);
    world
        .us_10_5_last_posted_body
        .insert((prefix.to_string(), n, email), body.to_string());
    // Clear cached issue-page body so subsequent Then assertions re-GET.
    world.us_10_last_issue_body = None;
}

// ---- Givens ----------------------------------------------------------

#[given(
    regex = r#"^(\w+) has previously posted a comment on "(\w+)-(\d+)" with body "([\s\S]*)"$"#
)]
async fn given_member_previously_posted_comment(
    world: &mut FoundryWorld,
    who: String,
    prefix: String,
    n: i32,
    body: String,
) {
    let unescaped = body.replace("\\n", "\n").replace("\\t", "\t");
    seed_comment_via_post_handler(world, &who, &prefix, n, &unescaped).await;
}

#[given(regex = r#"^(\w+) has deleted (?:her|his) own comment on "(\w+)-(\d+)"$"#)]
async fn given_member_has_deleted_own_comment(
    world: &mut FoundryWorld,
    who: String,
    prefix: String,
    n: i32,
) {
    ensure_harness(world).await;
    let (email, password) = identity_for(&who);
    let comment_id = world
        .us_10_5_last_comment_id_by_author
        .get(&(prefix.clone(), n, email.clone()))
        .copied()
        .unwrap_or_else(|| {
            panic!("no previously-posted comment by {who} on {prefix}-{n} captured")
        });
    let session = sign_in_and_capture_cookie(world, &email, &password).await;
    let (combined, csrf_token) = mint_csrf(world, &session).await;
    let project_slug = lookup_project_slug_by_prefix(world, &prefix).await;
    let base = world.harness.as_ref().expect("harness").base_url();
    let http = world.http.as_ref().expect("http");
    let url =
        format!("{base}/team/backend/project/{project_slug}/issues/{n}/comments/{comment_id}",);
    let resp = http
        .delete(&url)
        .header(reqwest::header::COOKIE, combined)
        .header("HX-CSRF", csrf_token)
        .send()
        .await
        .expect("delete comment for pre-tombstone");
    assert!(
        resp.status().is_success(),
        "expected DELETE seed to succeed, got {}",
        resp.status()
    );
    world.us_10_last_issue_body = None;
}

// ---- Whens -----------------------------------------------------------

#[when(regex = r#"^(\w+) requests the edit form for (?:her|his) comment on "(\w+)-(\d+)"$"#)]
async fn when_member_requests_edit_form(
    world: &mut FoundryWorld,
    who: String,
    prefix: String,
    n: i32,
) {
    ensure_harness(world).await;
    let (email, password) = identity_for(&who);
    let comment_id = world
        .us_10_5_last_comment_id_by_author
        .get(&(prefix.clone(), n, email.clone()))
        .copied()
        .unwrap_or_else(|| panic!("no comment id captured for {who} on {prefix}-{n}"));
    let session = sign_in_and_capture_cookie(world, &email, &password).await;
    let project_slug = lookup_project_slug_by_prefix(world, &prefix).await;
    let base = world.harness.as_ref().expect("harness").base_url();
    let http = world.http.as_ref().expect("http");
    let url = format!(
        "{base}/team/backend/project/{project_slug}/issues/{n}/comments/{comment_id}/edit",
    );
    let resp = http
        .get(&url)
        .header(reqwest::header::COOKIE, session)
        .send()
        .await
        .expect("get edit form");
    world.last_status = Some(resp.status());
    world.last_headers = Some(resp.headers().clone());
    let body = resp.text().await.unwrap_or_default();
    world.us_10_5_last_edit_form_body = Some(body.clone());
    world.last_body = Some(body);
}

#[when(
    regex = r#"^(\w+) submits an edit to (?:her|his) comment on "(\w+)-(\d+)" with body "([\s\S]*)"$"#
)]
async fn when_member_submits_edit_to_own_comment(
    world: &mut FoundryWorld,
    who: String,
    prefix: String,
    n: i32,
    body: String,
) {
    let unescaped = body.replace("\\n", "\n").replace("\\t", "\t");
    submit_patch(world, &who, &prefix, n, &unescaped).await;
}

#[when(
    regex = r#"^(\w+) submits an edit to (\w+)'s comment on "(\w+)-(\d+)" with body "([\s\S]*)"$"#
)]
async fn when_non_author_submits_edit_to_others_comment(
    world: &mut FoundryWorld,
    who: String,
    target_author: String,
    prefix: String,
    n: i32,
    body: String,
) {
    let unescaped = body.replace("\\n", "\n").replace("\\t", "\t");
    let target_email = email_for(&target_author);
    submit_patch_by_target_author(world, &who, &prefix, n, &target_email, &unescaped).await;
}

#[when(
    regex = r#"^(\w+) submits an edit to (?:her|his) soft-deleted comment on "(\w+)-(\d+)" with body "([\s\S]*)"$"#
)]
async fn when_member_submits_edit_to_soft_deleted_comment(
    world: &mut FoundryWorld,
    who: String,
    prefix: String,
    n: i32,
    body: String,
) {
    let unescaped = body.replace("\\n", "\n").replace("\\t", "\t");
    submit_patch(world, &who, &prefix, n, &unescaped).await;
}

async fn submit_patch(world: &mut FoundryWorld, who: &str, prefix: &str, n: i32, body: &str) {
    ensure_harness(world).await;
    let (email, password) = identity_for(who);
    let comment_id = world
        .us_10_5_last_comment_id_by_author
        .get(&(prefix.to_string(), n, email.clone()))
        .copied()
        .unwrap_or_else(|| panic!("no comment id captured for {who} on {prefix}-{n}"));
    submit_patch_by_id(world, who, prefix, n, comment_id, body).await;
}

async fn submit_patch_by_target_author(
    world: &mut FoundryWorld,
    actor: &str,
    prefix: &str,
    n: i32,
    target_email: &str,
    body: &str,
) {
    let comment_id = world
        .us_10_5_last_comment_id_by_author
        .get(&(prefix.to_string(), n, target_email.to_string()))
        .copied()
        .unwrap_or_else(|| {
            panic!("no comment id captured for target {target_email} on {prefix}-{n}")
        });
    submit_patch_by_id(world, actor, prefix, n, comment_id, body).await;
}

async fn submit_patch_by_id(
    world: &mut FoundryWorld,
    who: &str,
    prefix: &str,
    n: i32,
    comment_id: uuid::Uuid,
    body: &str,
) {
    let (email, password) = identity_for(who);
    let session = sign_in_and_capture_cookie(world, &email, &password).await;
    let (combined, csrf_token) = mint_csrf(world, &session).await;
    let project_slug = lookup_project_slug_by_prefix(world, prefix).await;
    let base = world.harness.as_ref().expect("harness").base_url();
    let http = world.http.as_ref().expect("http");
    let url =
        format!("{base}/team/backend/project/{project_slug}/issues/{n}/comments/{comment_id}",);
    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("body_markdown", body.to_string());
    form.insert("_csrf", csrf_token);
    let started = std::time::Instant::now();
    let resp = http
        .patch(&url)
        .header(reqwest::header::COOKIE, combined)
        .form(&form)
        .send()
        .await
        .expect("patch comment");
    world.us_09_last_action_started_at = Some(started);
    world.last_status = Some(resp.status());
    world.last_headers = Some(resp.headers().clone());
    let body_text = resp.text().await.unwrap_or_default();
    world.last_body = Some(body_text);
    world.us_10_last_issue_body = None;
}

#[when(regex = r#"^(\w+) deletes (?:her|his) own comment on "(\w+)-(\d+)"$"#)]
async fn when_member_deletes_own_comment(
    world: &mut FoundryWorld,
    who: String,
    prefix: String,
    n: i32,
) {
    let target_email = email_for(&who);
    submit_delete_by_target(world, &who, &prefix, n, &target_email).await;
}

#[when(regex = r#"^(\w+) deletes (?:her|his) own comment on "(\w+)-(\d+)" again$"#)]
async fn when_member_deletes_own_comment_again(
    world: &mut FoundryWorld,
    who: String,
    prefix: String,
    n: i32,
) {
    let target_email = email_for(&who);
    submit_delete_by_target(world, &who, &prefix, n, &target_email).await;
}

#[when(regex = r#"^(\w+) deletes (?:her|his) own "([\s\S]+)" comment on "(\w+)-(\d+)"$"#)]
async fn when_member_deletes_own_comment_by_body(
    world: &mut FoundryWorld,
    who: String,
    body_substring: String,
    prefix: String,
    n: i32,
) {
    let comment_id = world
        .us_10_5_last_comment_id_by_body
        .get(&(prefix.clone(), n, body_substring.clone()))
        .copied()
        .unwrap_or_else(|| {
            panic!("no comment with body {body_substring:?} captured on {prefix}-{n}")
        });
    submit_delete_by_id(world, &who, &prefix, n, comment_id).await;
}

#[when(regex = r#"^(\w+) deletes (\w+)'s comment on "(\w+)-(\d+)"$"#)]
async fn when_admin_deletes_others_comment(
    world: &mut FoundryWorld,
    who: String,
    target_author: String,
    prefix: String,
    n: i32,
) {
    let target_email = email_for(&target_author);
    submit_delete_by_target(world, &who, &prefix, n, &target_email).await;
}

async fn submit_delete_by_target(
    world: &mut FoundryWorld,
    actor: &str,
    prefix: &str,
    n: i32,
    target_email: &str,
) {
    let comment_id = world
        .us_10_5_last_comment_id_by_author
        .get(&(prefix.to_string(), n, target_email.to_string()))
        .copied()
        .unwrap_or_else(|| {
            panic!("no comment id captured for target {target_email} on {prefix}-{n}")
        });
    submit_delete_by_id(world, actor, prefix, n, comment_id).await;
}

async fn submit_delete_by_id(
    world: &mut FoundryWorld,
    actor: &str,
    prefix: &str,
    n: i32,
    comment_id: uuid::Uuid,
) {
    ensure_harness(world).await;
    let (email, password) = identity_for(actor);
    let session = sign_in_and_capture_cookie(world, &email, &password).await;
    let (combined, csrf_token) = mint_csrf(world, &session).await;
    let project_slug = lookup_project_slug_by_prefix(world, prefix).await;
    let base = world.harness.as_ref().expect("harness").base_url();
    let http = world.http.as_ref().expect("http");
    let url =
        format!("{base}/team/backend/project/{project_slug}/issues/{n}/comments/{comment_id}",);
    let started = std::time::Instant::now();
    let resp = http
        .delete(&url)
        .header(reqwest::header::COOKIE, combined)
        .header("HX-CSRF", csrf_token)
        .send()
        .await
        .expect("delete comment");
    world.us_09_last_action_started_at = Some(started);
    world.last_status = Some(resp.status());
    world.last_headers = Some(resp.headers().clone());
    let body_text = resp.text().await.unwrap_or_default();
    world.last_body = Some(body_text);
    world.us_10_last_issue_body = None;
}

#[when(regex = r#"^(\w+) cancels the edit on (?:her|his) comment on "(\w+)-(\d+)"$"#)]
async fn when_member_cancels_edit(world: &mut FoundryWorld, who: String, prefix: String, n: i32) {
    ensure_harness(world).await;
    let (email, password) = identity_for(&who);
    let comment_id = world
        .us_10_5_last_comment_id_by_author
        .get(&(prefix.clone(), n, email.clone()))
        .copied()
        .unwrap_or_else(|| panic!("no comment id captured for {who} on {prefix}-{n}"));
    let session = sign_in_and_capture_cookie(world, &email, &password).await;
    let project_slug = lookup_project_slug_by_prefix(world, &prefix).await;
    let base = world.harness.as_ref().expect("harness").base_url();
    let http = world.http.as_ref().expect("http");
    // D3 = A: GET /…/comments/{id} returns the single-card fragment.
    let url =
        format!("{base}/team/backend/project/{project_slug}/issues/{n}/comments/{comment_id}",);
    let resp = http
        .get(&url)
        .header(reqwest::header::COOKIE, session)
        .send()
        .await
        .expect("get cancel-edit single comment");
    world.last_status = Some(resp.status());
    world.last_headers = Some(resp.headers().clone());
    let body_text = resp.text().await.unwrap_or_default();
    world.last_body = Some(body_text);
}

// ---- Thens -----------------------------------------------------------

#[then(
    regex = r#"^the response is an htmx fragment containing a textarea whose value is the raw markdown source of (?:her|his) comment$"#
)]
async fn then_response_textarea_with_raw_markdown(world: &mut FoundryWorld) {
    let body = world
        .us_10_5_last_edit_form_body
        .as_ref()
        .or(world.last_body.as_ref())
        .cloned()
        .expect("edit form body captured");
    let doc = parse(&body);
    let textareas = select_all(&doc, "textarea");
    assert!(
        !textareas.is_empty(),
        "expected at least one <textarea> in edit-form fragment; body was:\n{body}"
    );
    let textarea_text = text_of(&textareas[0]);
    // The textarea should contain SOME markdown source — we don't pin
    // an exact match because the scenario's Given body uses bold
    // markdown (** characters) which the renderer would otherwise
    // remove. Asserting non-empty + at-least-one-marker-char is
    // sufficient to prove "raw markdown, not rendered HTML".
    assert!(
        !textarea_text.trim().is_empty(),
        "expected non-empty textarea body in edit form; got {textarea_text:?}"
    );
    // Compare against the most-recently-stashed body for an author
    // who has a captured posted body. The WS scenario posts a body
    // containing `**Set-Cookie SameSite=Lax**` and the textarea must
    // carry the literal `**` markers.
    let posted = world
        .us_10_5_last_posted_body
        .values()
        .next()
        .cloned()
        .unwrap_or_default();
    if !posted.is_empty() {
        assert_eq!(
            textarea_text.trim(),
            posted.trim(),
            "textarea value does not match the raw markdown source\n\
             posted: {posted:?}\n\
             textarea: {textarea_text:?}"
        );
    }
}

/// Lazily fetch the issue page as Mei (the canonical reader in slice-5
/// scenarios). Caches in `world.us_10_last_issue_body` so multiple Then
/// steps in the same scenario share the same response.
async fn ensure_issue_page_body(world: &mut FoundryWorld, prefix: &str, n: i32) -> String {
    if let Some(b) = world.us_10_last_issue_body.clone() {
        return b;
    }
    let project_slug = lookup_project_slug_by_prefix(world, prefix).await;
    let base = world.harness.as_ref().expect("harness").base_url();
    let session = sign_in_and_capture_cookie(world, "mei@acme.com", MEMBER_PASSWORD).await;
    let http = world.http.as_ref().expect("http");
    let url = format!("{base}/team/backend/project/{project_slug}/issues/{n}");
    let resp = http
        .get(&url)
        .header(reqwest::header::COOKIE, session)
        .send()
        .await
        .expect("get issue page");
    let body = resp.text().await.unwrap_or_default();
    world.us_10_last_issue_body = Some(body.clone());
    body
}

#[then(
    regex = r#"^the issue page for "(\w+)-(\d+)" shows a comment by (\w+) with an "edited" indicator$"#
)]
async fn then_issue_page_comment_has_edited_indicator(
    world: &mut FoundryWorld,
    prefix: String,
    n: i32,
    who: String,
) {
    let body = ensure_issue_page_body(world, &prefix, n).await;
    let author = email_for(&who);
    let section = comment_section_by_author(&body, &author)
        .unwrap_or_else(|| panic!("no comment by {author} in issue page body:\n{body}"));
    // The "edited" indicator is the `.comment-edited-marker` element
    // (server-rendered when `row.edited == true`). Substring match on
    // the class so a v0.2 polish that renames "(edited)" copy doesn't
    // red the test.
    let html = section.root_element().html();
    assert!(
        html.contains("comment-edited-marker"),
        "expected .comment-edited-marker inside comment by {author}; got:\n{html}"
    );
}

#[then(
    regex = r#"^the issue page for "(\w+)-(\d+)" does NOT show a comment by (\w+) containing the text "([\s\S]+)"$"#
)]
async fn then_issue_page_no_comment_with_text(
    world: &mut FoundryWorld,
    prefix: String,
    n: i32,
    who: String,
    text: String,
) {
    let body = ensure_issue_page_body(world, &prefix, n).await;
    let author = email_for(&who);
    let doc = parse(&body);
    let selector = format!(r#".comment[data-author="{author}"]"#);
    let sel = scraper::Selector::parse(&selector).expect("selector");
    for el in doc.select(&sel) {
        let inner = el.text().collect::<String>();
        assert!(
            !inner.contains(&text),
            "found unexpected text {text:?} in comment by {author}:\n{inner}"
        );
    }
}

#[then(
    regex = r#"^the issue page for "(\w+)-(\d+)" still shows a comment by (\w+) containing the text "([\s\S]+)"$"#
)]
async fn then_issue_page_still_shows_comment_with_text(
    world: &mut FoundryWorld,
    prefix: String,
    n: i32,
    who: String,
    text: String,
) {
    then_issue_page_shows_comment_with_text(world, prefix, n, who, text).await;
}

#[then(
    regex = r#"^the issue page for "(\w+)-(\d+)" shows a comment by (\w+) containing the text "([\s\S]+)"$"#
)]
async fn then_issue_page_shows_comment_with_text(
    world: &mut FoundryWorld,
    prefix: String,
    n: i32,
    who: String,
    text: String,
) {
    let body = ensure_issue_page_body(world, &prefix, n).await;
    let author = email_for(&who);
    let doc = parse(&body);
    let selector = format!(r#".comment[data-author="{author}"]"#);
    let sel = scraper::Selector::parse(&selector).expect("selector");
    let mut found = false;
    for el in doc.select(&sel) {
        let inner = el.text().collect::<String>();
        if inner.contains(&text) {
            found = true;
            break;
        }
    }
    assert!(
        found,
        "no comment by {author} containing {text:?} on issue page:\n{body}"
    );
}

#[then(regex = r#"^the issue page for "(\w+)-(\d+)" no longer shows a comment by (\w+)$"#)]
async fn then_issue_page_no_longer_shows_comment_by(
    world: &mut FoundryWorld,
    prefix: String,
    n: i32,
    who: String,
) {
    let body = ensure_issue_page_body(world, &prefix, n).await;
    let author = email_for(&who);
    let doc = parse(&body);
    let selector = format!(r#".comment[data-author="{author}"]"#);
    let sel = scraper::Selector::parse(&selector).expect("selector");
    let count = doc.select(&sel).count();
    assert_eq!(
        count, 0,
        "expected zero comment cards by {author} on {prefix}-{n}, got {count}; body:\n{body}"
    );
}

#[then(regex = r"^the response status is 200$")]
async fn then_response_status_200(world: &mut FoundryWorld) {
    let status = world.last_status.expect("status captured");
    assert_eq!(
        status,
        StatusCode::OK,
        "expected 200, got {status} body={body}",
        body = world.last_body.as_deref().unwrap_or("")
    );
}

#[then(regex = r"^the response status is 410$")]
async fn then_response_status_410(world: &mut FoundryWorld) {
    let status = world.last_status.expect("status captured");
    assert_eq!(
        status,
        StatusCode::GONE,
        "expected 410 Gone, got {status} body={body}",
        body = world.last_body.as_deref().unwrap_or("")
    );
}

#[then(regex = r#"^the response is an htmx fragment containing the text "([\s\S]+)"$"#)]
async fn then_response_fragment_contains_text(world: &mut FoundryWorld, text: String) {
    let body = world.last_body.clone().unwrap_or_default();
    let doc = parse(&body);
    // Concatenate ALL text inside the body fragment.
    let concatenated: String = doc.root_element().text().collect();
    assert!(
        concatenated.contains(&text),
        "expected text {text:?} in response fragment; got: {concatenated:?}\n\
         raw body: {body}"
    );
}

#[then(regex = r#"^the response is an htmx fragment that does NOT contain a <(\w+)> element$"#)]
async fn then_response_fragment_no_element(world: &mut FoundryWorld, tag: String) {
    let body = world.last_body.clone().unwrap_or_default();
    let doc = parse(&body);
    let sel = scraper::Selector::parse(&tag).expect("selector");
    let count = doc.select(&sel).count();
    assert_eq!(
        count, 0,
        "expected NO <{tag}> in response fragment; got {count}; body: {body}"
    );
}

#[then(regex = r#"^the event payload's comment author email is "([^"]+)"$"#)]
async fn then_event_payload_comment_author_email(world: &mut FoundryWorld, expected: String) {
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
        "expected comment-author email {expected:?}, got {observed:?} in payload {:?}",
        evt.payload_json
    );
}

// Mark `assert_comment_has_element_with_text` + `text_of` as used to
// satisfy the import-pull above (they're called from helper closures
// referenced indirectly via the html_assertions facade). Without these
// the import shows as warning-unused under #[deny(warnings)] builds.
#[allow(dead_code)]
fn _silence_helper_imports() {
    let _: fn(&str, &str, &str, &str) -> String = assert_comment_has_element_with_text;
}
