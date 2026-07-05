//! issue-edit-dialog step definitions — click an issue card to edit its title +
//! description from a pre-filled modal, save = OOB card-replace in place.
//!
//! HARNESS BOUNDARY (distill/test-scenarios.md): HTTP-level (reqwest + scraper),
//! NOT a JS browser, so it cannot execute htmx. The scenarios pin (a) the card
//! `hx-get` WIRING (S1), (b) the pre-filled edit fragment CONTRACT (S2), (c) the
//! save endpoint contract END TO END at the store + response level (S3), (d)
//! validation (S4), tenancy non-enumerability (S5), and the no-JS plain-form
//! fallback (S6). The live click→dialog→save→card-update interaction + the modal
//! look are browser-dogfooded (walking-skeleton.md).
//!
//! REUSES the board-new-issue Background seed (`a workspace "Acme" … member
//! "Mei" … team "Backend"`), the `(\w+) is signed in` Given (us_07), and the
//! `Mei fetches the "Sandbox" board` / `the response redirects to the "…" board`
//! / `the response is the "…" error fragment` steps (feature_board_new_issue).
//! Only the edit-specific phrases are new here (each globally unique).

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

fn edit_path(team: &str, project: &str, number: i32) -> String {
    format!("/team/{team}/project/{project}/issues/{number}/edit")
}

// ----- Background: seed the project + its GEN-1 issue ------------------------

#[given(
    regex = r#"^a project "([^"]+)" \(key "([^"]+)"\) with an issue "([^"]+)" titled "([^"]*)" described "([^"]*)"$"#
)]
async fn project_with_issue(
    world: &mut FoundryWorld,
    project: String,
    key_prefix: String,
    issue_key: String,
    title: String,
    description: String,
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

    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, description_md, author_id)
              VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(project_id)
    .bind(ws.0)
    .bind(number_of(&issue_key))
    .bind(&title)
    .bind(&description)
    .bind(author.0)
    .execute(pool)
    .await
    .expect("insert issue");
}

/// S5 — seed an issue in a SEPARATE workspace (distinct team/project slugs) so
/// Mei's request for its path resolves to a team her acting workspace does not
/// contain → the uniform non-enumerable refusal.
#[given(regex = r#"^an issue "([^"]+)" exists in a DIFFERENT workspace from Mei$"#)]
async fn foreign_issue(world: &mut FoundryWorld, issue_key: String) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();

    let workspace_id = uuid::Uuid::now_v7();
    let user_id = uuid::Uuid::now_v7();
    let team_id = uuid::Uuid::now_v7();
    let project_id = uuid::Uuid::now_v7();
    let foreign_title = "Secret foreign title".to_string();
    let (prefix, _) = issue_key.rsplit_once('-').expect("foreign key has -N");
    let number = number_of(&issue_key);

    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'Rivals Inc')")
        .bind(workspace_id)
        .execute(pool)
        .await
        .expect("insert foreign workspace");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, 'rival@rivals.test', 'rival@rivals.test', 'Rival', 'x')",
    )
    .bind(user_id)
    .execute(pool)
    .await
    .expect("insert foreign user");
    sqlx::query(
        "INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'Rivals', 'rivals')",
    )
    .bind(team_id)
    .bind(workspace_id)
    .execute(pool)
    .await
    .expect("insert foreign team");
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, 'Secret', 'secret', $4)",
    )
    .bind(project_id)
    .bind(team_id)
    .bind(workspace_id)
    .bind(prefix)
    .execute(pool)
    .await
    .expect("insert foreign project");
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, description_md, author_id)
              VALUES ($1, $2, $3, $4, $5, 'foreign body', $6)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(project_id)
    .bind(workspace_id)
    .bind(number)
    .bind(&foreign_title)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("insert foreign issue");

    world.ied_foreign_path = Some(("rivals".to_string(), "secret".to_string(), number));
    world.ied_foreign_title = Some(foreign_title);
}

// ----- When: open the dialog / save / request a foreign path ----------------

#[when(regex = r#"^Mei opens the edit dialog for "([^"]+)"$"#)]
async fn open_dialog(world: &mut FoundryWorld, key: String) {
    let url = edit_path(TEAM_SLUG, PROJECT_SLUG, number_of(&key));
    capture_get(world, &url, true).await;
}

#[when(
    regex = r#"^Mei saves the edit dialog for "([^"]+)" with title "([^"]*)" and description "([^"]*)"$"#
)]
async fn save_dialog(world: &mut FoundryWorld, key: String, title: String, description: String) {
    let url = edit_path(TEAM_SLUG, PROJECT_SLUG, number_of(&key));
    capture_edit_post(world, &url, &title, &description, true).await;
}

