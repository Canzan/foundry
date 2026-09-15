# Slice 03 — A marker shows exactly where the card will land

**Story**: US-CDF-03 | **Estimate**: 0.75 day | **Depends on**: slice 01; reuses slice 02's exit-path teardown if landed first

## Goal

During a card drag, a single marker shows the slot the card will take — before,
between or after any card — and the card lands exactly there, with a reload
agreeing.

## IN scope

- One marker on the board while over a lane; none while over no lane (AC-3.1).
- One slot computation feeding the marker, the optimistic landing and the POST's `after` (AC-3.2, D7). `after` omitted for the top slot (unchanged, D9).
- The dragged card's own slot never offered as a neighbour (AC-3.4).
- No oscillation: a still pointer keeps the marker in one slot (AC-3.5, D8).
- Teardown on every exit, including a refused drop (AC-3.6).
- A rule between cards (the `.lane-drop-indicator` shape), coloured from existing tokens at ≥3:1 against the lane surface in both palettes; stylesheet re-hash (AC-3.7, D8, D13).
- Reload-equality oracle after each successful drop (AC-3.3, D11).

## OUT of scope

- Any change to how the server orders or persists (`reposition_issue_with_outbox` untouched).
- Re-colouring `.lane-drop-indicator` (a separate follow-up; see *Out of Scope* in `feature-delta.md`).
- Auto-scroll during a card drag.
- Suppressing a no-op drop into the card's own slot.

## Learning hypothesis

**Disproves, if it fails:** that a marker can sit among the cards without shifting
the geometry the slot calculation reads. An in-flow marker pushes the cards below
it down, which moves their midpoints; if that makes the marker jump between two
slots under a still pointer, the marker must take no space in the card flow — a
design change that DESIGN resolves before this slice is planned.

**Confirms, if it succeeds:** what Priya sees is where the card lands — KPI 4 —
and the shipped positional drop becomes usable without a corrective second drag.

## Acceptance criteria

AC-3.1 … AC-3.8 (see `feature-delta.md` US-CDF-03).

## Production data

Seeded Identity Platform board with In-Progress holding AUTH-3, AUTH-12, AUTH-19
and AUTH-41 in Backlog; order verified after reload against the store's
`issues.position`.

## Dogfood moment (manual, real browser)

Same day, on a real board with 6+ cards in one lane: drag a card slowly down the
lane and hold still near a card boundary — the marker must not flicker between
two slots; drop at the top, middle and bottom and reload each time.

## Reference class

`card-ranking-within-status` slice 01 (the insertion index in `board-dnd.js`) and
`board-lane-reorder` slice 03 (the indicator).

## Pre-slice SPIKE

**Not run.** DESIGN DDD-4 (accepted 2026-09-13, ADR-BOARD-CARD-002) chose a
zero-footprint marker: one absolutely positioned element that also records the slot.
Its footprint is zero, so it cannot move the geometry the slot calculation reads. The
spike's condition ("run only if DESIGN has not already chosen a zero-footprint
marker") is therefore not met.

*Original text, kept for the record:* Optional, ≤1 hour: measure whether a 2–3px
in-flow rule causes oscillation at a card midpoint in Chrome and Firefox. Run only
if DESIGN has not already chosen a zero-footprint marker.
