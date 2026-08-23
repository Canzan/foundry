# Slice 04 — Delete a lane that holds cards: move them or delete them

Story: US-BLM-04 | Estimate: 1 day | job_id: `job-board-lane-shaping`

## Goal

Deleting a lane with N ≥ 1 cards opens the fate dialog (D7): lane name + live
card count, exactly two actions — "Move all N to [surviving-lane picker,
leftmost preselected]" or "Delete all N permanently" — plus cancel. One atomic
operation: lane removal + card fate together, never an observable laneless
intermediate state.

## IN

- Fate dialog (full-lane arm of D7) in `#modal-root`, declarative close only
  (BR-4). Copy states the count and, on the delete arm, permanence.
- Destination picker: surviving lanes only (dying lane excluded), leftmost
  preselected.
- Move arm: all cards in the lane **at confirm time** append to the bottom of
  the destination lane preserving relative order (0012 positions); one 0013
  `status` change event per card, same transaction, actor = the operator.
- Delete arm: each card removed via the hard-cascade shape
  (`delete_issue_cascade` precedent — comments, attachments, history go with
  it); cards vanish from board and search.
- Atomicity/race: fate applies to confirm-time membership — a card filed
  after the dialog rendered is moved/deleted with the rest, never stranded
  (shared-artifact "card count").
- Cancel writes nothing: no lane change, no issue change, no 0013 events.
- Same authz/CSRF/error contracts as slice 03 (D10).

## OUT

- Single-issue delete affordance (triggered suggestion, separate feature).
- Undo/restore of deleted lanes or cards (feature OUT).

## Learning Hypothesis

The two fates cover the job completely — the dialog never needs a third
option (e.g. "distribute", "archive"). If the operator hesitates at the
dialog in demo, the missing option is add-lane regret (D9's successor
feature), not a third fate.

## Acceptance Criteria

- [ ] Move: Todo (AUTH-12/15/18) → Backlog: column gone; cards at Backlog's
      bottom in relative order; three Todo → Backlog entries in the change
      report attributed to Priya.
- [ ] Delete: Done on "Scratch" (SCR-2, SCR-5): lane and cards gone from
      board and search after the counted, permanence-stating choice.
- [ ] Picker lists exactly the surviving lanes, leftmost preselected.
- [ ] Esc cancel: lane, cards, positions, change history byte-identical.
- [ ] Race: AUTH-21 filed into Todo mid-dialog is moved with the rest — zero
      laneless issues after every scenario, provable by query.

## Dependencies

Slice 03 (delete affordance, dialog frame, refuse-if-last, authz plumbing).
