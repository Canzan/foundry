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

// The `scaffold_pending()` panic marker lived here until step 05-01 wired the
// last step that used it (`the first visible card is highlighted as selected`).
// Every remaining @pending scenario's steps are UNWRITTEN rather than scaffolded,
// and `.fail_on_skipped()` (UI-4) makes an unwired step FAIL by name the moment
// one is unskipped — a louder, more honest RED than a shared panic string, and
// the reason the scaffold is not kept "just in case" (AGENTS.md: remove dead code
// outright, do not leave it inert).

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

/// The ADR-005 search panel. JS-injected by `keyboard.js` on any board page —
/// deliberately NOT in the templates (ADR-005 accepts that cost), so this
/// selector is the only place the lane names it.
const SEARCH_PANEL_SELECTOR: &str = "#kb-search-panel";

/// The box `/` focuses. `name='q'` is not decoration: it is the shipped route's
/// own query parameter (`SearchQuery`, keyboard.rs), so the panel speaks the
/// server's vocabulary rather than a client-invented one.
const SEARCH_INPUT_SELECTOR: &str = "#kb-search-panel input[name='q']";

/// The SHIPPED fragment's own markup (`partials/search_results.html:2,4`),
/// asserted by the names the server already renders. If these scenarios matched
/// on client-invented classes, they would pass over a client that reimplemented
/// matching — which is exactly what ADR-005 §2 ("zero template delta", results
/// honoured as-is) forbids.
const SEARCH_RESULTS_SELECTOR: &str = "#kb-search-panel ul.search-results";
const SEARCH_RESULT_ROW_SELECTOR: &str = "#kb-search-panel li.search-result[data-issue-key]";
const SEARCH_EMPTY_SELECTOR: &str = "#kb-search-panel ul.search-results[data-empty='true']";

/// The rows the results list is showing right now, as (key, title) pairs read
/// from the shipped fragment's own `data-issue-key` + `.title`.
async fn search_result_rows(browser: &fantoccini::Client) -> Vec<(String, String)> {
    let rows = browser
        .execute(
            "return Array.prototype.map.call(
               document.querySelectorAll(arguments[0]),
               function (row) {
                 var title = row.querySelector('.title');
                 return [row.getAttribute('data-issue-key'), title ? title.textContent : ''];
               }
             );",
            vec![SEARCH_RESULT_ROW_SELECTOR.into()],
        )
        .await
        .expect("read the search result rows");
    rows.as_array()
        .expect("the search-result probe returns an array")
        .iter()
        .map(|pair| {
            let pair = pair.as_array().expect("each row is a [key, title] pair");
            (
                pair[0].as_str().unwrap_or_default().to_string(),
                pair[1].as_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

/// Waits until the results list settles into the state `predicate` accepts.
///
/// The fragment arrives over a real `fetch`, so a bare read races the network and
/// would flake green-or-red at random. Bounded and polled: it fails with the
/// list's ACTUAL contents rather than a timeout with no diagnosis, and it cannot
/// pass by waiting — the predicate has to hold.
async fn wait_for_results<F>(browser: &fantoccini::Client, what: &str, predicate: F)
where
    F: Fn(&[(String, String)]) -> bool,
{
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let seen = loop {
        let seen = search_result_rows(browser).await;
        if predicate(&seen) {
            return;
        }
        if std::time::Instant::now() >= deadline {
            break seen;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    };
    panic!(
        "the search results never showed {what}.\n  showing instead: {seen:?}\n  The results are \
         the SHIPPED `GET …/search?q=` fragment (keyboard.rs, search_results.html), honoured \
         as-is per ADR-005 §2 — so either `/` never revealed the panel, the input never fetched, \
         or the fragment was not mounted into {SEARCH_PANEL_SELECTOR}."
    );
}

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

/// The issue keys the board is rendering, in the order Mei sees them. Sorted, so
/// the "nothing else changed" assertion speaks to WHICH cards survived an `Esc`
/// rather than to a column ordering that belongs to another feature.
async fn board_issue_keys(browser: &fantoccini::Client) -> Vec<String> {
    let keys = browser
        .execute(
            "return Array.prototype.map.call(
               document.querySelectorAll('.board .issue-card[data-issue-key]'),
               function (card) { return card.getAttribute('data-issue-key'); }
             ).sort();",
            Vec::new(),
        )
        .await
        .expect("read the board's issue keys");
    keys.as_array()
        .expect("the issue-key probe returns an array")
        .iter()
        .map(|key| key.as_str().unwrap_or_default().to_string())
        .collect()
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

/// The board's cards IN THE ORDER MEI SEES THEM, derived from the RENDERED
/// GEOMETRY (`getBoundingClientRect`), not from DOM order.
///
/// This is the scenario's ORACLE and it is deliberately computed a different way
/// than any implementation would: sort by column x, then by y within the column
/// — literally "left column top to bottom, then the next column". An assertion
/// against `querySelectorAll` order would merely restate the implementation's own
/// traversal and would pass for a `j` that walked a list which happened to share
/// the DOM's order for the wrong reason. Rects are what Mei's eyes get.
///
/// Off-screen and zero-area cards are excluded: an element with no box is not
/// something "visible" can mean.
async fn visible_card_order(browser: &fantoccini::Client) -> Vec<String> {
    let keys = browser
        .execute(
            r#"return Array.prototype.map.call(
                 document.querySelectorAll('.board .issue-card[data-issue-key]'),
                 function (card) {
                   var r = card.getBoundingClientRect();
                   return {
                     key: card.getAttribute('data-issue-key'),
                     x: Math.round(r.left + window.scrollX),
                     y: Math.round(r.top + window.scrollY),
                     boxed: r.width > 0 && r.height > 0
                   };
                 }
               ).filter(function (c) { return c.boxed; })
                .sort(function (a, b) { return (a.x - b.x) || (a.y - b.y); })
                .map(function (c) { return c.key; });"#,
            Vec::new(),
        )
        .await
        .expect("read the board's cards in on-screen order");
    json_strings(&keys, "the on-screen card order")
}

/// The hidden `#kb-items` carrier's order (`board.html:12`) — ASC by issue number
/// across every column, which is a DIFFERENT order than the visible board's
/// column-grouped DESC (ADR-008). Read ONLY so the `@kb-items-collision` scenario
/// can prove the two orders genuinely disagree, i.e. that walking the carrier
/// would be observably wrong rather than accidentally right.
///
/// Returns empty once ADR-008's retirement lands (step 05-05 deletes the
/// carrier); the collision arm self-retires with it — see its own note.
async fn carrier_order(browser: &fantoccini::Client) -> Vec<String> {
    let keys = browser
        .execute(
            "return Array.prototype.map.call(
               document.querySelectorAll('#kb-items li[data-issue-key]'),
               function (li) { return li.getAttribute('data-issue-key'); }
             );",
            Vec::new(),
        )
        .await
        .expect("read the hidden #kb-items carrier order");
    json_strings(&keys, "the #kb-items carrier order")
}

fn json_strings(value: &serde_json::Value, what: &str) -> Vec<String> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("{what} probe returns an array"))
        .iter()
        .map(|key| key.as_str().unwrap_or_default().to_string())
        .collect()
}

/// The keys of every card currently wearing the ring. A Vec, not an Option: "two
/// cards are selected" is a real failure mode (a projection that adds without
/// clearing) and it must be nameable rather than silently reading the first.
async fn selected_keys(browser: &fantoccini::Client) -> Vec<String> {
    let keys = browser
        .execute(
            r#"return Array.prototype.map.call(
                 document.querySelectorAll(".board .issue-card[aria-selected='true']"),
                 function (card) { return card.getAttribute('data-issue-key'); }
               );"#,
            Vec::new(),
        )
        .await
        .expect("read the selected cards");
    json_strings(&keys, "the selection")
}

/// EXACTLY `expected` is selected. `context` names the action under test so the
/// failure reads as a sentence about the product, not about a selector.
async fn assert_selected_is(browser: &fantoccini::Client, expected: &str, context: &str) {
    let selected = selected_keys(browser).await;
    assert_eq!(
        selected,
        vec![expected.to_string()],
        "after {context} the ring is on {selected:?}, but Mei is looking at {expected} — \
         selection must land on the card she sees there, and on exactly one card (ADR-004)"
    );
}

/// Collects the errors the PAGE reports, from now on: uncaught exceptions
/// (`window.onerror`) and `console.error` — the two channels `keyboard.js` uses
/// for a real defect (it deliberately never swallows).
///
/// Installed AFTER the Given's own presses, so the count starts at zero and any
/// entry belongs to the keystroke under test. Without this, "no error occurs"
/// would be unfalsifiable prose: a handler that threw on every `k` at the
/// boundary would still leave the ring where it was, and the scenario would pass
/// over a broken build.
async fn install_error_probe(browser: &fantoccini::Client) {
    browser
        .execute(
            "window.__kbErrors = [];
             window.addEventListener('error', function (e) {
               window.__kbErrors.push('uncaught: ' + e.message);
             });
             window.addEventListener('unhandledrejection', function (e) {
               window.__kbErrors.push('unhandled rejection: ' + e.reason);
             });
             var native = console.error.bind(console);
             console.error = function () {
               window.__kbErrors.push(Array.prototype.join.call(arguments, ' '));
               return native.apply(console, arguments);
             };
             return null;",
            Vec::new(),
        )
        .await
        .expect("install the page-error probe");
}

