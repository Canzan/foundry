//! nav-collapse-toggle — the left-nav fold lane's step definitions.
//!
//! WHAT THE STEPS BELOW ARE ALLOWED TO KNOW. Three public facts, and nothing
//! else:
//!
//!   * the rendered geometry of the rail, the content column and the shell, read
//!     from the live browser;
//!   * the fold mark on the document element, and the choice the ORIGIN holds —
//!     the two declared channels the mechanism travels through;
//!   * the control's own accessible contract: a real `button` in the brand row
//!     carrying `aria-expanded`, `aria-controls` and an accessible name.
//!
//! The control is located BY THAT CONTRACT (`.sidebar__brand button[aria-expanded]`)
//! and not by a class of its own. A class would be a fixture fact nobody promised;
//! the accessible contract is the promise itself, so a control that drops it fails
//! here rather than passing under a selector no operator can perceive.
//!
//! WHAT THE STEPS DELIBERATELY DO NOT ASSERT. The strip's width in pixels: it is a
//! design value, not a product bound, so every folded-state assertion says
//! "narrower than the open rail" and never names a number. The exact wording of
//! the accessible name: the promise is that the name says which way the nav is set
//! and what the next press does, so the STATE is read from `aria-expanded` (exact)
//! and the NAME is required to be non-empty, to speak of the nav, and to CHANGE
//! with the press — a bare glyph, a missing label and a name written once at build
//! time each fail, while the wording stays the implementation's to choose.
//!
//! WHY ITS OWN STEP MODULE: the acceptance crate carries one `feature_*.rs` step
//! module per feature, and cucumber-rs requires globally-unique step text, so each
//! feature owns its phrases. The Background lines are the SHIPPED HTTP-lane seed
//! steps (`feature_board_new_issue.rs`): they seed Acme / Mei / Backend / Sandbox
//! and spawn the ONE shared `InProcHarness`. The browser Givens below open a
//! session against THAT SAME in-process origin, so both lanes exercise one app —
//! the choice `feature_pwa_mobile.rs` and `feature_canzan_theme.rs` both made.
//!
//! THE SERVED-MARKUP PARSE IS LOCAL ON PURPOSE. The no-flash scenario reads the
//! served `<head>` with `scraper`, a few lines that mirror what the theme lane
//! does for its own tag. Reaching into that module's private helper would couple
//! two features' oracles together; the shared thing here is the crate's `scraper`
//! dependency, not a helper.

use crate::support::browser_harness;
use crate::support::harness::signed_in_get;
use crate::support::served_markup::head_scripts;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use fantoccini::Locator;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------- the contract

/// The origin key the fold choice is remembered under, and the single value the
/// feature ever writes into it. The open state is the ABSENCE of the key, which
/// is what makes "a browser that never touched the control" and "an operator who
/// opened the nav again" the same document.
const NAV_CHOICE_KEY: &str = "foundry.nav";
const FOLDED_CHOICE: &str = "collapsed";

/// The one channel from the script to the stylesheet: a VALUELESS attribute on
/// the document element.
const FOLD_MARK: &str = "data-nav-collapsed";

/// The served path of the asset that stamps the mark. The tag carrying it must
/// stay render-blocking.
const NAV_SCRIPT_PATH: &str = "/static/js/nav-collapse.js";

/// The shipped width of the open rail. Named because the promise is that opening
/// the nav again returns it to what shipped — never to bound the folded strip,
/// which no promise fixes.
const OPEN_RAIL_PX: f64 = 240.0;

/// The breakpoint the phone rules are guarded by, so every phone assertion can
/// prove it was measured on the side of it that it claims to be about.
const PHONE_BREAKPOINT_PX: f64 = 480.0;

const NAV_CONTROL: &str = ".sidebar__brand button[aria-expanded]";
const DASHBOARD_WITNESS: &str = ".dash .card-grid .card a";
const REPORT_WITNESS: &str = "section.change-report[data-change-report]";
const SIGN_IN_WITNESS: &str = "form[action='/sign-in'] input[type='password']";

/// The longest workspace name the product admits — 24 characters, the ceiling the
/// app's own name rule enforces in the database (anything longer is refused with
/// "The name must be a valid workspace name: trimmed, not empty, no control
/// characters, at most 24 characters"). It is the widest brand row an operator can
/// create, so it is the worst case the folded strip has to survive.
const LONGEST_WORKSPACE_NAME: &str = "Northwind Logistics Ltd.";

const MEI_EMAIL: &str = "mei@acme.com";
const MEI_PASSWORD: &str = "mei-correct-horse-battery-staple";
const TEAM_SLUG: &str = "backend";
const PROJECT_SLUG: &str = "sandbox";

const WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_TAB_STOPS: usize = 40;

/// Where a step leaves a reading for the step after it. `sessionStorage`, so it
/// cannot collide with the `localStorage` key the production script owns, and so
/// it survives the reload and the navigation the scenarios drive.
const BEFORE_KEY: &str = "foundry.acceptance.nav-before";
const NAME_KEY: &str = "foundry.acceptance.nav-name";
const CHOICE_KEY: &str = "foundry.acceptance.nav-choice";

// ==========================================================================
// GIVEN — the page, and the state the operator left it in
// ==========================================================================

/// The baseline: a desk-width browser on the dashboard with nothing remembered.
/// Asserts the OPEN state before the scenario presses anything, so a run where
/// the rail was already folded cannot report a press as the thing that folded it,
/// and stashes the reading the geometry Then compares against.
#[given(regex = r"^the operator has the dashboard open at desk width with the left nav open$")]
async fn given_dashboard_open_at_desk_width(world: &mut FoundryWorld) {
    let browser = desk_width_dashboard(world).await;
    let reading = read_nav(&browser).await;
    assert_open_at_desk_width(&reading, "before the operator touched the control");
    stash(&browser, BEFORE_KEY, &reading).await;
    world.browser = Some(browser);
}

/// Law L1, the folding half, used as a PRECONDITION by every scenario about what
/// happens next. It presses the real control and refuses to continue unless the
/// rail actually folded: without that guard, every persistence scenario below
/// would pass on a browser that never folded anything.
#[given(regex = r"^the operator has folded the left nav away on the dashboard$")]
async fn given_folded_away_on_the_dashboard(world: &mut FoundryWorld) {
    let browser = desk_width_dashboard(world).await;
    let before = read_nav(&browser).await;
    assert_open_at_desk_width(&before, "before the operator folded the nav away");
    press_the_control(&browser, "to fold the left nav away").await;
    let after = read_nav(&browser).await;
    assert_folded(&after, &before, "after the operator folded the nav away");
    stash(&browser, BEFORE_KEY, &before).await;
    world.browser = Some(browser);
}

#[given(
    regex = r"^the operator has folded the left nav away on the dashboard and opened it again$"
)]
async fn given_folded_then_opened_again(world: &mut FoundryWorld) {
    let browser = desk_width_dashboard(world).await;
    let before = read_nav(&browser).await;
    assert_open_at_desk_width(&before, "before the operator folded the nav away");
    press_the_control(&browser, "to fold the left nav away").await;
    let folded = read_nav(&browser).await;
    assert_folded(&folded, &before, "after the first press");
    press_the_control(&browser, "to open the left nav again").await;
    let reopened = read_nav(&browser).await;
    assert_open_at_desk_width(&reopened, "after the operator opened the nav again");
    world.browser = Some(browser);
}

/// The two rows of the double-press law start from opposite ends of the same
/// two-value domain.
#[given(regex = r"^the operator has the dashboard open with the left nav (open|folded away)$")]
async fn given_dashboard_with_nav_in_state(world: &mut FoundryWorld, state: String) {
    let browser = desk_width_dashboard(world).await;
    let open = read_nav(&browser).await;
    assert_open_at_desk_width(&open, "when the dashboard opened");
    if state == "folded away" {
        press_the_control(&browser, "to reach the folded-away starting state").await;
        let folded = read_nav(&browser).await;
        assert_folded(&folded, &open, "at the folded-away starting state");
    }
    world.browser = Some(browser);
}

/// Two tabs of ONE browser, which is the smallest shape that has two actors on one
/// remembered choice. The second tab is loaded and asserted here, so the When only
/// has to press.
#[given(
    regex = r"^the operator has the dashboard open in two tabs of one browser with the left nav open$"
)]
async fn given_dashboard_in_two_tabs(world: &mut FoundryWorld) {
    let browser = desk_width_dashboard(world).await;
    let reading = read_nav(&browser).await;
    assert_open_at_desk_width(&reading, "in the tab the operator is reading");

    let reading_tab = browser.window().await.expect("read the current tab handle");
    let opened = browser
        .new_window(true)
        .await
        .expect("open a second tab of the same browser");
    browser
        .switch_to_window(opened.handle)
        .await
        .expect("switch to the second tab");
    goto_dashboard(&browser, world).await;
    let second = read_nav(&browser).await;
    assert_open_at_desk_width(&second, "in the second tab");
    browser
        .switch_to_window(reading_tab)
        .await
        .expect("switch back to the tab the operator is reading");
    world.browser = Some(browser);
}

