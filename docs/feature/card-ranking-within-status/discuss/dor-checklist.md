# Definition of Ready — card-ranking-within-status

| # | DoR item | Status | Evidence |
|---|----------|--------|----------|
| 1 | User value clear (job traced) | ✓ | JTBD + 2 elevator pitches (`user-stories.md`); traces to the deferred "within-column reorder" (evolution 2026-07-05) |
| 2 | Stories INVEST-sized | ✓ | 2 slices, each end-to-end ≤~1 day (`story-map.md`) |
| 3 | ACs testable | ✓ | Given/When/Then (`acceptance-criteria.md`); persist-contract acceptance + gesture dogfood |
| 4 | Dependencies identified | ✓ | Reuses `board-dnd.js`, `change_issue_state`, the board read + card/column templates (`requirements.md` seams table) |
| 5 | No unresolved questions | ◑ | Rank model / persist path / wire format / realtime / migration backfill / a11y deferred to DESIGN (ODD-1..6) |
| 6 | Feasibility confirmed | ✓ | Drag+drop, drop targets, optimistic move+revert, CSRF header, SSE broadcast all shipped in `issue-status-move`; only the persisted rank model + ordered read + migration are new |
| 7 | Scope boundaries explicit | ✓ | Within + cross-status positional IN; cross-project rank, priority auto-sort, keyboard reorder, touch polish, multi-select, `cancelled` OUT (`requirements.md`) |
| 8 | NFR constraints stated | ✓ | Adds migration `0012` + rank column; read changes to rank order; tenancy/CSRF; progressive enhancement; concurrency/precision must be addressed; reuse one JS file, CSP-safe |
| 9 | Measurable outcome | ✓ | `outcome-kpis.md` |

**Verdict**: READY for DESIGN (required — the persisted **rank model** + position-carrying persist + migration
backfill are genuine architecture decisions, ODD-1..6). Note the key delta from `issue-status-move`: this feature
**does** add a migration and change the board read; the predecessor's "no migration / one persist path"
constraint does not carry over.
