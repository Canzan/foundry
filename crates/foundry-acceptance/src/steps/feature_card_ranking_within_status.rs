//! card-ranking-within-status step definitions — slice 01: rank issue cards
//! within a status column (persisted, shared, per-`(project, state)` order).
//!
//! HARNESS BOUNDARY (distill/acceptance-review.md): HTTP-level (reqwest +
//! scraper), NOT a JS browser — the SAME split as issue-status-move. The suite
//! pins the persist contract (POST /state with an `after` neighbour key → store
//! `position` + `state`), the ordered board read, the zero-shuffle default, the
//! new-issue slot, and non-enumerability. The live drag gesture + optimistic
//! move/revert are browser-dogfooded (JS the HTTP harness can't drive).
//!
//! REUSES the shipped Background givens (`a workspace "Acme" … member "Mei" …
//! team "Backend"` from feature_board_new_issue, `Mei is signed in` from
//! us_07_project_create), `Mei fetches the "…" board` (feature_board_new_issue),
//! and from feature_issue_status_move: `"…" has state "…" in the store` and
//! `the board loads the drag-and-drop script`. Only the ranking-specific phrases
//! are new here (each globally unique). The HTTP helpers are copied/adapted from
//! feature_issue_status_move — `capture_drop_post` is extended with the optional
//! `&after=<key>` the drop handler sends (ADR-002 D2, watch-item R6: one body
//! shape for within- and cross-status).

use crate::support::harness::InProcHarness;
use crate::world::FoundryWorld;
use cucumber::gherkin::Step;
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
/// Title seeded onto the multi-issue Background rows.
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

// ----- Background: seed the project with several issues in named columns ------

#[given(regex = r#"^a project "([^"]+)" \(key "([^"]+)"\) with issues:$"#)]
async fn project_with_issues(
    world: &mut FoundryWorld,
    project: String,
    key_prefix: String,
    step: &Step,
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

    // Rows after the header: | key | column |. Newest-issue numbering: the
    // project's next_issue_number is set just past the seeded max so a freshly
    // filed issue (S6) gets a number ABOVE every seed (a realistic "newest").
    let table = step.table.as_ref().expect("issues data table");
    let seeds: Vec<(i32, &'static str)> = table
        .rows
        .iter()
        .skip(1)
        .map(|row| (number_of(&row[0]), normalize_column(&row[1])))
        .collect();
    let next_number = seeds.iter().map(|(n, _)| *n).max().unwrap_or(0) + 1;

    let project_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix, next_issue_number)
              VALUES ($1, $2, $3, $4, $5, $6, $7)",
    )
    .bind(project_id)
    .bind(team.0)
    .bind(ws.0)
    .bind(&project)
    .bind(slugify(&project))
    .bind(&key_prefix)
    .bind(next_number)
    .execute(pool)
    .await
    .expect("insert project");
    // board-lane-management sweep: raw-SQL projects need their lane rows.
    crate::support::harness::seed_lanes_for_project(pool, project_id).await;

    // Seed each issue with the DEFAULT position (0). The ordered read
    // (`position ASC, number DESC`) then yields number-DESC per column with no
    // explicit rank — the zero-shuffle default the migration reproduces (R7).
    for (number, state) in seeds {
        sqlx::query(
            "INSERT INTO issues (id, project_id, workspace_id, number, title, description_md, state, author_id)
                  VALUES ($1, $2, $3, $4, $5, '', $6, $7)",
        )
        .bind(uuid::Uuid::now_v7())
        .bind(project_id)
        .bind(ws.0)
        .bind(number)
        .bind(SEED_TITLE)
        .bind(state)
        .bind(author.0)
        .execute(pool)
        .await
        .expect("insert seed issue");
    }
}