/// A browser that already holds the fold choice from an earlier visit, written at
/// the ORIGIN and read back — if the write had silently failed the scenario would
/// collapse into the never-chosen one while still passing.
#[given(regex = r"^the browser remembers the folded-away choice from an earlier visit$")]
async fn given_browser_remembers_the_folded_choice(world: &mut FoundryWorld) {
    let browser = browser_harness::new_session().await;
    reach_the_origin(&browser, world).await;
    write_remembered_choice(&browser, Some(FOLDED_CHOICE)).await;
    assert_eq!(
        remembered_choice(&browser).await.as_deref(),
        Some(FOLDED_CHOICE),
        "the fold choice is not at the origin, so this scenario would be exercising the \
         never-chosen path and asserting nothing about what a later page reads"
    );
    world.browser = Some(browser);
}

/// The whole finite domain of what the origin can hold: nothing, this app's own
/// choice, and the strings an outside writer can leave behind. Each is written and
/// READ BACK, so a row can never pass by having stored nothing.
#[given(regex = r"^the browser remembers (.+) for the left nav$")]
async fn given_browser_remembers_value(world: &mut FoundryWorld, what: String) {
    let wanted = remembered_value_of(&what);
    let browser = browser_harness::new_session().await;
    reach_the_origin(&browser, world).await;
    write_remembered_choice(&browser, wanted).await;
    assert_eq!(
        remembered_choice(&browser).await.as_deref(),
        wanted,
        "the origin does not hold {what} after it was written, so this row would observe some \
         other state than the one it names"
    );
    world.browser = Some(browser);
}

/// Scripting switched off AT THE BROWSER — a real content setting, not a
/// simulation. The anti-vacuity half is asserted on the page in the Then, where
/// the keyboard layer's own marker can be looked for.
#[given(regex = r"^a browser session with scripting switched off$")]
async fn given_session_with_scripting_off(world: &mut FoundryWorld) {
    world.browser = Some(browser_harness::new_session_without_scripting().await);
}

/// Stored state refused for the origin, PROBED against the real origin because
/// the content setting is per-origin: a probe on `about:blank` would exercise no
/// setting at all and report success, and the scenario would quietly become a
/// second copy of the plain sign-in one.
#[given(regex = r"^a browser session whose browser refuses this site access to stored state$")]
async fn given_session_refusing_stored_state(world: &mut FoundryWorld) {
    let browser = browser_harness::new_dark_session_refusing_site_storage().await;
    reach_the_origin(&browser, world).await;
    let outcome = browser
        .execute(
            "try { window.localStorage.getItem('probe'); return 'READ_OK'; }
             catch (err) { return 'THREW:' + err.name; }",
            Vec::new(),
        )
        .await
        .expect("probe stored-state access at the real origin");
    assert_eq!(
        outcome.as_str(),
        Some("THREW:SecurityError"),
        "reading stored state at the origin returned {outcome:?} instead of throwing, so stored \
         state is NOT actually refused and this scenario would hold whether or not the read is \
         guarded. The capability is `SiteStorage::Refused` in support/browser_harness.rs."
    );
    world.browser = Some(browser);
}

/// The keyboard route to the control: focus cleared, then Tab pressed in document
/// order until the control holds it. The walk is recorded, so a failure names
/// every stop it did make instead of saying only that it never arrived.
#[given(
    regex = r"^the operator has walked the keyboard focus to the nav control on the dashboard$"
)]
async fn given_walked_the_focus_to_the_control(world: &mut FoundryWorld) {
    let browser = desk_width_dashboard(world).await;
    let before = read_nav(&browser).await;
    assert_open_at_desk_width(&before, "before the operator reached for the keyboard");
    let walk = tab_until_the_control_holds_focus(&browser).await;
    let focus = read_focus(&browser).await;
    assert!(
        focus["isNavControl"].as_bool().unwrap_or(false),
        "the keyboard walk ended on {:?} rather than on the nav control — reached stops: {walk:?}",
        focus["description"].as_str().unwrap_or("nothing")
    );
    stash(&browser, BEFORE_KEY, &before).await;
    world.browser = Some(browser);
}

/// The opening reading of the control: the STATE it reports and the NAME it
/// carries, both captured before the press so the Then can show the name was
/// re-derived rather than written once.
#[given(regex = r"^the control reports the left nav open and names what the next press does$")]
async fn given_control_reports_the_nav_open(world: &mut FoundryWorld) {
    let browser = world.browser.take().expect("a browser session");
    let reading = read_nav(&browser).await;
    assert_control_reports(&reading, false, "while the left nav is open");
    let name = control_name(&reading);
    stash(&browser, NAME_KEY, &serde_json::Value::String(name.clone())).await;
    world.browser = Some(browser);
}

/// The LONGEST workspace name an operator can actually create, set at the store
/// because the name is server-rendered and the scenario is about what the layout
/// does with it.
///
/// Not an arbitrary length, and deliberately not a huge one: the app enforces its
/// own name rule in the database, so a name above the limit is REFUSED and no
/// operator can ever put one in the rail. Measured against that rule — the store
/// rejects anything longer with its own diagnostic — so this is the widest brand
/// row the product admits, which is the honest worst case for the folded strip.
#[given(regex = r"^the workspace is named with the longest name the app allows$")]
async fn given_the_longest_workspace_name(world: &mut FoundryWorld) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let updated = sqlx::query("UPDATE workspaces SET name = $1 WHERE name = 'Acme'")
        .bind(LONGEST_WORKSPACE_NAME)
        .execute(pool)
        .await
        .unwrap_or_else(|err| {
            panic!(
                "the app refused {LONGEST_WORKSPACE_NAME:?} ({} characters) for a workspace \
                 name: {err}. This step is supposed to use the LONGEST name the product admits; \
                 if the name rule has moved, move this constant with it rather than loosening \
                 the rule.",
                LONGEST_WORKSPACE_NAME.chars().count()
            )
        });
    assert_eq!(
        updated.rows_affected(),
        1,
        "the Background's workspace was not renamed, so the rail would render the short name and \
         this scenario would assert nothing about a long one"
    );
}

// ==========================================================================
// WHEN — the operator's own press, keystroke, reload or navigation
// ==========================================================================

#[when(regex = r"^she folds the left nav away with its control$")]
async fn when_she_folds_the_nav_away(world: &mut FoundryWorld) {
    let browser = world.browser.take().expect("a browser session");
    press_the_control(&browser, "to fold the left nav away").await;
    world.browser = Some(browser);
}

/// The long-name scenario's one stimulus: the page is opened and the nav folded
/// in the same step, because the Given before it arranges the workspace rather
/// than the browser.
#[when(regex = r"^the operator folds the left nav away on the dashboard$")]
async fn when_operator_folds_the_nav_away_on_the_dashboard(world: &mut FoundryWorld) {
    let browser = desk_width_dashboard(world).await;
    let before = read_nav(&browser).await;
    assert_open_at_desk_width(&before, "before the operator folded the nav away");
    press_the_control(&browser, "to fold the left nav away").await;
    stash(&browser, BEFORE_KEY, &before).await;
    world.browser = Some(browser);
}

#[when(regex = r"^she opens the left nav again with its control$")]
async fn when_she_opens_the_nav_again(world: &mut FoundryWorld) {
    let browser = world.browser.take().expect("a browser session");
    press_the_control(&browser, "to open the left nav again").await;
    world.browser = Some(browser);
}

#[when(regex = r"^she presses the nav control twice$")]
async fn when_she_presses_the_control_twice(world: &mut FoundryWorld) {
    let browser = world.browser.take().expect("a browser session");
    press_the_control(&browser, "the first of two presses").await;
    press_the_control(&browser, "the second of two presses").await;
    world.browser = Some(browser);
}

#[when(regex = r"^she reloads that page$")]
async fn when_she_reloads_that_page(world: &mut FoundryWorld) {
    let browser = world.browser.take().expect("a browser session");
    browser.refresh().await.expect("reload the page");
    wait_for(&browser, DASHBOARD_WITNESS, "the reloaded dashboard").await;
    world.browser = Some(browser);
}

#[when(regex = r"^she opens the project board$")]
async fn when_she_opens_the_project_board(world: &mut FoundryWorld) {
    let browser = world.browser.take().expect("a browser session");
    goto_board(&browser, world).await;
    world.browser = Some(browser);
}

#[when(regex = r"^she folds the left nav away in the second tab$")]
async fn when_she_folds_the_nav_in_the_second_tab(world: &mut FoundryWorld) {
    let browser = world.browser.take().expect("a browser session");
    let reading_tab = browser.window().await.expect("read the current tab handle");
    let tabs = browser.windows().await.expect("list the browser's tabs");
    let other = tabs
        .into_iter()
        .find(|handle| *handle != reading_tab)
        .expect("the Given must have opened a second tab");
    browser
        .switch_to_window(other)
        .await
        .expect("switch to the second tab");
    let before = read_nav(&browser).await;
    press_the_control(&browser, "to fold the left nav away in the second tab").await;
    let after = read_nav(&browser).await;
    assert_folded(&after, &before, "in the second tab after its own press");
    browser
        .switch_to_window(reading_tab)
        .await
        .expect("switch back to the tab the operator is reading");
    world.browser = Some(browser);
}

