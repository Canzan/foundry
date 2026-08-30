# Feature: canzan-theme-system — foundry wears canzan's palette and voice, in the
# light the operator's environment demands.
#
# Scenario SSOT. The `## Wave: DISTILL` section of
# docs/feature/canzan-theme-system/feature-delta.md points here; this file is the
# executable truth. Four slices: S01 board+rail palette, S02 remaining surfaces,
# S03 typography, S04 the three-state control.
#
# ---------------------------------------------------------------------------
# HARNESS NOTE — THE DEVICE-PREFERENCE ORACLE (the load-bearing test decision)
# ---------------------------------------------------------------------------
# The trap here is the exact shape of pwa-mobile-rendering's ADR-003, one layer
# over. If a "dark mode" scenario drives dark ONLY by stamping an explicit theme
# choice on the document, then the `@media (prefers-color-scheme: dark)` block is
# GREEN WHETHER OR NOT IT EXISTS — the attribute selector alone satisfies the
# assertion. And the media path is the DEFAULT state, the one most operators get.
# An untested default is the worst possible coverage shape.
#
# MECHANISM (empirically verified, twice — see feature-delta § Device-preference
# oracle): a session-creation capability, `--force-dark-mode` injected into
# `goog:chromeOptions.args`, the same idiom `open_mobile_session` already
# establishes for `mobileEmulation.deviceMetrics`. Verified against raw headless
# Chrome (`--dump-dom`) and against chromedriver 151.0.7922.138 over W3C
# `POST /session` + `execute/sync`:
#
#   flags: <none>                                 matchMedia=false  cssvar=LIGHT
#   flags: --force-dark-mode                      matchMedia=true   cssvar=DARK
#   flags: --enable-features=WebContentsForceDark matchMedia=false  cssvar=LIGHT
#
# BOTH the matchMedia result AND the computed CSS custom property flip, so the
# media block genuinely applies — this is not merely the JS API reporting a
# preference. `--enable-features=WebContentsForceDark` measurably does NOT work;
# it is Chrome's auto-darkening feature, a DIFFERENT thing. Do not "fix" the flag
# to it — that would silently break the oracle back to green-over-nothing.
#
# NOT CDP. `POST /session/{id}/goog/cdp/execute` was considered and rejected: a
# runtime call can race page load, where a session capability cannot, and the
# capability needs no side-channel HTTP client. (fantoccini 0.21.5 does expose
# `Client::issue_cmd`, so CDP was reachable — recorded so nobody reopens this as
# a discovery. The decision is determinism, not availability.)
#
# ANTI-VACUITY GUARD. The baseline is `false`/LIGHT, so the guard discriminates:
# scenario 1 below asserts the instrument itself before any scenario asserts
# foundry's rendering, and every dark-by-device scenario re-asserts it in its
# Given. If the flag ever stops working the guard fails LOUDLY instead of the
# suite silently testing light twice.
#
# ---------------------------------------------------------------------------
# EVERY scenario is @pending. acceptance.rs `filter_run` excludes @pending from
# every lane, so @all (including @needs-browser) stays green until DELIVER
# un-pends them ONE AT A TIME, in slice order. Scenario 1 is un-pended FIRST:
# if the instrument cannot prove a dark device, nothing below it is worth running.
#
# Background reuses the SHIPPED HTTP-lane seed steps (feature_board_new_issue.rs)
# — Acme / Mei / Backend / Sandbox — rather than duplicating fixture setup, the
# same choice pwa-mobile-rendering made. DISCUSS tells this story as Priya on the
# "Identity Platform" board; the scenarios below say "the operator" so the seeded
# fixture and the narrative do not contradict each other.
#
# Assertions are COMPUTED FACTS read from the live browser — resolved colours,
# measured contrast ratios, loaded font entries, resource-timing origins, element
# rects. Never screenshots. Legibility-in-a-dark-room and "does it feel like
# canzan" stay human dogfood items, as DISCUSS says (KPI 1 and 5 carry an
# explicitly qualitative component).

