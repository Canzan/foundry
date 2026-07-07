//! issue-change-history step definitions — slice 01: status-change history +
//! the human timeline on the issue-detail page.
//!
//! HARNESS BOUNDARY (distill/acceptance-review.md): HTTP and store level
//! (reqwest, scraper, real Postgres). Slice 01 pins the in-tx record contract
//! (reads of the new `issue_change_events` table), the detail-page timeline
//! render (newest-first, attributed, plain-language), the empty-timeline
//! genesis (UC-1), append-only accumulation, the no-op no-record rule,
//! non-enumerable refusal of a foreign issue, and the board card's detail-link
//! alongside its preserved quick-edit control.
//!
//! REUSES the globally-registered givens/whens (do NOT redefine): the Background
//! `a workspace "Acme" … member "Mei" … team "Backend"` (feature_board_new_issue),
//! `a project "…" (key "…") with issues:` (feature_card_ranking — seeds titles as
//! `"Seeded issue"`), `Mei is signed in` (us_07), `Mei saves the edit dialog for
//! "…" with status "…"` (feature_issue_status_move — drives the state change),
//! `Mei fetches the "…" board` (feature_board_new_issue), `a foreign issue "…"
//! exists in another workspace` + `the response is a non-enumerable refusal`
//! (feature_card_ranking). Only the change-history-specific phrases are new here.

use crate::support::harness::InProcHarness;
use crate::world::FoundryWorld;
use cucumber::{then, when};
use reqwest::redirect::Policy;
use scraper::{Html, Selector};
use std::collections::HashMap;

const MEMBER_PASSWORD: &str = "mei-correct-horse-battery-staple";
const MEI_EMAIL: &str = "mei@acme.com";
const TEAM_SLUG: &str = "backend";
const PROJECT_SLUG: &str = "sandbox";

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(Policy::none())
        .cookie_store(false)
        .build()
        .expect("build reqwest client")
}

async fn ensure_http(world: &mut FoundryWorld) {
    if world.http.is_none() {
        world.http = Some(client());
    }
}

fn number_of(key: &str) -> i32 {
    key.rsplit_once('-')
        .and_then(|(_, n)| n.parse().ok())
        .unwrap_or_else(|| panic!("issue key {key:?} must end in -N"))
}

fn prefix_of(key: &str) -> &str {
    key.rsplit_once('-')
        .map(|(p, _)| p)
        .unwrap_or_else(|| panic!("issue key {key:?} must contain a -"))
}

/// A single recorded change, read back through the append-only table joined to
/// the issue (by `key_prefix + number`) and the acting user (`display_name`).
struct ChangeEvent {
    field: String,
    old_value: Option<String>,
    new_value: String,
    actor: String,
}

/// All change events for an issue key, ordered OLDEST-first (`created_at ASC`,
/// `id ASC` tiebreak — `id` is a time-ordered uuid v7). The append-only reads
/// (count / earliest / recorded-for) all derive from this one query.
async fn change_events_for(world: &FoundryWorld, key: &str) -> Vec<ChangeEvent> {
    let harness = world.harness.as_ref().expect("harness");
    let rows: Vec<(String, Option<String>, String, String)> = sqlx::query_as(
        "SELECT e.field, e.old_value, e.new_value, u.display_name
           FROM issue_change_events e
           JOIN issues   i ON i.id = e.issue_id
           JOIN projects p ON p.id = i.project_id
           JOIN users    u ON u.id = e.actor_id
          WHERE p.key_prefix = $1 AND i.number = $2
          ORDER BY e.created_at ASC, e.id ASC",
    )
    .bind(prefix_of(key))
    .bind(number_of(key))
    .fetch_all(harness.app.state.store.pool())
    .await
    .unwrap_or_else(|e| panic!("read change events for {key} from store: {e}"));
    rows.into_iter()
        .map(|(field, old_value, new_value, actor)| ChangeEvent {
            field,
            old_value,
            new_value,
            actor,
        })
        .collect()
}

