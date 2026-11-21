//! ui-bare-pages — every page the operator lands on looks like Foundry and
//! tells her what it is for (`tests/features/ui-bare-pages.feature`).
//!
//! WHAT THE STEPS BELOW ARE ALLOWED TO KNOW. Public facts of one running
//! application, and nothing else:
//!
//!   * the body one address answers for what the asking client DECLARED about
//!     its destination, and what that answer tells a cache it depends on;
//!   * the shortcuts an answer advertises, in order, read the same way out of
//!     both bodies;
//!   * what the browser PAINTS around what she came for — the frame's width,
//!     where it sits between the two edges of the window, and whether the
//!     product name is drawn above the page's heading inside it. Never a class
//!     name: a page can carry the class and still run edge to edge;
//!   * the intent a served field declares for the browser;
//!   * the text, the chip and the heading of an issue's own page, with the
//!     expected escaping computed HERE rather than read back off the render.
//!
//! THE EXPECTED VALUES ARE WRITTEN OUT. The shortcut list is written in the
//! feature file rather than read off one of the two answers, because an answer
//! compared against an answer agrees with itself even when both lost a row.
//!
//! EVERY READING IS GUARDED AGAINST PASSING ON NOTHING. A reading that found no
//! row, no field, no frame and no text would otherwise satisfy "nothing is
//! wrong" over a page that rendered nothing at all, so each step asserts its
//! population before it asserts the law over it.
//!
//! Reused unchanged, per the globally-unique-phrase rule (cucumber-rs) — DO NOT
//! redefine here:
//!   - `a workspace "…" exists with a member "…" on team "…"`  (feature_board_new_issue)
//!   - `a project "…" with key prefix "…" exists under "…"`    (feature_board_new_issue)

use crate::support::browser_harness;
use crate::support::harness::{establish_session, get_with_cookie, InProcHarness};
use crate::support::html_assertions as html;
use crate::world::FoundryWorld;
use cucumber::gherkin::Step;
use cucumber::{given, then, when};
use foundry_app::Clock;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use secrecy::SecretString;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

const TEST_NOW: &str = "2026-01-15T12:00:00Z";
const MEI_EMAIL: &str = "mei@acme.com";
const MEI_PASSWORD: &str = "mei-correct-horse-battery-staple";
const TEAM_SLUG: &str = "backend";
const PROJECT_SLUG: &str = "sandbox";
const KEY_PREFIX: &str = "GEN";
const PRODUCT_NAME: &str = "Foundry";

/// The one phone device these readings are taken on, and the desk window the
/// shipped browser lane fixes.
const PHONE_WIDTH_PX: u32 = 390;
const PHONE_HEIGHT_PX: u32 = 844;

/// Sub-pixel slack on a box read from a live layout.
const SLACK_PX: f64 = 1.0;
/// The smallest gap at desk width that reads as a margin the operator can see.
const DESK_MARGIN_PX: f64 = 16.0;
/// The widest gap a full-width phone card may keep on each side: the page's own
/// padding, not a margin.
const PHONE_GAP_PX: f64 = 48.0;

/// The seed the generated descriptions are drawn from, and how many are drawn.
/// Recorded here so a failure replays exactly: the failing text is printed with
/// this seed and its index.
const DESCRIPTION_SEED: u64 = 20_261_121;
const DESCRIPTION_CASES: usize = 24;
/// The shortest text a draw may be. The page is read for the text it CONTAINS,
/// so a run of one or two characters would already stand somewhere in any
/// served document and could not tell a page that shows the description from a
/// page that shows none. Every drawn text is long enough to be a reading of its
/// own.
const DESCRIPTION_MIN_LEN: usize = 24;
const LONG_WORD_LEN: usize = 4000;

fn now_anchor() -> time::OffsetDateTime {
    time::OffsetDateTime::parse(TEST_NOW, &time::format_description::well_known::Rfc3339)
        .expect("parse anchor")
}

async fn ensure_harness(world: &mut FoundryWorld) {
    if world.harness.is_none() {
        world.harness = Some(InProcHarness::spawn(now_anchor()).await);
    }
    if world.http.is_none() {
        world.http = Some(
            reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .cookie_store(false)
                .build()
                .expect("build reqwest client"),
        );
    }
}

fn harness_of(world: &FoundryWorld) -> &InProcHarness {
    world
        .harness
        .as_ref()
        .expect("the Background must have spawned the application")
}

fn http_of(world: &FoundryWorld) -> &reqwest::Client {
    world.http.as_ref().expect("an http client")
}

// ==========================================================================
// The record each stimulus leaves for its own assertions
// ==========================================================================

fn remember(world: &mut FoundryWorld, record: serde_json::Value) {
    world.last_body = Some(record.to_string());
}

fn recalled(world: &FoundryWorld) -> serde_json::Value {
    let raw = world
        .last_body
        .clone()
        .expect("the stimulus of this scenario recorded what it read");
    serde_json::from_str(&raw).expect("the record is the one the stimulus wrote")
}

fn text_at(record: &serde_json::Value, key: &str) -> String {
    record[key].as_str().unwrap_or_default().to_string()
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ==========================================================================
// The pages a signed-out visitor can land on
// ==========================================================================

/// The live address of each page the feature names, minted the way the operator
/// would reach it: a real bootstrap token, a real signed invite, and a real
/// reset link taken out of the message the application sent.
async fn address_of(world: &mut FoundryWorld, page: &str) -> String {
    ensure_harness(world).await;
    match page {
        "the sign-in page" => "/sign-in".to_string(),
        "the forgotten-password page" => "/forgot-password".to_string(),
        "the keyboard shortcut list" => "/keyboard-help".to_string(),
        "the claim-your-workspace page" => {
            let harness = harness_of(world);
            let raw = "a-live-claim-token";
            let hash: [u8; 32] = {
                let mut h = Sha256::new();
                h.update(raw.as_bytes());
                h.finalize().into()
            };
            let expires_at = harness.fake_clock.now() + time::Duration::minutes(30);
            harness
                .app
                .state
                .store
                .insert_bootstrap_token(uuid::Uuid::now_v7(), &hash, expires_at)
                .await
                .expect("mint the first-admin claim token this page is reached by");
            format!("/bootstrap?token={}", urlencoding::encode(raw))
        }
        "the invitation page where she sets her password" => {
            let harness = harness_of(world);
            let store = harness.app.state.store.clone();
            let invite_id = uuid::Uuid::now_v7();
            let expires_at = harness.fake_clock.now() + time::Duration::days(7);
            let throwaway = foundry_auth::hash_password(&SecretString::new(
                "never-seen-initial-credential".to_string().into(),
            ))
            .await
            .expect("hash the throwaway initial credential");
            store
                .provision_workspace(
                    uuid::Uuid::now_v7(),
                    "Northwind",
                    uuid::Uuid::now_v7(),
                    "priya@northwind.test",
                    "priya@northwind.test",
                    "Priya Nair",
                    &throwaway,
                    invite_id,
                    expires_at,
                )
                .await
                .expect("provision the workspace whose invitation this page belongs to");
            let secret = harness.app.state.session_secret.clone();
            let token = foundry_auth::InviteToken::new(invite_id, expires_at, &secret)
                .expect("mint the live invitation signature");
            format!(
                "/invites/accept?id={invite_id}&sig={}",
                urlencoding::encode(&token.signature)
            )
        }
        "the page where she chooses a new password" => reset_link(world).await,
        other => panic!(
            "the row names {other:?}, which no step knows how to reach — write it the way the \
             others are written rather than letting the scenario read some other page"
        ),
    }
}

/// Ask for a reset the way she does, and take the link out of the message the
/// application actually sent her.
async fn reset_link(world: &mut FoundryWorld) -> String {
    let base = harness_of(world).base_url();
    let http = http_of(world).clone();
    let form_page = http
        .get(format!("{base}/forgot-password"))
        .send()
        .await
        .expect("open the forgotten-password page");
    let csrf = form_page
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|s| s.starts_with("foundry_csrf="))
        .and_then(|s| s.strip_prefix("foundry_csrf="))
        .and_then(|rest| rest.split(';').next())
        .expect("the forgotten-password page mints an anti-forgery cookie")
        .to_string();
    let mut form = HashMap::new();
    form.insert("email", MEI_EMAIL.to_string());
    form.insert("_csrf", csrf.clone());
    http.post(format!("{base}/forgot-password"))
        .header(reqwest::header::COOKIE, format!("foundry_csrf={csrf}"))
        .form(&form)
        .send()
        .await
        .expect("ask for a reset link");
    let delivery = harness_of(world)
        .fake_email
        .last_to(MEI_EMAIL)
        .expect("the application sent her a reset message");
    let link = delivery
        .body
        .split_whitespace()
        .find(|word| word.contains("/reset-password?token="))
        .expect("the reset message carries her reset link")
        .to_string();
    let token = reqwest::Url::parse(&link)
        .expect("the reset link parses")
        .query_pairs()
        .find(|(k, _)| k == "token")
        .map(|(_, v)| v.into_owned())
        .expect("the reset link carries a token");
    format!("/reset-password?token={}", urlencoding::encode(&token))
}

