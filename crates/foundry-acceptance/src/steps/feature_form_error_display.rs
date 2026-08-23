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

/// A live card in the Backlog column — the OOB append target a successful create
/// writes into (`hx-swap-oob="beforeend:[data-column='backlog']"`, issues.rs:571).
/// A create that succeeds drops a fresh `[data-issue-key]` article HERE.
const BACKLOG_CARD_SELECTOR: &str = ".board [data-column='backlog'] [data-issue-key]";

/// Any error slot on the whole page (not scoped to the dialog). The blast-radius
/// guard (S4) asserts NONE of these is displayed-with-text after a 2xx create —
/// if the handler mis-fired on success it would have swapped into one of these.
const ANY_ERROR_SLOT_SELECTOR: &str = "[data-error-slot]";

/// The opt-in error slot the beforeSwap handler routes a 4xx fragment INTO
/// (new_issue_modal.html — the div this step adds inside the form). The whole
/// mechanism resolves to: a slot exists ⇒ the 400 body is swapped here instead of
/// being discarded.
const ERROR_SLOT_SELECTOR: &str = "#modal-root [data-modal='new-issue'] [data-error-slot]";

// --- Slice 02 (S5): the issue EDIT dialog joins the contract ----------------
// The card carries `hx-get={edit_url}` → `hx-target="#modal-root"`
// (issue_card.html:1), so clicking it swaps the edit dialog
// (issue_edit_modal.html, `data-modal='edit-issue'`) into #modal-root by the
// shipped mechanism — this lane invents no URL.

/// The mounted issue-edit dialog (issue_edit_modal.html:1).
const EDIT_DIALOG_SELECTOR: &str = "#modal-root [data-modal='edit-issue']";

/// The edit dialog's title field (issue_edit_modal.html:6).
const EDIT_TITLE_FIELD_SELECTOR: &str = "#modal-root [data-modal='edit-issue'] input[name='title']";

/// The edit dialog's Save button (issue_edit_modal.html:16).
const EDIT_SAVE_BUTTON_SELECTOR: &str =
    "#modal-root [data-modal='edit-issue'] button[type='submit']";

/// The opt-in error slot the beforeSwap handler routes the edit endpoint's 4xx
/// fragment INTO (issue_edit_modal.html — the div step 02-01 adds inside the
/// form). Against a tree WITHOUT that slot the 400 is discarded and this never
/// resolves — the reproduction that proves the slot is load-bearing per form.
const EDIT_ERROR_SLOT_SELECTOR: &str = "#modal-root [data-modal='edit-issue'] [data-error-slot]";

// --- Slice 02 (S6): the comment-EDIT form joins the contract ----------------
// The comment card carries an Edit button `hx-get={edit_url}` →
// `hx-target="#comment-{id}"` (comment_card.html:4); clicking it swaps
// comment_edit_form.html (`form.comment-edit-form`, `hx-patch={patch_url}`)
// in place — this lane invents no URL and uses the shipped comment-edit flow.

/// The seeded comment's original body — set by the seeding Given and asserted
/// UNCHANGED in the store after the rejected edit. A single space would trim to
/// empty (the server-empty trick), so this is deliberately a non-blank sentence.
const ORIGINAL_COMMENT_BODY: &str = "The gateway needs a circuit breaker";

/// The seeded comment's Edit button on the issue page (comment_card.html:4).
const COMMENT_EDIT_BUTTON_SELECTOR: &str = ".comment .comment-edit-button";

/// The mounted comment-edit form's textarea (comment_edit_form.html:2).
const COMMENT_EDIT_TEXTAREA_SELECTOR: &str = ".comment-edit-form textarea[name='body_markdown']";

/// The comment-edit form's Save button (comment_edit_form.html:3).
const COMMENT_SAVE_BUTTON_SELECTOR: &str = ".comment-edit-form .comment-save-button";

