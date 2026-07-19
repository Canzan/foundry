<!-- markdownlint-disable MD024 -->
# User Stories — pwa-mobile-rendering

LeanUX stories. Following the navigation-bar-linear-ui precedent, JTBD is framed lightly (no `docs/product`
SSOT in this repo); each story ties to a **mobile/PWA outcome KPI** (see `outcome-kpis.md`).

## Primary job (JTBD one-liner)

**When I'm away from my desk (commute, standup, on the floor), I want to open Foundry on my phone and actually
read/triage issues — and keep it one tap away like an app — so I can stay on top of work without a laptop.**

- Functional: view the board, open an issue, file/triage from a phone.
- Emotional: relief that the tool "just works" on mobile instead of a pinch-zoom mess.
- Social: not the person who says "I'll look when I'm back at my desk."

## System Constraints

- Web tier is `crates/foundry-app`; server-rendered askama templates extending `base.html`. No SPA, no Node.
- Styling goes in the single hashed CSS file `static/css/foundry.<hash>.css`; **any CSS change rotates the hash
  in `base.html` AND `lib.rs:297`**.
- Manifest, icons, and any service worker are **static assets** under `/static` — no new content routes, no
  migration (latest stays `0014`).
- Tests use the **fantoccini `@needs-browser` lane at a mobile window size** — NOT Playwright (D-TOOL).
- PWA install needs a valid manifest over HTTPS (or localhost).
- Personas: **Mei** (workspace member on "Acme", the mobile user); **Ariane** (instance admin, installs it).

---

## US-01: Foundry renders on a phone without zooming out (Walking Skeleton)

### Problem
Mei opens the "Acme" board on her phone. Because `base.html` declares no viewport, the browser renders at
~980px and shrinks everything to unreadable — she has to pinch-zoom and pan just to read a card. The single
most basic expectation of a modern web app — "it fits my screen" — fails at the first byte.

### Who
- Workspace member on a phone | signed in | wants to read the board without pinch-zooming.

### Solution
Add `<meta name="viewport" content="width=device-width, initial-scale=1">` to `base.html`, and make the
primary authed pages (dashboard, board, issue page, the modals) **not overflow horizontally** at a 390px phone
width — the precondition for every later responsive refinement.

### Elevator Pitch
Before: on a phone the board loads at desktop width, zoomed out — unreadable without pinch-zoom.
After: open `/team/acme/project/…` on a 390px-wide browser → the page fits the screen with **no horizontal
scrollbar** and text is legible at 1× zoom.
Decision enabled: Mei decides she can actually use Foundry on her phone at all.

### UAT Scenarios (BDD)
#### Scenario: The layout declares a mobile viewport
Given the shared layout renders any authed page
Then the page head contains `<meta name="viewport" content="width=device-width, initial-scale=1">`

#### Scenario: The board fits a phone screen
Given Mei is signed in on the "Acme" workspace
When she opens the "Acme" board in a browser sized to 390×844
Then the page has no horizontal overflow (documentElement.scrollWidth <= innerWidth)
And the page is not rendered at a zoomed-out desktop width

### Acceptance Criteria
- AC-01.1: `base.html` `<head>` contains the exact viewport meta (asserted in the rendered DOM).
- AC-01.2: At a 390px-wide browser window, dashboard, board, issue page, and an open modal each show
  `scrollWidth <= innerWidth` (no horizontal overflow).
- AC-01.3: The CSS hash is rotated in `base.html` and `lib.rs:297` if any CSS lands in this slice; fmt/clippy
  + the shipped check for the hashed URL stay green.
- AC-01.4: All shipped acceptance scenarios (HTTP + `@needs-browser`) remain green — the viewport meta is
  additive.

---

## US-02: The board, dialogs, and nav are usable at phone width

### Problem
With the viewport meta in place the page fits, but the layout is still desktop-shaped: the board's columns sit
side-by-side and run off-screen, the modals are sized for a wide window, and the sidebar rail eats half a
phone screen. Mei can see the page but can't comfortably *use* it.

### Who
- Workspace member on a phone | wants to scroll the board, open a card, and use the nav with a thumb.

### Solution
Add responsive `@media` rules so at mobile width: the board columns become intentionally scrollable/stacked
(not page-overflowing), modals/dialogs fit the viewport (full-width sheet, scrollable body), the sidebar/nav
collapses to a mobile affordance (e.g. a top bar / toggle), and primary controls are tappable (~44px targets).