/// Stamps the URL Mei is resting on INTO the live page, so a later "does not
/// navigate away" assertion can name it without this module hard-coding which
/// page the scenario happens to be on (`c` rests on the dashboard, `Esc` on the
/// board — same claim, two surfaces).
///
/// The stamp is a `window` property ON PURPOSE: a navigation destroys the
/// document and takes it with it. So the assertion has two independent ways to
/// red — the URL differs, or the stamp is gone — and the second one bites even
/// if a navigation somehow lands back on the same URL.
async fn capture_url_at_rest(browser: &fantoccini::Client) {
    browser
        .execute(
            "window.__kbUrlAtRest = window.location.href; return null;",
            Vec::new(),
        )
        .await
        .expect("stamp the resting URL onto the page");
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

/// Types into the search box the way Mei does: real keystrokes into the element
/// `/` focused, through the same delegated listener. `send_keys` rather than an
/// `execute`-d value assignment on purpose — assigning `.value` fires no `input`
/// event, so the results fragment would never be fetched and the assertion would
/// be about a list nothing filled.
#[when(regex = r#"^Mei types "([^"]+)" into the search box$"#)]
async fn mei_types_into_search(world: &mut FoundryWorld, text: String) {
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .find(Locator::Css(SEARCH_INPUT_SELECTOR))
        .await
        .expect("the search panel must carry an input to type into")
        .send_keys(&text)
        .await
        .unwrap_or_else(|err| panic!("type {text:?} into the search box: {err}"));
}

/// The precondition the three finding scenarios share: the panel is open because
/// the REAL `/` key opened it. Not `execute`-d, not clicked — a Given that
/// revealed the panel any other way would green "Mei finds an issue" over a
/// layer where `/` is not bound at all, which is this feature's own disease.
#[given(regex = r#"^Mei has focused the board search box by pressing "/"$"#)]
async fn focused_search_by_pressing_slash(world: &mut FoundryWorld) {
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
        .send_keys("/")
        .await
        .expect("press the search key");
    let (focused, _) = focused_field(browser).await;
    assert!(
        focused.contains("[name=q]"),
        "pressing `/` on the board did not focus the search box (focus is on {focused} instead), \
         so the finding this scenario asserts cannot be exercised at all. ADR-005 §2: `/` reveals \
         and focuses the JS-injected panel."
    );
    // From here on, any focus() the page makes on this box is a RE-grab. The
    // probe counts from zero because the legitimate focus — the `/` above — has
    // already happened. Passive for every scenario that never reads the count.
    browser_harness::probe_focus_grabs(browser, SEARCH_INPUT_SELECTOR).await;
}

/// The Esc scenario's precondition: the panel is open (by the REAL `/`) AND
/// carries a query, so `Esc` has something to clear and the restore it asserts
/// is distinguishable from a panel that was empty all along.
#[given(regex = r#"^Mei has focused the board search box by pressing "/" and typed a query$"#)]
async fn focused_search_and_typed_query(world: &mut FoundryWorld) {
    focused_search_by_pressing_slash(world).await;
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .find(Locator::Css(SEARCH_INPUT_SELECTOR))
        .await
        .expect("the search panel must carry an input to type into")
        .send_keys("session")
        .await
        .expect("type a query into the search box");
    // The query has to have LANDED, or `Esc` would be clearing a box the
    // keystrokes had not reached yet and the scenario would pass on a race.
    let (_, value) = focused_field(browser).await;
    assert_eq!(
        value, "session",
        "the query never reached the search box, so this scenario cannot show `Esc` clearing one."
    );
}

// --- Then: the user-visible outcomes ----------------------------------------

/// FR-7 / AC-04.1, half one. Asked of the LIVE `document.activeElement`, so it
/// speaks to where Mei's next keystroke actually lands.
#[then(regex = r"^the search input is focused$")]
async fn search_input_is_focused(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .wait()
        .at_most(std::time::Duration::from_secs(5))
        .for_element(Locator::Css(SEARCH_INPUT_SELECTOR))
        .await
        .expect(
            "pressing `/` revealed no search input. ADR-005 §2: keyboard.js injects a hidden \
             search panel on every board page and `/` reveals + focuses it.",
        );
    let (focused, _) = focused_field(browser).await;
    assert!(
        focused.contains("[name=q]"),
        "pressing `/` left focus on {focused}, not the search box. Revealing the panel without \
         focusing it makes `/` an affordance Mei has to reach for with the mouse."
    );
}

/// FR-7 / AC-04.1, half two — THE CLASSIC BUG. The handler must
/// `preventDefault()` its own slash, or the very key that opens the box types a
/// stray `/` into it and Mei's first search is for `/session`. This assertion is
/// the whole regression guard: deleting the `preventDefault()` reds it here and
/// nowhere else.
#[then(regex = r"^the search input is empty$")]
async fn search_input_is_empty(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let (_, value) = focused_field(browser).await;
    assert_eq!(
        value, "",
        "the search box contains {value:?} after `/` opened it — the slash that opened the panel \
         was also typed INTO it (FR-7). The `/` handler must preventDefault() its own keypress."
    );
}

/// AC-04.5, arm one. `/` is one of the seven advertised shortcuts, and this is
/// the assertion that says it is subject to the SAME guard chain as every other
/// key: typed into a focused text field it is a character, not a command. Read
/// from the box's live `value`, so it speaks to what Mei sees.
#[then(regex = r#"^the search box contains "([^"]+)"$"#)]
async fn search_box_contains(world: &mut FoundryWorld, expected: String) {
    let browser = world.browser.as_ref().expect("browser session");
    let value = browser
        .find(Locator::Css(SEARCH_INPUT_SELECTOR))
        .await
        .expect("the search panel must carry an input to read")
        .prop("value")
        .await
        .expect("read the search box's value")
        .unwrap_or_default();
    assert_eq!(
        value, expected,
        "the search box holds {value:?}, not {expected:?} — the `/` Mei typed INTO the focused box \
         was eaten as a shortcut. Guard 4 makes a typed `/` inert in a text-entry context because \
         `/` is a character the field consumes (isConsumableByTextEntry), with no special case for \
         `/` anywhere: BR-2 forbids one. If this reds, the guard's domain has been narrowed past \
         the character keys and `c`, `j`, `k` and `?` are untypable too."
    );
}

/// AC-04.5, arm two — the PAIRED assertion, and it is not a restatement of arm
/// one. Arm one reds when `/`'s handler calls `preventDefault()`; this arm reds
/// when the handler RUNS AT ALL. A `dispatch` that focused the box without
/// preventing the default would leave the text intact — arm one green — while
/// yanking focus mid-word. Counting the grabs is the only way to see it: a
/// `focus()` on an already-focused element fires no event and moves no caret.
#[then(regex = r"^search focus was not grabbed again$")]
async fn search_focus_not_grabbed_again(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let grabs = browser_harness::focus_grabs(browser).await;
    assert_eq!(
        grabs, 0,
        "the page called focus() on the search box {grabs} time(s) while Mei was typing into it, \
         so `/`'s handler ran from inside a focused text field. The keystroke should never have \
         reached `dispatch`: guard 4 owns it (ADR-002)."
    );
    let (focused, _) = focused_field(browser).await;
    assert!(
        focused.contains("[name=q]"),
        "focus left the search box for {focused} while Mei typed into it — the count above is zero \
         only because nothing re-grabbed a box she had already lost."
    );
}

/// AC-04.6 / ADR-005 §2 — `Esc` "hides it, clears the query and results, and
/// restores the board", stated as all four of those things. The board arm is the
/// one that makes this more than "the panel is hidden": ADR-005 §4 rests on the
/// cards STAYING in the DOM (it is what lets slice 05's `Enter` resolve through
/// a board card), so a close that took the board with it would satisfy a
/// hidden-panel check and break the surface underneath.
#[then(regex = r"^the search panel closes and the board is restored$")]
async fn search_panel_closes_and_board_restored(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let state = browser
        .wait()
        .at_most(std::time::Duration::from_secs(5))
        .for_element(Locator::Css(
            format!("{SEARCH_PANEL_SELECTOR}[hidden]").as_str(),
        ))
        .await;
    assert!(
        state.is_ok(),
        "`Esc` left the search panel visible. ADR-003 §2: the panel is the third layer of the \
         DOM-derived Esc stack (help, else the modal, else the search panel) — it is not a fourth \
         handler and it is not exempt from the peel."
    );

    let (value, results) = browser
        .execute(
            "var input = document.querySelector(\"#kb-search-panel input[name='q']\");
             var results = document.querySelector('#kb-search-panel [data-search-results]');
             return [input ? input.value : '<gone>', results ? results.innerHTML.trim() : '<gone>'];",
            Vec::new(),
        )
        .await
        .map(|v| {
            let parts = v.as_array().expect("the panel probe returns a pair").clone();
            (
                parts[0].as_str().unwrap_or_default().to_string(),
                parts[1].as_str().unwrap_or_default().to_string(),
            )
        })
        .expect("read the search panel's state after Esc");
    assert_eq!(
        value, "",
        "`Esc` hid the panel but left {value:?} in the box, so the next `/` reopens Mei's LAST \
         search instead of a fresh one. ADR-005 §2: Esc clears the query."
    );
    assert_eq!(
        results, "",
        "`Esc` hid the panel but left its results mounted, so the next `/` shows stale answers to \
         a question Mei has not asked yet. ADR-005 §2: Esc clears the results."
    );

    let keys = board_issue_keys(browser).await;
    assert_eq!(
        keys,
        vec!["AUTH-1", "AUTH-2", "AUTH-3", "AUTH-4"],
        "the board is not intact after `Esc` closed the search panel — it shows {keys:?}. The \
         panel OVERLAYS the board rather than replacing it (ADR-005 §4), which is the property \
         slice 05's `Enter`-via-the-board-card depends on."
    );

    let (focused, _) = focused_field(browser).await;
    assert!(
        !focused.contains("[name=q]"),
        "`Esc` hid the panel but left focus inside its box ({focused}), so Mei is typing into a \
         field she cannot see and the board shortcuts stay inert behind guard 4."
    );
}

/// AC-04.2 — the substring path (`filter_matches`, keyboard.rs). Asserts on the
/// TITLE Mei reads, not on a key, because the claim is "typing part of a title
/// finds the issue".
#[then(regex = r#"^the results list shows the issue "([^"]+)"$"#)]
async fn results_show_issue_titled(world: &mut FoundryWorld, title: String) {
    let browser = world.browser.as_ref().expect("browser session");
    wait_for_results(browser, &format!("the issue titled {title:?}"), |rows| {
        rows.iter().any(|(_, shown)| shown.trim() == title)
    })
    .await;
}

/// AC-04.3 — the exact-key path. "EXACTLY" is the assertion: a client that
/// ignored the server's exact-key branch and substring-matched `AUTH-2` would
/// still show AUTH-2, so `len() == 1` is what makes this scenario bite.
#[then(regex = r"^the results list shows exactly the issue AUTH-2$")]
async fn results_show_exactly_auth2(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    wait_for_results(browser, "exactly AUTH-2 and nothing else", |rows| {
        rows.len() == 1 && rows[0].0 == "AUTH-2"
    })
    .await;
}

/// AC-04.4 — the shipped `data-empty="true"` marker (`search_results.html:2`).
/// Asserting the MARKER rather than "zero rows" is what distinguishes "nothing
/// matched" from "no query yet" and from "the fetch never happened" — all three
/// render zero rows, and only one of them is the outcome this scenario claims.
#[then(regex = r"^the results list shows an empty state indicating nothing matched$")]
async fn results_show_empty_state(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let empty = !browser
            .find_all(Locator::Css(SEARCH_EMPTY_SELECTOR))
            .await
            .expect("look for the empty-state marker")
            .is_empty();
        if empty {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "a search matching nothing never rendered the empty state. The shipped fragment \
             returns `ul.search-results[data-empty=\"true\"]` for no matches \
             (search_results.html:2) — so either the fragment was never fetched, or the client \
             dropped the marker while mounting it."
        );
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    let rows = search_result_rows(browser).await;
    assert!(
        rows.is_empty(),
        "the empty state is marked, but the list still shows {rows:?} — the marker and the \
         contents disagree."
    );
    assert!(
        !browser
            .find_all(Locator::Css(SEARCH_RESULTS_SELECTOR))
            .await
            .expect("look for the results list")
            .is_empty(),
        "no results list is mounted at all"
    );
}

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
    capture_url_at_rest(browser).await;
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
        "the key opened {modals} modal(s) — a shortcut with no target must be a no-op. Shared by \
         AC-03.3 (`c` on a page with no project: no modal for an issue that has nowhere to go) and \
         AC-06.2/FR-9 (`Enter` with nothing selected: no modal for an issue Mei never chose). The \
         second reads on the board, where cards ARE present — so this reds for an `Enter` that \
         falls back to opening the first card when the ring is empty."
    );
}

/// The arm with teeth, shared by AC-03.3 (`c` with no project) and AC-07.2 (`Esc`
/// with nothing open). "No modal" alone would pass for an implementation that
/// RECONSTRUCTED the URL and did `location.href = ...`: there would be no modal on
/// the dashboard, because Mei would no longer be on the dashboard. That is the
/// failure this arm names — and it is the same failure for `Esc`, where the classic
/// bug is a shortcut that "goes back" when the stack is empty rather than popping
/// nothing.
///
/// Compared against the stamp `capture_url_at_rest` planted in the Given, NOT
/// against a URL this step recomputes: the two scenarios rest on different pages
/// and the claim ("Mei is where she was") is the same one. A destroyed stamp is
/// itself a navigation, so this reds even if the trip lands back on the same URL.
#[then(regex = r"^the browser does not navigate away$")]
async fn browser_does_not_navigate_away(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let actual = browser.current_url().await.expect("read the current URL");
    let stamped = browser
        .execute("return window.__kbUrlAtRest || null;", Vec::new())
        .await
        .expect("read back the resting-URL stamp");
    let expected = stamped.as_str().unwrap_or_else(|| {
        panic!(
            "the resting-URL stamp this scenario planted on the page is GONE and Mei is now at \
             {actual} — the document was replaced, which is exactly the navigation this step \
             forbids. A shortcut with nothing to act on is a no-op: never a navigation, never an \
             error (AC-03.3 / AC-07.2)."
        )
    });
    assert_eq!(
        actual.as_str().trim_end_matches('/'),
        expected.trim_end_matches('/'),
        "the key took Mei from {expected} to {actual}. A shortcut with no target — `c` with no \
         project, `Esc` with an empty layer stack — is a no-op (AC-03.3 / AC-07.2). This is what \
         reds if anyone reconstructs the new-issue URL instead of clicking the board's own shipped \
         trigger, or lets `Esc` fall through to a history-back default."
    );
}

// --- SLICE 03 / STEP 03-02 (US-07): the Esc layer stack (ADR-003) ------------
//
// The three scenarios below are where `Esc`'s LAYERED contract is exercised for
// the first time. The binding itself (`closeTopLayer()`) arrived at step 02-01 as
// a disclosed spillover — AC-02.6's Given required `Esc` to actually close the
// modal — so this step's contribution is the ASSERTIONS, and the layered scenario
// is the one that has never run. That is a legitimate outcome only because the
// assertions are FALSIFIABLE: collapsing `#kb-overlay-root` and `#modal-root` into
// one host, or making `Esc` clear both at once, reds the `@layered` scenario.
// Verified by doing it, not by assuming it (see the DELIVER report for 03-02).

/// AC-07.1, first arm. The DOM condition ADR-003 §2 makes canonical — the host is
/// empty — via a bounded wait, never a sleep. Then the WHOLE-document check, which
/// is the one that would catch a "close" that merely hid the modal or moved it out
/// of `#modal-root` while leaving it on the page.
#[then(regex = r"^the modal closes$")]
async fn the_modal_closes(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .wait()
        .at_most(std::time::Duration::from_secs(10))
        .for_element(Locator::Css("#modal-root:empty"))
        .await
        .expect(
            "pressing \"Esc\" must clear #modal-root — the new-issue modal is still on screen. \
             `Esc` reaches the dispatch layer from INSIDE the autofocused title field only because \
             guard 4's domain declines the keys a text field cannot consume (upstream-issues.md \
             UI-3): Escape produces no character and a text input does nothing with it. If that \
             narrowing is ever reverted, this is one of the steps that reds.",
        );
    let modals = any_modal_count(browser).await;
    assert_eq!(
        modals, 0,
        "#modal-root is empty but {modals} modal(s) are still in the document — the modal was \
         moved or hidden rather than closed, and Mei is still looking at a dialog"
    );
}

/// AC-07.1, second arm: "…with nothing else changed". Esc RESTORES — it does not
/// navigate, reload, or cost Mei the board state she had. The four seeded cards
/// are asserted BY NAME rather than by count: a board that re-rendered from a
/// reload would also show four cards, and the point of US-07 is that Mei's page is
/// the one she left.
#[then(regex = r"^Mei is back on the AUTH board with nothing else changed$")]
async fn back_on_board_nothing_changed(world: &mut FoundryWorld) {
    let expected = board_url(world);
    let browser = world.browser.as_ref().expect("browser session");
    let actual = browser.current_url().await.expect("read the current URL");
    assert_eq!(
        actual.as_str().trim_end_matches('/'),
        expected.trim_end_matches('/'),
        "\"Esc\" navigated Mei to {actual} instead of simply dismissing the modal — Esc must never \
         navigate (FR-5). Closing a dialog is not a trip through history."
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
    assert!(
        !help_overlay_is_open(browser).await,
        "\"Esc\" closed the modal but opened the help overlay — one press, one layer, and this \
         press had only the modal to peel (ADR-003 §2)"
    );
    let cards = board_issue_keys(browser).await;
    assert_eq!(
        cards,
        vec!["AUTH-1", "AUTH-2", "AUTH-3", "AUTH-4"],
        "the board came back carrying {cards:?} instead of the four cards Mei left on it. \
         Dismissing a modal must not cost her the board state underneath (AC-07.1)."
    );
}

/// The `@layered` proof's precondition (ADR-003's raison d'être), driven by two
/// REAL presses. A Given that injected the help fragment, or mounted the modal via
/// `execute()`, would be Fixture Theater of the purest kind: it would green the
/// layered assertion over a build where the two hosts do not exist at all.
///
/// The order is the order Mei lives: `c` opens the modal (which autofocuses its
/// title), then `?` opens the help OVER it. Both waits are the anti-vacuity guard
/// — if either layer is not genuinely up, the scenario must fail HERE, on its
/// premise, rather than pass an Esc assertion against a stack that was never two
/// deep.
#[given(
    regex = r#"^Mei has the new-issue modal open and has pressed "\?" to show the help overlay$"#
)]
async fn modal_open_with_help_over_it(world: &mut FoundryWorld) {
    open_new_issue_modal_by_pressing_c(world).await;
    let browser = world.browser.as_ref().expect("browser session");
    press_help_and_await_overlay(browser).await;
    assert_eq!(
        open_modal_count(browser).await,
        1,
        "pressing \"?\" over the open modal DESTROYED it — the help rendered into #modal-root \
         instead of into its own #kb-overlay-root host. This is precisely the single-shared-mount \
         design ADR-003 rejects: with one htmx swap target, help cannot sit OVER a modal, and the \
         layered Esc this scenario proves becomes unexpressible."
    );
}

