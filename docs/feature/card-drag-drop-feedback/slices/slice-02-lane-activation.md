# Slice 02 — The lane under a dragged card activates

**Story**: US-CDF-02 | **Estimate**: 0.5 day | **Depends on**: slice 01 (event-time lane resolution)

## Goal

While a card is dragged over a lane, that lane — and only that lane — visibly
activates, and every way the drag ends leaves no lane lit.

## IN scope

- Activation of exactly the lane under the dragged card; none while over no lane (AC-2.1).
- Steady activation while the pointer crosses the lane's own cards (AC-2.2).
- Teardown on every exit: drop, `Escape`, release outside any lane, refused or failed POST (AC-2.3, D5).
- No activation for foreign drags (AC-2.4, D4); works on replaced and inserted lanes (AC-2.7, D3).
- Styling from existing `--cz-*` tokens only; boundary ≥3:1 against the page in both palettes; no reflow (AC-2.5, AC-2.6, D6).
- Stylesheet content-hash rename across `base.html`, `lib.rs` tests and `static/VENDOR.md` in the same change (AC-2.8, D13).

## OUT of scope

- The insertion marker (slice 03) and placeholders (slice 04).
- A new colour token or hue: ruled out by the user (D6). Activation is a stronger surface and boundary change from existing tokens.
- Touch, keyboard, announcements (D14).

## Learning hypothesis

**Disproves, if it fails:** (a) that a delegated activation can stay steady while
the pointer passes over the lane's child cards — the HTML5 `dragleave`-on-child
trap; and (b) that a change built from the neutral palette reads as
"activating" to Priya. If (a) fails, activation must be derived from the lane
under the pointer on every `dragover` instead of enter/leave — a design change,
not a scope change. If (b) fails at dogfood, the change is strengthened within existing tokens. A
hue is not added (D6); the finding goes back to the user instead.

**Confirms, if it succeeds:** the exit-path teardown works for a class on a lane,
and slice 03 extends it to a positioned element.

## Acceptance criteria

AC-2.1 … AC-2.8 (see `feature-delta.md` US-CDF-02).

## Production data

Seeded Homelab Ops board (four lanes, OPS-3 in Backlog, OPS-9 in Done), light
and dark palettes, through the `@needs-browser` lane; the contrast oracle reads
the computed colours.

## Dogfood moment (manual, real browser)

Same day, on a real board in Chrome and Firefox, light and dark: drag a card
slowly across three lanes and over the cards inside them — watch for flicker;
press `Escape` mid-drag; release over the page header. Nothing may stay lit.

## Reference class

`board-lane-reorder` slice 03 (drop indicator + teardown on every exit + re-hash, 0.5 day).

## Pre-slice SPIKE

None.