// ==========================================================================
// The shortcut list — one address, two bodies, one cache contract
// ==========================================================================

fn declared_destination(declaring: &str) -> Option<&'static str> {
    match declaring {
        "as a page it is opening" => Some("document"),
        "as a script fetching it" => Some("empty"),
        "as a frame loading it" => Some("iframe"),
        "declaring nothing at all" => None,
        other => panic!("no step knows how a client asks {other:?}"),
    }
}

async fn ask_for_the_shortcut_list(
    world: &mut FoundryWorld,
    declaring: &str,
) -> (u16, String, String) {
    ensure_harness(world).await;
    let base = harness_of(world).base_url();
    let mut request = http_of(world).get(format!("{base}/keyboard-help"));
    if let Some(destination) = declared_destination(declaring) {
        request = request.header("sec-fetch-dest", destination);
    }
    let response = request.send().await.expect("ask for the shortcut list");
    let status = response.status().as_u16();
    let vary = response
        .headers()
        .get_all(reqwest::header::VARY)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .collect::<Vec<_>>()
        .join(", ");
    let body = response.text().await.unwrap_or_default();
    (status, body, vary)
}

#[when(
    regex = r"^a client asks for the keyboard shortcut list (as a page it is opening|as a script fetching it|as a frame loading it|declaring nothing at all)$"
)]
async fn when_a_client_asks_for_the_shortcut_list(world: &mut FoundryWorld, declaring: String) {
    let (status, body, vary) = ask_for_the_shortcut_list(world, &declaring).await;
    remember(
        world,
        serde_json::json!({ "declaring": declaring, "status": status, "body": body, "vary": vary }),
    );
}

/// The shortcuts a body advertises, in the order it writes them: each key read
/// together with the line that FOLLOWS it, which is the line a reader takes as
/// its meaning. A key the body leaves with nothing after it is advertised with
/// nothing beside it, so a body that writes more keys than meanings is read as
/// the short run it is instead of being quietly cut to the shorter of the two.
fn advertised_shortcuts(body: &str) -> AdvertisedShortcuts {
    let doc = html::parse(body);
    html::select_all(&doc, "[data-shortcut]")
        .iter()
        .map(|key| {
            let beside = key
                .next_siblings()
                .find_map(scraper::ElementRef::wrap)
                .map(|following| one_line(&following.text().collect::<String>()))
                .unwrap_or_default();
            (one_line(&key.text().collect::<String>()), beside)
        })
        .collect()
}

fn assert_answered_something(record: &serde_json::Value) {
    let status = record["status"].as_u64().unwrap_or(0);
    let body = text_at(record, "body");
    assert!(
        status == 200 && !body.trim().is_empty(),
        "WHAT: asking for the shortcut list {:?} answered {status} with a body of {} characters. \
         WHY: with nothing answered there is nothing for the readings below to hold of, so a green \
         here would mean nothing. FIX: the address is public and answers every client.",
        text_at(record, "declaring"),
        body.len()
    );
}

#[then(regex = r"^it is given (a framed page|the bare list)$")]
async fn then_it_is_given(world: &mut FoundryWorld, body_shape: String) {
    let record = recalled(world);
    assert_answered_something(&record);
    let declaring = text_at(&record, "declaring");
    let body = text_at(&record, "body");
    let lower = body.to_ascii_lowercase();
    let wrapped = lower.contains("<html") || lower.contains("<!doctype html");
    let shortcuts = advertised_shortcuts(&body);
    let doc = html::parse(&body);
    let styled = !html::select_all(&doc, "link[rel='stylesheet'][href^='/static/css/']").is_empty();
    let own_heading = !html::select_all(&doc, "h1").is_empty();
    let dialog = !html::select_all(
        &doc,
        "section.keyboard-help[role='dialog'][aria-label='Keyboard shortcuts']",
    )
    .is_empty();
    assert!(
        !shortcuts.is_empty(),
        "WHAT: the answer to a client asking {declaring:?} advertises no shortcut at all. \
         WHY: whichever body it is, the shortcut list is what the operator came for. \
         FIX: both bodies carry the shipped rows. Answer was:\n{body}"
    );
    if body_shape == "a framed page" {
        assert!(
            wrapped && styled && own_heading,
            "WHAT: a client asking {declaring:?} is given a body that is a whole document: \
             {wrapped}, draws Foundry's own stylesheet: {styled}, and carries its own heading: \
             {own_heading}. WHY: a browser opening this address paints the answer as the whole \
             page, so a bare run of keys in browser-default type is the whole of what she sees. \
             FIX: answer a declared page destination with the framed page that extends the shared \
             head and the card frame. Answer was:\n{body}"
        );
    } else {
        assert!(
            !wrapped && dialog,
            "WHAT: a client asking {declaring:?} is given a body with a document wrapper: \
             {wrapped}, carrying the labelled dialog the overlay needs: {dialog}. \
             WHY: this answer is written straight into a live page; a whole document nests a \
             second head and body inside the page the operator is already on. FIX: answer every \
             destination but a declared page with the bare list, exactly as today. \
             Answer was:\n{body}"
        );
    }
}

#[then(regex = r"^the answer tells a cache it depends on what the client declared$")]
async fn then_the_answer_names_its_dependency(world: &mut FoundryWorld) {
    let record = recalled(world);
    assert_answered_something(&record);
    let declaring = text_at(&record, "declaring");
    let vary = text_at(&record, "vary");
    assert!(
        vary.to_ascii_lowercase().contains("sec-fetch-dest"),
        "WHAT: the answer to a client asking {declaring:?} tells a cache it depends on {vary:?}. \
         WHY: one address answers two bodies, chosen from what the client declared, so a cache \
         that is not told which declaration the answer depends on is free to hand the page body \
         to the overlay or the bare list to a browser opening the address — and only the OTHER \
         reader ever sees that. FIX: name the declared destination as the answer's dependency on \
         BOTH bodies, beside the one decision that chooses between them."
    );
}

#[when(regex = r"^the shortcut list is asked for both as a page and as a script fetching it$")]
async fn when_the_list_is_asked_for_both_ways(world: &mut FoundryWorld) {
    let (page_status, page_body, _) =
        ask_for_the_shortcut_list(world, "as a page it is opening").await;
    let (script_status, script_body, _) =
        ask_for_the_shortcut_list(world, "as a script fetching it").await;
    remember(
        world,
        serde_json::json!({
            "pageStatus": page_status,
            "pageBody": page_body,
            "scriptStatus": script_status,
            "scriptBody": script_body,
        }),
    );
}

/// The shortcuts one answer advertises: each key and what it says that key does,
/// in the order the body writes them.
type AdvertisedShortcuts = Vec<(String, String)>;

fn both_answers(record: &serde_json::Value) -> (AdvertisedShortcuts, AdvertisedShortcuts) {
    (
        advertised_shortcuts(&text_at(record, "pageBody")),
        advertised_shortcuts(&text_at(record, "scriptBody")),
    )
}

#[then(regex = r"^both answers advertise exactly these shortcuts, in this order$")]
async fn then_both_answers_advertise_exactly(world: &mut FoundryWorld, step: &Step) {
    let record = recalled(world);
    let table = step
        .table
        .as_ref()
        .expect("this step is written with its table of shortcuts");
    let wanted: Vec<(String, String)> = table
        .rows
        .iter()
        .skip(1)
        .map(|row| (one_line(&row[0]), one_line(&row[1])))
        .collect();
    let (page, script) = both_answers(&record);
    for (whose, found) in [("the page", &page), ("the overlay's list", &script)] {
        assert_eq!(
            found, &wanted,
            "WHAT: {whose} advertises {found:?}; it must advertise exactly {wanted:?}. \
             WHY: a key the operator is told about and cannot press, or a key she can press and is \
             never told about, is worse than no list at all. FIX: derive both bodies from the one \
             shipped table through one builder and render the one row owner, so neither body can \
             write a list of its own."
        );
    }
}