/// Three loads with no press — the case that matters, because a load that
/// normalised the origin would erase a real fold choice the first time storage
/// happened to be unreadable. The byte the operator left is noted BEFORE the
/// walk, so the comparison is against her value and not against a constant.
#[when(
    regex = r"^the operator opens the dashboard, the project board and the project change report$"
)]
async fn when_operator_opens_three_pages(world: &mut FoundryWorld) {
    let browser = world.browser.take().expect("a browser session");
    let noted = remembered_choice(&browser).await;
    stash(
        &browser,
        CHOICE_KEY,
        &serde_json::Value::String(noted.clone().unwrap_or_default()),
    )
    .await;
    sign_in_and_settle(&browser, world).await;
    goto_dashboard(&browser, world).await;
    let on_dashboard = read_nav(&browser).await;
    goto_board(&browser, world).await;
    let on_board = read_nav(&browser).await;
    goto_report(&browser, world).await;
    let on_report = read_nav(&browser).await;
    stash(
        &browser,
        BEFORE_KEY,
        &serde_json::json!([on_dashboard, on_board, on_report]),
    )
    .await;
    world.browser = Some(browser);
}

#[when(regex = r"^the operator opens the dashboard page$")]
async fn when_operator_opens_the_dashboard_page(world: &mut FoundryWorld) {
    let browser = world.browser.take().expect("a browser session");
    sign_in_and_settle(&browser, world).await;
    goto_dashboard(&browser, world).await;
    world.browser = Some(browser);
}

/// An outside writer — another tool on the origin, an extension, devtools — puts
/// a VALUE where the feature writes none. The rules select on the mark's
/// presence, so this must still give exactly one of the two navs.
#[when(
    regex = r"^another tool stamps the page with a folded-away mark carrying a value of its own$"
)]
async fn when_another_tool_stamps_a_valued_mark(world: &mut FoundryWorld) {
    let browser = world.browser.take().expect("a browser session");
    browser
        .execute(
            "document.documentElement.setAttribute(arguments[0], 'maybe'); return true;",
            vec![serde_json::Value::String(FOLD_MARK.to_string())],
        )
        .await
        .expect("stamp the document with a valued fold mark");
    world.browser = Some(browser);
}

#[when(regex = r"^she presses (Enter|Space)$")]
async fn when_she_presses_a_key(world: &mut FoundryWorld, key: String) {
    let browser = world.browser.take().expect("a browser session");
    browser_harness::press_key(&browser, &key).await;
    world.browser = Some(browser);
}

#[when(regex = r"^she walks the whole page with the Tab key$")]
async fn when_she_walks_the_page_with_tab(world: &mut FoundryWorld) {
    let browser = world.browser.take().expect("a browser session");
    let walk = walk_the_tab_order(&browser).await;
    stash(&browser, BEFORE_KEY, &walk).await;
    world.browser = Some(browser);
}

#[when(regex = r"^the operator opens the sign-in screen of this app$")]
async fn when_operator_opens_the_sign_in_screen(world: &mut FoundryWorld) {
    let browser = world.browser.take().expect("a browser session");
    goto_sign_in(&browser, world).await;
    world.browser = Some(browser);
}

#[when(regex = r"^the operator opens the dashboard on a phone (\d+) px wide$")]
async fn when_operator_opens_the_dashboard_on_a_phone(world: &mut FoundryWorld, width: u32) {
    let remembered = carry_choice_to_a_phone(world, width).await;
    let browser = world.browser.take().expect("a phone session");
    sign_in_and_settle(&browser, world).await;
    goto_dashboard(&browser, world).await;
    assert_eq!(
        remembered_choice(&browser).await,
        remembered,
        "the phone session does not hold the choice the Given wrote, so this row would observe \
         the other rail state from the one it names"
    );
    world.browser = Some(browser);
}

#[when(regex = r"^the operator opens the project board on a phone (\d+) px wide$")]
async fn when_operator_opens_the_board_on_a_phone(world: &mut FoundryWorld, width: u32) {
    let remembered = carry_choice_to_a_phone(world, width).await;
    let browser = world.browser.take().expect("a phone session");
    sign_in_and_settle(&browser, world).await;
    goto_board(&browser, world).await;
    assert_eq!(
        remembered_choice(&browser).await,
        remembered,
        "the phone session does not hold the choice the Given wrote, so this scenario would \
         observe the open rail instead of the folded one"
    );
    world.browser = Some(browser);
}

// ==========================================================================
// THEN — what the operator can see of it
// ==========================================================================

/// Law L1's own observation, and the reason the feature exists: the content box
/// takes EXACTLY the room the rail released. A rail that narrows while the
/// content stays put leaves an empty band down the left of the page — the nav
/// looks right and the value is gone.
#[then(
    regex = r"^the left nav is a narrow strip and the page content is wider by the room the nav released$"
)]
async fn then_strip_and_content_took_the_room(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let before = unstash(browser, BEFORE_KEY).await;
    let after = read_nav(browser).await;
    assert_folded(&after, &before, "after the press");

    let released = px(&before, "railWidth") - px(&after, "railWidth");
    let gained = px(&after, "contentWidth") - px(&before, "contentWidth");
    assert!(
        (released - gained).abs() <= 1.5,
        "the rail released {released:.1}px and the content box gained {gained:.1}px of it — the \
         page is not taking the room the nav gave up, so the operator folded the nav away and got \
         an empty band instead of a wider page. The content column is already flexible: a rule \
         that pins its width, or a fixed gap left where the rail was, is what produces this."
    );
}

#[then(regex = r"^the left nav and the page content still fill the width of the shell$")]
async fn then_rail_and_content_fill_the_shell(world: &mut FoundryWorld) {
    let reading = read_nav(browser_of(world)).await;
    let rail = px(&reading, "railWidth");
    let content = px(&reading, "contentWidth");
    let shell = px(&reading, "shellWidth");
    assert!(
        (rail + content - shell).abs() <= 1.5,
        "the rail ({rail:.1}px) and the content box ({content:.1}px) sum to {:.1}px inside a \
         {shell:.1}px shell — the two no longer partition the page, so either something is \
         overlapping or a strip of the shell is painting nothing.",
        rail + content
    );
}

/// Law L3: the press — and only the press — writes the origin, in exactly one
/// canonical form. The open state is the ABSENCE of the key, never a word of its
/// own, which is what the no-scripting promise rests on.
#[then(regex = r"^the browser remembers (no nav choice at all|the folded-away choice)$")]
async fn then_browser_remembers(world: &mut FoundryWorld, what: String) {
    let browser = browser_of(world);
    let held = remembered_choice(browser).await;
    let wanted = if what == "the folded-away choice" {
        Some(FOLDED_CHOICE)
    } else {
        None
    };
    assert_eq!(
        held.as_deref(),
        wanted,
        "the origin holds {held:?} where it should hold {wanted:?}. The open state must be the \
         ABSENCE of the key: a word written for it makes an operator who opened the nav again \
         indistinguishable from a browser that never touched the control, and the next load would \
         have two spellings of one state to interpret."
    );
}

#[then(regex = r"^the left nav is still a narrow strip there$")]
async fn then_still_a_narrow_strip_there(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let before = unstash(browser, BEFORE_KEY).await;
    let after = read_nav(browser).await;
    assert_folded(
        &after,
        &before,
        "on the page the operator came back to, which must still show the nav she folded away",
    );
}

/// The no-flash promise, layer (a) — the LOAD-BEARING one, and a fact about what
/// the app SENDS rather than about how fast the loopback is. The tag must be a
/// head script carrying none of `defer`, `async`, `type="module"`; each of those
/// moves the stamp past the paint while leaving every settled-page assertion in
/// this file green.
///
/// The second arm is the anti-vacuity guard: the same parse must SEE deferring
/// attributes on the neighbours it is not asserting about. Without it, a parse
/// that found no attributes at all would satisfy the first arm by blindness.
#[then(
    regex = r"^the page already carries the folded-away mark, from a script the browser may not defer past the first paint$"
)]
async fn then_mark_is_stamped_by_a_render_blocking_script(world: &mut FoundryWorld) {
    let reading = read_nav(browser_of(world)).await;
    assert!(
        reading["marked"].as_bool().unwrap_or(false),
        "the page the operator navigated to carries no fold mark on its document element, so the \
         rail she folded away is open again — nothing read her remembered choice on this load"
    );

    let served = served_dashboard_html(world).await;
    let head = head_scripts(&served);
    assert!(
        head.len() >= 2,
        "the served document declares {} script(s) in <head> — the parse is not seeing the \
         document shell, so anything asserted about the stamping tag would be vacuous",
        head.len()
    );
    let (src, deferring) = head
        .iter()
        .find(|(src, _)| src.ends_with(NAV_SCRIPT_PATH))
        .unwrap_or_else(|| {
            panic!(
                "no <script> in <head> references {NAV_SCRIPT_PATH}. The fold mark has to be on \
                 the document element BEFORE the browser is permitted to paint, which is what a \
                 render-blocking tag in the head buys. At the foot of the body it lands after the \
                 page is already painted, and an operator who folded the nav away sees it flash \
                 open on every navigation. Head scripts found: {head:?}"
            )
        });
    assert!(
        deferring.is_empty(),
        "the `{src}` tag carries {deferring:?}. Each of those defers execution past parsing, so \
         the mark lands AFTER the first paint and the flash of the open nav is back — and it does \
         so silently, because every settled-page assertion in this lane still passes. Copying the \
         deferred neighbours in the same head is how this regression arrives."
    );
    let deferred_neighbours = head
        .iter()
        .filter(|(src, attrs)| !src.ends_with(NAV_SCRIPT_PATH) && !attrs.is_empty())
        .count();
    assert!(
        deferred_neighbours > 0,
        "no other head script carries a deferring attribute, so the parse above cannot tell a \
         render-blocking tag from a deferred one and the assertion held by blindness. Head \
         scripts found: {head:?}"
    );
}

