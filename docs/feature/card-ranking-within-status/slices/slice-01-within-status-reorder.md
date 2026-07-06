# Slice 01 — Reorder cards within a status

**Goal**: drag a card up/down inside its own column, drop it at an exact slot → the new order persists and every
viewer sees it.
**Story**: US-01.

**IN scope**
- A **persisted per-status rank** (column or side table on `issues`) + migration **`0012`** that backfills rank
  per `(project, state)` from the current `number DESC` order (zero-shuffle) and defines the new-issue default
  slot (top of column).
- Board read change: `list_issues_by_project` (`lib.rs:1238`) orders each status by rank with a deterministic
  tiebreak (DESIGN ODD-1/read contract).
- A **position-carrying persist** for a same-status reorder (no state change) — endpoint shape per DESIGN ODD-2;
  wire format per ODD-3 (neighbour keys vs index). Tenancy/CSRF inherited; foreign issue → non-enumerable refusal.
- Extend `board-dnd.js` (`drop`, line 63-99): compute the **insertion index** from the drop location instead of
  `appendChild`-to-end; POST the position; keep optimistic move + revert-on-failure + the `x-csrf-token` header.
- Acceptance: the position-persist contract + the ordered read + the backfill migration (store/endpoint level);
  the drag gesture is browser-dogfooded.

**OUT of scope**: cross-status positional drop (slice 02); keyboard/a11y reorder; touch polish; multi-select;
priority auto-sort; realtime is DESIGN's call (ODD-4) — if broadcast, it rides the shipped outbox.

**Learning hypothesis**: disproves "the chosen rank model + a position-carrying persist + an ordered read + a
zero-shuffle backfill migration is a clean, race-safe increment" if precision-exhaustion, renumber cost,
concurrent reorders, or the migration backfill needs machinery we lack.

**Seams**: `issues` schema (`0001_init.sql:64` → new `0012`); `list_issues_by_project` (`lib.rs:1238`); the state
persist path (`change_issue_state` / `/state`) for the endpoint pattern; `board-dnd.js`; `issue_card.html`
`[data-issue-key]`/`[data-state-url]`.
**Dependencies**: DESIGN ODD-1/2/3/5 (rank model, persist path, wire format, migration backfill).
**Effort**: ~1–1.5 days (migration + rank model + read + JS insertion index — carries the feature's uncertainty).
