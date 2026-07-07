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
use secrecy::ExposeSecret;
use std::collections::HashMap;
use time::format_description::well_known::Rfc3339;

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

/// By-only variant (slice 02, S10): a rank change carries no human-meaningful
/// old/new labels in the scenario, so we assert exactly one event for the field
/// with the right actor (a cross-status drop also records `status`, so filter by
/// field rather than asserting the total count).
#[then(regex = r#"^a change event is recorded for "([^"]+)": field "([^"]+)", by "([^"]+)"$"#)]
async fn change_recorded_by(world: &mut FoundryWorld, key: String, field: String, actor: String) {
    let events = change_events_for(world, &key).await;
    let matching: Vec<&ChangeEvent> = events.iter().filter(|e| e.field == field).collect();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one {field:?} change event for {key}, found {}",
        matching.len()
    );
    assert_eq!(
        matching[0].actor, actor,
        "recorded actor mismatch for the {field:?} change of {key}"
    );
}

/// Slice 02 (S9): at least one change event for the named field (a multi-field
/// save writes one row PER changed field).
#[then(regex = r#"^"([^"]+)" has a change event for field "([^"]+)"$"#)]
async fn has_change_for_field(world: &mut FoundryWorld, key: String, field: String) {
    let events = change_events_for(world, &key).await;
    assert!(
        events.iter().any(|e| e.field == field),
        "expected a {field:?} change event for {key}, found fields {:?}",
        events.iter().map(|e| &e.field).collect::<Vec<_>>()
    );
}

/// Slice 02 (S9): an UNCHANGED field records nothing — no row for it.
#[then(regex = r#"^"([^"]+)" has no change event for field "([^"]+)"$"#)]
async fn has_no_change_for_field(world: &mut FoundryWorld, key: String, field: String) {
    let events = change_events_for(world, &key).await;
    assert!(
        !events.iter().any(|e| e.field == field),
        "an unchanged {field:?} must record NOTHING for {key}, found {:?}",
        events.iter().map(|e| &e.field).collect::<Vec<_>>()
    );
}

// ----- When: edit title/description via the edit-dialog POST (slice 02) -------

/// S8: change ONLY the title. Re-post the seeded (empty) description and no
/// status so the save's sole delta is the title (mirrors issue-status-move's
/// `capture_status_post`, which re-posts the seeded title to isolate its delta).
#[when(regex = r#"^Mei edits "([^"]+)" title to "([^"]+)"$"#)]
async fn edit_title(world: &mut FoundryWorld, key: String, title: String) {
    capture_edit_post(world, &key, &title, "").await;
}

