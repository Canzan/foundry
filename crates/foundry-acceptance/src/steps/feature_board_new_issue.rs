//! board-new-issue step definitions — wire the inert "New issue" board button.
//!
//! HARNESS BOUNDARY (distill/test-scenarios.md): this suite is HTTP-level
//! (reqwest + scraper), NOT a JS browser, so it cannot execute htmx. The
//! scenarios therefore pin (a) the WIRING attributes (S1 button `hx-get` +
//! `#modal-root`, S2 form `hx-post`), (b) the shipped endpoint CONTRACTS the
//! wiring depends on (S3 OOB Backlog card, S4 error fragment), and (c) the
//! no-JS plain-form fallback END TO END (S5). The live click→swap→close
//! interaction is verified by browser dogfood (walking-skeleton.md), mirroring
//! the us-12 "press c" split.
//!
//! Self-contained: seeds its own Acme / Backend / Sandbox fixture + member Mei
//! and drives GET/POST through the in-process axum harness. Every step phrase
//! is globally unique (a cucumber-rs requirement); the shared `<who> is signed
//! in` Background line is reused from `us_07_project_create`.

use crate::support::harness::InProcHarness;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use scraper::{Html, Selector};
use secrecy::SecretString;
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

// ----- Background: seed workspace + member + team + project ----------------

#[given(regex = r#"^a workspace "([^"]+)" exists with a member "([^"]+)" on team "([^"]+)"$"#)]
async fn workspace_member_team(
    world: &mut FoundryWorld,
    workspace: String,
    _member: String,
    team: String,
) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let workspace_id = uuid::Uuid::now_v7();
    let user_id = uuid::Uuid::now_v7();
    let team_id = uuid::Uuid::now_v7();
    let team_slug = slugify(&team);
    let hash = foundry_auth::hash_password(&SecretString::new(MEMBER_PASSWORD.to_string().into()))
        .await
        .expect("hash member pw");
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, $2)")
        .bind(workspace_id)
        .bind(&workspace)
        .execute(pool)
        .await
        .expect("insert workspace");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(user_id)
    .bind(MEI_EMAIL)
    .bind(MEI_EMAIL)
    .bind("Mei")
    .bind(&hash)
    .execute(pool)
    .await
    .expect("insert member");
    sqlx::query(
        "INSERT INTO workspace_memberships (workspace_id, user_id, role) VALUES ($1, $2, 'member')",
    )
    .bind(workspace_id)
    .bind(user_id)
    .execute(pool)
    .await
    .expect("insert workspace membership");
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, $3, $4)")
        .bind(team_id)
        .bind(workspace_id)
        .bind(&team)
        .bind(&team_slug)
        .execute(pool)
        .await
        .expect("insert team");
    sqlx::query("INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'member')")
        .bind(team_id)
        .bind(user_id)
        .execute(pool)
        .await
        .expect("insert team membership");
}

#[given(regex = r#"^a project "([^"]+)" with key prefix "([^"]+)" exists under "([^"]+)"$"#)]
async fn project_exists_under(
    world: &mut FoundryWorld,
    project: String,
    key_prefix: String,
    team: String,
) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let ws: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM workspaces LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("fetch workspace");
    let team_row: (uuid::Uuid,) =
        sqlx::query_as("SELECT id FROM teams WHERE workspace_id = $1 AND name = $2")
            .bind(ws.0)
            .bind(&team)
            .fetch_one(pool)
            .await
            .expect("fetch team");
    let project_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(project_id)
    .bind(team_row.0)
    .bind(ws.0)
    .bind(&project)
    .bind(slugify(&project))
    .bind(&key_prefix)
    .execute(pool)
    .await
    .expect("insert project");
    // board-lane-management sweep: raw-SQL projects need their lane rows.
    crate::support::harness::seed_lanes_for_project(pool, project_id).await;
}

// ----- When: fetch the board / modal, post an issue ------------------------

#[when(regex = r#"^Mei fetches the "([^"]+)" board$"#)]
async fn fetch_board(world: &mut FoundryWorld, project: String) {
    ensure_harness(world).await;
    let url = format!("/team/{TEAM_SLUG}/project/{}", slugify(&project));
    capture_get(world, &url, false).await;
}

#[when(regex = r#"^Mei fetches the new-issue modal for "([^"]+)"$"#)]
async fn fetch_modal(world: &mut FoundryWorld, project: String) {
    ensure_harness(world).await;
    let url = format!("/team/{TEAM_SLUG}/project/{}/issues/new", slugify(&project));
    capture_get(world, &url, true).await;
}

#[when(regex = r#"^Mei posts a new issue titled "([^"]*)" to "([^"]+)" as an htmx request$"#)]
async fn post_titled_htmx(world: &mut FoundryWorld, title: String, project: String) {
    ensure_harness(world).await;
    let url = format!("/team/{TEAM_SLUG}/project/{}/issues", slugify(&project));
    capture_post(world, &url, &title, true).await;
}