#[when(regex = r#"^Mei saves the edit dialog for "([^"]+)" with an empty title$"#)]
async fn save_dialog_empty(world: &mut FoundryWorld, key: String) {
    let url = edit_path(TEAM_SLUG, PROJECT_SLUG, number_of(&key));
    capture_edit_post(world, &url, "", "ignored body", true).await;
}

#[when(regex = r#"^Mei requests the edit dialog for that issue's path$"#)]
async fn request_foreign_path(world: &mut FoundryWorld) {
    let (team, project, number) = world
        .ied_foreign_path
        .clone()
        .expect("the foreign issue path was seeded");
    let url = edit_path(&team, &project, number);
    capture_get(world, &url, true).await;
}

#[when(regex = r#"^Mei submits the edit form for "([^"]+)" as a plain form with title "([^"]*)"$"#)]
async fn submit_plain(world: &mut FoundryWorld, key: String, title: String) {
    let url = edit_path(TEAM_SLUG, PROJECT_SLUG, number_of(&key));
    capture_edit_post(world, &url, &title, "", false).await;
}

// ----- Then: S1 card wiring -------------------------------------------------

#[then(
    regex = r#"^the "([^"]+)" card carries an hx-get to its issue-edit endpoint targeting the modal container$"#
)]
async fn card_carries_edit_hx_get(world: &mut FoundryWorld, key: String) {
    let body = world.last_body.clone().unwrap_or_default();
    let doc = Html::parse_document(&body);
    let selector = Selector::parse(&format!("article.issue-card[data-issue-key='{key}']"))
        .expect("valid selector");
    let card = doc
        .select(&selector)
        .next()
        .unwrap_or_else(|| panic!("no issue card for {key:?}: {body}"));
    let hx_get = card
        .value()
        .attr("hx-get")
        .expect("the card must carry an hx-get to its edit endpoint");
    assert_eq!(
        hx_get,
        edit_path(TEAM_SLUG, PROJECT_SLUG, number_of(&key)),
        "card hx-get must be the absolute issue-edit endpoint"
    );
    assert_eq!(
        card.value().attr("hx-target"),
        Some("#modal-root"),
        "the card must target the #modal-root modal container"
    );
}

// ----- Then: S2 pre-filled dialog -------------------------------------------

#[then(regex = r#"^the dialog title field contains "([^"]*)"$"#)]
async fn dialog_title_field(world: &mut FoundryWorld, expected: String) {
    let body = world.last_body.clone().unwrap_or_default();
    let doc = Html::parse_fragment(&body);
    let selector = Selector::parse("form input[name='title']").expect("valid selector");
    let input = doc
        .select(&selector)
        .next()
        .unwrap_or_else(|| panic!("dialog has no title input: {body}"));
    assert_eq!(
        input.value().attr("value").unwrap_or_default(),
        expected,
        "the title field must be pre-filled with the issue's current title"
    );
}

#[then(regex = r#"^the dialog description field contains "([^"]*)"$"#)]
async fn dialog_description_field(world: &mut FoundryWorld, expected: String) {
    let body = world.last_body.clone().unwrap_or_default();
    let doc = Html::parse_fragment(&body);
    let selector = Selector::parse("form textarea[name='description']").expect("valid selector");
    let textarea = doc
        .select(&selector)
        .next()
        .unwrap_or_else(|| panic!("dialog has no description textarea: {body}"));
    let text: String = textarea.text().collect();
    assert_eq!(
        text, expected,
        "the description field must be pre-filled with the issue's current body"
    );
}

#[then(
    regex = r#"^the dialog form carries an hx-post to the save endpoint and the hidden "_csrf" field$"#
)]
async fn dialog_form_hx_post_and_csrf(world: &mut FoundryWorld) {
    let body = world.last_body.clone().unwrap_or_default();
    let doc = Html::parse_fragment(&body);
    let form = doc
        .select(&Selector::parse("form").expect("valid selector"))
        .next()
        .unwrap_or_else(|| panic!("dialog has no form: {body}"));
    assert_eq!(
        form.value().attr("hx-post"),
        Some(edit_path(TEAM_SLUG, PROJECT_SLUG, 1).as_str()),
        "the dialog form must hx-post to the save endpoint"
    );
    // no-JS fallback: a native method=post to the same action.
    assert!(
        form.value()
            .attr("method")
            .map(|m| m.eq_ignore_ascii_case("post"))
            .unwrap_or(false),
        "the dialog form must keep method=\"post\" for the no-JS fallback: {body}"
    );
    let csrf = Selector::parse("form input[type='hidden'][name='_csrf']").expect("valid selector");
    assert!(
        doc.select(&csrf).next().is_some(),
        "the dialog form must carry the hidden _csrf field: {body}"
    );
}