/// The rendered validation error INSIDE the comment-edit form. Unlike the
/// modals (which swap `innerHTML` into #modal-root, so their `[data-error-slot]`
/// div SURVIVES and fills), the comment-edit form carries `hx-swap="outerHTML"`
/// (it replaces #comment-{id} with the re-rendered card on 2xx). form-errors.js
/// re-points the swap at the form's `[data-error-slot]` but keeps that outerHTML
/// spec, so on 4xx the slot div is REPLACED by the server's error fragment
/// (issue_400.html → `<div class="error">…`). The opt-in slot is still
/// load-bearing: without it the handler finds no slot, htmx 2.0.4 discards the
/// 400, and no `.error` ever appears. So we assert on the resulting `.error`.
const COMMENT_ERROR_SELECTOR: &str = ".comment-edit-form .error";

/// The server's byte-identical validation message for an empty comment body
/// (foundry-services comments.rs: `trimmed.is_empty()` → "Comment cannot be
/// empty"). The oracle asserts THIS text becomes visible in the slot.
const COMMENT_EMPTY_MESSAGE: &str = "Comment cannot be empty";

const WAIT_TIMEOUT: Duration = Duration::from_secs(10);

/// Parse the trailing number off an issue key like "GEN-1" → 1.
fn number_of(key: &str) -> i32 {
    key.rsplit_once('-')
        .and_then(|(_, n)| n.parse().ok())
        .unwrap_or_else(|| panic!("issue key {key:?} must end in -N"))
}

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
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css(NEW_ISSUE_TRIGGER))
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

/// Bounded-poll the dialog's error slot until it DISPLAYS and CONTAINS `message`
/// (the same oracle `validation_error_visible` uses). Used by S3's Given, which
/// must reach the "error is shown, form still mounted" state before the retry.
async fn wait_for_error_visible(browser: &fantoccini::Client, message: &str) {
    let deadline = Instant::now() + WAIT_TIMEOUT;
    loop {
        if let Ok(slot) = browser.find(Locator::Css(ERROR_SLOT_SELECTOR)).await {
            let displayed = slot.is_displayed().await.unwrap_or(false);
            let text = slot.text().await.unwrap_or_default();
            if displayed && text.contains(message) {
                return;
            }
        }
        if Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!(
        "S3 precondition never held: the error {message:?} did not become visible in the dialog, \
         so the fix-and-resubmit path cannot be exercised."
    );
}

/// Clears the (server-empty) title, types `title`, and submits the STILL-MOUNTED
/// dialog again. This only works if the prior 4xx swap left the form + its hidden
/// `_csrf` intact — a full-`#modal-root` replace would have removed the field.
async fn retype_title_and_resubmit(world: &mut FoundryWorld, title: &str) {
    let browser = world.browser.as_ref().expect("browser session");
    let field = browser
        .find(Locator::Css(TITLE_FIELD_SELECTOR))
        .await
        .expect(
            "the title field is gone before the retry — the 4xx swap must have replaced the form \
             instead of filling its [data-error-slot], dropping the field and its hidden _csrf.",
        );
    field.clear().await.expect("clear the whitespace title");
    field.send_keys(title).await.expect("type the real title");
    browser
        .find(Locator::Css(CREATE_BUTTON_SELECTOR))
        .await
        .expect("the dialog must still carry a Create button for the retry")
        .click()
        .await
        .expect("resubmit the new-issue dialog");
}

/// Opens the dialog by clicking the shipped trigger, types a VALID `title`, and
/// submits — the untouched 2xx success path (S4). No whitespace trick: this title
/// survives the server's `title.trim()` and creates a real issue.
async fn open_dialog_and_submit_valid_title(world: &mut FoundryWorld, title: &str) {
    let browser = world.browser.as_ref().expect("browser session");
    settle_ready(browser).await;
    browser
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css(NEW_ISSUE_TRIGGER))
        .await
        .expect("the Sandbox board must render the New issue trigger")
        .click()
        .await
        .expect("click the New issue trigger");
    let field = browser
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css(TITLE_FIELD_SELECTOR))
        .await
        .expect("clicking New issue must open the dialog with a title field");
    field.send_keys(title).await.expect("type a valid title");
    browser
        .find(Locator::Css(CREATE_BUTTON_SELECTOR))
        .await
        .expect("the dialog must carry a Create button")
        .click()
        .await
        .expect("submit the new-issue dialog");
}