/// The no-flash promise, layer (b) — a SUPPORTING measurement, and it stays one.
/// On a loopback origin it can pass by luck even with a deferred script, so it
/// never replaces the served-tag shape above. It cannot produce a false RED,
/// which is why it is worth having.
#[then(regex = r"^that script finished loading before the screen first painted$")]
async fn then_script_finished_before_first_paint(world: &mut FoundryWorld) {
    let timings = browser_of(world)
        .execute(
            "var needle = arguments[0];
             var script = performance.getEntriesByType('resource').filter(function (entry) {
               return entry.name.indexOf(needle) !== -1;
             });
             var paint = performance.getEntriesByType('paint').filter(function (entry) {
               return entry.name === 'first-contentful-paint';
             });
             return [
               script.length ? script[0].responseEnd : null,
               paint.length ? paint[0].startTime : null
             ];",
            vec![serde_json::Value::String(NAV_SCRIPT_PATH.to_string())],
        )
        .await
        .expect("read the browser's resource and paint timing");
    let row = timings.as_array().expect("the probe returns a pair");
    let fetched = row[0].as_f64().unwrap_or_else(|| {
        panic!(
            "the browser recorded no resource-timing entry for {NAV_SCRIPT_PATH} — the script was \
             never fetched on this navigation, so it cannot have settled the rail before paint"
        )
    });
    let painted = row[1].as_f64().unwrap_or_else(|| {
        panic!(
            "the browser recorded no first-contentful-paint entry, so this supporting measurement \
             could not be taken at all"
        )
    });
    assert!(
        fetched <= painted,
        "the fold script finished at {fetched:.1}ms and the screen first painted at \
         {painted:.1}ms, so it arrived {:.1}ms too late and the operator saw a frame with the \
         wrong nav. (Supporting measurement; the deterministic guard is the served-tag shape.)",
        fetched - painted
    );
}

#[then(
    regex = r"^the left nav is back at its full width with every nav item in sight and selectable$"
)]
async fn then_nav_is_back_with_items_usable(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let reading = read_nav(browser).await;
    assert_open_at_desk_width(&reading, "after the operator opened the nav again");
    assert_items_usable(browser, &reading, "after the operator opened the nav again").await;
}

#[then(regex = r"^the left nav is still at its full width$")]
async fn then_nav_is_still_at_full_width(world: &mut FoundryWorld) {
    let reading = read_nav(browser_of(world)).await;
    assert_open_at_desk_width(
        &reading,
        "on the page the operator reloaded after opening the nav again",
    );
}

/// Law L4: the operation is the negation of a boolean, so two presses are the
/// identity — on the rail AND at the origin.
#[then(regex = r"^the left nav is (open|folded away) again$")]
async fn then_nav_is_in_state_again(world: &mut FoundryWorld, state: String) {
    let browser = browser_of(world);
    let reading = read_nav(browser).await;
    if state == "open" {
        assert_open_at_desk_width(&reading, "after two presses from the open state");
    } else {
        let rail = px(&reading, "railWidth");
        let marked = reading["marked"].as_bool().unwrap_or(false);
        assert!(
            marked && rail < OPEN_RAIL_PX - 8.0,
            "after two presses from the folded-away state the rail is {rail:.1}px wide and the \
             fold mark is {}. Two presses are the identity on a two-value state: they must leave \
             the nav exactly where they found it.",
            if marked { "present" } else { "absent" }
        );
    }
}

#[then(regex = r"^the tab she was reading still shows the open left nav$")]
async fn then_the_reading_tab_is_unchanged(world: &mut FoundryWorld) {
    let reading = read_nav(browser_of(world)).await;
    assert_open_at_desk_width(
        &reading,
        "in the tab the operator is reading, after a press in the other tab. Nothing promises \
         that a press reaches a document that is already open, and a cross-tab listener would \
         move the page under her while she reads it",
    );
}

/// The "until it navigates" half of the two-tab law, and the arm that makes the
/// first assertion mean something: it proves the other tab's press really did
/// reach the origin.
#[then(regex = r"^that tab shows the narrow strip once she reloads it$")]
async fn then_the_reading_tab_follows_after_a_reload(world: &mut FoundryWorld) {
    let browser = world.browser.take().expect("a browser session");
    let before = read_nav(&browser).await;
    browser.refresh().await.expect("reload the page");
    wait_for(&browser, DASHBOARD_WITNESS, "the reloaded dashboard").await;
    let after = read_nav(&browser).await;
    assert_folded(
        &after,
        &before,
        "in the tab the operator reloaded, which must now read the choice the other tab made",
    );
    world.browser = Some(browser);
}

#[then(regex = r"^every one of those pages shows the narrow strip$")]
async fn then_every_page_shows_the_strip(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let readings = unstash(browser, BEFORE_KEY).await;
    let readings = readings.as_array().expect("three readings were recorded");
    for (reading, page) in
        readings
            .iter()
            .zip(["the dashboard", "the project board", "the change report"])
    {
        assert!(
            reading["marked"].as_bool().unwrap_or(false),
            "{page} carries no fold mark, so the nav the operator folded away is open there"
        );
        let rail = px(reading, "railWidth");
        assert!(
            rail > 0.0 && rail < OPEN_RAIL_PX - 8.0,
            "{page} renders the rail {rail:.1}px wide, not the narrow strip the remembered \
             choice asks for"
        );
    }
}

#[then(regex = r"^the remembered choice is byte-for-byte the one she left$")]
async fn then_remembered_choice_is_unchanged(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let noted = unstash(browser, CHOICE_KEY).await;
    let noted = noted.as_str().unwrap_or_default().to_string();
    let held = remembered_choice(browser).await.unwrap_or_default();
    assert_eq!(
        held, noted,
        "three page loads with no press changed the remembered choice from {noted:?} to \
         {held:?}. A load that rewrites or normalises the origin erases a real folded-away choice \
         the first time stored state happens to be unreadable."
    );
}

/// Law L5, read as the operator experiences it: every string that is not this
/// app's own choice leaves the nav OPEN, and no string produces a third nav.
#[then(regex = r"^the left nav renders (open|folded away)$")]
async fn then_nav_renders_in_state(world: &mut FoundryWorld, state: String) {
    let reading = read_nav(browser_of(world)).await;
    // The row would hold vacuously on a page where nothing reads the origin at
    // all: "open" is also what an app with no fold mechanism renders. So the
    // control has to BE there, reporting the state the row names.
    assert_control_reports(
        &reading,
        state == "folded away",
        &format!("for a remembered choice that should leave the nav {state}"),
    );
    if state == "open" {
        assert!(
            !reading["marked"].as_bool().unwrap_or(false),
            "the document carries the fold mark {:?} although the origin holds no choice this \
             feature ever wrote — an unrecognised string is being applied verbatim, so the \
             operator gets a nav no rule describes",
            reading["markValue"]
        );
        assert_open_at_desk_width(&reading, "for a choice this feature never wrote");
    } else {
        assert!(
            reading["marked"].as_bool().unwrap_or(false),
            "the origin holds this app's own fold choice and the document carries no mark, so \
             nothing read it on this load"
        );
        let rail = px(&reading, "railWidth");
        assert!(
            rail > 0.0 && rail < OPEN_RAIL_PX - 8.0,
            "the remembered fold choice left the rail {rail:.1}px wide rather than a narrow strip"
        );
    }
}

/// The construction claim, observed: the mark is VALUELESS and the rules select
/// on its presence, so a value an outside writer leaves selects the SAME strip.
/// Measured against the valueless mark in the same document, so no pixel width is
/// named and the comparison cannot drift with the design value.
#[then(regex = r"^the left nav is exactly as narrow as the strip this app's own mark gives$")]
async fn then_valued_mark_gives_the_same_strip(world: &mut FoundryWorld) {
    let browser = world.browser.take().expect("a browser session");
    let with_value = read_nav(&browser).await;
    assert_eq!(
        with_value["markValue"].as_str(),
        Some("maybe"),
        "the valued mark is not on the document, so this scenario would compare the strip with \
         itself"
    );
    browser
        .execute(
            "document.documentElement.setAttribute(arguments[0], ''); return true;",
            vec![serde_json::Value::String(FOLD_MARK.to_string())],
        )
        .await
        .expect("replace the valued mark with the valueless one this feature writes");
    let valueless = read_nav(&browser).await;
    let valued_width = px(&with_value, "railWidth");
    let valueless_width = px(&valueless, "railWidth");
    assert!(
        valueless_width > 0.0 && valueless_width < OPEN_RAIL_PX - 8.0,
        "the mark this feature writes leaves the rail {valueless_width:.1}px wide, so there is no \
         narrow strip to compare an outside writer's value against"
    );
    assert!(
        (valued_width - valueless_width).abs() <= 0.5,
        "a mark carrying a value renders the rail {valued_width:.1}px wide where the valueless \
         mark renders it {valueless_width:.1}px — the rules are selecting on the mark's VALUE, so \
         there is a third nav width that is neither folded nor open and that no operator could \
         repair."
    );
    world.browser = Some(browser);
}

