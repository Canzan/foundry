//! keyboard-shortcut-bindings — the `@needs-browser` lane's step definitions.
//!
//! SLICE 01 / STEP 01-01 landed the INSTRUMENT (ADR-007): the lane-probe
//! scenario drives a REAL headless Chrome, via chromedriver, against the REAL
//! app served by `InProcHarness::base_url()`.
//!
//! STEP 01-02 unskips three more US-01 scenarios — the overlay-over-context
//! arms (board still visible / no navigation), the ODD-3 no-modal-mount page,
//! and the US-07 Esc-restores arm. All three proved GREEN BY INHERITANCE: 01-01's
//! `keyboard.js` + the `base.html`-hosted `#kb-overlay-root` already satisfy
//! them, so this step contributed the ASSERTIONS, not the binding. That is a
//! legitimate outcome (the roadmap's slice-01 dispatch layer was built once and
//! covers all four), but it is only meaningful because the assertions are
//! FALSIFIABLE: unbinding `?` in `keyboard.js` reds all four. Anyone widening
//! these steps should re-run that check rather than trust the green.
//!
//! Every remaining scenario still carries `@pending` (excluded from EVERY lane)
//! and its steps still panic with the `__SCAFFOLD__` marker below — assertion-class
//! RED (correct test / missing production), never IMPORT_ERROR / BROKEN.
//!
//! STILL @pending IN SLICE 01 — "the overlay lists exactly the seven advertised
//! shortcuts and each is bound". Its middle arm, *"every shortcut it lists is
//! bound and does something"*, cannot hold until slice 05: `slice-01`'s own scope
//! puts KPI-1 at **2/7** ("no character key is bound here"), and `c` / `/` / `j` /
//! `k` / `Enter` land in slices 03-05. Binding them now to satisfy the assertion
//! would be speculative code with no requiring test; asserting only over the
//! implemented subset would weaken the scenario. It reds honestly until 7/7 —
//! see the DELIVER report for step 01-02.
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

fn dashboard_url(world: &FoundryWorld) -> String {
    let base = world.harness.as_ref().expect("harness").base_url();
    format!("{base}/dashboard")
}

/// The overlay host's contents (ADR-003). `#kb-overlay-root` is empty exactly
/// when no help is showing, so "is the help open?" is a DOM question with a DOM
/// answer — never a stored flag that htmx could desync (ADR-003 §2).
const OVERLAY_SELECTOR: &str = "#kb-overlay-root .keyboard-help";

/// Every modal the app can mount, counted at its shipped host. `new_issue_modal.html:1`
/// and `issue_edit_modal.html` both carry `data-modal`, and both are swapped into
/// `#modal-root` — so "how many modals are open?" is one DOM query.
const MODAL_SELECTOR: &str = "#modal-root [data-modal]";

/// The same question asked of the WHOLE document. On a page with no `#modal-root`
/// — the dashboard — `MODAL_SELECTOR` is zero for ANY implementation, so a
/// no-modal assertion scoped to the host would be vacuous exactly where AC-03.3
/// needs it to bite.
const ANY_MODAL_SELECTOR: &str = "[data-modal]";

/// The board's own shipped new-issue trigger (`board.html:6`). It carries the
/// hx-get, the hx-target and the swap; `keyboard.js` CLICKS it rather than
/// reconstructing its URL, so the keyboard path and the pointer path open the same
/// modal by the same mechanism and the client does zero CSRF work (DESIGN
/// wave-decisions). Its ABSENCE on the dashboard is what makes `c` a no-op there.
const NEW_ISSUE_TRIGGER_SELECTOR: &str = "[data-action='new-issue']";

/// The autofocused title input the guard must protect (`new_issue_modal.html:6`).
const TITLE_FIELD_SELECTOR: &str = "#modal-root [data-modal='new-issue'] input[name='title']";

/// How many modals are mounted right now.
async fn open_modal_count(browser: &fantoccini::Client) -> usize {
    browser
        .find_all(Locator::Css(MODAL_SELECTOR))
        .await
        .expect("count the open modals")
        .len()
}

/// How many modals are mounted anywhere in the document.
async fn any_modal_count(browser: &fantoccini::Client) -> usize {
    browser
        .find_all(Locator::Css(ANY_MODAL_SELECTOR))
        .await
        .expect("count every modal on the page")
        .len()
}

/// Whether the help overlay is showing. Typing `?` into a field must not open it,
/// so this is one of the paired assertion's witnesses.
async fn help_overlay_is_open(browser: &fantoccini::Client) -> bool {
    !browser
        .find_all(Locator::Css(OVERLAY_SELECTOR))
        .await
        .expect("look for the help overlay")
        .is_empty()
}

/// The `name` + `value` of whatever currently has focus. The guard is a function
/// of the LIVE focus (ADR-002), so focus is the thing these scenarios interrogate.
async fn focused_field(browser: &fantoccini::Client) -> (String, String) {
    let described = browser
        .execute(
            "var el = document.activeElement;
             if (!el) { return ['<none>', '']; }
             return [el.tagName + (el.name ? '[name=' + el.name + ']' : ''), el.value || ''];",
            Vec::new(),
        )
        .await
        .expect("read the focused element");
    let parts = described.as_array().expect("focus probe returns a pair");
    (
        parts[0].as_str().unwrap_or_default().to_string(),
        parts[1].as_str().unwrap_or_default().to_string(),
    )
}

/// No card is marked as selected. Selection lands in slice 05, so today this is a
/// standing zero — but asserting it HERE is what makes "no selection moves"
/// falsifiable the moment `j`/`k` are bound: if the guard ever lets them through
/// from a text field, this reds.
async fn selected_card_count(browser: &fantoccini::Client) -> usize {
    browser
        .find_all(Locator::Css(
            ".issue-card[aria-selected='true'], .issue-card[data-kb-selected]",
        ))
        .await
        .expect("count the selected cards")
        .len()
}

