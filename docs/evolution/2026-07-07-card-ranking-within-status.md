# Evolution — card-ranking-within-status (rank issue cards within a status, and place them precisely across statuses)

**Finalized**: 2026-07-07
**Commits**: DISCUSS + DESIGN + DISTILL `501d9c9` → DELIVER `b8e3ae6` (slice 01, within-status) + `e248506`
(slice 02, cross-status positional — un-`@pends` `@us-02` + JS cache-revalidation fix). Trunk-based; repo legacy
multi-file convention; DES exempt. Feature dir PRESERVED.
**Wave coverage**: full pipeline WITH a real DESIGN — DISCUSS (2 stories → 2 slices) → DESIGN (ADR-001 contiguous
`position` model + ADR-002 persist-path/realtime; D1–D7 user-ratified 2026-07-06) → DISTILL (10-scenario SSOT) →
DELIVER (2 slices). NOTE: delivery landed ad-hoc (no `deliver/roadmap.json` / `execution-log.json` were written);
completeness is evidenced by git history + the green `@all` acceptance lane, not an execution log.
**Scope**: within a status column the order was not user-controllable — `list_issues_by_project` read
`ORDER BY number DESC` and the drag handler always appended to the end of the target column. There was no
persisted per-status ordering (`issues` had no rank column). This adds a **manual rank** — a member drags a card
to an exact slot, within its column or into another, and that order persists for every viewer — through the
shipped `change_issue_state` path (now carrying an optional `after` neighbour key). Migration `0012`.

## Milestone — the board column is now a priority signal

`issue-status-move` (2026-07-05) explicitly deferred within-column reorder as its first out-of-scope item; this
feature delivers it. The board column top-to-bottom order is now the fine-grained "what I'll pick up next" signal
that the coarse `priority` enum can't express — and a cross-status drop sets **state AND rank in one gesture**.

## What shipped

### Slice 01 — reorder within a status (`b8e3ae6`)
- **Migration `0012_issue_position.sql`** (first since `0011`): adds `position INTEGER` to `issues`, backfills
  per `(project_id, state)` via `row_number() OVER (PARTITION BY project_id, state ORDER BY number DESC)` — a
  **zero-shuffle** backfill (first render matches the old newest-first order) — and indexes `(project_id, state,
  position)`. (ADR-001)
- **Contiguous rank model**: one `position` per `(project, state)`, reindexed in a single transaction per move;
  board read is `ORDER BY position ASC, number DESC`. No precision/renumber tail-risk at kanban scale. (ADR-001)
- **Board read**: `Store::list_issues_by_project` orders each status by `position ASC, number DESC`.
- **Position-carrying persist**: `POST /issues/{n}/state` gains an optional `after: Option<String>` neighbour key
  (serde default). A within-status reorder is "same `state` + new `after`" → a **position-only write with no
  outbox emit** (a reposition-store sibling to `update_issue_state_with_outbox`, conditional emit). Wire format is
  the neighbour issue key (reusing the card's existing `data-issue-key`; absent ⇒ top). Unknown/foreign `after`
  key → non-enumerable refusal. Tenancy/CSRF inherited. (ADR-002, D3/D4/D5)
- **New issue → top of Backlog**: `insert` shifts positions +1 in-tx so a newly filed issue lands at position 0.
- **`board-dnd.js`**: the drop handler computes the **insertion index** from the drop location instead of
  `appendChild`-to-end, and POSTs `state` + `after`; keeps optimistic move + revert-on-failure + `x-csrf-token`.
- Threaded through the service (`issue_service`, optional `after`) and the app handler/form.
- Store-level coverage: `reposition_issue.rs` (224 lines).

### Slice 02 — cross-status positional drop, atomic (`e248506`)
- A drop into a **different** column carries the slice-01 insertion index alongside the target state: one request
  writes **state + rank atomically** — `state` = the target column, `after` = the new neighbour. As DESIGN D3
  predicted ("one request shape for both slices — slice 02 becomes nearly free"), this reused the shipped
  `change_issue_state` path + slice-01's position write with **no new persist machinery**; the commit added the
  `@us-02` step definitions (+44 lines) and un-`@pended` the three cross-status scenarios into `@all`.
- Anchor flow verified: GEN-3 (Backlog) dropped between Todo's GEN-4 and GEN-2 → GEN-3 becomes `todo`, ranked
  between them, persisted; a rejected cross-status drop changes neither state nor rank (revert to origin column
  AND origin slot).
- **JS cache-revalidation fix** (same commit): app-owned static JS is now served with a revalidating cache header
  so edits to `board-dnd.js` actually reach browsers (browsers had been running a stale cached script — the
  "card-ranking bug"). A dedicated scenario pins it.

Both slices reuse the SHIPPED `POST /issues/{n}/state` → `change_issue_state`. ONE persist path, one request shape.

## Decisions realized (D1–D7, user-ratified 2026-07-06)
| # | Decision | Status |
|---|---|---|
| D1 | Contiguous `position INTEGER` per `(project, state)`, single-tx reindex, read `position ASC, number DESC` | IMPLEMENTED |
| D2 | Migration `0012` — zero-shuffle backfill; new issue → top of Backlog | IMPLEMENTED |
| D3 | Extend `POST /state` with optional `after`; one request shape for both slices | IMPLEMENTED |
| D4 | Wire format = neighbour issue key (reuse `data-issue-key`); unknown `after` ⇒ non-enumerable refusal | IMPLEMENTED |
| D5 | v1 realtime = state-only broadcast; pure reorder emits no outbox row (conditional emit) | IMPLEMENTED |
| D6 | Keyboard/a11y reorder deferred to a follow-up | DEFERRED (as designed) |
| D7 | Repo legacy multi-file convention; DES exempt | IMPLEMENTED |

## Verification
- **DELIVER**: card-ranking `@all` — 10 scenarios (7 `@us-01` within-status + 3 `@us-02` cross-status), all live,
  0 `@pending`. Shipped and pushed to `origin/main` with full CI green (recorded 418/418 at push).
- **Finalize**: `main` == `origin/main`, tree clean; the feature's scenarios are wired into the `@all` lane.
- **Browser dogfood**: native HTML5 DnD gesture isn't CDP-synthesizable, so the drop handler is exercised via
  genuine `dragstart`→`drop` DragEvents (same events a mouse drag fires) — the same split as `issue-status-move`.
  The HTTP suite pins the persist contract (POST `/state` with `after` → store position + state), the ordered
  board read, the zero-shuffle default, the new-issue slot, tenancy/non-enumerability, and progressive enhancement.

## Cross-viewer convergence (UC-1)
v1 realtime is state-only. A cross-status move broadcasts its state live (other viewers see the card in the new
column via the shipped outbox → SSE); a pure within-status reorder is local to the actor and other viewers
converge on next board load. Convergence is verified as a persisted re-read (a fresh board GET), NOT a live
two-client SSE-position scenario. No scope change from DESIGN.

## Deferred (out of scope)
Keyboard/a11y reorder (D6); touch-drag polish; multi-select; priority auto-sort; live two-client SSE-position
push (v1 is state-only broadcast + on-reload convergence).