/// THE assertion of the `@layered` scenario, and the reason ADR-003 exists. One
/// press peels ONE layer: the help is gone AND the modal beneath it is untouched.
///
/// This is the arm that reds if anyone collapses the two hosts back into one, or
/// "simplifies" `closeTopLayer()` into clearing both. Asserted as a DOM fact about
/// the modal's own host, plus displayed-ness — a modal still in `#modal-root` but
/// hidden would satisfy a presence-only check while Mei stares at a closed dialog.
#[then(regex = r"^the new-issue modal is still open$")]
async fn new_issue_modal_still_open(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let modal = browser
        .find_all(Locator::Css(MODAL_SELECTOR))
        .await
        .expect("look for the modal that must have SURVIVED the first Esc");
    assert_eq!(
        modal.len(),
        1,
        "the first \"Esc\" closed the help overlay AND took the new-issue modal with it. Esc peels \
         the TOPMOST layer, one per press (BR-4 / ADR-003 §2): Mei pressed once to dismiss the \
         help she had just opened, and lost the issue she was in the middle of writing. This is \
         what reds if #kb-overlay-root and #modal-root are collapsed into one host, or if \
         closeTopLayer() ever clears both."
    );
    assert!(
        modal[0].is_displayed().await.expect("modal displayed?"),
        "the new-issue modal is still in #modal-root but is no longer displayed — it survived the \
         first Esc in the DOM only, which is not what \"still open\" means to Mei"
    );
}

#[when(regex = r#"^Mei presses "Esc" a second time$"#)]
async fn mei_presses_esc_again(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .find(Locator::Css("body"))
        .await
        .expect("find the document body")
        .send_keys(browser_harness::key_chord("Esc"))
        .await
        .expect("press \"Esc\" a second time");
}

/// The `@layered` scenario's closing arm: the stack is peeled to the bottom, one
/// press at a time. Without this, "help closes and the modal stays" would be
/// satisfied by an `Esc` that can ONLY ever close help — the modal would outlive
/// every press, and Mei could never dismiss it at all.
#[then(regex = r"^the new-issue modal closes$")]
async fn new_issue_modal_closes(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .wait()
        .at_most(std::time::Duration::from_secs(10))
        .for_element(Locator::Css("#modal-root:empty"))
        .await
        .expect(
            "the SECOND \"Esc\" must clear #modal-root. With the help already peeled, the modal is \
             now the topmost layer — an Esc that only ever closes the overlay would leave Mei \
             unable to dismiss the modal by keyboard at all (ADR-003 §2).",
        );
}

/// AC-07.2's premise, ASSERTED rather than assumed: the stack really is empty. A
/// no-op scenario whose starting state secretly had a layer open would be testing
/// the ordinary close path and calling it a no-op.
#[given(regex = r"^Mei is viewing the AUTH board with no modal or overlay open$")]
async fn viewing_board_nothing_open(world: &mut FoundryWorld) {
    let url = board_url(world);
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .goto(&url)
        .await
        .expect("navigate to the AUTH board");
    browser_harness::wait_for_kb_ready(browser).await;
    assert_eq!(
        any_modal_count(browser).await,
        0,
        "the board loaded with a modal already open — this scenario's whole premise is that \"Esc\" \
         has NOTHING to close"
    );
    assert!(
        !help_overlay_is_open(browser).await,
        "the board loaded with the help overlay already open — see above: the stack must be empty \
         for this scenario to be about an empty stack"
    );
    capture_url_at_rest(browser).await;
    // The board Mei is looking at, stamped so "nothing happens" can be a
    // comparison against what was there rather than a list of things this step
    // remembered to check.
    browser
        .execute(
            "window.__kbBoardAtRest = document.querySelector('.board').innerHTML; return null;",
            Vec::new(),
        )
        .await
        .expect("stamp the board's contents");
}

/// AC-07.2 — "nothing happens", asserted as a DIFF rather than as a checklist.
///
/// A checklist ("no modal, no overlay, no selection") only catches the things this
/// step thought to name; the claim is that Mei's page is UNTOUCHED, so the board's
/// own markup before and after is the honest witness. It is what catches an `Esc`
/// that quietly cleared a card, collapsed a column, or dropped a highlight — the
/// mutations nobody would have listed in advance.
///
/// The `?` sentinel LAST, for the reason `no_modal_opens` documents: a negative
/// assertion needs the layer proven LIVE (an unbound layer also does nothing) and
/// needs the Esc press to have SETTLED. `closeTopLayer()` runs synchronously inside
/// the keydown handler, so had Esc done anything, it would have done it strictly
/// before the help fetch this blocks on. An ordering argument on one loopback
/// origin — not a sleep. The overlay it opens lands in `#kb-overlay-root`, which is
/// outside `.board` and so cannot disturb the diff above it.
#[then(regex = r"^nothing happens$")]
async fn nothing_happens(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let unchanged = browser
        .execute(
            "return window.__kbBoardAtRest === document.querySelector('.board').innerHTML;",
            Vec::new(),
        )
        .await
        .expect("compare the board against its resting contents");
    assert_eq!(
        unchanged.as_bool(),
        Some(true),
        "pressing \"Esc\" with nothing open CHANGED the board. An empty layer stack pops nothing — \
         Esc must never reach for a default when there is no layer to peel (AC-07.2). Mei pressed \
         a key that should have done nothing and the page under her moved."
    );
    assert_eq!(
        any_modal_count(browser).await,
        0,
        "pressing \"Esc\" with nothing open OPENED a modal"
    );
    press_help_and_await_overlay(browser).await;
    assert_eq!(
        any_modal_count(browser).await,
        0,
        "a modal appeared on the board after \"Esc\" — the press was a no-op only in the sense \
         that nobody was watching when it settled"
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

/// AC-05.1. "FIRST VISIBLE" is resolved from the RECTS (`visible_card_order`),
/// never from DOM order and never from the hidden `#kb-items` carrier — the
/// whole point of the slice is that selection follows the eyes.
#[then(regex = r"^the first visible card is highlighted as selected$")]
async fn first_visible_card_selected(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let order = visible_card_order(browser).await;
    assert_selected_is(browser, &order[0], "\"j\" on a board with no selection").await;
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

// --- SLICE 05 / STEP 05-01: j/k walk the VISIBLE cards (US-05, ADR-004) ------
//
// The one thing every step below is built to refuse: the hidden `#kb-items`
// carrier (`board.html:12`). It is ASC-by-number across all columns; the visible
// board is column-grouped and DESC-within-column (`projects.rs:864-879`) — a
// DIFFERENT order. Every "which card" question here is answered from
// `visible_card_order`'s RECTS, so an implementation that walked the carrier reds
// on the very first press rather than passing for the wrong reason (ADR-008).

/// AC-05.1's Given. Names three of the four seeded cards; asserts they are on
/// screen rather than trusting the Background, because "first VISIBLE card" is
/// meaningless if nothing is visible.
#[given(regex = r"^Mei is viewing the AUTH board showing issues AUTH-3, AUTH-2 and AUTH-1$")]
async fn viewing_board_showing_cards(world: &mut FoundryWorld) {
    let url = board_url(world);
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .goto(&url)
        .await
        .expect("navigate to the AUTH board");
    browser_harness::wait_for_kb_ready(browser).await;
    let order = visible_card_order(browser).await;
    for key in ["AUTH-3", "AUTH-2", "AUTH-1"] {
        assert!(
            order.iter().any(|k| k == key),
            "the board is not showing {key} (it shows {order:?}) — this scenario asserts which \
             card `j` lands on, so the cards it names must be on screen"
        );
    }
    assert!(
        selected_keys(browser).await.is_empty(),
        "a card is already selected on a freshly-loaded board — selection is ephemeral and starts \
         empty (BR-5), and `j`'s first press would then be asserting the wrong thing"
    );
}

/// The Given for AC-05.2 and AC-05.4: selection is established by pressing the
/// REAL `j` and is ASSERTED, never mounted from JS. A Given that stamped
/// `aria-selected` onto a card itself would be Fixture Theater — it would green
/// `k`'s boundary over a build where `j` is not bound at all.
async fn select_first_visible_card(world: &mut FoundryWorld) {
    let url = board_url(world);
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .goto(&url)
        .await
        .expect("navigate to the AUTH board");
    browser_harness::wait_for_kb_ready(browser).await;
    press(browser, "j").await;
    let order = visible_card_order(browser).await;
    assert_selected_is(browser, &order[0], "the Given's own \"j\"").await;
}

#[given(regex = r"^Mei has selected the first visible card on the AUTH board$")]
async fn has_selected_first_visible_card(world: &mut FoundryWorld) {
    select_first_visible_card(world).await;
}

#[given(regex = r"^Mei has the first visible card selected$")]
async fn has_first_visible_card_selected(world: &mut FoundryWorld) {
    select_first_visible_card(world).await;
    let browser = world.browser.as_ref().expect("browser session");
    install_error_probe(browser).await;
}

/// One real keystroke at `body`, through the same document-delegated listener a
/// human's press reaches (ADR-001).
async fn press(browser: &fantoccini::Client, key: &str) {
    press_at(browser, "body", key).await;
}

/// One real keystroke delivered AT `selector`.
///
/// WebDriver's Element Send Keys focuses its target before typing, so the target
/// is not cosmetic — it is "where the user's focus is when they press the key".
/// `press()` types at `body`, which is right for a sighted keyboard user (no focus
/// prerequisite; the document-delegated listener fires immediately) and WRONG for
/// the ADR-006 composite: typing at `body` would BLUR the board this scenario just
/// Tabbed to, and `aria-activedescendant` announces only from the focused
/// composite. That would be the test destroying the very precondition it asserts.
async fn press_at(browser: &fantoccini::Client, selector: &str, key: &str) {
    browser
        .find(Locator::Css(selector))
        .await
        .unwrap_or_else(|err| panic!("find {selector:?} to press {key:?} at it: {err}"))
        .send_keys(browser_harness::key_chord(key))
        .await
        .unwrap_or_else(|err| panic!("press {key:?} at {selector:?}: {err}"));
}

/// Records where the ring is RIGHT NOW onto the page, building the walk the Then
/// reads back. Stamped on `window` (the house idiom, as `capture_url_at_rest`):
/// a navigation would destroy it, so a `j` that navigated cannot quietly satisfy
/// a walk assertion.
async fn record_selection_step(browser: &fantoccini::Client) {
    browser
        .execute(
            r#"window.__kbSelectionWalk = window.__kbSelectionWalk || [];
               var el = document.querySelector(".board .issue-card[aria-selected='true']");
               window.__kbSelectionWalk.push(el ? el.getAttribute('data-issue-key') : null);
               return null;"#,
            Vec::new(),
        )
        .await
        .expect("record where the selection landed");
}

async fn selection_walk(browser: &fantoccini::Client) -> Vec<String> {
    let walk = browser
        .execute(
            "if (!window.__kbSelectionWalk) { throw new Error('no selection walk was recorded'); }
             return window.__kbSelectionWalk.map(function (k) { return k === null ? '<none>' : k; });",
            Vec::new(),
        )
        .await
        .expect("read the recorded selection walk");
    json_strings(&walk, "the selection walk")
}

#[when(regex = r#"^Mei presses "j" and then "k"$"#)]
async fn presses_j_then_k(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    press(browser, "j").await;
    record_selection_step(browser).await;
    press(browser, "k").await;
    record_selection_step(browser).await;
}

/// AC-05.2. Both halves of the move, against the ORDER MEI SEES.
#[then(regex = r"^the selection moves to the second visible card and back to the first$")]
async fn selection_moves_second_then_first(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let order = visible_card_order(browser).await;
    assert!(
        order.len() >= 2,
        "the board shows {} card(s) — a scenario about moving to the SECOND card needs two",
        order.len()
    );
    let walk = selection_walk(browser).await;
    assert_eq!(
        walk,
        vec![order[1].clone(), order[0].clone()],
        "`j` then `k` walked {walk:?}, but the cards Mei sees are {order:?} — `j` must move to the \
         next card ON SCREEN and `k` must bring the ring straight back (AC-05.2)"
    );
}

/// The `@kb-items-collision` arm, and the reason this scenario exists at all.
///
/// The board renders TWO orders: the visible, column-grouped DESC one Mei reads,
/// and the hidden `#kb-items` ASC-by-number carrier whose own shipped comment
/// once claimed to be "the source of truth for the keyboard navigation order"
/// (`projects.rs:881-885`). They DISAGREE. This step first proves they disagree —
/// otherwise it could not tell the two implementations apart and would be
/// asserting nothing — and then proves `j` followed the eyes.
///
/// The carrier arm SELF-RETIRES: step 05-05 deletes `#kb-items` (ADR-008), after
/// which `carrier_order` is empty and only the geometry arm below runs. That arm
/// is the permanent one; the carrier arm is a guard for exactly as long as there
/// is a wrong list on the page to be tempted by.
#[then(regex = r"^the selection order matches the order the cards appear on screen$")]
async fn selection_order_matches_screen(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let order = visible_card_order(browser).await;
    let walk = selection_walk(browser).await;
    let carrier = carrier_order(browser).await;
    if !carrier.is_empty() {
        assert_ne!(
            carrier, order,
            "the hidden #kb-items carrier and the visible board are rendering the SAME order — \
             this scenario exists to prove `j` walks what Mei sees rather than the carrier, and it \
             cannot distinguish them while they agree. Re-seed the board so the two orders differ."
        );
        assert_ne!(
            walk[0], carrier[1],
            "`j` moved to {}, which is the hidden #kb-items carrier's second entry — not the \
             second card on screen ({}). Selection must be built from the VISIBLE cards; the \
             carrier is `hidden aria-hidden` ASC-by-number and can carry no ring (ADR-008).",
            walk[0], order[1]
        );
    }
    assert_eq!(
        walk[0], order[1],
        "`j` moved to {} but the card Mei sees below the first one is {} — the walk order must be \
         the on-screen order (rects), which is column-grouped and DESC-within-column (AC-05.1)",
        walk[0], order[1]
    );
}

/// AC-05.3's Given. Seeds enough cards to overflow the FIXED 1280x900 viewport
/// (ADR-007 pins it, which is the only reason this scenario is deterministic) and
/// then ASSERTS the overflow: a board that happens to fit would make the
/// scrollIntoView assertion pass without anything ever scrolling.
#[given(regex = r"^Mei is viewing the AUTH board with more cards than fit on screen$")]
async fn board_with_more_cards_than_fit(world: &mut FoundryWorld) {
    seed_extra_backlog_issues(world, 5..=40).await;
    let url = board_url(world);
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .goto(&url)
        .await
        .expect("navigate to the AUTH board");
    browser_harness::wait_for_kb_ready(browser).await;
    let below = cards_below_the_fold(browser).await;
    assert!(
        below > 0,
        "every card on the board fits inside the viewport, so nothing can be \"below the fold\" — \
         this scenario would assert scrollIntoView over a page that never needs to scroll. Seed \
         more cards, or the fixed window (browser_harness.rs) grew."
    );
}

/// Seeds extra backlog issues so the board overflows. Same INSERT + counter
/// repair as the Background's own seeding: `next_issue_number` is advanced
/// because a direct INSERT bypasses its only writer, and a board whose next
/// create would 500 is a database the app can never produce.
async fn seed_extra_backlog_issues(
    world: &mut FoundryWorld,
    numbers: std::ops::RangeInclusive<i32>,
) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let project: (uuid::Uuid, uuid::Uuid) =
        sqlx::query_as("SELECT id, workspace_id FROM projects WHERE key_prefix = 'AUTH'")
            .fetch_one(pool)
            .await
            .expect("fetch the AUTH project");
    let author: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("fetch author");
    let last = *numbers.end();
    for number in numbers {
        sqlx::query(
            "INSERT INTO issues (id, workspace_id, project_id, number, title, state, priority, author_id)
                  VALUES ($1, $2, $3, $4, $5, 'backlog', 'medium', $6)",
        )
        .bind(uuid::Uuid::now_v7())
        .bind(project.1)
        .bind(project.0)
        .bind(number)
        .bind(format!("Seeded overflow issue {number}"))
        .bind(author.0)
        .execute(pool)
        .await
        .expect("seed an overflow AUTH issue");
    }
    sqlx::query(
        "UPDATE projects SET next_issue_number = GREATEST(next_issue_number, $2) WHERE id = $1",
    )
    .bind(project.0)
    .bind(last + 1)
    .execute(pool)
    .await
    .expect("advance next_issue_number past the overflow issues");
}

/// How many cards are NOT fully inside the viewport right now.
async fn cards_below_the_fold(browser: &fantoccini::Client) -> usize {
    let count = browser
        .execute(
            "return Array.prototype.filter.call(
               document.querySelectorAll('.board .issue-card[data-issue-key]'),
               function (card) { return card.getBoundingClientRect().bottom > window.innerHeight; }
             ).length;",
            Vec::new(),
        )
        .await
        .expect("count the cards below the fold");
    count.as_u64().expect("a count") as usize
}

/// AC-05.3. "Repeatedly until the selection passes the bottom of the viewport" is
/// made exact: count the cards FULLY VISIBLE before any press (N), then press `j`
/// N+1 times. The (N+1)th card in on-screen order is, by construction, one that
/// was NOT on screen when Mei started — so the Then's claim ("it is on screen
/// now") can only be satisfied by something having scrolled.
///
/// The count is taken BEFORE the first press for the obvious reason: each press
/// may scroll, and a count taken later would be a count of a page the assertion
/// itself moved.
#[when(
    regex = r#"^Mei presses "j" repeatedly until the selection passes the bottom of the viewport$"#
)]
async fn presses_j_past_the_fold(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let order = visible_card_order(browser).await;
    let fully_visible = browser
        .execute(
            "return Array.prototype.filter.call(
               document.querySelectorAll('.board .issue-card[data-issue-key]'),
               function (card) {
                 var r = card.getBoundingClientRect();
                 return r.top >= 0 && r.bottom <= window.innerHeight;
               }
             ).length;",
            Vec::new(),
        )
        .await
        .expect("count the cards on screen before any press")
        .as_u64()
        .expect("a count") as usize;
    assert!(
        fully_visible < order.len(),
        "all {} cards are already on screen — there is no card to walk PAST the fold to",
        order.len()
    );
    let target = order[fully_visible].clone();
    for _ in 0..=fully_visible {
        press(browser, "j").await;
    }
    browser
        .execute(
            "window.__kbScrollTarget = arguments[0]; return null;",
            vec![serde_json::Value::String(target)],
        )
        .await
        .expect("stamp the below-the-fold target");
}