/// Drives the REAL `c` key on the board and waits for the modal — the shared
/// precondition of the guard scenarios. A Given that mounted the modal via htmx
/// or `execute()` would be Fixture Theater: it would green the guard half over a
/// layer where `c` is not bound at all, which is the exact vacuity D15 exists to
/// prevent.
async fn open_new_issue_modal_by_pressing_c(world: &mut FoundryWorld) {
    let url = board_url(world);
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .goto(&url)
        .await
        .expect("navigate to the AUTH board");
    browser_harness::wait_for_kb_ready(browser).await;
    browser
        .find(Locator::Css("body"))
        .await
        .expect("find the document body")
        .send_keys("c")
        .await
        .expect("press \"c\"");
    browser
        .wait()
        .at_most(std::time::Duration::from_secs(10))
        .for_element(Locator::Css(TITLE_FIELD_SELECTOR))
        .await
        .expect(
            "pressing \"c\" on the board must open the new-issue modal — the paired assertion's \
             first half (D15). Without this the guard half below is VACUOUS: a layer that binds \
             nothing at all would pass it.",
        );
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
    // Advance the project's per-project issue-number allocator past the seeded
    // issues. These four rows were INSERTed directly, which bypasses
    // `insert_issue_with_outbox`'s `UPDATE projects SET next_issue_number = ...`
    // (store/lib.rs:1378) — the ONLY writer of that counter. Without this, the
    // column still reads 1 while AUTH-1..4 exist, so the next real create is
    // handed number 1, collides with the seeded AUTH-1's unique key, and the POST
    // answers 500.
    //
    // That state is UNREACHABLE through the application: no code path creates an
    // issue without advancing the counter. A fixture that builds it is asserting
    // over a database the app can never produce — the pointer path would 500 here
    // too, and it would have nothing to do with the keyboard. `GREATEST` mirrors
    // the shipped idiom (`feature_mwt_slice_01_coexist.rs:347`,
    // `feature_card_ranking_within_status.rs:109`), which seed issues the same way
    // and repair the counter for the same reason.
    sqlx::query(
        "UPDATE projects SET next_issue_number = GREATEST(next_issue_number, 5) WHERE id = $1",
    )
    .bind(project_id)
    .execute(pool)
    .await
    .expect("advance next_issue_number past the seeded AUTH issues");
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
async fn viewing_board(world: &mut FoundryWorld) {
    let url = board_url(world);
    world
        .browser
        .as_ref()
        .expect("browser session")
        .goto(&url)
        .await
        .expect("navigate to the AUTH board");
}

/// The ADR-003 / ODD-3 scenario's precondition: the dashboard extends
/// `base.html` but — unlike `board.html:13` — carries NO `#modal-root`. The
/// step ASSERTS that absence rather than assuming it, so the day someone hoists
/// the modal mount into the shared shell this scenario stops being the
/// no-modal-mount case it claims to be, loudly, instead of silently.
#[given(regex = r"^Mei is viewing the dashboard, a page with no modal mount point$")]
async fn viewing_dashboard(world: &mut FoundryWorld) {
    let url = dashboard_url(world);
    let browser = world.browser.as_ref().expect("browser session");
    browser.goto(&url).await.expect("navigate to the dashboard");
    browser
        .find(Locator::Css("h1"))
        .await
        .expect("the dashboard must render for a signed-in Mei");
    assert!(
        browser
            .find_all(Locator::Css("#modal-root"))
            .await
            .expect("look for a modal mount")
            .is_empty(),
        "the dashboard now has a #modal-root — this scenario exists to prove `?` works on a page \
         WITHOUT one (ADR-003 / ODD-3). Either the mount was hoisted into the shared shell (which \
         ADR-003 rejects) or this scenario needs a different no-modal-mount page."
    );
}

// --- When: the key presses (the heart of every scenario) --------------------

/// Presses `key` on the REAL page. The keystroke goes to `document.body` (no
/// text field focused), which is where the ADR-001 document-delegated listener
/// receives it — the same path a human's keypress takes.
///
/// Named keys are mapped to their WebDriver code points: `send_keys("Esc")`
/// would type the three characters E, s, c.
#[when(regex = r#"^Mei presses "([^"]+)"$"#)]
async fn mei_presses_key(world: &mut FoundryWorld, key: String) {
    let browser = world.browser.as_ref().expect("browser session");
    browser_harness::wait_for_kb_ready(browser).await;
    browser
        .find(Locator::Css("body"))
        .await
        .expect("find the document body")
        .send_keys(browser_harness::key_chord(&key))
        .await
        .unwrap_or_else(|err| panic!("press {key:?}: {err}"));
}

/// Types into the title field the way Mei does: into the FOCUSED element, one
/// real keystroke at a time, through the same document-delegated listener every
/// other press in this lane goes through. `send_keys` on the element (rather
/// than `execute`-ing a value assignment) is what makes this a guard test at all
/// — setting `.value` from JS fires no `keydown` and would green over a layer
/// that eats every keystroke.
#[when(regex = r#"^Mei types "([^"]+)" into the title field$"#)]
async fn mei_types_into_title(world: &mut FoundryWorld, text: String) {
    type_into_title(world, &text).await;
}

/// The paired assertion's second half (D15). Same action as the step above; the
/// feature file words it differently because here the characters ARE the point —
/// `c`, `j`, `k`, `/` and `?` are five of the seven advertised shortcuts, so
/// every one of them is a keystroke the unguarded layer would have swallowed.
#[when(regex = r#"^Mei types the characters "([^"]+)" into the title field$"#)]
async fn mei_types_characters_into_title(world: &mut FoundryWorld, text: String) {
    type_into_title(world, &text).await;
}

async fn type_into_title(world: &mut FoundryWorld, text: &str) {
    let browser = world.browser.as_ref().expect("browser session");
    browser_harness::wait_for_kb_ready(browser).await;
    browser
        .find(Locator::Css(TITLE_FIELD_SELECTOR))
        .await
        .expect("the new-issue modal must carry a title field to type into")
        .send_keys(text)
        .await
        .unwrap_or_else(|err| panic!("type {text:?} into the title field: {err}"));
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
#[then(regex = r"^the keyboard shortcut list appears as an overlay over the (board|dashboard)$")]
async fn overlay_appears(world: &mut FoundryWorld, surface: String) {
    let browser = world.browser.as_ref().expect("browser session");
    let overlay = browser
        .wait()
        .at_most(std::time::Duration::from_secs(10))
        .for_element(Locator::Css(OVERLAY_SELECTOR))
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
    // The page Mei was on must still be underneath. `.board` on the board;
    // the dashboard has no `.board` — its own heading is the "still there"
    // witness (ADR-003: `#kb-overlay-root` lives in base.html, so the overlay
    // does not depend on the board's markup at all).
    let beneath = match surface.as_str() {
        "dashboard" => "h1",
        _ => ".board",
    };
    assert!(
        browser
            .find(Locator::Css(beneath))
            .await
            .unwrap_or_else(|err| panic!(
                "the {surface} ({beneath}) must still exist behind the overlay: {err}"
            ))
            .is_displayed()
            .await
            .expect("page beneath displayed?"),
        "the overlay replaced the {surface} instead of layering over it"
    );
}

// --- SLICE 03 (US-03): `c` files an issue, end to end, from the keyboard ------

/// AC-03.1, first arm. The modal is OVER the board: mounted in `#modal-root` and
/// the board still rendered behind it — the same "layered, not replaced"
/// distinction `overlay_appears` draws for `?`.
#[then(regex = r"^the new-issue modal opens over the board$")]
async fn new_issue_modal_opens(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let modal = browser
        .wait()
        .at_most(std::time::Duration::from_secs(10))
        .for_element(Locator::Css(MODAL_SELECTOR))
        .await
        .expect(
            "pressing \"c\" on the AUTH board must open the new-issue modal. `keyboard.js` binds \
             `c` to a click on board.html:6's OWN `[data-action='new-issue']` trigger, which \
             carries the hx-get and hx-target — so a failure here means either the key is not \
             dispatched or the shipped trigger is gone.",
        );
    assert!(
        modal.is_displayed().await.expect("modal displayed?"),
        "the new-issue modal is in the DOM but not displayed — Mei cannot see the form she is \
         supposed to type into"
    );
    assert_eq!(
        open_modal_count(browser).await,
        1,
        "expected exactly the new-issue modal to be open after pressing \"c\""
    );
    assert!(
        browser
            .find(Locator::Css(".board"))
            .await
            .expect("the AUTH board must still be rendered behind the modal")
            .is_displayed()
            .await
            .expect("board displayed?"),
        "the modal replaced the board instead of opening over it — `c` navigated Mei away instead \
         of layering a dialog on the page she was on"
    );
}

/// AC-03.1, second arm — the focus handoff, and the reason this scenario exists
/// rather than "a modal appeared".
///
/// A modal that opens UNFOCUSED makes Mei reach for the mouse, which defeats the
/// whole JTBD: the point of `c` is that her hands never leave the keyboard. So
/// the assertion is about `document.activeElement`, read from the LIVE page — not
/// about the presence of an `autofocus` attribute, which is a claim about markup
/// rather than about where the next keystroke lands.
///
/// "…and ready for typing" is the EMPTY half: a field that opened focused but
/// pre-seeded with the `c` that opened it is the classic bug (the same one FR-7
/// names for `/`), and it would pass a focus-only assertion.
#[then(regex = r"^the title field is focused and ready for typing$")]
async fn title_field_focused_and_ready(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let (focused, value) = focused_field(browser).await;
    assert_eq!(
        focused, "INPUT[name=title]",
        "the new-issue modal opened with {focused} focused instead of the title field. Mei's next \
         keystroke goes nowhere and she has to reach for the mouse — which is exactly the \
         hands-on-keyboard flow `c` exists to provide (AC-03.1)."
    );
    assert_eq!(
        value, "",
        "the title field opened focused but already containing {value:?} — the \"c\" that opened \
         the modal leaked into the field. Mei has to clear it before she can type, so the \
         shortcut costs her a keystroke instead of saving one."
    );
}

/// AC-03.2's precondition, driven by a REAL `c` press. A Given that mounted the
/// modal via htmx or `execute()` would be Fixture Theater: the scenario's claim is
/// the KEYBOARD round trip, so the keyboard has to open it.
#[given(regex = r#"^Mei has opened the new-issue modal by pressing "c"$"#)]
async fn modal_opened_by_pressing_c(world: &mut FoundryWorld) {
    open_new_issue_modal_by_pressing_c(world).await;
}

/// AC-03.2 — the full round trip, with NO POINTER AT ANY STEP.
///
/// Keys go to `active_element()`, never to an element this step located by CSS.
/// That is the whole discipline: `find(TITLE_FIELD_SELECTOR).send_keys(...)`
/// FOCUSES the field as a side effect, so it would green this scenario over a
/// modal that opened unfocused — i.e. over a build where Mei really does have to
/// reach for the mouse. Typing into whatever the browser says is focused is the
/// only way this step can speak to the keyboard flow it claims.
///
/// Submitting is `Enter`, not a click on the Create button, for the same reason.
/// `Enter` reaches the form because the guard chain (ADR-002) declines it inside a
/// text-entry context — `NATIVE_TEXT_ENTRY_KEYS` names it as a key the browser
/// acts on natively — so the browser submits the form through the shipped
/// `hx-post`. No client code is involved in the submit at all.
#[when(regex = r#"^Mei types "([^"]+)" and submits the form$"#)]
async fn types_and_submits(world: &mut FoundryWorld, text: String) {
    let browser = world.browser.as_ref().expect("browser session");
    // The board's issue keys BEFORE filing, stashed on the page so the Then can
    // name the card that is genuinely NEW. This matters here specifically: the
    // Background titles AUTH-2 "Session cookie not cleared on sign-out", which is
    // the very title Mei types. "a card with that title exists" is therefore TRUE
    // BEFORE she files anything — a vacuous assertion. The delta is the fact.
    let captured = browser
        .execute(
            "window.__kbFiledTitle = arguments[0];
             window.__kbKeysBeforeFiling = Array.prototype.map.call(
               document.querySelectorAll('.board .issue-card[data-issue-key]'),
               function (card) { return card.getAttribute('data-issue-key'); }
             );
             return window.__kbKeysBeforeFiling.length;",
            vec![serde_json::json!(text)],
        )
        .await
        .expect("record the board's issue keys before filing");
    assert!(
        captured.as_u64().unwrap_or(0) > 0,
        "the AUTH board shows no issue cards before filing, so the \"a NEW issue appeared\" \
         assertion below would have no baseline to be new against"
    );

    let active = browser
        .active_element()
        .await
        .expect("read the focused element to type into");
    active.send_keys(&text).await.unwrap_or_else(|err| {
        panic!(
            "could not type {text:?} into the focused element: {err}. This step types into \
             whatever the browser says is FOCUSED — deliberately, so it cannot pass over a modal \
             that opened unfocused. If this fails, the focus handoff (AC-03.1) is broken."
        )
    });
    active
        .send_keys(browser_harness::key_chord("Enter"))
        .await
        .expect("press \"Enter\" to submit the new-issue form");
}

/// AC-03.2's outcome, stated as the DELTA rather than as a presence check.
///
/// The shipped POST answers htmx with an out-of-band append
/// (`issues.rs:553` — `hx-swap-oob="beforeend:[data-column='backlog']"`), so the
/// new card lands in the board's Backlog column and the modal clears. This asserts
/// exactly ONE key appeared that was not there before, and that ITS card carries
/// the typed title — not merely that some card somewhere does, which AUTH-2 already
/// satisfies.
#[then(regex = r"^a new issue with that title appears on the AUTH board$")]
async fn new_issue_appears_on_board(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    // The submit is an htmx round trip, so the card arrives asynchronously. Wait
    // on the CONDITION this scenario is about — the modal cleared, which is the
    // shipped response's own primary swap (issues.rs:553 answers with nothing but
    // an out-of-band append) — rather than on a duration.
    browser
        .wait()
        .at_most(std::time::Duration::from_secs(10))
        .for_element(Locator::Css("#modal-root:empty"))
        .await
        .expect(
            "submitting the new-issue form left the modal open. The shipped POST answers htmx with \
             an out-of-band append and an EMPTY primary body (issues.rs:553), so `#modal-root` \
             clearing is how the create is known to have succeeded — a modal still on screen means \
             the form was rejected or never submitted at all.",
        );
    let described = browser
        .execute(
            "var before = window.__kbKeysBeforeFiling || [];
             var title = window.__kbFiledTitle;
             var cards = Array.prototype.slice.call(
               document.querySelectorAll('.board .issue-card[data-issue-key]')
             );
             var added = cards.filter(function (card) {
               return before.indexOf(card.getAttribute('data-issue-key')) === -1;
             });
             return {
               added: added.map(function (card) {
                 var titleNode = card.querySelector('.title');
                 return {
                   key: card.getAttribute('data-issue-key'),
                   title: titleNode ? titleNode.textContent.trim() : '<no .title node>',
                   column: card.closest('[data-column]')
                     ? card.closest('[data-column]').getAttribute('data-column')
                     : '<not in a column>'
                 };
               }),
               expectedTitle: title
             };",
            Vec::new(),
        )
        .await
        .expect("read the board's new cards");
    let added = described["added"]
        .as_array()
        .expect("the probe reports the added cards")
        .clone();
    let expected_title = described["expectedTitle"].as_str().unwrap_or_default();
    assert_eq!(
        added.len(),
        1,
        "Mei typed a title and pressed Enter, and the AUTH board gained {} new card(s) (saw \
         {added:?}). Exactly one issue must appear. Note the Background already titles AUTH-2 \
         {expected_title:?} — that is why this asserts the DELTA and not merely that some card \
         carries the title, which was true before she filed anything.",
        added.len()
    );
    assert_eq!(
        added[0]["title"].as_str(),
        Some(expected_title),
        "a new card appeared on the board but it is titled {:?} instead of {expected_title:?} — \
         the title Mei typed did not reach the issue that was created",
        added[0]["title"].as_str().unwrap_or_default()
    );
}

// --- AC-03.3: the create key is scoped to a surface that has a project --------

/// AC-03.3's precondition, ASSERTED rather than assumed. The dashboard is the
/// no-project surface, and what makes it one is the ABSENCE of board.html:6's
/// `[data-action='new-issue']` trigger — which is precisely the thing `c` looks
/// for. If someone ever renders that trigger into the shared shell, this scenario
/// stops being the no-project case it claims to be, and it says so here rather
/// than passing quietly.
#[given(regex = r"^Mei is viewing the dashboard, a page with no team or project$")]
async fn viewing_dashboard_no_project(world: &mut FoundryWorld) {
    let url = dashboard_url(world);
    let browser = world.browser.as_ref().expect("browser session");
    browser.goto(&url).await.expect("navigate to the dashboard");
    browser
        .find(Locator::Css("h1"))
        .await
        .expect("the dashboard must render for a signed-in Mei");
    browser_harness::wait_for_kb_ready(browser).await;
    assert!(
        browser
            .find_all(Locator::Css(NEW_ISSUE_TRIGGER_SELECTOR))
            .await
            .expect("look for a new-issue trigger")
            .is_empty(),
        "the dashboard carries a {NEW_ISSUE_TRIGGER_SELECTOR} — this scenario exists to prove \
         \"c\" is a no-op where there is NO project to create an issue in (AC-03.3). With the \
         trigger present there IS a target, and the scenario proves nothing."
    );
}

/// AC-03.3, first arm. Scoped to the WHOLE document, not to `#modal-root`: the
/// dashboard has no modal mount at all, so `#modal-root [data-modal]` is zero by
/// construction there and would pass over any implementation whatsoever. Asking
/// "is there a modal ANYWHERE" is the only form of this assertion that a broken
/// `c` could fail.
///
/// The `?` sentinel first, for the reason `press_help_and_await_overlay` documents:
/// a negative assertion needs the layer proven LIVE (an unbound layer opens no
/// modal either) and needs the `c` press to have SETTLED. `openNewIssue()` clicks
/// synchronously inside the keydown handler, so had `c` fired, its htmx GET would
/// have been issued strictly before the help fetch this blocks on. An ordering
/// argument on one loopback origin — not a sleep.
#[then(regex = r"^no modal opens$")]
async fn no_modal_opens(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    press_help_and_await_overlay(browser).await;
    let modals = any_modal_count(browser).await;
    assert_eq!(
        modals, 0,
        "pressing \"c\" on a page with no project opened {modals} modal(s) — a shortcut with no \
         target must be a no-op, never a modal for an issue that has nowhere to go (AC-03.3)"
    );
}

/// AC-03.3, second arm — and the one with teeth. "No modal" alone would pass for
/// an implementation that RECONSTRUCTED the URL and did `location.href = ...`:
/// there would be no modal on the dashboard, because Mei would no longer be on the
/// dashboard. That is the failure this arm names. It is also why `c` clicks
/// board.html:6's own trigger instead of building a URL — with no trigger there is
/// nothing to click, and doing nothing falls out for free (DESIGN wave-decisions).
#[then(regex = r"^the browser does not navigate away$")]
async fn browser_does_not_navigate_away(world: &mut FoundryWorld) {
    let expected = dashboard_url(world);
    let browser = world.browser.as_ref().expect("browser session");
    let actual = browser.current_url().await.expect("read the current URL");
    assert_eq!(
        actual.as_str().trim_end_matches('/'),
        expected.trim_end_matches('/'),
        "pressing \"c\" took Mei from the dashboard to {actual}. A shortcut with no target is a \
         no-op — never a navigation, and never an error (AC-03.3). This is what reds if anyone \
         reconstructs the new-issue URL instead of clicking the board's own shipped trigger."
    );
}

// --- SLICE 02 (US-02): the guard chain (ADR-002) -----------------------------

/// The paired assertion's starting state, asserted rather than assumed: the board
/// is up, the layer is live, and nothing is focused into a text field — so a `c`
/// pressed next reaches the dispatch layer on its merits.
#[given(regex = r"^Mei is viewing the AUTH board with no text field focused$")]
async fn viewing_board_nothing_focused(world: &mut FoundryWorld) {
    let url = board_url(world);
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .goto(&url)
        .await
        .expect("navigate to the AUTH board");
    browser_harness::wait_for_kb_ready(browser).await;
    let (focused, _) = focused_field(browser).await;
    assert!(
        !focused.starts_with("INPUT") && !focused.starts_with("TEXTAREA"),
        "the board loaded with {focused} already focused — this scenario's whole premise is that \
         the FIRST half runs OUTSIDE a text field. With a field focused the guard would suppress \
         \"c\" and the \"layer is live\" half would fail for the wrong reason."
    );
}

/// The paired assertion's first half (D15) — and the reason this scenario is not
/// vacuous. `c` fired from the board, so the layer is demonstrably BOUND. Only
/// against that proof does the second half's "nothing fired" mean anything.
#[then(regex = r"^the new-issue modal opens, proving the shortcut layer is live$")]
async fn modal_opens_proving_layer_live(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .wait()
        .at_most(std::time::Duration::from_secs(10))
        .for_element(Locator::Css(TITLE_FIELD_SELECTOR))
        .await
        .expect(
            "pressing \"c\" outside any text field did not open the new-issue modal, so the \
             shortcut layer is NOT live. The guard half of this scenario would now pass \
             VACUOUSLY — which is exactly what D15 pairs these halves to prevent. Fix the \
             binding; do NOT split this scenario.",
        );
    assert_eq!(
        open_modal_count(browser).await,
        1,
        "expected exactly the new-issue modal to be open after pressing \"c\""
    );
}

/// The heart of the feature (AC-02.2). Every character Mei typed landed, in order,
/// unswallowed — including `c`, `j`, `k`, `/` and `?`, five advertised shortcuts.
#[then(regex = r"^exactly those characters are entered into the field$")]
async fn exactly_those_characters_entered(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let value = browser
        .find(Locator::Css(TITLE_FIELD_SELECTOR))
        .await
        .expect("the title field must still be there to have received the characters")
        .prop("value")
        .await
        .expect("read the title field's value")
        .unwrap_or_default();
    assert_eq!(
        value, "cjk/?",
        "Mei typed \"cjk/?\" into the title and the field holds {value:?}. The guard chain \
         (ADR-002) is letting shortcut keys through from a text-entry context — she cannot type."
    );
}

/// The guard half's negative witnesses, all three at once because the scenario
/// asserts them as one outcome: her keystrokes did nothing EXCEPT enter text.
/// `?` did not open the help, `c` did not open a second modal, `/` did not grab a
/// search box, and `j`/`k` moved no selection.
#[then(regex = r"^no additional modal opens, no search is focused, and no selection moves$")]
async fn nothing_else_fired(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let modals = open_modal_count(browser).await;
    assert_eq!(
        modals, 1,
        "typing into the title opened {modals} modals — \"c\" fired from inside a text field"
    );
    assert!(
        !help_overlay_is_open(browser).await,
        "typing \"?\" into the title opened the help overlay — the guard chain does not cover the \
         title field, so Mei cannot type a question mark into an issue title"
    );
    let (focused, _) = focused_field(browser).await;
    assert_eq!(
        focused, "INPUT[name=title]",
        "focus left the title field for {focused} while Mei was typing — a shortcut fired and \
         moved it (\"/\" grabbing a search box is the intended reading of this assertion)"
    );
    let selected = selected_card_count(browser).await;
    assert_eq!(
        selected, 0,
        "typing \"jk\" into the title moved the board selection ({selected} card(s) selected) — \
         j/k fired from inside a text field"
    );
}

/// Same precondition as the paired assertion's first half, and driven the same
/// way — through a real `c` press — for the same anti-vacuity reason.
#[given(regex = r"^Mei has the new-issue modal open on the AUTH board$")]
async fn new_issue_modal_open_on_board(world: &mut FoundryWorld) {
    open_new_issue_modal_by_pressing_c(world).await;
}

#[then(regex = r#"^the title field contains "([^"]+)"$"#)]
async fn title_field_contains(world: &mut FoundryWorld, expected: String) {
    let browser = world.browser.as_ref().expect("browser session");
    let value = browser
        .find(Locator::Css(TITLE_FIELD_SELECTOR))
        .await
        .expect("the title field must still be there to have received the text")
        .prop("value")
        .await
        .expect("read the title field's value")
        .unwrap_or_default();
    assert_eq!(
        value, expected,
        "Mei typed {expected:?} and the title holds {value:?}. Note the \"c\", \"/\", \"k\" and \
         \"i\" in that sentence — each is a keystroke an unguarded layer eats (ADR-002)."
    );
}

#[then(regex = r"^no additional modal was opened$")]
async fn no_additional_modal(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let modals = open_modal_count(browser).await;
    assert_eq!(
        modals, 1,
        "{modals} modals are open — typing a \"c\" into the title fired the create shortcut"
    );
    assert!(
        !help_overlay_is_open(browser).await,
        "the help overlay opened while Mei typed into the title field"
    );
}

#[then(regex = r"^no card selection changed$")]
async fn no_card_selection_changed(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let selected = selected_card_count(browser).await;
    assert_eq!(
        selected, 0,
        "{selected} card(s) became selected while Mei typed into the title field"
    );
}

/// AC-02.6's precondition — and the assertion that keeps this scenario honest.
///
/// The guard reads the LIVE event target (ADR-002), so "the shortcuts come back"
/// is only a claim about anything if Mei has ACTUALLY left the field. This step
/// therefore drives the real keys and then asserts focus is out of the input. If
/// it is not, this reds HERE with a plain diagnostic rather than at the `c` press
/// below, where a reader would misdiagnose it as a broken guard.
#[given(regex = r#"^Mei has typed in the title field and then pressed "Esc" to leave it$"#)]
async fn typed_then_escaped_out_of_title(world: &mut FoundryWorld) {
    open_new_issue_modal_by_pressing_c(world).await;
    type_into_title(world, "cache").await;
    let browser = world.browser.as_ref().expect("browser session");
    let (focused, value) = focused_field(browser).await;
    assert_eq!(
        (focused.as_str(), value.as_str()),
        ("INPUT[name=title]", "cache"),
        "Mei's typing did not land in the focused title field, so this scenario is not yet in the \
         state it describes"
    );
    browser
        .find(Locator::Css(TITLE_FIELD_SELECTOR))
        .await
        .expect("the title field must be there to press Esc in")
        .send_keys(browser_harness::key_chord("Esc"))
        .await
        .expect("press \"Esc\" in the title field");
    browser
        .wait()
        .at_most(std::time::Duration::from_secs(10))
        .for_element(Locator::Css("#modal-root:empty"))
        .await
        .expect(
            "\"Esc\" pressed in the title field did not close the new-issue modal, so Mei never \
             left the text-entry context and the re-enablement this scenario asserts (AC-02.6) \
             cannot be exercised at all.",
        );
    let (after, _) = focused_field(browser).await;
    assert!(
        !after.starts_with("INPUT"),
        "after \"Esc\" the focus is still in {after} — Mei has not left the text field"
    );
}

/// AC-02.6. The guard is a function of the CURRENT focus, never a sticky flag: the
/// press AFTER leaving the field works, with no state to reset. This is the K-E4
/// failure mode (shortcuts stay dead after a field is touched) made falsifiable.
#[then(regex = r"^the new-issue modal opens$")]
async fn new_issue_modal_reopens(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .wait()
        .at_most(std::time::Duration::from_secs(10))
        .for_element(Locator::Css(TITLE_FIELD_SELECTOR))
        .await
        .expect(
            "\"c\" did nothing after Mei left the title field — the guard has latched. It must \
             read the live event target, not a mode flag set on focus (ADR-002, AC-02.6).",
        );
}

/// Still slice 05's. `no modal opens` shared this scaffold until step 03-01 gave
/// it a real assertion (AC-03.3), so only the selection arm remains pending.
#[then(regex = r"^the first visible card is highlighted as selected$")]
async fn first_visible_card_selected(_world: &mut FoundryWorld) {
    scaffold_pending();
}

// --- SLICE 02 / STEP 02-02: the guard chain's three edges --------------------
//
// All three edges were already in `isInert` when this step began (01-01 built the
// whole chain), so the production code below is GREEN BY INHERITANCE and this
// step's contribution is the ASSERTIONS. That is only worth anything if they are
// FALSIFIABLE — see the per-scenario notes for what breaks each one.

/// The `?` press's outcome is a full `GET /keyboard-help` round-trip into
/// `#kb-overlay-root`. Pressing it AFTER the thing under test and waiting for the
/// overlay is this lane's SENTINEL for a negative assertion: `openNewIssue` calls
/// `trigger.click()` SYNCHRONOUSLY inside the keydown handler, so had a chord
/// fired, htmx's GET would have been issued strictly BEFORE the help fetch this
/// wait blocks on. The overlay arriving therefore means the earlier request either
/// landed (and we would see a modal) or was never made. That is an ordering
/// argument on one loopback origin, not a sleep — and it is what makes "no modal
/// opened" a settled fact rather than a race we happened to win.
async fn press_help_and_await_overlay(browser: &fantoccini::Client) {
    browser
        .find(Locator::Css("body"))
        .await
        .expect("find the document body")
        .send_keys(browser_harness::key_chord("?"))
        .await
        .expect("press \"?\" as the sentinel");
    browser
        .wait()
        .at_most(std::time::Duration::from_secs(10))
        .for_element(Locator::Css(OVERLAY_SELECTOR))
        .await
        .expect(
            "the sentinel \"?\" press did not open the help overlay, so the shortcut layer is not \
             live and every negative assertion in this scenario would pass VACUOUSLY — an unbound \
             layer opens no modal for a copy chord either. Fix the binding before reading the \
             result below.",
        );
}

/// Records, for every `c`-ish keydown that reaches the END of the bubble path,
/// which modifiers carried it and whether anything called `preventDefault()`.
///
/// On `window`, so it runs AFTER `keyboard.js`'s `document` listener: what it sees
/// is the event's state once our layer has had its turn. `defaultPrevented === false`
/// is the OBSERVABLE meaning of "left for the browser to handle" — it is the
/// difference between the layer declining the chord and the layer swallowing it.
const INSTALL_CHORD_PROBE: &str = r#"
window.__kbChordProbe = [];
window.addEventListener("keydown", function (event) {
  if (event.key !== "c" && event.key !== "C") { return; }
  window.__kbChordProbe.push({
    ctrl: event.ctrlKey === true,
    meta: event.metaKey === true,
    defaultPrevented: event.defaultPrevented === true,
  });
});
return true;
"#;

/// WebDriver's code points for the modifiers this scenario asserts BOTH of.
/// Linux CI's copy chord is Ctrl and macOS's is Meta; asserting only the local
/// one would green here and rot on the other runner (ADR-007 honest limit 2).
const WEBDRIVER_CTRL: char = '\u{E009}';
const WEBDRIVER_META: char = '\u{E03D}';
const WEBDRIVER_SHIFT: char = '\u{E008}';

/// Presses `character` while `modifier` is held, as a real key-down/up sequence
/// through the Actions API — the browser produces a genuine `KeyboardEvent` with
/// the modifier flag set, which is the thing guard 2 reads.
async fn press_chord(browser: &fantoccini::Client, modifier: char, character: char) {
    use fantoccini::actions::{InputSource, KeyAction, KeyActions};
    let actions = KeyActions::new("kb-chord".to_string())
        .then(KeyAction::Down { value: modifier })
        .then(KeyAction::Down { value: character })
        .then(KeyAction::Up { value: character })
        .then(KeyAction::Up { value: modifier });
    browser
        .perform_actions(actions)
        .await
        .unwrap_or_else(|err| panic!("press the {modifier:?}+{character:?} chord: {err}"));
}

/// AC-02.3's precondition. Selecting the text is what makes this a COPY chord
/// rather than an arbitrary modifier press: it is the state in which a real user
/// hits Ctrl/Cmd+C and expects the browser — not Foundry — to act.
///
/// The selection is ASSERTED, not assumed. A chord pressed over an empty
/// selection is a different scenario than the one this claims to be.
#[given(regex = r#"^Mei is viewing the AUTH board with the text "([^"]+)" selected on the page$"#)]
async fn board_with_text_selected(world: &mut FoundryWorld, text: String) {
    let url = board_url(world);
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .goto(&url)
        .await
        .expect("navigate to the AUTH board");
    browser_harness::wait_for_kb_ready(browser).await;
    // Read back from `window.getSelection()` — the DOCUMENT's live selection, not
    // the Range object this script built — so the assertion below witnesses that
    // the selection actually took.
    //
    // Via `getRangeAt(0)` rather than `Selection.toString()`: the latter is
    // specified over the selection AS RENDERED and returns "" under headless
    // Chrome even for a selection that is unambiguously present (rangeCount === 1,
    // a visible node, a 56x18 client rect). Verified directly while writing this
    // step. `getRangeAt(0)` reads the same live selection through its DOM contents
    // instead of the paint, which is the thing this precondition is actually about.
    let selected = browser
        .execute(
            "var node = document.querySelector(arguments[0]);
             if (!node) { return '<no such element>'; }
             var range = document.createRange();
             range.selectNodeContents(node);
             var selection = window.getSelection();
             selection.removeAllRanges();
             selection.addRange(range);
             if (selection.rangeCount !== 1) { return '<the selection did not take>'; }
             return selection.getRangeAt(0).toString();",
            vec![serde_json::json!(format!("#issue-{text} .key"))],
        )
        .await
        .expect("select the issue key's text on the board");
    assert_eq!(
        selected.as_str().unwrap_or_default(),
        text,
        "the board does not carry a selectable {text:?} to copy (issue_card.html:1 renders it as \
         `#issue-{text} .key`), so this scenario would press a copy chord over an EMPTY selection \
         — a different scenario than the one it claims to be"
    );
    browser
        .execute(INSTALL_CHORD_PROBE, Vec::new())
        .await
        .expect("install the chord probe");
}

/// BOTH modifiers, in one press step, because the scenario asserts both (ADR-007
/// honest limit 2): Linux CI's copy chord is Ctrl and macOS's is Meta. Asserting
/// only the local platform's would leave the other runner's arm untested.
#[when(regex = r"^Mei presses the copy chord with Ctrl and again with Cmd$")]
async fn presses_copy_chord_both_modifiers(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    press_chord(browser, WEBDRIVER_CTRL, 'c').await;
    press_chord(browser, WEBDRIVER_META, 'c').await;
    press_help_and_await_overlay(browser).await;
}

/// AC-02.3, first arm. NON-ACTIVATION — never "the text was copied", which is
/// unassertable headless (ADR-007 honest limit 2). The chord is a browser/OS
/// affordance; a keyboard layer that grabs it steals Copy from every user.
#[then(regex = r"^the new-issue modal does not open for either modifier$")]
async fn no_modal_for_either_modifier(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let modals = open_modal_count(browser).await;
    assert_eq!(
        modals, 0,
        "the copy chord opened {modals} modal(s) — \"c\" fired while a modifier was held, so \
         Ctrl+C / Cmd+C files an issue instead of copying the selection (ADR-002 guard 2)"
    );
}

/// AC-02.3, second arm — and the one that says INERT rather than merely "did
/// nothing". A layer could open no modal and still have swallowed the chord with
/// `preventDefault()`; then Copy is broken and no modal-count assertion notices.
/// `defaultPrevented === false` is the observable difference.
#[then(regex = r"^the keydown default was not prevented for either modifier$")]
async fn default_not_prevented_for_either_modifier(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let probe = browser
        .execute("return window.__kbChordProbe;", Vec::new())
        .await
        .expect("read the chord probe");
    let seen = probe
        .as_array()
        .expect("the probe records an array")
        .clone();
    let carried = |ctrl: bool, meta: bool| {
        seen.iter().find(|entry| {
            entry["ctrl"].as_bool() == Some(ctrl) && entry["meta"].as_bool() == Some(meta)
        })
    };
    for (label, ctrl, meta) in [("Ctrl", true, false), ("Cmd", false, true)] {
        let entry = carried(ctrl, meta).unwrap_or_else(|| {
            panic!(
                "no {label}+C keydown ever reached the end of the bubble path (the probe saw \
                 {seen:?}). Either the chord was never delivered, or something CONSUMED the event \
                 before it bubbled — both mean this scenario cannot speak to guard 2."
            )
        });
        assert_eq!(
            entry["defaultPrevented"].as_bool(),
            Some(false),
            "the {label}+C keydown came back with defaultPrevented === true — the keyboard layer \
             swallowed the copy chord instead of leaving it to the browser. No modal opened, so a \
             modal-count assertion alone would have called this a pass, and Copy would be broken \
             for every user (ADR-002 guard 2)."
        );
    }
}

/// AC-02.4 / BR-7 — the scenario that reds on the OBVIOUS wrong implementation
/// ("ignore any keydown with a modifier"). `?` IS Shift+/ on a US layout, so a
/// guard that treats Shift as a suppressor makes the help key — one of the seven
/// — STRUCTURALLY UNREACHABLE, and every other Shift-produced character with it.
///
/// Driven through the Actions API rather than `send_keys("?")` ON PURPOSE: this
/// scenario's whole subject is that `shiftKey` is true on the event, and only a
/// real Shift-down/slash/Shift-up sequence makes it so.
#[when(regex = r#"^Mei presses "\?" which the browser produces as Shift and "/"$"#)]
async fn presses_shift_slash(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    browser_harness::wait_for_kb_ready(browser).await;
    let observed = browser
        .execute(
            "window.__kbShiftProbe = null;
             window.addEventListener('keydown', function (event) {
               if (event.key === '?') {
                 window.__kbShiftProbe = { shift: event.shiftKey === true, key: event.key };
               }
             });
             return true;",
            Vec::new(),
        )
        .await
        .expect("install the shift probe");
    assert_eq!(observed.as_bool(), Some(true), "shift probe installed");
    press_chord(browser, WEBDRIVER_SHIFT, '/').await;
    let probe = browser
        .execute("return window.__kbShiftProbe;", Vec::new())
        .await
        .expect("read the shift probe");
    assert_eq!(
        probe["shift"].as_bool(),
        Some(true),
        "the Shift+/ sequence did not reach the page as a `?` keydown carrying shiftKey === true \
         (the probe saw {probe:?}). This scenario exists to prove Shift is NOT a suppressor; \
         without shiftKey set on the event it proves nothing at all."
    );
}

/// The IME scenario's precondition (ADR-007 honest limit 1, restated in code
/// because it governs how much this scenario is worth): **WebDriver `send_keys`
/// cannot produce composition**, so the composing state is SIMULATED — a real
/// `CompositionEvent` dispatched at the focused title field. Listeners fire for
/// untrusted events, so our predicate IS truthfully exercised; the INPUT METHOD
/// is not. A real-IME regression can still reach Mei, and the `@manual` scenario
/// at the foot of the feature file carries that residual risk explicitly rather
/// than letting this green imply it away.
///
/// The modal is opened by a REAL `c` press (never htmx-injected): a Given that
/// mounted it directly would green this over a layer where `c` is not bound.
#[given(regex = r"^Mei's Japanese IME is composing text in the title field$")]
async fn ime_composing_in_title(world: &mut FoundryWorld) {
    open_new_issue_modal_by_pressing_c(world).await;
    let browser = world.browser.as_ref().expect("browser session");
    let composing = browser
        .execute(
            "var field = document.querySelector(arguments[0]);
             if (!field) { return '<no title field>'; }
             field.focus();
             field.dispatchEvent(new CompositionEvent('compositionstart', {
               bubbles: true, cancelable: true, data: ''
             }));
             field.dispatchEvent(new CompositionEvent('compositionupdate', {
               bubbles: true, cancelable: true, data: 'ち'
             }));
             return document.activeElement === field ? 'composing' : '<title field not focused>';",
            vec![serde_json::json!(TITLE_FIELD_SELECTOR)],
        )
        .await
        .expect("begin an IME composition in the title field");
    assert_eq!(
        composing.as_str(),
        Some("composing"),
        "the title field is not focused and composing, so there is no composition for the next \
         key to arrive in the middle of"
    );
    assert_eq!(
        open_modal_count(browser).await,
        1,
        "exactly the new-issue modal must be open before the composing key arrives — otherwise \
         \"no ADDITIONAL modal opens\" has no baseline to be additional to"
    );
}

/// The composing keydown. `isComposing: true` AND `keyCode: 229` together, because
/// `isInert`'s guard 1 reads both: 229 is the legacy composition sentinel that
/// stays reliable on IME/browser pairs where `isComposing` is unset on the
/// composition-TERMINATING event.
///
/// `dispatchEvent` returns false exactly when a listener called
/// `preventDefault()`, which is how the "left to the input method" arm below gets
/// a real observable instead of a tautology (an untrusted keydown inserts no text
/// either way, so asserting the field's value would assert nothing).
#[when(regex = r#"^a "c" key arrives while composition is in progress$"#)]
async fn composing_c_arrives(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let not_prevented = browser
        .execute(
            "var field = document.querySelector(arguments[0]);
             if (!field) { return '<no title field>'; }
             var event = new KeyboardEvent('keydown', {
               key: 'c', code: 'KeyC', keyCode: 229, which: 229,
               isComposing: true, bubbles: true, cancelable: true
             });
             window.__kbImeSurvived = field.dispatchEvent(event);
             return window.__kbImeSurvived;",
            vec![serde_json::json!(TITLE_FIELD_SELECTOR)],
        )
        .await
        .expect("deliver a \"c\" keydown mid-composition");
    assert!(
        not_prevented.is_boolean(),
        "the composing keydown was never dispatched ({not_prevented:?}) — the title field was \
         gone, so this scenario exercised nothing"
    );
}

/// AC-02.1's IME arm. `c` mid-composition must not file an issue — for Mei that is
/// a modal erupting over the word she is halfway through typing.
#[then(regex = r"^no additional modal opens$")]
async fn no_additional_modal_opens(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let modals = open_modal_count(browser).await;
    assert_eq!(
        modals, 1,
        "{modals} modals are open — a \"c\" delivered mid-composition fired the create shortcut, \
         so composing any word beginning with \"c\" erupts a modal over Mei's half-typed text \
         (ADR-002 guard 1)"
    );
    assert!(
        !help_overlay_is_open(browser).await,
        "the help overlay opened from a composing keystroke"
    );
}

/// "Left to the input method" made observable: the layer did not call
/// `preventDefault()` on the composing keydown, so the IME keeps the keystroke it
/// is in the middle of composing with. A layer that suppressed the key would open
/// no modal EITHER — the arm above cannot tell the two apart, which is why this
/// one exists.
#[then(regex = r"^the composing character is left to the input method$")]
async fn composing_character_left_to_ime(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let survived = browser
        .execute("return window.__kbImeSurvived;", Vec::new())
        .await
        .expect("read whether the composing keydown survived");
    assert_eq!(
        survived.as_bool(),
        Some(true),
        "the composing \"c\" keydown was cancelled (preventDefault) by the page — the keyboard \
         layer took a keystroke that belongs to the input method, so Mei's composition loses the \
         character. No modal opened, so the assertion above would have called this a pass."
    );
}

/// FR-4: `?` is an OVERLAY, not a page transition. The shipped full-page
/// `GET /keyboard-help` route stays reachable — this asserts the SHORTCUT did
/// not route Mei to it (which is exactly what a naive `location.href =
/// "/keyboard-help"` binding would do, and would otherwise look "green" to a
/// test that only checked the shortcut list is on screen).
#[then(regex = r"^the browser did not navigate away from the board$")]
async fn no_navigation(world: &mut FoundryWorld) {
    let expected = board_url(world);
    let browser = world.browser.as_ref().expect("browser session");
    let actual = browser.current_url().await.expect("read the current URL");
    assert_eq!(
        actual.as_str().trim_end_matches('/'),
        expected.trim_end_matches('/'),
        "pressing \"?\" navigated Mei from the board to {actual} — the help must layer OVER the \
         page she is on, leaving the URL untouched (FR-4). She lost her place."
    );
}

/// AC-01.1's second half. `overlay_appears` proves the list is on screen; this
/// proves the board is STILL THERE UNDERNEATH — the difference between an
/// overlay and a replacement.
#[then(regex = r"^the board is still visible behind it$")]
async fn board_visible_behind(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    assert!(
        browser
            .find(Locator::Css(".board"))
            .await
            .expect("the board must still exist behind the overlay")
            .is_displayed()
            .await
            .expect("board displayed?"),
        "the overlay replaced the board instead of layering over it — Mei's place is gone"
    );
}

/// Mei has the help open over the board: the precondition shared by the Esc
/// scenario and the advertised-set contract. Drives the REAL `?` press rather
/// than injecting the overlay — a Given that rendered the fragment itself would
/// be Fixture Theater, greening the Esc arm over an unbound `?`.
#[given(
    regex = r"^Mei ha[sd] (?:the|opened the) help overlay open over the AUTH board$|^Mei has opened the help overlay on the AUTH board$"
)]
async fn help_overlay_open_over_board(world: &mut FoundryWorld) {
    let url = board_url(world);
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .goto(&url)
        .await
        .expect("navigate to the AUTH board");
    browser_harness::wait_for_kb_ready(browser).await;
    browser
        .find(Locator::Css("body"))
        .await
        .expect("find the document body")
        .send_keys(browser_harness::key_chord("?"))
        .await
        .expect("press \"?\"");
    browser
        .wait()
        .at_most(std::time::Duration::from_secs(10))
        .for_element(Locator::Css(OVERLAY_SELECTOR))
        .await
        .expect("pressing \"?\" must open the help overlay");
}

