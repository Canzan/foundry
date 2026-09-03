# Slice 02 — Drag a column header to move a lane

**Story**: US-BLR-02 | **Estimate**: 1 day | **Depends on**: slice 01 (the write port)

## Goal

Pressing a column header and sliding it — with a mouse, a finger or a pen —
moves the lane, committing through the same port and transaction slice 01
proved.

## IN scope

- A new browser module for the lane drag, built on **Pointer Events** so touch works (D3) — deliberately not the `dragstart`/`drop` mechanism `board-dnd.js` uses for cards.
- Movement **threshold** so a press that does not travel still delivers its click: `⋯` keeps opening the menu (D2).
- Optimistic DOM move on release, one POST naming the destination **neighbour by slug**, revert to the **exact** origin slot on non-2xx or network error — `board-dnd.js`'s shipped behaviour, copied (D6, D7).
- `Escape` cancels an in-flight drag as a **new arm of `closeTopLayer()`**, above the lane-menu arm; `pointercancel` reverts identically (D10, BR-4).
- Drag-in-flight CSS from existing `--cz-*` tokens, correct in both palettes.
- Browser-lane scenarios for the drag, the `Escape` cancel, the refusal revert, and the drop carrying a **real** CSRF token (not the HTTP lane's injected one).
- A `@mobile` scenario proving the touch drag at a 390px viewport.
- Regression: the shipped card drag-and-drop scenarios pass **unmodified** (D16).

## OUT of scope

- Auto-scroll at the board's edge and the drop indicator (slice 03).
- Any change to the write port, transaction or refusals — slice 01 owns those.
- Migrating `board-dnd.js`'s card drag onto Pointer Events.

## Learning hypothesis

**Disproves, if it fails:** that two drag systems can coexist in one board
region — a Pointer Events lane drag in the header, a native HTML5 card drag in
the body — without either stealing the other's gesture. A failure means the
divergence D3 records is not sustainable, and converging cards onto Pointer
Events stops being a deferred successor and becomes a prerequisite.

**Confirms, if it succeeds:** the threshold is a sufficient boundary between a
header's click targets and its drag gesture, and slice 03's refinements are
additive.

## Acceptance criteria

AC-2.1 … AC-2.8 (see `feature-delta.md` US-BLR-02).

## Production data

Real seeded boards with cards in every lane, so the card-drag regression is
exercised against actual `.issue-card` elements rather than an empty board.

## Dogfood moment

Same day: drag a real lane on the live board with a mouse, then again on a
phone with a finger.

## Pre-slice SPIKE

None — but note the mechanism divergence needs its **ADR recorded in DESIGN**
(D3) before this slice lands, so a future reader finds a decision rather than
an inconsistency between two drag implementations.