#[when(regex = r#"^Mei posts a new issue with an empty title to "([^"]+)" as an htmx request$"#)]
async fn post_empty_htmx(world: &mut FoundryWorld, project: String) {
    ensure_harness(world).await;
    let url = format!("/team/{TEAM_SLUG}/project/{}/issues", slugify(&project));
    capture_post(world, &url, "", true).await;
}

#[when(regex = r#"^Mei posts a new issue titled "([^"]*)" to "([^"]+)" as a plain form$"#)]
async fn post_titled_plain(world: &mut FoundryWorld, title: String, project: String) {
    ensure_harness(world).await;
    let url = format!("/team/{TEAM_SLUG}/project/{}/issues", slugify(&project));
    capture_post(world, &url, &title, false).await;
}

// ----- Then: S1 button wiring ---------------------------------------------

#[then(regex = r#"^the "New issue" button carries an hx-get to the new-issue modal endpoint$"#)]
async fn button_carries_hx_get(world: &mut FoundryWorld) {
    let hx_get = button_attr(world, "hx-get").expect("the New issue button must carry an hx-get");
    assert_eq!(
        hx_get,
        format!("/team/{TEAM_SLUG}/project/sandbox/issues/new"),
        "button hx-get must be the absolute modal endpoint, not a fragile relative URL"
    );
}

#[then(regex = r"^the button targets a modal container$")]
async fn button_targets_modal(world: &mut FoundryWorld) {
    assert_eq!(
        button_attr(world, "hx-target").as_deref(),
        Some("#modal-root"),
        "button must target the #modal-root modal container"
    );
}

#[then(regex = r"^the board contains a modal container element$")]
async fn board_has_modal_root(world: &mut FoundryWorld) {
    let body = world.last_body.as_deref().unwrap_or("");
    let doc = Html::parse_document(body);
    let selector = Selector::parse("#modal-root").expect("valid selector");
    assert!(
        doc.select(&selector).next().is_some(),
        "board is missing the #modal-root container the button swaps into"
    );
}

// ----- Then: S2 modal form wiring -----------------------------------------

#[then(regex = r"^the modal form carries an hx-post to the issues collection$")]
async fn modal_form_carries_hx_post(world: &mut FoundryWorld) {
    assert_eq!(
        form_attr(world, "hx-post"),
        Some(format!("/team/{TEAM_SLUG}/project/sandbox/issues")),
        "modal form must hx-post to the issues collection"
    );
}

#[then(regex = r#"^the modal form still carries method="post" and the hidden "_csrf" field$"#)]
async fn modal_form_keeps_method_and_csrf(world: &mut FoundryWorld) {
    let body = world.last_body.clone().unwrap_or_default();
    let doc = Html::parse_fragment(&body);
    let form_selector = Selector::parse("form").expect("valid selector");
    let form = doc
        .select(&form_selector)
        .next()
        .expect("modal must contain a form");
    let method = form.value().attr("method").unwrap_or_default();
    assert!(
        method.eq_ignore_ascii_case("post"),
        "no-JS fallback requires method=\"post\", found {method:?}"
    );
    let csrf_selector =
        Selector::parse("form input[type='hidden'][name='_csrf']").expect("valid selector");
    assert!(
        doc.select(&csrf_selector).next().is_some(),
        "modal form must keep the hidden _csrf field for the no-JS fallback"
    );
}

// ----- Then: S3 OOB create card -------------------------------------------

#[then(regex = r#"^the response is an out-of-band fragment targeting the "([^"]+)" column$"#)]
async fn response_is_oob_targeting_column(world: &mut FoundryWorld, column: String) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "htmx create must return 200 OK"
    );
    let body = world.last_body.as_deref().unwrap_or("");
    let marker = format!("hx-swap-oob=\"beforeend:[data-column='{column}']\"");
    assert!(
        body.contains(&marker),
        "response is not an out-of-band fragment for the {column:?} column: {body}"
    );
}

#[then(regex = r#"^it renders a card showing the key "([^"]+)" and the title "([^"]+)"$"#)]
async fn card_shows_key_and_title(world: &mut FoundryWorld, key: String, title: String) {
    let body = world.last_body.clone().unwrap_or_default();
    let doc = Html::parse_fragment(&body);
    let selector = Selector::parse(&format!("article.issue-card[data-issue-key='{key}']"))
        .expect("valid selector");
    let card = doc
        .select(&selector)
        .next()
        .unwrap_or_else(|| panic!("no issue card for key {key:?}: {body}"));
    let text: String = card.text().collect();
    assert!(text.contains(&key), "card missing key {key:?}: {body}");
    assert!(
        text.contains(&title),
        "card missing title {title:?}: {body}"
    );
}

// ----- Then: S4 error fragment --------------------------------------------