#[then(regex = r"^the two answers advertise the identical shortcuts in the identical order$")]
async fn then_the_two_answers_agree(world: &mut FoundryWorld) {
    let record = recalled(world);
    let (page, script) = both_answers(&record);
    assert!(
        !page.is_empty() && !script.is_empty(),
        "WHAT: one of the two answers advertises nothing at all — the page: {} rows, the \
         overlay's list: {} rows. WHY: comparing an empty list with an empty list agrees with \
         itself, so this reading would pass over two bodies that advertise nothing. \
         FIX: both bodies carry the shipped rows.",
        page.len(),
        script.len()
    );
    assert_eq!(
        page, script,
        "WHAT: the page advertises {page:?} while the overlay's list advertises {script:?}. \
         WHY: the operator meets both bodies at the same address and must not learn two different \
         sets of keys from them. FIX: one builder over the shipped table, one row owner, included \
         by both."
    );
}

// ==========================================================================
// What the browser is told each field holds
// ==========================================================================

fn field_selector(field: &str) -> &'static str {
    match field {
        "her email" => "input[type='email']",
        "her password" | "her new password" => "input[type='password'][name='password']",
        "the confirmation of her new password" => "input[type='password'][name='confirm']",
        other => panic!("no step knows which field {other:?} is"),
    }
}

fn intent_token(intent: &str) -> Option<&'static str> {
    match intent {
        "her username" => Some("username"),
        "her current password" => Some("current-password"),
        "a password she is creating" => Some("new-password"),
        "nothing" => None,
        other => panic!("no step knows what {other:?} means to a browser"),
    }
}

#[when(
    regex = r"^a visitor is served (the sign-in page|the forgotten-password page|the claim-your-workspace page|the invitation page where she sets her password|the page where she chooses a new password)$"
)]
async fn when_a_visitor_is_served(world: &mut FoundryWorld, page: String) {
    let address = address_of(world, &page).await;
    let base = harness_of(world).base_url();
    let response = http_of(world)
        .get(format!("{base}{address}"))
        .send()
        .await
        .expect("open the page a visitor lands on");
    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();
    assert_eq!(
        status, 200,
        "WHAT: {page} answered {status}. WHY: a page that did not render cannot be read for what \
         it tells the browser, and a reading of an error page would pass for want of a field. \
         FIX: this is a defect of the scenario's own setup, not of the page."
    );
    remember(
        world,
        serde_json::json!({ "page": page, "address": address, "body": body }),
    );
}

#[then(regex = r#"^the page tells the browser that "([^"]+)" holds (.+)$"#)]
async fn then_the_page_declares_the_field_intent(
    world: &mut FoundryWorld,
    field: String,
    intent: String,
) {
    let record = recalled(world);
    let page = text_at(&record, "page");
    let body = text_at(&record, "body");
    let selector = field_selector(&field);
    let doc = html::parse(&body);
    let found = html::select_all(&doc, selector);
    assert_eq!(
        found.len(),
        1,
        "WHAT: {page} carries {n} fields answering to {field:?}. WHY: with no such field, or more \
         than one, the reading below would say nothing about the field the promise names. \
         FIX: this is a defect of the scenario, not of the page. Page was:\n{body}",
        n = found.len()
    );
    let declared = found[0].value().attr("autocomplete").map(str::to_string);
    let wanted = intent_token(&intent).map(str::to_string);
    assert_eq!(
        declared, wanted,
        "WHAT: on {page} the field {field:?} declares {declared:?}; it must declare {wanted:?}. \
         WHY: this is the only thing the page can say to her password manager — declare nothing \
         and she types her password by hand on every sign-in; declare the stored-secret intent \
         where a password is being CREATED and the manager offers her an old secret instead of \
         generating and saving a new one. FIX: declare the intent of the flow the field belongs \
         to, and leave every other input alone."
    );
}

// ==========================================================================
// An issue's own page
// ==========================================================================

const DESCRIPTION_TWO_LINES: &str = "Safari drops the refresh token.\nChrome keeps it.";
const ISSUE_TITLE: &str = "Refresh token rotation broken on Safari";

/// Put an issue in the store the page reads from, in the state the row names,
/// and read it back: an insert that silently failed would leave the page
/// reading some other issue.
async fn store_an_issue(
    world: &mut FoundryWorld,
    title: &str,
    description: &str,
    lane: &str,
) -> i32 {
    ensure_harness(world).await;
    let pool = harness_of(world).app.state.store.pool();
    let project: (uuid::Uuid, uuid::Uuid) =
        sqlx::query_as("SELECT id, workspace_id FROM projects WHERE key_prefix = $1")
            .bind(KEY_PREFIX)
            .fetch_one(pool)
            .await
            .expect("her project exists");
    let author: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(MEI_EMAIL)
        .fetch_one(pool)
        .await
        .expect("she exists");
    let number: (i32,) =
        sqlx::query_as("SELECT COALESCE(MAX(number), 0) + 1 FROM issues WHERE project_id = $1")
            .bind(project.0)
            .fetch_one(pool)
            .await
            .expect("the next number on her board");
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, description_md, state, author_id)
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(project.0)
    .bind(project.1)
    .bind(number.0)
    .bind(title)
    .bind(description)
    .bind(lane)
    .bind(author.0)
    .execute(pool)
    .await
    .expect("write the issue this scenario is about");
    let stored: (String, String, String) = sqlx::query_as(
        "SELECT title, description_md, state FROM issues WHERE project_id = $1 AND number = $2",
    )
    .bind(project.0)
    .bind(number.0)
    .fetch_one(pool)
    .await
    .expect("read back the issue this scenario is about");
    assert_eq!(
        (stored.0.as_str(), stored.1.as_str(), stored.2.as_str()),
        (title, description, lane),
        "the issue was not stored as this scenario wrote it, so the page would be read against \
         something else"
    );
    number.0
}

fn issue_address(number: i32) -> String {
    format!("/team/{TEAM_SLUG}/project/{PROJECT_SLUG}/issues/{number}")
}

fn issue_key(number: i32) -> String {
    format!("{KEY_PREFIX}-{number}")
}

async fn open_issue_page(world: &mut FoundryWorld, number: i32) -> (u16, String) {
    let harness = harness_of(world);
    let http = http_of(world);
    let session = establish_session(harness, http, MEI_EMAIL, MEI_PASSWORD).await;
    let answer = get_with_cookie(harness, http, &issue_address(number), &session).await;
    world.dash_session_cookie = Some(session);
    (answer.status.as_u16(), answer.body)
}

#[given(
    regex = r"^an issue whose description runs over two lines, sitting in (backlog|todo|in_progress|done)$"
)]
async fn given_an_issue_in_a_lane(world: &mut FoundryWorld, lane: String) {
    let number = store_an_issue(world, ISSUE_TITLE, DESCRIPTION_TWO_LINES, &lane).await;
    remember(
        world,
        serde_json::json!({ "number": number, "lane": lane, "title": ISSUE_TITLE }),
    );
}

#[when(regex = r"^the operator opens that issue's own page$")]
async fn when_she_opens_that_issue(world: &mut FoundryWorld) {
    let record = recalled(world);
    let number = record["number"].as_i64().expect("the issue's number") as i32;
    let (status, body) = open_issue_page(world, number).await;
    assert_eq!(
        status, 200,
        "WHAT: her own issue's page answered {status}. WHY: a refusal cannot be read for the \
         issue, and a reading of a refusal page would pass for want of content. FIX: this is a \
         defect of the scenario's own setup."
    );
    let mut next = record.clone();
    next["body"] = serde_json::Value::String(body);
    remember(world, next);
}

#[then(regex = r"^the page is headed by the issue's key alone$")]
async fn then_the_heading_is_the_key(world: &mut FoundryWorld) {
    let record = recalled(world);
    let number = record["number"].as_i64().expect("the issue's number") as i32;
    let body = text_at(&record, "body");
    let doc = html::parse(&body);
    let headings: Vec<String> = html::select_all(&doc, "h1")
        .iter()
        .map(|el| one_line(&el.text().collect::<String>()))
        .collect();
    assert_eq!(
        headings,
        vec![issue_key(number)],
        "WHAT: the page is headed by {headings:?}; it must be headed by {:?} and nothing else. \
         WHY: the heading is how she knows which issue she is on, and it is what every other \
         reading of this page is matched against. FIX: leave the key alone in the heading and give \
         the title a heading of its own below it.",
        issue_key(number)
    );
}

