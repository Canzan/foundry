//! issue-status-move step definitions — slice 01: change an issue's status from
//! the edit dialog (server-driven card relocation).
//!
//! HARNESS BOUNDARY (distill/test-scenarios.md): HTTP-level (reqwest + scraper),
//! NOT a JS browser. Slice 01 pins (S1) the dialog status control pre-set to the
//! current state, (S2) the save that persists the new state AND returns the
//! two-op OOB card relocation (delete the old card + append a fresh one to the
//! target column) with the dialog dismissed, and (S3) the no-JS plain-form save.
//! The live dialog gesture + the card-move animation are browser-dogfooded
//! (walking-skeleton.md). Slice 02 (drag-and-drop) stays @pending.
//!
//! REUSES the board-new-issue Background workspace/member/team seed
//! (`a workspace "Acme" … member "Mei" … team "Backend"`), the
//! `(\w+) is signed in` Given (us_07), and the issue-edit-dialog
//! `Mei opens the edit dialog for "…"` When. Only the status-specific phrases
//! are new here (each globally unique).

use crate::support::harness::InProcHarness;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use scraper::{Html, Selector};
use std::collections::HashMap;

const TEST_NOW: &str = "2026-01-15T12:00:00Z";
const MEMBER_PASSWORD: &str = "mei-correct-horse-battery-staple";
const MEI_EMAIL: &str = "mei@acme.com";
const TEAM_SLUG: &str = "backend";
const PROJECT_SLUG: &str = "sandbox";
/// The title the Background seeds GEN-1 with. The status-save When steps re-post
/// it so `edit_issue_details` (which requires a non-empty title) succeeds while
/// the scenario exercises ONLY the state transition.
const SEED_TITLE: &str = "Seeded issue";

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

fn number_of(key: &str) -> i32 {
    key.rsplit_once('-')
        .and_then(|(_, n)| n.parse().ok())
        .unwrap_or_else(|| panic!("issue key {key:?} must end in -N"))
}

fn edit_path(team: &str, project: &str, number: i32) -> String {
    format!("/team/{team}/project/{project}/issues/{number}/edit")
}

// ----- Background: seed the project + its GEN-1 issue in a named column -------

#[given(regex = r#"^a project "([^"]+)" \(key "([^"]+)"\) with an issue "([^"]+)" in "([^"]+)"$"#)]
async fn project_with_issue_in_state(
    world: &mut FoundryWorld,
    project: String,
    key_prefix: String,
    issue_key: String,
    column: String,
) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();

    let ws: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM workspaces WHERE name = 'Acme'")
        .fetch_one(pool)
        .await
        .expect("fetch Acme workspace");
    let team: (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM teams WHERE workspace_id = $1 AND name = 'Backend'")
            .bind(ws.0)
            .fetch_one(pool)
            .await
            .expect("fetch Backend team");
    let author: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(MEI_EMAIL)
        .fetch_one(pool)
        .await
        .expect("fetch Mei");

    let project_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(project_id)
    .bind(team.0)
    .bind(ws.0)
    .bind(&project)
    .bind(slugify(&project))
    .bind(&key_prefix)
    .execute(pool)
    .await
    .expect("insert project");

    let state = normalize_column(&column);
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, description_md, state, author_id)
              VALUES ($1, $2, $3, $4, $5, '', $6, $7)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(project_id)
    .bind(ws.0)
    .bind(number_of(&issue_key))
    .bind(SEED_TITLE)
    .bind(state)
    .bind(author.0)
    .execute(pool)
    .await
    .expect("insert issue");
}

// ----- When: save the dialog with a new status -------------------------------

#[when(regex = r#"^Mei saves the edit dialog for "([^"]+)" with status "([^"]+)"$"#)]
async fn save_dialog_status(world: &mut FoundryWorld, key: String, status: String) {
    let url = edit_path(TEAM_SLUG, PROJECT_SLUG, number_of(&key));
    capture_status_post(world, &url, &status, true).await;
}

#[when(
    regex = r#"^Mei submits the edit form for "([^"]+)" with status "([^"]+)" as a plain form$"#
)]
async fn submit_status_plain(world: &mut FoundryWorld, key: String, status: String) {
    let url = edit_path(TEAM_SLUG, PROJECT_SLUG, number_of(&key));
    capture_status_post(world, &url, &status, false).await;
}

// ----- Then: S1 status control pre-set ---------------------------------------

#[then(regex = r#"^the dialog has a status control with "([^"]+)" selected$"#)]
async fn dialog_has_status_selected(world: &mut FoundryWorld, expected: String) {
    let body = world.last_body.clone().unwrap_or_default();
    let doc = Html::parse_fragment(&body);
    let selector =
        Selector::parse("form select[name='state'] option[selected]").expect("valid selector");
    let option = doc
        .select(&selector)
        .next()
        .unwrap_or_else(|| panic!("dialog has no selected status option: {body}"));
    let label: String = option.text().collect();
    assert_eq!(
        label.trim(),
        expected,
        "the status control must pre-select the issue's current state"
    );
}

// ----- Then: S2/S3 the store holds the new state -----------------------------