// ----- Then: the in-tx record contract (store reads) -------------------------

#[then(
    regex = r#"^a change event is recorded for "([^"]+)": field "([^"]+)", old "([^"]+)", new "([^"]+)", by "([^"]+)"$"#
)]
async fn change_recorded(
    world: &mut FoundryWorld,
    key: String,
    field: String,
    old: String,
    new: String,
    actor: String,
) {
    let events = change_events_for(world, &key).await;
    assert_eq!(
        events.len(),
        1,
        "expected exactly one change event for {key}, found {}",
        events.len()
    );
    let event = &events[0];
    assert_eq!(event.field, field, "recorded field mismatch for {key}");
    assert_eq!(
        event.old_value.as_deref(),
        Some(old.as_str()),
        "recorded old value mismatch for {key}"
    );
    assert_eq!(
        event.new_value, new,
        "recorded new value mismatch for {key}"
    );
    assert_eq!(event.actor, actor, "recorded actor mismatch for {key}");
}

#[then(regex = r#"^"([^"]+)" has (\d+) change events in the store$"#)]
async fn has_n_change_events(world: &mut FoundryWorld, key: String, expected: usize) {
    let events = change_events_for(world, &key).await;
    assert_eq!(
        events.len(),
        expected,
        "expected {expected} change events for {key}, found {}",
        events.len()
    );
}

#[then(
    regex = r#"^the earliest "([^"]+)" change event still reads field "([^"]+)", old "([^"]+)", new "([^"]+)"$"#
)]
async fn earliest_change_unchanged(
    world: &mut FoundryWorld,
    key: String,
    field: String,
    old: String,
    new: String,
) {
    let events = change_events_for(world, &key).await;
    let earliest = events
        .first()
        .unwrap_or_else(|| panic!("{key} has no change events; expected an earliest one"));
    assert_eq!(earliest.field, field, "earliest field mismatch for {key}");
    assert_eq!(
        earliest.old_value.as_deref(),
        Some(old.as_str()),
        "earliest old value mismatch for {key} — append-only must never mutate it"
    );
    assert_eq!(
        earliest.new_value, new,
        "earliest new value mismatch for {key} — append-only must never mutate it"
    );
}

#[then(regex = r#"^no change event is recorded for "([^"]+)"$"#)]
async fn no_change_recorded(world: &mut FoundryWorld, key: String) {
    let events = change_events_for(world, &key).await;
    assert!(
        events.is_empty(),
        "a no-op save must record NOTHING for {key}, found {} event(s)",
        events.len()
    );
}

// ----- When: open the issue-detail page --------------------------------------

#[when(regex = r#"^Mei opens the detail page for "([^"]+)"$"#)]
async fn open_detail_page(world: &mut FoundryWorld, key: String) {
    let url = format!(
        "/team/{TEAM_SLUG}/project/{PROJECT_SLUG}/issues/{number}",
        number = number_of(&key)
    );
    capture_get(world, &url).await;
}

// ----- Then: the human timeline on the detail page ---------------------------

/// Extract the timeline entries (newest-first as rendered) from the last detail-
/// page body: `(field, new_value_slug, plain_text)` per `.change-event`.
fn timeline_entries(body: &str) -> Vec<(String, String, String)> {
    let doc = Html::parse_document(body);
    let container = Selector::parse("[data-change-timeline]").expect("valid selector");
    assert!(
        doc.select(&container).next().is_some(),
        "the detail page must render a change-timeline container: {body}"
    );
    let entry = Selector::parse("[data-change-timeline] .change-event").expect("valid selector");
    doc.select(&entry)
        .map(|e| {
            let field = e.value().attr("data-change-field").unwrap_or_default();
            let new_value = e.value().attr("data-new-value").unwrap_or_default();
            let text: String = e.text().collect();
            (field.to_string(), new_value.to_string(), text)
        })
        .collect()
}

