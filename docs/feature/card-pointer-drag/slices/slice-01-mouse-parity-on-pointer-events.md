# Slice 01 — The mouse drag, unchanged to the hand, on Pointer Events

**Story**: US-CPD-01 | **Estimate**: ~1 day | **Depends on**: nothing unshipped | **Type**: mechanism swap, parity-gated

## Goal

The card drag runs on Pointer Events for the mouse. Every shipped card-drag
scenario passes with its Gherkin byte-identical and only its input driver
re-pointed. A click still opens a card, Escape still cancels, and foreign drags
are still swallowed by the retained HTML5 listeners.

## IN scope

- `board-dnd.js`: a card session opened by a primary-button `pointerdown` on an `.issue-card` inside `#board-columns` that travels past the movement threshold (6 px lane precedent). Lane and slot are resolved **from the point** at event time. Activation, marker, landing-at-marker, `after`, POST, exact-origin revert and placeholder behaviour are all unchanged (D10).
- The retained HTML5 listeners keep swallowing foreign drags inside `#board-columns` only (D3). A native `dragstart` on an own card is prevented or never produced (OQ-4 decides how).
- Click suppression: the synthetic click after a drag's release never opens the edit dialog. A sub-threshold press still does (D6).
- The new `closeTopLayer()` arm for an in-flight card drag, with no `keydown` listener in the module (D11). A mouse `pointercancel` reverts (D12).
- A visible carried card that follows the cursor and does not block resolution under it (D15; ghost vs node is DESIGN's call, OQ-7).
- **Driver re-pointing:** the `browser_harness.rs` drag kit drives pointer input for card drags (keeping `DragSpot` live-geometry resolution). `drag_start_foreign` stays `DragEvent`. "presses Escape" becomes a real key press. `feature_board_lane_reorder.rs:1137` and `keyboard_shortcut_bindings.rs:3043` are re-pointed (C4).
- The 5 new `@us-cpd-01` scenarios.
- Stylesheet re-hash if CSS is touched (D18).

## OUT of scope

- Any touch- or pen-specific behaviour: hold, `touch-action`, callout suppression (slice 02).
- Edge auto-scroll (slice 03).
- Any server, template-protocol or POST change (D1).
- Any edit to a shipped `.feature` file (D4).

## Learning hypothesis

**Disproves, if it fails:** that the whole card-drag-drop-feedback contract is
input-agnostic, meaning the shipped Gherkin describes behaviour and not
`DragEvent` mechanics. If a shipped scenario can only pass by rewording it
(e.g. "presses Escape" cannot reach a real key handler, or a still-pointer
oracle depends on `dragover` cadence), that scenario encoded mechanism. The
slice stops and the user decides whether the Gherkin or the design changes.

**Confirms, if it succeeds:** slices 02-03 add only a lift rule and a scroll
rule on top of one proven session. The touch path is not a second
implementation.

## Acceptance criteria

AC-1.1 … AC-1.9 (see `feature-delta.md` US-CPD-01). Gate: `git diff` on every
shipped `.feature` = empty; all re-driven scenarios green; foreign scenarios
green on `DragEvent`; POST body byte-identical (fetch spy).

## Production data

Seeded Identity Platform (AUTH-3/12/19/41/42/43) and Homelab Ops (OPS-3/7/9,
empty Staging), the CDF fixtures, in the containerised
`selenium/standalone-chrome` lane. Run with `FOUNDRY_XTASK_INCLUDE_DOCKER=1`,
otherwise the browser lane is silently skipped. Check for a concurrent foundry
session first.

## Dogfood moment (manual, real browser)

Same day, on Priya's instance in Chrome and Firefox with a real mouse. Drag
AUTH-41 between AUTH-3 and AUTH-12 and reload. Click a card: its dialog opens.
Drag a card: no dialog opens. Press Escape mid-drag. Delete a card from its
popup, then drag without reloading. Drop `keys.png` from Finder on a lane:
nothing moves and the tab stays. Record the results in the delivery notes. Arm
an event recorder before blaming the product for a drag that "did nothing".

## Reference class

`board-lane-dnd.js` (pointer session, threshold, `pointercancel`, Escape arm,
post-`pointerup` click trap) and CDF slice 01 (replace-proof, test-first).

## Pre-slice SPIKE

None for mouse. The DESIGN-time touch spike for slice 02 runs in parallel.
