//! new-issue-dialog-description step definitions — thread an OPTIONAL description
//! through the new-issue create path, mirroring the shipped edit dialog.
//!
//! HARNESS BOUNDARY (distill/test-scenarios.md): HTTP-level (reqwest + scraper)
//! plus direct store reads, NOT a JS browser. The four slice-1 scenarios pin S1
//! (the modal now carries a `description` textarea beside the title input), S2
//! (the create endpoint persists the typed description and returns the OOB
//! Backlog card), S3 (a filed description round-trips to the edit dialog), and
//! S4 (an empty description is stored verbatim and still renders the card).
//!
//! REUSES the board-new-issue Background seed (`a workspace "Acme" … member
//! "Mei" … team "Backend"`, `a project "Sandbox" with key prefix "GEN" …`),
//! the `Mei is signed in` Given (us_07), the board-new-issue
//! `the response is an out-of-band fragment targeting the "…" column` /
//! `it renders a card showing the key "…" and the title "…"` steps, and the
//! issue-edit-dialog `Mei opens the edit dialog for "…"` /
//! `the dialog description field contains "…"` steps. Only the create-with-
//! description phrases are new here (each globally unique per cucumber-rs).

use crate::support::harness::InProcHarness;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::redirect::Policy;
use scraper::{Html, Selector};
use std::collections::HashMap;

const TEST_NOW: &str = "2026-01-15T12:00:00Z";
const MEMBER_PASSWORD: &str = "mei-correct-horse-battery-staple";
const MEI_EMAIL: &str = "mei@acme.com";
const TEAM_SLUG: &str = "backend";

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

/// Parse the trailing number off an issue key like "GEN-1" → 1.
fn number_of(key: &str) -> i32 {
    key.rsplit_once('-')
        .and_then(|(_, n)| n.parse().ok())
        .unwrap_or_else(|| panic!("issue key {key:?} must end in -N"))
}

// ----- When: fetch the dialog / file a described issue ----------------------

#[when(regex = r#"^Mei fetches the new-issue dialog for "([^"]+)"$"#)]
async fn fetch_dialog(world: &mut FoundryWorld, project: String) {
    ensure_harness(world).await;
    let url = format!("/team/{TEAM_SLUG}/project/{}/issues/new", slugify(&project));
    capture_get(world, &url, true).await;
}

#[when(
    regex = r#"^Mei files a new issue titled "([^"]*)" described "([^"]*)" to "([^"]+)" as an htmx request$"#
)]
async fn file_described_htmx(
    world: &mut FoundryWorld,
    title: String,
    description: String,
    project: String,
) {
    ensure_harness(world).await;
    let url = format!("/team/{TEAM_SLUG}/project/{}/issues", slugify(&project));
    capture_create_post(world, &url, &title, &description, true).await;
}

// ----- Given: a described issue already filed (precondition for S3) ---------

#[given(regex = r#"^Mei has filed an issue titled "([^"]*)" described "([^"]*)" to "([^"]+)"$"#)]
async fn has_filed_described(
    world: &mut FoundryWorld,
    title: String,
    description: String,
    project: String,
) {
    ensure_harness(world).await;
    let url = format!("/team/{TEAM_SLUG}/project/{}/issues", slugify(&project));
    // File through the real create endpoint (htmx) so the precondition exercises
    // the production write path, not a direct store insert.
    capture_create_post(world, &url, &title, &description, true).await;
}

// ----- Then: S1 modal carries the description textarea ----------------------

#[then(
    regex = r#"^the new-issue modal form carries a "description" textarea beside the title input$"#
)]
async fn modal_carries_description_textarea(world: &mut FoundryWorld) {
    let body = world.last_body.clone().unwrap_or_default();
    let doc = Html::parse_fragment(&body);
    let title_selector = Selector::parse("form input[name='title']").expect("valid selector");
    assert!(
        doc.select(&title_selector).next().is_some(),
        "the new-issue modal must keep its title input: {body}"
    );
    let textarea_selector =
        Selector::parse("form textarea[name='description']").expect("valid selector");
    assert!(
        doc.select(&textarea_selector).next().is_some(),
        "the new-issue modal must carry a description textarea beside the title input: {body}"
    );
}

