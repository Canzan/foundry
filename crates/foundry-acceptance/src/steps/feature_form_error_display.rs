//! form-error-display-contract — the `@needs-browser` lane's step definitions.
//!
//! SLICE 01 / STEP 01-01 lands BOTH the mechanism (static/js/form-errors.js +
//! the create dialog's `[data-error-slot]`) AND the instrument that proves it:
//! a real headless Chrome, via chromedriver, driving a REJECTED issue create end
//! to end and asserting the reason is VISIBLE in the rendered DOM.
//!
//! WHY A BROWSER AND NOT reqwest+scraper (DESIGN ADR-002): the fix is CLIENT-SIDE
//! JavaScript. The shipped HTTP acceptance lane (`feature_board_new_issue.rs`)
//! already proves the SERVER contract — a 400 + `issue-create-error` fragment for
//! an empty title — and it stays GREEN. But reqwest runs no JS, so it can never
//! see whether the browser DISPLAYS that 400 or DISCARDS it. That HTTP-body
//! blindness (RCA Root Cause B) is the exact hole that let the defect ship green:
//! htmx 2.0.4 does not swap 4xx bodies, so the correct 400 fragment was thrown
//! away and the form silently did nothing. These scenarios ADD the DOM assertion
//! the HTTP lane never had.
//!
//! WHY A WHITESPACE TITLE FOR "an empty title": the modal's title input carries
//! `required`, and htmx validates a form before it POSTs (`validation:halted`) —
//! so a genuinely-empty field is rejected in the browser and NEVER reaches the
//! server, meaning there is no 400 to discard and no defect to reproduce. The
//! defect this feature closes is precisely the browser-valid / server-invalid
//! gap: a title of one space passes native `required`, htmx POSTs it, and the
//! server's `title.trim()` (issues.rs:97) makes it empty → the byte-identical
//! 400 "Title is required" fragment. That is the real class of validation error
//! that ships, and it is the only "empty title" a browser will actually submit.
//!
//! FALSIFIABILITY (the reproduction of the bug): against a tree WITHOUT
//! form-errors.js or the `[data-error-slot]`, the 400 is discarded — the error
//! never appears — so `the validation error "Title is required" is visible
//! inside the dialog` FAILS. That RED, seen before GREEN, is the direct proof the
//! browser oracle catches what the HTTP lane could not.
//!
//! Every step phrase here is globally unique (a cucumber-rs requirement). The
//! Background lines (`a workspace ... exists`, `a project ... exists`, `Mei is
//! signed in`) are the shipped HTTP-lane steps (feature_board_new_issue.rs /
//! us_07_project_create.rs): they seed Acme / Mei / Backend / Sandbox and spawn
//! the ONE shared `InProcHarness`. The browser Givens below open a fantoccini
//! session against THAT SAME in-process origin and sign Mei in through the real
//! form — so both lanes exercise the identical app.

use crate::support::browser_harness;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use fantoccini::Locator;
use std::time::{Duration, Instant};

const MEI_EMAIL: &str = "mei@acme.com";
const MEI_PASSWORD: &str = "mei-correct-horse-battery-staple";
const TEAM_SLUG: &str = "backend";
const PROJECT_SLUG: &str = "sandbox";
const PROJECT_KEY_PREFIX: &str = "GEN";

/// The ADR-001 readiness marker `form-errors.js` sets at init (mirrors
/// keyboard.js's `[data-kb-ready]`). Present ⇒ the document-delegated
/// `htmx:beforeSwap` listener is attached, never merely "the file parsed".
const READY_SELECTOR: &str = "[data-form-errors-ready]";

/// The board's own shipped "New issue" trigger (board.html:6). Clicking it fires
/// the same `hx-get` → swap into `#modal-root` that a pointer click does, so the
/// dialog opens by the shipped mechanism and this lane invents no URL.
const NEW_ISSUE_TRIGGER: &str = "[data-action='new-issue']";

/// The mounted new-issue dialog (new_issue_modal.html:1).
const DIALOG_SELECTOR: &str = "#modal-root [data-modal='new-issue']";

/// The dialog's title field (new_issue_modal.html:6).
const TITLE_FIELD_SELECTOR: &str = "#modal-root [data-modal='new-issue'] input[name='title']";

/// The dialog's submit button (new_issue_modal.html:8).
const CREATE_BUTTON_SELECTOR: &str = "#modal-root [data-modal='new-issue'] button[type='submit']";

/// The opt-in error slot the beforeSwap handler routes a 4xx fragment INTO
/// (new_issue_modal.html — the div this step adds inside the form). The whole
/// mechanism resolves to: a slot exists ⇒ the 400 body is swapped here instead of
/// being discarded.
const ERROR_SLOT_SELECTOR: &str = "#modal-root [data-modal='new-issue'] [data-error-slot]";

