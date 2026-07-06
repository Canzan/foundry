# Feature: issue-status-move — move an issue between status columns.
#
# Source SSOT for docs/feature/issue-status-move/distill/test-scenarios.md.
# Two mechanisms, ONE shipped persist path (change_issue_state / POST /state):
# slice 01 = a status control in the edit dialog (server OOB card-relocation);
# slice 02 = drag-and-drop (native HTML5 DnD in a new app JS; client move).
#
# HARNESS NOTE: the HTTP suite pins the persist contract + wiring (draggable /
# drop-target markers, dialog status control) + tenancy + no-JS. The live DRAG
# GESTURE and the card-move animation are browser-dogfooded (JS the HTTP harness
# can't drive) — same split as board-new-issue / issue-edit-dialog.
#
# EVERY scenario @pending until DELIVER wires + un-@pends (kept out of @all).

@issue-status-move @us-status-move @driving_port
Feature: A member moves an issue between status columns
  Via the edit dialog's status control or by dragging the card, an issue moves to
  another column and its state persists — through the shipped change_issue_state
  path, with tenancy/CSRF intact and drag as a progressive enhancement.

  Background:
    Given a workspace "Acme" exists with a member "Mei" on team "Backend"
    And a project "Sandbox" (key "GEN") with an issue "GEN-1" in "Backlog"
    And Mei is signed in

  # ---- Slice 01: dialog status ----
  @us-01 @real-io
  Scenario: The edit dialog exposes a status control pre-set to the current state
    When Mei opens the edit dialog for "GEN-1"
    Then the dialog has a status control with "Backlog" selected

  @us-01 @real-io
  Scenario: Saving a new status relocates the card to that column
    When Mei saves the edit dialog for "GEN-1" with status "Todo"
    Then the issue "GEN-1" has state "todo" in the store
    And the response deletes the old "GEN-1" card and appends it to the "todo" column
    And the dialog is dismissed without a full navigation

  @us-01 @real-io
  Scenario: No-JS fallback saves the status change
    When Mei submits the edit form for "GEN-1" with status "Done" as a plain form
    Then "GEN-1" has state "done" in the store
    And the board shows "GEN-1" under the "done" column

  # ---- Slice 02: drag-and-drop (persist contract + wiring; gesture is dogfood) ----
  @pending @us-02 @real-io
  Scenario: Cards are draggable and columns are drop targets
    When Mei fetches the "Sandbox" board
    Then each issue card is marked draggable and carries its state-post URL
    And each state column is marked as a drop target for its slug
    And the board loads the drag-and-drop script

  @pending @us-02 @real-io
  Scenario: A drop persists the new state (the endpoint the drop handler posts to)
    When Mei posts a state change for "GEN-1" to "in_progress" as the drop handler would
    Then "GEN-1" has state "in_progress" in the store

  @pending @us-02 @real-io @error
  Scenario: A rejected drop does not change the issue's state
    When a drop posts an invalid state for "GEN-1"
    Then the response is a validation error
    And "GEN-1" keeps its previous state in the store
