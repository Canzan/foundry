# Acceptance Criteria (Given/When/Then) — issue-status-move

The HTTP suite (reqwest + scraper) pins the persist contract + wiring + tenancy + no-JS; the live drag gesture
and the card-move animation are browser-dogfooded (DnD is a JS interaction the HTTP harness can't drive — same
split as board-new-issue/issue-edit-dialog). DESIGN finalizes ODD-1..4.

```gherkin
Background:
  Given a workspace "Acme" with a member "Mei" on team "Backend"
  And a project "Sandbox" (key "GEN") with an issue "GEN-1" in "Backlog"
  And Mei is signed in

# ---- Slice 1: dialog status ----
Scenario: The edit dialog exposes a status control pre-set to the current state
  When Mei opens the edit dialog for "GEN-1"
  Then the dialog has a status control with "Backlog" selected

Scenario: Saving a new status moves the card to that column
  Given the edit dialog for "GEN-1" is open
  When Mei saves the dialog with status "Todo"
  Then the issue "GEN-1" has state "todo" in the store
  And the response relocates the "GEN-1" card into the "todo" column
  And the dialog is dismissed without a full navigation

Scenario: No-JS fallback saves the status change
  Given htmx is unavailable
  When Mei submits the edit form for "GEN-1" with status "Done"
  Then "GEN-1" has state "done" in the store
  And the board shows "GEN-1" under the "done" column

# ---- Slice 2: drag-and-drop (persist contract; gesture is dogfood) ----
Scenario: Cards are draggable and columns are drop targets
  When Mei fetches the board
  Then each issue card is marked draggable
  And each state column is marked as a drop target for its slug

Scenario: Dropping a card persists its new state (endpoint contract)
  When Mei posts a state change for "GEN-1" to "in_progress" as the drop handler would
  Then "GEN-1" has state "in_progress" in the store
  And the response is the shipped state acknowledgement

Scenario: A rejected drop does not move the issue
  When a drop posts an invalid state for "GEN-1"
  Then the response is a validation error
  And "GEN-1" keeps its previous state in the store
```

## Store/service (reuse)
`change_issue_state` + `update_issue_state_with_outbox` are shipped + tested (us-08/board scenarios). New
coverage focuses on the DIALOG-drives-state path (slice 1) and the DROP-persist wiring (slice 2), not
re-testing the state write itself.
