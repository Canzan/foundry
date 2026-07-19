# Requirements — pwa-mobile-rendering

## Problem

Foundry cannot be used on a phone and cannot be installed like an app. A workspace member away from their desk
has no way to check or triage issues from a mobile browser — and the browser offers no "Add to Home Screen"
because the app declares none of the Progressive Web App primitives.

## Verified current state (in-tree, 2026-07-19)

The gap is foundational, not cosmetic:

| Concern | Current state |
|---------|---------------|
| Viewport meta | **Absent.** `base.html` `<head>` has no `<meta name="viewport">` — so mobile browsers render at a ~980px desktop width and zoom out; nothing renders "correctly" until this exists. |
| Responsive CSS | **Effectively none.** The single hashed stylesheet (`foundry.eb0e86f8.css`) has **zero `@media` breakpoints** (the `max-width`/`min-width` occurrences are `max-width: 100%`-type declarations, not responsive rules). The app is desktop-fixed. |
| Web app manifest | **Absent.** No `manifest.json`/`.webmanifest`, no `<link rel="manifest">`. Not installable. |
| Icons | **Absent.** No app icons (192/512 png, maskable, apple-touch-icon). |
| Theme / status-bar | **Absent.** No `theme-color`, no `apple-mobile-web-app-*` meta. |
| Service worker | **Absent.** No `sw.js`/service-worker registration. |

Frontend is server-rendered askama + htmx + vanilla JS; static assets served by `foundry-app`. No SPA, no
bundler, no Node.

## Non-negotiable decisions carried from intake

- **D-TOOL — Tests use the EXISTING fantoccini + chromedriver `@needs-browser` lane, NOT Playwright.** The user
  explicitly overrode the original "use Playwright" wording with *"Stick with fantoccini."* No `package.json`,
  no Node, no second browser stack — the repo stays Rust-only. Mobile viewport is driven by chromedriver window
  sizing / CDP device metrics in the shipped `BrowserHarness`. This is the load-bearing constraint on how
  "renders correctly on mobile" is verified.

## Constraints

- **CSS hash-rotation**: the stylesheet is hand-hashed (`foundry.<sha256-prefix>.css`); every CSS change MUST
  rotate the hash in **both** `base.html` (the `<link>`) and the hardcoded assertion at
  `crates/foundry-app/src/lib.rs:297`. No build-time hashing (precedent: navigation-bar-linear-ui evolution).
- **PWA install criteria** need a valid manifest served over **HTTPS** (or `localhost`). Modern Chrome no
  longer strictly requires a service worker for the install prompt, but some criteria / iOS behaviours differ —
  the minimal-SW-for-installability question is a DESIGN call (see US-03 / ODD).
- **No new server routes for content** — the manifest, icons, and (if any) service worker are **static assets**
  served by the existing `/static` mechanism; no new dynamic endpoints, no migration (latest stays `0014`).
- **Chrome-free / pre-auth pages** (signin, forgot, bootstrap_*, invite_accept, invalid_page,
  payload_too_large) must still render on mobile and carry the viewport meta, but need not be "app-shell"
  styled beyond not overflowing.
- **Progressive enhancement preserved**: the htmx/no-JS behaviour and every shipped acceptance oracle stay
  green; responsive CSS and the manifest are additive.

## Functional requirements

| # | Requirement | Story |
|---|-------------|-------|
| FR-1 | `base.html` carries `<meta name="viewport" content="width=device-width, initial-scale=1">`. | US-01 |
| FR-2 | The primary authed surfaces render with **no horizontal overflow** at a phone width (target 390px): dashboard, project board, issue page, and the modal dialogs. | US-01, US-02 |
| FR-3 | Responsive layout at mobile width: the board columns are usable (stack or horizontally scroll intentionally, not overflow the page), modals/dialogs fit the viewport, the nav/sidebar collapses to a mobile affordance. | US-02 |
| FR-4 | A valid web app manifest (`name`, `short_name`, `icons` incl. 192+512 + maskable, `theme_color`, `background_color`, `display: standalone`, `start_url`, `scope`) is served and linked from `base.html`. | US-03 |
| FR-5 | `theme-color` + `apple-mobile-web-app-*` meta present; app launches in **standalone** display mode when installed. | US-03 |
| FR-6 | Tap targets for primary controls meet a minimum size (WCAG 2.5.5 target ~44px) at mobile width. | US-02 |
| FR-7 | Every requirement above is asserted in the **fantoccini `@needs-browser` lane at a mobile window size** (viewport meta present; `scrollWidth <= innerWidth`; manifest link + served + valid fields; standalone display; controls visible/tappable). | all |

## Out of scope (v1) — flagged for DESIGN confirmation

- **Offline / service-worker caching** of pages or assets (htmx + SW caching has real correctness gotchas —
  stale fragments, CSRF, the hashed-asset cache story). v1 targets **installable + responsive**; a minimal SW
  is included ONLY if DESIGN finds it required for the install prompt (ODD-3). Full offline is a follow-up.
- **Native push notifications** (a separate, larger feature).
- **A separate mobile layout/route** — this is responsive CSS on the existing server-rendered pages, not a new
  mobile SPA.
- **Touch-specific interactions** (swipe gestures, long-press) beyond making existing controls tappable.
- **Playwright / any Node toolchain** (D-TOOL).

## Dependencies and seams

| Seam | Location | Use |
|------|----------|-----|
| Shared layout head | `crates/foundry-app/templates/base.html` | viewport meta, manifest link, theme-color, apple meta |
| Hashed stylesheet | `static/css/foundry.<hash>.css` (+ hash in `base.html` & `lib.rs:297`) | responsive `@media` rules |
| Static asset serving | `foundry-app` `/static` + `static_cache_control` (`lib.rs:~251-325`) | manifest.json, icons, sw.js |
| Browser test lane | `crates/foundry-acceptance/src/support/browser_harness.rs` (fantoccini, chromedriver) | drive at a mobile window, assert the DOM |
| Analog UI feature | `docs/feature/navigation-bar-linear-ui/` | house style + the hash-rotation precedent |