/// AC-05.3 + NFR-7. Three claims, each falsifiable on its own:
///   1. the ring is on the card that was BELOW THE FOLD (so `j` really walked there);
///   2. that card is now fully inside the viewport (so something SCROLLED — drop
///      `scrollIntoView` from keyboard.js and this reds while 1 and 3 still pass);
///   3. the highlight is actually rendered AND is not colour alone — a non-zero
///      outline, which is what survives a high-contrast mode and a colour-blind
///      reader (NFR-7). A ring implemented as `background: blue` reds here.
#[then(regex = r"^the selected card is scrolled into view and its highlight is visible$")]
async fn selected_card_scrolled_into_view(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let target = browser
        .execute("return window.__kbScrollTarget || null;", Vec::new())
        .await
        .expect("read the below-the-fold target")
        .as_str()
        .expect("the When stamped a target key")
        .to_string();
    assert_selected_is(
        browser,
        &target,
        "walking \"j\" past the bottom of the viewport",
    )
    .await;

    let report = browser
        .execute(
            r#"var card = document.querySelector(".board .issue-card[aria-selected='true']");
               if (!card) { throw new Error('nothing is selected'); }
               var r = card.getBoundingClientRect();
               var style = window.getComputedStyle(card);
               return {
                 top: Math.round(r.top),
                 bottom: Math.round(r.bottom),
                 viewport: window.innerHeight,
                 displayed: style.visibility !== 'hidden' && style.display !== 'none',
                 outline: parseFloat(style.outlineWidth) || 0,
                 outlineStyle: style.outlineStyle
               };"#,
            Vec::new(),
        )
        .await
        .expect("measure the selected card");
    let top = report["top"].as_f64().expect("top");
    let bottom = report["bottom"].as_f64().expect("bottom");
    let viewport = report["viewport"].as_f64().expect("viewport");
    assert!(
        top >= 0.0 && bottom <= viewport,
        "the selected card {target} sits at {top}..{bottom} in a {viewport}px viewport — it is \
         still off screen, so Mei is looking at a ring she cannot see. The selection must be \
         scrolled into view as it moves (AC-05.3)"
    );
    assert!(
        report["displayed"].as_bool().unwrap_or(false),
        "the selected card {target} is not being displayed at all"
    );
    let outline = report["outline"].as_f64().unwrap_or(0.0);
    let outline_style = report["outlineStyle"].as_str().unwrap_or("none");
    assert!(
        outline > 0.0 && outline_style != "none",
        "the selected card's highlight has no outline (outline-width {outline}px, style \
         {outline_style}) — a ring drawn with colour alone is invisible to a colour-blind reader \
         and in forced-colours mode (NFR-7)"
    );
}

/// AC-05.4. `k` at the top is BOUNDED, not a wrap: the ring stays exactly where it
/// was. Asserted against the on-screen first card, so a `k` that wrapped to the
/// LAST card reds rather than "still selected something".
#[then(regex = r"^the first card remains selected$")]
async fn first_card_remains_selected(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let order = visible_card_order(browser).await;
    assert_selected_is(
        browser,
        &order[0],
        "\"k\" with the first card already selected",
    )
    .await;
}

/// The other half of AC-05.4 — and the half that is easy to write as prose and
/// never assert. The boundary is where an index-walk throws (`items[-1]` is
/// undefined, and `.classList` on it is a TypeError), so "no error" is a real
/// claim about a real failure, read from the probe the Given installed.
#[then(regex = r"^no error occurs$")]
async fn no_error_occurs(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let errors = browser
        .execute(
            "if (!window.__kbErrors) { throw new Error('no error probe was installed'); }
             return window.__kbErrors;",
            Vec::new(),
        )
        .await
        .expect("read the page-error probe");
    let errors = json_strings(&errors, "the page errors");
    assert!(
        errors.is_empty(),
        "pressing \"k\" at the first card raised {errors:?} — the boundary must be a quiet no-op \
         (FR-8), not an exception Mei never sees but which kills every later press"
    );
}

// --- SLICE 05 / STEP 05-02: drag coexistence + a11y (AC-05.8, AC-05.7) -------

/// The board region itself (`board.html:7`). ADR-006 makes THIS the focusable
/// ARIA composite: one `tabindex="0"` on ONE container, which is what keeps D-4's
/// rejection of roving tabindex intact (it adds one tab stop, not N).
const BOARD_SELECTOR: &str = ".board";

/// Presses `j` AT `from` until the ring is on `key`. Every press is a REAL
/// keystroke through the document-delegated listener — the selection is never
/// mounted from JS, which would be Fixture Theater (it would green these
/// scenarios over a build where `j` is unbound entirely).
///
/// The count is DERIVED from where `key` sits in the on-screen order, so this
/// helper cannot mask a `j` that moves by the wrong step: press N+1 times and the
/// ring must be on the (N+1)th card Mei sees, or the assertion below reds.
async fn select_by_pressing_j(browser: &fantoccini::Client, from: &str, key: &str) {
    let order = visible_card_order(browser).await;
    let index = order.iter().position(|k| k == key).unwrap_or_else(|| {
        panic!(
            "{key} is not on the board (it shows {order:?}) — this scenario asserts what happens to \
             {key}'s ring, so {key} must be a card Mei can see"
        )
    });
    for _ in 0..=index {
        press_at(browser, from, "j").await;
    }
    assert_selected_is(
        browser,
        key,
        format!("{} press(es) of \"j\"", index + 1).as_str(),
    )
    .await;
}

/// AC-05.8's Given. The ring is put on AUTH-2 by pressing the real `j`.
#[given(regex = r"^Mei has selected AUTH-2 on the AUTH board$")]
async fn has_selected_auth2(world: &mut FoundryWorld) {
    let url = board_url(world);
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .goto(&url)
        .await
        .expect("navigate to the AUTH board");
    browser_harness::wait_for_kb_ready(browser).await;
    // At `body`: Hiroshi is a pointer user and Mei is a sighted keyboard user —
    // neither has Tabbed anywhere, and the document-delegated listener needs no
    // focus prerequisite. That is the ADR-006 asymmetry this scenario relies on.
    select_by_pressing_j(browser, "body", "AUTH-2").await;
    // AUTH-2's on-screen position BEFORE the drag — the "old slot" the Then reads
    // back. Stamped on `window` (the house idiom, as `__kbScrollTarget`): a
    // navigation destroys it, so a drag that reloaded the page cannot quietly
    // satisfy the assertion.
    let order = visible_card_order(browser).await;
    let index = order
        .iter()
        .position(|k| k == "AUTH-2")
        .expect("AUTH-2 is on the board");
    browser
        .execute(
            "window.__kbSlotBeforeDrag = arguments[0]; return null;",
            vec![serde_json::Value::from(index as u64)],
        )
        .await
        .expect("stamp AUTH-2's slot before the drag");
}

/// AC-05.8's When — and the honest limit it carries.
///
/// `board-dnd.js` implements NATIVE HTML5 drag-and-drop (`dragstart` / `dragover`
/// / `drop`). WebDriver's pointer actions do NOT synthesise the native drag
/// protocol in Chrome — a mouse-down/move/up sequence produces a text selection,
/// never a `dragstart` — so the gesture is DISPATCHED: a real `DragEvent` with a
/// real `DataTransfer` at each of the three stages, on the real elements, into
/// `board-dnd.js`'s own real listeners. Same limit, same disclosure discipline as
/// the `@ime` scenario (ADR-007 honest limit 1): the handlers under test run for
/// real; only the input substrate is simulated. `board-dnd.js` is NOT modified —
/// this step reaches it exactly as the browser would.
///
/// The drop lands in the FIRST column that is not AUTH-2's own, and the `clientY`
/// is the column's own top, so `insertBeforeTarget` resolves against the real
/// geometry rather than a coordinate this step invented.
#[when(regex = r"^Hiroshi drags AUTH-2 into another column with the mouse$")]
async fn hiroshi_drags_auth2_to_another_column(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let moved_into = browser
        .execute(
            r#"var card = document.querySelector(".board .issue-card[data-issue-key='AUTH-2']");
               if (!card) { throw new Error('AUTH-2 is not on the board'); }
               var from = card.closest('[data-column]');
               var into = null;
               var columns = document.querySelectorAll('.board [data-column]');
               for (var i = 0; i < columns.length; i++) {
                 if (columns[i] !== from) { into = columns[i]; break; }
               }
               if (!into) { throw new Error('the board renders only one column — nothing to drag INTO'); }
               var transfer = new DataTransfer();
               function fire(target, type, extra) {
                 var event = new DragEvent(type, Object.assign({
                   bubbles: true, cancelable: true, composed: true, dataTransfer: transfer
                 }, extra || {}));
                 target.dispatchEvent(event);
                 return event;
               }
               var rect = into.getBoundingClientRect();
               fire(card, 'dragstart');
               fire(into, 'dragover', { clientY: rect.top });
               fire(into, 'drop', { clientY: rect.top });
               fire(card, 'dragend');
               window.__kbDraggedInto = into.getAttribute('data-column');
               return window.__kbDraggedInto;"#,
            Vec::new(),
        )
        .await
        .expect("drag AUTH-2 into another column")
        .as_str()
        .expect("the column AUTH-2 was dragged into")
        .to_string();
    assert!(
        !moved_into.is_empty(),
        "the drag reported no destination column"
    );
}