const WAIT_TIMEOUT: Duration = Duration::from_secs(10);

/// Opens ONE browser session against the shared harness, signs Mei in through the
/// REAL sign-in form, and navigates to the Sandbox board. Both slice-01 Givens
/// funnel through here: the fixtures + harness already exist (the HTTP Background
/// seeded them), so this only adds the browser view onto the same origin.
async fn open_browser_on_sandbox_board(world: &mut FoundryWorld) {
    let browser = browser_harness::new_session().await;
    {
        let harness = world
            .harness
            .as_ref()
            .expect("the HTTP Background must have spawned harness");
        browser_harness::sign_in_through_browser(&browser, harness, MEI_EMAIL, MEI_PASSWORD).await;
        let url = format!(
            "{}/team/{TEAM_SLUG}/project/{PROJECT_SLUG}",
            harness.base_url()
        );
        browser
            .goto(&url)
            .await
            .expect("navigate to the Sandbox board");
    }
    world.browser = Some(browser);
}

/// Best-effort wait for the handler to attach before we submit. Non-panicking ON
/// PURPOSE: in GREEN it removes the race (submitting before the beforeSwap
/// listener exists would discard the 400 and flake); against the CURRENT tree it
/// simply falls through so the verdict rests on the DOM assertion — the honest
/// reproduction — rather than on this intermediate marker. S1's own `the page
/// reports the form-error handler is ready` is the strict gate for the marker.
async fn settle_ready(browser: &fantoccini::Client) {
    let _ = browser
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css(READY_SELECTOR))
        .await;
}

/// Opens the dialog by CLICKING the shipped trigger, types a server-empty title
/// (one space — see the module header on why "empty" is whitespace), and submits
/// through the shipped Create button. No URL is reconstructed and no production
/// markup is altered: this is the pointer path a human takes.
async fn open_dialog_and_submit_empty_title(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    settle_ready(browser).await;
    browser
        .find(Locator::Css(NEW_ISSUE_TRIGGER))
        .await
        .expect("the Sandbox board must render the New issue trigger")
        .click()
        .await
        .expect("click the New issue trigger");
    let title = browser
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css(TITLE_FIELD_SELECTOR))
        .await
        .expect("clicking New issue must open the dialog with a title field");
    // One space: the browser's `required` sees a non-empty value and lets htmx
    // POST; the server's `title.trim()` sees empty and returns the 400. A truly
    // empty field would be halted by htmx before any request — no 400, no defect.
    title.send_keys(" ").await.expect("type a whitespace title");
    browser
        .find(Locator::Css(CREATE_BUTTON_SELECTOR))
        .await
        .expect("the dialog must carry a Create button")
        .click()
        .await
        .expect("submit the new-issue dialog");
}

// --- Given ------------------------------------------------------------------

/// S1's lane probe entry point: bring up the browser view on the board. The
/// chromedriver process + session are started lazily by `new_session()` (the
/// shipped ADR-007 harness), so "has started chromedriver" is satisfied by the
/// first browser touch here.
#[given(
    regex = r#"^the browser lane has started chromedriver and navigated to the "Sandbox" board$"#
)]
async fn lane_navigated_to_sandbox_board(world: &mut FoundryWorld) {
    open_browser_on_sandbox_board(world).await;
}

/// S2's precondition, worded for the error scenario. Same view as S1's probe.
#[given(regex = r#"^Mei is viewing the "Sandbox" board in a real browser$"#)]
async fn viewing_sandbox_board(world: &mut FoundryWorld) {
    open_browser_on_sandbox_board(world).await;
}

/// The ADR-001 readiness assertion (S1). Strict: if `form-errors.js` is not
/// loaded or threw before init, the marker never appears and the lane fails HERE,
/// once, with a clear diagnosis — the anti-vacuity hook that says the mechanism
/// is actually live before any submit is attempted.
#[given(regex = r"^the page reports the form-error handler is ready$")]
async fn page_reports_handler_ready(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css(READY_SELECTOR))
        .await
        .expect(
            "the form-error handler never reported ready — form-errors.js sets \
             document.documentElement.dataset.formErrorsReady at init. Either it is not loaded \
             from base.html, or it threw before attaching its htmx:beforeSwap listener.",
        );
}

// --- When -------------------------------------------------------------------

/// The shared action for S1 and S2: open the create dialog and submit it with an
/// (server-)empty title, driving the real request → 400 → beforeSwap path.
#[when(regex = r"^Mei opens the new-issue dialog and submits it with an empty title$")]
async fn opens_dialog_and_submits_empty(world: &mut FoundryWorld) {
    open_dialog_and_submit_empty_title(world).await;
}

