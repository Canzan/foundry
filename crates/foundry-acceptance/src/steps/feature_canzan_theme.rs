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
use fantoccini::Locator;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

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
async fn given_the_operator_opens_the_sandbox_board(world: &mut FoundryWorld) {
    open_the_board(world).await;
}

#[then(
    regex = r"^the page frame, the rail, the lane columns and every issue card render in the dark palette$"
)]
async fn then_the_page_frame_the_rail_the_lane_columns_and(world: &mut FoundryWorld) {
    assert_renders_in(browser_of(world), DARK).await;
}

/// The sweep, and the oracle for "a light-palette colour" is COMPUTED rather
/// than enumerated: render the SAME board on a light device, collect every
/// opaque colour it paints, and assert the dark render shares none of them.
///
/// Enumerating the light values by hand would only catch the ones the author
/// remembered — and the defect this scenario exists to catch is precisely the
/// rule the author FORGOT (the rail carried fifteen literals and no tokens).
/// Any rule left un-re-pointed paints the same colour on both devices, so it
/// lands in the intersection whether or not anyone anticipated it.
#[then(regex = r"^no surface on the screen renders in a light-palette colour$")]
async fn then_no_surface_on_the_screen_renders_in_a_light(world: &mut FoundryWorld) {
    let dark_paint = painted_opaque_colours(browser_of(world)).await;
    let light = light_device_board(world).await;
    let light_paint = painted_opaque_colours(&light).await;
    light.close().await.ok();

    let shared: Vec<String> = dark_paint
        .iter()
        .filter(|(colour, _)| light_paint.contains_key(*colour))
        .map(|(colour, where_)| {
            format!(
                "{} at `{where_}` (light render paints it at `{}`)",
                hex(*colour),
                light_paint[colour]
            )
        })
        .collect();
    assert!(
        shared.is_empty(),
        "the dark board paints {} colour(s) the LIGHT board also paints, so those surfaces never \
         moved onto the token seam:\n  - {}",
        shared.len(),
        shared.join("\n  - ")
    );
}

/// A session with NO stated device preference — and the probe is asserted here
/// too, in the same shape as the dark Given. The light arm is the baseline the
/// dark arm discriminates against; if it ever started reporting dark, the
/// "explicit dark choice overrules a LIGHT device" scenario would be green
/// without the attribute block existing at all.
#[given(regex = r"^a browser session whose device preference is light$")]
async fn given_a_browser_session_whose_device_preference_is_light(world: &mut FoundryWorld) {
    let browser = browser_harness::new_session().await;
    assert!(
        !browser_harness::device_prefers_dark(&browser).await,
        "a session with NO stated device preference reports that it prefers dark — the oracle no \
         longer discriminates, so a scenario that means to drive a LIGHT device would be driving \
         a dark one and asserting nothing about the explicit-choice path"
    );
    world.browser = Some(browser);
}

#[then(regex = r"^the page renders on canzan's paper background with canzan's jade accent$")]
async fn then_the_page_renders_on_canzan_s_paper_background_with(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    assert_renders_in(browser, LIGHT).await;
    let painted = painted_opaque_colours(browser).await;
    assert!(
        painted.contains_key(&LIGHT.jade),
        "canzan's jade ({}) is painted nowhere on the light board — the accent was retired \
         without a replacement reaching the screen. Painted: {:?}",
        hex(LIGHT.jade),
        painted.values().collect::<Vec<_>>()
    );
}

#[then(regex = r"^foundry's former blue and indigo accents appear nowhere on the screen$")]
async fn then_foundry_s_former_blue_and_indigo_accents_appear_nowhere(world: &mut FoundryWorld) {
    let painted = painted_opaque_colours(browser_of(world)).await;
    for (label, retired) in RETIRED_ACCENTS {
        assert!(
            !painted.contains_key(&retired),
            "the retired accent {label} still paints `{}` on the light board — three competing \
             hues were supposed to collapse into one jade (D-02)",
            painted[&retired]
        );
    }
}

#[given(regex = r"^the operator has already chosen the dark theme$")]
async fn given_the_operator_has_already_chosen_the_dark_theme(world: &mut FoundryWorld) {
    store_theme_choice(world, "dark").await;
}

#[then(regex = r"^the board renders in the dark palette$")]
async fn then_the_board_renders_in_the_dark_palette(world: &mut FoundryWorld) {
    assert_renders_in(browser_of(world), DARK).await;
}

#[given(regex = r"^the operator has already chosen the light theme$")]
async fn given_the_operator_has_already_chosen_the_light_theme(world: &mut FoundryWorld) {
    store_theme_choice(world, "light").await;
}