/// US-07 / AC-01.4: `Esc` peels the help layer. Asserted as the DOM condition
/// ADR-003 §2 makes canonical — the host is empty — via a bounded wait, never a
/// sleep.
#[then(regex = r"^the help overlay closes$")]
async fn help_overlay_closes(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .wait()
        .at_most(std::time::Duration::from_secs(10))
        .for_element(Locator::Css("#kb-overlay-root:empty"))
        .await
        .expect(
            "pressing \"Esc\" must clear #kb-overlay-root — the help overlay is still on screen",
        );
}

/// "…with nothing else changed" is the whole point of US-07: Esc RESTORES, it
/// does not navigate, reload, or disturb the page beneath.
#[then(regex = r"^Mei is still on the AUTH board with nothing else changed$")]
async fn still_on_board_unchanged(world: &mut FoundryWorld) {
    let expected = board_url(world);
    let browser = world.browser.as_ref().expect("browser session");
    let actual = browser.current_url().await.expect("read the current URL");
    assert_eq!(
        actual.as_str().trim_end_matches('/'),
        expected.trim_end_matches('/'),
        "\"Esc\" navigated Mei to {actual} instead of simply dismissing the help — Esc must never \
         navigate (FR-5)"
    );
    assert!(
        browser
            .find(Locator::Css(".board"))
            .await
            .expect("the AUTH board must still be rendered after Esc")
            .is_displayed()
            .await
            .expect("board displayed?"),
        "the board is gone after \"Esc\" — Mei did not end up where she was"
    );
}