#[then(regex = r#"^the "([^"]+)" timeline shows a "([^"]+)" change to "([^"]+)" by "([^"]+)"$"#)]
async fn timeline_shows_change(
    world: &mut FoundryWorld,
    _key: String,
    field: String,
    to_label: String,
    actor: String,
) {
    let body = world.last_body.clone().unwrap_or_default();
    let entries = timeline_entries(&body);
    assert!(
        entries.iter().any(|(f, _new, text)| {
            f == &field && text.contains(&to_label) && text.contains(&actor)
        }),
        "the timeline must show a {field:?} change to {to_label:?} by {actor:?}; entries were {entries:?}: {body}"
    );
}

#[then(regex = r#"^the "([^"]+)" timeline lists the "([^"]+)" change above the "([^"]+)" change$"#)]
async fn timeline_order(world: &mut FoundryWorld, _key: String, upper: String, lower: String) {
    let body = world.last_body.clone().unwrap_or_default();
    let entries = timeline_entries(&body);
    let upper_idx = entries
        .iter()
        .position(|(_, new, _)| new == &upper)
        .unwrap_or_else(|| panic!("timeline missing a {upper:?} change: {entries:?}"));
    let lower_idx = entries
        .iter()
        .position(|(_, new, _)| new == &lower)
        .unwrap_or_else(|| panic!("timeline missing a {lower:?} change: {entries:?}"));
    assert!(
        upper_idx < lower_idx,
        "the {upper:?} change (index {upper_idx}) must be listed ABOVE the {lower:?} change (index {lower_idx}) — newest-first: {entries:?}"
    );
}

#[then(regex = r#"^the "([^"]+)" timeline is empty$"#)]
async fn timeline_empty(world: &mut FoundryWorld, _key: String) {
    let body = world.last_body.clone().unwrap_or_default();
    let entries = timeline_entries(&body);
    assert!(
        entries.is_empty(),
        "an unchanged issue's timeline must be EMPTY (no created event, UC-1), found {}: {body}",
        entries.len()
    );
}

// ----- Then: the board card carries BOTH controls (S7 regression) ------------

#[then(regex = r#"^each issue card still carries its edit-dialog control$"#)]
async fn cards_keep_edit_control(world: &mut FoundryWorld) {
    let body = world.last_body.clone().unwrap_or_default();
    let doc = Html::parse_document(&body);
    let card_selector = Selector::parse("article.issue-card").expect("valid selector");
    let cards: Vec<_> = doc.select(&card_selector).collect();
    assert!(
        !cards.is_empty(),
        "the board must render at least one issue card: {body}"
    );
    for card in cards {
        let hx_get = card.value().attr("hx-get").unwrap_or_default();
        assert!(
            hx_get.ends_with("/edit"),
            "every card must keep its quick-edit hx-get ending in /edit (R6), got {hx_get:?}: {body}"
        );
    }
}

#[then(regex = r#"^each issue card links to its detail page$"#)]
async fn cards_link_to_detail(world: &mut FoundryWorld) {
    let body = world.last_body.clone().unwrap_or_default();
    let doc = Html::parse_document(&body);
    let card_selector = Selector::parse("article.issue-card").expect("valid selector");
    let cards: Vec<_> = doc.select(&card_selector).collect();
    assert!(
        !cards.is_empty(),
        "the board must render at least one issue card: {body}"
    );
    let link_selector = Selector::parse("a[href]").expect("valid selector");
    for card in cards {
        let key = card.value().attr("data-issue-key").unwrap_or_default();
        let number = number_of(key);
        let want_suffix = format!("/issues/{number}");
        let links_to_detail = card.select(&link_selector).any(|a| {
            a.value()
                .attr("href")
                .map(|href| href.ends_with(&want_suffix))
                .unwrap_or(false)
        });
        assert!(
            links_to_detail,
            "each card must carry an <a href> to its detail page ending in {want_suffix:?} (R6): {body}"
        );
    }
}

// ----- internals: authenticated GET ------------------------------------------

async fn capture_get(world: &mut FoundryWorld, url: &str) {
    ensure_http(world).await;
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