#[then(regex = r#"^the response is the "([^"]+)" error fragment$"#)]
async fn response_is_error_fragment(world: &mut FoundryWorld, message: String) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::BAD_REQUEST),
        "empty title must be rejected with 400"
    );
    let body = world.last_body.clone().unwrap_or_default();
    let doc = Html::parse_fragment(&body);
    let selector = Selector::parse("div.error[data-hx-fragment='issue-create-error']")
        .expect("valid selector");
    let fragment = doc
        .select(&selector)
        .next()
        .unwrap_or_else(|| panic!("no issue-create-error fragment: {body}"));
    let text: String = fragment.text().collect();
    assert!(
        text.contains(&message),
        "error fragment missing {message:?}: {body}"
    );
}

#[then(regex = r"^the response is not a board and contains no issue card$")]
async fn response_not_board_no_card(world: &mut FoundryWorld) {
    let body = world.last_body.clone().unwrap_or_default();
    let doc = Html::parse_document(&body);
    let column_selector = Selector::parse("[data-column]").expect("valid selector");
    assert!(
        doc.select(&column_selector).next().is_none(),
        "error response must not render a board column: {body}"
    );
    let card_selector = Selector::parse("article.issue-card").expect("valid selector");
    assert!(
        doc.select(&card_selector).next().is_none(),
        "error response must not render an issue card: {body}"
    );
}

// ----- Then: S5 no-JS fallback --------------------------------------------

#[then(regex = r#"^the response redirects to the "([^"]+)" board$"#)]
async fn response_redirects_to_board(world: &mut FoundryWorld, project: String) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::SEE_OTHER),
        "plain-form create must 303 redirect"
    );
    let location = world
        .last_headers
        .as_ref()
        .and_then(|h| h.get(reqwest::header::LOCATION))
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert_eq!(
        location,
        format!("/team/{TEAM_SLUG}/project/{}", slugify(&project)),
        "redirect Location must point at the board"
    );
}

#[then(regex = r#"^fetching the board shows "([^"]+)" in the Backlog column$"#)]
async fn board_shows_title_in_backlog(world: &mut FoundryWorld, title: String) {
    ensure_harness(world).await;
    let url = format!("/team/{TEAM_SLUG}/project/sandbox");
    capture_get(world, &url, false).await;
    let body = world.last_body.clone().unwrap_or_default();
    let doc = Html::parse_document(&body);
    let selector = Selector::parse("[data-column='backlog'] article.issue-card .title")
        .expect("valid selector");
    let found = doc
        .select(&selector)
        .any(|el| el.text().collect::<String>().contains(&title));
    assert!(
        found,
        "the Backlog column does not show a card titled {title:?}: {body}"
    );
}

// ----- internals: DOM lookups + authenticated HTTP -------------------------

/// Read an attribute off the board's single `New issue` button from the last
/// captured board body. Returns an owned value so no DOM borrow escapes.
fn button_attr(world: &FoundryWorld, attr: &str) -> Option<String> {
    let body = world.last_body.as_deref().unwrap_or("");
    let doc = Html::parse_document(body);
    let selector = Selector::parse("button[data-action='new-issue']").expect("valid selector");
    let button = doc
        .select(&selector)
        .next()
        .expect("the board must render the New issue button");
    button.value().attr(attr).map(str::to_string)
}

/// Read an attribute off the modal's single `<form>` from the last captured
/// modal body. Returns an owned value so no DOM borrow escapes.
fn form_attr(world: &FoundryWorld, attr: &str) -> Option<String> {
    let body = world.last_body.as_deref().unwrap_or("");
    let doc = Html::parse_fragment(body);
    let selector = Selector::parse("form").expect("valid selector");
    let form = doc
        .select(&selector)
        .next()
        .expect("the modal must render a form");
    form.value().attr(attr).map(str::to_string)
}

async fn capture_get(world: &mut FoundryWorld, url: &str, htmx: bool) {
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

async fn capture_post(world: &mut FoundryWorld, url: &str, title: &str, htmx: bool) {
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let (session_pair, csrf) = sign_in(harness, http).await;
    let base = harness.base_url();
    let combined = format!("{session_pair}; foundry_csrf={csrf}");
    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("title", title.to_string());
    form.insert("_csrf", csrf);
    let mut request = http
        .post(format!("{base}{url}"))
        .header(reqwest::header::COOKIE, combined)
        .form(&form);
    if htmx {
        request = request.header("HX-Request", "true");
    }
    let resp = request.send().await.expect("post target url");
    store(world, resp).await;
}

async fn store(world: &mut FoundryWorld, resp: reqwest::Response) {
    world.last_status = Some(resp.status());
    world.last_headers = Some(resp.headers().clone());
    world.last_body = Some(resp.text().await.unwrap_or_default());
}

/// Sign Mei in and return `(session_pair, csrf_token)`: the `foundry_session=…`
/// cookie pair to carry on subsequent requests plus the double-submit CSRF token
/// (its matching `foundry_csrf` cookie is replayed by the caller on POSTs).
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