#[given(regex = r#"^a foreign issue "([^"]+)" exists in another workspace$"#)]
async fn foreign_issue_exists(world: &mut FoundryWorld, key: String) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();

    // A wholly separate workspace/user/team/project so the issue's number is NOT
    // resolvable inside Sandbox (GEN). A drop aimed at it must refuse
    // non-enumerably — its existence in another tenant must not leak.
    let (prefix, _) = key.rsplit_once('-').expect("foreign key has -N");
    let ws_id = uuid::Uuid::now_v7();
    let user_id = uuid::Uuid::now_v7();
    let team_id = uuid::Uuid::now_v7();
    let project_id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO workspaces (id, name) VALUES ($1, 'Foreign Co')")
        .bind(ws_id)
        .execute(pool)
        .await
        .expect("insert foreign workspace");
    sqlx::query(
        "INSERT INTO users (id, email_lower, email_display, display_name, password_hash)
              VALUES ($1, 'zed@foreign.test', 'zed@foreign.test', 'Zed', 'x')",
    )
    .bind(user_id)
    .execute(pool)
    .await
    .expect("insert foreign user");
    sqlx::query("INSERT INTO teams (id, workspace_id, name, slug) VALUES ($1, $2, 'Ops', 'ops')")
        .bind(team_id)
        .bind(ws_id)
        .execute(pool)
        .await
        .expect("insert foreign team");
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, 'Foreign', 'foreign', $4)",
    )
    .bind(project_id)
    .bind(team_id)
    .bind(ws_id)
    .bind(prefix)
    .execute(pool)
    .await
    .expect("insert foreign project");
    // board-lane-management sweep: raw-SQL projects need their lane rows.
    crate::support::harness::seed_lanes_for_project(pool, project_id).await;
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, description_md, state, author_id)
              VALUES ($1, $2, $3, $4, 'Foreign issue', '', 'todo', $5)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(project_id)
    .bind(ws_id)
    .bind(number_of(&key))
    .bind(user_id)
    .execute(pool)
    .await
    .expect("insert foreign issue");
}

// ----- When: drops (within-status) as the drop handler would ------------------

#[when(regex = r#"^Mei drops "([^"]+)" after "([^"]+)" in "([^"]+)" as the drop handler would$"#)]
async fn drop_after(world: &mut FoundryWorld, key: String, neighbour: String, column: String) {
    capture_drop_post(world, &key, &column, Some(&neighbour)).await;
}

#[when(regex = r#"^Mei drops "([^"]+)" at the top of "([^"]+)" as the drop handler would$"#)]
async fn drop_at_top(world: &mut FoundryWorld, key: String, column: String) {
    capture_drop_post(world, &key, &column, None).await;
}

#[when(regex = r#"^a drop posts an unknown neighbour "([^"]+)" for "([^"]+)" in "([^"]+)"$"#)]
async fn drop_unknown_neighbour(
    world: &mut FoundryWorld,
    neighbour: String,
    key: String,
    column: String,
) {
    capture_drop_post(world, &key, &column, Some(&neighbour)).await;
}

#[when(regex = r#"^Mei files a new issue titled "([^"]*)"$"#)]
async fn files_new_issue(world: &mut FoundryWorld, title: String) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let (session_pair, csrf) = sign_in(harness, http).await;
    let base = harness.base_url();
    let combined = format!("{session_pair}; foundry_csrf={csrf}");
    let url = format!("/team/{TEAM_SLUG}/project/{PROJECT_SLUG}/issues");
    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("title", title);
    form.insert("_csrf", csrf);
    let resp = http
        .post(format!("{base}{url}"))
        .header(reqwest::header::COOKIE, combined)
        .header("hx-request", "true")
        .form(&form)
        .send()
        .await
        .expect("post new issue");
    store(world, resp).await;

    // Capture the freshly-minted key from the returned card so the "newest
    // issue is first" assertion can address it without a magic number.
    let body = world.last_body.clone().unwrap_or_default();
    let doc = Html::parse_fragment(&body);
    let selector = Selector::parse("article.issue-card[data-issue-key]").expect("valid selector");
    let created = doc
        .select(&selector)
        .next()
        .and_then(|c| c.value().attr("data-issue-key"))
        .map(|k| k.to_string());
    world.card_ranking_created_key = created;
}

