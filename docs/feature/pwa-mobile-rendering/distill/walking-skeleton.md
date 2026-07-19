# Walking Skeleton — pwa-mobile-rendering

The load-bearing unknown is the **mobile oracle**: can the fantoccini lane drive a *faithful* mobile viewport
(so the viewport-meta effect is measurable), and does the app fit a phone once the meta lands? The skeleton is
`open_mobile_session()` + the viewport meta + the no-overflow assertion, end to end.

## First failing test (DELIVER entry)

**S1 — "A mobile browser session fits the board to the screen end to end"** (`@lane-probe @walking_skeleton`),
then **S2** (the primary surfaces fit).

RED → GREEN (slice 01):
1. **RED (the defect, in a real mobile viewport)**: build `open_mobile_session()` (chromedriver
   `goog:chromeOptions.mobileEmulation`, deviceMetrics 390×844 mobile:true; do NOT resize to desktop after
   connect). Drive the board and assert `documentElement.scrollWidth <= window.innerWidth`. On the current
   tree (no viewport meta) mobile Chrome uses the 980px fallback → overflow → RED. This is the first test that
   can see the defect.
   - Throwaway proof of ADR-003: under a plain narrow-window session the same no-viewport tree would PASS —
     confirming emulation (not resize) is required.
2. **GREEN (minimal)**: add `<meta name="viewport" content="width=device-width, initial-scale=1">` to
   `base.html`; add minimal CSS to kill obvious overflow at 390 if needed (rotate the hash in `base.html` AND
   `lib.rs:297`). Re-run: no overflow → GREEN. S2 (dashboard/board/issue/open-modal) green.
3. `cargo xtask smoke` + the `@needs-browser` mobile scenarios for slice 01 green; commit. Then DOGFOOD on a
   real phone (legibility at 1×).

## Slice sequence
1. **Slice 01** (skeleton) — `open_mobile_session()` + viewport meta + no-overflow. S1, S2, S11 (hash guard).
2. **Slice 02** — responsive `@media` (columns scroll, modal sheet, nav reflow, 44px) + S3–S6; the DESKTOP
   non-regression guard S7. Hash rotates ×2.
3. **Slice 03** — `manifest.webmanifest` + icons + head meta (theme-color, apple, manifest link); no SW.
   S8–S10. Verify `.webmanifest` content-type; HTTPS-install is a dogfood note.

## Lane safety
All `@pending` → excluded from every lane. `fail_on_skipped()` stays ON. `@needs-browser` is in the `all`
lane, excluded from the fast default (the shipped split). Bounded `wait().for_element`, never sleeps; clean up
chromedrivers between runs (the session-hazard note). Full `@all` at finalize.

## Why the oracle is the deliverable
The viewport meta is one line; the durable value is a fantoccini oracle that faithfully renders a MOBILE
viewport (ADR-003) — so "renders correctly on mobile" is a measured layout fact, not a screenshot or a
green-over-nothing narrow-window check. Every later slice asserts through it.