/// NFR-8 — the drag must work exactly as it does today, with the keyboard layer
/// loaded beside it. Both halves of "completes" are asserted:
///   1. the card is now in the target column IN THE DOM (the optimistic move), and
///   2. the state POST reached the server and stuck — read from the DATABASE, not
///      from the page. A drag that moved the card optimistically and then had its
///      POST rejected would revert, and a DOM-only assertion could race that
///      revert and pass over a broken drag.
/// The page is deliberately NOT reloaded: selection is ephemeral (BR-5), and a
/// reload would destroy the very ring the next step exists to assert.
#[then(regex = r"^the drag completes as it does today$")]
async fn drag_completes_as_today(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let expected = browser
        .execute(
            "if (!window.__kbDraggedInto) { throw new Error('no drag was recorded'); }
             return window.__kbDraggedInto;",
            Vec::new(),
        )
        .await
        .expect("read the column AUTH-2 was dragged into")
        .as_str()
        .expect("a column slug")
        .to_string();
    let landed = browser
        .execute(
            r#"var card = document.querySelector(".board .issue-card[data-issue-key='AUTH-2']");
               if (!card) { return '<AUTH-2 left the board entirely>'; }
               var column = card.closest('[data-column]');
               return column ? column.getAttribute('data-column') : '<no column>';"#,
            Vec::new(),
        )
        .await
        .expect("read the column AUTH-2 now sits in")
        .as_str()
        .expect("a column slug")
        .to_string();
    assert_eq!(
        landed, expected,
        "AUTH-2 was dragged into the {expected:?} column but the board shows it in {landed:?} — the \
         drag itself must keep working unchanged with the keyboard layer loaded beside it (NFR-8)"
    );

    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let stored = loop {
        let stored: (String,) = sqlx::query_as(
            "SELECT state FROM issues
              WHERE number = 2
                AND project_id = (SELECT id FROM projects WHERE key_prefix = 'AUTH')",
        )
        .fetch_one(pool)
        .await
        .expect("read AUTH-2's stored state");
        if stored.0 == expected || std::time::Instant::now() >= deadline {
            break stored.0;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    };
    assert_eq!(
        stored, expected,
        "the board shows AUTH-2 in {expected:?} but the server still has it in {stored:?} — the \
         drop's state POST (board-dnd.js) never landed, so the move is an optimistic lie that the \
         next reload undoes (NFR-8)"
    );
}

/// THE INDEX-VS-KEY PROOF (AC-05.8, ADR-004) — the scenario the whole selection
/// model is shaped by, and the one that must RED the moment anyone stores an index.
///
/// The third arm below is the one that does that work, and it is here because the
/// obvious two arms DO NOT: verified by falsification at step 05-02, not assumed.
/// Porting `keyboard.js` to a stored index and running arms 1-2 alone left this
/// scenario GREEN. The reason is worth writing down, because it is exactly the
/// kind of thing a scenario passes over:
///
/// **`aria-selected` rides the NODE.** The drag MOVES AUTH-2's element rather than
/// re-rendering it, and nothing re-projects the ring afterwards — so the attribute
/// stays on AUTH-2 whatever the model stores. A stale index is not visible in the
/// ring at all. It is visible on the NEXT PRESS, which is precisely ADR-004's
/// stated harm: the index "silently re-points at a DIFFERENT issue while Mei is
/// looking away, and `Enter` then opens the wrong one". Silent is the operative
/// word — an assertion that only reads the ring cannot see it.
///
/// So "coherent" (this scenario's own title) is asserted as what it means: the
/// selection still IDENTIFIES AUTH-2, not merely that a leftover attribute sits on
/// it. Three arms:
///   1. the ring is on AUTH-2 — the NODE it started on, wherever that node now is;
///   2. the card that now occupies AUTH-2's OLD on-screen position is a DIFFERENT
///      card, and is not ringed (without this, arm 1 could pass over a board where
///      nothing moved, and the scenario would be asserting nothing at all);
///   3. the model still resolves to AUTH-2: `k` then `j` walks off the selection
///      and back onto it. Key-based, `k` lands on the card now before AUTH-2 and
///      `j` returns. Index-based, the stale slot walks from the WRONG place and
///      the ring comes back on someone else. This is the arm that reds.
#[allow(clippy::too_many_lines)]
#[then(regex = r"^the ring is still on AUTH-2, not on whatever now occupies the old slot$")]
async fn ring_still_on_auth2(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let index_before = browser
        .execute(
            "if (window.__kbSlotBeforeDrag === undefined) {
               throw new Error('the Given never recorded AUTH-2 slot');
             }
             return window.__kbSlotBeforeDrag;",
            Vec::new(),
        )
        .await
        .expect("read AUTH-2's slot from before the drag")
        .as_u64()
        .expect("a slot index") as usize;
    let order = visible_card_order(browser).await;
    let occupant = order
        .get(index_before)
        .unwrap_or_else(|| panic!("the board shrank to {order:?} after the drag"))
        .clone();
    assert_ne!(
        occupant, "AUTH-2",
        "AUTH-2 is still sitting at its own old on-screen position ({index_before}) after the drag \
         — nothing moved, so this scenario cannot tell a key-based selection from an index-based \
         one and is asserting nothing. Re-check the drag (board-dnd.js) or the seeded columns."
    );
    assert_selected_is(
        browser,
        "AUTH-2",
        &format!(
            "dragging AUTH-2 into another column (the card now at its old position {index_before} \
             is {occupant})"
        ),
    )
    .await;

    // Arm 3 — COHERENCE. See this step's own note: arms 1-2 are green under a
    // stored index, because the ring rides the dragged node. The model is
    // interrogated the only way a user can interrogate it — by pressing a key —
    // and the oracle is computed from the board's CURRENT on-screen order, which
    // is the order Mei's eyes now report.
    let now_at = order
        .iter()
        .position(|k| k == "AUTH-2")
        .expect("AUTH-2 is still on the board");
    assert!(
        now_at > 0,
        "the drag left AUTH-2 first on screen, so `k` has nowhere to step back to and this arm \
         cannot distinguish a key from an index — re-check which column the drag targeted"
    );
    let neighbour = order[now_at - 1].clone();
    press(browser, "k").await;
    assert_selected_is(
        browser,
        &neighbour,
        &format!(
            "\"k\" after the drag (AUTH-2 now sits at on-screen position {now_at}, so `k` must step \
             to {neighbour} — the card Mei sees immediately before it. A selection that stored \
             AUTH-2's OLD slot ({index_before}) steps from where AUTH-2 USED to be and lands \
             somewhere else entirely, which is ADR-004's whole case against an index)"
        ),
    )
    .await;
    press(browser, "j").await;
    assert_selected_is(
        browser,
        "AUTH-2",
        "\"k\" and then \"j\" after the drag (the ring must walk off AUTH-2 and back onto it — the \
         selection identifies the ISSUE by its key, so it survives the card moving; an index \
         identifies a SLOT, and the slot now holds a different issue)",
    )
    .await;
}

/// AC-05.7's Given — the ADR-006 accepted cost, made executable.
///
/// A screen-reader user in browse mode must Tab to the board ONCE before `j`/`k`
/// arrive at all; this step is that Tab. It presses the real `Tab` key from the
/// top of the document until DOM focus lands on the board, and asserts it got
/// there — so this Given REDS while the board is not a focusable composite, which
/// is exactly what ADR-006 asks slice 05 to build. It never calls `.focus()`:
/// focusing the board from JS would green the scenario over a board no Tab can
/// ever reach, which is the accessibility failure the ADR is about.
#[given(regex = r"^Mei has Tabbed to focus the AUTH board as a screen-reader user would$")]
async fn tabbed_to_focus_the_board(world: &mut FoundryWorld) {
    let url = board_url(world);
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .goto(&url)
        .await
        .expect("navigate to the AUTH board");
    browser_harness::wait_for_kb_ready(browser).await;
    let body = browser
        .find(Locator::Css("body"))
        .await
        .expect("find the document body");
    let mut focused = String::new();
    for _ in 0..40 {
        body.send_keys(browser_harness::key_chord("Tab"))
            .await
            .expect("press Tab");
        focused = browser
            .execute(
                r#"var el = document.activeElement;
                   if (!el) { return '<none>'; }
                   return el.matches('.board') ? 'board' : (el.tagName.toLowerCase() +
                     (el.id ? '#' + el.id : ''));"#,
                Vec::new(),
            )
            .await
            .expect("read what has focus")
            .as_str()
            .expect("a description of the focused element")
            .to_string();
        if focused == "board" {
            return;
        }
    }
    panic!(
        "40 presses of Tab never landed focus on the board (focus is on {focused:?} instead). An \
         AT user reaches `j`/`k` ONLY through focus mode, which their screen reader enters when \
         focus lands on a composite widget — so a board with no tab stop is a board whose \
         selection keys never arrive at all (ADR-006). The board must be a focusable composite: \
         `tabindex=\"0\"` plus a composite role."
    );
}

/// AC-05.7's When. `j` moves the ring to AUTH-2 by real presses, delivered AT the
/// board Mei just Tabbed to — because that is the whole point of ADR-006: the
/// keys arrive only once the composite has focus, and the press must therefore
/// come from inside it. Typing at `body` here would blur the board and assert a
/// path no AT user takes.
#[when(regex = r#"^Mei presses "j" to move the selection to AUTH-2$"#)]
async fn presses_j_to_select_auth2(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    select_by_pressing_j(browser, BOARD_SELECTOR, "AUTH-2").await;
}

/// AC-05.7 / ADR-006's core mechanism. `aria-activedescendant` is read from the
/// BOARD (the composite that has focus) and must point at AUTH-2's shipped
/// `id="issue-AUTH-2"` (`issue_card.html:1`) — the id is the ADR's whole reason
/// for choosing this mechanism, so the assertion names it rather than any
/// client-invented attribute.
///
/// The board's ROLE is asserted beside it, because `aria-activedescendant` on an
/// element with no composite role announces NOTHING: AT resolves the active
/// descendant only for a widget that owns options. An attribute that is present
/// and inert is the failure this scenario exists to prevent, and it is
/// indistinguishable from success unless the role is checked too.
#[then(regex = r"^the board's active descendant is the AUTH-2 card$")]
async fn board_active_descendant_is_auth2(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let report = browser
        .execute(
            r#"var board = document.querySelector(arguments[0]);
               if (!board) { throw new Error('the board region is not on the page'); }
               var active = board.getAttribute('aria-activedescendant');
               return {
                 active: active,
                 role: board.getAttribute('role'),
                 focused: document.activeElement === board,
                 target: active ? (document.getElementById(active)
                   ? document.getElementById(active).getAttribute('data-issue-key') : '<no such id>')
                   : '<unset>'
               };"#,
            vec![BOARD_SELECTOR.into()],
        )
        .await
        .expect("read the board's active descendant");
    assert!(
        report["focused"].as_bool().unwrap_or(false),
        "the board lost DOM focus during the presses — `aria-activedescendant` announces only from \
         the focused composite, so a board that cannot HOLD focus while `j` fires exposes nothing \
         (ADR-006)"
    );
    let role = report["role"].as_str().unwrap_or("<unset>");
    assert_eq!(
        role, "listbox",
        "the board's role is {role:?}. `aria-activedescendant` is resolved by assistive technology \
         only on a composite that owns options — on a plain `div` the attribute is present and \
         inert, which reviews as accessible and is silent in use. ADR-006 chooses `listbox` \
         specifically (not `grid`: `j`/`k` are one linear sequence with no 2D arrow navigation)"
    );
    let target = report["target"].as_str().unwrap_or("<unset>");
    assert_eq!(
        target,
        "AUTH-2",
        "the board's aria-activedescendant is {:?}, which resolves to {target:?} — it must be \
         AUTH-2's own card `id=\"issue-AUTH-2\"` (issue_card.html:1). That shipped id is the \
         prerequisite ADR-006 picked this mechanism for; pointing it anywhere else announces the \
         wrong issue on every move (AC-05.7)",
        report["active"].as_str().unwrap_or("<unset>")
    );
}

/// `aria-selected` is STATE, not just an announcement (ADR-006): a screen-reader
/// user can query what is selected, not only hear it change once. Asserted as
/// EXACTLY-ONE — a projection that adds the state without clearing it elsewhere
/// leaves AT reading two selected options out of one selection.
#[then(regex = r"^the AUTH-2 card is marked selected for assistive technology$")]
async fn auth2_marked_selected_for_at(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    assert_selected_is(browser, "AUTH-2", "\"j\" moving the selection to AUTH-2").await;
    let role = browser
        .execute(
            r#"var card = document.querySelector(".board .issue-card[data-issue-key='AUTH-2']");
               if (!card) { throw new Error('AUTH-2 is not on the board'); }
               return card.getAttribute('role') || '<unset>';"#,
            Vec::new(),
        )
        .await
        .expect("read AUTH-2's role")
        .as_str()
        .expect("a role")
        .to_string();
    assert_eq!(
        role, "option",
        "AUTH-2's card has role {role:?}. `aria-selected` is only meaningful on an option inside \
         the listbox — on an `article` it is an attribute AT ignores, so the selection would be \
         marked in the DOM and absent from the accessibility tree (ADR-006)"
    );
}

/// NFR-7. The ring must be a SHAPE change, not a colour change: read the computed
/// outline off the live selected card. `background: blue` — the tempting one-liner
/// — reds here, and so does a ring that is only a colour swap, because neither
/// survives forced-colours mode or a colour-blind reader.
#[then(regex = r"^the selection highlight does not rely on colour alone$")]
async fn highlight_not_colour_alone(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let report = browser
        .execute(
            r#"var card = document.querySelector(".board .issue-card[aria-selected='true']");
               if (!card) { throw new Error('nothing is selected'); }
               var style = window.getComputedStyle(card);
               return {
                 width: parseFloat(style.outlineWidth) || 0,
                 style: style.outlineStyle
               };"#,
            Vec::new(),
        )
        .await
        .expect("measure the selected card's ring");
    let width = report["width"].as_f64().unwrap_or(0.0);
    let outline_style = report["style"].as_str().unwrap_or("none");
    assert!(
        width > 0.0 && outline_style != "none",
        "the selected card's ring is outline-width {width}px / outline-style {outline_style} — a \
         highlight drawn with colour alone disappears in forced-colours mode and is invisible to a \
         colour-blind reader (NFR-7). The ring must be a shape change: `outline` + `outline-offset`"
    );
}