/// The assertion the `:not([data-theme="light"])` guard exists for. The device
/// says dark (the Given asserted the browser agrees) and the operator said
/// light; delete the guard from the media block and this is the only thing in
/// the suite that goes red.
#[then(regex = r"^the board renders in the light palette$")]
async fn then_the_board_renders_in_the_light_palette(world: &mut FoundryWorld) {
    assert_renders_in(browser_of(world), LIGHT).await;
}

#[when(regex = r"^the operator selects an issue card with the keyboard$")]
async fn when_the_operator_selects_an_issue_card_with_the_keyboard(world: &mut FoundryWorld) {
    select_first_card_with_the_keyboard(browser_of(world)).await;
}

#[then(regex = r"^the selected card carries the selection ring as an outline$")]
async fn then_the_selected_card_carries_the_selection_ring_as_an(world: &mut FoundryWorld) {
    assert_selection_ring_is_an_outline(browser_of(world), DARK).await;
}

#[then(regex = r"^the ring is present in the light palette as well as the dark one$")]
async fn then_the_ring_is_present_in_the_light_palette_as(world: &mut FoundryWorld) {
    let light = light_device_board(world).await;
    select_first_card_with_the_keyboard(&light).await;
    assert_selection_ring_is_an_outline(&light, LIGHT).await;
    light.close().await.ok();
}

#[then(regex = r"^every body-size text pair on the board and rail reaches at least 4\.5 to 1$")]
async fn then_every_body_size_text_pair_on_the_board_and(world: &mut FoundryWorld) {
    assert_text_contrast(browser_of(world), "dark", true).await;
}

#[then(regex = r"^every large-text and control-boundary pair reaches at least 3 to 1$")]
async fn then_every_large_text_and_control_boundary_pair_reaches_at(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    assert_text_contrast(browser, "dark", false).await;
    assert_control_boundary_contrast(browser, "dark").await;
}

#[then(regex = r"^the same holds when the device preference is light$")]
async fn then_the_same_holds_when_the_device_preference_is_light(world: &mut FoundryWorld) {
    let light = light_device_board(world).await;
    assert_text_contrast(&light, "light", true).await;
    assert_text_contrast(&light, "light", false).await;
    assert_control_boundary_contrast(&light, "light").await;
    light.close().await.ok();
}

/// KPI 4's render-contract half. These selectors are the acceptance suite's own
/// vocabulary across ~66 feature files, so a restyle that renamed one would take
/// the whole suite with it — this asserts the guarantee directly rather than
/// leaving it to be discovered as collateral damage.
#[then(regex = r"^every semantic surface the board is built from is still present$")]
async fn then_every_semantic_surface_the_board_is_built_from_is(world: &mut FoundryWorld) {
    let browser = browser_of(world);
    for selector in [
        ".app-shell",
        ".app-shell__content",
        ".sidebar",
        ".sidebar__brand",
        ".sidebar__monogram",
        ".sidebar__workspace",
        ".sidebar__nav",
        ".sidebar__item",
        ".sidebar__item--active",
        ".sidebar__user",
        ".board",
        ".column",
        ".issue-card",
    ] {
        let count = count_of(browser, selector).await;
        assert!(
            count > 0,
            "`{selector}` is no longer on the board — the restyle churned a render-contract \
             selector the acceptance suite selects on (D-11). Restyle, never rename."
        );
    }
}

#[then(regex = r"^every lane column still declares which lane it is$")]
async fn then_every_lane_column_still_declares_which_lane_it_is(world: &mut FoundryWorld) {
    assert_every_marker_present(browser_of(world), ".column", "data-column").await;
}