// --- Then -------------------------------------------------------------------

/// S1 — the dialog must survive the rejected submit. A slot-only swap leaves the
/// form (and its `.modal-dialog`) mounted and displayed; a whole-`#modal-root`
/// swap (or a navigation) would take it away.
#[then(regex = r"^the dialog stays open$")]
async fn dialog_stays_open(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let dialog = browser.find(Locator::Css(DIALOG_SELECTOR)).await.expect(
        "the new-issue dialog closed after a rejected submit — the error swap must target the \
             form's [data-error-slot], NOT replace #modal-root, so the dialog stays open.",
    );
    assert!(
        dialog.is_displayed().await.expect("dialog displayed?"),
        "the new-issue dialog is in the DOM but not displayed after the rejected submit"
    );
}

/// S1 + S2 — THE ORACLE (ADR-002). Bounded-polls the form's error slot until it
/// both DISPLAYS and CONTAINS the message. Against a tree with no form-errors.js
/// and no slot this can never hold: htmx discards the 400, nothing is swapped,
/// and the error is invisible — which is the defect, reproduced.
#[then(regex = r#"^the validation error "([^"]+)" is visible inside the dialog$"#)]
async fn validation_error_visible(world: &mut FoundryWorld, message: String) {
    let browser = world.browser.as_ref().expect("browser session");
    let deadline = Instant::now() + WAIT_TIMEOUT;
    loop {
        if let Ok(slot) = browser.find(Locator::Css(ERROR_SLOT_SELECTOR)).await {
            let displayed = slot.is_displayed().await.unwrap_or(false);
            let text = slot.text().await.unwrap_or_default();
            if displayed && text.contains(&message) {
                return;
            }
        }
        if Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let slot_state = match browser.find(Locator::Css(ERROR_SLOT_SELECTOR)).await {
        Ok(slot) => format!(
            "slot present, displayed={:?}, text={:?}",
            slot.is_displayed().await.unwrap_or(false),
            slot.text().await.unwrap_or_default()
        ),
        Err(_) => "no [data-error-slot] in the dialog at all".to_string(),
    };
    panic!(
        "the validation error {message:?} never became visible inside the dialog ({slot_state}). \
         The server returns a byte-identical 400 + \"{message}\" fragment; without form-errors.js \
         routing the 4xx body into the form's [data-error-slot], htmx 2.0.4 discards it and Mei \
         sees nothing. THIS is the defect the browser oracle exists to catch — the HTTP lane sees \
         the same 400 before and after and cannot tell the difference."
    );
}

/// S2 — the dialog survives AND Mei can immediately retype. Asserts the title
/// field is still mounted, displayed and enabled (so it takes focus), which the
/// slot-only swap preserves and a full swap would destroy.
#[then(regex = r"^the dialog stays open with the title field still focusable$")]
async fn dialog_open_title_focusable(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let field = browser
        .find(Locator::Css(TITLE_FIELD_SELECTOR))
        .await
        .expect(
            "the title field is gone after the rejected submit — the error swap replaced the form \
             instead of filling its [data-error-slot], so Mei cannot fix and resubmit.",
        );
    assert!(
        field.is_displayed().await.expect("title displayed?"),
        "the title field is present but not displayed after the rejected submit"
    );
    assert!(
        field.is_enabled().await.expect("title enabled?"),
        "the title field is displayed but disabled — Mei cannot type a correction into it"
    );
}

/// S2 — the rejected create persisted NOTHING. Asserted against the REAL store
/// (the harness's Postgres), the authoritative source: a bug that created the
/// issue despite the 400 would show a row here. Sandbox is seeded empty, so the
/// count is a genuine before/after zero, not a vacuous one — an OOB success card
/// would have appended a row and flipped this to 1.
#[then(regex = r"^no card was added to the board$")]
async fn no_card_added(world: &mut FoundryWorld) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let count: (i64,) = sqlx::query_as(
        "SELECT count(*) FROM issues \
          WHERE project_id = (SELECT id FROM projects WHERE key_prefix = $1)",
    )
    .bind(PROJECT_KEY_PREFIX)
    .fetch_one(pool)
    .await
    .expect("count Sandbox issues");
    assert_eq!(
        count.0, 0,
        "the rejected create still persisted {} issue(s) in Sandbox — a create that failed \
         validation must write nothing.",
        count.0
    );

    let browser = world.browser.as_ref().expect("browser session");
    let cards = browser
        .find_all(Locator::Css(".board [data-issue-key]"))
        .await
        .expect("count the board's issue cards");
    assert!(
        cards.is_empty(),
        "the board rendered {} issue card(s) after the rejected create — no card should have been \
         appended.",
        cards.len()
    );
}