// ----- Then: order in the served board HTML ----------------------------------

#[then(regex = r#"^the "([^"]+)" column shows cards in order "([^"]+)"$"#)]
async fn column_shows_order(world: &mut FoundryWorld, column: String, expected_csv: String) {
    let url = format!("/team/{TEAM_SLUG}/project/{PROJECT_SLUG}");
    capture_get(world, &url).await;
    let body = world.last_body.clone().unwrap_or_default();
    let doc = Html::parse_document(&body);
    let selector = Selector::parse(&format!("[data-column='{column}'] article.issue-card"))
        .expect("valid selector");
    let actual: Vec<String> = doc
        .select(&selector)
        .filter_map(|c| c.value().attr("data-issue-key").map(str::to_string))
        .collect();
    let expected: Vec<String> = expected_csv
        .split(',')
        .map(|s| s.trim().to_string())
        .collect();
    assert_eq!(
        actual, expected,
        "the {column:?} column must render cards in the ranked order {expected:?}, got {actual:?}: {body}"
    );
}

// ----- Then: the persisted per-(project,state) rank --------------------------

#[then(regex = r#"^"([^"]+)" is ranked after "([^"]+)" in the "([^"]+)" column in the store$"#)]
async fn ranked_after_in_store(
    world: &mut FoundryWorld,
    key: String,
    neighbour: String,
    _column: String,
) {
    let key_pos = read_issue_position(world, &key).await;
    let neighbour_pos = read_issue_position(world, &neighbour).await;
    assert_eq!(
        key_pos,
        neighbour_pos + 1,
        "{key} (position {key_pos}) must be ranked immediately after {neighbour} (position {neighbour_pos})"
    );
}

// ----- Then: non-enumerable refusal ------------------------------------------

#[then(regex = r#"^the response is a non-enumerable refusal$"#)]
async fn non_enumerable_refusal(world: &mut FoundryWorld) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::NOT_FOUND),
        "a foreign / unknown-neighbour drop must refuse with a 404-class response"
    );
    let body = world.last_body.clone().unwrap_or_default();
    assert!(
        !body.contains("GEN-") && !body.contains("ZZZ-"),
        "the refusal must not echo any issue key (no enumeration oracle): {body}"
    );
}

// ----- Then: the new-issue slot ----------------------------------------------

#[then(regex = r#"^the newest issue is first in the "([^"]+)" column$"#)]
async fn newest_first_in_column(world: &mut FoundryWorld, column: String) {
    let created = world
        .card_ranking_created_key
        .clone()
        .expect("a new issue must have been filed");
    let url = format!("/team/{TEAM_SLUG}/project/{PROJECT_SLUG}");
    capture_get(world, &url).await;
    let body = world.last_body.clone().unwrap_or_default();
    let doc = Html::parse_document(&body);
    let selector = Selector::parse(&format!("[data-column='{column}'] article.issue-card"))
        .expect("valid selector");
    let first = doc
        .select(&selector)
        .next()
        .and_then(|c| c.value().attr("data-issue-key").map(str::to_string));
    assert_eq!(
        first.as_deref(),
        Some(created.as_str()),
        "the freshly-filed {created:?} must be the FIRST card in the {column:?} column: {body}"
    );
}

// ----- Then: the drag-and-drop script must reach browsers (cache policy) -----
// Regression guard for the stale-JS bug: `/static/js/board-dnd.js` was served
// `immutable, max-age=1y` at a NON-content-hashed URL, so an edited handler was
// pinned stale in the browser and never re-fetched — the drag kept running the
// old logic. The app-owned JS must revalidate so JS fixes reach browsers. This
// is exactly the failure the HTTP persist-contract scenarios could not catch
// (they bypass the served JS + its cache header).