#[then(regex = r"^the page shows the issue's title$")]
async fn then_the_page_shows_the_title(world: &mut FoundryWorld) {
    let record = recalled(world);
    let body = text_at(&record, "body");
    let title = text_at(&record, "title");
    let doc = html::parse(&body);
    let shown: Vec<String> = html::select_all(&doc, "h2")
        .iter()
        .map(|el| one_line(&el.text().collect::<String>()))
        .collect();
    assert!(
        shown.contains(&one_line(&title)),
        "WHAT: the page shows the headings {shown:?}; none of them is the issue's title \
         {title:?}. WHY: a page headed by a key alone tells her nothing about the issue; the \
         title is what she came to read. FIX: show the stored title below the key."
    );
}

#[then(regex = r"^the page shows what she wrote, line breaks and all, as text$")]
async fn then_the_page_shows_what_she_wrote(world: &mut FoundryWorld) {
    let record = recalled(world);
    let body = text_at(&record, "body");
    if let Some(complaint) = description_complaint(&body, DESCRIPTION_TWO_LINES) {
        panic!(
            "WHAT: the page does not show what she wrote: {complaint}. WHY: she opened the issue \
             to read it, so the page owes her all of her text, as text and never as markup, with \
             the line breaks she typed. FIX: show the stored description, escaped by the template \
             engine, wrapped with its line breaks kept and never shortened."
        );
    }
}

