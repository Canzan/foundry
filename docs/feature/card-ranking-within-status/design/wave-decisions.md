# DESIGN Decisions — card-ranking-within-status

## Key Decisions
- [D1] **Contiguous `position INTEGER` per `(project, state)`**, reindexed in one transaction per move; read
  `ORDER BY position ASC, number DESC`. Simplest correct model at kanban scale; no precision/renumber tail-risk.
  (ADR-001)
- [D2] **Migration `0012`** adds `position` + a **zero-shuffle** backfill via
  `row_number() OVER (PARTITION BY project_id, state ORDER BY number DESC)`; new issues land at **top of Backlog**.
  (ADR-001)
- [D3] **Extend `POST /issues/{n}/state`** with an optional `after` neighbour key; `board-dnd.js` always sends
  `state` + `after`. One endpoint, **one request shape for both slices** — slice 02 becomes nearly free. (ADR-002)
- [D4] **Wire format = neighbour issue key** (`after=<key>`; absent ⇒ top), reusing the card's existing
  `data-issue-key` — no new card attribute; race-robust. Unknown `after` key ⇒ non-enumerable refusal. (ADR-002)
- [D5] **v1 realtime = state-only broadcast**; rank is NOT pushed over SSE (mirrors `update_issue_details`,
  `lib.rs:1323`). A pure reorder emits **no** outbox row → the store needs a **conditional-emit** sibling to
  `update_issue_state_with_outbox`. Other viewers converge on next load. (ADR-002, upstream UC-1)
- [D6] **Keyboard/a11y reorder deferred** to a follow-up (v1 drag is mouse/touch only).
- [D7] Repo legacy multi-file convention; no `docs/product/` SSOT; DES exempt. Matches all prior features.

## Architecture Summary
- Pattern: modular monolith, ports-and-adapters (unchanged).
- Paradigm: imperative Rust (unchanged; no CLAUDE.md paradigm write).
- Key components touched: `board-dnd.js` (drop), `list_issues_by_project` (read), `change_issue_state` +
  `submit_state_change`/`ChangeStateForm` (persist), a reposition store method + `insert` (create default slot),
  migration `0012`.

## Reuse Analysis
| Existing Component | File | Overlap | Decision | Justification |
|--------------------|------|---------|----------|---------------|
| Drop handler | `static/js/board-dnd.js:63` | drop + POST state | EXTEND | add after-key + `after=`; ~20 LOC |
| Board read | `foundry-store/src/lib.rs:1238` | order board issues | EXTEND | `ORDER BY position ASC, number DESC` + backfill; no new read |
| State write | `lib.rs:1273` | state + outbox in a tx | EXTEND (sibling) | add reindex + **conditional** emit; current method can't express position + emits unconditionally |
| State use-case | `foundry-services/src/issues.rs:100` | authz + delegate | EXTEND | thread optional `after`; reuse resolve_member_project |
| Handler + form | `foundry-app/src/issues.rs:139` | `/state` endpoint | EXTEND | add `after: Option<String>` (serde default) |
| Create path | `insert_issue_with_outbox` | new-issue insert | EXTEND | insert at top of Backlog (shift +1) in-tx |
| Card markup | `foundry-app/src/issues.rs:532` | card attrs | REUSE | `after` reuses `data-issue-key` |
| Board bucketing | `foundry-app/src/projects.rs:587` | filter → columns | REUSE | filter preserves query order |
| Migration | `crates/foundry-store/migrations/` | — | CREATE NEW | `0012` (first since `0011`) — unavoidable |

## Technology Stack
- No new deps. Postgres `INTEGER` + `row_number()` backfill; sqlx transaction (existing idiom); vanilla JS
  (extend the CSP-safe `board-dnd.js`).

## Constraints Established
- Contiguity invariant per `(project, state)`; single-tx reindex touching source + target columns.
- Read order `position ASC, number DESC`; index → `(project_id, state, position)`.
- One write path (`/state` + `after`); tenancy/CSRF inherited; unknown/foreign ⇒ non-enumerable refusal.
- v1 rank not broadcast; conditional outbox emit (state-change only).
- Migration `0012` zero-shuffle; new issue → top of Backlog.

## Upstream Changes
- UC-1 (`upstream-changes.md`): "other viewers see the same order" is verified on-reload (persisted re-read), not
  via a live SSE-position push, in v1. No scope change.

## Handoff to DISTILL
Acceptance should cover: the position-persist contract on `/state` (within + cross-status, incl. the GEN-3 →
between GEN-4/GEN-2 anchor); the ordered read (`position ASC, number DESC`); the `0012` zero-shuffle backfill;
new-issue-at-top; revert-on-failure; non-enumerable refusal for foreign issue AND unknown `after`; progressive
enhancement (read honors rank, no-JS = no reorder). Verify cross-viewer convergence as a persisted re-read (UC-1),
NOT a live two-client SSE scenario. The drag gesture itself is browser-dogfooded (native HTML5 DnD isn't
CDP-synthesizable — exercise via genuine `dragstart`→`drop` DragEvents, per `issue-status-move`).
