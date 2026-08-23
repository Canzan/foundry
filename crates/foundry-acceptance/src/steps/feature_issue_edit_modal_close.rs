//! issue-edit-modal-close-icon — the `@needs-browser` lane's step definitions
//! for the edit dialog's close (×) control.
//!
//! STEP 01-01 (render-only, D-12/D-13/D-14): the close control EXISTS in the
//! rendered edit-dialog header — visible top right, accessibly named "Close",
//! with a >= 24×24 CSS px click target. NOTHING listens yet (the delegated
//! click trigger is step 01-02), so this module carries only the S1 Then
//! assertions; the close-click behaviour steps land with the wiring.
//!
//! WHY ITS OWN STEP MODULE: one `feature_*.rs` per feature (the
//! feature_pwa_mobile / feature_form_error_display convention) — cucumber-rs
//! step text binds GLOBALLY, so each feature owns only its new phrases.
//! REUSED SHIPPED PHRASES (defined elsewhere, deliberately NOT redefined here —
//! a second matching regex would be a cucumber ambiguity error):
//!   - the Background lines (workspace / project / issue seed / sign-in) —
//!     feature_board_new_issue.rs + feature_form_error_display.rs + us_06/us_07;
//!   - `Mei is viewing the "Sandbox" board in a real browser` —
//!     feature_form_error_display.rs:373 (opens the fantoccini session on the
//!     shared in-process origin and stores it in `world.browser`);
//!   - `Mei opens the edit dialog for "…"` — feature_issue_edit_dialog.rs:198,
//!     the HTTP-lane capture_get. It binds S1's When and proves the fragment
//!     route serves authed, but it CANNOT touch the browser session. The first
//!     Then below therefore mounts the dialog in the REAL browser (kb-ready
//!     wait per D-15, then the shipped card-click affordance: the whole
//!     `.issue-card` carries `hx-get="{edit_url}"` into `#modal-root`) before
//!     asserting — the browser-lane open the globally-bound When can't do.

use crate::support::browser_harness;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};
use fantoccini::Locator;
use std::time::{Duration, Instant};

/// The mounted edit dialog (issue_edit_modal.html:1) inside the modal host.
const EDIT_DIALOG_SELECTOR: &str = "#modal-root [data-modal='edit-issue']";
/// The close control THIS feature ships (D-12): a declarative
/// `[data-action="close-modal"]` button inside the dialog header.
const CLOSE_CONTROL_SELECTOR: &str =
    "#modal-root [data-modal='edit-issue'] .modal-header [data-action='close-modal']";

const WAIT_TIMEOUT: Duration = Duration::from_secs(10);

/// [`CLOSE_CONTROL_SELECTOR`] marshalled as the one-element `arguments` vector
/// the module's `browser.execute` probes take — one definition, three probes.
fn close_control_js_args() -> Vec<serde_json::Value> {
    vec![serde_json::Value::String(
        CLOSE_CONTROL_SELECTOR.to_string(),
    )]
}

/// Sandbox-board session identities — the SAME values the shipped Background
/// seeds. A sibling copy of feature_form_error_display.rs:48-51 on purpose:
/// step modules are peers, not a dependency tree, and its helper is private.
const MEI_EMAIL: &str = "mei@acme.com";
const MEI_PASSWORD: &str = "mei-correct-horse-battery-staple";
const TEAM_SLUG: &str = "backend";
const PROJECT_SLUG: &str = "sandbox";

/// Mount the `issue_key` edit dialog in the REAL browser session opened by the
/// shipped board Given. Idempotent: if the dialog is already in the DOM the
/// click is skipped, so every S1 Then can call this and only the first pays.
/// Per D-15 (Earned Trust) waits on the shipped `[data-kb-ready]` marker
/// before interacting — attachment proven, not assumed.
async fn ensure_edit_dialog_open_in_browser(world: &mut FoundryWorld, issue_key: &str) {
    let browser = world
        .browser
        .as_ref()
        .expect("the board Given must have opened a real browser session");
    if browser
        .find(Locator::Css(EDIT_DIALOG_SELECTOR))
        .await
        .is_ok()
    {
        return;
    }
    browser_harness::wait_for_kb_ready(browser).await;
    let card_selector = format!("[data-issue-key='{issue_key}']");
    browser
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css(card_selector.as_str()))
        .await
        .unwrap_or_else(|_| panic!("the board must render the {issue_key} card"))
        .click()
        .await
        .unwrap_or_else(|_| {
            panic!("click the {issue_key} card (the shipped hx-get edit affordance)")
        });
    browser
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css(EDIT_DIALOG_SELECTOR))
        .await
        .unwrap_or_else(|_| {
            panic!("clicking the {issue_key} card must swap the edit dialog into #modal-root")
        });
}

