//! canzan-theme-system — the theme lane's step definitions.
//!
//! SCAFFOLD: true
//!
//! RED-READY SCAFFOLD, authored by DISTILL. Every step below panics with an
//! actionable message naming the step it stands for. Every scenario in
//! `tests/features/canzan-theme-system.feature` carries `@pending`, and
//! `acceptance.rs::filter_run` excludes `@pending` from EVERY lane — so this
//! module compiles and links but executes nothing until DELIVER un-pends a
//! scenario. Un-pend ONE AT A TIME, in slice order, starting with the oracle
//! probe. When a scenario is un-pended its steps must be RED (a panic from a
//! real assertion), never BROKEN (an undefined step). `fail_on_skipped()` is on:
//! an undefined step FAILS the run rather than silently skipping, which is the
//! property that stops this lane going green over nothing.
//!
//! ==========================================================================
//! DELIVER OBLIGATION 1 — THE DEVICE-PREFERENCE ORACLE (blocking, do first)
//! ==========================================================================
//!
//! `support/browser_harness.rs` today has NO way to give a session a device
//! colour preference. Without one, a "dark mode" scenario can only drive dark by
//! stamping an explicit choice on the document — and then the
//! `@media (prefers-color-scheme: dark)` block is GREEN WHETHER OR NOT IT
//! EXISTS, because the attribute selector alone satisfies the assertion. The
//! media path is the DEFAULT state, the one most operators get. That is
//! pwa-mobile-rendering's ADR-003 trap one layer over, and it is the reason this
//! obligation blocks the slice rather than trailing it.
//!
//! MECHANISM — empirically verified, twice. Inject `--force-dark-mode` into
//! `goog:chromeOptions.args` at SESSION CREATION, the same idiom
//! `open_mobile_session` already establishes for `mobileEmulation.deviceMetrics`.
//! Measured against raw headless Chrome (`--dump-dom`) and against chromedriver
//! 151.0.7922.138 over W3C `POST /session` + `execute/sync`:
//!
//! ```text
//!   flags: <none>                                  matchMedia=false  cssvar=LIGHT
//!   flags: --force-dark-mode                       matchMedia=true   cssvar=DARK
//!   flags: --enable-features=WebContentsForceDark  matchMedia=false  cssvar=LIGHT
//! ```
//!
//! BOTH the `matchMedia` result AND the computed CSS custom property flip, so the
//! media block genuinely applies — not merely the JS API reporting a preference.
//!
//! `--enable-features=WebContentsForceDark` measurably does NOT work. It is
//! Chrome's auto-darkening feature, a DIFFERENT thing. Do not "fix" the flag to
//! it: that would silently return this lane to green-over-nothing.
//!
//! NOT CDP. `POST /session/{id}/goog/cdp/execute` was considered and rejected.
//! fantoccini 0.21.5 does expose `Client::issue_cmd` (`session.rs:338`) and
//! `session_id` (`client.rs:110`), so CDP was reachable — recorded here so nobody
//! reopens this as a discovery. It was rejected on DETERMINISM, not availability:
//! a runtime call can race page load where a session capability cannot, and the
//! capability needs no side-channel HTTP client and no new dependency.
//!
//! HELPERS TO ADD, as siblings of `new_session` / `open_mobile_session`. The
//! existing `Scripting` enum gets a `ColorScheme` peer and `open_session` takes
//! both, so the four combinations the feature file needs are expressible:
//!
//! ```text
//!   new_session()                            light device, scripting on   (SHIPPED, unchanged)
//!   new_session_without_scripting()          light device, scripting off  (SHIPPED, unchanged)
//!   new_dark_session()                       dark  device, scripting on   (NEW)
//!   new_dark_session_without_scripting()     dark  device, scripting off  (NEW)
//!   device_prefers_dark(&client) -> bool     the anti-vacuity probe       (NEW)
//! ```
//!
//! ANTI-VACUITY GUARD, MANDATORY. The baseline is `false`/LIGHT, so the guard
//! discriminates. `device_prefers_dark` reads
//! `window.matchMedia('(prefers-color-scheme: dark)').matches` and every
//! dark-by-device Given asserts it BEFORE asserting anything about foundry's own
//! rendering. If the flag ever stops taking effect the guard fails loudly instead
//! of the suite silently measuring the light palette twice. The `@oracle-probe`
//! scenario asserts both arms — dark session true, default session false — so the
//! probe itself cannot pass vacuously.
//!
//! ==========================================================================
//! DELIVER OBLIGATION 2 — THE STORAGE-REFUSED SESSION (measured, resolved)
//! ==========================================================================
//!
//! MEASURED, not assumed. Chrome's site-data content setting
//! (`profile.default_content_setting_values.cookies = 2`, applied through
//! `goog:chromeOptions.prefs`) makes stored-state access throw, against a real
//! `http://` origin under chromedriver 151 — `file://` would not have exercised
//! content settings at all:
//!
//! ```text
//!   no prefs (baseline)                     READ=ok              WRITE=ok
//!   cookies=2                               READ=SecurityError   WRITE=SecurityError
//!   cookies=2 + block_third_party_cookies   READ=SecurityError   WRITE=SecurityError
//! ```
//!
//! BOTH arms throw, not just the write. For `theme.js` that means the stored-choice
//! read throws, its catch returns "follow the device", and the page themes from the
//! device — a genuine, observable behaviour. Build the helper with this pref; there
//! is no fallback to take and none is documented, because none is needed.
//!
//! THE WRITE GUARD HAS NO SCENARIO, AND CANNOT. Blocking site data also blocks the
//! session cookie, so no signed-in screen is reachable. And the control mounts only
//! at `.sidebar__user` (`templates/partials/sidebar.html:10`), inside
//! `partials/sidebar.html`, which is included by `templates/app_shell.html:4` and
//! nothing else. `templates/signin.html:1` extends `base.html`, as do 14 other
//! templates; only 11 extend `app_shell.html`. So under this pref every reachable
//! page has no toggle: "storage is refused" and "the control exists" are mutually
//! exclusive BY CONSTRUCTION, not by harness limitation.
//!
//! Do NOT stub a throwing storage accessor by script injection to get around this.
//! It would test the stub, not the browser, and it would be the only assertion in
//! this lane that does not exercise a real substrate. Filling storage to its quota
//! was also considered and rejected: it is a real exception, but quota semantics
//! vary by platform and a short value overwriting an existing short key may not
//! throw at all — a flaky oracle for a failure mode with no user-visible symptom.
//!
//! The read guard covers the path that has a real consequence (script dies at parse
//! time -> no theming anywhere). The write guard's only symptom is an uncaught error
//! in a console no operator reads. See feature-delta § Divergences.
//!
//! ==========================================================================
//! DELIVER OBLIGATION 3 — CONTRAST IS COMPUTED, NEVER RESTATED
//! ==========================================================================
//!
//! The contrast scenarios must COMPUTE the ratio from colours resolved in the
//! live browser: read the foreground, resolve the effective background by walking
//! ancestors for the first non-transparent value, convert to relative luminance,
//! and compare. They must NOT restate the six figures recorded in the token
//! comments — a test that asserts a human's arithmetic against itself proves
//! nothing, and KPI 3 explicitly requires re-verification in DELIVER.
//!
//! The ancestor walk is also WHY the opaque-surface rule exists: a translucent
//! tinted surface resolves to its unblended colour under that algorithm and reads
//! as a failure on a perfectly legible page. The dashboard scenario asserts the
//! project key chip is opaque for exactly this reason.
//!
//! ==========================================================================
//! KNOWN GAP — THE FIRST-FRAME COLOUR IS NOT OBSERVABLE HERE
//! ==========================================================================
//!
//! The flash scenario cannot sample the painted colours of the FIRST frame. Doing
//! that soundly needs a paint-level capture surface this suite deliberately does
//! not use. What it asserts instead is layered, and the layering is deliberate:
//!
//!   (a) LOAD-BEARING, deterministic — the theme script is fetched and executed
//!       before the browser is permitted to paint. A source-level fact: the tag
//!       sits in the head and carries no attribute that would defer it. Goes RED
//!       the instant the tag is moved to the foot of the body or given
//!       defer/async/module, which DISCUSS names as the single most likely
//!       regression in this feature.
//!   (b) SUPPORTING, measured — the script's fetch completed before the page's
//!       first contentful paint, read from the browser's own paint timing.
//!
//! (b) can pass BY LUCK on a fast loopback even with a deferred script, so it can
//! produce a false GREEN. It cannot produce a false RED. It is recorded as a
//! supporting measurement, not the guarantee. Do not promote it to load-bearing
//! and do not delete (a) in favour of it.
//!
//! ==========================================================================
//! WHY ITS OWN STEP MODULE
//! ==========================================================================
//!
//! The acceptance crate carries one `feature_*.rs` step module per feature.
//! cucumber-rs requires globally-unique step text, so each feature owns its
//! phrases. The Background lines are the SHIPPED HTTP-lane seed steps
//! (`feature_board_new_issue.rs`): they seed Acme / Mei / Backend / Sandbox and
//! spawn the ONE shared `InProcHarness`. The browser Givens below open a
//! fantoccini session against THAT SAME in-process origin, so both lanes exercise
//! one app — the same choice `feature_pwa_mobile.rs` made.

