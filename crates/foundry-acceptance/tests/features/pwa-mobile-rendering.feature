# Feature: pwa-mobile-rendering — Foundry renders on a phone and installs like an app.
#
# Source SSOT for docs/feature/pwa-mobile-rendering/distill/test-scenarios.md.
# DISCUSS/DESIGN: base.html had NO viewport meta, the stylesheet had ZERO @media
# breakpoints, and there was no manifest — the app was desktop-fixed. This feature
# adds the viewport meta, responsive @media rules, and an installable manifest, all
# as head tags / CSS / static assets (no new route, no migration, latest stays 0014).
#
# HARNESS NOTE — @needs-browser, MOBILE EMULATION (DESIGN ADR-003, the load-bearing
# test decision). These scenarios drive a fantoccini session created with chromedriver
# goog:chromeOptions.mobileEmulation (deviceMetrics width:390 height:844 pixelRatio:3
# mobile:true) via a NEW open_mobile_session() harness helper — NOT a narrow desktop
# window. Headless --headless=new is DESKTOP Chrome: at a narrow window it lays out at
# the window width regardless of the viewport meta, so a resize-only test would be green
# whether or not the meta exists (green over nothing). mobileEmulation makes Chrome apply
# REAL mobile viewport semantics, so the no-viewport defect reproduces (RED) and the
# viewport-meta fix is measurable (GREEN). Assertions are LAYOUT FACTS (scrollWidth vs
# innerWidth, element rects, manifest fetch/parse), never screenshots; legibility + the
# OS install prompt + thumb-reach stay human phone-dogfood items. NOT Playwright (D-TOOL).
#
# EVERY scenario is @pending; acceptance.rs filter_run excludes @pending from every lane,
# so @all (incl. @needs-browser) stays green until DELIVER wires each slice and un-@pends.

@pwa-mobile-rendering @us-mobile @driving_port
Feature: A member uses Foundry on their phone and installs it
  On a phone the app fits the screen, the board and dialogs are usable with a thumb,
  and the browser offers to install it to the home screen — none of which is possible
  today. Desktop rendering is unchanged.

  Background:
    Given a workspace "Acme" exists with a member "Mei" on team "Backend"
    And a project "Sandbox" with key prefix "GEN" exists under "Backend"
    And Mei is signed in

  # ---------------------------------------------- Slice 01 — viewport + no overflow (skeleton)

  @needs-browser @mobile @slice1 @us-01 @lane-probe @walking_skeleton @driving_port
  Scenario: A mobile browser session fits the board to the screen end to end
    # The instrument proof: if open_mobile_session() can't drive a real 390px mobile
    # viewport and read the DOM, no other scenario here is worth writing.
    Given Mei opens the "Sandbox" board in a mobile browser at 390x844
    Then the page declares a mobile viewport meta
    And the page has no horizontal overflow

  @needs-browser @mobile @slice1 @us-01
  Scenario Outline: The primary authed surfaces fit a phone screen with no horizontal overflow
    Given Mei opens <surface> in a mobile browser at 390x844
    Then the page has no horizontal overflow

    Examples:
      | surface                             |
      | the dashboard                       |
      | the "Sandbox" board                 |
      | an issue page on "Sandbox"          |
      | the "Sandbox" board with the new-issue dialog open |

  # ---------------------------------------------- Slice 02 — responsive surfaces

  @needs-browser @mobile @slice2 @us-02
  Scenario: An opened dialog fits the phone screen and its body scrolls
    Given Mei is on the "Sandbox" board in a mobile browser at 390x844
    When she opens the new-issue dialog
    Then the dialog is no wider than the viewport
    And the page has no horizontal overflow
    And the dialog body scrolls when its content is taller than the viewport

  @needs-browser @mobile @slice2 @us-02
  Scenario: The board columns scroll within their container, not the page
    Given the "Sandbox" board has several status columns
    And Mei opens the "Sandbox" board in a mobile browser at 390x844
    Then the page has no horizontal overflow
    And the board columns container is horizontally scrollable

  @needs-browser @mobile @slice2 @us-02
  Scenario: The navigation collapses to a mobile affordance
    Given Mei opens the dashboard in a mobile browser at 390x844
    Then the full desktop sidebar rail is not shown at full width
    And a mobile navigation affordance is present

  @needs-browser @mobile @slice2 @us-02
  Scenario: Primary controls are large enough to tap
    Given Mei is on the "Sandbox" board in a mobile browser at 390x844
    Then the "New issue" control is at least 44px in its smaller dimension

  @needs-browser @slice2 @us-02 @desktop @scoped
  Scenario: Desktop rendering is unchanged
    # Blast-radius guard: the @media rules must not leak into desktop. Uses the shipped
    # DESKTOP session (open_session), not the mobile one.
    Given Mei opens the "Sandbox" board in a desktop browser
    Then the desktop sidebar rail is shown
    And the board layout matches the shipped desktop behaviour

  # ---------------------------------------------- Slice 03 — installable PWA (no service worker)

  @needs-browser @mobile @slice3 @us-03
  Scenario: The web app manifest is linked, served, and valid
    Given Mei opens the "Sandbox" board in a mobile browser at 390x844
    Then the page links a web app manifest
    And fetching the manifest returns 200 with valid JSON
    And the manifest declares name, short_name, start_url, scope, display "standalone", theme_color and background_color
    And the manifest lists icons including 192x192, 512x512 and a maskable icon

  @needs-browser @mobile @slice3 @us-03
  Scenario: The declared icons are served
    Given the manifest lists its icons
    When each icon URL is fetched
    Then it returns 200 with an image content-type

  @needs-browser @mobile @slice3 @us-03
  Scenario: The app declares standalone display, a theme color, and the apple meta
    Given Mei opens the "Sandbox" board in a mobile browser at 390x844
    Then the page contains a theme-color meta
    And the page contains apple-mobile-web-app-capable and an apple-touch-icon
    And the manifest display mode is "standalone"

  # ---------------------------------------------- Cross-cutting

  @needs-browser @mobile @slice1 @cross-feature
  Scenario: The CSS hash stays consistent across base.html and lib.rs
    # Asserted at the source level (not the browser) — a guard the DELIVER crafter runs.
    Given a CSS change has landed for this feature
    Then base.html and lib.rs reference the same foundry.<hash>.css
