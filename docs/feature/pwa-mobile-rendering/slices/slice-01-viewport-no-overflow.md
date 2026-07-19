# Slice 01 — Viewport meta + no horizontal overflow (walking skeleton)

**Goal**: the app fits a phone screen — add the viewport meta and ensure the primary authed pages don't
overflow horizontally at 390px — and build the fantoccini mobile-window oracle every later slice asserts through.

**Story**: US-01.

**IN scope**
- `base.html` `<head>` gains `<meta name="viewport" content="width=device-width, initial-scale=1">`.
- Minimal CSS to kill any obvious horizontal overflow at 390px on dashboard, board, issue page, open modal
  (e.g. `max-width: 100%` / `overflow-x` containment / box-sizing) — if any CSS lands, ROTATE the hash in
  `base.html` AND `lib.rs:297`.
- **The instrument**: a fantoccini `@needs-browser` helper that sizes the window to 390×844 (chromedriver
  `set_window_rect` / CDP device metrics) and reads `documentElement.scrollWidth` vs `innerWidth`; new step
  glue `feature_pwa_mobile.rs`, registered + force-linked; reuse `browser_harness` + sign-in/board nav helpers.
- Un-@pend US-01 scenarios.

**OUT of scope**
- Responsive layout of columns/nav/dialogs (slice 02); the manifest/icons (slice 03); offline/SW.

**Learning hypothesis**: disproves **"the fantoccini lane can drive a *mobile* window and measure the
viewport"** if chromedriver window sizing / CDP doesn't reliably produce a 390px layout viewport (headless
device-metrics quirks). Disproves **"the app already fits a phone"** — it does not (no viewport meta today).

**Acceptance**: `discuss/acceptance-criteria.md` US-01.

**Seams**: `base.html` head; `static/css/foundry.<hash>.css` + `lib.rs:297`; `browser_harness.rs`;
`navigation-bar-linear-ui` (hash-rotation precedent).

**Falsification**: the no-overflow scenario MUST be shown **RED against the current tree** (no viewport meta →
the mobile window renders at desktop width → `scrollWidth > innerWidth`). That RED is the defect reproduction
and the proof the oracle measures the real layout viewport, not a fixed window.

**Watch items**
- Headless chromedriver may need explicit device-metrics (CDP `Emulation.setDeviceMetricsOverride`) rather than
  just `set_window_rect` to get a true mobile *layout* viewport — resolve in the helper.
- CSS hash rotation ×2 or the shipped immutable-URL check fails.

**Dependencies**: none. **Effort**: ~0.5 day.