/// Line endings as a parser leaves them: a document parser turns a carriage
/// return into a line feed, so both sides of every reading below are brought to
/// the same ending before they are compared.
fn same_endings(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

/// THE ONE READING OF A DESCRIPTION, used by the named issue and by every
/// generated text. `None` means the page shows it as she wrote it.
///
/// Escaping is read twice over, with a reference computed HERE: her text reaches
/// the page's TEXT (markup let through unescaped would have become elements and
/// left the text without those characters), and a script she wrote is carried in
/// its escaped form with no live script of hers anywhere in the document.
fn description_complaint(body: &str, stored: &str) -> Option<String> {
    if stored.is_empty() {
        return None;
    }
    let stored = same_endings(stored);
    let doc = html::parse(body);
    let page_text = same_endings(&doc.root_element().text().collect::<String>());
    let lines: Vec<&str> = stored.split('\n').filter(|l| !l.is_empty()).collect();
    for line in &lines {
        if !page_text.contains(line) {
            return Some(format!(
                "the {} characters starting {:?} never reach the page, which holds {} characters \
                 of text in all",
                line.chars().count(),
                line.chars().take(60).collect::<String>(),
                page_text.chars().count()
            ));
        }
    }
    if lines.len() >= 2 {
        let flattened = lines.join("");
        let kept = page_text.contains(&stored) || body.to_ascii_lowercase().contains("<br");
        if !kept && page_text.contains(&flattened) {
            return Some("her line breaks are flattened into one line".to_string());
        }
    }
    if stored.to_ascii_lowercase().contains("<script") {
        if !body.to_ascii_lowercase().contains("&lt;script") {
            return Some(
                "the script she wrote about is not carried in the escaped form a page must write \
                 it in"
                    .to_string(),
            );
        }
        let live: Vec<String> = html::select_all(&doc, "script")
            .iter()
            .map(|el| el.text().collect::<String>())
            .filter(|text| text.contains("alert"))
            .collect();
        if !live.is_empty() {
            return Some(format!(
                "the script she wrote about is drawn as a live script: {live:?}"
            ));
        }
    }
    None
}

#[given(regex = r"^an issue with nothing written down, beside one that has a description$")]
async fn given_an_empty_and_a_filled_issue(world: &mut FoundryWorld) {
    let filled = store_an_issue(world, ISSUE_TITLE, DESCRIPTION_TWO_LINES, "backlog").await;
    let empty = store_an_issue(world, "Nothing written down yet", "", "backlog").await;
    remember(
        world,
        serde_json::json!({ "filled": filled, "empty": empty, "number": empty }),
    );
}

#[when(regex = r"^the operator opens both issues' own pages$")]
async fn when_she_opens_both_pages(world: &mut FoundryWorld) {
    let record = recalled(world);
    let filled = record["filled"].as_i64().expect("the filled issue") as i32;
    let empty = record["empty"].as_i64().expect("the empty issue") as i32;
    let (filled_status, filled_body) = open_issue_page(world, filled).await;
    let (empty_status, empty_body) = open_issue_page(world, empty).await;
    assert!(
        filled_status == 200 && empty_status == 200,
        "WHAT: her two issue pages answered {filled_status} and {empty_status}. WHY: a refusal \
         cannot be compared with a page. FIX: this is a defect of the scenario's own setup."
    );
    let mut next = record.clone();
    next["filledBody"] = serde_json::Value::String(filled_body);
    next["emptyBody"] = serde_json::Value::String(empty_body);
    remember(world, next);
}

/// WHERE THE DESCRIPTION LIVES, discovered from the page that HAS one rather
/// than assumed: the INNERMOST element whose own text is the stored
/// description. Anything wrapped around that element reads back exactly the
/// same text, so a wrapper and the line inside it are one place, not two, and
/// only the innermost matches are counted. The shape of that place is then what
/// the empty page is read for, so the law "the area is always there" is read
/// without naming anything the page was not already observed to use.
fn description_area_shape(body: &str, stored: &str) -> (String, usize) {
    let doc = html::parse(body);
    let carrying: Vec<_> = html::select_all(&doc, "*")
        .into_iter()
        .filter(|element| one_line(&element.text().collect::<String>()) == one_line(stored))
        .collect();
    let innermost: Vec<_> = carrying
        .iter()
        .filter(|element| {
            !carrying.iter().any(|other| {
                other.id() != element.id() && element.descendants().any(|n| n.id() == other.id())
            })
        })
        .collect();
    let shape = innermost
        .last()
        .map(|element| {
            format!(
                "{}|{}",
                element.value().name(),
                element.value().attr("class").unwrap_or_default()
            )
        })
        .unwrap_or_default();
    (shape, innermost.len())
}

/// Every place on a body of the same SHAPE: the same tag, carrying each of the
/// classes the filled page's description area carried. Presence rather than an
/// exact class string, because an empty area that keeps its own class and adds
/// the shipped empty-state one is the same place, drawn empty.
fn elements_of_shape(body: &str, shape: &str) -> Vec<String> {
    let (name, classes) = shape.split_once('|').unwrap_or((shape, ""));
    let wanted: Vec<&str> = classes.split_whitespace().collect();
    let doc = html::parse(body);
    html::select_all(&doc, name)
        .iter()
        .filter(|el| {
            let carried: Vec<&str> = el
                .value()
                .attr("class")
                .unwrap_or_default()
                .split_whitespace()
                .collect();
            wanted.iter().all(|one| carried.contains(one))
        })
        .map(|el| one_line(&el.text().collect::<String>()))
        .collect()
}

#[then(regex = r"^both pages hold exactly one place for the description$")]
async fn then_both_pages_hold_one_description_area(world: &mut FoundryWorld) {
    let record = recalled(world);
    let filled_body = text_at(&record, "filledBody");
    let empty_body = text_at(&record, "emptyBody");
    let (shape, count) = description_area_shape(&filled_body, DESCRIPTION_TWO_LINES);
    assert_eq!(
        count, 1,
        "WHAT: the page of the issue she wrote a description on holds {count} places carrying \
         exactly that text. WHY: none at all and she cannot read her issue; two and the page \
         contradicts itself about what the issue says. FIX: show the stored description in one \
         place on the page."
    );
    let on_the_empty_page = elements_of_shape(&empty_body, &shape);
    assert_eq!(
        on_the_empty_page.len(),
        1,
        "WHAT: the issue with nothing written down holds {n} places for a description, while the \
         issue that has one holds exactly one. WHY: an area that disappears when nothing is stored \
         leaves her unable to tell an empty issue from a page that forgot to show it. \
         FIX: draw the description area always, with its own copy when nothing is stored.",
        n = on_the_empty_page.len()
    );
    let mut next = record.clone();
    next["emptyAreaText"] = serde_json::Value::String(on_the_empty_page[0].clone());
    remember(world, next);
}

#[then(regex = r"^the empty one tells her nothing is written down yet$")]
async fn then_the_empty_area_says_so(world: &mut FoundryWorld) {
    let record = recalled(world);
    let copy = text_at(&record, "emptyAreaText");
    assert!(
        !copy.trim().is_empty(),
        "WHAT: the description area of the issue with nothing written down is blank. \
         WHY: a blank area reads as a page that failed to load the issue, not as an issue nobody \
         has described yet. FIX: give the empty area the same kind of copy the attachment and \
         comment areas already carry when there is nothing to show."
    );
}

#[then(regex = r#"^the page shows where the issue sits in Foundry's own chip, reading "([^"]+)"$"#)]
async fn then_the_chip_reads_the_lane(world: &mut FoundryWorld, lane: String) {
    let record = recalled(world);
    let body = text_at(&record, "body");
    let doc = html::parse(&body);
    let chips: Vec<(String, String)> = html::select_all(&doc, "span.state[data-state]")
        .iter()
        .map(|el| {
            (
                el.value()
                    .attr("data-state")
                    .unwrap_or_default()
                    .to_string(),
                one_line(&el.text().collect::<String>()),
            )
        })
        .collect();
    assert_eq!(
        chips,
        vec![(lane.clone(), lane.clone())],
        "WHAT: the page shows the status chips {chips:?}; it must show exactly one carrying \
         {lane:?} as the state it marks AND as what it reads. WHY: the chip is how she knows where \
         the issue sits, and it is the same chip she already knows from her board — two shapes of \
         it would drift apart. FIX: embed the one shipped chip, filled from the stored state."
    );
}

#[when(regex = r"^the operator follows Delete from that issue's own page$")]
async fn when_she_follows_delete(world: &mut FoundryWorld) {
    let record = recalled(world);
    let number = record["number"].as_i64().expect("the issue's number") as i32;
    let (status, body) = open_issue_page(world, number).await;
    assert_eq!(status, 200, "her own issue's page answered {status}");
    let doc = html::parse(&body);
    let anchors = html::select_all(&doc, "a[data-action='delete-issue'][href]");
    assert_eq!(
        anchors.len(),
        1,
        "WHAT: the page offers {n} Delete controls she can follow. WHY: following the address is \
         the whole mechanism of this control — with scripting on or off, the browser follows it. \
         FIX: keep the Delete control an anchor carrying its address; a presentation class must \
         not replace the mechanism.",
        n = anchors.len()
    );
    let href = anchors[0]
        .value()
        .attr("href")
        .unwrap_or_default()
        .to_string();
    let session = world
        .dash_session_cookie
        .clone()
        .expect("the page she read it from established her session");
    let answer = get_with_cookie(harness_of(world), http_of(world), &href, &session).await;
    let mut next = record.clone();
    next["confirmStatus"] = serde_json::Value::from(answer.status.as_u16());
    next["confirmBody"] = serde_json::Value::String(answer.body);
    remember(world, next);
}

#[then(regex = r"^she is asked to confirm before anything is deleted$")]
async fn then_she_is_asked_to_confirm(world: &mut FoundryWorld) {
    let record = recalled(world);
    let status = record["confirmStatus"].as_u64().unwrap_or(0);
    let body = text_at(&record, "confirmBody");
    let doc = html::parse(&body);
    let forms = html::select_all(&doc, "form[method='post'][action*='/delete']");
    assert!(
        status == 200 && !forms.is_empty(),
        "WHAT: following Delete answered {status} with {n} forms asking her to confirm. \
         WHY: one press must never delete her work outright, and the address she follows is what \
         carries her to the question. FIX: leave the Delete control's address and its destination \
         exactly as they are.",
        n = forms.len()
    );
}

// ==========================================================================
// Whatever she wrote — generated text from a recorded seed
// ==========================================================================

/// The texts this law is read over: the four the feature names, then texts drawn
/// from [`DESCRIPTION_SEED`] over the characters a description can hold —
/// markup, entities, quotes, line breaks and long unbroken runs.
fn descriptions_of_every_shape() -> Vec<(String, String)> {
    let mut texts = vec![
        ("nothing at all".to_string(), String::new()),
        (
            "something that looks like a script".to_string(),
            "before <script>alert('x')</script> after".to_string(),
        ),
        (
            "two lines ended the way a browser ends them".to_string(),
            "first line\r\nsecond line\r\n".to_string(),
        ),
        (
            "one unbroken word of four thousand letters".to_string(),
            "a".repeat(LONG_WORD_LEN),
        ),
    ];
    let alphabet: Vec<char> = "<>&\"'`\n\r \tabcdeèπ日םשعر\\/{}[]=-_0123456789"
        .chars()
        .collect();
    let mut rng = StdRng::seed_from_u64(DESCRIPTION_SEED);
    for case in 0..DESCRIPTION_CASES {
        let len = rng.gen_range(DESCRIPTION_MIN_LEN..=120);
        let text: String = (0..len)
            .map(|_| alphabet[rng.gen_range(0..alphabet.len())])
            .collect();
        texts.push((format!("drawn text {case}"), text));
    }
    texts
}

#[given(regex = r"^she has written descriptions of every shape, including these$")]
async fn given_descriptions_of_every_shape(world: &mut FoundryWorld, step: &Step) {
    let table = step
        .table
        .as_ref()
        .expect("this step is written with the texts it must always include");
    let named: Vec<String> = table
        .rows
        .iter()
        .skip(1)
        .map(|row| one_line(&row[0]))
        .collect();
    let texts = descriptions_of_every_shape();
    for name in &named {
        assert!(
            texts.iter().any(|(label, _)| label == name),
            "WHAT: the feature names the text {name:?}, which this reading never writes. \
             WHY: the named texts are the shapes a reader of the feature is promised were tried. \
             FIX: write that shape into the texts this law is read over."
        );
    }
    let mut written = Vec::new();
    for (label, text) in texts {
        let number = store_an_issue(world, "A piece of work she described", &text, "backlog").await;
        written.push(serde_json::json!({ "label": label, "text": text, "number": number }));
    }
    remember(world, serde_json::json!({ "written": written }));
}

#[when(regex = r"^each of those issues' own pages is opened$")]
async fn when_each_of_those_pages_is_opened(world: &mut FoundryWorld) {
    let record = recalled(world);
    let written = record["written"]
        .as_array()
        .cloned()
        .expect("the texts this law is read over were written");
    let mut read = Vec::new();
    for item in written {
        let number = item["number"].as_i64().expect("its number") as i32;
        let (status, body) = open_issue_page(world, number).await;
        assert_eq!(
            status, 200,
            "WHAT: the page of the issue described by {:?} answered {status}. WHY: a refusal \
             cannot be read for her text. FIX: this is a defect of the scenario's own setup.",
            item["label"]
        );
        read.push(serde_json::json!({
            "label": item["label"].clone(),
            "text": item["text"].clone(),
            "body": body,
        }));
    }
    remember(world, serde_json::json!({ "read": read }));
}

#[then(regex = r"^every page shows that description as text, whole, with its line breaks$")]
async fn then_every_page_shows_it_as_written(world: &mut FoundryWorld) {
    let record = recalled(world);
    let read = record["read"]
        .as_array()
        .cloned()
        .expect("the pages were read");
    assert!(
        read.len() >= DESCRIPTION_CASES,
        "WHAT: only {n} descriptions were read, fewer than the {DESCRIPTION_CASES} drawn plus the \
         named ones. WHY: a law read over too few texts is a law read over the examples alone. \
         FIX: this is a defect of the scenario.",
        n = read.len()
    );
    // ONE failure is reported SHORTEST-FIRST, so the text printed is the least
    // text that still breaks the law rather than whichever one happened to be
    // drawn first. Seed and index are printed with it so it replays exactly.
    let mut offenders: Vec<(usize, String, String, String)> = Vec::new();
    for item in &read {
        let text = item["text"].as_str().unwrap_or_default().to_string();
        let body = item["body"].as_str().unwrap_or_default().to_string();
        let label = item["label"].as_str().unwrap_or_default().to_string();
        if let Some(complaint) = description_complaint(&body, &text) {
            offenders.push((text.chars().count(), label, text, complaint));
        }
    }
    offenders.sort_by_key(|(len, _, _, _)| *len);
    assert!(
        offenders.is_empty(),
        "WHAT: {n} of the {total} descriptions she wrote are not shown as she wrote them. The \
         shortest offending text is {label:?}, {len} characters, {text:?} — {complaint}. \
         WHY: she wrote it, so the page owes her all of it, as text and never as markup, with the \
         line breaks she typed. FIX: show the stored text escaped by the template engine and \
         wrapped with its line breaks kept; never shorten it. Replay: seed {DESCRIPTION_SEED}.",
        n = offenders.len(),
        total = read.len(),
        label = offenders[0].1,
        len = offenders[0].0,
        text = offenders[0].2.chars().take(120).collect::<String>(),
        complaint = offenders[0].3
    );
}

// ==========================================================================
// THE BROWSER LANE — what she actually sees
// ==========================================================================

/// ONE live reading of what the browser PAINTS around what she came for. The
/// card is not looked up by class: it is the first box above the page's heading
/// that the stylesheet gives a surface, a border or a shadow of its own, which
/// is what the operator sees as a card.
const CARD_READING: &str = r#"
const body = document.body;
const pageBg = getComputedStyle(body).backgroundColor;
const rect = (el) => { if (!el) return null; const r = el.getBoundingClientRect();
  return { left: r.left, right: r.right, top: r.top, bottom: r.bottom, width: r.width, height: r.height }; };
const painted = (el) => {
  const s = getComputedStyle(el);
  const bg = s.backgroundColor;
  const surface = bg && bg !== 'rgba(0, 0, 0, 0)' && bg !== 'transparent' && bg !== pageBg;
  const border = parseFloat(s.borderTopWidth || '0') > 0 || parseFloat(s.borderLeftWidth || '0') > 0;
  const shadow = s.boxShadow && s.boxShadow !== 'none';
  return !!(surface || border || shadow);
};
const headings = Array.from(document.querySelectorAll('h1'));
const anchor = headings[0] || document.querySelector('form') || null;
const frames = [];
let walk = anchor ? anchor.parentElement : null;
while (walk && walk !== body && walk !== document.documentElement) {
  if (painted(walk)) frames.push(walk);
  walk = walk.parentElement;
}
const card = frames.length ? frames[0] : null;
let brand = null;
if (card && anchor) {
  const anchorTop = anchor.getBoundingClientRect().top;
  const candidates = Array.from(card.querySelectorAll('*')).filter((el) => {
    if (el === anchor || el.contains(anchor)) return false;
    const own = (el.textContent || '').trim();
    if (!own.includes(PRODUCT)) return false;
    if (Array.from(el.children).some((kid) => (kid.textContent || '').includes(PRODUCT))) return false;
    const r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0 && r.top <= anchorTop + 1;
  });
  if (candidates.length) brand = { text: (candidates[0].textContent || '').trim(), box: rect(candidates[0]) };
}
const sheets = Array.from(document.querySelectorAll("link[rel='stylesheet']")).map((l) => l.getAttribute('href') || '');
let loadedRules = 0;
for (const sheet of Array.from(document.styleSheets)) {
  try { if (sheet.href && sheet.href.includes('/static/css/')) loadedRules += sheet.cssRules.length; } catch (err) {}
}
const controls = Array.from(document.querySelectorAll('input:not([type=hidden]), button, select, textarea')).map(rect);
return {
  heading: headings.length ? (headings[0].textContent || '').trim() : null,
  headingCount: headings.length,
  frameCount: frames.length,
  card: rect(card),
  brand: brand,
  viewport: window.innerWidth,
  docWidth: document.documentElement.scrollWidth,
  controls: controls,
  rail: !!document.querySelector('.sidebar, nav#sidebar-nav'),
  themeControl: !!document.querySelector('.theme-toggle'),
  stylesheets: sheets,
  loadedRules: loadedRules,
  doctype: !!document.doctype,
  pageBg: pageBg
};
"#;

/// ONE live reading of the shortcut list as the operator meets it on the page.
const SHORTCUT_READING: &str = r#"
const rect = (el) => { if (!el) return null; const r = el.getBoundingClientRect();
  return { left: r.left, right: r.right, top: r.top, bottom: r.bottom, width: r.width, height: r.height }; };
const keys = Array.from(document.querySelectorAll('[data-shortcut]'));
const rows = keys.map((key) => {
  const meaning = key.nextElementSibling;
  return {
    key: (key.textContent || '').trim(),
    meaning: meaning ? (meaning.textContent || '').trim() : null,
    keyBox: rect(key),
    meaningBox: rect(meaning)
  };
});
const instruction = document.querySelector('.kb-selection-instruction');
return { rows: rows, instruction: instruction ? (instruction.textContent || '').trim() : null,
         instructionBox: rect(instruction), url: window.location.pathname };
"#;

/// ONE live reading of the two controls the issue page carries beside the issue.
const CONTROL_READING: &str = r#"
const body = document.body;
const pageBg = getComputedStyle(body).backgroundColor;
const pageFont = getComputedStyle(body).fontFamily;
const del = document.querySelector("a[data-action='delete-issue']");
const file = document.querySelector("input[type='file']");
const look = (el) => {
  if (!el) return null;
  const s = getComputedStyle(el);
  return { fill: s.backgroundColor, borderWidth: parseFloat(s.borderTopWidth || '0'),
           padInline: parseFloat(s.paddingLeft || '0'), font: s.fontFamily, radius: s.borderRadius };
};
return { delete: look(del), file: look(file), pageBg: pageBg, pageFont: pageFont };
"#;

fn browser_of(world: &FoundryWorld) -> &fantoccini::Client {
    world
        .browser
        .as_ref()
        .expect("a browser session must have been opened first")
}

async fn read_with(browser: &fantoccini::Client, script: &str) -> serde_json::Value {
    let source = format!("const PRODUCT = {PRODUCT_NAME:?};\n{script}");
    browser
        .execute(&source, Vec::new())
        .await
        .expect("take the live reading this scenario is about")
}

async fn open_in_browser(world: &mut FoundryWorld, address: &str) {
    let base = harness_of(world).base_url();
    let browser = browser_of(world);
    browser
        .goto(&format!("{base}{address}"))
        .await
        .unwrap_or_else(|err| panic!("open {address}: {err}"));
}

/// The one page of the family no address reaches: she is sent to it by asking
/// for a reset, so the scenario asks for one the way she does.
async fn land_on_the_check_your_email_page(world: &mut FoundryWorld) {
    open_in_browser(world, "/forgot-password").await;
    let browser = browser_of(world);
    let field = browser
        .find(fantoccini::Locator::Css("input[type='email']"))
        .await
        .unwrap_or_else(|err| panic!("the forgotten-password page offers no email field: {err}"));
    field
        .send_keys(MEI_EMAIL)
        .await
        .unwrap_or_else(|err| panic!("her email could not be typed: {err}"));
    browser
        .find(fantoccini::Locator::Css("form button[type='submit']"))
        .await
        .unwrap_or_else(|err| panic!("the forgotten-password page offers nothing to press: {err}"))
        .click()
        .await
        .unwrap_or_else(|err| panic!("the reset could not be asked for: {err}"));
    wait_until_heading_changes(browser, "Forgot password").await;
}

/// A bounded wait on the page she is now looking at: the heading she came from
/// is gone. Never a sleep on a guess, and never an unbounded loop.
async fn wait_until_heading_changes(browser: &fantoccini::Client, was: &str) {
    for _ in 0..50 {
        let heading = browser
            .execute(
                "const h = document.querySelector('h1'); return h ? h.textContent.trim() : '';",
                Vec::new(),
            )
            .await
            .map(|v| v.as_str().unwrap_or_default().to_string())
            .unwrap_or_default();
        if !heading.is_empty() && heading != was {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("the page she pressed through to never arrived; it still reads {was:?}");
}

#[given(
    regex = r"^the operator is signed out, looking at (the sign-in page|the forgotten-password page|the check-your-email page|the claim-your-workspace page|the invitation page where she sets her password|the page where she chooses a new password|the keyboard shortcut list) on (a desk|a phone)$"
)]
async fn given_signed_out_looking_at(world: &mut FoundryWorld, page: String, device: String) {
    let address = if page == "the check-your-email page" {
        String::new()
    } else {
        address_of(world, &page).await
    };
    ensure_harness(world).await;
    let browser = if device == "a phone" {
        browser_harness::open_mobile_session_sized(PHONE_WIDTH_PX, PHONE_HEIGHT_PX).await
    } else {
        browser_harness::new_session().await
    };
    world.browser = Some(browser);
    if page == "the check-your-email page" {
        land_on_the_check_your_email_page(world).await;
    } else {
        open_in_browser(world, &address).await;
    }
    let reading = read_with(browser_of(world), CARD_READING).await;
    remember(
        world,
        serde_json::json!({ "page": page, "device": device, "reading": reading }),
    );
}

#[given(
    regex = r"^the operator is signed out, looking at the sign-in page on a desk that asks for the dark palette$"
)]
async fn given_signed_out_in_the_dark(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    let browser = browser_harness::new_dark_session().await;
    assert!(
        browser_harness::device_prefers_dark(&browser).await,
        "the screen this row is about does not ask for the dark palette, so it would read the \
         light one twice and call one of them dark"
    );
    world.browser = Some(browser);
    open_in_browser(world, "/sign-in").await;
    let reading = read_with(browser_of(world), CARD_READING).await;
    remember(
        world,
        serde_json::json!({ "page": "the sign-in page", "device": "a desk", "reading": reading }),
    );
}