use crate::support::browser_harness;
use crate::world::FoundryWorld;
use cucumber::{given, then, when};

/// RED-scaffold marker. Panics — which the runner classifies as RED (the
/// implementation is missing), not BROKEN (the test is). Every step below stands
/// on this until DELIVER replaces it with a real assertion.
///
/// Panicking rather than returning is deliberate: a step that quietly returns
/// would let an un-pended scenario pass while asserting nothing, which is the
/// failure mode this whole lane is built to refuse.
fn scaffold(step: &str) -> ! {
    panic!(
        "__SCAFFOLD__ canzan-theme-system: step not yet implemented -- {step}\n  \
         DISTILL authored this scenario; DELIVER implements the step and un-pends \
         the scenario. See the module header for the three DELIVER obligations \
         (device-preference oracle, storage-refused session, computed contrast) \
         and the recorded first-frame gap."
    )
}

/// Open a DARK-DEVICE session and REFUSE to continue unless the browser agrees.
///
/// The guard is the load-bearing part, not the constructor. Nine scenarios (the
/// dark-by-device ones) carry no explicit Then-step re-assertion and rely
/// ENTIRELY on this: if `--force-dark-mode` ever stops taking effect, without the
/// guard all nine would pass while rendering LIGHT — the suite measuring the light
/// palette twice and calling it dark-mode coverage. So this fails the scenario
/// BEFORE staging anything.
#[given(regex = r"^a browser session whose device preference is dark$")]
async fn given_a_browser_session_whose_device_preference_is_dark(world: &mut FoundryWorld) {
    let browser = browser_harness::new_dark_session().await;
    assert!(
        browser_harness::device_prefers_dark(&browser).await,
        "the device-preference oracle is BROKEN: a session opened with --force-dark-mode \
         reports window.matchMedia('(prefers-color-scheme: dark)').matches == false, so the \
         @media (prefers-color-scheme: dark) block would never apply and every dark-by-device \
         scenario would silently measure the LIGHT palette. Do NOT weaken this assertion and do \
         NOT substitute --enable-features=WebContentsForceDark (Chrome's auto-darkening feature \
         — measured, it flips neither matchMedia nor the computed custom property). See \
         support/browser_harness.rs::ColorScheme."
    );
    world.browser = Some(browser);
}