#[then(regex = r"^every issue card still declares which issue it is$")]
async fn then_every_issue_card_still_declares_which_issue_it_is(world: &mut FoundryWorld) {
    assert_every_marker_present(browser_of(world), ".issue-card", "data-issue-key").await;
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

// ============================================================================
// THE PALETTE ORACLE — resolved colours and COMPUTED ratios, never restated
// ============================================================================
//
// Everything below reads the LIVE browser. Two kinds of fact are asserted and
// they are deliberately different in kind:
//
//   * IDENTITY — "this surface resolves to canzan's paper" — compares against
//     the token CONTRACT (intake.md's table), because "is it the canzan
//     palette?" is a question about an agreed set of values.
//   * LEGIBILITY — "this pair reaches 4.5:1" — is COMPUTED here from the
//     resolved foreground and the resolved ancestor background. It is NEVER
//     compared against the six ratios recorded in the stylesheet's token
//     comments: those are one human's arithmetic, and asserting them against
//     themselves would prove nothing (DELIVER obligation 3).

const MEI_EMAIL: &str = "mei@acme.com";
const MEI_PASSWORD: &str = "mei-correct-horse-battery-staple";
const TEAM_SLUG: &str = "backend";
const PROJECT_SLUG: &str = "sandbox";
const PROJECT_KEY_PREFIX: &str = "GEN";
const WAIT_TIMEOUT: Duration = Duration::from_secs(10);

/// The storage key `theme.js` will read when the three-state control ships at
/// slice 04. Until then the choice is carried across the sign-in navigation by
/// [`apply_stored_theme_choice`], a HARNESS SHIM — the production carrier does
/// not exist yet, and this slice's subject is the stylesheet's two dark blocks,
/// not the persistence. Delete the shim at 04-01, when theme.js does this for
/// real before first paint.
const THEME_STORAGE_KEY: &str = "foundry.theme";

/// The canzan token contract (`docs/feature/canzan-theme-system/intake.md`),
/// with `--cz-faint` at foundry's rebound value (D-04). These are CONTRACT
/// values, transcribed from the intake table — not read back out of the
/// stylesheet, which would make the assertion circular.
#[derive(Clone, Copy)]
struct Palette {
    name: &'static str,
    bg: Rgb,
    bg_2: Rgb,
    surface: Rgb,
    line: Rgb,
    text: Rgb,
    muted: Rgb,
    jade: Rgb,
}

type Rgb = (u8, u8, u8);

const LIGHT: Palette = Palette {
    name: "light",
    bg: (0xfb, 0xfb, 0xf9),
    bg_2: (0xf3, 0xf4, 0xf1),
    surface: (0xff, 0xff, 0xff),
    line: (0xe3, 0xe5, 0xe0),
    text: (0x12, 0x16, 0x14),
    muted: (0x5c, 0x64, 0x5f),
    jade: (0x1a, 0x7a, 0x5e),
};

const DARK: Palette = Palette {
    name: "dark",
    bg: (0x0a, 0x0c, 0x0b),
    bg_2: (0x0f, 0x13, 0x12),
    surface: (0x13, 0x18, 0x17),
    line: (0x1f, 0x25, 0x23),
    text: (0xe8, 0xeb, 0xe8),
    muted: (0x8d, 0x95, 0x8f),
    jade: (0x62, 0xc9, 0xa6),
};

/// The three accent hues this feature retires (D-02) and the two tinted
/// surfaces that carried them. None may survive anywhere on a rendered board.
const RETIRED_ACCENTS: [(&str, Rgb); 6] = [
    ("--accent #2452c9", (0x24, 0x52, 0xc9)),
    ("rail indigo #5b5bd6", (0x5b, 0x5b, 0xd6)),
    ("rail indigo #3a3ad1", (0x3a, 0x3a, 0xd1)),
    ("rail tint #ecedff", (0xec, 0xed, 0xff)),
    ("card-key indigo #4f46e5", (0x4f, 0x46, 0xe5)),
    ("card-key tint #eef2ff", (0xee, 0xf2, 0xff)),
];

/// Every colour the browser actually PAINTS with, per visible element: its text
/// colour, its background, each border side that has a width, and its outline.
/// Returned as `[where, property, resolved-value]`.
const PAINT_PROBE: &str = r#"
var out = [];
var nodes = document.querySelectorAll('*');
for (var i = 0; i < nodes.length; i++) {
  var el = nodes[i];
  if (!el.getClientRects().length) { continue; }
  var cs = window.getComputedStyle(el);
  var where = el.tagName.toLowerCase() + (el.className && typeof el.className === 'string' ? '.' + el.className.trim().split(/\s+/).join('.') : '');
  out.push([where, 'color', cs.color]);
  out.push([where, 'background-color', cs.backgroundColor]);
  var sides = ['Top', 'Right', 'Bottom', 'Left'];
  for (var s = 0; s < sides.length; s++) {
    if (parseFloat(cs['border' + sides[s] + 'Width']) > 0 && cs['border' + sides[s] + 'Style'] !== 'none') {
      out.push([where, 'border-' + sides[s].toLowerCase() + '-color', cs['border' + sides[s] + 'Color']]);
    }
  }
  if (cs.outlineStyle !== 'none' && parseFloat(cs.outlineWidth) > 0) {
    out.push([where, 'outline-color', cs.outlineColor]);
  }
}
return out;
"#;

/// Every text-bearing element inside the app shell (the rail AND the board),
/// with its resolved foreground and the EFFECTIVE background found by walking
/// ancestors to the first fully-opaque one. The ancestor walk is why D-05 bars
/// a translucent tint from carrying text on its own: such a surface resolves to
/// its unblended colour and reads as a failure on a legible page.
const TEXT_PAIR_PROBE: &str = r#"
function opaqueBackgroundOf(node) {
  while (node) {
    var value = window.getComputedStyle(node).backgroundColor;
    var parts = value.match(/[\d.]+/g);
    if (parts && (parts.length < 4 || parseFloat(parts[3]) === 1)) { return value; }
    node = node.parentElement;
  }
  return window.getComputedStyle(document.documentElement).backgroundColor;
}
var out = [];
var shell = document.querySelector('.app-shell');
if (!shell) { return out; }
var nodes = shell.querySelectorAll('*');
for (var i = 0; i < nodes.length; i++) {
  var el = nodes[i];
  if (!el.getClientRects().length) { continue; }
  var owns = false;
  for (var c = 0; c < el.childNodes.length; c++) {
    var child = el.childNodes[c];
    if (child.nodeType === 3 && child.textContent.trim().length) { owns = true; }
  }
  if (!owns) { continue; }
  var cs = window.getComputedStyle(el);
  out.push({
    where: el.tagName.toLowerCase() + (el.className && typeof el.className === 'string' ? '.' + el.className.trim().split(/\s+/).join('.') : ''),
    text: el.textContent.trim().slice(0, 40),
    foreground: cs.color,
    background: opaqueBackgroundOf(el),
    fontSize: parseFloat(cs.fontSize),
    fontWeight: parseInt(cs.fontWeight, 10) || 400
  });
}
return out;
"#;

/// Every interactive control on the shell with the colours that IDENTIFY it —
/// its own fill and its border — against the surface it sits on. WCAG 1.4.11
/// asks for 3:1 from whichever of those does the identifying, which is why the
/// assertion takes the BETTER of the two rather than demanding both.
const CONTROL_BOUNDARY_PROBE: &str = r#"
function opaqueBackgroundOf(node) {
  while (node) {
    var value = window.getComputedStyle(node).backgroundColor;
    var parts = value.match(/[\d.]+/g);
    if (parts && (parts.length < 4 || parseFloat(parts[3]) === 1)) { return value; }
    node = node.parentElement;
  }
  return window.getComputedStyle(document.documentElement).backgroundColor;
}
var out = [];
var shell = document.querySelector('.app-shell');
if (!shell) { return out; }
var nodes = shell.querySelectorAll('button, .button, input[type=submit], input[type=text], input[type=email], input[type=password], textarea');
for (var i = 0; i < nodes.length; i++) {
  var el = nodes[i];
  if (!el.getClientRects().length) { continue; }
  var cs = window.getComputedStyle(el);
  var border = null;
  var sides = ['Top', 'Right', 'Bottom', 'Left'];
  for (var s = 0; s < sides.length; s++) {
    if (parseFloat(cs['border' + sides[s] + 'Width']) > 0 && cs['border' + sides[s] + 'Style'] !== 'none') {
      border = cs['border' + sides[s] + 'Color'];
      break;
    }
  }
  out.push({
    where: el.tagName.toLowerCase() + (el.className && typeof el.className === 'string' ? '.' + el.className.trim().split(/\s+/).join('.') : '') + '[' + (el.textContent.trim().slice(0, 20)) + ']',
    fill: cs.backgroundColor,
    border: border,
    adjacent: opaqueBackgroundOf(el.parentElement)
  });
}
return out;
"#;

/// `rgb(r, g, b)` / `rgba(r, g, b, a)` as the browser reports them.
fn parse_colour(raw: &str) -> Option<(Rgb, f64)> {
    let text = raw.trim();
    let body = text
        .strip_prefix("rgba(")
        .or_else(|| text.strip_prefix("rgb("))?;
    let mut parts = body.strip_suffix(')')?.split(',').map(str::trim);
    let red = parts.next()?.parse::<f64>().ok()?;
    let green = parts.next()?.parse::<f64>().ok()?;
    let blue = parts.next()?.parse::<f64>().ok()?;
    let alpha = match parts.next() {
        Some(value) => value.parse::<f64>().ok()?,
        None => 1.0,
    };
    Some(((red as u8, green as u8, blue as u8), alpha))
}

fn hex(colour: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", colour.0, colour.1, colour.2)
}

/// WCAG 2.1 relative luminance (1.4.3 / 1.4.11), computed here rather than
/// quoted from anywhere.
fn relative_luminance(colour: Rgb) -> f64 {
    fn channel(value: u8) -> f64 {
        let srgb = f64::from(value) / 255.0;
        if srgb <= 0.03928 {
            srgb / 12.92
        } else {
            ((srgb + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(colour.0) + 0.7152 * channel(colour.1) + 0.0722 * channel(colour.2)
}

fn contrast_ratio(one: Rgb, other: Rgb) -> f64 {
    let (first, second) = (relative_luminance(one), relative_luminance(other));
    let (lighter, darker) = if first >= second {
        (first, second)
    } else {
        (second, first)
    };
    (lighter + 0.05) / (darker + 0.05)
}

/// WCAG 1.4.3's "large text": >= 24px, or >= 18.66px at weight >= 700.
fn is_large_text(font_size: f64, font_weight: i64) -> bool {
    font_size >= 24.0 || (font_size >= 18.66 && font_weight >= 700)
}

/// Every OPAQUE colour the page paints with. Translucent values are excluded
/// deliberately: a translucent literal is identical in both palettes, so it
/// cannot discriminate one from the other. That hole is the seam scanner's
/// (check-arch S1), not this sweep's.
async fn painted_opaque_colours(client: &fantoccini::Client) -> BTreeMap<Rgb, String> {
    let observations = client
        .execute(PAINT_PROBE, Vec::new())
        .await
        .expect("sweep every painted colour on the page");
    let mut painted = BTreeMap::new();
    for entry in observations.as_array().expect("the probe returns an array") {
        let row = entry.as_array().expect("each observation is a triple");
        let where_ = row[0].as_str().unwrap_or_default();
        let property = row[1].as_str().unwrap_or_default();
        let raw = row[2].as_str().unwrap_or_default();
        if let Some((colour, alpha)) = parse_colour(raw) {
            if alpha == 1.0 {
                painted
                    .entry(colour)
                    .or_insert_with(|| format!("{where_} {{ {property} }}"));
            }
        }
    }
    assert!(
        painted.len() >= 5,
        "the paint sweep resolved only {} opaque colours — the probe is not seeing the page, so \
         any 'no light colour here' assertion built on it would be vacuous",
        painted.len()
    );
    painted
}

/// Assert every surface named by US-CTS-01 resolves to `palette`.
async fn assert_renders_in(client: &fantoccini::Client, palette: Palette) {
    let expectations: [(&str, &str, Rgb); 10] = [
        ("html", "background-color", palette.bg),
        ("html", "color", palette.text),
        ("body", "background-color", palette.bg),
        (".sidebar", "background-color", palette.bg_2),
        (".sidebar", "border-right-color", palette.line),
        (".column", "background-color", palette.bg_2),
        (".issue-card", "background-color", palette.surface),
        (".issue-card", "color", palette.text),
        (
            ".sidebar__item:not(.sidebar__item--active)",
            "color",
            palette.muted,
        ),
        (".sidebar__item--active", "color", palette.jade),
    ];
    for (selector, property, expected) in expectations {
        let resolved = resolve_all(client, selector, property).await;
        assert!(
            !resolved.is_empty(),
            "no element matched `{selector}` on the board — the {} palette assertion would be \
             vacuous. The render contract must still be on the page.",
            palette.name
        );
        for (index, raw) in resolved.iter().enumerate() {
            let (colour, _) = parse_colour(raw)
                .unwrap_or_else(|| panic!("`{selector}` #{index} reported {property} as {raw:?}"));
            assert_eq!(
                colour,
                expected,
                "`{selector}` #{index} resolved {property} to {} — the {} palette binds it to {}. \
                 A surface still painting its other-palette value is the light stripe down a dark \
                 app this scenario exists to catch.",
                hex(colour),
                palette.name,
                hex(expected)
            );
        }
    }
}

/// The resolved value of `property` for EVERY element matching `selector`.
async fn resolve_all(client: &fantoccini::Client, selector: &str, property: &str) -> Vec<String> {
    let script = "var property = arguments[1];
         return Array.prototype.map.call(
             document.querySelectorAll(arguments[0]),
             function (el) { return window.getComputedStyle(el).getPropertyValue(property); }
         );";
    let values = client
        .execute(
            script,
            vec![
                serde_json::Value::String(selector.to_string()),
                serde_json::Value::String(property.to_string()),
            ],
        )
        .await
        .unwrap_or_else(|err| panic!("resolve {property} for {selector}: {err}"));
    values
        .as_array()
        .expect("the resolver returns an array")
        .iter()
        .map(|value| value.as_str().unwrap_or_default().to_string())
        .collect()
}

/// The Sandbox board URL on the shared origin.
fn board_url(world: &FoundryWorld) -> String {
    let harness = world.harness.as_ref().expect("harness");
    format!(
        "{}/team/{TEAM_SLUG}/project/{PROJECT_SLUG}",
        harness.base_url()
    )
}

/// Seed GEN-1 into the Sandbox project the HTTP Background created, so the board
/// has a card. Mirrors `feature_pwa_mobile.rs::seed_sandbox_issue` — the store
/// pool directly, no HTTP round-trip. A PRECONDITION, not an outcome: no
/// assertion below reads anything this writes except "a card exists".
async fn seed_sandbox_issue(world: &FoundryWorld) {
    let harness = world.harness.as_ref().expect("harness");
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
    sqlx::query(
        "INSERT INTO issues (id, project_id, workspace_id, number, title, description_md, state, author_id)
              VALUES ($1, $2, $3, 1, 'Wears the canzan palette', '', 'backlog', $4)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(project.0)
    .bind(project.1)
    .bind(author.0)
    .execute(pool)
    .await
    .expect("insert the Sandbox issue");
}

/// Sign in through the REAL form and settle on the post-sign-in navigation.
async fn sign_in_and_settle(browser: &fantoccini::Client, world: &FoundryWorld) {
    let harness = world
        .harness
        .as_ref()
        .expect("the HTTP Background must have spawned the harness");
    browser_harness::sign_in_through_browser(browser, harness, MEI_EMAIL, MEI_PASSWORD).await;
    let deadline = Instant::now() + WAIT_TIMEOUT;
    loop {
        let current = browser
            .current_url()
            .await
            .map(|url| url.to_string())
            .unwrap_or_default();
        if (!current.is_empty() && !current.contains("/sign-in")) || Instant::now() >= deadline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// HARNESS SHIM (see [`THEME_STORAGE_KEY`]). Re-stamp the operator's stored
/// choice onto the document after a navigation, because the production carrier
/// — `theme.js`, which does this from `<head>` before first paint — ships at
/// slice 04. What is under test here is the stylesheet's `[data-theme]` block
/// and the `:not([data-theme="light"])` guard on the media block; the attribute
/// is the PRECONDITION those rules react to, never the outcome asserted.
async fn apply_stored_theme_choice(browser: &fantoccini::Client) {
    browser
        .execute(
            "var choice = null;
             try { choice = window.localStorage.getItem(arguments[0]); } catch (err) { choice = null; }
             if (choice === 'dark' || choice === 'light') {
               document.documentElement.setAttribute('data-theme', choice);
             } else {
               document.documentElement.removeAttribute('data-theme');
             }
             return choice;",
            vec![serde_json::Value::String(THEME_STORAGE_KEY.to_string())],
        )
        .await
        .expect("re-apply the stored theme choice after navigation");
}

/// Record the operator's explicit choice on the origin, before she navigates.
async fn store_theme_choice(world: &mut FoundryWorld, choice: &str) {
    let browser = world
        .browser
        .take()
        .expect("a browser session must have been opened first");
    let base = world.harness.as_ref().expect("harness").base_url();
    browser
        .goto(&format!("{base}/sign-in"))
        .await
        .expect("reach the origin so a choice can be stored against it");
    browser
        .execute(
            "try { window.localStorage.setItem(arguments[0], arguments[1]); } catch (err) {}
             return true;",
            vec![
                serde_json::Value::String(THEME_STORAGE_KEY.to_string()),
                serde_json::Value::String(choice.to_string()),
            ],
        )
        .await
        .expect("store the operator's theme choice");
    world.browser = Some(browser);
}

/// Sign in and land on the Sandbox board in `browser`, with any stored choice
/// re-applied.
async fn land_on_board(browser: &fantoccini::Client, world: &FoundryWorld) {
    sign_in_and_settle(browser, world).await;
    let url = board_url(world);
    browser.goto(&url).await.expect("navigate to the board");
    browser
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css(".app-shell .board .column"))
        .await
        .expect("the board must render the authed shell, the rail and its lanes");
    let landed = browser
        .current_url()
        .await
        .map(|url| url.to_string())
        .unwrap_or_default();
    assert!(
        landed.starts_with(&url),
        "opening the board landed on {landed} instead of {url} — the surface redirected away."
    );
    apply_stored_theme_choice(browser).await;
}

/// The board, opened in the session the scenario's Given already created.
async fn open_the_board(world: &mut FoundryWorld) {
    seed_sandbox_issue(world).await;
    let browser = world
        .browser
        .take()
        .expect("a browser session must have been opened before the board");
    land_on_board(&browser, world).await;
    world.browser = Some(browser);
}

/// A SECOND session on a LIGHT device, on the same board — the "and the same
/// holds in the other palette" arm. Opened fresh rather than re-themed, because
/// the device preference is a session capability and cannot be changed on a
/// live session.
async fn light_device_board(world: &FoundryWorld) -> fantoccini::Client {
    let browser = browser_harness::new_session().await;
    assert!(
        !browser_harness::device_prefers_dark(&browser).await,
        "a session with no stated device preference reports it prefers dark — the oracle no \
         longer discriminates and the light arm would be measuring dark twice"
    );
    land_on_board(&browser, world).await;
    browser
}

fn browser_of(world: &FoundryWorld) -> &fantoccini::Client {
    world
        .browser
        .as_ref()
        .expect("a browser session must have been opened first")
}

/// COMPUTE every text pair's ratio from the live browser and assert the tier it
/// owes. `body_size` selects which half of WCAG 1.4.3 is being asserted.
async fn assert_text_contrast(client: &fantoccini::Client, palette_name: &str, body_size: bool) {
    let pairs = client
        .execute(TEXT_PAIR_PROBE, Vec::new())
        .await
        .expect("measure every text pair on the board and rail");
    let pairs = pairs.as_array().expect("the probe returns an array");
    let mut measured = 0usize;
    for pair in pairs {
        let where_ = pair["where"].as_str().unwrap_or_default();
        let text = pair["text"].as_str().unwrap_or_default();
        let font_size = pair["fontSize"].as_f64().unwrap_or(0.0);
        let font_weight = pair["fontWeight"].as_i64().unwrap_or(400);
        if is_large_text(font_size, font_weight) == body_size {
            continue;
        }
        let Some((foreground, _)) = parse_colour(pair["foreground"].as_str().unwrap_or_default())
        else {
            continue;
        };
        let Some((background, _)) = parse_colour(pair["background"].as_str().unwrap_or_default())
        else {
            continue;
        };
        let ratio = contrast_ratio(foreground, background);
        let floor = if body_size { 4.5 } else { 3.0 };
        assert!(
            ratio >= floor,
            "{palette_name} palette: `{where_}` ({text:?}, {font_size}px/{font_weight}) computes \
             {ratio:.2}:1 — {} on {} — below the {floor}:1 this text size owes (WCAG 1.4.3). \
             Measured in the live browser from the resolved foreground and the first opaque \
             ancestor background; NOT read from the stylesheet's token comments.",
            hex(foreground),
            hex(background)
        );
        measured += 1;
    }
    if body_size {
        assert!(
            measured >= 8,
            "only {measured} body-size text pairs were measured on the {palette_name} board — the \
             probe is not seeing the page and the assertion would be vacuous"
        );
    }
}

/// COMPUTE each control's identifying contrast against the surface it sits on
/// (WCAG 1.4.11): the better of its fill and its border must reach 3:1.
async fn assert_control_boundary_contrast(client: &fantoccini::Client, palette_name: &str) {
    let controls = client
        .execute(CONTROL_BOUNDARY_PROBE, Vec::new())
        .await
        .expect("measure every control boundary on the board and rail");
    let controls = controls.as_array().expect("the probe returns an array");
    assert!(
        !controls.is_empty(),
        "no control was found on the {palette_name} board — the 1.4.11 assertion would be vacuous"
    );
    for control in controls {
        let where_ = control["where"].as_str().unwrap_or_default();
        let Some((adjacent, _)) = parse_colour(control["adjacent"].as_str().unwrap_or_default())
        else {
            continue;
        };
        let mut best = 0.0f64;
        let mut how = String::new();
        for (label, raw) in [("fill", &control["fill"]), ("border", &control["border"])] {
            let Some(text) = raw.as_str() else { continue };
            let Some((colour, alpha)) = parse_colour(text) else {
                continue;
            };
            if alpha < 1.0 {
                continue;
            }
            let ratio = contrast_ratio(colour, adjacent);
            if ratio > best {
                best = ratio;
                how = format!("{label} {}", hex(colour));
            }
        }
        assert!(
            best >= 3.0,
            "{palette_name} palette: `{where_}` computes only {best:.2}:1 ({how}) against {} — \
             nothing identifies this control at WCAG 1.4.11's 3:1. A hairline divider is not a \
             control boundary.",
            hex(adjacent)
        );
    }
}

/// Press `j` — the shipped board shortcut — and wait for the ring to land.
/// Driven through the REAL keyboard layer rather than by adding the class, so a
/// ring that stopped being applied fails here instead of being simulated.
async fn select_first_card_with_the_keyboard(client: &fantoccini::Client) {
    browser_harness::wait_for_kb_ready(client).await;
    browser_harness::press_key(client, "j").await;
    client
        .wait()
        .at_most(WAIT_TIMEOUT)
        .for_element(Locator::Css(".issue-card.kb-selected"))
        .await
        .expect("pressing `j` must ring the first issue card");
}

/// The ring is a SHAPE, not a colour swap: an outline, present, 2px, in the
/// palette's accent — and the card's fill and border must be UNCHANGED from an
/// unselected card, which is what distinguishes "an outline was added" from
/// "the background was repainted" or "the border colour was swapped". Both of
/// those look right on screen and both break forced-colours mode; the border
/// swap additionally costs no layout only by luck.
async fn assert_selection_ring_is_an_outline(client: &fantoccini::Client, palette: Palette) {
    let probe = client
        .execute(
            "var selected = document.querySelector('.issue-card.kb-selected');
             if (!selected) { throw new Error('no card carries the selection ring'); }
             var plain = null;
             var cards = document.querySelectorAll('.issue-card');
             for (var i = 0; i < cards.length; i++) {
               if (!cards[i].classList.contains('kb-selected')) { plain = cards[i]; }
             }
             var ring = window.getComputedStyle(selected);
             var base = plain ? window.getComputedStyle(plain) : null;
             return {
               outlineStyle: ring.outlineStyle,
               outlineWidth: ring.outlineWidth,
               outlineColor: ring.outlineColor,
               fill: ring.backgroundColor,
               border: ring.borderTopColor,
               plainFill: base ? base.backgroundColor : null,
               plainBorder: base ? base.borderTopColor : null
             };",
            Vec::new(),
        )
        .await
        .expect("read the selection ring");

    let outline_style = probe["outlineStyle"].as_str().unwrap_or_default();
    assert_ne!(
        outline_style, "none",
        "the {} selection ring is not an OUTLINE — a background fill or a border swap vanishes in \
         forced-colours mode and relies on colour alone (ADR-004 / NFR-7)",
        palette.name
    );
    let outline_width: f64 = probe["outlineWidth"]
        .as_str()
        .unwrap_or_default()
        .trim_end_matches("px")
        .parse()
        .unwrap_or(0.0);
    assert!(
        outline_width >= 2.0,
        "the {} selection ring is {outline_width}px — too thin to read as a shape change",
        palette.name
    );
    let (ring_colour, _) = parse_colour(probe["outlineColor"].as_str().unwrap_or_default())
        .expect("the ring has a resolved outline colour");
    assert_eq!(
        ring_colour,
        palette.jade,
        "the {} selection ring resolves to {} rather than the palette's accent {}",
        palette.name,
        hex(ring_colour),
        hex(palette.jade)
    );
    let ratio = contrast_ratio(ring_colour, palette.surface);
    assert!(
        ratio >= 3.0,
        "the {} selection ring computes {ratio:.2}:1 against the card it rings — below WCAG \
         1.4.11's 3:1 for a non-text indicator",
        palette.name
    );

    if let (Some(plain_fill), Some(plain_border)) =
        (probe["plainFill"].as_str(), probe["plainBorder"].as_str())
    {
        assert_eq!(
            probe["fill"].as_str().unwrap_or_default(),
            plain_fill,
            "the {} ringed card's BACKGROUND differs from an unringed card — the ring is a fill, \
             not an outline",
            palette.name
        );
        assert_eq!(
            probe["border"].as_str().unwrap_or_default(),
            plain_border,
            "the {} ringed card's BORDER differs from an unringed card — the ring is a border \
             swap, not an outline",
            palette.name
        );
    }
}

/// How many elements match `selector` right now.
async fn count_of(client: &fantoccini::Client, selector: &str) -> u64 {
    let count = client
        .execute(
            "return document.querySelectorAll(arguments[0]).length;",
            vec![serde_json::Value::String(selector.to_string())],
        )
        .await
        .unwrap_or_else(|err| panic!("count {selector}: {err}"));
    count.as_u64().unwrap_or_default()
}

/// Every element matching `selector` carries a NON-EMPTY `marker` attribute —
/// and there is at least one of them, so the assertion cannot pass on an empty
/// board.
async fn assert_every_marker_present(client: &fantoccini::Client, selector: &str, marker: &str) {
    let values = client
        .execute(
            "var marker = arguments[1];
             return Array.prototype.map.call(
                 document.querySelectorAll(arguments[0]),
                 function (el) { return el.getAttribute(marker); }
             );",
            vec![
                serde_json::Value::String(selector.to_string()),
                serde_json::Value::String(marker.to_string()),
            ],
        )
        .await
        .unwrap_or_else(|err| panic!("read {marker} from {selector}: {err}"));
    let values = values.as_array().expect("the reader returns an array");
    assert!(
        !values.is_empty(),
        "no `{selector}` is on the board, so `{marker}` cannot be asserted — the render contract \
         has to be PRESENT for this scenario to mean anything"
    );
    for (index, value) in values.iter().enumerate() {
        let declared = value.as_str().unwrap_or_default();
        assert!(
            !declared.trim().is_empty(),
            "`{selector}` #{index} no longer declares `{marker}` — the restyle dropped a data \
             marker the acceptance suite and the client layers both select on (D-11)"
        );
    }
}