/// S9: change title AND description in one save → one row per changed field.
#[when(regex = r#"^Mei edits "([^"]+)" title to "([^"]+)" and description to "([^"]+)"$"#)]
async fn edit_title_and_description(
    world: &mut FoundryWorld,
    key: String,
    title: String,
    description: String,
) {
    capture_edit_post(world, &key, &title, &description).await;
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

// ----- Slice 03: the program JSON change feed (/api/v1/.../history) ----------

/// S11/S12: a program (machine credential) GETs the issue's change history feed.
/// Mirrors the us-w05a/us-w05c bearer path: mint a REAL EdDSA machine credential
/// bound to Mei (registered in the denylist), present it as `Authorization:
/// Bearer` against `/api/v1/teams/backend/projects/sandbox/issues/{n}/history`.
/// A foreign/absent key (ZZZ-9) resolves to number 9 inside Sandbox — absent, so
/// the API refuses non-enumerably (never leaking the other tenant's issue).
#[when(regex = r#"^a program requests the change history of "([^"]+)"$"#)]
async fn program_requests_history(world: &mut FoundryWorld, key: String) {
    ensure_http(world).await;
    let bearer = mint_bearer_for_mei(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let url = format!(
        "{base}/api/v1/teams/{TEAM_SLUG}/projects/{PROJECT_SLUG}/issues/{number}/history",
        base = harness.base_url(),
        number = number_of(&key)
    );
    let resp = http
        .get(url)
        .header(reqwest::header::ACCEPT, "application/json")
        .header(reqwest::header::AUTHORIZATION, format!("Bearer {bearer}"))
        .send()
        .await
        .expect("get history feed");
    world.last_status = Some(resp.status());
    world.last_headers = Some(resp.headers().clone());
    world.last_body = Some(resp.text().await.unwrap_or_default());
}

/// One event parsed from the history JSON feed. `old` is present-but-null where
/// the event has no old value.
#[derive(Debug, serde::Deserialize)]
struct HistoryEvent {
    actor: String,
    field: String,
    old: Option<String>,
    new: String,
    at: String,
}

fn parse_history(world: &FoundryWorld) -> Vec<HistoryEvent> {
    let body = world.last_body.clone().unwrap_or_default();
    serde_json::from_str::<Vec<HistoryEvent>>(&body).unwrap_or_else(|e| {
        panic!(
            "history feed must be a JSON array of events but parse failed ({e}); status {:?}, body {body:?}",
            world.last_status
        )
    })
}

#[then(
    regex = r#"^the history JSON lists the events oldest-first, each with actor, field, old, new, and a timestamp$"#
)]
async fn history_json_shape(world: &mut FoundryWorld) {
    assert_eq!(
        world.last_status.map(|s| s.as_u16()),
        Some(200),
        "the history feed must answer 200; body {:?}",
        world.last_body
    );
    // The five keys must all be PRESENT on every object (including a null `old`)
    // — parse as raw values so an absent key is caught, not defaulted away.
    let body = world.last_body.clone().unwrap_or_default();
    let raw: Vec<serde_json::Value> = serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("history not a JSON array ({e}): {body}"));
    for object in &raw {
        for slot in ["actor", "field", "old", "new", "at"] {
            assert!(
                object.get(slot).is_some(),
                "every history event must carry the {slot:?} key: {object}"
            );
        }
    }

    let events = parse_history(world);
    assert!(
        events.len() >= 2,
        "expected at least 2 change events in the feed, got {}",
        events.len()
    );
    for event in &events {
        assert!(
            !event.actor.is_empty(),
            "event actor must be non-empty: {event:?}"
        );
        assert!(
            !event.field.is_empty(),
            "event field must be non-empty: {event:?}"
        );
        assert!(
            !event.new.is_empty(),
            "event new value must be non-empty: {event:?}"
        );
        assert!(
            time::OffsetDateTime::parse(&event.at, &Rfc3339).is_ok(),
            "event timestamp must be ISO-8601 UTC (RFC3339): {event:?}"
        );
    }
    // Oldest-first: timestamps are monotonically non-decreasing.
    let timestamps: Vec<time::OffsetDateTime> = events
        .iter()
        .map(|e| time::OffsetDateTime::parse(&e.at, &Rfc3339).expect("timestamp parsed above"))
        .collect();
    assert!(
        timestamps.windows(2).all(|w| w[0] <= w[1]),
        "the feed must be ordered oldest-first (ascending timestamps): {timestamps:?}"
    );
}

#[then(regex = r#"^the JSON events are the same as the stored change events for "([^"]+)"$"#)]
async fn history_json_matches_store(world: &mut FoundryWorld, key: String) {
    let events = parse_history(world);
    // `change_events_for` reads the append-only table directly, OLDEST-first —
    // the SAME order the feed serializes. Compare the field/old/new sequence to
    // prove the JSON is the stored events (one source of truth, AC-03.4).
    let stored = change_events_for(world, &key).await;
    assert_eq!(
        events.len(),
        stored.len(),
        "the feed must carry exactly the stored events for {key}: feed {}, store {}",
        events.len(),
        stored.len()
    );
    for (json, row) in events.iter().zip(stored.iter()) {
        assert_eq!(json.field, row.field, "field mismatch for {key}");
        assert_eq!(
            json.old.as_deref(),
            row.old_value.as_deref(),
            "old value mismatch for {key}"
        );
        assert_eq!(json.new, row.new_value, "new value mismatch for {key}");
    }
}