/// The ADR-006 RATIFICATION's own obligation on slice 05, made a test rather than
/// a promise.
///
/// KPI-4 holds **conditionally** — "once the board is focused". The user ratified
/// Option A on the condition that the qualifier travels with the claim, and that
/// the instruction reaches the USER through the help overlay's own copy, not just
/// the ADR. This step is the only thing standing between that condition and being
/// quietly dropped: an accepted cost nobody is told about is an undocumented bug.
#[then(regex = r#"^the help overlay tells the user to Tab to the board, then press "j" or "k"$"#)]
async fn help_overlay_documents_the_tab(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    press(browser, "?").await;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let copy = loop {
        let text = browser
            .execute(
                "var help = document.querySelector('#kb-overlay-root .keyboard-help');
                 return help ? help.textContent : '';",
                Vec::new(),
            )
            .await
            .expect("read the help overlay's copy")
            .as_str()
            .unwrap_or_default()
            .to_string();
        let lowered = text.to_lowercase();
        if (lowered.contains("tab") && lowered.contains("board"))
            || std::time::Instant::now() >= deadline
        {
            break text;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    };
    let lowered = copy.to_lowercase();
    assert!(
        lowered.contains("tab") && lowered.contains("board"),
        "the help overlay never tells the user to Tab to the board before pressing `j`/`k`; it \
         reads: {copy:?}\n  ADR-006's ratification (Option A, 2026-07-15) accepts the one-time Tab \
         as a documented cost ON THE CONDITION that the instruction reaches the user through the \
         help overlay's own copy — the discoverability surface this whole feature is built around. \
         An AT user who is never told stands on the board pressing `j` while nothing happens, \
         which is indistinguishable from the feature being absent."
    );
    assert!(
        lowered.contains('j') && lowered.contains('k'),
        "the help overlay mentions Tabbing to the board but never names the keys it unlocks; it \
         reads: {copy:?} — the ratified instruction is \"Tab to the board, then `j`/`k`\", and half \
         of it is not an instruction"
    );
}

// NOTE: This is a REPRESENTATIVE subset. The remaining ~70 concrete Given/When/Then
// phrases in keyboard-shortcut-bindings.feature are added by DELIVER as it unskips
// each slice (they are inert while the scenarios are @pending). Keeping the starter
// small avoids registering broad regexes that could later collide once real
// per-slice steps are introduced.

// --- SLICE 05 / STEP 05-03 (US-06): `Enter` opens the selected issue ---------
//
// The SEVENTH and last key. `Enter` reuses the pattern `c` established: the board
// card is ITSELF the shipped open affordance (`issue_card.html:1` carries the
// `hx-get`, the `hx-target` and the swap a pointer click already uses), so the
// binding CLICKS it rather than reconstructing a URL — no client CSRF, and the
// keyboard path and the pointer path open the same modal by the same mechanism.
//
// The `@guard` scenario below is expected to be GREEN BY INHERITANCE: `Enter` is
// in `NATIVE_TEXT_ENTRY_KEYS`, so guard 4 makes it inert inside a text field and
// the browser submits the form natively. ADR-002 cites exactly this as evidence
// the guard is structural. Its assertions still have to BITE — see the falsifica-
// tion note on `form_is_submitted`.

/// The modal a board card's own `hx-get` produces (`issue_edit_modal.html:1`).
/// NAMED, not `[data-modal]`: "a modal opened" would pass for a build where
/// `Enter` opened the NEW-ISSUE modal, or the wrong issue's — the two failures
/// this scenario exists to catch (ADR-004 rejects an index precisely because it
/// opens a DIFFERENT issue silently).
const EDIT_MODAL_SELECTOR: &str = "#modal-root [data-modal='edit-issue']";

/// The issue key the open modal is showing, read from the modal's own shipped
/// heading (`issue_edit_modal.html:3` — `<h2>Edit {{ key }}</h2>`). Asserted by
/// the server's own markup rather than a client-invented marker, so this cannot
/// pass over a client that mounted a modal of its own devising.
async fn open_modal_issue_key(browser: &fantoccini::Client) -> Option<String> {
    let heading = browser
        .execute(
            r##"var modal = document.querySelector("#modal-root [data-modal='edit-issue']");
               if (!modal) { return null; }
               var h = modal.querySelector('h2');
               return h ? h.textContent.trim() : '<no heading>';"##,
            Vec::new(),
        )
        .await
        .expect("read the open issue modal's heading");
    heading.as_str().map(|s| s.to_string())
}

/// Waits for the issue modal to be up and returns the key it names.
async fn await_issue_modal(browser: &fantoccini::Client, context: &str) -> String {
    browser
        .wait()
        .at_most(std::time::Duration::from_secs(10))
        .for_element(Locator::Css(EDIT_MODAL_SELECTOR))
        .await
        .unwrap_or_else(|err| {
            panic!(
                "{context} opened no issue modal in #modal-root ({err}). The card carries its own \
                 hx-get/hx-target/hx-swap (issue_card.html:1) — the same affordance a pointer \
                 click uses — so `Enter` clicking the selected card is all this needs. This reds \
                 if `Enter` is unbound, or if it reconstructs a URL and navigates instead."
            )
        });
    open_modal_issue_key(browser)
        .await
        .expect("the issue modal is up, so its heading is readable")
}

/// AC-06.1's Given. The ring is put on AUTH-2 by pressing the REAL `j` — never
/// mounted from JS, which would be Fixture Theater (it would green `Enter` over a
/// build where `j` is unbound entirely). The resting URL is stamped so the sibling
/// no-op scenarios can prove `Enter` never navigated.
#[given(regex = r#"^Mei is viewing the AUTH board and has selected AUTH-2 with the "j" key$"#)]
async fn viewing_board_with_auth2_selected(world: &mut FoundryWorld) {
    let url = board_url(world);
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .goto(&url)
        .await
        .expect("navigate to the AUTH board");
    browser_harness::wait_for_kb_ready(browser).await;
    assert_eq!(
        any_modal_count(browser).await,
        0,
        "a modal is already open on a freshly-loaded board — the assertion that `Enter` OPENS one \
         would then be true before Mei pressed anything"
    );
    select_by_pressing_j(browser, "body", "AUTH-2").await;
    capture_url_at_rest(browser).await;
}

/// AC-06.1's outcome, and the arm that makes it non-vacuous: the modal must name
/// AUTH-2 — the card Mei's ring is on — not merely exist. A modal for AUTH-1 is
/// the exact silent failure ADR-004's key-based model exists to prevent, and
/// "a modal opened" would pass for it.
#[then(regex = r"^the issue modal for AUTH-2 opens over the board$")]
async fn issue_modal_for_auth2_opens(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let heading = await_issue_modal(browser, "pressing \"Enter\" with AUTH-2 selected").await;
    assert!(
        heading.contains("AUTH-2"),
        "`Enter` opened an issue modal, but it is showing {heading:?} — Mei's ring is on AUTH-2. \
         Opening a DIFFERENT issue than the one selected is the silent, data-losing failure \
         ADR-004 rejects an index-based selection to prevent: she would edit the wrong issue."
    );
    // OVER the board, not instead of it (AC-06.1's own words). A build that
    // navigated to the issue's full page would satisfy "a modal named AUTH-2 is
    // on screen" nowhere — but one that replaced the board's own markup could.
    let cards = visible_card_order(browser).await;
    assert!(
        cards.iter().any(|k| k == "AUTH-2"),
        "the AUTH-2 modal is open but the board is gone from behind it (it shows {cards:?}) — \
         `Enter` must open the modal OVER the board, never navigate away from it"
    );
}

/// AC-06.2's Given (FR-9). Selection starts empty and is ASSERTED so, because the
/// whole claim is "no target => no action": if a card were already ringed, the
/// no-op below would be measuring nothing.
#[given(regex = r"^Mei is viewing the AUTH board and has not selected any card$")]
async fn viewing_board_with_nothing_selected(world: &mut FoundryWorld) {
    let url = board_url(world);
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .goto(&url)
        .await
        .expect("navigate to the AUTH board");
    browser_harness::wait_for_kb_ready(browser).await;
    let cards = visible_card_order(browser).await;
    assert!(
        !cards.is_empty(),
        "the AUTH board shows no cards at all, so \"Enter with nothing SELECTED does nothing\" \
         would pass for the trivial reason that there is nothing to open — the scenario's claim is \
         about the absence of a SELECTION, not the absence of issues"
    );
    assert!(
        selected_keys(browser).await.is_empty(),
        "a card is already selected on a freshly-loaded board — selection is ephemeral and starts \
         empty (BR-5), and this scenario's entire premise is that nothing is selected"
    );
    capture_url_at_rest(browser).await;
}

/// AC-06.3's Given. The modal is opened by the REAL `c` and the title is typed
/// into whatever the browser says is FOCUSED — never into an element located by
/// CSS, which would focus the field as a side effect and green this over a modal
/// that opened unfocused.
///
/// A CARD IS SELECTED FIRST, and that is what gives this scenario teeth.
///
/// Found at 05-03's falsification: with nothing selected, removing `Enter` from
/// `NATIVE_TEXT_ENTRY_KEYS` — i.e. DELETING guard 4's protection of this very key
/// — left all 33 scenarios GREEN. The scenario could not fail for the reason it
/// names. Two independent things were hiding the defect: `dispatch` reaches
/// `openSelected()`, which no-ops because `selectedKey` is null; and the shortcut
/// layer never calls `preventDefault()`, so the browser submits the form anyway.
/// The behaviour was real and protected — but NOT by the arm the scenario names.
/// That is exactly the UI-5 shape (guard 1 is unfalsifiable because guard 4
/// already covers its scenario's key), rediscovered on a different guard.
///
/// The scenario's own second arm — "no issue card is OPENED behind the modal" —
/// presupposes a card that COULD open. With no selection there is none, so the
/// arm is trivially true for every implementation, including one with no guard at
/// all. Selecting a card is therefore not an embellishment of the Given: it is
/// what the Then already claims. Same discipline as `no modal opens` scoping
/// itself to the whole document — a negative assertion has to be able to bite.
///
/// The Gherkin is UNCHANGED (it is DISTILL's); this realises an underspecified
/// Given in the only way that makes its own Then falsifiable. With it, deleting
/// `Enter` from `NATIVE_TEXT_ENTRY_KEYS` REDS this scenario: the shortcut layer
/// opens the ringed card's modal from inside the form — Mei's draft gone, in an
/// unrelated issue. Verified by doing it.
#[given(regex = r"^Mei has the new-issue modal open with a title typed into it$")]
async fn new_issue_modal_open_with_title_typed(world: &mut FoundryWorld) {
    let url = board_url(world);
    let title = "Enter must submit this form";
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .goto(&url)
        .await
        .expect("navigate to the AUTH board");
    browser_harness::wait_for_kb_ready(browser).await;
    // The REAL `j`, so the ring is genuinely there and this precondition cannot be
    // Fixture Theater. Pressed AFTER the navigation and stamped on `window`: the
    // stamp is destroyed by a page load, so a submit that navigated (rather than
    // htmx-swapping) cannot quietly satisfy the Then that reads it back.
    press(browser, "j").await;
    let selected = selected_keys(browser).await;
    assert_eq!(
        selected.len(),
        1,
        "the Given's own \"j\" left the ring on {selected:?} instead of exactly one card — this \
         scenario's \"no issue card is opened behind the modal\" is only falsifiable if there IS a \
         selected card that a mis-guarded `Enter` could open"
    );
    browser
        .execute(
            "window.__kbSelectedBeforeTyping = arguments[0]; return null;",
            vec![serde_json::json!(selected[0])],
        )
        .await
        .expect("stamp the selected key before the modal opens");
    // The REAL `c`, on the board Mei is already looking at — the same paired
    // assertion `open_new_issue_modal_by_pressing_c` makes (D15). Inlined rather
    // than delegated because that helper navigates first, which would wipe both
    // the ring and the stamp above.
    press(browser, "c").await;
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
    // WAIT for `autofocus` to actually land before typing. `open_new_issue_...`
    // waits for the field to EXIST, and htmx inserts it a beat before the browser
    // moves focus into it — so the first keystroke lands on `body` and is silently
    // dropped. Observed directly at this step's RED: the field held
    // "nter must submit this form".
    //
    // Waiting on `document.activeElement` — rather than typing into the element
    // found by CSS, which would FOCUS it as a side effect — is what keeps this a
    // real guard test: `find(...).send_keys(...)` would green the scenario over a
    // modal that opened unfocused, i.e. over a build where Mei really does have to
    // reach for the mouse.
    browser
        .wait()
        .at_most(std::time::Duration::from_secs(10))
        .for_element(Locator::Css(
            "#modal-root [data-modal='new-issue'] input[name='title']:focus",
        ))
        .await
        .expect(
            "the new-issue modal's title field never took focus, so a keystroke typed at the \
             focused element would land on `body` and be dropped. `new_issue_modal.html:6` carries \
             `autofocus` — this reds if that attribute is removed (AC-03.1's own claim).",
        );
    browser
        .active_element()
        .await
        .expect("read the focused element")
        .send_keys(title)
        .await
        .expect("type the title into the focused field");
    // The board's keys BEFORE filing + the title, so "the form is submitted" can
    // assert the DELTA rather than a presence check. Same discipline as AC-03.2's
    // own When: the Background already titles AUTH-2, so "a card with a title
    // exists" is true before Mei files anything.
    let captured = browser
        .execute(
            "window.__kbFiledTitle = arguments[0];
             window.__kbKeysBeforeFiling = Array.prototype.map.call(
               document.querySelectorAll('.board .issue-card[data-issue-key]'),
               function (card) { return card.getAttribute('data-issue-key'); }
             );
             return window.__kbKeysBeforeFiling.length;",
            vec![serde_json::json!(title)],
        )
        .await
        .expect("record the board's issue keys before filing");
    assert!(
        captured.as_u64().unwrap_or(0) > 0,
        "the AUTH board shows no issue cards behind the modal, so the \"a new issue appeared\" \
         delta below would have no baseline to be new against"
    );
    let value = browser
        .find(Locator::Css(TITLE_FIELD_SELECTOR))
        .await
        .expect("the new-issue modal must carry a title field")
        .prop("value")
        .await
        .expect("read the title field's value");
    assert_eq!(
        value.as_deref(),
        Some(title),
        "the title Mei typed did not land in the field (it holds {value:?}) — this scenario's When \
         presses Enter IN that field, so the field has to be the thing she is typing into"
    );
}

/// AC-06.3's When. The key goes to the FOCUSED element — the title field Mei is
/// typing in — which is the whole point: `Enter` here must be the FORM's, not the
/// shortcut layer's.
#[when(regex = r#"^Mei presses "Enter" in the title field$"#)]
async fn presses_enter_in_the_title_field(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .active_element()
        .await
        .expect("read the focused element")
        .send_keys(browser_harness::key_chord("Enter"))
        .await
        .expect("press \"Enter\" in the title field");
}

/// AC-06.3's outcome — GREEN BY INHERITANCE, and this is the arm that proves it
/// is not green for nothing.
///
/// `Enter` reaches the shipped `hx-post` (`new_issue_modal.html:4`) because guard
/// 4 declines the keys a text-entry context consumes natively, and
/// `NATIVE_TEXT_ENTRY_KEYS` names `Enter`. No client code is on this path at all —
/// which is exactly ADR-002's cited evidence that the guard is structural rather
/// than a pile of special cases. Remove `Enter` from `NATIVE_TEXT_ENTRY_KEYS` and
/// this REDS: the shortcut layer eats the keypress, the form never submits, and
/// `no_issue_card_opened_behind_the_modal` reds beside it. Verified by doing it.
///
/// Asserted as the DELTA, for AC-03.2's reason: the Background titles AUTH-2, so
/// a presence check would be true before she filed anything.
#[then(regex = r"^the form is submitted$")]
async fn form_is_submitted(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .wait()
        .at_most(std::time::Duration::from_secs(10))
        .for_element(Locator::Css("#modal-root:empty"))
        .await
        .expect(
            "pressing \"Enter\" in the title field left the new-issue modal open — the form was \
             never submitted. The shipped POST answers htmx with an out-of-band append and an \
             EMPTY primary body (issues.rs:553), so `#modal-root` clearing is how the create is \
             known to have succeeded. This is what reds if `Enter` is removed from \
             NATIVE_TEXT_ENTRY_KEYS: the shortcut layer would eat the keypress instead of letting \
             the field consume it (ADR-002).",
        );
    let described = browser
        .execute(
            "var before = window.__kbKeysBeforeFiling || [];
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
                   title: titleNode ? titleNode.textContent.trim() : '<no .title node>'
                 };
               }),
               expectedTitle: window.__kbFiledTitle
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
        "Mei pressed Enter in the title field and the AUTH board gained {} new card(s) (saw \
         {added:?}). The form submitting means exactly one issue is filed.",
        added.len()
    );
    assert_eq!(
        added[0]["title"].as_str(),
        Some(expected_title),
        "an issue was filed but it is titled {:?} instead of {expected_title:?} — the title Mei \
         typed did not reach the issue the form created",
        added[0]["title"].as_str().unwrap_or_default()
    );
}