#[given(regex = r"^the browser reports that its device prefers dark$")]
#[then(regex = r"^the browser reports that its device prefers dark$")]
async fn given_the_browser_reports_that_its_device_prefers_dark(world: &mut FoundryWorld) {
    let browser = world
        .browser
        .as_ref()
        .expect("a browser session must have been opened first");
    assert!(
        browser_harness::device_prefers_dark(browser).await,
        "the browser does not report a dark device preference"
    );
}

/// The SECOND arm of the probe, and the reason it cannot pass vacuously: the
/// baseline is `false`, so "dark session reports true" only means something
/// alongside "unstated session reports false". Without this arm a `matchMedia`
/// shim that always answered `true` would satisfy the first arm.
#[then(
    regex = r"^a browser session with no stated device preference reports that it prefers light$"
)]
async fn then_a_browser_session_with_no_stated_device_preference_reports(
    _world: &mut FoundryWorld,
) {
    let browser = browser_harness::new_session().await;
    let prefers_dark = browser_harness::device_prefers_dark(&browser).await;
    assert!(
        !prefers_dark,
        "a session with NO stated device preference reports it prefers dark — the oracle no \
         longer discriminates, so a dark assertion would hold against any session and every \
         dark-by-device scenario would be green over nothing"
    );
}

#[given(regex = r#"^the operator opens the "Sandbox" board$"#)]
#[when(regex = r#"^the operator opens the "Sandbox" board$"#)]
async fn given_the_operator_opens_the_sandbox_board(_world: &mut FoundryWorld) {
    scaffold("the operator opens the \"Sandbox\" board");
}