fn card_reading(world: &FoundryWorld) -> (String, String, serde_json::Value) {
    let record = recalled(world);
    (
        text_at(&record, "page"),
        text_at(&record, "device"),
        record["reading"].clone(),
    )
}

fn number_at(reading: &serde_json::Value, path: &[&str]) -> f64 {
    let mut cursor = reading;
    for key in path {
        cursor = &cursor[*key];
    }
    cursor.as_f64().unwrap_or(f64::NAN)
}

#[then(regex = r"^the page is one full document drawn with Foundry's own stylesheet$")]
async fn then_the_page_is_a_full_styled_document(world: &mut FoundryWorld) {
    let (page, _, reading) = card_reading(world);
    let own: Vec<String> = reading["stylesheets"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .filter(|href| href.starts_with("/static/css/"))
        .collect();
    let rules = reading["loadedRules"].as_u64().unwrap_or(0);
    assert!(
        reading["doctype"].as_bool().unwrap_or(false) && !own.is_empty() && rules > 0,
        "WHAT: {page} is a whole document: {}, draws the application's own stylesheets {own:?}, \
         and the browser resolved {rules} rules out of them. WHY: a body that is not a document, \
         or a document whose stylesheet never loaded, is browser-default type and no frame at all \
         — which is the whole of what the operator sees. FIX: render the page from the shared head \
         and link the stylesheet from the application's own static path.",
        reading["doctype"]
    );
}

#[then(regex = r"^what she came for sits in one centred card under the product name$")]
async fn then_it_sits_in_one_centred_card(world: &mut FoundryWorld) {
    let (page, device, reading) = card_reading(world);
    assert_eq!(
        reading["headingCount"].as_u64().unwrap_or(0),
        1,
        "WHAT: {page} carries {} headings of its own. WHY: the page says what it is for once; a \
         second one is the product name pretending to be a heading, and no heading at all leaves \
         her reading a form with no name. FIX: keep the page's own single heading and draw the \
         product name as a line above it.",
        reading["headingCount"]
    );
    assert_eq!(
        reading["frameCount"].as_u64().unwrap_or(0),
        1,
        "WHAT: {page} draws {} painted frames around what she came for, at {device}. \
         WHY: none at all and the form runs edge to edge as an unfinished page; two and the card \
         is boxed inside another box. FIX: draw exactly one card — a surface of its own, centred, \
         with a width of its own. Reading: {reading}",
        reading["frameCount"]
    );
    let brand = reading["brand"].clone();
    assert!(
        brand.is_object(),
        "WHAT: {page} draws no product name above its heading inside the card. WHY: this is often \
         the first page anyone ever sees of the application; a form with no name on it says \
         nothing about what she is signing in to. FIX: draw the product name as a line at the top \
         of the card, above the page's own heading. Reading: {reading}"
    );
    let (left, right, width) = (
        number_at(&reading, &["card", "left"]),
        number_at(&reading, &["card", "right"]),
        number_at(&reading, &["card", "width"]),
    );
    let viewport = number_at(&reading, &["viewport"]);
    let gap_right = viewport - right;
    assert!(
        width > 0.0 && (left - gap_right).abs() <= 2.0,
        "WHAT: the card on {page} runs from {left:.0} to {right:.0} in a {viewport:.0} px window \
         at {device}, so it keeps {left:.0} px on the left and {gap_right:.0} px on the right. \
         WHY: a card pushed to one side reads as a page that broke, not as a page that was \
         designed. FIX: centre it with the same margin on both sides."
    );
    if device == "a phone" {
        assert!(
            left > 0.0 && left <= PHONE_GAP_PX && width >= viewport - 2.0 * PHONE_GAP_PX,
            "WHAT: on her phone the card on {page} is {width:.0} px wide inside a {viewport:.0} px \
             screen, with {left:.0} px beside it. WHY: on a phone she has no width to spare — the \
             card takes the screen inside the page's own padding, and a desk-width margin here \
             wastes a third of what she can read. FIX: let the card fill the width inside the page \
             padding below the phone breakpoint."
        );
    } else {
        assert!(
            left >= DESK_MARGIN_PX && width < viewport - 2.0 * DESK_MARGIN_PX,
            "WHAT: at desk width the card on {page} is {width:.0} px wide inside a {viewport:.0} \
             px window, with {left:.0} px beside it. WHY: a form stretched across a desk window is \
             the bare page this Request exists to replace; she reads a line of a form, not a page \
             of it. FIX: give the card a width of its own and a margin she can see."
        );
    }
}

#[then(regex = r"^nothing of the page runs off the side of her screen$")]
async fn then_nothing_runs_off_the_side(world: &mut FoundryWorld) {
    let (page, device, reading) = card_reading(world);
    let viewport = number_at(&reading, &["viewport"]);
    let document = number_at(&reading, &["docWidth"]);
    assert!(
        document <= viewport + SLACK_PX,
        "WHAT: {page} lays out {document:.0} px across a {viewport:.0} px screen at {device}, so \
         it slides sideways. WHY: a page that slides sideways hides half of itself behind the edge \
         of the screen, and she cannot tell there is more. FIX: keep the card inside the screen \
         rather than at a fixed width."
    );
    let controls = reading["controls"].as_array().cloned().unwrap_or_default();
    let off_screen: Vec<String> = controls
        .iter()
        .filter(|box_of| {
            box_of["right"].as_f64().unwrap_or(0.0) > viewport + SLACK_PX
                || box_of["left"].as_f64().unwrap_or(0.0) < -SLACK_PX
        })
        .map(|box_of| box_of.to_string())
        .collect();
    assert!(
        off_screen.is_empty(),
        "WHAT: {n} of the controls on {page} lie outside her {viewport:.0} px screen at {device}: \
         {off_screen:?}. WHY: a field or a submit she cannot see is a page she cannot finish. \
         FIX: keep every control inside the card and the card inside the screen.",
        n = off_screen.len()
    );
}

#[then(regex = r"^the page offers her no navigation rail and no palette control$")]
async fn then_no_rail_and_no_palette_control(world: &mut FoundryWorld) {
    let (page, _, reading) = card_reading(world);
    assert!(
        !reading["rail"].as_bool().unwrap_or(false)
            && !reading["themeControl"].as_bool().unwrap_or(false),
        "WHAT: {page} offers a navigation rail: {}, and a palette control: {}. \
         WHY: signed out she can go nowhere a rail would offer, and the public shortcut list is \
         served without reading a session at all — a rail there would change who the page serves. \
         FIX: the card frame includes no rail and no palette control.",
        reading["rail"],
        reading["themeControl"]
    );
}

// ----- the shortcut list as she opens it -------------------------------------

#[given(regex = r"^the operator opens the keyboard shortcut list from the nav$")]
async fn given_she_opens_the_list_from_the_nav(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    let browser = browser_harness::new_session().await;
    browser_harness::sign_in_through_browser(&browser, harness_of(world), MEI_EMAIL, MEI_PASSWORD)
        .await;
    world.browser = Some(browser);
    open_in_browser(world, "/").await;
    let browser = browser_of(world);
    let link = browser
        .find(fantoccini::Locator::Css("a[href='/keyboard-help']"))
        .await
        .unwrap_or_else(|err| {
            panic!("the nav offers her no way to the keyboard shortcut list: {err}")
        });
    link.click().await.unwrap_or_else(|err| {
        panic!("the way to the keyboard shortcut list could not be followed: {err}")
    });
    wait_until_on_path(browser_of(world), "/keyboard-help").await;
    let reading = read_with(browser_of(world), SHORTCUT_READING).await;
    remember(world, serde_json::json!({ "reading": reading }));
}

/// A bounded wait on the address the browser is now at — the navigation she
/// started has landed. Never a sleep on a guess, and never an unbounded loop.
async fn wait_until_on_path(browser: &fantoccini::Client, path: &str) {
    for _ in 0..50 {
        let here = browser
            .execute("return window.location.pathname;", Vec::new())
            .await
            .map(|v| v.as_str().unwrap_or_default().to_string())
            .unwrap_or_default();
        if here == path {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("the browser never arrived at {path}, so whatever is read next is some other page");
}

#[given(regex = r"^the operator opens the keyboard shortcut list on her phone$")]
async fn given_she_opens_the_list_on_her_phone(world: &mut FoundryWorld) {
    ensure_harness(world).await;
    let browser = browser_harness::open_mobile_session_sized(PHONE_WIDTH_PX, PHONE_HEIGHT_PX).await;
    world.browser = Some(browser);
    open_in_browser(world, "/keyboard-help").await;
    let reading = read_with(browser_of(world), SHORTCUT_READING).await;
    remember(world, serde_json::json!({ "reading": reading }));
}

fn shortcut_rows(world: &FoundryWorld) -> serde_json::Value {
    recalled(world)["reading"].clone()
}

#[then(regex = r"^each shortcut key is drawn on the same line as what it does$")]
async fn then_each_key_sits_beside_its_meaning(world: &mut FoundryWorld) {
    let reading = shortcut_rows(world);
    let rows = reading["rows"].as_array().cloned().unwrap_or_default();
    assert!(
        rows.len() >= 7,
        "WHAT: the page she opened draws {n} shortcut rows. WHY: with no row drawn there is \
         nothing for this reading to hold of, so a green here would mean nothing — and an \
         unstyled answer painted as a page draws the list she came for as a bare run of keys. \
         FIX: answer a page navigation with the framed page that carries the shipped rows. \
         Reading: {reading}",
        n = rows.len()
    );
    let mut broken: Vec<String> = Vec::new();
    for row in &rows {
        let key_box = &row["keyBox"];
        let meaning_box = &row["meaningBox"];
        let (key_top, key_bottom) = (
            key_box["top"].as_f64().unwrap_or(f64::NAN),
            key_box["bottom"].as_f64().unwrap_or(f64::NAN),
        );
        let (meaning_top, meaning_bottom, meaning_left) = (
            meaning_box["top"].as_f64().unwrap_or(f64::NAN),
            meaning_box["bottom"].as_f64().unwrap_or(f64::NAN),
            meaning_box["left"].as_f64().unwrap_or(f64::NAN),
        );
        let key_right = key_box["right"].as_f64().unwrap_or(f64::NAN);
        let drawn = key_bottom - key_top > 0.0 && meaning_bottom - meaning_top > 0.0;
        let same_line = drawn
            && (key_top + key_bottom) / 2.0 > meaning_top
            && (key_top + key_bottom) / 2.0 < meaning_bottom;
        let beside = meaning_left >= key_right - SLACK_PX;
        if !(same_line && beside) {
            broken.push(format!(
                "{} / {} (key {key_top:.0}-{key_bottom:.0} ending at {key_right:.0}, meaning \
                 {meaning_top:.0}-{meaning_bottom:.0} starting at {meaning_left:.0})",
                row["key"], row["meaning"]
            ));
        }
    }
    assert!(
        broken.is_empty(),
        "WHAT: {n} shortcut rows draw the key away from what it does: {broken:?}. \
         WHY: she reads a key and its meaning as one pair; a key on one line and its meaning on \
         the next makes her match them by counting. FIX: style the rows on the rows' own class so \
         the page gets the same pairing the overlay has, instead of only inside the overlay host.",
        n = broken.len()
    );
}

#[then(regex = r"^the instruction about choosing a card is drawn below the list$")]
async fn then_the_instruction_is_below_the_list(world: &mut FoundryWorld) {
    let reading = shortcut_rows(world);
    let instruction = reading["instruction"].as_str().unwrap_or_default();
    assert!(
        !instruction.trim().is_empty(),
        "WHAT: the page she opened carries no instruction about choosing a card at all. \
         WHY: the list advertises keys that do nothing until a card is chosen; a reader who is \
         never told that stands there pressing them. FIX: carry the shipped instruction on the \
         page as the overlay does. Reading: {reading}"
    );
    let rows = reading["rows"].as_array().cloned().unwrap_or_default();
    let last_row_bottom = rows
        .iter()
        .filter_map(|row| row["meaningBox"]["bottom"].as_f64())
        .fold(f64::MIN, f64::max);
    let instruction_top = number_at(&reading, &["instructionBox", "top"]);
    assert!(
        instruction_top >= last_row_bottom - SLACK_PX,
        "WHAT: the instruction is drawn at {instruction_top:.0} while the list ends at \
         {last_row_bottom:.0}, so it sits inside or above the list. WHY: an instruction in the \
         middle of the pairs reads as one of them. FIX: draw it below the rows, as the overlay \
         does."
    );
}

// ----- the controls beside the issue -----------------------------------------

#[given(regex = r"^the operator is looking at an issue's own page$")]
async fn given_she_is_looking_at_an_issue_page(world: &mut FoundryWorld) {
    let number = store_an_issue(world, ISSUE_TITLE, DESCRIPTION_TWO_LINES, "backlog").await;
    let browser = browser_harness::new_session().await;
    browser_harness::sign_in_through_browser(&browser, harness_of(world), MEI_EMAIL, MEI_PASSWORD)
        .await;
    world.browser = Some(browser);
    open_in_browser(world, &issue_address(number)).await;
    let reading = read_with(browser_of(world), CONTROL_READING).await;
    remember(
        world,
        serde_json::json!({ "number": number, "reading": reading }),
    );
}

#[then(regex = r"^Delete is drawn as one of Foundry's controls, not as a bare link$")]
async fn then_delete_is_drawn_as_a_control(world: &mut FoundryWorld) {
    let reading = recalled(world)["reading"].clone();
    let control = reading["delete"].clone();
    assert!(
        control.is_object(),
        "WHAT: the issue page draws no Delete control at all. WHY: with none drawn there is \
         nothing for this reading to hold of. FIX: keep the control on the page. Reading: {reading}"
    );
    let padding = number_at(&reading, &["delete", "padInline"]);
    let border = number_at(&reading, &["delete", "borderWidth"]);
    let fill = reading["delete"]["fill"].as_str().unwrap_or_default();
    let page_bg = reading["pageBg"].as_str().unwrap_or_default();
    assert!(
        padding >= 6.0 && (border > 0.0 || fill != page_bg),
        "WHAT: Delete is drawn with {padding:.0} px beside its word, a {border:.0} px edge and a \
         fill of {fill:?} over a page of {page_bg:?}. WHY: beside a styled page, a bare underlined \
         word reads as an unfinished page, and the one control that destroys her work is the one \
         that looks least deliberate. FIX: give it the quiet control presentation the rest of the \
         application already uses, and leave its address alone. Reading: {reading}"
    );
}

#[then(regex = r"^the control for attaching a file is drawn in Foundry's own type$")]
async fn then_the_file_control_is_in_the_page_type(world: &mut FoundryWorld) {
    let reading = recalled(world)["reading"].clone();
    assert!(
        reading["file"].is_object(),
        "WHAT: the issue page draws no control for attaching a file. WHY: with none drawn this \
         reading would hold over nothing. FIX: keep the upload control on the page. \
         Reading: {reading}"
    );
    let page_font = reading["pageFont"].as_str().unwrap_or_default();
    let control_font = reading["file"]["font"].as_str().unwrap_or_default();
    assert!(
        !page_font.trim().is_empty(),
        "WHAT: the page itself resolves no type at all, so comparing a control with it would mean \
         nothing. FIX: this is a defect of the reading, not of the page."
    );
    assert_eq!(
        control_font, page_font,
        "WHAT: the control for attaching a file is drawn in {control_font:?} while the page around \
         it is drawn in {page_font:?}. WHY: a browser's own raw file control is the one thing on \
         the page that does not belong to the application, and it is drawn beside her \
         attachments. FIX: give the upload label and its field the stylesheet's own control type."
    );
}