// --- The no-JS path (AC-01.5 / NFR-4 / ODD-8) -------------------------------

/// Replaces the Background's ordinary session with one whose BROWSER has
/// JavaScript switched off, signs in through the real form (a plain POST form —
/// no-JS by construction), and lands on the AUTH board.
///
/// The two assertions here are the anti-vacuity guard, and they are the reason
/// this Given is not just "goto the board": they prove scripting is REALLY off.
/// `.board` present says the page rendered; `[data-kb-ready]` absent says
/// `keyboard.js` never ran. Without the second, a session that silently kept
/// scripting on would green this scenario while proving nothing about the
/// scripting-off path — which is the failure mode this whole feature exists to
/// close.
#[given(regex = r"^scripting is disabled in Mei's browser on the AUTH board$")]
async fn scripting_disabled_on_board(world: &mut FoundryWorld) {
    let url = board_url(world);
    if let Some(previous) = world.browser.take() {
        let _ = previous.close().await;
    }
    let browser = browser_harness::new_session_without_scripting().await;
    {
        let harness = world.harness.as_ref().expect("harness");
        browser_harness::sign_in_through_browser(&browser, harness, MEI_EMAIL, MEI_PASSWORD).await;
    }
    browser
        .goto(&url)
        .await
        .expect("navigate to the AUTH board with scripting off");
    browser.find(Locator::Css(".board")).await.expect(
        "the AUTH board must render with scripting off — the board is server-rendered HTML and \
         must not depend on script to exist (NFR-4)",
    );
    assert!(
        browser
            .find_all(Locator::Css(browser_harness::KB_READY_SELECTOR))
            .await
            .expect("look for the keyboard-layer readiness marker")
            .is_empty(),
        "{} is present with scripting disabled — the browser ran keyboard.js, so scripting is NOT \
         actually off and this scenario would prove nothing about the no-JS path",
        browser_harness::KB_READY_SELECTOR
    );
    world.browser = Some(browser);
}