#[then(regex = r"^the left nav is a narrow strip and the control still holds the focus, visibly$")]
async fn then_keyboard_press_folded_and_kept_focus(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let before = unstash(browser, BEFORE_KEY).await;
    let after = read_nav(browser).await;
    assert_folded(
        &after,
        &before,
        "after the operator pressed the key on the focused control",
    );
    let focus = read_focus(browser).await;
    assert!(
        focus["isNavControl"].as_bool().unwrap_or(false),
        "after the keypress the focus sits on {:?} rather than on the control — the operator has \
         lost her place in the page and cannot press it again",
        focus["description"].as_str().unwrap_or("nothing")
    );
    let style = focus["outlineStyle"].as_str().unwrap_or("none");
    let width = focus["outlineWidth"].as_f64().unwrap_or(0.0);
    assert!(
        style != "none" && width >= 2.0,
        "the focused control draws outline-style {style:?} at {width}px — an operator working \
         from the keyboard cannot see where she is"
    );
}

#[then(
    regex = r"^the control reports the left nav folded away and names what the next press does$"
)]
async fn then_control_reports_the_nav_folded(world: &mut FoundryWorld) {
    let reading = read_nav(browser_of(world)).await;
    assert_control_reports(&reading, true, "after the operator folded the nav away");
}

#[then(regex = r"^its wording is not the wording it carried before the press$")]
async fn then_control_wording_changed(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let before = unstash(browser, NAME_KEY).await;
    let before = before.as_str().unwrap_or_default().to_string();
    let now = control_name(&read_nav(browser).await);
    assert_ne!(
        now, before,
        "the control still reads {before:?} after the press — its name is written once at build \
         time and never re-derived, so it describes the state the nav USED to be in and tells the \
         operator the wrong thing about her next press"
    );
}

/// The screen-reader stressor, observed as a keyboard fact: the hidden items must
/// leave the focus order and the accessibility tree TOGETHER. A width-only or
/// opacity-only fold leaves links that are invisible and still focusable, so the
/// operator tabs into a nav she cannot see.
#[then(regex = r"^no hidden nav item ever takes the keyboard focus$")]
async fn then_no_hidden_item_takes_focus(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let walk = unstash(browser, BEFORE_KEY).await;
    let stops = walk.as_array().expect("the Tab walk recorded its stops");
    assert!(
        stops.len() >= 2,
        "the Tab walk made {} stop(s), so `no hidden nav item took the focus` would hold over a \
         page nothing walked",
        stops.len()
    );
    let trespass: Vec<String> = stops
        .iter()
        .filter(|stop| stop["inFoldedRegion"].as_bool().unwrap_or(false))
        .map(|stop| stop["description"].as_str().unwrap_or("?").to_string())
        .collect();
    assert!(
        trespass.is_empty(),
        "the Tab walk stopped on {} element(s) inside the folded-away part of the rail: {:?}. \
         They are hidden from the eye and still reachable from the keyboard, which is what a fold \
         done with width or opacity produces instead of taking them out of the layout.",
        trespass.len(),
        trespass
    );
    let reading = read_nav(browser).await;
    assert_eq!(
        reading["navDisplay"].as_str(),
        Some("none"),
        "the nav items are still laid out while the rail is folded away ({:?}) — they must leave \
         the layout, not merely be clipped",
        reading["navDisplay"]
    );
}

#[then(
    regex = r"^the monogram, the nav control and the theme control are still there, and the theme control still carries its name$"
)]
async fn then_strip_keeps_its_controls(world: &mut FoundryWorld) {
    let reading = read_nav(browser_of(world)).await;
    assert_ne!(
        reading["monogramDisplay"].as_str(),
        Some("none"),
        "the monogram is gone from the folded strip, so the operator has no sign of which \
         workspace she is in"
    );
    assert!(
        reading["control"]["shown"].as_bool().unwrap_or(false),
        "the nav control is not visible in the folded strip, so the operator has no way to get \
         the nav back"
    );
    let theme = &reading["themeControl"];
    assert!(
        theme["shown"].as_bool().unwrap_or(false),
        "the theme control is gone from the folded strip — folding the nav must not take away a \
         control this feature never touched"
    );
    let name = theme["name"].as_str().unwrap_or_default();
    assert!(
        !name.trim().is_empty(),
        "the theme control carries no accessible name in the folded strip: hiding its word must \
         cost it no name, because the name lives in its label and not in the word"
    );
}

#[then(regex = r"^the left nav renders at its full width with every nav item usable$")]
async fn then_nav_renders_full_width_with_items_usable(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let reading = read_nav(browser).await;
    assert_eq!(
        reading["kbReady"].as_u64(),
        Some(0),
        "the keyboard layer's own marker is on the page, so this session is running scripts after \
         all and the claim about the scripting-off path would prove nothing"
    );
    assert_open_at_desk_width(&reading, "with scripting switched off");
    assert_items_usable(browser, &reading, "with scripting switched off").await;
}

/// The control exists only in script, so with scripting off it is ABSENT rather
/// than present-but-dead. Two guards against a vacuous pass: the rail must be
/// PRESENT (it is server-rendered), and the sweep looks past the known selector
/// for anything a keyboard could reach that claims to work the nav.
#[then(
    regex = r"^no control for the left nav exists in the document, and none can be reached with the keyboard$"
)]
async fn then_no_nav_control_exists(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let sweep = browser
        .execute(
            "var rails = document.querySelectorAll('.sidebar').length;
             var hits = [];
             var nodes = document.querySelectorAll('button, a, [role=button], input, [tabindex]');
             for (var i = 0; i < nodes.length; i++) {
               var node = nodes[i];
               var name = ((node.getAttribute('aria-label') || '') + ' ' +
                           (node.getAttribute('title') || '') + ' ' +
                           node.textContent).toLowerCase();
               var claims = node.hasAttribute('aria-expanded') ||
                            name.indexOf('nav') !== -1 ||
                            name.indexOf('collapse') !== -1 ||
                            name.indexOf('fold') !== -1;
               var reachable = node.tabIndex >= 0;
               if (claims && reachable && node.closest('.sidebar')) {
                 hits.push(name.trim().slice(0, 60));
               }
             }
             return [rails, hits];",
            Vec::new(),
        )
        .await
        .expect("sweep the document for a control over the left nav");
    let row = sweep.as_array().expect("the sweep returns a pair");
    assert!(
        row[0].as_u64().unwrap_or(0) > 0,
        "the rail did not render at all, so `no control for the left nav` would hold over a page \
         with no nav on it. The shell is server-rendered and must exist with scripting off."
    );
    let hits = row[1].as_array().expect("the sweep returns its hits");
    assert!(
        hits.is_empty(),
        "with scripting off the rail still offers {} keyboard-reachable control(s) over the nav: \
         {hits:?}. A server-rendered control sits there looking live and does nothing when it is \
         pressed, which is worse for the operator than no control at all.",
        hits.len()
    );
}

/// Stored state refused: the read throws, the guard swallows it, and the operator
/// loses persistence and NOTHING else. The recorder is then proved on the same
/// session, because "no errors were recorded" is exactly what an instrument that
/// records nothing would say.
#[then(
    regex = r"^the page renders, carries no folded-away mark, and reports no script error to the operator$"
)]
async fn then_storage_refused_page_is_quiet(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    browser
        .find(Locator::Css(SIGN_IN_WITNESS))
        .await
        .expect("the sign-in screen must render its form, or nothing below means anything");
    let marked = browser
        .execute(
            "return document.documentElement.hasAttribute(arguments[0]);",
            vec![serde_json::Value::String(FOLD_MARK.to_string())],
        )
        .await
        .expect("read the fold mark off the document");
    assert_eq!(
        marked.as_bool(),
        Some(false),
        "the document carries the fold mark although the remembered choice could not be read at \
         all — the guard's catch must leave the nav open rather than guess"
    );
    // ANTI-VACUITY: a screen that runs no fold script at all reports nothing and
    // carries no mark either, so "quiet" would hold over an app without the
    // mechanism. The asset must have been FETCHED on this load; what is being
    // observed is a guard that swallowed a real refusal, not an absence.
    let fetched = browser
        .execute(
            "var needle = arguments[0];
             return performance.getEntriesByType('resource').filter(function (entry) {
               return entry.name.indexOf(needle) !== -1;
             }).length;",
            vec![serde_json::Value::String(NAV_SCRIPT_PATH.to_string())],
        )
        .await
        .expect("read the browser's resource timing for the fold asset");
    assert!(
        fetched.as_u64().unwrap_or(0) > 0,
        "this screen never fetched {NAV_SCRIPT_PATH}, so nothing touched stored state and `the \
         page renders and nothing is reported` would hold over an app with no fold mechanism in \
         it. The asset is loaded from the document shell every template extends."
    );
    assert_no_script_error_reported(browser, "while the storage-refused screen loaded").await;
}

