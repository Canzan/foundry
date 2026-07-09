//! navigation-bar-linear-ui — step definitions for the shared Linear-style sidebar.
//!
//! These scenarios drive the real HTTP surface through the in-process axum harness
//! (`support::harness::InProcHarness`) with a real per-scenario Postgres
//! (testcontainers, `@real-io`), mirroring `feature_dashboard_enhancements` — the
//! near-identical "Linear-feel dashboard" feature whose Background + persona +
//! assertion glue this module deliberately shares.
//!
//! Reuses, per the globally-unique-phrase rule (cucumber-rs) — DO NOT redefine here:
//!   - `a workspace "Acme" exists with admin "Ada" and display name "…"`  (feature_dashboard_enhancements)
//!   - `a project "…" with key prefix "…" exists in "…"`                  (feature_dashboard_enhancements)
//!   - `(\w+) is signed in`                                               (us_07_project_create)
//!   - `(\w+) visits "/"`                                                 (feature_dashboard_enhancements)
//!   - `Ada is an instance super-admin`                                   (feature_dashboard_enhancements)
//!   - `a member "…" who is not an instance admin is signed in`           (feature_dashboard_enhancements)
//!   - `a member "…" whose display name is "…" is signed in`             (feature_dashboard_enhancements)
//!   - `the response body contains "…"`                                   (us_06_signin)
//!   - `the response body contains a link to "…"`                         (feature_dashboard_enhancements)
//!   - `the response body does not contain a link to "…"`                 (feature_dashboard_enhancements)
//!   - `the response body contains the escaped display name`              (feature_dashboard_enhancements)
//!   - `the response body does not contain a live "<b>" element`          (feature_dashboard_enhancements)
//!
//! The reused `(\w+) visits "/"` / persona Givens set + read `us_07_signed_in_email`
//! / `us_07_signed_in_password` and stash the response in `world.last_body` /
//! `last_status` / `last_headers`; every NEW assertion below reads `world.last_body`.
//! The NEW `opens the authenticated page "…"` / `opens the pre-auth page "…"` When
//! steps write those SAME slots, so the reused dashboard Then steps compose over them
//! unchanged.
//!
//! NEW step phrasings (nav-specific — sidebar presence/absence, per-item active
//! state, the exactly-one-current invariant, the footer user-menu contents, and the
//! Board deep-link target) are defined here. Assertions target the DESIGN-documented
//! class contract (`.sidebar`, `.sidebar__nav .sidebar__item`, `.sidebar__user`,
//! `nav[aria-label="Primary"]`, `aria-current="page"`) so they pin the rail's shape
//! for DELIVER.

use crate::support::harness::{establish_session, get_with_cookie, InProcHarness};
use crate::support::html_assertions as html;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use reqwest::redirect::Policy;

const TEST_NOW: &str = "2026-01-15T12:00:00Z";

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

/// Ensure the per-scenario harness + client exist. The Background's reused
/// `a workspace … exists …` Given already spawns them (and resets state), so this
/// is a defensive no-op in practice — it only spawns when a scenario's first step
/// is a NEW Given that precedes any reused seeding.
async fn ensure_harness(world: &mut FoundryWorld) {
    if world.harness.is_none() {
        world.harness = Some(InProcHarness::spawn(now_anchor()).await);
    }
    if world.http.is_none() {
        world.http = Some(client());
    }
}

// ---- primary-item + user-menu DOM helpers (DESIGN class contract) ----------

/// Owned view of each primary nav item: (trimmed label, aria-current, href).
/// Selects the DESIGN-documented `.sidebar__nav .sidebar__item` anchors.
fn primary_items(body: &str) -> Vec<(String, Option<String>, Option<String>)> {
    let doc = html::parse(body);
    html::select_all(&doc, ".sidebar__nav .sidebar__item")
        .into_iter()
        .map(|el| {
            let label = el.text().collect::<String>().trim().to_string();
            let aria = el.value().attr("aria-current").map(str::to_string);
            let href = el.value().attr("href").map(str::to_string);
            (label, aria, href)
        })
        .collect()
}

/// The primary item whose label contains `label` (icon+label items render the
/// label as their only text node).
fn item_labeled(body: &str, label: &str) -> Option<(String, Option<String>, Option<String>)> {
    primary_items(body)
        .into_iter()
        .find(|(text, _, _)| text.contains(label))
}