/// A POINTER path to the help: the shipped `sidebar.html:13` link, clicked.
/// ADR-003 (ODD-8) keeps this link precisely so it exists — removing it reds here.
#[when(regex = r#"^Mei follows the sidebar "Keyboard shortcuts" link$"#)]
async fn follows_sidebar_help_link(world: &mut FoundryWorld) {
    world
        .browser
        .as_ref()
        .expect("browser session")
        .find(Locator::Css(".sidebar a[href='/keyboard-help']"))
        .await
        .expect(
            "the sidebar must carry a \"Keyboard shortcuts\" link — it is the no-JS path to the \
             shortcut list (ADR-003 / ODD-8) and the only way to read it with scripting off",
        )
        .click()
        .await
        .expect("follow the sidebar \"Keyboard shortcuts\" link");
}

/// The full-page help — `GET /keyboard-help` as its own page rather than layered
/// over the board. With scripting off there is no overlay to be confused with, so
/// "shown" is exactly: this URL, this list, on screen.
#[then(regex = r"^the full-page keyboard help is shown$")]
async fn full_page_help_shown(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let url = browser.current_url().await.expect("read the current URL");
    assert_eq!(
        url.path(),
        "/keyboard-help",
        "the sidebar link led to {url} instead of the full-page help"
    );
    let help = browser.find(Locator::Css(".keyboard-help")).await.expect(
        "the full-page keyboard help did not render with scripting off — the link is the no-JS \
         path to the shortcut list (NFR-4)",
    );
    assert!(
        help.is_displayed().await.expect("help displayed?"),
        "the shortcut list is in the DOM but not displayed — Mei cannot read it"
    );
    assert!(
        !help
            .find_all(Locator::Css("dt[data-shortcut]"))
            .await
            .expect("read the listed shortcuts")
            .is_empty(),
        "the help page rendered but lists no shortcuts"
    );
}