@canzan-theme-system @us-theme
Feature: foundry follows the light the operator's environment asks for
  A dark-preferring operator's board, rail and every other screen render on ink
  without her asking. An operator whose device is set the other way can overrule
  it for this one app, and hand the decision back. Nothing she already knows how
  to do moves.

  Background:
    Given a workspace "Acme" exists with a member "Mei" on team "Backend"
    And a project "Sandbox" with key prefix "GEN" exists under "Backend"
    And Mei is signed in

  # ==========================================================================
  # 0 — THE INSTRUMENT. Un-pend this one FIRST.
  # ==========================================================================

  @needs-browser @real-io @lane-probe @oracle-probe @slice1
  Scenario: The browser under test can be given a device that prefers dark
    # Proves the oracle before anything relies on it. RED-trigger: the device
    # preference capability stops taking effect (flag renamed, Chrome changes its
    # semantics, someone "fixes" it to the auto-darkening feature). Without this
    # scenario every dark-by-device assertion below could pass while silently
    # measuring the light palette twice.
    Given a browser session whose device preference is dark
    Then the browser reports that its device prefers dark
    And a browser session with no stated device preference reports that it prefers light

  # ==========================================================================
  # S01 — the board and its rail wear canzan's palette  (US-CTS-01)
  # ==========================================================================

  @needs-browser @real-io @slice1 @us-cts-01
  Scenario: A dark-preferring operator's whole board is dark, rail included
    # RED-trigger: the device-driven dark block is absent or its guard is wrong —
    # the board keeps painting paper on a dark device.
    Given a browser session whose device preference is dark
    And the browser reports that its device prefers dark
    When the operator opens the "Sandbox" board
    Then the page frame, the rail, the lane columns and every issue card render in the dark palette
    And no surface on the screen renders in a light-palette colour

  @needs-browser @real-io @slice1 @us-cts-01
  Scenario: A light-preferring operator sees canzan's paper-and-jade palette
    # RED-trigger: the canzan tokens were not adopted, or one of foundry's three
    # retired accent hues survives somewhere on the board.
    Given a browser session whose device preference is light
    When the operator opens the "Sandbox" board
    Then the page renders on canzan's paper background with canzan's jade accent
    And foundry's former blue and indigo accents appear nowhere on the screen

  @needs-browser @real-io @slice1 @us-cts-01 @error
  Scenario: An explicit dark choice overrules a light device
    # RED-trigger: the explicit-choice dark block is missing, so only the device
    # can produce dark and the operator's own choice is inert.
    Given a browser session whose device preference is light
    And the operator has already chosen the dark theme
    When the operator opens the "Sandbox" board
    Then the board renders in the dark palette

  @needs-browser @real-io @slice1 @us-cts-01 @error
  Scenario: An explicit light choice overrules a dark device
    # The scenario that earns the guard. RED-trigger: the device-driven dark block
    # is written without the "unless the operator chose light" exception, so a dark
    # device wins over the operator's explicit light choice. This is the single
    # mechanism written in two files; nothing else in the suite catches it.
    Given a browser session whose device preference is dark
    And the browser reports that its device prefers dark
    And the operator has already chosen the light theme
    When the operator opens the "Sandbox" board
    Then the board renders in the light palette

  @needs-browser @real-io @slice1 @us-cts-01 @error
  Scenario: The keyboard selection ring reads as a shape, not only a colour
    # RED-trigger: the ring is restyled as a background fill or a border swap while
    # picking up the jade palette. Both would look right and both would break
    # forced-colours mode and cost layout space.
    Given a browser session whose device preference is dark
    And the operator opens the "Sandbox" board
    When the operator selects an issue card with the keyboard
    Then the selected card carries the selection ring as an outline
    And the ring is present in the light palette as well as the dark one

  @needs-browser @real-io @slice1 @us-cts-01 @kpi
  Scenario: Board and rail text stays legible in both palettes
    # KPI 3. Ratios are COMPUTED from the resolved colours in the live browser, not
    # restated from the six figures a human typed into the token comments.
    # RED-trigger: rebinding the faint tier back to canzan.net's own value, which
    # measures 3.24:1 light and 3.52:1 dark and fails at label size.
    Given a browser session whose device preference is dark
    And the operator opens the "Sandbox" board
    Then every body-size text pair on the board and rail reaches at least 4.5 to 1
    And every large-text and control-boundary pair reaches at least 3 to 1
    And the same holds when the device preference is light

  @needs-browser @real-io @slice1 @us-cts-01 @error
  Scenario: Everything the operator already selects on is still on the page
    # KPI 4's render-contract half, asserted rather than asserted-about. RED-trigger:
    # any semantic class or data marker renamed or dropped by the restyle.
    Given a browser session whose device preference is dark
    When the operator opens the "Sandbox" board
    Then every semantic surface the board is built from is still present
    And every lane column still declares which lane it is
    And every issue card still declares which issue it is

  @real-io @slice1 @us-cts-01
  Scenario: The installed app's brand colours come from the canzan contract
    # HTTP lane — a markup fact, no browser needed. RED-trigger: the off-contract
    # brand literals survive, or a manifest key is dropped while its value moves.
    Given the operator requests the "Sandbox" board page
    Then the page states one brand colour for a light device and another for a dark one
    And both brand colours are canzan contract values
    And the installable app description still declares its brand and background colours

  # ==========================================================================
  # S02 — every remaining screen matches  (US-CTS-02)
  # ==========================================================================

  @needs-browser @real-io @slice2 @us-cts-02 @kpi
  Scenario: The shortcut overlay is legible over a dark board
    # RED-trigger: the overlay block keeps its own colour values instead of taking
    # tokens — a white card floating over an ink board, the most visible defect
    # this feature could ship.
    Given a browser session whose device preference is dark
    And the operator opens the "Sandbox" board
    When the operator opens the keyboard shortcut list
    Then the list, its keycaps and the layer behind it all render in the dark palette
    And the shortcut text and the keycap text each reach at least 4.5 to 1 against the surface behind them

  @needs-browser @real-io @slice2 @us-cts-02
  Scenario: The signed-in dashboard matches the board
    # RED-trigger: the dashboard block keeps its 21 colour values; or the project
    # key chip takes a translucent tint with no opaque surface beneath it, which
    # reads as a failure to any contrast measurement that walks up for a background.
    Given a browser session whose device preference is dark
    When the operator opens the dashboard
    Then the project cards, the section labels and the action controls render in the dark palette
    And the project key chip sits on an opaque surface, not a translucent one

  @needs-browser @real-io @slice2 @us-cts-02
  Scenario: The new-issue dialog and the layer behind it are dark
    # RED-trigger: the dialog and its backdrop keep their own values, so opening a
    # dialog on a dark board flashes a white card over a dark scrim.
    Given a browser session whose device preference is dark
    And the operator opens the "Sandbox" board
    When the operator opens the new-issue dialog
    Then the dialog, its label, its text field and the layer behind it all render in the dark palette
    And the text the operator types is legible without selecting the field

  @needs-browser @real-io @slice2 @us-cts-02 @error
  Scenario: A screen with no chrome still honours the chosen theme
    # RED-trigger: the theme is applied from the rail's own chrome rather than from
    # the document, so the 15 screens that have no rail stay light for an operator
    # who chose dark.
    Given a browser session whose device preference is light
    And the operator has already chosen the dark theme
    When the operator opens the sign-in screen, which has no rail and no theme control
    Then the sign-in screen renders in the dark palette
    And no theme control is present on it

  @needs-browser @real-io @slice2 @us-cts-02 @error
  Scenario: No surface anywhere is left light-only
    # The sweep. Walks every element on all five surface groups in the dark palette
    # and asserts none resolves to a light-palette value. RED-trigger: any single
    # rule missed by the audit — the light rectangle in a dark app that this
    # scenario exists to catch.
    Given a browser session whose device preference is dark
    When the operator visits the board, the dashboard, an issue, the shortcut list and the sign-in screen
    Then no element on any of those screens renders in a light-palette colour

  # ==========================================================================
  # S03 — foundry reads in canzan's voice  (US-CTS-03)
  # ==========================================================================

  @needs-browser @real-io @slice3 @us-cts-03
  Scenario: Headings, body text and keys each carry their intended typeface
    # RED-trigger: the typeface declarations exist but no blob is served, so nothing
    # actually loads. Asserting only that the declarations exist would be green over
    # nothing — this asserts the faces REPORT AS LOADED and that the real heading,
    # card title and issue key resolve to them.
    Given a browser session whose device preference is light
    When the operator opens the "Sandbox" board
    Then the canzan display, body and mono typefaces all report as loaded
    And the project heading is set in the canzan display typeface
    And the card titles are set in the canzan body typeface
    And the issue key is set in the canzan mono typeface

  @needs-browser @real-io @slice3 @us-cts-03 @kpi
  Scenario: No request for a typeface ever leaves foundry's own origin
    # KPI 7 asserts zero cross-origin requests. RED-trigger: a typeface pulled from
    # an external host — which the vendoring policy forbids outright, and which an
    # air-gapped operator would experience as missing type.
    Given a browser session whose device preference is light
    When the operator opens the board and then the dashboard
    Then every typeface the pages requested was served by foundry itself
    And no request made by either page left foundry's own origin

  @needs-browser @real-io @slice3 @us-cts-03 @kpi
  Scenario: Column and section labels are legible at label size
    # The eyebrow idiom is the reason the faint tier had to move. RED-trigger: the
    # faint tier reverts, and the lane headers fail at label size in both palettes.
    Given a browser session whose device preference is dark
    And the operator opens the "Sandbox" board
    Then each lane header reaches at least 4.5 to 1 against the surface behind it
    And the same holds when the device preference is light

  @needs-browser @real-io @slice3 @us-cts-03 @error
  Scenario: A typeface that has not arrived costs a typeface, never a word
    # RED-trigger: a typeface declared without a swap policy. The browser's default
    # is a blocking period during which the text is INVISIBLE — a blank board rather
    # than an unstyled one, which is the failure this scenario exists to forbid.
    Given a browser session whose device preference is light
    When the operator opens the "Sandbox" board
    Then every canzan typeface is declared to swap in rather than hold the text back
    And every string on the board occupies space from the first frame

  @needs-browser @real-io @slice3 @us-cts-03 @error
  Scenario: The board does not move when the typefaces arrive
    # Compares the board's geometry with the canzan faces applied against the same
    # board forced onto its fallback stack, so the comparison is deterministic
    # rather than a race against loading. RED-trigger: a fallback stack whose
    # metrics are far enough off that the columns or cards shift when the real
    # faces land.
    Given the operator opens the "Sandbox" board in the canzan typefaces
    When the same board is rendered in the fallback typefaces instead
    Then the lane columns and the issue cards occupy the same positions in both

  # ==========================================================================
  # S04 — one control, three states  (US-CTS-04)
  # ==========================================================================

  @needs-browser @real-io @slice4 @us-cts-04 @pending
  Scenario: An operator who has never chosen a theme is not given one
    # The mechanism written in two files. "Following the device" must be the ABSENCE
    # of a choice on the document, not a third written value — the stylesheet's
    # "unless she chose light" guard depends on that absence. RED-trigger: the
    # control writes a third value for the follow-the-device state, which looks
    # correct in the toggle and silently breaks dark-by-device for everyone.
    Given a browser session whose device preference is dark
    And the operator has never used the theme control
    When the operator opens the "Sandbox" board
    Then the document records no theme choice at all
    And the board renders in the dark palette

  @needs-browser @real-io @slice4 @us-cts-04 @pending
  Scenario: The control cycles through following the device, light, dark, and back
    # RED-trigger: a two-state toggle, which can never hand the decision back to the
    # device — the setting this operator wants most of the time.
    Given a browser session whose device preference is light
    And the operator opens the "Sandbox" board
    And the control shows that foundry is following her device
    When she activates it once, then again, then a third time
    Then it moves to light, then to dark, then back to following her device
    And on each step the page repaints to the palette the control names

  @needs-browser @real-io @slice4 @us-cts-04 @pending
  Scenario: A chosen theme survives navigation and reload
    # Chains directly onto the cycle above: the operator is left on dark-by-choice.
    # RED-trigger: the choice is held in the page rather than on the origin, so it
    # evaporates on the first navigation.
    Given the operator has chosen dark while her device prefers light
    When she opens the change report, then the dashboard, then reloads
    Then every one of those screens renders in the dark palette

  @needs-browser @real-io @slice4 @us-cts-04 @kpi @pending
  Scenario: A chosen dark screen never flashes light
    # KPI 2. TWO assertions, deliberately: the first is the deterministic backstop,
    # the second the outcome measurement.
    #   (a) the theme script is fetched and run BEFORE the browser is allowed to
    #       paint — a source-level fact that goes RED the instant the tag is moved
    #       to the foot of the body or given a defer/async/module attribute, which
    #       DISCUSS names as the single most likely regression in this feature;
    #   (b) the script's fetch completes BEFORE the page's first contentful paint,
    #       read from the browser's own paint timing.
    # HONEST LIMIT: (b) can pass by luck on a fast loopback even with a deferred
    # script, so it is a supporting measurement, not the load-bearing one. There is
    # no sound way to sample the painted colours of the FIRST frame without the CDP
    # surface this suite deliberately does not use. See feature-delta
    # § Flash-of-wrong-theme oracle — recorded as a known gap, not papered over.
    Given the operator has chosen dark while her device prefers light
    When she navigates to any foundry screen
    Then the theme is settled before the browser is permitted to paint
    And the theme script finished loading before the screen first painted

  @needs-browser @real-io @slice4 @us-cts-04 @error @pending
  Scenario: With scripting disabled the control does not exist and the device decides
    # foundry's SECOND scripting-disabled scenario — it does not assume a blanket
    # no-JS guarantee, which DISCUSS established foundry does not have. Driven with
    # a dark device so the assertion discriminates: a light-device session would
    # pass whether or not the device-driven palette works.
    # RED-trigger: the control is server-rendered, so with scripting off it is
    # present but dead — worse than absent.
    Given a browser session with scripting disabled whose device preference is dark
    When the operator opens the "Sandbox" board
    Then the board renders in the dark palette
    And no theme control is present anywhere on the screen

  @needs-browser @real-io @slice4 @us-cts-04 @error @pending
  Scenario: With site storage refused the screen still themes and nothing is reported
    # The READ guard. MEASURED, not assumed: with Chrome's site-data content setting
    # blocking the origin, BOTH reading and writing stored state throw SecurityError
    # (chromedriver 151, real http:// origin). So the stored choice is unreadable, the
    # guard's catch returns "follow the device", and the page themes from the device.
    #
    # Driven on the SIGN-IN screen by necessity, not by preference: blocking site data
    # also blocks the session cookie, so no signed-in screen is reachable under this
    # condition. That is also why the WRITE guard has NO scenario at all — see the
    # feature-delta § Divergences: the toggle mounts only inside the rail, the rail
    # renders only on signed-in screens, so "storage refused" and "the control exists"
    # are mutually exclusive by construction.
    #
    # RED-trigger: the first read of the stored choice is unguarded, so the script dies
    # at parse time — taking the device-driven palette down with it on every screen.
    Given a browser session that refuses access to site storage, whose device preference is dark
    When the operator opens the sign-in screen
    Then the sign-in screen renders in the dark palette
    And nothing is reported to the operator

  @needs-browser @real-io @slice4 @us-cts-04 @error @pending
  Scenario: A stored choice that means nothing is treated as no choice at all
    # RED-trigger: an unrecognised stored value is applied verbatim, leaving the
    # document in a state no palette matches — a screen with no theme rather than
    # the device's.
    Given a browser session whose device preference is dark
    And the stored theme choice is a value foundry does not recognise
    When the operator opens the "Sandbox" board
    Then the document records no theme choice at all
    And the board renders in the dark palette

  @needs-browser @real-io @slice4 @us-cts-04 @pending
  Scenario: The control says which theme is active and which the next press will select
    # RED-trigger: the control is labelled with a bare glyph, so an operator using
    # assistive technology cannot tell what it does or what pressing it will do.
    Given the operator opens the "Sandbox" board
    And the control shows that foundry is following the device
    When its accessible name is read
    Then it states that foundry is following the device and names the theme the next press selects
    And after each press the name describes the new state and the next one

  @needs-browser @real-io @slice4 @us-cts-04 @pending
  Scenario: The control is reachable and large enough to hit
    # RED-trigger: the control ships without joining the mobile touch-target rule,
    # or with no visible focus indicator in one of the two palettes.
    Given the operator opens the "Sandbox" board
    Then the theme control is reachable in reading order with a visible focus indicator
    And its focus indicator is visible in both palettes
    And its target is at least 24 by 24 at desktop width
    And its target is at least 44 by 44 at phone width
