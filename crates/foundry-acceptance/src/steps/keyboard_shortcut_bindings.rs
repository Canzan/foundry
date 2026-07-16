//! keyboard-shortcut-bindings — the `@needs-browser` lane's step definitions.
//!
//! SLICE 01 / STEP 01-01 has landed the INSTRUMENT (ADR-007): the lane-probe
//! scenario is unskipped and drives a REAL headless Chrome, via chromedriver,
//! against the REAL app served by `InProcHarness::base_url()`. Every other
//! scenario in `tests/features/keyboard-shortcut-bindings.feature` still carries
//! `@pending` (excluded from EVERY lane) and its steps still panic with the
//! `__SCAFFOLD__` marker below — assertion-class RED (correct test / missing
//! production), never IMPORT_ERROR / BROKEN.
//!
//! WHY A BROWSER AND NOT reqwest+scraper: every AC in this feature asserts
//! key-pressed -> user-visible outcome (NFR-1). The shipped port-to-port suite
//! (`us_12_keyboard_nav.rs`) proves the SERVER contracts and is GREEN today while
//! the client keyboard layer is 100% ABSENT — it never presses a key. That gap IS
//! the feature (ODD-9). A scenario a port-to-port test could satisfy on `main` is,
//! by construction, not a scenario for this feature.
//!
//! ============================ STILL TO WIRE (slices 02-05) ====================
//!   - Remove `@pending` from a slice's scenarios; watch them RED for the right
//!     reason; implement `static/js/keyboard.js` to GREEN.
//!   - The IME step is SIMULATED via `client.execute()` dispatching a
//!     CompositionEvent + KeyboardEvent{isComposing:true,keyCode:229} (WebDriver
//!     send_keys cannot compose). The copy-chord step asserts NON-ACTIVATION +
//!     defaultPrevented===false for BOTH Ctrl and Meta (clipboard is unassertable
//!     headless). The `@grep-litmus` step is a SOURCE-TREE grep over `crates/`
//!     (NOT the browser) — hence it carries no `@needs-browser` tag.
//!   - Slice 05 also executes the ADR-008 13-site `#kb-items` retirement (mind
//!     TRAP B at projects.rs:1110) and retires the us-12 `@manual` drill (ADR-007 §5).

use crate::support::browser_harness;
use crate::support::harness::InProcHarness;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use fantoccini::Locator;
use secrecy::SecretString;

const TEST_NOW: &str = "2026-01-15T12:00:00Z";
const MEI_EMAIL: &str = "mei@acme.com";
const MEI_PASSWORD: &str = "mei-correct-horse-battery-staple";
const TEAM_SLUG: &str = "backend";
const PROJECT_SLUG: &str = "auth";

/// The one place the scaffold's pending-panic message lives. Assertion-class
/// (a panic), so the Red-Gate reads an unskipped step as RED, not BROKEN.
fn scaffold_pending() -> ! {
    panic!(
        "__SCAFFOLD__ keyboard-shortcut-bindings: pending DELIVER. This step's slice is not \
         implemented yet — the @needs-browser lane (fantoccini + chromedriver, ADR-007) exists as \
         of step 01-01, but static/js/keyboard.js does not yet bind this behaviour. \
         See docs/feature/keyboard-shortcut-bindings/distill/test-scenarios.md for the wiring."
    );
}

fn now_anchor() -> time::OffsetDateTime {
    time::OffsetDateTime::parse(TEST_NOW, &time::format_description::well_known::Rfc3339)
        .expect("parse anchor")
}

fn board_url(world: &FoundryWorld) -> String {
    let base = world.harness.as_ref().expect("harness").base_url();
    format!("{base}/team/{TEAM_SLUG}/project/{PROJECT_SLUG}")
}

// --- Background / navigation ------------------------------------------------