#[then(
    regex = r"^the page frame, the rail, the lane columns and every issue card render in the dark palette$"
)]
async fn then_the_page_frame_the_rail_the_lane_columns_and(_world: &mut FoundryWorld) {
    scaffold("the page frame, the rail, the lane columns and every issue card render in the dark palette");
}

#[then(regex = r"^no surface on the screen renders in a light-palette colour$")]
async fn then_no_surface_on_the_screen_renders_in_a_light(_world: &mut FoundryWorld) {
    scaffold("no surface on the screen renders in a light-palette colour");
}

#[given(regex = r"^a browser session whose device preference is light$")]
async fn given_a_browser_session_whose_device_preference_is_light(_world: &mut FoundryWorld) {
    scaffold("a browser session whose device preference is light");
}

#[then(regex = r"^the page renders on canzan's paper background with canzan's jade accent$")]
async fn then_the_page_renders_on_canzan_s_paper_background_with(_world: &mut FoundryWorld) {
    scaffold("the page renders on canzan's paper background with canzan's jade accent");
}

#[then(regex = r"^foundry's former blue and indigo accents appear nowhere on the screen$")]
async fn then_foundry_s_former_blue_and_indigo_accents_appear_nowhere(_world: &mut FoundryWorld) {
    scaffold("foundry's former blue and indigo accents appear nowhere on the screen");
}

#[given(regex = r"^the operator has already chosen the dark theme$")]
async fn given_the_operator_has_already_chosen_the_dark_theme(_world: &mut FoundryWorld) {
    scaffold("the operator has already chosen the dark theme");
}

#[then(regex = r"^the board renders in the dark palette$")]
async fn then_the_board_renders_in_the_dark_palette(_world: &mut FoundryWorld) {
    scaffold("the board renders in the dark palette");
}

#[given(regex = r"^the operator has already chosen the light theme$")]
async fn given_the_operator_has_already_chosen_the_light_theme(_world: &mut FoundryWorld) {
    scaffold("the operator has already chosen the light theme");
}

#[then(regex = r"^the board renders in the light palette$")]
async fn then_the_board_renders_in_the_light_palette(_world: &mut FoundryWorld) {
    scaffold("the board renders in the light palette");
}

#[when(regex = r"^the operator selects an issue card with the keyboard$")]
async fn when_the_operator_selects_an_issue_card_with_the_keyboard(_world: &mut FoundryWorld) {
    scaffold("the operator selects an issue card with the keyboard");
}

#[then(regex = r"^the selected card carries the selection ring as an outline$")]
async fn then_the_selected_card_carries_the_selection_ring_as_an(_world: &mut FoundryWorld) {
    scaffold("the selected card carries the selection ring as an outline");
}