/// AC-06.3's second arm — the one that names the bug. A build that bound `Enter`
/// with a per-shortcut carve-out (or with no guard at all) would open the SELECTED
/// card's modal from inside the form: Mei types a title, presses Enter, and lands
/// in an unrelated issue with her draft gone. Scoped to the WHOLE document, not to
/// `#modal-root`: the submit clears that host, so a host-scoped count would be
/// zero for any implementation and this arm would be vacuous exactly where it must
/// bite.
#[then(regex = r"^no issue card is opened behind the modal$")]
async fn no_issue_card_opened_behind_the_modal(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let opened = browser
        .find_all(Locator::Css("[data-modal='edit-issue']"))
        .await
        .expect("count the issue modals anywhere on the page")
        .len();
    let ringed = browser
        .execute(
            "return window.__kbSelectedBeforeTyping || null;",
            Vec::new(),
        )
        .await
        .expect("read back the key that was ringed before Mei typed");
    let ringed = ringed
        .as_str()
        .expect("the Given stamped the ringed key onto the page");
    assert_eq!(
        opened, 0,
        "pressing \"Enter\" in the title field submitted the form AND opened {opened} issue \
         modal(s) — the keypress was consumed twice. {ringed} was ringed behind the modal, and a \
         mis-guarded `Enter` opens it: Mei types a title, presses Enter, and lands in an unrelated \
         issue with her draft gone. `Enter` inside a text-entry context belongs to the field alone \
         (ADR-002 guard 4); a shortcut that also fires there is the carve-out BR-2 forbids. This \
         is what reds if `Enter` is removed from NATIVE_TEXT_ENTRY_KEYS."
    );
    // The ring must also not have MOVED. The selection survives the form (it is a
    // detached string — ADR-004), so it is still on the card Mei ringed; a build
    // where the shortcut layer ran inside the field could walk it instead.
    assert_selected_is(
        browser,
        ringed,
        "typing a title and pressing \"Enter\" inside the form",
    )
    .await;
}

/// AC-06.4's Given. AUTH-2 is selected with the real `j` and opened with the real
/// `Enter` — the round trip this scenario is about starts with the keyboard, so
/// the keyboard has to drive every step of it.
#[given(regex = r#"^Mei has opened AUTH-2 by pressing "Enter"$"#)]
async fn has_opened_auth2_by_pressing_enter(world: &mut FoundryWorld) {
    let url = board_url(world);
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .goto(&url)
        .await
        .expect("navigate to the AUTH board");
    browser_harness::wait_for_kb_ready(browser).await;
    select_by_pressing_j(browser, "body", "AUTH-2").await;
    press(browser, "Enter").await;
    let heading = await_issue_modal(browser, "the Given's own \"Enter\"").await;
    assert!(
        heading.contains("AUTH-2"),
        "the Given pressed \"Enter\" with AUTH-2 ringed and the modal that opened shows \
         {heading:?} — this scenario asserts what survives closing AUTH-2's modal, so AUTH-2's is \
         the one that has to be open"
    );
}

/// AC-06.4 / AC-07.3 — selection is not a casualty of the layer stack.
///
/// Two arms, and the second is the one with teeth. "AUTH-2 is still selected"
/// alone would pass for a build that left a stale ring on a dead model; pressing
/// `j` and requiring it to land on the card AFTER AUTH-2 proves the model itself
/// survived — `selectedKey` still resolves and the walk resumes from where Mei
/// left it, rather than restarting at the first card.
///
/// ADR-004 predicts this costs nothing: `Esc` clears CONTAINERS (ADR-003) and
/// `selectedKey` is a detached string it never touches.
#[then(regex = r#"^AUTH-2 is still selected so "j" moves to the next card$"#)]
async fn auth2_still_selected_and_j_moves_on(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    assert_selected_is(
        browser,
        "AUTH-2",
        "opening AUTH-2 and closing it with \"Esc\"",
    )
    .await;
    let order = visible_card_order(browser).await;
    let index = order
        .iter()
        .position(|k| k == "AUTH-2")
        .expect("AUTH-2 is on the board");
    let next = order.get(index + 1).unwrap_or_else(|| {
        panic!(
            "AUTH-2 is the LAST card Mei can see (the board shows {order:?}), so \"j moves to the \
             next card\" has no next card to move to and this scenario cannot assert what it says"
        )
    });
    press(browser, "j").await;
    assert_selected_is(
        browser,
        next,
        "closing AUTH-2's modal with \"Esc\" and pressing \"j\"",
    )
    .await;
}

// --- SLICE 05 / STEP 05-04 ---------------------------------------------------
//
// Three scenarios, one theme: the OPEN PATH stays single and the delegated layer
// stays live across surfaces and across swaps.
//
//   1. `@one-open-path @critical` — `/` -> "AUTH-2" -> `j` -> `Enter` opens THE
//      SAME modal a pointer click on AUTH-2's board card produces. ADR-005 §4:
//      search-result rows carry NO `hx-get` (`search_results.html:4`), so `Enter`
//      resolves `selectedKey` back to the board card and clicks ITS shipped
//      affordance. Zero server delta, one path.
//   2. `@named-edge` — the board renders only {backlog,todo,in_progress,done}
//      while search returns EVERY issue, so an issue in `cancelled` is findable
//      and has NO card. `Enter` is a no-op (FR-9).
//   3. `@htmx-swap` — filing via `c` OOB-appends a card (`issues.rs:553`) and
//      swaps `#modal-root`. `j`/`Enter` still work with no reload, and the ARIA
//      composite still covers the card htmx just swapped in.

/// The search panel's own selected row. `aria-selected`, not a client-invented
/// marker: the row is an `option` inside the results `listbox` for exactly the
/// reason a card is one inside the board (ADR-006 — `aria-selected` is STATE).
const SEARCH_ROW_SELECTED_SELECTOR: &str =
    "#kb-search-panel li.search-result[aria-selected='true']";

/// The state the board does NOT render. `0001_init.sql:72` permits
/// {backlog,todo,in_progress,done,cancelled}; `DEFAULT_COLUMNS` (projects.rs:49)
/// renders the first four. `cancelled` is therefore the ONLY state that makes an
/// issue findable-but-cardless — the edge ADR-005 §4 names. If a future migration
/// widens the enum this constant still holds; if it renders a Cancelled column,
/// the Given below reds rather than passing over an edge that no longer exists.
const UNRENDERED_STATE: &str = "cancelled";

/// Everything `#modal-root` is holding, verbatim.
///
/// The WHOLE markup, not a probe for a field: this is the one-open-path proof's
/// evidence, and the claim is that two paths produce the SAME modal — not that
/// both produce something with AUTH-2 in it somewhere. A second, client-authored
/// modal that merely happened to name the right issue would satisfy any narrower
/// read and is precisely the duplication AC-06.5 forbids.
///
/// Comparable across the two paths because the `_csrf` value is REUSED from the
/// session's cookie rather than minted per request (`csrf.rs:56-70`
/// `ensure_csrf_cookie` — "reuse the request's CSRF cookie if present"), so a
/// byte-identical comparison is legitimate here and is NOT masking anything. If
/// that ever changes, this assertion reds loudly rather than silently weakening.
async fn modal_root_markup(browser: &fantoccini::Client) -> String {
    let markup = browser
        .execute(
            "var host = document.getElementById('modal-root');
             return host ? host.innerHTML : null;",
            Vec::new(),
        )
        .await
        .expect("read #modal-root's markup");
    markup
        .as_str()
        .expect("the board carries a #modal-root to read (board.html:13)")
        .to_string()
}

/// Reveals the panel with the REAL `/`, types `query` with REAL keystrokes, waits
/// for the SHIPPED fragment to show exactly that issue, then moves the ring onto
/// the result row with the REAL `j`.
///
/// `j` is pressed AT THE SEARCH INPUT, which is where `/` left Mei's focus — not
/// at `body`. Pressing it anywhere else would be the test arranging a focus state
/// the user never has, and would green a build whose `j` cannot actually be
/// reached from the surface this scenario is about.
///
/// The ring is ASSERTED here rather than assumed: this is a Given, and "selected
/// the result with j" is either true or the scenario below is measuring `Enter`
/// against a selection that was never made.
#[given(regex = r#"^Mei has searched the board for "([^"]+)" and selected the result with "j"$"#)]
async fn searched_and_selected_the_result(world: &mut FoundryWorld, query: String) {
    let url = board_url(world);
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .goto(&url)
        .await
        .expect("navigate to the AUTH board");
    browser_harness::wait_for_kb_ready(browser).await;
    install_error_probe(browser).await;
    assert_eq!(
        any_modal_count(browser).await,
        0,
        "a modal is already open on a freshly-loaded board — the assertions below about the modal \
         `Enter` opens would then be true before Mei pressed anything"
    );
    press(browser, "/").await;
    let (focused, _) = focused_field(browser).await;
    assert!(
        focused.contains("[name=q]"),
        "pressing `/` on the board did not focus the search box (focus is on {focused} instead), \
         so the search-surface flow this scenario is about cannot even begin"
    );
    browser
        .find(Locator::Css(SEARCH_INPUT_SELECTOR))
        .await
        .expect("the search panel must carry an input to type into")
        .send_keys(&query)
        .await
        .unwrap_or_else(|err| panic!("type {query:?} into the search box: {err}"));
    let wanted = query.clone();
    wait_for_results(
        browser,
        &format!("exactly the issue {query}"),
        move |rows| rows.len() == 1 && rows[0].0 == wanted,
    )
    .await;

    // The press Mei actually makes, from the place `/` actually left her.
    press_at(browser, SEARCH_INPUT_SELECTOR, "j").await;

    let selected = browser
        .execute(
            "var row = document.querySelector(arguments[0]);
             var box = document.querySelector(arguments[1]);
             return {
               selected: row ? row.getAttribute('data-issue-key') : null,
               query: box ? box.value : null
             };",
            vec![
                SEARCH_ROW_SELECTED_SELECTOR.into(),
                SEARCH_INPUT_SELECTOR.into(),
            ],
        )
        .await
        .expect("read the selected search result row");
    let ringed = selected["selected"].as_str();
    let box_value = selected["query"].as_str().unwrap_or_default();
    assert_eq!(
        ringed,
        Some(query.as_str()),
        "`j` did not move the ring onto the {query} result row (the ringed row is {ringed:?}, and \
         the search box now reads {box_value:?}). ADR-005 §3: with the panel open, `j`/`k` walk \
         ONLY `li.search-result` rows. If the box reads {query:?} + \"j\", the `j` was CONSUMED BY \
         THE SEARCH BOX as a character (ADR-002 guard 4 — the box is a text-entry context and `j` \
         is a character it can consume), and never reached the dispatch table at all."
    );
    capture_url_at_rest(browser).await;
}

