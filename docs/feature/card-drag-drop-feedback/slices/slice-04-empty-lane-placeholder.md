# Slice 04 — Empty lanes read truthfully during and after a drag, and after a remote delete

**Story**: US-CDF-04 | **Estimate**: 0.5 day (DISCUSS 0.75; DESIGN DDD-5 removes both client placeholder writers) | **Depends on**: slice 01

## Goal

A lane shows "No issues yet" exactly when it holds no card, and never when it
does. That includes a lane that just received a drop, a lane a drag just
emptied, a refused drop, and a lane whose last card another tab deleted. The
board matches a reload without one.

## IN scope

*Mechanism (DESIGN DDD-5, ADR-BOARD-CARD-003, accepted 2026-09-13):*
`partials/board_columns.html` renders the existing `<p class="empty">` in **every**
lane. One CSS rule pair (`.column > .empty { display: none }` and
`.column:not(:has(> .issue-card)) > .empty { display: block }`) shows it only in a
lane holding no card. **No script writes, clones or restores a placeholder.** The
stylesheet is re-hashed per D13.

- Destination: the placeholder stops displaying on a successful optimistic drop (AC-4.1). This follows from the CSS, not from a script.
- Origin: the placeholder displays when a drag takes the lane's last card (AC-4.1). This follows from the CSS.
- Revert: a refused or failed POST restores both lanes' cards (ADR-BOARD-CARD-001), and the placeholders follow by construction (AC-4.3).
- **Remote delete (D16):** when `board-live.js` drops a card on `IssueDeleted` and that leaves its lane holding no card, the lane shows the placeholder without a reload. Other lanes are untouched (AC-4.7). **`board-live.js` is unchanged.**
- One source for the placeholder's words and markup: the server's own node, rendered identically in both render paths. No client copy and no client writer (AC-4.2, D10).
- Oracle note for DISTILL: "no placeholder" means **not displayed** (computed `display`), not absent from the DOM.
- Hover, cancel and foreign drags never change a placeholder (AC-4.4).
- A reload-equality check for both lanes after every successful drag (AC-4.5, D11). Works on replaced and inserted lanes (AC-4.6).

## OUT of scope

- Changing the placeholder's words or style.
- Any server-rendered fragment after a drop (D9); the POST response stays a state chip.
- Handling any SSE event other than `IssueDeleted`; a general live board is a separate feature.

## Learning hypothesis

**Disproves, if it fails:** that the placeholder can be kept truthful in the
browser by a declarative rule alone (DDD-5), with the server's own node as the
single source and no script writing it, including on revert and after a remote
delete. If a lane ever displays a placeholder beside a card, or none while empty,
the fallback is ADR-BOARD-CARD-003's `<template>` alternative. It is documented
only, and adopting it brings back two client writers and the 0.75d estimate. That
decision goes back to the user.

**Confirms, if it succeeds:** the board a user sees equals a reload in every
lane a drag or a remote delete touches. That closes RCA finding 3 and KPI 5.

## Acceptance criteria

AC-4.1 … AC-4.7 (see `feature-delta.md` US-CDF-04).

## Production data

A seeded Homelab Ops board with an empty Staging lane and In-Progress holding
only OPS-7. The refusal case uses a real concurrent delete of OPS-7, so the
server's uniform 404 drives the revert. The remote case uses two real browser
sessions on the same board through the shipped SSE stream (the
`issue-card-delete` two-window idiom).

## Dogfood moment (manual, real browser)

Same day, on a real board:

1. Drag the only card out of a lane and into an empty one, then reload. Nothing may change.
2. Have a drop refused (delete the card in a second tab first). Both lanes must read as they did before the drag.
3. With the board open on two screens, delete a lane's last card on one. The other must show "No issues yet" in that lane.

## Reference class

`issue-status-move` slice 02 (optimistic move with an exact-origin revert) and
`issue-card-delete` slice 03 (the `board-live.js` two-window path).

## Pre-slice SPIKE

None.
