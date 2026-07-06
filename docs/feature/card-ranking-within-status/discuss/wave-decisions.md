# DISCUSS Decisions — card-ranking-within-status

## Key Decisions
- [D1] **Persisted per-status rank is the core.** Cards get a shared, persisted ordering per `(project, state)`;
  the board read changes from `ORDER BY number DESC` to rank order with a deterministic tiebreak. This is the
  first migration since `0011` → `0012`.
- [D2] **Two slices, within-status first.** Slice 01 (within-status reorder) ships ALL the rank machinery
  (migration + rank + position persist + ordered read + `board-dnd.js` insertion index); slice 02 (cross-status
  positional drop) folds the position into the shipped cross-column move so one gesture writes state + rank.
- [D3] **Scope = BOTH within and cross-status positional** (user-confirmed 2026-07-06). Slice 02 supersedes
  today's append-to-end drop with a precise-slot drop that sets state + rank atomically. The GEN-3 → between
  Todo's GEN-4 and GEN-2 example is the slice-02 acceptance anchor.
- [D4] **Migration must be zero-shuffle.** `0012` backfills rank from the current `number DESC` order per
  `(project, state)` so every existing board looks identical on first render; new-issue default slot is defined
  (top of its column, preserving "newest first").
- [D5] **Reuse, don't re-add.** Extend the shipped `board-dnd.js` (no second JS file), reuse `change_issue_state`
  for the state part of slice 02, inherit tenancy/CSRF and the SSE outbox from `issue-status-move`.
- [D6] **Rank is set by JS drag only; rendered for everyone.** No no-JS reorder control (none required for v1),
  but the read path honors rank so no-JS viewers see the same order. Progressive enhancement, as shipped.
- [D7] **Real DESIGN required.** The rank representation, persist-path shape, position wire format, realtime
  convergence, migration backfill, and a11y stance are genuine architecture decisions (ODD-1..6).
- [D8] **Repo legacy multi-file convention; no SSOT (`docs/product/` absent); DES exempt.** Matches all prior
  features on trunk.

## Requirements Summary
- Primary need: order cards within a status (and place them precisely when moving across statuses) so the column
  communicates "what's next", not creation order. Delivers the item `issue-status-move` explicitly deferred.
- Walking skeleton: not applicable (brownfield increment). Slice 01 is the rank walking-skeleton — the whole
  persist→read→gesture loop for one column.
- Feature type: user-facing (UI; extends the app's client JS), brownfield.

## Constraints Established
- Adds migration `0012` + a persisted rank; board read becomes rank-ordered (deterministic tiebreak).
- Migration backfill is zero-shuffle; new-issue default slot defined.
- Tenancy/CSRF preserved; foreign issue → non-enumerable refusal (no 500).
- Progressive enhancement: rank set via drag only, order rendered for all.
- Concurrency + rank-precision behaviour must be specified by the chosen rank model (renumber/rebalance story).
- One client JS file (extend `board-dnd.js`), CSP-safe.

## Scope Assessment: PASS
Right-sized as 2 slices. Slice 01 carries the only real novelty (persisted rank model + migration + ordered
read); slice 02 is a thin extension folding position into the shipped move. DESIGN de-risks the rank model.

## Handoff to DESIGN
Resolve ODD-1 (rank representation: integer+renumber vs fractional vs lexorank vs reindex-per-column),
ODD-2 (persist path: extend `/state` with a position vs dedicated `/rank`/`/move`), ODD-3 (position wire format:
neighbour issue keys vs client index), ODD-4 (realtime: broadcast reorder via outbox/SSE + convergence contract
vs local-only), ODD-5 (migration `0012` backfill from `number DESC` + new-issue default slot), ODD-6 (keyboard/
a11y reorder in v1 or deferred). Plus: concurrency safety of the position write and the deterministic read
tiebreak.

## Upstream Changes
None to a prior wave's assumptions — this is the planned follow-on to `issue-status-move` (which named
"within-column reorder" as its first deferred item). Brownfield; grounded in the shipped board read
(`lib.rs:1246`), drop handler (`board-dnd.js:79`), state persist, and the `issues` schema (`0001_init.sql:64`).