/// BR-6, scoped honestly to what slice 01 binds.
///
/// Slice 01 binds exactly two shortcuts: `?` (show this list) and `Esc` (close
/// it). `Esc` only undoes `?`, so `?` is the sole advertised ACTION in scope, and
/// this asserts it is not keyboard-only: the very list `?` renders is here, on a
/// page reached by a pointer click, in a browser that cannot press `?` at all.
/// The `?` row's own presence is the falsifiable part — a full-page fork that
/// dropped it ("you are already here") would red, and that row is what tells a
/// no-JS reader the capability exists.
///
/// LIMIT, stated rather than implied: this does NOT prove BR-6 for `c` / `/` /
/// `j` / `k` / `Enter`. Those are bound in slices 02-05 and their pointer paths
/// belong to those slices; `architecture.md:411` already names search as JS-only
/// with no full-page fork. This step asserts the help capability only.
#[then(regex = r"^no advertised action is reachable only by keyboard$")]
async fn no_keyboard_only_action(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let keys: Vec<String> = {
        let mut collected = Vec::new();
        for entry in browser
            .find_all(Locator::Css(".keyboard-help dt[data-shortcut]"))
            .await
            .expect("read the advertised shortcuts")
        {
            collected.push(
                entry
                    .attr("data-shortcut")
                    .await
                    .expect("read data-shortcut")
                    .unwrap_or_default(),
            );
        }
        collected
    };
    assert!(
        keys.iter().any(|key| key == "?"),
        "the pointer-reachable help does not advertise \"?\" itself (it lists {keys:?}) — the one \
         action slice 01 binds would then be discoverable only to someone who already knew the \
         key, which is what BR-6 forbids"
    );
    let described = browser
        .find_all(Locator::Css(".keyboard-help dd"))
        .await
        .expect("read the shortcut descriptions")
        .len();
    assert_eq!(
        described,
        keys.len(),
        "the help lists {} shortcuts but {described} descriptions — an advertised key with no \
         label tells a no-JS reader nothing about what it does",
        keys.len()
    );
}

// NOTE: This is a REPRESENTATIVE subset. The remaining ~70 concrete Given/When/Then
// phrases in keyboard-shortcut-bindings.feature are added by DELIVER as it unskips
// each slice (they are inert while the scenarios are @pending). Keeping the starter
// small avoids registering broad regexes that could later collide once real
// per-slice steps are introduced.