#[then(
    regex = r"^that screen offers no nav control, carries the mark no rule there consumes, and reports no script error$"
)]
async fn then_railless_screen_is_quiet(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let probe = browser
        .execute(
            "return [
               document.querySelectorAll('.sidebar').length,
               document.querySelectorAll(arguments[0]).length,
               document.documentElement.hasAttribute(arguments[1])
             ];",
            vec![
                serde_json::Value::String(NAV_CONTROL.to_string()),
                serde_json::Value::String(FOLD_MARK.to_string()),
            ],
        )
        .await
        .expect("read the railless screen");
    let row = probe.as_array().expect("the probe returns a triple");
    assert_eq!(
        row[0].as_u64(),
        Some(0),
        "this screen has a rail after all, so it is not the no-rail case this scenario is about"
    );
    assert_eq!(
        row[1].as_u64(),
        Some(0),
        "a nav control was built on a screen with no nav to work — reaching into an absent mount \
         point is how the same script dies on every pre-auth screen in the app"
    );
    assert_eq!(
        row[2].as_bool(),
        Some(true),
        "the remembered fold choice left no mark on this document. The mark is stamped from the \
         choice whatever the page is; here no rule consumes it, and a load that skipped the stamp \
         is a load that decided the choice does not apply."
    );
    assert_no_script_error_reported(browser, "while the screen with no rail loaded").await;
}

#[then(
    regex = r"^the page does not scroll sideways, and the left nav still lays out as the top bar$"
)]
async fn then_phone_page_fits_and_rail_is_a_bar(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let reading = read_nav(browser).await;
    assert_mechanism_agrees_with_the_origin(browser, &reading, "the dashboard on a phone").await;
    assert_no_sideways_scroll(&reading, "the dashboard on a phone");
    let viewport = px(&reading, "viewport");
    assert!(
        viewport <= PHONE_BREAKPOINT_PX,
        "this session lays out at {viewport}px, above the {PHONE_BREAKPOINT_PX}px breakpoint — \
         the phone rules never applied, so the promise about phone widths would be measured \
         against the desk-width layout"
    );
    assert_eq!(
        reading["shellDirection"].as_str(),
        Some("column"),
        "below the breakpoint the shell must stack and the rail must span the top as a bar; it is \
         laying out as {:?} instead, so a fold rule has reached in and applied the desk-width \
         rail geometry inside a row",
        reading["shellDirection"]
    );
    let rail = px(&reading, "railWidth");
    assert!(
        rail >= viewport * 0.9,
        "the rail is {rail:.1}px wide in a {viewport:.1}px phone viewport — it is being drawn as \
         a narrow vertical rail inside the top-bar layout, which is exactly what widens the \
         document at phone widths"
    );
}

#[then(regex = r"^that page does not scroll sideways$")]
async fn then_that_page_does_not_scroll_sideways(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let reading = read_nav(browser).await;
    assert_mechanism_agrees_with_the_origin(browser, &reading, "the page the operator is on").await;
    assert_no_sideways_scroll(&reading, "the page the operator is on");
}

#[then(regex = r"^the board columns still scroll inside their own strip$")]
async fn then_board_columns_scroll_in_their_strip(world: &mut FoundryWorld) {
    let probe = browser_of(world)
        .execute(
            "var board = document.querySelector('.board');
             if (!board) { throw new Error('no board on the page'); }
             return [board.scrollWidth, board.clientWidth, getComputedStyle(board).overflowX];",
            Vec::new(),
        )
        .await
        .expect("measure the board's own strip");
    let row = probe.as_array().expect("the probe returns a triple");
    let scroll = row[0].as_f64().unwrap_or_default();
    let client = row[1].as_f64().unwrap_or_default();
    let overflow = row[2].as_str().unwrap_or("");
    assert!(
        scroll > client,
        "the columns do not overflow their own strip (scrollWidth {scroll} <= clientWidth \
         {client}), so `the strip scrolls, not the page` would hold over a board with nothing to \
         scroll"
    );
    assert!(
        overflow == "auto" || overflow == "scroll",
        "the board's strip is not scrollable sideways (overflow-x: {overflow}) — the page takes \
         the columns' overflow instead, and the whole board slides under the operator's thumb"
    );
}

#[then(
    regex = r"^the strip is still narrower than the open nav, with the workspace name not laid out at all$"
)]
async fn then_long_name_does_not_widen_the_strip(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    let before = unstash(browser, BEFORE_KEY).await;
    let reading = read_nav(browser).await;
    assert_folded(
        &reading,
        &before,
        "with the longest workspace name the app allows in the rail",
    );
    assert_eq!(
        reading["workspaceDisplay"].as_str(),
        Some("none"),
        "the workspace name is still laid out in the folded strip ({:?}). Clipped is not enough: \
         the shipped name never wraps, so a merely-clipped name widens the strip and the page \
         with it.",
        reading["workspaceDisplay"]
    );
}

// ==========================================================================
// THE READING — one probe, so a geometry claim and a state claim can never
// drift into two different moments
// ==========================================================================

const NAV_PROBE: &str = r#"
var rail = document.querySelector('.sidebar');
var content = document.querySelector('.app-shell__content');
var shell = document.querySelector('.app-shell');
var nav = document.querySelector('.sidebar__nav');
var control = document.querySelector('.sidebar__brand button[aria-expanded]');
var theme = document.querySelector('.theme-toggle');
var items = Array.prototype.slice.call(document.querySelectorAll('.sidebar__nav .sidebar__item'));
function width(el) { return el ? el.getBoundingClientRect().width : null; }
function display(el) { return el ? getComputedStyle(el).display : null; }
function shown(el) {
  return !!el && getComputedStyle(el).display !== 'none' && el.getBoundingClientRect().width > 0;
}
var controlled = null;
if (control && control.getAttribute('aria-controls')) {
  controlled = document.getElementById(control.getAttribute('aria-controls'));
}
return {
  marked: document.documentElement.hasAttribute(arguments[0]),
  markValue: document.documentElement.getAttribute(arguments[0]),
  railWidth: width(rail),
  contentWidth: width(content),
  shellWidth: width(shell),
  viewport: document.documentElement.clientWidth,
  docWidth: document.documentElement.scrollWidth,
  shellDirection: shell ? getComputedStyle(shell).flexDirection : null,
  navDisplay: display(nav),
  workspaceDisplay: display(document.querySelector('.sidebar__workspace')),
  monogramDisplay: display(document.querySelector('.sidebar__monogram')),
  itemCount: items.length,
  itemsShown: items.filter(shown).length,
  itemsFocusable: items.filter(function (el) {
    return el.tabIndex >= 0 && el.hasAttribute('href') && el.textContent.trim().length > 0;
  }).length,
  control: control ? {
    tag: control.tagName.toLowerCase(),
    expanded: control.getAttribute('aria-expanded'),
    controls: control.getAttribute('aria-controls'),
    controlsTheNav: controlled !== null && controlled === nav,
    name: (control.getAttribute('aria-label') || control.textContent || '').trim(),
    shown: shown(control),
    tabindex: control.getAttribute('tabindex')
  } : null,
  themeControl: theme ? {
    shown: shown(theme),
    name: (theme.getAttribute('aria-label') || '').trim()
  } : null,
  kbReady: document.querySelectorAll('[data-kb-ready]').length
};
"#;

async fn read_nav(client: &fantoccini::Client) -> serde_json::Value {
    client
        .execute(
            NAV_PROBE,
            vec![serde_json::Value::String(FOLD_MARK.to_string())],
        )
        .await
        .expect("read the rail, the content column and the control from the live page")
}

fn px(reading: &serde_json::Value, key: &str) -> f64 {
    reading[key].as_f64().unwrap_or_else(|| {
        panic!(
            "the page has no {key} to measure — the rail, the content column or the shell is \
             missing, so this is not the application shell at all. Reading: {reading}"
        )
    })
}