// ----- Then: S3 save persists + OOB card replace ----------------------------

#[then(
    regex = r#"^the issue "([^"]+)" has title "([^"]*)" and description "([^"]*)" in the store$"#
)]
async fn issue_has_title_and_description(
    world: &mut FoundryWorld,
    key: String,
    title: String,
    description: String,
) {
    let (stored_title, stored_description) = read_issue_fields(world, &key).await;
    assert_eq!(stored_title, title, "stored title mismatch");
    assert_eq!(
        stored_description, description,
        "stored description_md mismatch"
    );
}

#[then(
    regex = r#"^the response is an out-of-band card replacement keyed on "([^"]+)" showing "([^"]*)"$"#
)]
async fn response_is_oob_card_replace(world: &mut FoundryWorld, key: String, title: String) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "htmx save must return 200 OK"
    );
    let body = world.last_body.clone().unwrap_or_default();
    let marker = format!("hx-swap-oob=\"outerHTML:[data-issue-key='{key}']\"");
    assert!(
        body.contains(&marker),
        "response is not an OOB outerHTML replacement keyed on {key:?}: {body}"
    );
    let doc = Html::parse_fragment(&body);
    let selector = Selector::parse(&format!("article.issue-card[data-issue-key='{key}']"))
        .expect("valid selector");
    let card = doc
        .select(&selector)
        .next()
        .unwrap_or_else(|| panic!("OOB response has no card for {key:?}: {body}"));
    let text: String = card.text().collect();
    assert!(
        text.contains(&title),
        "the replaced card must show the new title {title:?}: {body}"
    );
    // R2: the replaced card stays clickable (re-emits its own hx-get).
    assert_eq!(
        card.value().attr("hx-get"),
        Some(edit_path(TEAM_SLUG, PROJECT_SLUG, number_of(&key)).as_str()),
        "the OOB-replaced card must keep its hx-get so it stays clickable"
    );
}

// ----- Then: S4 store unchanged / S6 store updated --------------------------

#[then(regex = r#"^the issue "([^"]+)" (?:still )?has title "([^"]*)" in the store$"#)]
async fn issue_has_title(world: &mut FoundryWorld, key: String, title: String) {
    let (stored_title, _) = read_issue_fields(world, &key).await;
    assert_eq!(stored_title, title, "stored title mismatch for {key}");
}

// ----- Then: S5 uniform non-enumerable refusal ------------------------------

#[then(regex = r#"^the response is the uniform not-found page with no echoed title$"#)]
async fn uniform_not_found_no_title(world: &mut FoundryWorld) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::NOT_FOUND),
        "a foreign issue must be refused with a uniform 404"
    );
    let body = world.last_body.clone().unwrap_or_default();
    // The shipped resource_not_found_page copy (ADR-003).
    assert!(
        body.contains("does not exist or is not available"),
        "refusal must be the uniform not-found page: {body}"
    );
    let foreign_title = world
        .ied_foreign_title
        .clone()
        .expect("foreign title seeded");
    assert!(
        !body.contains(&foreign_title),
        "the refusal must NOT echo the foreign issue's title (no enumeration oracle): {body}"
    );
}

// ----- internals: authenticated HTTP + DB reads -----------------------------

async fn read_issue_fields(world: &mut FoundryWorld, key: &str) -> (String, String) {
    let (prefix, _) = key.rsplit_once('-').expect("issue key has -N");
    let number = number_of(key);
    let harness = world.harness.as_ref().expect("harness");
    sqlx::query_as(
        "SELECT i.title, i.description_md
           FROM issues i
           JOIN projects p ON p.id = i.project_id
          WHERE p.key_prefix = $1 AND i.number = $2",
    )
    .bind(prefix)
    .bind(number)
    .fetch_one(harness.app.state.store.pool())
    .await
    .unwrap_or_else(|e| panic!("read issue {key} from store: {e}"))
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

async fn capture_edit_post(
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
    let resp = request.send().await.expect("post edit url");
    store(world, resp).await;
}

async fn store(world: &mut FoundryWorld, resp: reqwest::Response) {
    world.last_status = Some(resp.status());
    world.last_headers = Some(resp.headers().clone());
    world.last_body = Some(resp.text().await.unwrap_or_default());
}

/// Sign Mei in and return `(session_pair, csrf_token)` — mirrors the
/// board-new-issue harness (no cookie jar; re-authenticates per request).
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