#[then(regex = r"^the ring is present in the light palette as well as the dark one$")]
async fn then_the_ring_is_present_in_the_light_palette_as(_world: &mut FoundryWorld) {
    scaffold("the ring is present in the light palette as well as the dark one");
}

#[then(regex = r"^every body-size text pair on the board and rail reaches at least 4\.5 to 1$")]
async fn then_every_body_size_text_pair_on_the_board_and(_world: &mut FoundryWorld) {
    scaffold("every body-size text pair on the board and rail reaches at least 4.5 to 1");
}

#[then(regex = r"^every large-text and control-boundary pair reaches at least 3 to 1$")]
async fn then_every_large_text_and_control_boundary_pair_reaches_at(_world: &mut FoundryWorld) {
    scaffold("every large-text and control-boundary pair reaches at least 3 to 1");
}

#[then(regex = r"^the same holds when the device preference is light$")]
async fn then_the_same_holds_when_the_device_preference_is_light(_world: &mut FoundryWorld) {
    scaffold("the same holds when the device preference is light");
}

#[then(regex = r"^every semantic surface the board is built from is still present$")]
async fn then_every_semantic_surface_the_board_is_built_from_is(_world: &mut FoundryWorld) {
    scaffold("every semantic surface the board is built from is still present");
}

#[then(regex = r"^every lane column still declares which lane it is$")]
async fn then_every_lane_column_still_declares_which_lane_it_is(_world: &mut FoundryWorld) {
    scaffold("every lane column still declares which lane it is");
}

#[then(regex = r"^every issue card still declares which issue it is$")]
async fn then_every_issue_card_still_declares_which_issue_it_is(_world: &mut FoundryWorld) {
    scaffold("every issue card still declares which issue it is");
}

#[given(regex = r#"^the operator requests the "Sandbox" board page$"#)]
async fn given_the_operator_requests_the_sandbox_board_page(_world: &mut FoundryWorld) {
    scaffold("the operator requests the \"Sandbox\" board page");
}

#[then(regex = r"^the page states one brand colour for a light device and another for a dark one$")]
async fn then_the_page_states_one_brand_colour_for_a_light(_world: &mut FoundryWorld) {
    scaffold("the page states one brand colour for a light device and another for a dark one");
}

#[then(regex = r"^both brand colours are canzan contract values$")]
async fn then_both_brand_colours_are_canzan_contract_values(_world: &mut FoundryWorld) {
    scaffold("both brand colours are canzan contract values");
}

#[then(
    regex = r"^the installable app description still declares its brand and background colours$"
)]
async fn then_the_installable_app_description_still_declares_its_brand_and(
    _world: &mut FoundryWorld,
) {
    scaffold("the installable app description still declares its brand and background colours");
}

#[when(regex = r"^the operator opens the keyboard shortcut list$")]
async fn when_the_operator_opens_the_keyboard_shortcut_list(_world: &mut FoundryWorld) {
    scaffold("the operator opens the keyboard shortcut list");
}

#[then(regex = r"^the list, its keycaps and the layer behind it all render in the dark palette$")]
async fn then_the_list_its_keycaps_and_the_layer_behind_it(_world: &mut FoundryWorld) {
    scaffold("the list, its keycaps and the layer behind it all render in the dark palette");
}

#[then(
    regex = r"^the shortcut text and the keycap text each reach at least 4\.5 to 1 against the surface behind them$"
)]
async fn then_the_shortcut_text_and_the_keycap_text_each_reach(_world: &mut FoundryWorld) {
    scaffold("the shortcut text and the keycap text each reach at least 4.5 to 1 against the surface behind them");
}

#[when(regex = r"^the operator opens the dashboard$")]
async fn when_the_operator_opens_the_dashboard(_world: &mut FoundryWorld) {
    scaffold("the operator opens the dashboard");
}

#[then(
    regex = r"^the project cards, the section labels and the action controls render in the dark palette$"
)]
async fn then_the_project_cards_the_section_labels_and_the_action(_world: &mut FoundryWorld) {
    scaffold(
        "the project cards, the section labels and the action controls render in the dark palette",
    );
}