/// Bounded-poll until the dialog is GONE (a successful create returns an empty
/// primary body, so htmx clears `#modal-root` and the dialog unmounts). Shared by
/// S3 + S4. Falsification hook: a handler that fires on 2xx would redirect the
/// empty body into the slot, leaving `#modal-root` populated and the dialog OPEN.
async fn wait_for_dialog_closed(browser: &fantoccini::Client) {
    let deadline = Instant::now() + WAIT_TIMEOUT;
    loop {
        match browser.find(Locator::Css(DIALOG_SELECTOR)).await {
            Err(_) => return,
            Ok(dialog) => {
                if !dialog.is_displayed().await.unwrap_or(true) {
                    return;
                }
            }
        }
        if Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!(
        "the new-issue dialog never closed after a successful create — a valid create returns an \
         empty primary body so htmx clears #modal-root; if the dialog is still up, the 2xx swap was \
         mis-routed (the error handler must fire ONLY on 4xx)."
    );
}

/// Bounded-poll the Backlog column for a card whose title matches `title` (the
/// OOB `beforeend:[data-column='backlog']` append, issues.rs:571). Shared by S3 +
/// S4 — proves the create actually persisted a card the operator can see.
async fn wait_for_backlog_card(browser: &fantoccini::Client, title: &str) {
    let deadline = Instant::now() + WAIT_TIMEOUT;
    loop {
        if let Ok(cards) = browser.find_all(Locator::Css(BACKLOG_CARD_SELECTOR)).await {
            for card in &cards {
                if card.text().await.unwrap_or_default().contains(title) {
                    return;
                }
            }
        }
        if Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!(
        "no Backlog card titled {title:?} appeared after the create. On the fix-and-resubmit path \
         this means the retry was rejected — most likely the 4xx swap dropped the form's hidden \
         _csrf, so the resubmit failed CSRF and created nothing."
    );
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

/// S5's seed: insert an issue into the EXISTING Sandbox project (created by the
/// HTTP Background) so the board renders its clickable card. Runs BEFORE the
/// browser Given, so the card is in the DB by the time the board loads.
#[given(regex = r#"^the "Sandbox" project has an issue "([^"]+)" titled "([^"]+)"$"#)]
async fn sandbox_has_issue(world: &mut FoundryWorld, issue_key: String, title: String) {
    let harness = world
        .harness
        .as_ref()
        .expect("the HTTP Background must have spawned harness");
    let pool = harness.app.state.store.pool();
    let project: (uuid::Uuid, uuid::Uuid) =
        sqlx::query_as("SELECT id, workspace_id FROM projects WHERE key_prefix = $1")
            .bind(PROJECT_KEY_PREFIX)
            .fetch_one(pool)
            .await
            .expect("fetch the Sandbox project seeded by the Background");
    let author: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(MEI_EMAIL)
        .fetch_one(pool)
        .await
        .expect("fetch Mei");
    // board-lane-management sweep: 0015 dropped the state DEFAULT — INSERT it.
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, description_md, state, author_id)
              VALUES ($1, $2, $3, $4, $5, '', 'backlog', $6)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(project.0)
    .bind(project.1)
    .bind(number_of(&issue_key))
    .bind(&title)
    .bind(author.0)
    .execute(pool)
    .await
    .expect("insert the Sandbox issue");
}

/// S3's precondition: reach the rejected-submit state (dialog open, error shown)
/// so the retry can prove the slot-only swap preserved the form + its `_csrf`.
/// Opens the browser view, submits the whitespace title, and blocks until the
/// "Title is required" reason is visible in the dialog's slot.
#[given(
    regex = r#"^Mei has submitted the new-issue dialog with an empty title and sees "([^"]+)"$"#
)]
async fn submitted_empty_and_sees_error(world: &mut FoundryWorld, message: String) {
    open_browser_on_sandbox_board(world).await;
    open_dialog_and_submit_empty_title(world).await;
    let browser = world.browser.as_ref().expect("browser session");
    wait_for_error_visible(browser, &message).await;
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

/// S3's action: type a real title into the SAME still-mounted dialog and submit
/// again. Succeeds only if the prior 4xx swap left the form (and its hidden
/// `_csrf`) intact — the direct proof of the slot-only swap.
#[when(regex = r#"^Mei types a title "([^"]+)" and submits the dialog again$"#)]
async fn retypes_title_and_resubmits(world: &mut FoundryWorld, title: String) {
    retype_title_and_resubmit(world, &title).await;
}

/// S4's action: open the dialog and submit a VALID title — the untouched 2xx
/// success path the error handler must not perturb.
#[when(regex = r#"^Mei opens the new-issue dialog and submits a valid title "([^"]+)"$"#)]
async fn opens_dialog_and_submits_valid(world: &mut FoundryWorld, title: String) {
    open_dialog_and_submit_valid_title(world, &title).await;
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

/// S3 + S4 — a successful create returns an empty primary body, so htmx clears
/// `#modal-root` and the dialog unmounts. This is the positive counterpart to
/// S1's `the dialog stays open`: on 2xx it MUST go away.
#[then(regex = r"^the dialog closes$")]
async fn dialog_closes(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    wait_for_dialog_closed(browser).await;
}

/// S3 + S4 — the create actually landed a visible card in Backlog (the OOB
/// append). Bounded-polls so it survives the swap latency.
#[then(regex = r#"^a card "([^"]+)" appears in the Backlog column$"#)]
async fn card_appears_in_backlog(world: &mut FoundryWorld, title: String) {
    let browser = world.browser.as_ref().expect("browser session");
    wait_for_backlog_card(browser, &title).await;
}

/// S3 — the whole retry happened via htmx in place: the browser is STILL on the
/// Sandbox board URL, never a full-page navigation. Proves "without a page
/// reload". A unique phrase (not the keyboard lane's `did not navigate away from
/// the board`, which is pinned to a DIFFERENT project's URL) — cucumber-rs
/// requires globally-unique step text.
#[then(regex = r#"^the browser is still on the "Sandbox" board without a reload$"#)]
async fn did_not_navigate_away(world: &mut FoundryWorld) {
    let harness = world.harness.as_ref().expect("harness");
    let expected = format!(
        "{}/team/{TEAM_SLUG}/project/{PROJECT_SLUG}",
        harness.base_url()
    );
    let browser = world.browser.as_ref().expect("browser session");
    let current = browser
        .current_url()
        .await
        .expect("read the browser's current URL");
    assert_eq!(
        current.as_str(),
        expected,
        "the browser navigated away to {current} — the fix-and-resubmit must swap in place via \
         htmx, never trigger a full-page reload."
    );
}

/// S4 — the blast-radius guard. After a 2xx create NO error slot anywhere on the
/// page may be displaying text: if the handler had fired on success it would have
/// mis-swapped the response into a slot and surfaced a spurious message.
#[then(regex = r"^no validation error is shown anywhere on the page$")]
async fn no_validation_error_anywhere(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let slots = browser
        .find_all(Locator::Css(ANY_ERROR_SLOT_SELECTOR))
        .await
        .expect("scan the page for error slots");
    for slot in &slots {
        let displayed = slot.is_displayed().await.unwrap_or(false);
        let text = slot.text().await.unwrap_or_default();
        assert!(
            !displayed || text.trim().is_empty(),
            "a validation error is showing after a SUCCESSFUL create (slot text {text:?}) — the \
             beforeSwap handler must fire only on 4xx, never on the 2xx success path."
        );
    }
}

// --- Slice 02 (S5): issue-edit dialog joins the contract --------------------

/// S5's action: click the shipped card to open the edit dialog, blank the
/// pre-filled title down to a server-empty value, and Save through the shipped
/// button. As in slice 01 (see the module header), "empty" is ONE SPACE: a
/// truly-cleared field carries native `required`, so htmx would halt the POST
/// (`validation:halted`) and no 400 would exist to display. A single space
/// passes native `required`, htmx POSTs, and the edit endpoint's `title.trim()`
/// (issues.rs:97) makes it empty → the byte-identical 400 "Title is required"
/// fragment routed through the beforeSwap handler.
#[when(regex = r#"^Mei opens the edit dialog for "([^"]+)", clears the title, and saves$"#)]
async fn opens_edit_clears_and_saves(world: &mut FoundryWorld, key: String) {
    let browser = world.browser.as_ref().expect("browser session");
    settle_ready(browser).await;
    let card_selector = format!(".board [data-issue-key='{key}']");
    browser
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css(card_selector.as_str()))
        .await
        .expect("the Sandbox board must render the seeded issue card")
        .click()
        .await
        .expect("click the issue card to open its edit dialog");
    let title = browser
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css(EDIT_TITLE_FIELD_SELECTOR))
        .await
        .expect("clicking the card must open the edit dialog with a pre-filled title field");
    title.clear().await.expect("clear the pre-filled title");
    // One space: server-empty after trim(), but browser-valid so htmx POSTs it.
    title.send_keys(" ").await.expect("type a whitespace title");
    browser
        .find(Locator::Css(EDIT_SAVE_BUTTON_SELECTOR))
        .await
        .expect("the edit dialog must carry a Save button")
        .click()
        .await
        .expect("submit the edit dialog");
}

