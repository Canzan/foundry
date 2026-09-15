# ADR-BOARD-CARD-002: Drop feedback is recomputed on every dragover, and the insertion marker is a zero-footprint element that records the slot

## Status

Accepted (2026-09-13; card-drag-drop-feedback DESIGN wave, confirmed by the user).
Covers DDD-3 and DDD-4. It depends on ADR-BOARD-CARD-001 (delegated listeners, one
session).

## Context

Two kinds of feedback are added to the card drag. Both must survive a board replace
(D3), end clean on every exit (D5) and use existing tokens only at ≥3:1 (D6, D8).

**Lane activation.** HTML5 drag-and-drop fires `dragleave` on a lane when the
pointer enters one of its **children**. A naive "activate on `dragenter`, clear on
`dragleave`" blinks the lane off and on as the pointer crosses its cards (AC-2.2).
The two textbook remedies both carry state that a real browser can corrupt:

- A depth counter per lane drifts whenever a `dragleave` is lost, for example when a node is removed under the pointer or the board is replaced mid-drag. The lane then stays lit after the drag.
- A containment test on `relatedTarget` depends on a field that some engines report as null for drag events. Synthetic events always set it, so this remedy passes CI and flickers in a real browser.

**Insertion marker.** The slot is already computed by the midpoint rule
(`insertBeforeTarget`), and `after` is the key of the card above the landed card.
Two risks remain:

- An in-flow marker pushes the cards below it down, which moves the midpoints the rule reads (slice 03's oscillation hypothesis).
- A marker computed at the last `dragover` and a landing computed again at `drop` are two computations. If the pointer moves between them, the card can land in a slot the marker never showed (D7).

## Decision

**Activation: `dragover` is the only activator.**

1. On each session `dragover`, set `data-card-drop-target` on the lane resolved at
   event time and remove it from every other lane, found by query. Write the DOM only
   when the lane changes.
2. A `dragover` that resolves no lane clears activation.
3. A document `dragleave` whose `relatedTarget` is null *schedules* a clear for the
   next animation frame, and any `dragover` cancels it. This detects "the pointer
   left the window". It is robust even where every child crossing reports a null
   `relatedTarget`, because the crossing's own `dragover` follows in the same
   processing step and cancels the scheduled clear.
4. Style: `.column[data-card-drop-target]` gets an **inset `outline` in `--cz-muted`**
   (5.89:1 light, 6.38:1 dark against the page, recorded at the token seam). Being an
   outline, it takes no layout, the reason `.kb-selected` uses one, so AC-2.6
   (identical bounding rectangles) holds. The inset offset keeps it inside a board
   that scrolls horizontally. The surface shift is chosen in DELIVER from existing
   tokens (OQ-1).

**Marker: one zero-footprint element that *is* the slot.**

5. `slotFor(lane, y, card)` (the renamed `insertBeforeTarget`, with the same midpoint
   rule and the dragged card still skipped) runs on each session `dragover`.
6. The result is written to one `div[data-card-drop-marker]` inside the active lane,
   carrying `data-before-key` (the key of the card the slot precedes; empty means the
   end). The marker is absolutely positioned (`.column` is already `position:
   relative`, `css:1146`) in the 8px gap above that card, or below the last card,
   with `pointer-events: none`. It takes no space, so no card moves and a still
   pointer can never move it (AC-3.5). There is never more than one; placing it
   removes any other.
7. **On drop the card lands at the live marker's slot.** The drop handler resolves
   `data-before-key` inside the drop lane. `slotFor` runs again only when no marker
   sits in that lane (a drop with no prior `dragover`). `after` is still derived from
   the landed card by the shipped `neighbourAbove`, so the request body is unchanged.
8. Colour `--cz-black`: ≈13.8:1 light and ≈16.4:1 dark against `--cz-bg-2`,
   computed at design time. DELIVER re-measures with the suite's contrast oracle
   against whichever activated surface OQ-1 settles. It is a rule, the shape of
   `.lane-drop-indicator`, but not that rule's colour (1.49:1).
9. Teardown, shared by every exit: remove every `[data-card-drop-marker]` and every
   `data-card-drop-target`, found by query.

## Alternatives Considered

| Alternative | Rejected because |
|---|---|
| Activation by `dragenter`/`dragleave` depth counter | The counter drifts when a leave is lost. A lingering highlight is the D5 failure DISCUSS names worst. |
| Activation by `dragleave` + `relatedTarget` containment | Engine-dependent, and invisible to the synthetic-event lane (D12). |
| CSS `:hover`-style activation | No such state exists during a native drag; drag events are the only signal. |
| An in-flow marker element, the `.lane-drop-indicator` shape | Every card below the marker shifts as it moves. Oscillation can be argued away for insert-before semantics, but that is an argument, not a construction, and it makes the cards jump. |
| `::before`/`::after` on the neighbouring card via a class | It needs three variants (before a card, after the last, empty lane), `position: relative` on every card, and pseudo-elements that `querySelectorAll` cannot count, which weakens the exactly-one-marker oracle. |
| Recompute the slot at `drop` without consulting the marker | Two computations at two instants. The marker can show one slot while the card lands in another. |
| Store the last computed slot in the session | It holds a card node across events. A remote delete detaches it and `insertBefore` throws (ADR-BOARD-CARD-001). |
| Share the lane drag's indicator code | ADR-BOARD-LANE-007: the two drags share no code. They also differ in axis, footprint and contrast obligation. |

## Consequences

- Positive: activation has no counter to corrupt and no browser-specific field to
  trust. It converges on the next `dragover`.
- Positive: marker and landing are one datum. KPI 4 ("lands where the marker
  showed") holds by construction rather than by agreement between two computations.
- Positive: every DOM oracle is a plain count: `[data-card-drop-target]` and
  `[data-card-drop-marker]`, each exactly one or none.
- Negative: activation work runs on every `dragover` (~20 Hz while dragging). It is a
  `closest()` call, one query and, for the marker, one rect read per card in the
  active lane. That is negligible at board scale, and DOM writes happen only on
  change.
- Negative: AC-2.2 is only observable in the synthetic lane if the oracle is read
  **between** the `dragleave` and the next `dragover`. DISTILL must write it that
  way, or a naive clear-on-leave implementation passes.
- Obligation: slice 03's optional oscillation spike is moot and is not run.
