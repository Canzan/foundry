# Evolution — pwa-mobile-rendering (Foundry fits a phone and installs like an app)

**Finalized**: 2026-07-19
**Commits**: DISCUSS `ee574eb`, DESIGN `9e9d370`, DISTILL `c1d1682`, DELIVER `bd1ea26` → `329616e` (3
DES-monitored TDD steps = 3 slices). Full pipeline (`/nw:new` → DISCUSS → DESIGN → DISTILL → DELIVER).
Trunk-based; DES integrity exit 0; adversarial review APPROVED (0 defects). Feature dir PRESERVED. **Not
pushed.**
**Scope**: Foundry was desktop-only — `base.html` had NO `<meta name="viewport">`, the stylesheet had **zero
`@media` breakpoints**, and there was no manifest — so a phone rendered it at ~980px, zoomed out, and couldn't
install it. This ships the viewport meta, responsive CSS, and an installable manifest, all as head tags / CSS /
static assets: **no new route, no migration** (latest stays `0014`), **no Node**, **no service worker**.
**Test tooling**: the user asked for Playwright, then overrode it with *"stick with fantoccini."* Verified in
the EXISTING `@needs-browser` lane via a NEW mobile-emulation session — no second browser stack.

## Milestone — the app fits a phone, is usable with a thumb, and installs to the home screen

Three thin slices, each dogfoodable on a real phone:
- **Slice 01** — `<meta name="viewport">` + the mobile oracle. The board fits a 390px viewport with no
  horizontal overflow.
- **Slice 02** — responsive `@media (max-width:480px)`: board columns scroll within their container (page
  never overflows), the dialog becomes a full-width scrollable sheet, the sidebar reflows to a top bar
  (CSS-only, no new JS), tap targets ≥ 44px. Desktop untouched.
- **Slice 03** — a valid `manifest.webmanifest` + 192/512/maskable + apple-touch icons + `theme-color` + apple
  meta. Installable, launches standalone. No service worker (ADR-002).

## The oracle was the real work — and it caught itself being dishonest twice

The one-line viewport meta is trivial; the durable value is a fantoccini oracle that *faithfully* renders a
mobile viewport. Two green-over-nothing traps were found and fixed during DELIVER:

1. **Emulation, not a narrow window (ADR-003).** `--headless=new` is *desktop* Chrome: at a narrow window it
   lays out at the window width regardless of the viewport meta, so a resize-only test would be green whether
   or not the meta exists. Proven empirically (narrow-resize clamps `innerWidth` to 500; the no-viewport tree
   passes vacuously). The fix: `open_mobile_session()` injecting chromedriver
   `goog:chromeOptions.mobileEmulation` (deviceMetrics 390×844, `mobile:true`) — real mobile viewport
   semantics.
2. **`clientWidth`, not `innerWidth`.** Under emulation `window.innerWidth` *expands to the overflowed content*
   — a 390px layout with an 848px board reported `innerWidth == scrollWidth == 1120`, so `scrollWidth <=
   innerWidth` passed **vacuously over a real overflow**. The fix: assert against
   `documentElement.clientWidth` (the fixed 390 layout viewport that a horizontally-overflowing surface
   exceeds). Plus an exact-URL anti-vacuity guard so a sign-in redirect race can't measure the wrong page.

The review's verdict on this dimension: *"a naive test would measure window.innerWidth (vacuous under
emulation), but the crafter knows the substrate and uses documentElement.clientWidth."* Both are the standing
"a green can be an artefact of the instrument" lesson, caught by execution.

## Falsification demonstrated, not asserted

Each green was shown red first: no-viewport → board overflows a 390 layout viewport (S1/S2); pre-`@media` →
dialog 4340px / columns overflow the page (S3/S4); an **unbounded `@media`** collapses the *desktop* rail (S7,
the blast-radius guard); a manifest referencing a missing icon → 404 (S9). Mutation testing has no product
Rust target (CSS + static assets + head tags + a test-infra helper) — RED_UNIT is `SKIPPED`/`NOT_APPLICABLE`
on all three steps, the client-layer precedent.

## What shipped

- `open_mobile_session()` (browser_harness.rs, +58) — reusable mobile-emulation session; the desktop
  `open_session` is untouched (no desktop-lane regression).
- `base.html` head — viewport meta, `<link rel="manifest">`, `theme-color`, apple meta + apple-touch-icon.
- `foundry.7c858984.css` (renamed from `eb0e86f8` via two hash rotations, in `base.html` + `lib.rs:297`) —
  the `@media` responsive block.
- `static/manifest.webmanifest` (served as `application/manifest+json` by `ServeDir`/mime_guess — no
  `manifest.json` fallback needed) + `static/icons/{192,512,maskable-512,apple-touch}.png`.
- 11 `@needs-browser` mobile-emulation scenarios (14 with the outline rows) + the source-level CSS-hash guard.

## Open / deferred

- **Offline / service worker** — deliberately OUT (ADR-002); a follow-up with its own caching contract.
- **Touch drag-and-drop** — `board-dnd.js` still functions at mobile width but touch-DnD usability is a noted
  v1 limitation (read/triage is the job).
- **Human phone dogfood** — legibility at 1×, the real OS install prompt (needs HTTPS), and thumb-reach are
  human checks the headless lane can't judge; the lane asserts layout/manifest *facts* only.
- **Hamburger drawer nav** — v1 uses a CSS-only top-bar reflow; a drawer (needs JS) is a follow-up if the top
  bar proves cramped.
