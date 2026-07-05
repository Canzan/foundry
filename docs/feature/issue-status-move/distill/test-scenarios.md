# DISTILL Test Scenarios — issue-status-move

> SSOT: `crates/foundry-acceptance/tests/features/issue-status-move.feature`. All `@pending`; DELIVER un-@pends
> per slice. Persist path (`change_issue_state`) is shipped+tested; new coverage = the dialog-drives-state +
> DnD-wiring/persist contract. The live drag gesture is browser-dogfood.

## Config
- framework: cucumber-rs; glue in DELIVER at `steps/feature_issue_status_move.rs` (reuse board/sign-in/issue-
  seed + issue-edit-dialog helpers). Real Postgres (testcontainers) + reqwest + scraper. `@real-io`.
- HARNESS BOUNDARY: HTTP-level. Automated: dialog status control + OOB relocation contract + no-JS; DnD
  draggable/drop-target/script wiring + the drop-persist endpoint. NOT automatable: the drag gesture + the
  optimistic client move/revert — browser-dogfood.

## Catalog
| # | Scenario | Slice/AC | Drives |
|---|----------|----------|--------|
| S1 | Dialog status control pre-set | 01 / AC-01.1 | GET edit dialog; assert `<select>` with current state selected |
| S2 | Save new status relocates card | 01 / AC-01.2/.3 | POST edit w/ new status; store state; OOB delete-old + append-to-column |
| S3 | No-JS status save | 01 / AC-01.6 | plain POST w/ status; 303 board; card under new column |
| S4 | Cards draggable + columns drop targets + script | 02 / AC-02.1 | GET board; assert `draggable`, card state-URL data-*, `[data-column]` targets, `board-dnd.js` linked |
| S5 | Drop persists new state | 02 / AC-02.2 | POST /state (the drop's request) → store state |
| S6 | Rejected drop keeps state | 02 / AC-02.3 | POST /state invalid → validation error; state unchanged |

## Browser-dogfood checklist (not automated)
1. Open a card's dialog → change status → Save → the card moves to the new column, dialog closes, no reload.
2. Drag a card from Backlog to Todo → it lands in Todo and persists (reload confirms).
3. Simulate a failed drop (offline) → the card snaps back to its origin column.

## Reconciliation
DESIGN ADR-001/002 (ODD-1..4) reflected: S2 = server OOB relocation; S4 = native-DnD wiring + script; S5/S6 =
the shipped `/state` contract; realtime (outbox) is unchanged shipped behaviour, not re-tested here.
