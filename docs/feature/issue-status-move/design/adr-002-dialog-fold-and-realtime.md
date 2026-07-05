# ADR-002 — Dialog folds state via shipped paths; realtime is free (ODD-3, ODD-4)

**Status**: ACCEPTED (user-ratified 2026-07-05)

## Decision
- **ODD-3**: `submit_edit` applies title/description via the existing `edit_issue_details` AND, only when the
  submitted state differs from current, the state via the shipped `issue_service::change_issue_state`. Two
  reused calls, ONE persist path per mechanism — no new state-write. This preserves correct outbox semantics:
  state changes emit (realtime), title/description edits do not.
- **ODD-4**: keep `change_issue_state`'s shipped outbox emit. Status moves (dialog OR drag) therefore broadcast
  to other board viewers through the existing outbox→SSE path with NO new work.

## Alternatives rejected
- Fold state into `update_issue_details` (one store UPDATE): fewer round-trips but would either duplicate the
  outbox logic or drop the realtime emit — reusing `change_issue_state` is cleaner and correct.
- Suppress the outbox on move (local-only): the shipped realtime already handles state events; suppressing it
  would be extra work to REMOVE a working feature. Kept.

## Consequences
No new store/service method for slice 01 — pure reuse. Realtime edit-broadcast of STATUS is free (unlike
issue-edit-dialog's title/description, which stays local by design).