#[then(regex = r#"^the API response is a uniform non-enumerable refusal$"#)]
async fn api_non_enumerable_refusal(world: &mut FoundryWorld) {
    assert_eq!(
        world.last_status.map(|s| s.as_u16()),
        Some(404),
        "a foreign/absent issue must be refused 404 (never a 500); body {:?}",
        world.last_body
    );
    let body = world.last_body.clone().unwrap_or_default();
    // The uniform JSON envelope with the stable `not_found` code, and NO echo of
    // any issue key (no enumeration oracle).
    let parsed: serde_json::Value = serde_json::from_str(&body)
        .unwrap_or_else(|e| panic!("404 body must be the JSON error envelope ({e}): {body}"));
    assert_eq!(
        parsed
            .get("error")
            .and_then(|error| error.get("code"))
            .and_then(|code| code.as_str()),
        Some("not_found"),
        "the refusal envelope code must be \"not_found\": {body}"
    );
    assert!(
        !body.contains("GEN-") && !body.contains("ZZZ-"),
        "the refusal must not echo any issue key (no enumeration oracle): {body}"
    );
}

/// Mint a REAL EdDSA machine credential bound to Mei and register it in the
/// denylist, returning the compact JWT to present as the bearer. Mirrors the
/// us-w05a/w05c minting (`feature_a_programmatic`): a PRECONDITION (a real
/// admin-issued credential), not the behaviour under test. `exp` is against the
/// real wall clock (jsonwebtoken validates `exp` against `SystemTime::now`).
async fn mint_bearer_for_mei(world: &mut FoundryWorld) -> String {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let (user_id, workspace_id): (uuid::Uuid, uuid::Uuid) = sqlx::query_as(
        "SELECT u.id, wm.workspace_id
           FROM users u
           JOIN workspace_memberships wm ON wm.user_id = u.id
          WHERE u.email_lower = $1
          LIMIT 1",
    )
    .bind(MEI_EMAIL)
    .fetch_one(pool)
    .await
    .unwrap_or_else(|e| panic!("resolve Mei for the machine credential: {e}"));

    let jti = uuid::Uuid::now_v7();
    let now = time::OffsetDateTime::now_utc();
    let exp = now + time::Duration::seconds(3600);
    harness
        .app
        .state
        .store
        .insert_machine_token(
            jti,
            user_id,
            workspace_id,
            None,
            exp,
            "history reader",
            user_id,
        )
        .await
        .expect("register machine token");

    let claims = foundry_auth::MachineTokenClaims {
        sub: user_id,
        scope: None,
        iat: now.unix_timestamp(),
        exp: exp.unix_timestamp(),
        jti,
        iss: foundry_auth::MACHINE_TOKEN_ISS.to_string(),
        aud: foundry_auth::MACHINE_TOKEN_AUD.to_string(),
    };
    foundry_auth::test_keys::signer()
        .mint(&claims)
        .expect("mint machine jwt")
        .expose_secret()
        .to_string()
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

/// POST the issue-edit form varying title + description (no `state`, so the save
/// is a pure title/description edit — the drop path drives status/rank). Mirrors
/// `capture_status_post` in issue-status-move (CSRF in cookie + `_csrf` field).
async fn capture_edit_post(world: &mut FoundryWorld, key: &str, title: &str, description: &str) {
    ensure_http(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let (session_pair, csrf) = sign_in(harness, http).await;
    let base = harness.base_url();
    let combined = format!("{session_pair}; foundry_csrf={csrf}");
    let url = format!(
        "/team/{TEAM_SLUG}/project/{PROJECT_SLUG}/issues/{number}/edit",
        number = number_of(key)
    );
    let mut form: HashMap<&str, String> = HashMap::new();
    form.insert("title", title.to_string());
    form.insert("description", description.to_string());
    form.insert("_csrf", csrf);
    let resp = http
        .post(format!("{base}{url}"))
        .header(reqwest::header::COOKIE, combined)
        .header("HX-Request", "true")
        .form(&form)
        .send()
        .await
        .expect("post edit url");
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
