# ADR-BOARD-LANE-007: Lanes drag on Pointer Events; cards keep HTML5 drag-and-drop

## Status

Accepted (board-lane-reorder DESIGN wave, 2026-09-03)

## Context

`board-lane-reorder` adds a drag gesture to the board's column headers. The
board already has a drag gesture: `board-dnd.js` moves **cards** using native
HTML5 drag-and-drop (`dragstart` / `dragover` / `drop`), shipped with
`issue-status-move` slice 02 and extended by `card-ranking-within-status`.

The obvious choice is to build the lane drag the same way. DISCUSS (D3)
rejected it for one disqualifying reason: **HTML5 drag-and-drop emits no events
on touch input.** A lane drag built on `dragstart` would be inert on a phone —
days after `fix-lane-menu-clipped-mobile` shipped specifically to make this
board usable on one, and while KPI 5 requires reorder to complete by touch at a
390px viewport.

This leaves the board with two drag mechanisms. That is a real inconsistency,
and an undocumented inconsistency is indistinguishable from drift. This ADR
exists so a future reader finds a decision.

## Decision

**Lanes drag on Pointer Events. Cards keep HTML5 drag-and-drop. The divergence
is deliberate and, for now, permanent.**

The lane drag lives in its own module (`static/js/board-lane-dnd.js`),
delegating from the column header, and:

- begins only after the pointer travels past a movement threshold, so a press that does not travel still delivers its click to the `⋯` trigger inside the same header (D2);
- moves the DOM optimistically on release and reverts to the **exact** origin slot on a non-2xx response or a network error — `board-dnd.js`'s shipped behaviour, copied deliberately so the two drags feel identical to the hand even though they are not the same code (D6);
- names its destination by **neighbour slug**, mirroring the card drag's `after` parameter and ADR-BOARD-LANE-006's identity resolution (D7);
- cancels on `Escape` through a new arm of `keyboard.js::closeTopLayer()`, never its own `keydown` listener (D10, BR-4);
- commits through the same use case the `⋯` menu's Move items call — one write seam, two surfaces (Driving Port 3).

### Why not converge now

Migrating `board-dnd.js` onto Pointer Events would remove the inconsistency and
would give cards a touch drag they equally lack today. It is deliberately **not**
in this feature:

- it is a rewrite of a shipped, mutation-tested interaction with its own regression surface (`card-ranking-within-status`'s ranking semantics, the `after` protocol, the exact-origin revert);
- it would put a rewrite of working code on the critical path of a feature that adds a capability;
- it is separable — nothing in the lane drag depends on how cards drag.

It is recorded as the first deferred successor in `board-lane-reorder`'s
feature-delta, and this ADR is the reason that successor exists.

### The boundary that must hold

The two mechanisms share one region of the DOM, so the feature's real
regression risk is gesture theft (D16). The boundary is **origin-based and
absolute**: a gesture beginning on `.issue-card` is a card move; a gesture
beginning on the column header is a lane move; neither ever becomes the other.
The shipped card-drag acceptance scenarios must pass **unmodified** — not
adapted — as the standing proof.

## Alternatives Considered

| Alternative | Rejected because |
|---|---|
| Native HTML5 DnD, matching `board-dnd.js` | Emits nothing on touch. Consistency with a mechanism that cannot serve half the devices is not consistency worth having, and it would make KPI 5 unmeetable. |
| A drag library (SortableJS, dragula, etc.) | Solves touch, auto-scroll and the drop indicator in one dependency. Rejected on this repo's standing posture: the presentation tier is hand-authored with no build step, every vendored asset carries a recorded sha256 in `VENDOR.md`, and `check-arch` R1–R3 police that pipeline. A ~100-line module against a platform API is cheaper here than a vendored library. |
| Converge cards onto Pointer Events first, then build the lane drag on it | Architecturally the tidiest order, and rejected on sequencing: it puts a rewrite of shipped, working code ahead of the capability the user asked for, and it would make this feature's estimate depend on a regression surface it does not own. |
| Pointer Events for lanes AND a compatibility shim so cards keep working unchanged | A shim implies the two systems interact. They do not — the boundary is which element the gesture starts on, which needs no shared code at all. |
| Keyboard-only reorder, no drag | Already rejected at DISCUSS (D4 ships the menu path *as well*, not instead). The menu alone would satisfy the job but not the request. |

## Consequences

- The board carries two drag implementations. This ADR is the record that it is a choice; `brief.md` §lanes points here.
- `keyboard.js` gains a fourth arm on `closeTopLayer()` — the second use of the ADR-BOARD-LANE-005 arm pattern, which is evidence the pattern generalises rather than a special case.
- Cards still cannot be dragged on touch. That gap predates this feature and is not widened by it, but it becomes more visible once lanes *can* be — a user who can drag a column but not a card will read that as a bug. The deferred convergence is the answer, and it should be prioritised accordingly.
- Pointer Events require explicit handling of pointer capture and `pointercancel`; the latter is a real exit path (a system gesture stealing the pointer) that HTML5 DnD hid, so the revert logic has one more entry point than `board-dnd.js` does.
