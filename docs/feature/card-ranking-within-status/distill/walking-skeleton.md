# Walking Skeleton — card-ranking-within-status

The persist path ships (`change_issue_state` / `POST /state`). The NET-NEW machinery is a contiguous `position`
per `(project, state)` (migration `0012`), a position-carrying persist (the `after` param), and an ordered read.
Slice 01 ships ALL of it in one column; slice 02 reuses it to make a cross-status drop set state + rank atomically.

## First failing test (DELIVER entry) — slice 01
**S2 — "Reordering within a column persists the new order"**.
RED→GREEN:
1. Migration `0012`: `ALTER TABLE issues ADD COLUMN position INTEGER NOT NULL DEFAULT 0`; backfill
   `position = row_number() OVER (PARTITION BY project_id, state ORDER BY number DESC) - 1`; index
   `(project_id, state, position)`.
2. Read: `list_issues_by_project` → `ORDER BY position ASC, number DESC` (the per-state filter in
   `build_board_page` preserves it — no struct/card change).
3. Persist: `ChangeStateForm` gains `after: Option<String>`; `change_issue_state` threads it; a new store method
   `reposition_issue_with_outbox` reindexes the source + target columns in one tx and emits the outbox row
   **iff `state` changed** (ADR-002 D5 conditional emit).
4. Wire the DELIVER glue: reuse `capture_drop_post`, adding an optional `&after=<key>` to the body.

S1 (zero-shuffle default), S3 (top slot), S4 (unknown-`after` refuse), S5 (foreign refuse), S6 (new-issue top),
S7 (order in server HTML) then green off the same machinery.

## Slice 02
**S8 — the GEN-3 anchor** (cross-status drop sets state AND rank). GREEN: extend the `board-dnd.js` drop handler
so a drop into a DIFFERENT column carries the insertion `after` key (it already carries the target state); the
already-built `reposition_issue_with_outbox` writes state + position atomically. S9 (cross-drop to top) + S10
(rejected drop inert) green off the same path. Then DOGFOOD the cross-column gesture on the sandbox board.

Because both slices issue the SAME request shape (`state` + `after`), slice 02 is almost entirely a client-JS +
dogfood delta — the server contract is already proven by slice 01.

## Lane safety
All `@pending` (excluded by `filter_run`); `@all` stays green until DELIVER un-@pends per slice. Full `@all`
(incl. issue-status-move 6/6 regression) at finalize.