fn control_name(reading: &serde_json::Value) -> String {
    reading["control"]["name"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// The OPEN rail at desk width: no mark, the shipped width, and the items in the
/// layout. Named `at desk width` because the open rail on a phone is a top bar
/// and this is not the assertion for it.
fn assert_open_at_desk_width(reading: &serde_json::Value, when: &str) {
    assert!(
        !reading["marked"].as_bool().unwrap_or(false),
        "{when} the document carries the fold mark {:?}, so the rail is folded away where it \
         should be open",
        reading["markValue"]
    );
    let rail = px(reading, "railWidth");
    assert!(
        (rail - OPEN_RAIL_PX).abs() <= 2.0,
        "{when} the rail is {rail:.1}px wide instead of the shipped {OPEN_RAIL_PX}px it has \
         always been — opening the nav again must return it to what shipped, not to some other \
         width"
    );
    assert_ne!(
        reading["navDisplay"].as_str(),
        Some("none"),
        "{when} the nav items are out of the layout although the rail is open"
    );
}

/// The FOLDED rail: the mark present, the rail strictly narrower than it was,
/// still drawn, and the items out of the layout rather than merely out of sight.
/// No pixel width is named: the strip's width is a design value, not a promise.
fn assert_folded(reading: &serde_json::Value, before: &serde_json::Value, when: &str) {
    let control_absent = reading["control"].is_null();
    assert!(
        !control_absent,
        "{when} there is no control for the left nav in the rail's brand row at all. The control \
         is a `button` carrying `aria-expanded` and `aria-controls`, built by the asset that owns \
         the fold state and appended into `.sidebar__brand`; nothing the operator can press means \
         nothing she can fold. Reading: {reading}"
    );
    assert!(
        reading["marked"].as_bool().unwrap_or(false),
        "{when} the document element carries no fold mark, so the rules that draw the narrow \
         strip have nothing to select on"
    );
    let rail = px(reading, "railWidth");
    let was = px(before, "railWidth");
    assert!(
        rail > 0.0,
        "{when} the rail is not drawn at all ({rail:.1}px). A folded nav is a narrow control \
         strip the operator can get back from, never a rail of zero width with the control \
         somewhere else."
    );
    assert!(
        rail < was - 8.0,
        "{when} the rail is {rail:.1}px wide where it was {was:.1}px — it did not narrow into a \
         control strip, so nothing was released for the page content"
    );
    assert_eq!(
        reading["navDisplay"].as_str(),
        Some("none"),
        "{when} the nav items are still in the layout ({:?}) — they must leave the layout and the \
         accessibility tree together, or the operator can tab into a nav she cannot see",
        reading["navDisplay"]
    );
}

/// The control's own contract: a real button, `aria-expanded` telling the state
/// exactly, `aria-controls` resolving to the rail's own nav, and a name that
/// speaks of the nav. The WORDING is the implementation's to choose; that it
/// exists, names its subject and changes with the press is the promise.
fn assert_control_reports(reading: &serde_json::Value, folded: bool, when: &str) {
    let control = &reading["control"];
    assert!(
        !control.is_null(),
        "{when} no control for the left nav is in the rail at all, so nothing reports anything. \
         Reading: {reading}"
    );
    assert_eq!(
        control["tag"].as_str(),
        Some("button"),
        "{when} the control is a <{}> rather than a button — only a button answers Enter and \
         Space without code of its own",
        control["tag"].as_str().unwrap_or("?")
    );
    assert_eq!(
        control["expanded"].as_str(),
        Some(if folded { "false" } else { "true" }),
        "{when} the control reports aria-expanded={:?}. The state it publishes is what an \
         operator using assistive technology is told, so a stale or missing one tells her the \
         opposite of what the page shows.",
        control["expanded"]
    );
    assert!(
        control["controlsTheNav"].as_bool().unwrap_or(false),
        "{when} the control's aria-controls ({:?}) does not resolve to the rail's own primary \
         navigation, so the thing it works is not named to anyone who cannot see the page",
        control["controls"]
    );
    let name = control["name"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_string();
    assert!(
        !name.is_empty(),
        "{when} the control carries no accessible name — it is a bare glyph, and an operator \
         using assistive technology is told neither where she is nor what pressing it will do"
    );
    assert!(
        name.to_lowercase().contains("nav"),
        "{when} the control's name is {name:?} and never says what it works. A name has to name \
         its subject: the operator hears it out of the page's context."
    );
}

async fn assert_items_usable(client: &fantoccini::Client, reading: &serde_json::Value, when: &str) {
    let count = reading["itemCount"].as_u64().unwrap_or(0);
    assert!(
        count >= 2,
        "{when} the rail renders {count} nav item(s) — the shipped rail carries Home and Board, \
         so `every item is usable` would hold over a rail with nothing in it"
    );
    assert_eq!(
        reading["itemsShown"].as_u64(),
        Some(count),
        "{when} only {:?} of the {count} nav items are in sight",
        reading["itemsShown"]
    );
    assert_eq!(
        reading["itemsFocusable"].as_u64(),
        Some(count),
        "{when} only {:?} of the {count} nav items can be reached and followed — an item the \
         operator can see and cannot select is not usable",
        reading["itemsFocusable"]
    );
    let focused = client
        .execute(
            "var item = document.querySelector('.sidebar__nav .sidebar__item');
             if (!item) { throw new Error('no nav item to select'); }
             item.focus();
             return document.activeElement === item;",
            Vec::new(),
        )
        .await
        .expect("try to select the first nav item");
    assert_eq!(
        focused.as_bool(),
        Some(true),
        "{when} the first nav item cannot take the focus, so the operator cannot select it"
    );
}

/// THE ANTI-VACUITY GUARD FOR EVERY "NOTHING WENT WRONG" OBSERVATION. "The page
/// does not scroll sideways" and "the nav lays out as the bar" are both TRUE of
/// an app with no fold mechanism in it at all, so each of those observations is
/// paired with this: the control is on the page, and the mark on the document
/// agrees with the choice the ORIGIN holds. Without it, the folded-away rows of
/// those scenarios would measure the open nav twice and call it coverage.
async fn assert_mechanism_agrees_with_the_origin(
    client: &fantoccini::Client,
    reading: &serde_json::Value,
    what: &str,
) {
    let remembered = remembered_choice(client).await;
    let folded = remembered.as_deref() == Some(FOLDED_CHOICE);
    assert_control_reports(reading, folded, &format!("on {what}"));
    assert_eq!(
        reading["marked"].as_bool(),
        Some(folded),
        "on {what} the origin holds {remembered:?} and the document element carries the fold mark \
         {}. The state under test was never reached, so every measurement below it would have \
         been taken on the other nav.",
        if reading["marked"].as_bool().unwrap_or(false) {
            "anyway"
        } else {
            "not at all"
        }
    );
}

fn assert_no_sideways_scroll(reading: &serde_json::Value, what: &str) {
    let doc = px(reading, "docWidth");
    let viewport = px(reading, "viewport");
    assert!(
        doc <= viewport + 0.5,
        "{what} is {doc:.1}px wide inside a {viewport:.1}px viewport, so it scrolls sideways. \
         Something in the shell is wider than the screen: a fixed rail width applied inside the \
         phone's top bar does exactly this, at every phone width."
    );
}

/// "Nothing was reported" — read from the browser's OWN record, armed as a
/// session capability before the first navigation, then PROVED on the same
/// session: a deliberate uncaught throw must come back, or the assertion above
/// passed over an instrument that records nothing at all.
async fn assert_no_script_error_reported(client: &fantoccini::Client, when: &str) {
    let reported = browser_harness::unhandled_script_errors(client).await;
    assert!(
        reported.is_empty(),
        "the browser recorded {} unhandled script error(s) {when}:\n  - {}\nEvery touch of stored \
         state is supposed to be guarded: it throws, the catch leaves the nav open, and nothing \
         surfaces. An unguarded touch kills the script where it stands and takes the control down \
         on every screen for this operator.",
        reported.len(),
        reported.join("\n  - ")
    );
    client
        .execute(
            "setTimeout(function () { throw new Error('__NAV_RECORDER_PROBE__'); }, 0); \
             return null;",
            Vec::new(),
        )
        .await
        .expect("fire a deliberate uncaught error at the recorder");
    let deadline = Instant::now() + WAIT_TIMEOUT;
    let mut seen: Vec<String> = Vec::new();
    while Instant::now() < deadline {
        seen.extend(browser_harness::unhandled_script_errors(client).await);
        if seen
            .iter()
            .any(|line| line.contains("__NAV_RECORDER_PROBE__"))
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!(
        "a deliberate uncaught error was thrown and the recorder did not report it, so the \
         assertion above passed over an instrument that records NOTHING — `nothing was reported` \
         would hold on a page that reported everything. Recorded instead: {seen:?}"
    );
}

// ==========================================================================
// THE KEYBOARD ORDER
// ==========================================================================

const FOCUS_PROBE: &str = r#"
var el = document.activeElement;
if (!el) { return { description: 'nothing', isNavControl: false, inFoldedRegion: false }; }
var description = el.tagName.toLowerCase();
if (el.className && typeof el.className === 'string' && el.className.trim()) {
  description += '.' + el.className.trim().split(/\s+/).join('.');
}
var text = (el.getAttribute('aria-label') || el.textContent || '').trim().slice(0, 40);
var style = getComputedStyle(el);
return {
  description: description + (text ? ' [' + text + ']' : ''),
  isNavControl: !!(el.closest && el.closest('.sidebar__brand') && el.hasAttribute('aria-expanded')),
  inFoldedRegion: !!(el.closest && (el.closest('.sidebar__nav') || el.closest('.sidebar__user')) &&
                     !el.closest('.theme-toggle')),
  outlineStyle: style.outlineStyle,
  outlineWidth: parseFloat(style.outlineWidth) || 0,
  isBody: el === document.body
};
"#;

async fn read_focus(client: &fantoccini::Client) -> serde_json::Value {
    client
        .execute(FOCUS_PROBE, Vec::new())
        .await
        .expect("read what holds the keyboard focus")
}

async fn clear_focus(client: &fantoccini::Client) {
    client
        .execute(
            "if (document.activeElement && document.activeElement.blur) {
               document.activeElement.blur();
             }
             return true;",
            Vec::new(),
        )
        .await
        .expect("clear the focus before walking the reading order");
}

/// Tab from a cleared focus until the nav control holds it, returning the stops
/// made on the way so a failure names the order that DID happen.
async fn tab_until_the_control_holds_focus(client: &fantoccini::Client) -> Vec<String> {
    clear_focus(client).await;
    let mut walked = Vec::new();
    for _ in 0..MAX_TAB_STOPS {
        browser_harness::press_key(client, "Tab").await;
        let focus = read_focus(client).await;
        walked.push(
            focus["description"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
        );
        if focus["isNavControl"].as_bool().unwrap_or(false) {
            return walked;
        }
    }
    panic!(
        "the nav control was never reached in {MAX_TAB_STOPS} Tab presses, so it is not in the \
         reading order and an operator working from the keyboard cannot fold the nav at all. The \
         walk stopped at: {walked:?}"
    );
}

/// Walk the WHOLE reading order once and record every stop, so an assertion can
/// ask what the keyboard could reach rather than only where it ended up.
async fn walk_the_tab_order(client: &fantoccini::Client) -> serde_json::Value {
    clear_focus(client).await;
    let mut stops = Vec::new();
    for _ in 0..MAX_TAB_STOPS {
        browser_harness::press_key(client, "Tab").await;
        let focus = read_focus(client).await;
        let back_at_the_start = focus["isBody"].as_bool().unwrap_or(false) && !stops.is_empty();
        stops.push(focus);
        if back_at_the_start {
            break;
        }
    }
    serde_json::Value::Array(stops)
}

// ==========================================================================
// THE ORIGIN, THE PAGES AND THE SESSION
// ==========================================================================

/// A desk-width session, signed in through the real form, on the dashboard.
async fn desk_width_dashboard(world: &mut FoundryWorld) -> fantoccini::Client {
    let browser = match world.browser.take() {
        Some(browser) => browser,
        None => browser_harness::new_session().await,
    };
    sign_in_and_settle(&browser, world).await;
    goto_dashboard(&browser, world).await;
    browser
}

async fn sign_in_and_settle(browser: &fantoccini::Client, world: &FoundryWorld) {
    let harness = world
        .harness
        .as_ref()
        .expect("the HTTP Background must have spawned the harness");
    browser_harness::sign_in_through_browser(browser, harness, MEI_EMAIL, MEI_PASSWORD).await;
    let deadline = Instant::now() + WAIT_TIMEOUT;
    loop {
        let here = browser
            .current_url()
            .await
            .map(|url| url.to_string())
            .unwrap_or_default();
        if (!here.is_empty() && !here.contains("/sign-in")) || Instant::now() >= deadline {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn base_url(world: &FoundryWorld) -> String {
    world
        .harness
        .as_ref()
        .expect("harness")
        .base_url()
        .to_string()
}

fn board_url(world: &FoundryWorld) -> String {
    format!(
        "{}/team/{TEAM_SLUG}/project/{PROJECT_SLUG}",
        base_url(world)
    )
}

async fn goto_dashboard(browser: &fantoccini::Client, world: &FoundryWorld) {
    let url = format!("{}/", base_url(world));
    browser.goto(&url).await.expect("navigate to the dashboard");
    wait_for(browser, DASHBOARD_WITNESS, "the dashboard").await;
}

async fn goto_board(browser: &fantoccini::Client, world: &FoundryWorld) {
    let url = board_url(world);
    browser
        .goto(&url)
        .await
        .expect("navigate to the project board");
    wait_for(
        browser,
        browser_harness::BOARD_READY_SELECTOR,
        "the project board",
    )
    .await;
    let landed = browser
        .current_url()
        .await
        .map(|url| url.to_string())
        .unwrap_or_default();
    assert!(
        landed.starts_with(&url),
        "opening the project board landed on {landed} instead of {url} — the surface redirected \
         away, so the rail measured here belongs to another page"
    );
}

async fn goto_report(browser: &fantoccini::Client, world: &FoundryWorld) {
    let url = format!("{}/report", board_url(world));
    browser
        .goto(&url)
        .await
        .expect("navigate to the project change report");
    wait_for(browser, REPORT_WITNESS, "the project change report").await;
}

async fn goto_sign_in(browser: &fantoccini::Client, world: &FoundryWorld) {
    let url = format!("{}/sign-in", base_url(world));
    browser
        .goto(&url)
        .await
        .expect("navigate to the sign-in screen");
    wait_for(browser, SIGN_IN_WITNESS, "the sign-in screen").await;
}

/// Reach the origin so the stored choice bound to it can be read or written —
/// `localStorage` does not exist before a document does, and the content setting
/// the refused-storage session uses is per-origin.
async fn reach_the_origin(browser: &fantoccini::Client, world: &FoundryWorld) {
    goto_sign_in(browser, world).await;
}

async fn wait_for(browser: &fantoccini::Client, selector: &str, what: &str) {
    browser
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css(selector))
        .await
        .unwrap_or_else(|err| panic!("{what} never rendered `{selector}`: {err}"));
}

/// What the ORIGIN holds under the fold key: `None` for the absence that IS the
/// open state.
async fn remembered_choice(client: &fantoccini::Client) -> Option<String> {
    let value = client
        .execute(
            "try { return window.localStorage.getItem(arguments[0]); } catch (err) { return null; }",
            vec![serde_json::Value::String(NAV_CHOICE_KEY.to_string())],
        )
        .await
        .expect("read the remembered nav choice");
    value.as_str().map(str::to_string)
}

async fn write_remembered_choice(client: &fantoccini::Client, value: Option<&str>) {
    let argument = match value {
        Some(value) => serde_json::Value::String(value.to_string()),
        None => serde_json::Value::Null,
    };
    client
        .execute(
            "try {
               if (arguments[1] === null) { window.localStorage.removeItem(arguments[0]); }
               else { window.localStorage.setItem(arguments[0], arguments[1]); }
             } catch (err) {}
             return true;",
            vec![
                serde_json::Value::String(NAV_CHOICE_KEY.to_string()),
                argument,
            ],
        )
        .await
        .expect("leave the remembered nav choice at the origin");
}

/// The Examples column of the stored-choice table, read as what the origin holds.
/// `None` is the absence of the key — the open state and the pre-feature default.
fn remembered_value_of(what: &str) -> Option<&'static str> {
    match what.trim() {
        "nothing at all" => None,
        "the choice \"collapsed\"" => Some("collapsed"),
        "the word \"expanded\"" => Some("expanded"),
        "the word \"Collapsed\"" => Some("Collapsed"),
        "the value \"1\"" => Some("1"),
        "the value \"null\"" => Some("null"),
        other => panic!(
            "the table row names {other:?}, which no step knows how to leave at the origin — add \
             the row to `remembered_value_of` or write it the way the others are written"
        ),
    }
}

/// Carry the choice the Given left at the origin onto a REAL emulated phone of
/// the stated width. A phone is a new session, so the choice has to be written
/// again at the origin: the promise is about what a phone-width browser does with
/// a choice, and a desk-width session cannot answer it.
async fn carry_choice_to_a_phone(world: &mut FoundryWorld, width: u32) -> Option<String> {
    let desk = world.browser.take().expect("the Given opened a session");
    let remembered = remembered_choice(&desk).await;
    desk.close().await.ok();
    let phone = browser_harness::open_mobile_session_sized(width, 844).await;
    reach_the_origin(&phone, world).await;
    write_remembered_choice(&phone, remembered.as_deref()).await;
    assert_eq!(
        remembered_choice(&phone).await,
        remembered,
        "the phone session does not hold the choice the Given left, so this row would observe \
         some other state than the one it names"
    );
    world.browser = Some(phone);
    remembered
}

/// The dashboard AS SERVED, over real HTTP — the markup foundry SENDS, which is
/// what the render-blocking claim is about.
async fn served_dashboard_html(world: &FoundryWorld) -> String {
    let outcome = {
        let harness = world.harness.as_ref().expect("harness");
        let http = world.http.as_ref().expect("http");
        signed_in_get(harness, http, MEI_EMAIL, MEI_PASSWORD, "/").await
    };
    assert_eq!(
        outcome.status,
        reqwest::StatusCode::OK,
        "requesting the signed-in dashboard returned {} instead of 200, so nothing below can read \
         the served markup",
        outcome.status
    );
    outcome.body
}

/// One real activation of the real control, through a real press. The panic text
/// is the RED message this whole lane leads with while the control does not
/// exist, so it says WHAT is missing, WHY it matters and WHERE it belongs.
async fn press_the_control(client: &fantoccini::Client, what_for: &str) {
    let control = client
        .find(Locator::Css(NAV_CONTROL))
        .await
        .unwrap_or_else(|err| {
            panic!(
                "there is no control for the left nav to press {what_for}: nothing in the rail's \
                 brand row matches `{NAV_CONTROL}` ({err}). The operator is promised a control \
                 she can press to fold the left nav away; it is a `button` carrying \
                 `aria-expanded` and `aria-controls`, built by the asset that owns the fold state \
                 and appended into `.sidebar__brand` so it stays visible in the folded strip."
            )
        });
    control
        .click()
        .await
        .unwrap_or_else(|err| panic!("the nav control could not be pressed {what_for}: {err}"));
}

/// Where a step leaves a reading for the step after it. `sessionStorage`, so it
/// never collides with the `localStorage` key the production asset owns and so it
/// survives the reload and the navigation these scenarios drive.
async fn stash(client: &fantoccini::Client, key: &str, value: &serde_json::Value) {
    client
        .execute(
            "window.sessionStorage.setItem(arguments[0], arguments[1]); return true;",
            vec![
                serde_json::Value::String(key.to_string()),
                serde_json::Value::String(value.to_string()),
            ],
        )
        .await
        .expect("stash a reading for the step after this one");
}

async fn unstash(client: &fantoccini::Client, key: &str) -> serde_json::Value {
    let raw = client
        .execute(
            "return window.sessionStorage.getItem(arguments[0]);",
            vec![serde_json::Value::String(key.to_string())],
        )
        .await
        .expect("read back the reading the earlier step stashed");
    let raw = raw.as_str().unwrap_or_else(|| {
        panic!(
            "no reading was stashed under {key}, so the step before this one did not run or the \
             page lost it — nothing can be compared"
        )
    });
    serde_json::from_str(raw).expect("a stashed reading is JSON")
}

fn browser_of(world: &FoundryWorld) -> &fantoccini::Client {
    world
        .browser
        .as_ref()
        .expect("a browser session must have been opened first")
}