/// S5 — THE ORACLE, scoped to the EDIT dialog's slot. Bounded-polls until the
/// slot both DISPLAYS and CONTAINS the message. Against a tree where
/// issue_edit_modal.html has no `[data-error-slot]`, htmx 2.0.4 discards the
/// 400 and this never holds — the reproduction that proves the slot is
/// load-bearing PER FORM even though the handler is unchanged from slice 01.
#[then(regex = r#"^the validation error "([^"]+)" is visible inside the edit dialog$"#)]
async fn validation_error_visible_edit(world: &mut FoundryWorld, message: String) {
    let browser = world.browser.as_ref().expect("browser session");
    let deadline = Instant::now() + WAIT_TIMEOUT;
    loop {
        if let Ok(slot) = browser.find(Locator::Css(EDIT_ERROR_SLOT_SELECTOR)).await {
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
    let slot_state = match browser.find(Locator::Css(EDIT_ERROR_SLOT_SELECTOR)).await {
        Ok(slot) => format!(
            "slot present, displayed={:?}, text={:?}",
            slot.is_displayed().await.unwrap_or(false),
            slot.text().await.unwrap_or_default()
        ),
        Err(_) => "no [data-error-slot] in the edit dialog at all".to_string(),
    };
    panic!(
        "the validation error {message:?} never became visible inside the edit dialog \
         ({slot_state}). The edit endpoint returns a byte-identical 400 + \"{message}\" fragment; \
         without a [data-error-slot] in issue_edit_modal.html the generic handler finds no slot in \
         THIS form, htmx 2.0.4 discards the 400, and Mei sees nothing — the per-form defect this \
         step closes."
    );
}

/// S5 — the edit dialog survives the rejected save. A slot-only swap leaves the
/// form (and its `.modal-dialog`) mounted; a whole-`#modal-root` swap would take
/// it away, matching S1's `the dialog stays open` for the create dialog.
#[then(regex = r"^the edit dialog stays open$")]
async fn edit_dialog_stays_open(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let dialog = browser
        .find(Locator::Css(EDIT_DIALOG_SELECTOR))
        .await
        .expect(
            "the edit dialog closed after a rejected save — the error swap must target the form's \
             [data-error-slot], NOT replace #modal-root, so the dialog stays open.",
        );
    assert!(
        dialog.is_displayed().await.expect("edit dialog displayed?"),
        "the edit dialog is in the DOM but not displayed after the rejected save"
    );
}

/// S5 — the rejected edit mutated NOTHING. Asserted against the REAL store (the
/// authoritative source): a bug that persisted the whitespace title would show a
/// changed row here. The board's card DOM is checked too, since a mis-routed OOB
/// swap could have rewritten the card even without a store change.
#[then(regex = r#"^the "([^"]+)" card still shows "([^"]+)"$"#)]
async fn card_still_shows(world: &mut FoundryWorld, key: String, title: String) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let (prefix, _) = key.rsplit_once('-').expect("issue key has -N");
    let stored: (String,) = sqlx::query_as(
        "SELECT i.title FROM issues i \
           JOIN projects p ON p.id = i.project_id \
          WHERE p.key_prefix = $1 AND i.number = $2",
    )
    .bind(prefix)
    .bind(number_of(&key))
    .fetch_one(pool)
    .await
    .unwrap_or_else(|e| panic!("read issue {key} from store: {e}"));
    assert_eq!(
        stored.0, title,
        "the rejected edit still mutated the stored title to {:?} — a save that failed validation \
         must write nothing.",
        stored.0
    );

    let browser = world.browser.as_ref().expect("browser session");
    let card_selector = format!(".board [data-issue-key='{key}']");
    let card = browser
        .find(Locator::Css(card_selector.as_str()))
        .await
        .unwrap_or_else(|_| {
            panic!("the {key} card is gone from the board after the rejected edit")
        });
    let text = card.text().await.unwrap_or_default();
    assert!(
        text.contains(&title),
        "the {key} card no longer shows {title:?} after the rejected edit (card text {text:?})"
    );
}

// --- Slice 02 (S6): comment-edit form joins the contract --------------------

/// S6's seed: insert issue GEN-1 into the EXISTING Sandbox project AND a comment
/// by Mei on it, so the issue page renders a comment card with an Edit button.
/// Runs BEFORE the browser Given, so both rows are in the DB by page load. Uses
/// the store pool directly (like `sandbox_has_issue`) — no HTTP round-trip.
#[given(regex = r#"^the "Sandbox" project has an issue "([^"]+)" with a comment by Mei$"#)]
async fn sandbox_issue_with_comment(world: &mut FoundryWorld, issue_key: String) {
    let harness = world
        .harness
        .as_ref()
        .expect("the HTTP Background must have spawned harness");
    let pool = harness.app.state.store.pool();
    let project: (uuid::Uuid, uuid::Uuid) =
        sqlx::query_as("SELECT id, workspace_id FROM projects WHERE key_prefix = $1")
            .bind(PROJECT_KEY_PREFIX)
            .fetch_one(pool)
            .await
            .expect("fetch the Sandbox project seeded by the Background");
    let author: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE email_lower = $1")
        .bind(MEI_EMAIL)
        .fetch_one(pool)
        .await
        .expect("fetch Mei");
    let issue_id = uuid::Uuid::now_v7();
    // board-lane-management sweep: 0015 dropped the state DEFAULT — INSERT it.
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, description_md, state, author_id)
              VALUES ($1, $2, $3, $4, $5, '', 'backlog', $6)",
    )
    .bind(issue_id)
    .bind(project.0)
    .bind(project.1)
    .bind(number_of(&issue_key))
    .bind("Keep me")
    .bind(author.0)
    .execute(pool)
    .await
    .expect("insert the Sandbox issue");
    sqlx::query(
        "INSERT INTO comments (id, workspace_id, issue_id, author_id, body_markdown, body_html)
              VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(project.1)
    .bind(issue_id)
    .bind(author.0)
    .bind(ORIGINAL_COMMENT_BODY)
    .bind(format!("<p>{ORIGINAL_COMMENT_BODY}</p>"))
    .execute(pool)
    .await
    .expect("insert Mei's comment on the Sandbox issue");
}