fn body_of(world: &FoundryWorld) -> String {
    world.last_body.clone().unwrap_or_default()
}

// ---- When: authenticated + pre-auth visits --------------------------------

/// Visit any AUTHENTICATED full-page surface as the signed-in persona. Mirrors the
/// reused `(\w+) visits "/"` (establish ONE session, GET the path) but for an
/// arbitrary path, so the presence / active-state scenarios can sweep the whole
/// authed page set. Writes `last_status` / `last_headers` / `last_body`.
#[when(regex = r#"^(\w+) opens the authenticated page "([^"]+)"$"#)]
async fn opens_authenticated_page(world: &mut FoundryWorld, _who: String, path: String) {
    ensure_harness(world).await;
    let email = world
        .us_07_signed_in_email
        .clone()
        .expect("a persona is signed in");
    let password = world
        .us_07_signed_in_password
        .clone()
        .expect("signed-in password recorded");
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    let session = establish_session(harness, http, &email, &password).await;
    let outcome = get_with_cookie(harness, http, &path, &session).await;
    world.dash_session_cookie = Some(session);
    world.last_status = Some(outcome.status);
    world.last_headers = Some(outcome.headers);
    world.last_body = Some(outcome.body);
}

/// A signed-OUT visitor requests a pre-auth / utility page (no session cookie). The
/// pre-auth surface must render chrome-free (no shell, no sidebar). Writes the same
/// response slots so the absence Then steps read `last_body`.
#[when(regex = r#"^a visitor opens the pre-auth page "([^"]+)"$"#)]
async fn visitor_opens_preauth_page(world: &mut FoundryWorld, path: String) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let http = world.http.as_ref().expect("http");
    // No cookie at all — a genuinely unauthenticated GET.
    let outcome = get_with_cookie(harness, http, &path, "").await;
    world.last_status = Some(outcome.status);
    world.last_headers = Some(outcome.headers);
    world.last_body = Some(outcome.body);
}

// ---- Given: nav-specific preconditions -------------------------------------

/// Marker precondition for the chrome-free scenarios: the following visit is made by
/// a signed-out visitor. The reused Background still seeds the workspace + signs Ada
/// in, but the pre-auth GET presents no session cookie, so this only documents intent
/// and guarantees the harness exists.
#[given(regex = r"^a visitor is not signed in$")]
async fn a_visitor_is_not_signed_in(world: &mut FoundryWorld) {
    ensure_harness(world).await;
}

/// Remove every project from the named workspace so `resolve_board_href` has no
/// first project to deep-link to (ADR-003 fallback → the Board item targets `/`).
/// Safe because the Background seeds no issues/changes that reference the project.
#[given(regex = r#"^the "([^"]+)" workspace has no projects$"#)]
async fn workspace_has_no_projects(world: &mut FoundryWorld, ws_name: String) {
    ensure_harness(world).await;
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    sqlx::query(
        "DELETE FROM projects WHERE workspace_id = (SELECT id FROM workspaces WHERE name = $1)",
    )
    .bind(&ws_name)
    .execute(pool)
    .await
    .expect("delete projects for workspace");
}

// ---- Then: sidebar presence / absence --------------------------------------

#[then(regex = r"^a persistent left sidebar is shown$")]
async fn sidebar_is_shown(world: &mut FoundryWorld) {
    html::assert_has(&body_of(world), ".sidebar");
}

#[then(regex = r"^no navigation sidebar is shown$")]
async fn no_sidebar_is_shown(world: &mut FoundryWorld) {
    html::assert_not_has(&body_of(world), ".sidebar");
}

#[then(regex = r"^only the page's own content is visible$")]
async fn only_page_content_visible(world: &mut FoundryWorld) {
    // The app shell (offset content wrapper) is the structural marker of chrome;
    // its absence proves the page renders on the bare `base.html` parent.
    html::assert_not_has(&body_of(world), ".app-shell");
}

// ---- Then: rail identity ----------------------------------------------------