#[when(regex = r#"^the board drag-and-drop script is fetched$"#)]
async fn fetch_dnd_script(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let base = harness.base_url();
    let resp = http
        .get(format!("{base}/static/js/board-dnd.js"))
        .send()
        .await
        .expect("GET board-dnd.js");
    store(world, resp).await;
}

#[then(regex = r#"^it is served with a revalidating cache header so JS changes reach browsers$"#)]
async fn dnd_script_revalidates(world: &mut FoundryWorld) {
    assert_eq!(
        world.last_status,
        Some(StatusCode::OK),
        "board-dnd.js must be served"
    );
    let cc = world
        .last_headers
        .as_ref()
        .and_then(|h| h.get(reqwest::header::CACHE_CONTROL))
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();
    assert!(
        cc.contains("no-cache") && !cc.contains("immutable"),
        "the app-owned board-dnd.js must be served with a revalidating (non-immutable) cache \
         header so an edited handler reaches browsers — otherwise it is pinned stale behind its \
         unchanged URL for up to a year. Cache-Control was {cc:?}"
    );
}

// ----- internals: authenticated HTTP + DB reads ------------------------------

async fn read_issue_position(world: &mut FoundryWorld, key: &str) -> i32 {
    let (prefix, _) = key.rsplit_once('-').expect("issue key has -N");
    let number = number_of(key);
    let harness = world.harness.as_ref().expect("harness");
    let row: (i32,) = sqlx::query_as(
        "SELECT i.position
           FROM issues i
           JOIN projects p ON p.id = i.project_id
          WHERE p.key_prefix = $1 AND i.number = $2",
    )
    .bind(prefix)
    .bind(number)
    .fetch_one(harness.app.state.store.pool())
    .await
    .unwrap_or_else(|e| panic!("read issue {key} position from store: {e}"));
    row.0
}

async fn capture_get(world: &mut FoundryWorld, url: &str) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let (session_pair, _csrf) = sign_in(harness, http).await;
    let base = harness.base_url();
    let resp = http
        .get(format!("{base}{url}"))
        .header(reqwest::header::COOKIE, session_pair)
        .send()
        .await
        .expect("get target url");
    store(world, resp).await;
}

/// POST a state+position change the WAY the DnD drop handler does: the CSRF
/// token rides the `x-csrf-token` header, the body is a bare
/// `state=<slug>[&after=<key>]` urlencoded form (NO `_csrf` field). One body
/// shape for within- and cross-status drops (watch-item R6); `after` absent ⇒
/// drop at the top of the column.
async fn capture_drop_post(world: &mut FoundryWorld, key: &str, state: &str, after: Option<&str>) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let (session_pair, csrf) = sign_in(harness, http).await;
    let base = harness.base_url();
    let combined = format!("{session_pair}; foundry_csrf={csrf}");
    let url = format!(
        "/team/{TEAM_SLUG}/project/{PROJECT_SLUG}/issues/{number}/state",
        number = number_of(key)
    );
    let mut body = format!("state={state}");
    if let Some(neighbour) = after {
        body.push_str(&format!("&after={neighbour}"));
    }
    let resp = http
        .post(format!("{base}{url}"))
        .header(reqwest::header::COOKIE, combined)
        .header("x-csrf-token", csrf)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await
        .expect("post state url as the drop handler");
    store(world, resp).await;
}

async fn store(world: &mut FoundryWorld, resp: reqwest::Response) {
    world.last_status = Some(resp.status());
    world.last_headers = Some(resp.headers().clone());
    world.last_body = Some(resp.text().await.unwrap_or_default());
}

/// Sign Mei in and return `(session_pair, csrf_token)` — mirrors the
/// issue-status-move harness (no cookie jar; re-authenticates per request).
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
