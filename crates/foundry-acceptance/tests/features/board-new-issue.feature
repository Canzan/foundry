# Feature: board-new-issue — wire the inert "New issue" board button.
#
# Source SSOT for docs/feature/board-new-issue/distill/test-scenarios.md.
# The create backend is shipped + tested (us-08-file-issue, us-12-keyboard-nav):
# GET …/issues/new returns the modal, POST …/issues returns an OOB card that
# appends to the Backlog column (issues.rs:293). This feature adds ONLY the
# client wiring: the button opens the modal, the modal form submits via htmx.
#
# HARNESS NOTE: the acceptance suite is HTTP-level (reqwest + scraper), not a JS
# browser — so it asserts (a) the wiring attributes are present, (b) the shipped
# endpoint contracts the wiring depends on, and (c) the no-JS plain-form fallback
# end-to-end. The live click→modal→card→close interaction is verified by browser
# dogfood (as us-12 did for its "press c" flow), recorded in walking-skeleton.md.
#
# EVERY scenario is @pending; acceptance.rs filter_run excludes @pending from
# every lane, so @all stays green until DELIVER wires the templates + un-@pends.

@board-new-issue @us-new-issue @driving_port
Feature: The board's "New issue" button files an issue
  On a project board, the "New issue" button opens the shipped new-issue modal
  and filing a title drops the new card into Backlog — reusing the shipped modal
  endpoint, create POST, and out-of-band card, with the no-JS fallback intact.

  Background:
    Given a workspace "Acme" exists with a member "Mei" on team "Backend"
    And a project "Sandbox" with key prefix "GEN" exists under "Backend"
    And Mei is signed in

  @real-io
  Scenario: The New issue button is wired to open the modal
    When Mei fetches the "Sandbox" board
    Then the "New issue" button carries an hx-get to the new-issue modal endpoint
    And the button targets a modal container
    And the board contains a modal container element

  @real-io
  Scenario: The new-issue modal form is wired to submit via htmx
    When Mei fetches the new-issue modal for "Sandbox"
    Then the modal form carries an hx-post to the issues collection
    And the modal form still carries method="post" and the hidden "_csrf" field

  @real-io
  Scenario: An htmx create returns an out-of-band card for the Backlog column
    When Mei posts a new issue titled "Wire the button" to "Sandbox" as an htmx request
    Then the response is an out-of-band fragment targeting the "backlog" column
    And it renders a card showing the key "GEN-1" and the title "Wire the button"

  @real-io @error
  Scenario: An empty title returns the error fragment, not a card
    When Mei posts a new issue with an empty title to "Sandbox" as an htmx request
    Then the response is the "Title is required" error fragment
    And the response is not a board and contains no issue card

  @real-io
  Scenario: No-JS fallback — a plain form post still files the issue
    When Mei posts a new issue titled "Fallback works" to "Sandbox" as a plain form
    Then the response redirects to the "Sandbox" board
    And fetching the board shows "Fallback works" in the Backlog column