#[then(regex = r#"^the sidebar shows the workspace name "([^"]+)"$"#)]
async fn sidebar_shows_workspace(world: &mut FoundryWorld, name: String) {
    let body = body_of(world);
    let doc = html::parse(&body);
    let rail = html::select_all(&doc, ".sidebar");
    let rail = rail.first().unwrap_or_else(|| {
        panic!("no .sidebar element to read the workspace name from; body:\n{body}")
    });
    let text = rail.text().collect::<String>();
    assert!(
        text.contains(&name),
        "the sidebar must show the workspace name {name:?}; sidebar text was:\n{text:?}"
    );
}

#[then(regex = r#"^the sidebar footer shows the signed-in name "([^"]+)"$"#)]
async fn sidebar_footer_shows_name(world: &mut FoundryWorld, name: String) {
    let body = body_of(world);
    let doc = html::parse(&body);
    let footer = html::select_all(&doc, ".sidebar__user");
    let footer = footer.first().unwrap_or_else(|| {
        panic!("no .sidebar__user footer to read the signed-in name from; body:\n{body}")
    });
    let text = footer.text().collect::<String>();
    assert!(
        text.contains(&name),
        "the sidebar footer must show the signed-in name {name:?}; footer text was:\n{text:?}"
    );
}

#[then(regex = r#"^the sidebar shows primary navigation items "([^"]+)" and "([^"]+)"$"#)]
async fn sidebar_shows_primary_items(world: &mut FoundryWorld, first: String, second: String) {
    let body = body_of(world);
    let labels: Vec<String> = primary_items(&body)
        .into_iter()
        .map(|(l, _, _)| l)
        .collect();
    for wanted in [&first, &second] {
        assert!(
            labels.iter().any(|l| l.contains(wanted.as_str())),
            "the sidebar must show a primary item {wanted:?}; found items {labels:?}"
        );
    }
}

// ---- Then: active state -----------------------------------------------------

#[then(regex = r#"^the "([^"]+)" navigation item is marked as the current page$"#)]
async fn item_is_current(world: &mut FoundryWorld, label: String) {
    let body = body_of(world);
    let item = item_labeled(&body, &label)
        .unwrap_or_else(|| panic!("no primary nav item labeled {label:?}; body:\n{body}"));
    assert_eq!(
        item.1.as_deref(),
        Some("page"),
        "the {label:?} item must carry aria-current=\"page\"; found {:?}",
        item.1
    );
}

#[then(regex = r#"^the "([^"]+)" navigation item is not marked as current$"#)]
async fn item_is_not_current(world: &mut FoundryWorld, label: String) {
    let body = body_of(world);
    let item = item_labeled(&body, &label)
        .unwrap_or_else(|| panic!("no primary nav item labeled {label:?}; body:\n{body}"));
    assert_ne!(
        item.1.as_deref(),
        Some("page"),
        "the {label:?} item must NOT be marked current, but it carries aria-current=\"page\""
    );
}

#[then(regex = r"^exactly one primary navigation item is marked as the current page$")]
async fn exactly_one_current(world: &mut FoundryWorld) {
    let body = body_of(world);
    let current: Vec<String> = primary_items(&body)
        .into_iter()
        .filter(|(_, aria, _)| aria.as_deref() == Some("page"))
        .map(|(label, _, _)| label)
        .collect();
    assert_eq!(
        current.len(),
        1,
        "exactly one primary nav item must be current (never zero, never two); \
         current items were {current:?}"
    );
}