#[then(regex = r"^the project key chip sits on an opaque surface, not a translucent one$")]
async fn then_the_project_key_chip_sits_on_an_opaque_surface(_world: &mut FoundryWorld) {
    scaffold("the project key chip sits on an opaque surface, not a translucent one");
}

#[when(regex = r"^the operator opens the new-issue dialog$")]
async fn when_the_operator_opens_the_new_issue_dialog(_world: &mut FoundryWorld) {
    scaffold("the operator opens the new-issue dialog");
}

#[then(
    regex = r"^the dialog, its label, its text field and the layer behind it all render in the dark palette$"
)]
async fn then_the_dialog_its_label_its_text_field_and_the(_world: &mut FoundryWorld) {
    scaffold("the dialog, its label, its text field and the layer behind it all render in the dark palette");
}

#[then(regex = r"^the text the operator types is legible without selecting the field$")]
async fn then_the_text_the_operator_types_is_legible_without_selecting(_world: &mut FoundryWorld) {
    scaffold("the text the operator types is legible without selecting the field");
}

#[when(regex = r"^the operator opens the sign-in screen, which has no rail and no theme control$")]
async fn when_the_operator_opens_the_sign_in_screen_which_has(_world: &mut FoundryWorld) {
    scaffold("the operator opens the sign-in screen, which has no rail and no theme control");
}

#[then(regex = r"^the sign-in screen renders in the dark palette$")]
async fn then_the_sign_in_screen_renders_in_the_dark_palette(_world: &mut FoundryWorld) {
    scaffold("the sign-in screen renders in the dark palette");
}

#[then(regex = r"^no theme control is present on it$")]
async fn then_no_theme_control_is_present_on_it(_world: &mut FoundryWorld) {
    scaffold("no theme control is present on it");
}

#[when(
    regex = r"^the operator visits the board, the dashboard, an issue, the shortcut list and the sign-in screen$"
)]
async fn when_the_operator_visits_the_board_the_dashboard_an_issue(_world: &mut FoundryWorld) {
    scaffold("the operator visits the board, the dashboard, an issue, the shortcut list and the sign-in screen");
}

#[then(regex = r"^no element on any of those screens renders in a light-palette colour$")]
async fn then_no_element_on_any_of_those_screens_renders_in(_world: &mut FoundryWorld) {
    scaffold("no element on any of those screens renders in a light-palette colour");
}

#[then(regex = r"^the canzan display, body and mono typefaces all report as loaded$")]
async fn then_the_canzan_display_body_and_mono_typefaces_all_report(_world: &mut FoundryWorld) {
    scaffold("the canzan display, body and mono typefaces all report as loaded");
}

#[then(regex = r"^the project heading is set in the canzan display typeface$")]
async fn then_the_project_heading_is_set_in_the_canzan_display(_world: &mut FoundryWorld) {
    scaffold("the project heading is set in the canzan display typeface");
}

#[then(regex = r"^the card titles are set in the canzan body typeface$")]
async fn then_the_card_titles_are_set_in_the_canzan_body(_world: &mut FoundryWorld) {
    scaffold("the card titles are set in the canzan body typeface");
}

#[then(regex = r"^the issue key is set in the canzan mono typeface$")]
async fn then_the_issue_key_is_set_in_the_canzan_mono(_world: &mut FoundryWorld) {
    scaffold("the issue key is set in the canzan mono typeface");
}

#[when(regex = r"^the operator opens the board and then the dashboard$")]
async fn when_the_operator_opens_the_board_and_then_the_dashboard(_world: &mut FoundryWorld) {
    scaffold("the operator opens the board and then the dashboard");
}

#[then(regex = r"^every typeface the pages requested was served by foundry itself$")]
async fn then_every_typeface_the_pages_requested_was_served_by_foundry(_world: &mut FoundryWorld) {
    scaffold("every typeface the pages requested was served by foundry itself");
}

#[then(regex = r"^no request made by either page left foundry's own origin$")]
async fn then_no_request_made_by_either_page_left_foundry_s(_world: &mut FoundryWorld) {
    scaffold("no request made by either page left foundry's own origin");
}