/// Seeds Mei + the acme workspace + the Backend team, opens ONE browser session
/// for this scenario, and signs in THROUGH THE REAL FORM.
///
/// The sign-in is the Earned-Trust probe (ADR-007 §4): `harness.rs:401-406` emits
/// a `Secure` cookie over plain HTTP and its own comment concedes the port-to-port
/// test never checks whether a browser would send it back. This drives the actual
/// form, so the browser must accept it and present it on the next navigation.
#[given(
    regex = r#"^Mei is signed into a real browser on the "([^"]+)" workspace, a member of the "([^"]+)" team$"#
)]
async fn mei_signed_in_browser(world: &mut FoundryWorld, workspace: String, team: String) {
    let harness = InProcHarness::spawn(now_anchor()).await;
    let pool = harness.app.state.store.pool();
    let workspace_id = uuid::Uuid::now_v7();
    let user_id = uuid::Uuid::now_v7();
    let team_id = uuid::Uuid::now_v7();
    let hash = foundry_auth::hash_password(&SecretString::new(MEI_PASSWORD.to_string().into()))
        .await
        .expect("hash Mei's password");
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
    .bind("Mei Tanaka")
    .bind(&hash)
    .execute(pool)
    .await
    .expect("insert Mei");
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
        .bind(TEAM_SLUG)
        .execute(pool)
        .await
        .expect("insert team");
    sqlx::query("INSERT INTO team_memberships (team_id, user_id, role) VALUES ($1, $2, 'member')")
        .bind(team_id)
        .bind(user_id)
        .execute(pool)
        .await
        .expect("insert team membership");

    let browser = browser_harness::new_session().await;
    browser_harness::sign_in_through_browser(&browser, &harness, MEI_EMAIL, MEI_PASSWORD).await;
    world.harness = Some(harness);
    world.browser = Some(browser);
}

#[given(regex = r#"^the "([^"]+)" project exists with issues AUTH-1, AUTH-2, AUTH-3 and AUTH-4$"#)]
async fn project_exists_with_issues(world: &mut FoundryWorld, project: String) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let workspace: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM workspaces LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("fetch workspace");
    let team: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM teams WHERE workspace_id = $1")
        .bind(workspace.0)
        .fetch_one(pool)
        .await
        .expect("fetch team");
    let author: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("fetch author");
    let project_id = uuid::Uuid::now_v7();
    sqlx::query(
        "INSERT INTO projects (id, team_id, workspace_id, name, slug, key_prefix)
              VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(project_id)
    .bind(team.0)
    .bind(workspace.0)
    .bind(&project)
    .bind(PROJECT_SLUG)
    .bind("AUTH")
    .execute(pool)
    .await
    .expect("insert AUTH project");
    for number in 1..=4 {
        sqlx::query(
            "INSERT INTO issues (id, workspace_id, project_id, number, title, state, priority, author_id)
                  VALUES ($1, $2, $3, $4, $5, 'backlog', 'medium', $6)",
        )
        .bind(uuid::Uuid::now_v7())
        .bind(workspace.0)
        .bind(project_id)
        .bind(number)
        .bind(format!("Seeded issue {number}"))
        .bind(author.0)
        .execute(pool)
        .await
        .expect("seed AUTH issue");
    }
}

#[given(regex = r#"^AUTH-2 is titled "([^"]+)"$"#)]
async fn auth2_titled(world: &mut FoundryWorld, title: String) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    sqlx::query(
        "UPDATE issues SET title = $1
          WHERE number = 2
            AND project_id = (SELECT id FROM projects WHERE key_prefix = 'AUTH')",
    )
    .bind(&title)
    .execute(pool)
    .await
    .expect("retitle AUTH-2");
}

/// The lane probe's entry point. chromedriver + the session already came up in the
/// Background (which is where the sign-in probe ran); this navigates that session
/// to the board served on the harness's real port.
#[given(regex = r"^the browser lane has started chromedriver and navigated to the AUTH board$")]
async fn lane_navigated_to_board(world: &mut FoundryWorld) {
    let url = board_url(world);
    world
        .browser
        .as_ref()
        .expect("browser session")
        .goto(&url)
        .await
        .expect("navigate to the AUTH board");
}

#[given(regex = r"^Mei is viewing the AUTH project board$")]
async fn viewing_board(_world: &mut FoundryWorld) {
    scaffold_pending();
}

// --- When: the key presses (the heart of every scenario) --------------------

/// Presses `key` on the REAL page. The keystroke goes to `document.body` (no
/// text field focused), which is where the ADR-001 document-delegated listener
/// receives it — the same path a human's keypress takes.
#[when(regex = r#"^Mei presses "([^"]+)"$"#)]
async fn mei_presses_key(world: &mut FoundryWorld, key: String) {
    let browser = world.browser.as_ref().expect("browser session");
    browser_harness::wait_for_kb_ready(browser).await;
    browser
        .find(Locator::Css("body"))
        .await
        .expect("find the document body")
        .send_keys(&key)
        .await
        .unwrap_or_else(|err| panic!("press {key:?}: {err}"));
}