/// S2/S6's one-line precondition: a real browser on the Sandbox board WITH the
/// edit dialog already mounted. Opens the session lazily (sign-in + board, the
/// feature_form_error_display.rs:160 shape) when the scenario carries no board
/// Given of its own, then rides the idempotent mount above. Shared by the
/// opened-dialog Given and S3's focused-control Given.
async fn open_board_with_edit_dialog(world: &mut FoundryWorld, issue_key: &str) {
    if world.browser.is_none() {
        let browser = browser_harness::new_session().await;
        {
            let harness = world
                .harness
                .as_ref()
                .expect("the HTTP Background must have spawned harness");
            browser_harness::sign_in_through_browser(&browser, harness, MEI_EMAIL, MEI_PASSWORD)
                .await;
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
    ensure_edit_dialog_open_in_browser(world, issue_key).await;
}

#[given(regex = r#"^Mei has opened the edit dialog for "([^"]+)" in a real browser$"#)]
async fn has_opened_edit_dialog(world: &mut FoundryWorld, issue_key: String) {
    open_board_with_edit_dialog(world, &issue_key).await;
}

/// S2's When (AC-1.2): ONE real click on the rendered ×. The WebDriver click
/// lands on the element's centre — which may be a glyph child — exactly the
/// event target the delegated listener's `closest()` must resolve (D-10).
///
/// The click CARRIES its own close proof (the module's bounded host-empty
/// wait): every scenario that clicks the × expects the one close mechanism to
/// act, but the globally-bound `Then the dialog closes`
/// (feature_form_error_display.rs:590) polls the NEW-ISSUE selector and is
/// vacuous for the edit dialog — without this wait, the S4 error-state
/// scenario (whose remaining Thens all bind globally) would stay green over a
/// × that traps Mei in the validation-error state (AC-1.4).
#[when(regex = r"^Mei clicks the close control$")]
async fn clicks_close_control(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .find(Locator::Css(CLOSE_CONTROL_SELECTOR))
        .await
        .expect("the edit dialog must render its close control (step 01-01)")
        .click()
        .await
        .expect("click the close control");
    wait_for_modal_host_empty(
        browser,
        "the click on the close control left the edit dialog mounted — the delegated \
         [data-action=\"close-modal\"] listener (D-10) did not resolve the click to \
         keyboard.js::closeModal(), so the × renders but cannot close (AC-1.2/AC-1.4).",
    )
    .await;
}

/// Bounded-poll until the modal host is EMPTY — the one close mechanism,
/// keyboard.js::closeModal(), empties #modal-root, so an empty host IS "the
/// dialog closed". Shared by the S2/S3 after-state assertions; the close is
/// synchronous today, but the oracle is never a timing assumption.
///
/// NOTE (S2, deliberate): the globally-bound `Then the dialog closes`
/// (feature_form_error_display.rs:590) polls the NEW-ISSUE selector and is
/// therefore vacuous for the EDIT dialog — every scenario here that needs the
/// close proven carries it through THIS wait instead.
async fn wait_for_modal_host_empty(browser: &fantoccini::Client, diagnosis: &str) {
    let deadline = Instant::now() + WAIT_TIMEOUT;
    loop {
        let still_open = browser
            .execute(
                "var host = document.getElementById('modal-root');\
                 return !!host && host.childElementCount > 0;",
                vec![],
            )
            .await
            .expect("ask whether the modal host still holds a dialog");
        if still_open == serde_json::Value::Bool(false) {
            return;
        }
        if Instant::now() >= deadline {
            panic!("{diagnosis}");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// S2 — the after-state Esc leaves (AC-1.2): the modal host is EMPTY and the
/// board underneath is visible again.
#[then(regex = r"^the board is interactive again$")]
async fn board_interactive_again(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    wait_for_modal_host_empty(
        browser,
        "the edit dialog is still open — the activation of the close control did nothing. No \
         delegated click listener resolves [data-action=\"close-modal\"] to \
         keyboard.js::closeModal() (D-10, adr-modal-close-001), so the × renders but \
         cannot close, and Mei's only way out is still the unadvertised Esc key.",
    )
    .await;
    let card = browser
        .find(Locator::Css(".board [data-issue-key]"))
        .await
        .expect("the board must still hold its issue cards after the close");
    assert!(
        card.is_displayed().await.expect("card displayed?"),
        "the modal host is empty but the board's cards are not visible — the close must return \
         Mei to an interactive board, not a blank page."
    );
}

/// One real keystroke to whatever holds focus — the same path a human's
/// keypress takes to the ADR-001 document-delegated listener (the shipped
/// keypress idiom, now the shared W3C-key-actions dispatch).
async fn press(browser: &fantoccini::Client, key: &str) {
    browser_harness::press_key(browser, key).await;
}

/// The GEN-1 card carries the selection ring — `aria-selected="true"`, the
/// ADR-006 state the ring's CSS hook rides, asserted with a bounded wait.
async fn assert_gen1_selected(browser: &fantoccini::Client, when: &str) {
    browser
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css(
            ".board [data-issue-key='GEN-1'][aria-selected='true']",
        ))
        .await
        .unwrap_or_else(|_| {
            panic!(
                "{when}, the GEN-1 card carries no selection — the document-delegated keydown \
                 layer stopped acting after the ×-close (AC-1.5, Esc parity)."
            )
        });
}

/// S6 — Esc-parity of the after-state (AC-1.5, D-11). The dialog's autofocused
/// title field left with the host, so `document.activeElement` falls to BODY —
/// the SAME observable today's Esc close produces. Deliberately NOT "focus is
/// back on the triggering card": restore is DEFERRED (D-11), and if it ever
/// lands it lands inside closeModal() so BOTH triggers get it.
#[then(regex = r"^focus rests on a live element, just as an Esc close leaves it$")]
async fn focus_rests_on_live_element(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let state = browser
        .execute(
            "var el = document.activeElement;\
             return [!!el && el.isConnected === true, el === document.body];",
            vec![],
        )
        .await
        .expect("read the active element after the close");
    let arr = state.as_array().expect("[connected, is_body]");
    assert_eq!(
        arr[0].as_bool(),
        Some(true),
        "after the ×-close, document.activeElement is null or disconnected — focus died with the \
         dialog instead of resting on a live element (AC-1.5)."
    );
    assert_eq!(
        arr[1].as_bool(),
        Some(true),
        "after the ×-close, focus rests somewhere other than <body> — Esc parity (D-11) pins the \
         after-state to what closeTopLayer() leaves: the focused field goes away with the host \
         and activeElement falls to body."
    );
}

/// S6 — the document-delegated shortcut layer still ACTS after the ×-close
/// (AC-1.5): `j` selects the first visible card (GEN-1 is the board's only
/// one), `k` on the first card is the bounded no-op that leaves it selected —
/// both through the SAME keydown listener the new click listener sits beside,
/// from the body focus the close left.
#[then(regex = r#"^pressing "j" and then "k" still moves the card selection$"#)]
async fn j_then_k_still_move_selection(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    press(browser, "j").await;
    assert_gen1_selected(browser, "after pressing \"j\"").await;
    press(browser, "k").await;
    assert_gen1_selected(browser, "after pressing \"k\" (bounded at the first card)").await;
}

/// S6 — `c` still opens the new-issue dialog: the shipped shortcut clicks the
/// board's own trigger and htmx swaps the new-issue modal into the just-emptied
/// #modal-root. The board Mei got back is fully live, not merely visible.
#[then(regex = r#"^pressing "c" still opens the new-issue dialog$"#)]
async fn c_still_opens_new_issue(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    press(browser, "c").await;
    browser
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css("#modal-root [data-modal='new-issue']"))
        .await
        .unwrap_or_else(|_| {
            panic!(
                "pressing \"c\" after the ×-close did not open the new-issue dialog — the \
                 keyboard layer must still act on the board the close returned (AC-1.5)."
            )
        });
}

/// S1 — the close control exists in the header and sits in its TOP RIGHT.
/// Positional oracle: the control's right edge hugs the header's right edge
/// (within 16px — padding tolerance) and its box lies inside the header's
/// vertical band. A header with no `[data-action="close-modal"]` fails with
/// the business diagnosis: no close control in the rendered header.
#[then(regex = r"^the dialog shows a close control in the top right of its header$")]
async fn close_control_in_top_right(world: &mut FoundryWorld) {
    ensure_edit_dialog_open_in_browser(world, "GEN-1").await;
    let browser = world.browser.as_ref().expect("browser session");
    let rects = browser
        .execute(
            "var h = document.querySelector('#modal-root [data-modal=\"edit-issue\"] .modal-header');\
             if (!h) { throw new Error('the edit dialog has no .modal-header'); }\
             var c = h.querySelector('[data-action=\"close-modal\"]');\
             if (!c) { return null; }\
             var hr = h.getBoundingClientRect();\
             var cr = c.getBoundingClientRect();\
             return [hr.top, hr.right, hr.bottom, cr.top, cr.right, cr.bottom];",
            vec![],
        )
        .await
        .expect("measure the header and its close control");
    let arr = rects.as_array().unwrap_or_else(|| {
        panic!(
            "no close control in the rendered header — the edit dialog's .modal-header carries no \
             [data-action=\"close-modal\"] control (D-12). Mei has no visible way out of the \
             dialog; the unadvertised Esc key is the only exit."
        )
    });
    let (h_top, h_right, h_bottom) = (
        arr[0].as_f64().unwrap(),
        arr[1].as_f64().unwrap(),
        arr[2].as_f64().unwrap(),
    );
    let (c_top, c_right, c_bottom) = (
        arr[3].as_f64().unwrap(),
        arr[4].as_f64().unwrap(),
        arr[5].as_f64().unwrap(),
    );
    assert!(
        (h_right - c_right) <= 16.0,
        "the close control is not pinned to the header's right (control right edge {c_right}px, \
         header right edge {h_right}px) — .modal-header must lay out title | full-page link | × \
         with the × pinned right (D-13)."
    );
    assert!(
        c_top >= h_top - 1.0 && c_bottom <= h_bottom + 1.0,
        "the close control sits outside the header's vertical band (control {c_top}..{c_bottom}, \
         header {h_top}..{h_bottom}) — the × must live in the top bar of the dialog, not below it."
    );
}

/// S1 — assistive technology hears "Close". The control is a text-glyph (×)
/// button (OD-3), so the accessible name MUST come from aria-label="Close";
/// the glyph alone would announce as "multiplication sign" or nothing.
#[then(regex = r#"^the close control is named "Close" for assistive technology$"#)]
async fn close_control_named_close(world: &mut FoundryWorld) {
    ensure_edit_dialog_open_in_browser(world, "GEN-1").await;
    let browser = world.browser.as_ref().expect("browser session");
    let control = browser
        .find(Locator::Css(CLOSE_CONTROL_SELECTOR))
        .await
        .expect("no close control in the rendered header to carry an accessible name");
    let label = control
        .attr("aria-label")
        .await
        .expect("read the close control's aria-label")
        .unwrap_or_default();
    assert_eq!(
        label, "Close",
        "the close control's accessible name is {label:?}, not \"Close\" — a screen reader \
         announces the raw × glyph (or nothing) instead of the control's purpose (AC-1.6)."
    );
}

/// S1 — the click target meets the WCAG 2.2 minimum (AC-1.6 / NFR-WEBB-A11Y-02):
/// at least 24×24 CSS px, so the way out is hittable, not a pixel-hunt.
#[then(regex = r"^the close control's click target is at least 24 by 24 pixels$")]
async fn close_control_target_size(world: &mut FoundryWorld) {
    ensure_edit_dialog_open_in_browser(world, "GEN-1").await;
    let browser = world.browser.as_ref().expect("browser session");
    let dims = browser
        .execute(
            "var c = document.querySelector(arguments[0]);\
             if (!c) { throw new Error('no close control in the rendered header'); }\
             var r = c.getBoundingClientRect();\
             return [r.width, r.height];",
            close_control_js_args(),
        )
        .await
        .expect("measure the close control's click target");
    let arr = dims.as_array().expect("[width, height]");
    let w = arr[0].as_f64().expect("width is a number");
    let h = arr[1].as_f64().expect("height is a number");
    assert!(
        w >= 24.0 && h >= 24.0,
        "the close control's click target is {w}×{h}px, below the 24×24 CSS px minimum — \
         .modal-close must guarantee the target size (D-13, WCAG 2.2 target-size minimum)."
    );
}

// ===== S3 — no mouse required (AC-1.3, AC-1.6) ==============================

/// The Tab-reach bound: the dialog holds a handful of tabbable controls and the
/// page under it a handful more, so a focus walk that hasn't landed on the ×
/// within this many presses means it is NOT reachable, not merely far away.
const MAX_TAB_PRESSES: usize = 50;

/// Walk REAL Tab presses (one keystroke each, the same body-dispatch path as
/// every press in this lane) until `document.activeElement` IS the close
/// control. The native-button freebie is ASSERTED, never assumed (DESIGN open
/// question 2): if the walk never lands, the × is out of the keyboard tab
/// order and the D-12 button contract is violated — a production finding, not
/// something to work around here.
async fn tab_focus_close_control(browser: &fantoccini::Client) {
    browser_harness::wait_for_kb_ready(browser).await;
    for _ in 0..MAX_TAB_PRESSES {
        let focused = browser
            .execute(
                "return document.activeElement === document.querySelector(arguments[0]);",
                close_control_js_args(),
            )
            .await
            .expect("ask whether the close control holds focus");
        if focused == serde_json::Value::Bool(true) {
            return;
        }
        press(browser, "Tab").await;
    }
    panic!(
        "{MAX_TAB_PRESSES} Tab presses never landed focus on the close control — the × is not in \
         the keyboard tab order (AC-1.3). A native <button> is focusable for free, so either the \
         control is not a real button (D-12 violated) or something steals focus back."
    );
}

/// S3's When (AC-1.3): Mei reaches the × by keyboard alone.
#[when(regex = r"^Mei moves focus to the close control with the Tab key$")]
async fn moves_focus_to_close_with_tab(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    tab_focus_close_control(browser).await;
}

/// S3's second Given: the open GEN-1 dialog with focus already walked onto the
/// × — the Space scenario's whole precondition in one line.
#[given(regex = r#"^Mei has moved focus to the close control of the open "([^"]+)" edit dialog$"#)]
async fn has_moved_focus_to_close_control(world: &mut FoundryWorld, issue_key: String) {
    open_board_with_edit_dialog(world, &issue_key).await;
    let browser = world.browser.as_ref().expect("browser session");
    tab_focus_close_control(browser).await;
}

/// S3 — keyboard focus is VISIBLE (AC-1.6): the control holds focus AND its
/// computed style shows the D-13 `:focus-visible` outline (2px solid accent).
/// Tab-driven focus matches `:focus-visible` in Chrome, so a missing outline
/// here means the indicator rule is gone or overridden — a sighted keyboard
/// user would be tabbing blind.
#[then(regex = r"^the close control shows a visible focus indicator$")]
async fn close_control_shows_focus_indicator(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    let state = browser
        .execute(
            "var c = document.querySelector(arguments[0]);\
             if (!c) { throw new Error('no close control in the rendered header'); }\
             var s = getComputedStyle(c);\
             return [document.activeElement === c, s.outlineStyle, s.outlineWidth];",
            close_control_js_args(),
        )
        .await
        .expect("read the close control's focus + outline state");
    let arr = state
        .as_array()
        .expect("[focused, outlineStyle, outlineWidth]");
    assert_eq!(
        arr[0].as_bool(),
        Some(true),
        "the close control does not hold focus when its indicator is asserted — the Tab walk's \
         landing must still be current (AC-1.3)."
    );
    let outline_style = arr[1].as_str().unwrap_or_default().to_string();
    let outline_width: f64 = arr[2]
        .as_str()
        .unwrap_or_default()
        .trim_end_matches("px")
        .parse()
        .unwrap_or(0.0);
    assert!(
        outline_style != "none" && outline_width > 0.0,
        "the focused close control shows no visible indicator (outline {outline_style} \
         {outline_width}px) — .modal-close:focus-visible must draw the 2px accent outline \
         (D-13, AC-1.6); a keyboard user cannot see where they are."
    );
}

// ===== S2 — a dismissal saves nothing (AC-1.2, OD-4) ========================

/// The edit dialog's title field (issue_edit_modal.html:6) — a sibling of
/// feature_form_error_display.rs:99 on purpose (peer modules, private consts).
const EDIT_TITLE_FIELD_SELECTOR: &str = "#modal-root [data-modal='edit-issue'] input[name='title']";

/// The GEN-1 row's full editable surface — title, description_md, state — read
/// at the store through the shipped testcontainers-Postgres pool, the SAME
/// oracle feature_issue_edit_dialog.rs:417 ships (extended by `state`, because
/// this scenario's universe is "everything a save could touch", not one field).
async fn read_issue_snapshot(world: &FoundryWorld, key: &str) -> (String, String, String) {
    let (prefix, number) = key.rsplit_once('-').expect("issue key has -N");
    let number: i32 = number.parse().expect("issue key ends in a number");
    let harness = world.harness.as_ref().expect("harness");
    sqlx::query_as(
        "SELECT i.title, i.description_md, i.state
           FROM issues i
           JOIN projects p ON p.id = i.project_id
          WHERE p.key_prefix = $1 AND i.number = $2",
    )
    .bind(prefix)
    .bind(number)
    .fetch_one(harness.app.state.store.pool())
    .await
    .unwrap_or_else(|e| panic!("read issue {key} from store: {e}"))
}

/// S2's dirty-form Given (AC-1.2): REAL keystrokes into the edit dialog's title
/// field — `send_keys` on the element, never a JS `.value` assignment, so the
/// form is dirty the way Mei's would be. BEFORE the first keystroke it arms the
/// no-save oracle's two halves:
///  1. the store BEFORE-image (title, description_md, state) stashed in the
///     page, the delta the store read-back Then compares against;
///  2. a request probe wrapping XMLHttpRequest.open + window.fetch — htmx
///     drives saves through XHR, so from here to "the dialog closed" every
///     request the page makes is on the record. It records, it does not
///     intercept: the wrapped natives still run.
#[given(regex = r#"^Mei has typed "([^"]+)" into the title field without saving$"#)]
async fn has_typed_into_title_without_saving(world: &mut FoundryWorld, text: String) {
    let (title, description, state) = read_issue_snapshot(world, "GEN-1").await;
    let browser = world.browser.as_ref().expect("browser session");
    browser_harness::wait_for_kb_ready(browser).await;
    browser
        .execute(
            "window.__closeIconBaseline = arguments[0];\
             window.__closeIconRequests = [];\
             var record = function (method, url) {\
               window.__closeIconRequests.push(\
                 String(method).toUpperCase() + ' ' + String(url));\
             };\
             var xhrOpen = XMLHttpRequest.prototype.open;\
             XMLHttpRequest.prototype.open = function (method, url) {\
               record(method, url);\
               return xhrOpen.apply(this, arguments);\
             };\
             var nativeFetch = window.fetch;\
             window.fetch = function (input, init) {\
               var url = input && input.url ? input.url : input;\
               var method = (init && init.method) || (input && input.method) || 'GET';\
               record(method, url);\
               return nativeFetch.apply(this, arguments);\
             };\
             return true;",
            vec![serde_json::json!([title, description, state])],
        )
        .await
        .expect("arm the no-save oracle (store baseline + request probe)");
    browser
        .find(Locator::Css(EDIT_TITLE_FIELD_SELECTOR))
        .await
        .expect("the edit dialog must carry a title field to type the discarded edit into")
        .send_keys(&text)
        .await
        .unwrap_or_else(|err| panic!("type {text:?} into the title field: {err}"));
}

/// S2's browser-side no-save arm (OD-4, first half). "While the dialog closed"
/// is load-bearing: this step first PROVES the close (modal host empty — the
/// globally-bound `the dialog closes` polls the new-issue selector and cannot),
/// then asserts the probe recorded nothing that could save: no non-GET request
/// anywhere, and no request AT ALL to an /issues/…/edit endpoint.
#[then(regex = r"^no save request was sent while the dialog closed$")]
async fn no_save_request_while_dialog_closed(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    wait_for_modal_host_empty(
        browser,
        "the edit dialog never closed after the click on the close control — there is no \
         \"while the dialog closed\" window to certify save-free (AC-1.2): the delegated \
         [data-action=\"close-modal\"] listener (D-10) did not act.",
    )
    .await;
    let recorded = browser
        .execute(
            "if (!window.__closeIconRequests) {\
               throw new Error('the request probe was never armed — the typed-Given must run first');\
             }\
             return window.__closeIconRequests;",
            vec![],
        )
        .await
        .expect("read the request probe");
    let requests: Vec<String> =
        serde_json::from_value(recorded).expect("the request probe records strings");
    let save_shaped: Vec<&String> = requests
        .iter()
        .filter(|entry| {
            let method = entry.split(' ').next().unwrap_or("");
            let url = entry.split(' ').nth(1).unwrap_or("");
            method != "GET" || (url.contains("/issues/") && url.contains("/edit"))
        })
        .collect();
    assert!(
        save_shaped.is_empty(),
        "the page sent {save_shaped:?} between the typed edit and the completed close — \
         dismissing the dialog via the close control must save NOTHING (AC-1.2): the × is a \
         discard, not a submit."
    );
}

/// S2's store read-back arm (OD-4, the load-bearing half): the FULL editable
/// row re-read at the store and compared against the before-image the Given
/// stashed — title included, so the universe is every slot a save could touch,
/// not just the two this step names.
#[then(regex = r"^its description and status are unchanged in the store$")]
async fn description_and_status_unchanged_in_store(world: &mut FoundryWorld) {
    let stashed = {
        let browser = world.browser.as_ref().expect("browser session");
        browser
            .execute(
                "if (!window.__closeIconBaseline) {\
                   throw new Error('no store baseline was stashed — the typed-Given must run first');\
                 }\
                 return window.__closeIconBaseline;",
                vec![],
            )
            .await
            .expect("read the stashed store before-image")
    };
    let baseline: Vec<String> =
        serde_json::from_value(stashed).expect("the baseline is [title, description, state]");
    let (title, description, state) = read_issue_snapshot(world, "GEN-1").await;
    assert_eq!(
        (title, description, state),
        (
            baseline[0].clone(),
            baseline[1].clone(),
            baseline[2].clone()
        ),
        "GEN-1's stored row changed across a ×-close that saved nothing on the wire — the close \
         path itself wrote to the store (AC-1.2). Compared (title, description_md, state) against \
         the before-image captured before the discarded keystrokes."
    );
}

// ===== S4 — the existing exits are untouched (AC-1.4, regression surface) ====
// Step 02-02 wires ONLY step phrases; form-errors.js, closeTopLayer(), the
// layer stack, and the save route are shipped behaviour under guard here.

/// The edit form's Save button — a sibling of feature_form_error_display.rs:102
/// on purpose (peer modules, private consts).
const EDIT_SAVE_BUTTON_SELECTOR: &str =
    "#modal-root [data-modal='edit-issue'] button[type='submit']";

/// The edit form's opt-in error slot form-errors.js routes a 4xx reason into —
/// a sibling of feature_form_error_display.rs:109.
const EDIT_ERROR_SLOT_SELECTOR: &str = "#modal-root [data-modal='edit-issue'] [data-error-slot]";

/// The dialog header's "Open full page" link (issue_edit_modal.html:7) — the
/// board card no longer navigates, so this link is the route to the full view.
const FULL_PAGE_LINK_SELECTOR: &str =
    "#modal-root [data-modal='edit-issue'] .modal-header a.full-page-link";

/// S4's save guard (When): a REAL edit through the dialog — clear the
/// pre-filled title, type the new one keystroke-by-keystroke, click the shipped
/// Save button. The htmx 2xx path empties #modal-root and OOB-replaces the
/// card; the × beside the Save button must not have disturbed any of it.
#[when(regex = r#"^Mei changes the title to "([^"]+)" and saves$"#)]
async fn changes_title_and_saves(world: &mut FoundryWorld, new_title: String) {
    let browser = world.browser.as_ref().expect("browser session");
    browser_harness::wait_for_kb_ready(browser).await;
    let title = browser
        .find(Locator::Css(EDIT_TITLE_FIELD_SELECTOR))
        .await
        .expect("the edit dialog must carry a pre-filled title field to change");
    title.clear().await.expect("clear the pre-filled title");
    title
        .send_keys(&new_title)
        .await
        .unwrap_or_else(|err| panic!("type {new_title:?} into the title field: {err}"));
    browser
        .find(Locator::Css(EDIT_SAVE_BUTTON_SELECTOR))
        .await
        .expect("the edit dialog must carry a Save button")
        .click()
        .await
        .expect("submit the edit dialog");
}

/// S4's saved-card oracle (Then): distinct from the shipped `card still shows`
/// (feature_form_error_display.rs:754, the NOTHING-changed guard) — this one
/// proves the save LANDED. First the close (modal host EMPTY via the module's
/// own wait — the globally-bound `the dialog closes` polls the new-issue
/// selector and is vacuous here), then the stored title, then the card DOM the
/// OOB swap rewrote, bounded-polled to survive swap latency.
#[then(regex = r#"^the "([^"]+)" card shows "([^"]+)"$"#)]
async fn card_shows_saved_title(world: &mut FoundryWorld, key: String, title: String) {
    {
        let browser = world.browser.as_ref().expect("browser session");
        wait_for_modal_host_empty(
            browser,
            "the edit dialog never closed after a valid save — the 2xx path must clear \
             #modal-root exactly as before the × landed beside the Save button (AC-1.4): the \
             close control is strictly additive.",
        )
        .await;
    }
    let (stored_title, _, _) = read_issue_snapshot(world, &key).await;
    assert_eq!(
        stored_title, title,
        "the save was accepted but the store holds {stored_title:?}, not {title:?} — saving from \
         the dialog must still persist the edit with the close control present (AC-1.4)."
    );
    let browser = world.browser.as_ref().expect("browser session");
    let card_selector = format!(".board [data-issue-key='{key}']");
    let deadline = Instant::now() + WAIT_TIMEOUT;
    loop {
        let card = browser
            .find(Locator::Css(card_selector.as_str()))
            .await
            .unwrap_or_else(|_| panic!("the {key} card is gone from the board after the save"));
        let text = card.text().await.unwrap_or_default();
        if text.contains(&title) {
            return;
        }
        if Instant::now() >= deadline {
            panic!(
                "the {key} card never showed the saved title {title:?} (card text {text:?}) — \
                 the OOB card replace of the save path must still act with the close control \
                 present (AC-1.4)."
            );
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// S4's error-state Given (AC-1.4): drive the shipped rejection path for real —
/// clear the title down to a SINGLE SPACE (the feature_form_error_display.rs:660
/// server-empty trick: a truly-empty field halts on native `required`, so no
/// 400 would exist; one space passes the browser, htmx POSTs, `title.trim()`
/// rejects) and Save, then hold until form-errors.js has routed the 400 reason
/// into the edit form's [data-error-slot] — Mei genuinely SEES the message
/// before the × is asked to get her out of this state.
#[given(regex = r#"^Mei has saved it with an empty title and sees "([^"]+)" inside the dialog$"#)]
async fn has_saved_empty_title_and_sees(world: &mut FoundryWorld, message: String) {
    let browser = world.browser.as_ref().expect("browser session");
    browser_harness::wait_for_kb_ready(browser).await;
    let title = browser
        .find(Locator::Css(EDIT_TITLE_FIELD_SELECTOR))
        .await
        .expect("the edit dialog must carry a title field to blank");
    title.clear().await.expect("clear the pre-filled title");
    title.send_keys(" ").await.expect("type a whitespace title");
    browser
        .find(Locator::Css(EDIT_SAVE_BUTTON_SELECTOR))
        .await
        .expect("the edit dialog must carry a Save button")
        .click()
        .await
        .expect("submit the edit dialog");
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
            panic!(
                "the rejected save never surfaced {message:?} inside the edit dialog — without \
                 the shipped form-errors.js routing there is no validation-error state to prove \
                 the × escapes from (AC-1.4)."
            );
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// S4's full-page guard (When): one REAL click on the "Open full page" link the
/// × now sits beside — a full navigation, not an htmx swap.
#[when(regex = r#"^Mei follows the "Open full page" link$"#)]
async fn follows_full_page_link(world: &mut FoundryWorld) {
    let browser = world.browser.as_ref().expect("browser session");
    browser
        .find(Locator::Css(FULL_PAGE_LINK_SELECTOR))
        .await
        .expect("the edit dialog header must carry its \"Open full page\" link beside the ×")
        .click()
        .await
        .expect("follow the full-page link");
}

/// S4's navigation oracle (Then): the browser LEFT the board for the issue's
/// own page — URL pinned to the shipped detail route (issues.rs:57,
/// `…/issues/{n}`) and the page's `<h1>` announcing the issue key
/// (issue.html:4). Bounded-polled: a link click is a real navigation.
#[then(regex = r#"^the browser shows the "([^"]+)" issue page$"#)]
async fn browser_shows_issue_page(world: &mut FoundryWorld, key: String) {
    let browser = world.browser.as_ref().expect("browser session");
    let (_, number) = key.rsplit_once('-').expect("issue key has -N");
    let expected_path = format!("/team/{TEAM_SLUG}/project/{PROJECT_SLUG}/issues/{number}");
    let deadline = Instant::now() + WAIT_TIMEOUT;
    loop {
        let url = browser
            .current_url()
            .await
            .expect("read the browser's current URL");
        if url.path() == expected_path {
            break;
        }
        if Instant::now() >= deadline {
            panic!(
                "the browser sits on {url} instead of the {key} issue page ({expected_path}) — \
                 the \"Open full page\" link beside the close control must still navigate \
                 (AC-1.4, D-12: the × is OUTSIDE the form and steals nothing from its \
                 neighbours)."
            );
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let heading = browser
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css("h1"))
        .await
        .unwrap_or_else(|_| panic!("the {key} issue page must render its <h1> heading"));
    let text = heading.text().await.unwrap_or_default();
    assert_eq!(
        text, key,
        "the issue page's heading reads {text:?}, not {key:?} — the full-page link led \
         somewhere other than the {key} detail view (issue.html:4)."
    );
}