#[then(regex = r#"^(?:the issue )?"([^"]+)" has state "([^"]+)" in the store$"#)]
async fn issue_has_state(world: &mut FoundryWorld, key: String, state: String) {
    let stored = read_issue_state(world, &key).await;
    assert_eq!(stored, state, "stored state mismatch for {key}");
}

// ----- Then: S2 the two-op OOB card relocation -------------------------------

#[then(
    regex = r#"^the response deletes the old "([^"]+)" card and appends it to the "([^"]+)" column$"#
)]
async fn response_relocates_card(world: &mut FoundryWorld, key: String, column: String) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "an htmx status save must return 200 OK"
    );
    let body = world.last_body.clone().unwrap_or_default();

    // (a) delete the old card, matched by its stable id.
    let delete_marker = format!(r#"id="issue-{key}""#);
    assert!(
        body.contains(&delete_marker) && body.contains(r#"hx-swap-oob="delete""#),
        "response must delete the old card by its stable id (hx-swap-oob=\"delete\" on #issue-{key}): {body}"
    );

    // (b) append a fresh card to the target column.
    let append_marker = format!(r#"hx-swap-oob="beforeend:[data-column='{column}']""#);
    assert!(
        body.contains(&append_marker),
        "response must append a fresh card to the {column:?} column: {body}"
    );
    let doc = Html::parse_fragment(&body);
    let card_selector = Selector::parse(&format!("article.issue-card[data-issue-key='{key}']"))
        .expect("valid selector");
    assert!(
        doc.select(&card_selector).next().is_some(),
        "the appended fragment must carry a fresh {key:?} card: {body}"
    );
}

#[then(regex = r#"^the dialog is dismissed without a full navigation$"#)]
async fn dialog_dismissed(world: &mut FoundryWorld) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "an htmx status save closes the dialog via a 200 (no 303 navigation)"
    );
    let body = world.last_body.clone().unwrap_or_default();
    // The primary (non-OOB) body is empty, so htmx clears #modal-root — the
    // response must not re-render the edit dialog itself.
    assert!(
        !body.contains(r#"data-modal="edit-issue""#),
        "the response must NOT re-render the dialog (its primary body is empty so #modal-root clears): {body}"
    );
}

// ----- Then: S3 the board reflects the move ----------------------------------

#[then(regex = r#"^the board shows "([^"]+)" under the "([^"]+)" column$"#)]
async fn board_shows_under_column(world: &mut FoundryWorld, key: String, column: String) {
    let url = format!("/team/{TEAM_SLUG}/project/{PROJECT_SLUG}");
    capture_get(world, &url, false).await;
    let body = world.last_body.clone().unwrap_or_default();
    let doc = Html::parse_document(&body);
    let selector = Selector::parse(&format!(
        "[data-column='{column}'] article.issue-card[data-issue-key='{key}']"
    ))
    .expect("valid selector");
    assert!(
        doc.select(&selector).next().is_some(),
        "the {column:?} column must show the {key:?} card after the move: {body}"
    );
}

// ----- internals: authenticated HTTP + DB reads ------------------------------

async fn read_issue_state(world: &mut FoundryWorld, key: &str) -> String {
    let (prefix, _) = key.rsplit_once('-').expect("issue key has -N");
    let number = number_of(key);
    let harness = world.harness.as_ref().expect("harness");
    let row: (String,) = sqlx::query_as(
        "SELECT i.state
           FROM issues i
           JOIN projects p ON p.id = i.project_id
          WHERE p.key_prefix = $1 AND i.number = $2",
    )
    .bind(prefix)
    .bind(number)
    .fetch_one(harness.app.state.store.pool())
    .await
    .unwrap_or_else(|e| panic!("read issue {key} state from store: {e}"));
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

async fn capture_status_post(world: &mut FoundryWorld, url: &str, status: &str, htmx: bool) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let (session_pair, csrf) = sign_in(harness, http).await;
    let base = harness.base_url();
    let combined = format!("{session_pair}; foundry_csrf={csrf}");
    let mut form: HashMap<&str, String> = HashMap::new();
    // Re-post the seeded title so the shared title validation passes; the
    // scenario exercises ONLY the status transition.
    form.insert("title", SEED_TITLE.to_string());
    form.insert("description", String::new());
    form.insert("state", status.to_string());
    form.insert("_csrf", csrf);
    let mut request = http
        .post(format!("{base}{url}"))
        .header(reqwest::header::COOKIE, combined)
        .form(&form);
    if htmx {
        request = request.header("HX-Request", "true");
    }
    let resp = request.send().await.expect("post edit url");
    store(world, resp).await;
}

async fn store(world: &mut FoundryWorld, resp: reqwest::Response) {
    world.last_status = Some(resp.status());
    world.last_headers = Some(resp.headers().clone());
    world.last_body = Some(resp.text().await.unwrap_or_default());
}

/// Sign Mei in and return `(session_pair, csrf_token)` — mirrors the
/// issue-edit-dialog harness (no cookie jar; re-authenticates per request).
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

/// Map a human column label ("Backlog", "In-Progress") to the stored state slug.
fn normalize_column(label: &str) -> &'static str {
    match label.trim().to_ascii_lowercase().as_str() {
        "backlog" => "backlog",
        "todo" => "todo",
        "in-progress" | "in_progress" => "in_progress",
        "done" => "done",
        other => panic!("unknown column label {other:?}"),
    }
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