#[then(regex = r"^each lane header reaches at least 4\.5 to 1 against the surface behind it$")]
async fn then_each_lane_header_reaches_at_least_4_5_to(_world: &mut FoundryWorld) {
    scaffold("each lane header reaches at least 4.5 to 1 against the surface behind it");
}

#[then(regex = r"^every canzan typeface is declared to swap in rather than hold the text back$")]
async fn then_every_canzan_typeface_is_declared_to_swap_in_rather(_world: &mut FoundryWorld) {
    scaffold("every canzan typeface is declared to swap in rather than hold the text back");
}

#[then(regex = r"^every string on the board occupies space from the first frame$")]
async fn then_every_string_on_the_board_occupies_space_from_the(_world: &mut FoundryWorld) {
    scaffold("every string on the board occupies space from the first frame");
}

#[given(regex = r#"^the operator opens the "Sandbox" board in the canzan typefaces$"#)]
async fn given_the_operator_opens_the_sandbox_board_in_the_canzan(_world: &mut FoundryWorld) {
    scaffold("the operator opens the \"Sandbox\" board in the canzan typefaces");
}

#[when(regex = r"^the same board is rendered in the fallback typefaces instead$")]
async fn when_the_same_board_is_rendered_in_the_fallback_typefaces(_world: &mut FoundryWorld) {
    scaffold("the same board is rendered in the fallback typefaces instead");
}

#[then(regex = r"^the lane columns and the issue cards occupy the same positions in both$")]
async fn then_the_lane_columns_and_the_issue_cards_occupy_the(_world: &mut FoundryWorld) {
    scaffold("the lane columns and the issue cards occupy the same positions in both");
}

#[given(regex = r"^the operator has never used the theme control$")]
async fn given_the_operator_has_never_used_the_theme_control(_world: &mut FoundryWorld) {
    scaffold("the operator has never used the theme control");
}

#[then(regex = r"^the document records no theme choice at all$")]
async fn then_the_document_records_no_theme_choice_at_all(_world: &mut FoundryWorld) {
    scaffold("the document records no theme choice at all");
}

#[given(regex = r"^the control shows that foundry is following her device$")]
async fn given_the_control_shows_that_foundry_is_following_her_device(_world: &mut FoundryWorld) {
    scaffold("the control shows that foundry is following her device");
}

#[when(regex = r"^she activates it once, then again, then a third time$")]
async fn when_she_activates_it_once_then_again_then_a_third(_world: &mut FoundryWorld) {
    scaffold("she activates it once, then again, then a third time");
}

#[then(regex = r"^it moves to light, then to dark, then back to following her device$")]
async fn then_it_moves_to_light_then_to_dark_then_back(_world: &mut FoundryWorld) {
    scaffold("it moves to light, then to dark, then back to following her device");
}

#[then(regex = r"^on each step the page repaints to the palette the control names$")]
async fn then_on_each_step_the_page_repaints_to_the_palette(_world: &mut FoundryWorld) {
    scaffold("on each step the page repaints to the palette the control names");
}

#[given(regex = r"^the operator has chosen dark while her device prefers light$")]
async fn given_the_operator_has_chosen_dark_while_her_device_prefers(_world: &mut FoundryWorld) {
    scaffold("the operator has chosen dark while her device prefers light");
}

#[when(regex = r"^she opens the change report, then the dashboard, then reloads$")]
async fn when_she_opens_the_change_report_then_the_dashboard_then(_world: &mut FoundryWorld) {
    scaffold("she opens the change report, then the dashboard, then reloads");
}

#[then(regex = r"^every one of those screens renders in the dark palette$")]
async fn then_every_one_of_those_screens_renders_in_the_dark(_world: &mut FoundryWorld) {
    scaffold("every one of those screens renders in the dark palette");
}

#[when(regex = r"^she navigates to any foundry screen$")]
async fn when_she_navigates_to_any_foundry_screen(_world: &mut FoundryWorld) {
    scaffold("she navigates to any foundry screen");
}