### Elevator Pitch
Before: at phone width the board columns run off-screen, modals overflow, and the sidebar wastes half the
screen.
After: on a 390px browser → columns scroll intentionally, an opened dialog fills the screen as a scrollable
sheet, the nav collapses, and the New-issue / card controls are large enough to tap.
Decision enabled: Mei decides to actually triage on her phone, not just glance.

### UAT Scenarios (BDD)
#### Scenario: An opened dialog fits the phone screen
Given Mei is on the "Acme" board in a 390×844 browser
When she opens the new-issue dialog
Then the dialog fits within the viewport with no horizontal overflow
And its body scrolls if the content is taller than the screen

#### Scenario: The board columns do not overflow the page
Given Mei is on a board with several status columns in a 390×844 browser
Then the page itself has no horizontal overflow
And the columns are reachable by intentional scrolling within their container

### Acceptance Criteria
- AC-02.1: At 390px, the board, an open modal, and the issue page each have `scrollWidth <= innerWidth`; the
  columns container (not the page) owns any horizontal scroll.
- AC-02.2: An opened modal is ≤ viewport width and its body scrolls when taller than the viewport.
- AC-02.3: The sidebar/nav presents a mobile affordance at ≤ the mobile breakpoint (not the full desktop rail).
- AC-02.4: Primary tap targets (New issue, card open, nav items, dialog Save/Cancel) are ≥ ~44px in the
  smaller dimension at mobile width.
- AC-02.5: Desktop rendering (≥ the breakpoint) is unchanged — the `@media` rules are additive; the shipped
  desktop `@needs-browser` scenarios stay green.

---

## US-03: A member can install Foundry to their home screen

### Problem
Even fitting the screen, Foundry isn't an *app* — the browser offers no "Add to Home Screen", there's no icon,
and opening it always shows browser chrome. Mei wants it one tap away, launching clean like Linear or GitHub's
PWA.

### Who
- Workspace member (and instance admin) | on a mobile browser | wants a home-screen icon that launches
  standalone.

### Solution
Ship a valid web app manifest (`name`, `short_name`, `icons` incl. 192/512 + maskable, `theme_color`,
`background_color`, `display: standalone`, `start_url`, `scope`) as a static asset, link it from `base.html`,
add `theme-color` + `apple-mobile-web-app-*` meta and an `apple-touch-icon`. Include a minimal service worker
ONLY if DESIGN finds it required for the install prompt (ODD-3).

### Elevator Pitch
Before: no install option, no icon, always browser chrome.
After: open Foundry on a supporting mobile browser → the browser offers **Install / Add to Home Screen**; the
installed icon launches Foundry **standalone** (no browser address bar) at the board.
Decision enabled: Mei decides to keep Foundry on her home screen and treat it like a native app.

### UAT Scenarios (BDD)
#### Scenario: The manifest is linked, served, and valid
Given the shared layout renders an authed page
Then the head links a web app manifest
And fetching that manifest returns valid JSON with name, short_name, icons (192 and 512), theme_color,
    background_color, display "standalone", start_url, and scope

#### Scenario: The app declares standalone + theme color
Given the shared layout renders an authed page
Then the head contains a theme-color meta
And the manifest display mode is "standalone"

### Acceptance Criteria
- AC-03.1: `base.html` links `<link rel="manifest" href="/static/manifest.webmanifest">` (or `.json`), served
  with a sensible content-type + cache policy.
- AC-03.2: The manifest is valid JSON and contains all required fields (name, short_name, start_url, scope,
  display=standalone, theme_color, background_color, icons with 192 + 512 including a maskable icon).
- AC-03.3: `theme-color` meta + `apple-mobile-web-app-capable` / `apple-mobile-web-app-status-bar-style` +
  `apple-touch-icon` present.
- AC-03.4: Icons are real, served, and referenced at their declared sizes (fetch returns 200 image/png).
- AC-03.5: If a service worker ships (ODD-3), it registers without error and does NOT alter htmx behaviour or
  cache dynamic HTML (v1 has no offline requirement); if it does not ship, the install criteria are still met.
- AC-03.6: Installability asserted in the fantoccini lane to the extent chromedriver exposes it (manifest
  reachable + valid + `display=standalone` + theme-color present); the true OS install prompt is a browser
  dogfood item.