#[then(regex = r#"^the new-issue "description" textarea is empty$"#)]
async fn modal_description_textarea_empty(world: &mut FoundryWorld) {
    let body = world.last_body.clone().unwrap_or_default();
    let doc = Html::parse_fragment(&body);
    let selector = Selector::parse("form textarea[name='description']").expect("valid selector");
    let textarea = doc
        .select(&selector)
        .next()
        .unwrap_or_else(|| panic!("no description textarea: {body}"));
    let text: String = textarea.text().collect();
    assert!(
        text.is_empty(),
        "the description textarea must be empty on a normal dialog open, found {text:?}: {body}"
    );
}

// ----- Then: S2/S4 the store persisted the typed description ----------------

#[then(regex = r#"^the created "([^"]+)" issue "([^"]+)" has description "([^"]*)" in the store$"#)]
async fn created_issue_has_description(
    world: &mut FoundryWorld,
    _project: String,
    key: String,
    description: String,
) {
    let stored = read_description(world, &key).await;
    assert_eq!(
        stored, description,
        "stored description_md mismatch for {key}"
    );
}

// ----- internals: authenticated HTTP + DB reads -----------------------------

async fn read_description(world: &mut FoundryWorld, key: &str) -> String {
    let (prefix, _) = key.rsplit_once('-').expect("issue key has -N");
    let number = number_of(key);
    let harness = world.harness.as_ref().expect("harness");
    let row: (String,) = sqlx::query_as(
        "SELECT i.description_md
           FROM issues i
           JOIN projects p ON p.id = i.project_id
          WHERE p.key_prefix = $1 AND i.number = $2",
    )
    .bind(prefix)
    .bind(number)
    .fetch_one(harness.app.state.store.pool())
    .await
    .unwrap_or_else(|e| panic!("read issue {key} description from store: {e}"));
    row.0
}

async fn capture_get(world: &mut FoundryWorld, url: &str, htmx: bool) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let (session_pair, _csrf) = sign_in(harness, http).await;
    let base = harness.base_url();
    let mut request = http
        .get(format!("{base}{url}"))
        .header(reqwest::header::COOKIE, session_pair);
    if htmx {
        request = request.header("HX-Request", "true");
    }
    let resp = request.send().await.expect("get target url");
    store(world, resp).await;
}

async fn capture_create_post(
    world: &mut FoundryWorld,
    url: &str,
    title: &str,
    description: &str,
    htmx: bool,
) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let (session_pair, csrf) = sign_in(harness, http).await;
    let base = harness.base_url();
    let combined = format!("{session_pair}; foundry_csrf={csrf}");
    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("title", title.to_string());
    form.insert("description", description.to_string());
    form.insert("_csrf", csrf);
    let mut request = http
        .post(format!("{base}{url}"))
        .header(reqwest::header::COOKIE, combined)
        .form(&form);
    if htmx {
        request = request.header("HX-Request", "true");
    }
    let resp = request.send().await.expect("post create url");
    store(world, resp).await;
}

async fn store(world: &mut FoundryWorld, resp: reqwest::Response) {
    world.last_status = Some(resp.status());
    world.last_headers = Some(resp.headers().clone());
    world.last_body = Some(resp.text().await.unwrap_or_default());
}

/// Sign Mei in and return `(session_pair, csrf_token)` — mirrors the
/// board-new-issue / issue-edit-dialog harness (no cookie jar; re-authenticates
/// per request).
async fn sign_in(harness: &InProcHarness, http: &reqwest::Client) -> (String, String) {
    let base = harness.base_url();
    let get = http
        .get(format!("{base}/sign-in"))
        .send()
        .await
        .expect("get /sign-in for csrf");
    let csrf_token = get
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .and_then(|s| s.strip_prefix("foundry_csrf="))
        .and_then(|rest| rest.split(';').next())
        .expect("/sign-in must mint a foundry_csrf cookie")
        .to_string();
    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("email", MEI_EMAIL.to_string());
    form.insert("password", MEMBER_PASSWORD.to_string());
    form.insert("_csrf", csrf_token.clone());
    let resp = http
        .post(format!("{base}/sign-in"))
        .header(
            reqwest::header::COOKIE,
            format!("foundry_csrf={csrf_token}"),
        )
        .form(&form)
        .send()
        .await
        .expect("post /sign-in");
    let session_pair = resp
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_session="))
        .and_then(|s| s.split(';').next())
        .expect("sign-in must issue a foundry_session cookie")
        .to_string();
    (session_pair, csrf_token)
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
