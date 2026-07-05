# Feature: issue-edit-dialog — click an issue card to edit its title + description.
#
# Source SSOT for docs/feature/issue-edit-dialog/distill/test-scenarios.md.
# NET-NEW backend (DESIGN ADR-001/002): GET+POST …/issues/{n}/edit; save = OOB
# outerHTML card-replace keyed on data-issue-key + empty #modal-root to close;
# last-write-wins; no outbox in v1. Reuses the board-new-issue #modal-root +
# .modal styling; mirrors change_issue_state (service) + the comment inline-edit.
#
# HARNESS NOTE (as board-new-issue): the HTTP suite (reqwest + scraper) pins the
# WIRING (card hx-get; the pre-filled edit fragment; the save endpoint contract),
# the SAVE end-to-end at the store/endpoint level, tenancy/CSRF/validation, and
# the no-JS fallback. The live click→dialog→save→card-updates interaction is
# browser-dogfooded (recorded in walking-skeleton.md).
#
# EVERY scenario is @pending until DELIVER wires + un-@pends (kept out of @all).

@issue-edit-dialog @us-issue-edit @driving_port
Feature: A member edits an issue's title and description from a dialog
  Clicking an issue card opens a pre-filled edit dialog; saving persists the new
  title + description and updates the board card in place — reusing the board
  modal, with tenancy, CSRF, and the no-JS fallback intact.

  Background:
    Given a workspace "Acme" exists with a member "Mei" on team "Backend"
    And a project "Sandbox" (key "GEN") with an issue "GEN-1" titled "Old title" described "old body"
    And Mei is signed in

  @pending @real-io
  Scenario: The card is wired to open the edit dialog
    When Mei fetches the "Sandbox" board
    Then the "GEN-1" card carries an hx-get to its issue-edit endpoint targeting the modal container

  @pending @real-io
  Scenario: The edit dialog is pre-filled with the issue's current values
    When Mei opens the edit dialog for "GEN-1"
    Then the dialog title field contains "Old title"
    And the dialog description field contains "old body"
    And the dialog form carries an hx-post to the save endpoint and the hidden "_csrf" field

  @pending @real-io
  Scenario: Saving edits the issue and replaces the board card in place
    When Mei saves the edit dialog for "GEN-1" with title "New title" and description "new body"
    Then the issue "GEN-1" has title "New title" and description "new body" in the store
    And the response is an out-of-band card replacement keyed on "GEN-1" showing "New title"

  @pending @real-io @error
  Scenario: An empty title is rejected in the dialog, nothing persisted
    When Mei saves the edit dialog for "GEN-1" with an empty title
    Then the response is the "Title is required" error fragment
    And the issue "GEN-1" still has title "Old title" in the store

  @pending @real-io @security
  Scenario: Editing a foreign issue is refused non-enumerably
    Given an issue "GEN-9" exists in a DIFFERENT workspace from Mei
    When Mei requests the edit dialog for that issue's path
    Then the response is the uniform not-found page with no echoed title

  @pending @real-io
  Scenario: No-JS fallback saves the edit
    When Mei submits the edit form for "GEN-1" as a plain form with title "Plain edit"
    Then the response redirects to the "Sandbox" board
    And the issue "GEN-1" has title "Plain edit" in the store