/// S6's precondition: open the browser view on the GEN-1 issue page (not the
/// board). Same session dance as the board Given, but navigates to the issue
/// detail URL so the seeded comment card + its Edit button render.
#[given(regex = r#"^Mei is viewing the "([^"]+)" issue page in a real browser$"#)]
async fn viewing_issue_page(world: &mut FoundryWorld, issue_key: String) {
    let browser = browser_harness::new_session().await;
    {
        let harness = world
            .harness
            .as_ref()
            .expect("the HTTP Background must have spawned harness");
        browser_harness::sign_in_through_browser(&browser, harness, MEI_EMAIL, MEI_PASSWORD).await;
        let url = format!(
            "{}/team/{TEAM_SLUG}/project/{PROJECT_SLUG}/issues/{}",
            harness.base_url(),
            number_of(&issue_key)
        );
        browser
            .goto(&url)
            .await
            .expect("navigate to the GEN-1 issue page");
    }
    world.browser = Some(browser);
}

/// S6's action: click the shipped Edit button to swap in the comment-edit form,
/// blank the pre-filled textarea to a server-empty value, and Save. As in slice
/// 01, "empty" is ONE SPACE: the textarea carries native `required`, so a truly
/// cleared field would halt the POST (`validation:halted`) and no 400 would
/// exist. One space passes `required`, htmx PATCHes, and the edit endpoint's
/// `trim()` makes it empty → the byte-identical 400 "Comment cannot be empty"
/// fragment routed through the beforeSwap handler.
#[when(regex = r"^Mei edits that comment to an empty body and saves$")]
async fn edits_comment_to_empty_and_saves(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    settle_ready(browser).await;
    browser
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css(COMMENT_EDIT_BUTTON_SELECTOR))
        .await
        .expect("the issue page must render the seeded comment's Edit button")
        .click()
        .await
        .expect("click the comment Edit button");
    browser
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css(COMMENT_EDIT_TEXTAREA_SELECTOR))
        .await
        .expect("clicking Edit must swap in the comment-edit form with a textarea");
    // Blank the pre-filled body to a server-empty (whitespace) value. WebDriver
    // clear() proved unreliable on this pre-filled <textarea> under chromedriver
    // (it left the original text), so set the value deterministically via JS —
    // htmx reads the live .value at submit time. The net input is ONE SPACE:
    // browser-valid (passes native `required`), server-empty after trim() → the
    // 400 "Comment cannot be empty" fragment.
    browser
        .execute(
            "document.querySelector(\".comment-edit-form textarea[name='body_markdown']\").value = ' ';",
            vec![],
        )
        .await
        .expect("blank the comment body to a single space");
    browser
        .find(Locator::Css(COMMENT_SAVE_BUTTON_SELECTOR))
        .await
        .expect("the comment-edit form must carry a Save button")
        .click()
        .await
        .expect("submit the comment-edit form");
}

