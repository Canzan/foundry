# ADR-003 — The mobile oracle uses chromedriver `mobileEmulation`, not a narrow window

**Status**: Accepted (2026-07-19) · **Story**: all (the test mechanism) · **Resolves**: the DISCUSS slice-01 watch item

## Context

The tests must run in the existing fantoccini `@needs-browser` lane (D-TOOL, no Playwright). The shipped
`open_session` (browser_harness.rs) starts `--headless=new` at a **fixed desktop** `--window-size` and calls
`set_window_size(WINDOW_WIDTH, WINDOW_HEIGHT)`. The naive approach — just resize the window narrow — does **not
faithfully test mobile rendering**: headless `--headless=new` is *desktop* Chrome, and desktop Chrome uses the
window inner width as the layout viewport regardless of the viewport meta. It does **not** apply the ~980px
"desktop fallback" that a *mobile* browser applies when a page lacks `width=device-width`. So a narrow-window
test would neither reproduce the current defect (no-viewport → zoomed-out) nor measure the viewport-meta fix —
it would be green whether or not the meta exists. That is exactly the "green over nothing" failure this project
keeps catching.

## Decision

Add **`open_mobile_session()`** to `browser_harness.rs`: same as `open_session` but inject
`goog:chromeOptions.mobileEmulation` with `deviceMetrics` (`width: 390, height: 844, pixelRatio: 3, mobile:
true`) — and do **not** call `set_window_size` to the desktop dims afterward (the emulated deviceMetrics govern
the layout viewport). This makes headless Chrome apply **real mobile viewport semantics**:

- WITHOUT the viewport meta → the mobile ~980px fallback → the board overflows a 390 layout viewport (**RED**,
  the faithful defect reproduction).
- WITH the meta → layout viewport = 390 → no overflow (**GREEN**).

The harness already assembles `goog:chromeOptions` as a `serde_json` value, so this is a small additive
variant. Assertions read layout facts via `execute` / element rects (`documentElement.scrollWidth` vs
`window.innerWidth`, dialog width, control bounding boxes) and fetch/parse the manifest.

## Alternatives considered

- **`set_window_size(390, 844)` on a normal session** — rejected: desktop headless Chrome lays out at the
  window width irrespective of the viewport meta, so the meta's effect is invisible and the test can't
  distinguish fixed vs broken. Green over nothing.
- **CDP `Emulation.setDeviceMetricsOverride` at runtime** — equivalent semantics, but requires issuing raw CDP
  through chromedriver mid-session; `goog:chromeOptions.mobileEmulation` at session creation is simpler,
  declarative, and well-supported by chromedriver.
- **Playwright device descriptors** — rejected by D-TOOL (adds a Node stack).

## Consequences

- A **separate session type** for mobile scenarios; the shipped desktop `open_session` scenarios are untouched
  (no desktop regression risk from the mobile work).
- The falsification is real: the slice-01 no-overflow scenario is RED on the current (no-viewport) tree under
  `open_mobile_session()`, and GREEN after the meta — proving the oracle measures the true mobile viewport.
- The oracle proves **layout facts**, not "looks right": legibility, the OS install prompt, and thumb-reach
  stay human phone-dogfood items (the standing "a green lane can be an artefact of the instrument" discipline).
- `mobileEmulation` also sets a mobile user-agent + touch; keep assertions to layout/manifest facts so they're
  deterministic in headless.