#[when(regex = r#"^Mei types "[^"]+" into the title field$"#)]
async fn mei_types_into_title(_world: &mut FoundryWorld) {
    scaffold_pending();
}

#[when(regex = r#"^Mei types "[^"]+" into the search box$"#)]
async fn mei_types_into_search(_world: &mut FoundryWorld) {
    scaffold_pending();
}

// --- Then: the user-visible outcomes ----------------------------------------

/// The ADR-001 readiness marker. Both this lane's wait condition and US-02's
/// "the layer is live" precondition, so the anti-vacuity guard has a real hook.
#[then(regex = r"^the page reports the keyboard layer is ready$")]
async fn keyboard_layer_ready(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    browser_harness::wait_for_kb_ready(browser).await;
}

/// The Earned-Trust assertion (ADR-007 §4). The Background signed in through the
/// real form; if the browser had REFUSED the `Secure`-over-plain-HTTP cookie, or
/// declined to send it back, this navigation would have bounced to `/sign-in` and
/// the board's own markup would be absent. Failing HERE, once, at lane start, is
/// the point: a Secure-cookie substrate change becomes ONE clear diagnostic
/// instead of every scenario mysteriously failing at sign-in.
#[then(
    regex = r"^Mei is still signed in after the browser accepts the session cookie over plain HTTP$"
)]
async fn still_signed_in_over_plain_http(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let url = browser.current_url().await.expect("read the current URL");
    assert!(
        !url.path().starts_with("/sign-in"),
        "the browser bounced back to {url} — it did not present the session cookie on a plain-HTTP \
         navigation. `session_cookie_secure: true` (harness.rs:401-406) is emitted over http://; \
         the sanctioned fix is making that flag conditional for the browser harness, NOT working \
         around it per-scenario."
    );
    browser.find(Locator::Css(".board")).await.expect(
        "the AUTH board did not render for a signed-in Mei — the session cookie was not \
             honoured over plain HTTP",
    );
}

/// `?` -> the shipped `GET /keyboard-help` fragment, rendered into the ADR-003
/// `#kb-overlay-root` host, OVER the board. Asserts the user-visible outcome
/// (an overlay listing the shortcuts, displayed, with the board still behind it),
/// not the mechanism.
#[then(regex = r"^the keyboard shortcut list appears as an overlay over the (?:board|dashboard)$")]
async fn overlay_appears(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let overlay = browser
        .wait()
        .at_most(std::time::Duration::from_secs(10))
        .for_element(Locator::Css("#kb-overlay-root .keyboard-help"))
        .await
        .expect("pressing \"?\" must render the shortcut list into #kb-overlay-root");
    assert!(
        overlay.is_displayed().await.expect("overlay displayed?"),
        "the shortcut list is in the DOM but not displayed — it is not an overlay the user can see"
    );
    let listed = overlay
        .find_all(Locator::Css("dt[data-shortcut]"))
        .await
        .expect("read the listed shortcuts");
    assert!(
        !listed.is_empty(),
        "the overlay rendered but lists no shortcuts — GET /keyboard-help's <dt data-shortcut> \
         pairs never reached the host"
    );
    assert!(
        browser
            .find(Locator::Css(".board"))
            .await
            .expect("the board must still exist behind the overlay")
            .is_displayed()
            .await
            .expect("board displayed?"),
        "the overlay replaced the board instead of layering over it"
    );
}

#[then(regex = r"^the new-issue modal opens over the board$")]
async fn new_issue_modal_opens(_world: &mut FoundryWorld) {
    scaffold_pending();
}

#[then(regex = r"^(?:the first visible card is highlighted as selected|no modal opens)$")]
async fn selection_or_no_modal(_world: &mut FoundryWorld) {
    scaffold_pending();
}

#[then(regex = r"^the browser did not navigate away from the board$")]
async fn no_navigation(_world: &mut FoundryWorld) {
    scaffold_pending();
}

// NOTE: This is a REPRESENTATIVE subset. The remaining ~70 concrete Given/When/Then
// phrases in keyboard-shortcut-bindings.feature are added by DELIVER as it unskips
// each slice (they are inert while the scenarios are @pending). Keeping the starter
// small avoids registering broad regexes that could later collide once real
// per-slice steps are introduced.
