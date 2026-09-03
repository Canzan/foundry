# Slice 03 — Drag past the edge of a board wider than the screen

**Story**: US-BLR-03 | **Estimate**: 0.5 day | **Depends on**: slice 02 (the drag)

## Goal

A drag held near the edge of a horizontally scrollable board scrolls the board
under it, with a visible indicator showing the slot the lane will land in.

## IN scope

- Edge-zone auto-scroll during a lane drag, bounded by the board's scroll extent, never scrolling the page (D15, AC-3.1, AC-3.2).
- A drop indicator showing the destination slot, tracking the pointer across an auto-scroll (AC-3.3).
- Indicator teardown on every exit path: drop, `Escape` cancel, `pointercancel` (AC-3.4).
- Indicator and dragged-column styling from existing `--cz-*` tokens only, legible in both palettes (AC-3.5).
- Stylesheet re-hash and the `static/VENDOR.md` row updated in the same change (AC-3.7).
- A `@needs-browser @mobile` scenario dropping a lane at a destination only reachable by auto-scroll, at a 390px viewport.

## OUT of scope

- Vertical auto-scroll (the board does not scroll vertically at the lane level).
- Auto-scroll during a *card* drag — `board-dnd.js` is untouched.
- Any change to the write port or the addressing scheme; auto-scroll changes what is visible, never what is addressed (AC-3.6).

## Learning hypothesis

**Disproves, if it fails:** that lane reorder is usable on a phone at all. The
mobile RCA established that `.board` genuinely scrolls horizontally below
480px, so without auto-scroll a touch drag reaches roughly one lane in each
direction — which would make KPI 5 unmeetable and push reorder-on-mobile back
onto the menu path alone.

**Confirms, if it succeeds:** the drag is complete on the boards where
placement is hardest — the wide ones — and no further drag work is outstanding.

## Acceptance criteria

AC-3.1 … AC-3.7 (see `feature-delta.md` US-BLR-03).

## Production data

An eight-lane seeded board at a 390px viewport — wide enough that the
destination is genuinely off-screen, which a three-lane fixture would not prove.

## Dogfood moment

Same day: on a phone, drag the leftmost lane of a real wide board to the far
right across an auto-scroll.

## Pre-slice SPIKE

None — auto-scroll is well-trodden, and `keyboard.js::dismissLaneMenuOnScroll`
already proves the board emits its own scroll events in capture phase.