#[then(regex = r"^the sidebar is exposed as a navigation landmark$")]
async fn sidebar_is_landmark(world: &mut FoundryWorld) {
    html::assert_has(&body_of(world), r#"nav[aria-label="Primary"]"#);
}

#[then(regex = r"^the current navigation item carries an aria-current marker$")]
async fn current_item_carries_aria(world: &mut FoundryWorld) {
    let body = body_of(world);
    let doc = html::parse(&body);
    let marked = html::select_all(&doc, r#".sidebar__nav [aria-current="page"]"#);
    assert!(
        !marked.is_empty(),
        "the current primary nav item must carry aria-current=\"page\"; body:\n{body}"
    );
}

// ---- Then: Board deep-link target ------------------------------------------

#[then(regex = r#"^the sidebar links "([^"]+)" to "([^"]+)"$"#)]
async fn sidebar_links_label_to(world: &mut FoundryWorld, label: String, href: String) {
    let body = body_of(world);
    let item = item_labeled(&body, &label)
        .unwrap_or_else(|| panic!("no primary nav item labeled {label:?}; body:\n{body}"));
    assert_eq!(
        item.2.as_deref(),
        Some(href.as_str()),
        "the {label:?} item must link to {href:?}; found href {:?}",
        item.2
    );
}

// ---- Then: footer user menu ------------------------------------------------

#[then(regex = r#"^the user menu contains a link to "([^"]+)"$"#)]
async fn user_menu_contains_link(world: &mut FoundryWorld, path: String) {
    let selector = format!(r#".sidebar__user a[href="{path}"]"#);
    html::assert_has(&body_of(world), &selector);
}

#[then(regex = r#"^the user menu does not contain a link to "([^"]+)"$"#)]
async fn user_menu_absent_link(world: &mut FoundryWorld, path: String) {
    let selector = format!(r#".sidebar__user a[href="{path}"]"#);
    html::assert_not_has(&body_of(world), &selector);
}

#[then(
    regex = r#"^the user menu contains a sign-out control posting to "([^"]+)" with a CSRF token$"#
)]
async fn user_menu_signout_csrf(world: &mut FoundryWorld, action: String) {
    let body = body_of(world);
    // A native POST form (the no-JS control) targeting the reused endpoint …
    html::assert_has(
        &body,
        &format!(r#".sidebar__user form[method="post"][action="{action}"]"#),
    );
    // … carrying the hidden double-submit token (BR-3 / reused `_csrf`).
    html::assert_has(
        &body,
        &format!(r#".sidebar__user form[action="{action}"] input[name="_csrf"]"#),
    );
}

/// D1 remediation (adversarial review 04-03): the footer sign-out form must carry a
/// NON-EMPTY double-submit `_csrf` token whose value MATCHES the `foundry_csrf`
/// cookie set on the SAME response — otherwise `POST /sign-out` is refused by
/// `csrf_middleware` and sign-out silently fails. The shipped bug hardcoded an EMPTY
/// token on every page built via `NavContext::home_for` / `board_for` (i.e. every
/// authed page EXCEPT the dashboard), so the hidden input rendered `value=""` even
/// on pages that set a `foundry_csrf` cookie for their own forms. This sweep pins the
/// token both non-empty AND cookie-matched, so an empty (or mismatched) token reds.
#[then(
    regex = r"^the sidebar sign-out form carries a non-empty CSRF token matching the response CSRF cookie$"
)]
async fn signout_csrf_matches_cookie(world: &mut FoundryWorld) {
    let body = body_of(world);
    let tokens = html::collect_attributes(
        &body,
        r#".sidebar__user form[action="/sign-out"] input[name="_csrf"]"#,
        "value",
    );
    let token = tokens
        .first()
        .unwrap_or_else(|| panic!("no sign-out _csrf input in the sidebar footer; body:\n{body}"));
    assert!(
        !token.is_empty(),
        "the sign-out form's _csrf token must be NON-EMPTY (an empty token is CSRF-rejected at \
         POST /sign-out, so sign-out silently fails); found an empty token. body:\n{body}"
    );
    let headers = world
        .last_headers
        .as_ref()
        .expect("a response with headers was captured");
    let cookie_token = headers
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find_map(|s| s.strip_prefix("foundry_csrf="))
        .and_then(|rest| rest.split(';').next())
        .unwrap_or_else(|| {
            panic!("the response set no foundry_csrf cookie to match the sign-out token against")
        });
    assert_eq!(
        token.as_str(),
        cookie_token,
        "the sign-out _csrf token must EQUAL the response's foundry_csrf cookie (double-submit); \
         token={token:?} cookie={cookie_token:?}"
    );
}

// ---- Then: scoping guard (US-05) -------------------------------------------

#[then(regex = r#"^the sidebar does not contain a "([^"]+)" item$"#)]
async fn sidebar_absent_item(world: &mut FoundryWorld, label: String) {
    let body = body_of(world);
    let doc = html::parse(&body);
    for el in html::select_all(&doc, ".sidebar a") {
        let text = el.text().collect::<String>();
        assert!(
            !text.contains(&label),
            "the sidebar must NOT promote a {label:?} item, but found one: {text:?}"
        );
    }
}