/// S6 — THE ORACLE, scoped to the comment-edit form's slot. Bounded-polls until
/// the slot both DISPLAYS and CONTAINS the server message. Against a tree where
/// comment_edit_form.html has no `[data-error-slot]`, htmx 2.0.4 discards the
/// 400 and this never holds — the reproduction that proves the slot is
/// load-bearing PER FORM even though the handler is unchanged from slice 01.
#[then(regex = r"^the validation error is visible next to the comment$")]
async fn comment_validation_error_visible(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let deadline = Instant::now() + WAIT_TIMEOUT;
    loop {
        if let Ok(err) = browser.find(Locator::Css(COMMENT_ERROR_SELECTOR)).await {
            let displayed = err.is_displayed().await.unwrap_or(false);
            let text = err.text().await.unwrap_or_default();
            if displayed && text.contains(COMMENT_EMPTY_MESSAGE) {
                return;
            }
        }
        if Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let err_state = match browser.find(Locator::Css(COMMENT_ERROR_SELECTOR)).await {
        Ok(err) => format!(
            "error element present, displayed={:?}, text={:?}",
            err.is_displayed().await.unwrap_or(false),
            err.text().await.unwrap_or_default()
        ),
        Err(_) => "no error rendered in the comment-edit form at all".to_string(),
    };
    panic!(
        "the validation error {COMMENT_EMPTY_MESSAGE:?} never became visible next to the comment \
         ({err_state}). The edit endpoint returns a byte-identical 400 + \"{COMMENT_EMPTY_MESSAGE}\" \
         fragment; without a [data-error-slot] in comment_edit_form.html the generic handler finds \
         no slot in THIS form, htmx 2.0.4 discards the 400, and Mei sees nothing — the per-form \
         defect this step closes."
    );
}

/// S6 — the rejected edit persisted NOTHING. Asserted against the REAL store
/// (the authoritative source): a bug that saved the whitespace body would show a
/// changed row here. Because nothing was persisted, re-rendering the issue page
/// would still show the original comment text — that is "the comment still shows
/// its original text".
#[then(regex = r"^the comment still shows its original text$")]
async fn comment_still_shows_original(world: &mut FoundryWorld) {
    let harness = world.harness.as_ref().expect("harness");
    let pool = harness.app.state.store.pool();
    let stored: (String,) = sqlx::query_as(
        "SELECT c.body_markdown FROM comments c \
           JOIN issues i ON i.id = c.issue_id \
           JOIN projects p ON p.id = i.project_id \
          WHERE p.key_prefix = $1",
    )
    .bind(PROJECT_KEY_PREFIX)
    .fetch_one(pool)
    .await
    .expect("read the seeded comment from the store");
    assert_eq!(
        stored.0, ORIGINAL_COMMENT_BODY,
        "the rejected comment edit still mutated the stored body to {:?} — a save that failed \
         validation must write nothing.",
        stored.0
    );
}
