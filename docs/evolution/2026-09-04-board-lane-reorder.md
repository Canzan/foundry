# board-lane-reorder — evolution archive

Change a board's lane order in place. Users can **drag a column header** left or right
with Pointer Events, so the drag works with a mouse, a finger or a pen. They can also
use **Move list left / Move list right** in the `⋯` menu, which covers the keyboard,
assistive technology and moves that need precision.

Waves: DISCUSS → DESIGN (with a measured spike) → DISTILL → DELIVER. There was no
DISCOVER, no DEVOPS and no walking skeleton (D13). Landed on `main` as `84d025e`
(2026-09-03), followed by `da53d9f` and `2bcc356` (2026-09-04). Finalized
2026-09-25. The workspace `docs/feature/board-lane-reorder/` is preserved as the
full history; `feature-delta.md` is the SSOT for every wave.

## What shipped

- **One store transaction**, `Store::move_lane_before(project, mover, before?)`. It runs a single `UPDATE lanes SET position = CASE …` statement inside a `FOR UPDATE` transaction and resolves both ends by identity inside the lock. A move writes `lanes.position` and nothing else. **No migration**: the counter stays at 0015.
- **One use case** (`foundry_services::lanes::move_lane`) is the single seam for both surfaces, and one client path (cookie→header `fetch`) serves both.
- **The `⋯` menu grows from four items to six.** The two Move items are *rendered but disabled* at the board's ends, never omitted, so the item count stays the same for every column.
- **A new module, `board-lane-dnd.js`**, provides the Pointer Events drag, a movement threshold (so `⋯` stays clickable), edge auto-scroll, the drop indicator and optimistic revert-on-failure. Drag-cancel became a new arm of `closeTopLayer()`.
- **A new `check-arch` rule, `DEFERRABLE`**, fails closed if `0015`'s position constraint loses the keyword. It has 5 gold tests.
- **Tests:** 26 acceptance scenarios (26/26 `blr`, 141 steps), including 11 `@needs-browser` scenarios and 2 `@mobile` touch scenarios at 390px.
- **Records:** ADR-BOARD-LANE-006 (the move permutation) and ADR-BOARD-LANE-007 (the Pointer Events lane drag), plus `brief.md` §lanes and `registry.yaml` OUT-9 and OUT-10, with OUT-6 amended.

## The five things worth remembering

### 1. Measurement proved the central assumption and went further than it.

DISCUSS predicted that insert's two-statement shuffle cannot be reused for a move.
A move leaves no vacant slot, and `DEFERRABLE INITIALLY IMMEDIATE` checks at the end
of each statement. The spike ran against PostgreSQL 16.14 and confirmed it with a
duplicate-key error. All three candidate shapes then worked. The measurement also
found two things nobody had predicted:

- **All three shapes fail against a non-deferrable constraint.** `DEFERRABLE` is a precondition for every reorder, not a convenience of one shape. That is why the `check-arch` guard went from *recommended* to a DoD item.
- **The unlocked race fails silently.** It raises no error and leaves every invariant intact, yet the board ends up in an order nobody asked for (5 of 5 runs). So the concurrency oracle asserts the resulting **order**, never merely "no error".

### 2. The costliest bug was an off-by-one that returns 200.

"Move right" must name the lane **two** to the right, because lifting the mover out
shifts everything after it left by one. If the request names the lane one to the right,
the store is asked to put the lane where it already is. The result is a `NoOp` with
HTTP 200, and the board does not change. The same class of bug reached the browser lane
during DELIVER as a drag release on a knife edge. Only an oracle that checks board
*order* caught it, and mutation testing then showed nothing at the unit level pinned
it (`index + 2 → index * 2` survived). Five unit tests now pin it.

### 3. One over-broad selector looked like four unrelated bugs.

