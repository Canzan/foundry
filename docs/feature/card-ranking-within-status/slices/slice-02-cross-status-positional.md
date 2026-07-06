# Slice 02 — Cross-status positional drop

**Goal**: drag a card into a different column and drop it at an exact slot → one gesture sets both its status and
its rank.
**Story**: US-02.

**IN scope**
- Extend the `board-dnd.js` drop handler so a drop into a **different** column carries the **insertion index**
  (from slice 01) alongside the target state — instead of today's append-to-end.
- A persist that writes **state + rank atomically** in one request: reuse the shipped `change_issue_state` for the
  state part + slice-01's position write, combined per DESIGN ODD-2 (one endpoint vs a `{state, position}`
  payload). Revert on failure returns the card to its **origin column AND origin slot**.
- Acceptance anchor: GEN-3 (Backlog) dropped between Todo's GEN-4 and GEN-2 → GEN-3 is `todo` and ranked between
  them; persisted; other viewers converge.
- Tenancy/CSRF preserved; foreign issue → non-enumerable refusal.

**OUT of scope**: within-status reorder (slice 01, prerequisite); keyboard/a11y reorder; touch polish;
multi-select; priority auto-sort.

**Learning hypothesis**: disproves "a cross-status drop can set state AND rank atomically over the shipped state
persist + slice-01 machinery" if combining the state write and the position write into one gesture needs a
transaction or endpoint shape we lack.

**Seams**: `board-dnd.js` (slice-01 insertion index, now cross-column); `change_issue_state` / `/state` (state
part); slice-01 position persist + ordered read; `board.html` `[data-column]`.
**Dependencies**: slice 01 (rank machinery, insertion index) + DESIGN ODD-2/ODD-4 (combined persist, realtime).
**Effort**: ~0.5–1 day (thin extension — folds proven position into the shipped cross-column move).
