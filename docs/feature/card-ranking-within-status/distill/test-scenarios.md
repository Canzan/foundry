# DISTILL Test Scenarios — card-ranking-within-status

> SSOT: `crates/foundry-acceptance/tests/features/card-ranking-within-status.feature`. All `@pending`; DELIVER
> un-@pends per slice. The persist path (`change_issue_state` / `POST /state`) is shipped; NET-NEW coverage = the
> `position` write via the new `after` param, the ordered board read, the zero-shuffle default, the new-issue
> slot, and non-enumerability of the new write. The live drag gesture + optimistic move/revert are browser-dogfood.

## Config
- framework: cucumber-rs; glue in DELIVER at `steps/feature_card_ranking_within_status.rs`. Reuse the
  issue-status-move harness wholesale: the Background workspace/member/team seed, `(\w+) is signed in`,
  `Mei fetches the "…" board`, `capture_drop_post` (the `x-csrf-token` header form), `read_issue_state`, and
  `board loads the drag-and-drop script`. Real Postgres (testcontainers) + reqwest + scraper. `@real-io`.
- HARNESS BOUNDARY: HTTP-level. Automated: the `/state` + `after` persist contract (store `position` + `state`),
  the ordered read (`position ASC, number DESC`), zero-shuffle default order, new-issue-at-top, non-enumerable
  refusal (unknown neighbour + foreign issue), progressive enhancement (order in server HTML). NOT automatable:
  the drag gesture + optimistic client move/revert — browser-dogfood.
- NEW seed helper: the multi-issue data-table Given `a project "…" (key "…") with issues:` (extends the
  single-issue Given). NEW store read: `position` for `(project, state)` ordering assertions.

## Catalog
| # | Scenario | Slice/AC | Drives |
|---|----------|----------|--------|
| S1 | Column with no manual order = newest-first | 01 / AC-01.3, AC-01.6 | GET board; assert `todo` DOM order = number DESC (zero-shuffle) |
| S2 | Reorder within a column persists | 01 / AC-01.1/.2/.3 | POST /state `state=todo&after=GEN-2` (same state); store position; board order flips to `GEN-2, GEN-4` |
| S3 | Reorder to the top (no neighbour) | 01 / AC-01.1 | POST /state `state=todo` no `after`; position 0; board order |
| S4 | Unknown neighbour key refused | 01 / AC-01.5, ADR-002 | POST /state `after=GEN-404` → non-enumerable refusal; order unchanged |
| S5 | Foreign-issue reorder refused | 01 / AC-01.5 | POST /state for an issue in another workspace → non-enumerable refusal |
| S6 | New issue lands at top of Backlog | 01 / AC-01.6 | file via the real create path; newest is first in `backlog` |
| S7 | Ranked order in server HTML (no-JS) | 01 / AC-01.7 | reorder, then GET board; order present server-side; DnD script linked |
| S8 | Cross-status drop sets state AND rank (GEN-3 anchor) | 02 / AC-02.1/.2/.3 | POST /state `state=todo&after=GEN-4` for a Backlog card; store state+position; order `GEN-4, GEN-3, GEN-2` |
| S9 | Cross-status drop to the top of target | 02 / AC-02.1 | POST /state `state=todo` no `after`; state+position 0 |
| S10 | Rejected cross-status drop is inert | 02 / AC-02.4/.5 | POST /state invalid → 400; state + rank both unchanged |

## The headline (S8) — the user's example
GEN-3 in Backlog, Todo shows GEN-4 above GEN-2 → drop GEN-3 between GEN-4 and GEN-2 → GEN-3 is `todo` and ranked
between them (`GEN-4, GEN-3, GEN-2`). One gesture, one `POST /state state=todo&after=GEN-4`, state + position
atomic.

## Browser-dogfood checklist (not automated)
1. Drag a card up/down within Todo → it lands at the exact slot and persists (reload confirms).
2. Drag GEN-3 from Backlog and drop it between Todo's GEN-4 and GEN-2 → GEN-3 lands between them, `todo` persists.
3. Simulate a failed drop (offline) → the card snaps back to its origin slot (and origin column for a cross-drop).

## Reconciliation (DESIGN ADR-001/002, ODD-1..6)
- ADR-001 contiguous `position` → S2/S3/S8/S9 assert order via the store position + board render; S1 asserts the
  zero-shuffle default (`position ASC, number DESC` tiebreak); S6 asserts the new-issue top slot.
- ADR-002 `/state` + `after` (one request shape, both slices) → S2/S3 (same state) and S8/S9 (state change) issue
  the identical request via `capture_drop_post`; S4 pins the unknown-`after` → refuse decision.
- ADR-002 v1 realtime = state-only (upstream UC-1) → NO live two-client SSE-position scenario; S7 verifies
  "other viewers see the order" as a persisted re-read (fresh board GET), per the tightened AC wording.
- Non-enumerability (ADR-003 lineage) → S4 (unknown neighbour) + S5 (foreign issue) both refuse uniformly, no 500.