/// The `@one-open-path` proof (AC-06.5, ADR-005 §4), and the whole reason this
/// scenario is `@critical`.
///
/// It DISTINGUISHES "the pointer's modal" from "some modal" by running BOTH paths
/// in the same session and comparing the markup byte for byte:
///
///   1. capture what `Enter`-from-the-search-results mounted into `#modal-root`;
///   2. reload the board (a clean slate — no panel, no ring, no modal);
///   3. CLICK AUTH-2's card with a real pointer;
///   4. capture what THAT mounted;
///   5. assert the two are identical.
///
/// A build that minted a second open path — reconstructing `{edit_url}`, adding an
/// `hx-get` to the search fragment, or mounting a client-authored dialog — reds
/// here even when its modal names AUTH-2 correctly, because the markup would
/// differ. A build that opened the NEW-ISSUE modal, or AUTH-1's, reds twice over.
/// "A modal appeared" is exactly the assertion this proof refuses to be.
#[then(
    regex = r"^the modal that opens is the same one a pointer click on the AUTH-2 board card produces$"
)]
async fn modal_is_the_pointer_path_modal(world: &mut FoundryWorld) {
    let url = board_url(world);
    let browser = world.browser.as_ref().expect("browser session");

    let heading = await_issue_modal(
        browser,
        "pressing \"Enter\" with the AUTH-2 result selected",
    )
    .await;
    assert!(
        heading.contains("AUTH-2"),
        "`Enter` from the AUTH-2 search result opened an issue modal showing {heading:?}. ADR-005 \
         §4: `Enter` resolves the selected KEY back to `article.issue-card[data-issue-key=AUTH-2]` \
         and clicks THAT card's own shipped affordance — opening any other issue means the \
         resolution went to the wrong card."
    );
    let keyboard_markup = modal_root_markup(browser).await;
    assert!(
        !keyboard_markup.trim().is_empty(),
        "`Enter` left #modal-root empty while an [data-modal='edit-issue'] was found — the modal \
         is mounted somewhere other than the board's own shipped host"
    );
    // The board never went away: ADR-005 §4's resolution works only because the
    // panel OVERLAYS the cards rather than replacing them.
    let cards = visible_card_order(browser).await;
    assert!(
        cards.iter().any(|k| k == "AUTH-2"),
        "the AUTH-2 modal is open but AUTH-2's card is gone from the board behind it (it shows \
         {cards:?}). ADR-005 §4's whole premise is that the panel OVERLAYS the board, so the card \
         `Enter` resolves to is still in the DOM."
    );

    // --- the pointer's path, on a clean board -------------------------------
    browser
        .goto(&url)
        .await
        .expect("reload the AUTH board for the pointer path");
    browser_harness::wait_for_kb_ready(browser).await;
    assert_eq!(
        any_modal_count(browser).await,
        0,
        "the reloaded board already has a modal open, so the pointer path below would be compared \
         against a modal nobody clicked for"
    );
    browser
        .find(Locator::Css(".board .issue-card[data-issue-key='AUTH-2']"))
        .await
        .expect("AUTH-2's card must be on the board for Hiroshi to click")
        .click()
        .await
        .expect("click AUTH-2's board card");
    let pointer_heading = await_issue_modal(browser, "a pointer click on AUTH-2's card").await;
    assert!(
        pointer_heading.contains("AUTH-2"),
        "clicking AUTH-2's own card opened {pointer_heading:?} — the POINTER path is broken, so \
         this scenario cannot compare the keyboard's modal against it"
    );
    let pointer_markup = modal_root_markup(browser).await;

    assert_eq!(
        keyboard_markup, pointer_markup,
        "`Enter` from the search results and a pointer click on AUTH-2's board card produced \
         DIFFERENT modals. AC-06.5 requires exactly ONE open path, and ADR-005 §4 obtains it by \
         RESOLUTION (selectedKey -> the board card -> its own shipped hx-get), never by \
         duplication. A second path — a reconstructed {{edit_url}}, an hx-get added to \
         search_results.html, or a client-authored dialog — is what this difference means.\n  \
         keyboard: {keyboard_markup}\n  pointer:  {pointer_markup}"
    );
}

/// The named edge's precondition (ADR-005 §4). AUTH-9 is seeded in `cancelled`,
/// the one permitted state (`0001_init.sql:72`) the board does not render
/// (`DEFAULT_COLUMNS`, projects.rs:49) — so it is findable by search
/// (`list_issues_by_project` returns every issue) and has NO card.
///
/// BOTH halves are asserted, because the edge is the CONJUNCTION: an AUTH-9 that
/// the search cannot find, or that the board DOES render, makes the no-op below
/// true for a reason that has nothing to do with this scenario.
#[given(regex = r"^AUTH-9 exists in a state the board does not display$")]
async fn auth9_exists_in_an_unrendered_state(world: &mut FoundryWorld) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let project: (uuid::Uuid, uuid::Uuid) =
        sqlx::query_as("SELECT id, workspace_id FROM projects WHERE key_prefix = 'AUTH'")
            .fetch_one(pool)
            .await
            .expect("the AUTH project the Background seeded");
    let author: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users LIMIT 1")
        .fetch_one(pool)
        .await
        .expect("fetch author");
    sqlx::query(
        "INSERT INTO issues (id, workspace_id, project_id, number, title, state, priority, author_id)
              VALUES ($1, $2, $3, 9, $4, $5, 'medium', $6)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(project.1)
    .bind(project.0)
    .bind("Cancelled cookie rotation spike")
    .bind(UNRENDERED_STATE)
    .bind(author.0)
    .execute(pool)
    .await
    .unwrap_or_else(|err| {
        panic!(
            "seed AUTH-9 in the {UNRENDERED_STATE:?} state: {err}. `0001_init.sql:72` permits \
             {{backlog,todo,in_progress,done,cancelled}} and the board renders only the first \
             four (DEFAULT_COLUMNS, projects.rs:49) — if that CHECK constraint no longer admits \
             {UNRENDERED_STATE:?}, the findable-but-cardless edge this scenario pins needs a new \
             state, not a deleted assertion."
        )
    });
    sqlx::query(
        "UPDATE projects SET next_issue_number = GREATEST(next_issue_number, 10) WHERE id = $1",
    )
    .bind(project.0)
    .execute(pool)
    .await
    .expect("advance next_issue_number past the seeded AUTH-9");
}

/// AC-X.5's Given (NFR-6). The issue is filed with the REAL `c` and the REAL form
/// submit, so the swap this scenario is about is the one the APP performs
/// (`issues.rs:553` answers with an out-of-band `beforeend` append into the
/// Backlog column plus an empty primary body that clears `#modal-root`).
///
/// The swap is ASSERTED — the filed card must actually be on the board, and it is
/// remembered by key — because "shortcuts keep working after the page content is
/// swapped" over a page nothing swapped is a scenario about nothing.
#[given(regex = r#"^Mei has filed an issue by pressing "c" and submitting the form$"#)]
async fn has_filed_an_issue_by_pressing_c(world: &mut FoundryWorld) {
    open_new_issue_modal_by_pressing_c(world).await;
    let browser = world.browser.as_ref().expect("browser session");
    install_error_probe(browser).await;
    browser
        .execute(
            "window.__kbKeysBeforeFiling = Array.prototype.map.call(
               document.querySelectorAll('.board .issue-card[data-issue-key]'),
               function (card) { return card.getAttribute('data-issue-key'); }
             );
             // The RELOAD SENTINEL. A window property cannot survive a document
             // being replaced, so its absence later IS a reload — a stronger and
             // simpler witness than comparing URLs, which a reload leaves equal.
             window.__kbNoReloadSentinel = 'alive';
             return window.__kbKeysBeforeFiling.length;",
            Vec::new(),
        )
        .await
        .expect("record the board's keys before filing");
    let active = browser
        .active_element()
        .await
        .expect("read the focused element to type into");
    active
        .send_keys("Rotate the session secret")
        .await
        .expect("type the new issue's title");
    active
        .send_keys(browser_harness::key_chord("Enter"))
        .await
        .expect("submit the new-issue form");
    browser
        .wait()
        .at_most(std::time::Duration::from_secs(10))
        .for_element(Locator::Css("#modal-root:empty"))
        .await
        .expect(
            "submitting the new-issue form left the modal open, so the #modal-root swap this \
             scenario needs never happened (issues.rs:553 answers with an EMPTY primary body)",
        );
    let swapped_in = browser
        .execute(
            "var before = window.__kbKeysBeforeFiling || [];
             var added = Array.prototype.filter.call(
               document.querySelectorAll('.board .issue-card[data-issue-key]'),
               function (card) { return before.indexOf(card.getAttribute('data-issue-key')) === -1; }
             );
             window.__kbSwappedInKey = added.length === 1
               ? added[0].getAttribute('data-issue-key')
               : null;
             return window.__kbSwappedInKey;",
            Vec::new(),
        )
        .await
        .expect("identify the card htmx swapped in");
    assert!(
        swapped_in.as_str().is_some(),
        "filing the issue added no new card to the board (or more than one), so the htmx swap this \
         scenario exists to survive did not happen and every assertion below would be vacuous. The \
         shipped POST OOB-appends into [data-column='backlog'] (issues.rs:553)."
    );
    capture_url_at_rest(browser).await;
}

#[when(regex = r#"^Mei presses "j" and then "Enter"$"#)]
async fn presses_j_then_enter(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    press(browser, "j").await;
    press(browser, "Enter").await;
}

/// AC-X.5's outcome. THREE things, and all three are the same claim (NFR-6): the
/// ONE document-delegated listener needs no re-wiring after a swap, and neither
/// does the projection it drives.
///
///   1. `j` moved the ring — the delegated `keydown` still fires.
///   2. `Enter` opened THE SELECTED issue — the ring and the open path still
///      agree about which issue that is, over a board htmx has rewritten.
///   3. THE ARIA COMPOSITE COVERS THE SWAPPED-IN CARD. This is the arm that
///      requires the shared `htmx:afterSwap` hook, and it is not a bonus: ADR-006
///      derives the ring and `role`/`aria-selected` in ONE step precisely so the
///      visual and the semantic cannot drift. `markComposite()` ran at INIT, so a
///      card htmx appended AFTER init carries no `role="option"` — the board would
///      be a `listbox` holding a child that is not an `option`, and an AT user
///      would be told the swapped-in issue is not there. A hook that re-applied
///      only the ring would leave exactly that, which is why the assertion names
///      both and why the hook must be shared. Ring, role and activedescendant are
///      asserted TOGETHER because they are one projection or they are a bug.
#[then(regex = r"^the selection moves and the selected issue opens$")]
async fn selection_moves_and_selected_issue_opens(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let projection = browser
        .execute(
            r#"var cards = Array.prototype.slice.call(
                 document.querySelectorAll('.board .issue-card[data-issue-key]')
               );
               var region = document.querySelector('.board');
               var selected = cards.filter(function (c) {
                 return c.getAttribute('aria-selected') === 'true';
               });
               return {
                 ringed: cards.filter(function (c) { return c.classList.contains('kb-selected'); })
                              .map(function (c) { return c.getAttribute('data-issue-key'); }),
                 ariaSelected: selected.map(function (c) { return c.getAttribute('data-issue-key'); }),
                 activeDescendant: region ? region.getAttribute('aria-activedescendant') : null,
                 regionRole: region ? region.getAttribute('role') : null,
                 optionless: cards.filter(function (c) { return c.getAttribute('role') !== 'option'; })
                                  .map(function (c) { return c.getAttribute('data-issue-key'); }),
                 swappedIn: window.__kbSwappedInKey,
                 sentinel: window.__kbNoReloadSentinel || null
               };"#,
            Vec::new(),
        )
        .await
        .expect("read the board's projection after the swap");

    let ringed = json_strings(&projection["ringed"], "the ringed cards");
    let aria_selected = json_strings(&projection["ariaSelected"], "the aria-selected cards");
    let swapped_in = projection["swappedIn"]
        .as_str()
        .expect("the Given identified the card htmx swapped in")
        .to_string();
    let optionless = json_strings(&projection["optionless"], "the cards with no option role");

    // 1. The delegated listener survived the swap.
    assert_eq!(
        ringed.len(),
        1,
        "after filing an issue (which swaps #modal-root and OOB-appends a card), `j` left \
         {ringed:?} ringed — exactly one card must be selected. A handler bound to a card rather \
         than delegated on the document would be dead here, which is the whole of NFR-6."
    );
    // The ring and the ARIA state are ONE projection (ADR-006) — asserted as one.
    assert_eq!(
        aria_selected, ringed,
        "the ring is on {ringed:?} but `aria-selected=true` is on {aria_selected:?}. These are \
         DERIVED IN ONE STEP by design (ADR-006) exactly so they cannot drift; a swap that \
         re-applies one and not the other is the drift the single projection step exists to \
         prevent."
    );
    assert_eq!(
        projection["regionRole"].as_str(),
        Some("listbox"),
        "the board lost its `listbox` role across the htmx swap, so the composite `j`/`k` \
         announce through no longer exists (ADR-006)"
    );
    assert_eq!(
        projection["activeDescendant"].as_str(),
        Some(format!("issue-{}", ringed[0]).as_str()),
        "the ring is on {ringed:?} but the board's `aria-activedescendant` is {:?} — an AT user is \
         told about a different card than the one Mei can see (ADR-006: both are derived from the \
         same key, in the same step)",
        projection["activeDescendant"].as_str()
    );

    // 3. The composite covers the card htmx swapped in — the afterSwap hook.
    assert!(
        optionless.is_empty(),
        "{optionless:?} sit inside the board's `listbox` without `role=\"option\"` — and \
         {swapped_in} is the card htmx OOB-appended after `keyboard.js` initialised. \
         `markComposite()` runs at INIT; a card swapped in later is invisible to the composite \
         unless the SAME projection re-runs on `htmx:afterSwap`. ADR-006 requires the ring and the \
         ARIA composite be re-applied TOGETHER — a hook that restores only the ring leaves a ring \
         on a card with no role, which is why this asserts the whole projection and not half of it."
    );

    // 2. `Enter` opened the issue the ring is actually on.
    let heading = await_issue_modal(browser, "pressing \"j\" then \"Enter\" after the swap").await;
    assert!(
        heading.contains(&ringed[0]),
        "after the swap, `j` ringed {ringed:?} but `Enter` opened {heading:?}. Selection is a KEY \
         (ADR-004) precisely so the ring and the open path cannot disagree over a board htmx has \
         rewritten — an index-based selection is what silently opens the wrong issue here."
    );
}

/// The swap's defining claim (NFR-6): htmx replaced content IN PLACE. The witness
/// is the `window` sentinel the Given stamped — a full page load would destroy the
/// document and take it with it. Stronger than comparing URLs, which a reload
/// leaves equal.
#[then(regex = r"^no page reload was required$")]
async fn no_page_reload_was_required(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let survived = browser
        .execute(
            "return {
               sentinel: window.__kbNoReloadSentinel || null,
               url: window.location.href,
               rest: window.__kbUrlAtRest || null
             };",
            Vec::new(),
        )
        .await
        .expect("read the reload sentinel");
    assert_eq!(
        survived["sentinel"].as_str(),
        Some("alive"),
        "the `window` sentinel stamped before filing is GONE, so the document was replaced — the \
         page reloaded. Filing, selecting and opening are all htmx swaps into a live document \
         (NFR-6); a reload would also re-run `keyboard.js` from scratch, which is precisely how a \
         layer that needs re-wiring after a swap can look like one that does not."
    );
    assert_eq!(
        survived["url"].as_str(),
        survived["rest"].as_str(),
        "the browser navigated away from the board while filing, selecting and opening — every one \
         of those is an htmx swap over the SAME document (NFR-6)"
    );
}
