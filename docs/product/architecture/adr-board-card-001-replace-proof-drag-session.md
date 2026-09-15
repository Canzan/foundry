# ADR-BOARD-CARD-001: The card drag is one session behind document-delegated listeners, so it is replace-proof by construction

## Status

Accepted (2026-09-13; card-drag-drop-feedback DESIGN wave, confirmed by the user).
Covers DDD-1, DDD-2, DDD-6 and DDD-7. It builds on
`adr-board-lane-005-overflow-menu-as-layer-arm.md` rule 2 and does not amend it.

## Context

`board-dnd.js` binds `dragover` and `drop` to each `[data-column]` **once, at page
load** (`board-dnd.js:86-146`). Only `dragstart` is delegated. Five everyday
actions now replace `#board-columns` in place: the popup card delete (`87282d3`),
lane edit, insert and delete (OOB swaps), and lane move from the ⋯ menu
(`board-lane-dnd.js::applyBoard`). After any of them the lanes on screen carry
no listener. A lane-header drag only rearranges the existing lane nodes, so it is
not a replace, but delegation must hold across it too. *(Corrected at the
final review, 2026-09-13: this paragraph first counted the header drag among the
replaces; card-drag-drop-feedback DISTILL Upstream Issue #1. Fact only; the
decision is unchanged.)* Every card drop is then silently refused until a reload
(`rca-drag-after-board-replace.md`).

The rule that would have prevented this already existed in two places, each scoped
to its own module. ADR-BOARD-LANE-005 rule 2 made the lane menu's open state
DOM-derived, and `board-live.js` constraint 2 says "hold nothing; every event
re-queries the live document". Nobody applied the rule to the card drag, because it
had never been written down as a rule of the board.

Three more facts constrain the fix:

- `base.html:49` loads `board-dnd.js` on every page, so a document listener runs on
  the sign-in page and the issue page too.
- The shipped revert holds nodes past the drag's life: `from.insertBefore(card,
  fromNext)` runs when the POST answers. If `board-live.js` has removed `fromNext`
  in the meantime, `insertBefore` throws `NotFoundError`. If a replace has landed,
  the card reverts into a detached lane.
- A card dragged from another foundry tab carries `text/plain`, exactly like our own
  card drag, so the drag payload cannot tell the two apart. Only "did a `dragstart`
  happen on this page?" can.

## Decision

1. **Every card-drag listener is delegated on `document`.** The five listeners are
   `dragstart`, `dragover`, `dragleave`, `drop` and `dragend`. Each first resolves
   `event.target.closest('#board-columns')` and returns if it is null, so off the
   board the module does nothing. It then resolves the lane with
   `closest('[data-column]')` **at event time**. No listener is bound to any node
   inside `#board-columns`.
2. **One `CardDragSession` owns the in-flight drag.** Its lifecycle is `start(card)`
   → `over(lane, y)` → `drop(lane)` | `end()`. For one drag it holds the dragged card
   node plus the origin's **identity**: lane slug, next key and previous key. It
   never holds a lane, the active lane or the marker. `end()` is idempotent and tears
   down by querying the DOM. `dragend` always calls it, so drop, `Escape` and release
   outside all end clean. A new `dragstart` first ends any stale session.
3. **A foreign drag is any drag with no session.** Inside `#board-columns` with no
   session, `dragover` is `preventDefault()`-ed with `dataTransfer.dropEffect =
   "none"`, and `drop` is `preventDefault()`-ed and ignored. Nothing is activated,
   marked, moved or requested. `dataTransfer.types` is never inspected. *Fallback:*
   if the slice-01 dogfood finds a browser that navigates on a `"none"` drop, the
   swallow switches to accepting the drop (`"move"`) and cancelling it in the `drop`
   handler.
4. **The pending move holds no nodes.** The session ends at drop, so a second drag
   may begin before the response. The POST callbacks resolve the card by
   `data-issue-key` and the origin lane by slug, then the slot by next key, then
   previous key, then the end, all at response time. If the card is no longer on
   the live board, the revert is skipped, because a replace has already rendered
   server truth. The request body is byte-identical to the shipped one.
5. **The board-wide rule this records:** *a board behaviour binds no listener to a
   node inside `#board-columns`, and holds no such node past the event, or past the
   drag for the one dragged card.*

## Alternatives Considered

| Alternative | Rejected because |
|---|---|
| Keep per-lane binding and re-bind after every replace (`htmx:afterSwap`, plus a callback from `applyBoard`) | This is the RCA's fault with more steps. Five in-place refreshes ride two swap mechanisms, and the next trigger someone adds is the next silent refusal. |
| Watch `#board-columns`' parent with a `MutationObserver` and re-bind | A watcher of replaces, for a problem that delegation removes entirely. It adds a second moving part whose own failure is silent. |
| Delegate on `#board-columns` | The host is itself what gets replaced (`hx-swap-oob="true"` is an outerHTML swap), so the listener goes with it. |
| Discriminate foreign drags by `dataTransfer.types` | Cannot tell a card from another foundry tab (`text/plain`) from our own card drag, and synthetic events make it pass in CI regardless. |
| Keep the held-node revert (or use it while `isConnected`) | It throws or no-ops after a remote delete or a replace (see Context). The `isConnected` hybrid is two paths for one outcome. |
| Amend ADR-BOARD-LANE-005 to cover cards | ADRs are immutable, and 005 is about the lane menu as a `closeTopLayer()` arm. The board-wide rule deserves a record a reader can find by its subject. |
| A `check-arch` rule forbidding load-time lane listeners | Declined by the user (DISCUSS D15). The refresh-then-drag browser scenarios are the guard, and they also catch other shapes of the same fault, such as a stored lane node. |

## Consequences

- Positive: every card-drag behaviour, including the feedback added by later slices,
  survives any board replace without a hook. A lane inserted in place is a drop
  target immediately.
- Positive: `board-live.js`'s remote delete can no longer break a pending revert.
- Positive: the swallow no longer depends on which lanes happened to exist at load.
  It covers the gaps between lanes and every replaced lane.
- Negative: five `document` listeners on every page. Each returns off the board after
  one `closest()` call, and drag events fire only while something is being dragged,
  so the cost is nil in practice.
- Negative: the rule is held by acceptance scenarios, not by construction (D15). The
  US-CDF-01 regression, its five-refresh outline, the lane-header reorder guard and the "works after a refresh"
  scenario in each feedback slice are its standing proof. A future board script that
  binds per node at load will fail only if a scenario drags or clicks after a
  replace. New board behaviours must therefore carry such a scenario.
- Neutral: cards keep HTML5 drag-and-drop and lanes keep Pointer Events
  (ADR-BOARD-LANE-007). `keyboard.js` gains no arm, because a native drag's `Escape`
  is the browser's own cancel and arrives as `dragend` (BR-4 untouched).