#[then(regex = r"^the theme is settled before the browser is permitted to paint$")]
async fn then_the_theme_is_settled_before_the_browser_is_permitted(_world: &mut FoundryWorld) {
    scaffold("the theme is settled before the browser is permitted to paint");
}

#[then(regex = r"^the theme script finished loading before the screen first painted$")]
async fn then_the_theme_script_finished_loading_before_the_screen_first(_world: &mut FoundryWorld) {
    scaffold("the theme script finished loading before the screen first painted");
}

#[given(regex = r"^a browser session with scripting disabled whose device preference is dark$")]
async fn given_a_browser_session_with_scripting_disabled_whose_device_preference(
    _world: &mut FoundryWorld,
) {
    scaffold("a browser session with scripting disabled whose device preference is dark");
}

#[then(regex = r"^no theme control is present anywhere on the screen$")]
async fn then_no_theme_control_is_present_anywhere_on_the_screen(_world: &mut FoundryWorld) {
    scaffold("no theme control is present anywhere on the screen");
}

#[given(
    regex = r"^a browser session that refuses access to site storage, whose device preference is dark$"
)]
async fn given_a_browser_session_that_refuses_access_to_site_storage(_world: &mut FoundryWorld) {
    scaffold(
        "a browser session that refuses access to site storage, whose device preference is dark",
    );
}

#[when(regex = r"^the operator opens the sign-in screen$")]
async fn when_the_operator_opens_the_sign_in_screen(_world: &mut FoundryWorld) {
    scaffold("the operator opens the sign-in screen");
}

#[then(regex = r"^nothing is reported to the operator$")]
async fn then_nothing_is_reported_to_the_operator(_world: &mut FoundryWorld) {
    scaffold("nothing is reported to the operator");
}

#[given(regex = r"^the stored theme choice is a value foundry does not recognise$")]
async fn given_the_stored_theme_choice_is_a_value_foundry_does(_world: &mut FoundryWorld) {
    scaffold("the stored theme choice is a value foundry does not recognise");
}

#[given(regex = r"^the control shows that foundry is following the device$")]
async fn given_the_control_shows_that_foundry_is_following_the_device(_world: &mut FoundryWorld) {
    scaffold("the control shows that foundry is following the device");
}

#[when(regex = r"^its accessible name is read$")]
async fn when_its_accessible_name_is_read(_world: &mut FoundryWorld) {
    scaffold("its accessible name is read");
}

#[then(
    regex = r"^it states that foundry is following the device and names the theme the next press selects$"
)]
async fn then_it_states_that_foundry_is_following_the_device_and(_world: &mut FoundryWorld) {
    scaffold(
        "it states that foundry is following the device and names the theme the next press selects",
    );
}

#[then(regex = r"^after each press the name describes the new state and the next one$")]
async fn then_after_each_press_the_name_describes_the_new_state(_world: &mut FoundryWorld) {
    scaffold("after each press the name describes the new state and the next one");
}

#[then(regex = r"^the theme control is reachable in reading order with a visible focus indicator$")]
async fn then_the_theme_control_is_reachable_in_reading_order_with(_world: &mut FoundryWorld) {
    scaffold("the theme control is reachable in reading order with a visible focus indicator");
}

#[then(regex = r"^its focus indicator is visible in both palettes$")]
async fn then_its_focus_indicator_is_visible_in_both_palettes(_world: &mut FoundryWorld) {
    scaffold("its focus indicator is visible in both palettes");
}

#[then(regex = r"^its target is at least 24 by 24 at desktop width$")]
async fn then_its_target_is_at_least_24_by_24_at(_world: &mut FoundryWorld) {
    scaffold("its target is at least 24 by 24 at desktop width");
}

#[then(regex = r"^its target is at least 44 by 44 at phone width$")]
async fn then_its_target_is_at_least_44_by_44_at(_world: &mut FoundryWorld) {
    scaffold("its target is at least 44 by 44 at phone width");
}
