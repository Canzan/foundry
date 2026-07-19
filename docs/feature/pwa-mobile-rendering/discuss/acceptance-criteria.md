# Acceptance Criteria — pwa-mobile-rendering

Given/When/Then for DISTILL. All browser scenarios run in the **fantoccini `@needs-browser` lane at a mobile
window size** (chromedriver window sizing / CDP), never Playwright. Layout facts are asserted (viewport,
overflow, sizes, manifest), not screenshots.

## US-01 — viewport + no overflow (walking skeleton)

```gherkin
Scenario: The layout declares a mobile viewport
  Given the shared layout renders an authed page
  Then the head contains meta name="viewport" content="width=device-width, initial-scale=1"

Scenario: The board fits a phone screen (the defect reproduction)
  Given Mei is signed in on the "Acme" workspace
  And the browser window is sized to 390x844
  When Mei opens the "Acme" board
  Then the page has no horizontal overflow (documentElement.scrollWidth <= window.innerWidth)

Scenario Outline: The primary authed surfaces do not overflow at phone width
  Given Mei is signed in and the browser is 390x844
  When Mei opens <surface>
  Then the page has no horizontal overflow

  Examples:
    | surface                       |
    | the dashboard                 |
    | the "Acme" board              |
    | an issue page                 |
    | the new-issue dialog (open)   |
```

## US-02 — responsive surfaces

```gherkin
Scenario: An opened dialog fits the phone screen
  Given Mei is on the "Acme" board in a 390x844 browser
  When she opens the new-issue dialog
  Then the dialog element is no wider than the viewport
  And the page has no horizontal overflow
  And the dialog body scrolls when its content is taller than the viewport

Scenario: The board columns scroll within their container, not the page
  Given a board with several status columns in a 390x844 browser
  Then the page has no horizontal overflow
  And the columns container is horizontally scrollable

Scenario: The nav collapses to a mobile affordance
  Given Mei is signed in in a 390x844 browser
  When she opens the dashboard
  Then the full desktop sidebar rail is not shown at full width
  And a mobile navigation affordance is present

Scenario: Primary controls are tappable
  Given Mei is on the "Acme" board in a 390x844 browser
  Then the "New issue" control's rendered box is at least ~44px in its smaller dimension

Scenario: Desktop rendering is unchanged
  Given the browser is sized to a desktop width (e.g. 1280x900)
  When Mei opens the board
  Then the layout matches the shipped desktop behaviour (existing @needs-browser scenarios stay green)
```

## US-03 — installable PWA

```gherkin
Scenario: The manifest is linked, served, and valid
  Given the shared layout renders an authed page
  Then the head links rel="manifest"
  And fetching the manifest returns 200 with valid JSON
  And the manifest has name, short_name, start_url, scope, display "standalone", theme_color, background_color
  And the manifest lists icons including 192x192 and 512x512 and a maskable icon

Scenario: The declared icons are served
  Given the manifest lists its icons
  When each icon URL is fetched
  Then it returns 200 with an image content-type

Scenario: The app declares standalone + theme color + apple meta
  Given the shared layout renders an authed page
  Then the head contains a theme-color meta
  And the head contains apple-mobile-web-app-capable and an apple-touch-icon
  And the manifest display mode is "standalone"

Scenario: A service worker (if present) does not break htmx or cache dynamic HTML
  Given a service worker ships (ODD-3)
  Then it registers without a console error
  And it does not serve cached HTML for dynamic pages (v1 has no offline requirement)
```

## Cross-cutting / regression

```gherkin
Scenario: The CSS hash stays consistent across the two places
  Given a CSS change landed in a slice
  Then base.html and lib.rs:297 reference the same foundry.<hash>.css
  And the shipped "hashed URL is immutable" check passes

Scenario: No-JS and shipped oracles preserved
  Given scripting is disabled
  Then every page still renders and the shipped HTTP-lane + desktop @needs-browser scenarios stay green
```

## Browser-dogfood checklist (not automated — the human parts)
1. On a real phone: the board reads cleanly at 1× (legible, no pinch-zoom), columns scroll naturally.
2. Open the new-issue dialog on the phone — it's a usable sheet, Save/Cancel reachable with a thumb.
3. The browser offers **Install / Add to Home Screen**; the installed icon launches Foundry **standalone**.
4. iOS Safari "Add to Home Screen" shows the icon + launches without Safari chrome.