The menu click handler matched `closest("[data-lane-move-url]")`, but that attribute
also sits on `section.column`. As a result, any click inside a column fired a move to
the end, including the synthetic click that follows a drag's `pointerup`. It showed up
as a broken menu, a failed threshold scenario, a `StaleElementReference` and two dead
drags. The handler is now scoped to the two `data-action` values.

### 4. `hx-headers='js:…'` does not deliver the CSRF token in this app.

This was measured with a fetch spy, not assumed. The htmx request was refused silently,
while a manual POST succeeded. The Move items now POST through the same `fetch` idiom
the card drag uses.

### 5. The review gate caught a vacuous assertion, a class the suite had already been warned about.

`no drop indicator remains on the board` counted elements and asserted zero, which is
true if the indicator is never created. The DISTILL review (Sentinel) flagged it along
with the untested `pointercancel` path (D10). Both were fixed. The same review also
caught `feature-delta.md` recording an *intention* as an *outcome* (reuse of `assert_lane_labels_in_order`), which was
corrected in place.

## Quality evidence

| Gate | Result |
|---|---|
| `blr` acceptance lane | **26/26**, 141 steps |
| `check-arch` + 5 gold tests | PASS (one failed first and was right: SQL `--` comments were not stripped) |
| fmt / clippy `-D warnings` / release build / `cargo deny` | PASS |
| `cargo test --workspace` | PASS (one container flake, `PortNotExposed`, passes in isolation) |
| Full `all` acceptance lane at delivery | 726/734. None of the 8 failures came from this feature: 6 were a `pg_dump` 14 vs 16 environment mismatch and 2 were `.lane-menu-trigger` contrast failures in the then-uncommitted `fix-lane-menu-clipped-mobile` work |
| Mutation: `check_lane_position_deferrable` | 4/4 (100%) |
| Mutation: `views::board_columns` | 50% at first, then **6/6 viable (100%)** after the added tests |
| Consolidated review | DISCUSS approved · DESIGN approved (2 ADR findings fixed) · DISTILL rejected, then revised (1 blocker and 1 high, both fixed). There was no DEVOPS wave, so no fourth reviewer |

**Unmeasured, not proven:** `Store::move_lane_before` and `services::lanes::move_lane`
were left out of mutation testing because they need Postgres for every mutant. The
ADR-006 spike and the order-asserting acceptance scenarios measure them instead.

## Process waiver — DES integrity gate

**Waived at finalize, by user decision on 2026-09-25.** DELIVER expects every roadmap
step to go to a crafter subagent that emits DES markers. This feature was built
directly because Agent dispatch was disabled by user instruction at the time. As a
result, there is **no `deliver/` directory, no `roadmap.json`, no `execution-log.json`
and no DES audit log**, and `des-verify-integrity` would report zero entries. The
pre-finalize "all steps DONE" check could not run against a log. It was replaced by the
DELIVER record in `feature-delta.md` (25 → 26 scenarios green, gates above) and by the
three commits on `main`.

Carried-forward item 4 in `feature-delta.md` ("4-wave review has not run") is stale.
The review ran at the end of DISTILL, as the same document's *Consolidated four-wave
review* section records.

## Artifacts

No migration was needed. The ADRs were written directly to their permanent home, and
the workspace has no `design/`, `discuss/` or `distill/` subdirectories.

- `docs/product/architecture/adr-board-lane-006-lane-move-permutation.md`
- `docs/product/architecture/adr-board-lane-007-pointer-events-lane-drag.md`
- `docs/product/architecture/brief.md` §lanes
- `docs/feature/board-lane-reorder/` (`feature-delta.md`, `intake.md`, `slices/`), preserved

## Still open

- **Cards still cannot be dragged on touch.** `board-dnd.js` remains HTML5 drag-and-drop (`dragstart`), which touch never fires. Moving it to Pointer Events is the first deferred successor named in ADR-007; `card-drag-drop-feedback` improved the feedback but did not change the mechanism.
- `assert_lane_labels_in_order` has two near-duplicate implementations that could be shared.
