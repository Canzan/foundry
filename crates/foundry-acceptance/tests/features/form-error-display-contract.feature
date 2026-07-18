# Feature: form-error-display-contract — make htmx form validation errors VISIBLE in the browser.
#
# Source SSOT for docs/feature/form-error-display-contract/distill/test-scenarios.md.
# RCA (escalated from /nw-bugfix): htmx 2.0.4 does not swap 4xx bodies and the app ships
# no override/extension/handler, so validation errors returned correctly as 400/422 +
# fragment are DISCARDED by the browser — the form silently does nothing. DESIGN ADR-001:
# a single htmx:beforeSwap handler (static/js/form-errors.js) routes any 4xx fragment into
# the triggering form's opt-in [data-error-slot]; the server stays byte-identical.
#
# HARNESS NOTE — THIS FEATURE IS BROWSER-LANE ONLY (DESIGN ADR-002). The fix is CLIENT-SIDE
# JavaScript; the HTTP acceptance lane (reqwest + scraper) never runs JS, so it CANNOT see
# the swap — a byte-identical 400/422 response is returned before and after; only the DOM
# differs. That HTTP-body blindness is the exact hole (RCA Root Cause B) that let the defect
# ship green. So every scenario here is @needs-browser: it drives a REAL browser (fantoccini
# + chromedriver, the shipped lane) and asserts the error is PRESENT AND VISIBLE in the
# rendered DOM. The shipped HTTP-lane oracles (which assert the 400/422 + fragment BODY) are
# KEPT unchanged in their own features — they still guard the server contract; these ADD the
# DOM assertion they never had.
#
# EVERY scenario is @pending; acceptance.rs filter_run excludes @pending from every lane, so
# @all (incl. @needs-browser) stays green until DELIVER wires form-errors.js + the slots and
# un-@pends per slice. @needs-browser is IN the `all` lane and EXCLUDED from the fast default
# lane (the split keyboard-shortcut-bindings established).

@form-error-display-contract @us-error-visible @driving_port
Feature: A member sees why a form was rejected
  When a member submits a form with a validation error, the reason is shown in the
  browser — inside the form, next to what they were doing — instead of the form silently
  doing nothing. The server response is unchanged (the correct 400/422 + fragment); the
  browser now displays it.

  Background:
    Given a workspace "Acme" exists with a member "Mei" on team "Backend"
    And a project "Sandbox" with key prefix "GEN" exists under "Backend"
    And Mei is signed in

  # ------------------------------------------------ Slice 01 — contract + oracle (issue create)

  @needs-browser @slice1 @us-01 @lane-probe @walking_skeleton @driving_port @real-io
  Scenario: The browser lane can observe a rejected submit end to end
    # The instrument proof (mirrors keyboard-shortcut-bindings' lane-probe): if the browser
    # lane cannot submit an invalid form and read the resulting DOM, no other scenario here
    # is worth writing. Establishes form-errors.js is loaded and the error slot exists.
    Given the browser lane has started chromedriver and navigated to the "Sandbox" board
    And the page reports the form-error handler is ready
    When Mei opens the new-issue dialog and submits it with an empty title
    Then the dialog stays open
    And the validation error "Title is required" is visible inside the dialog

  @needs-browser @slice1 @us-01 @error @contract
  Scenario: An invalid create shows the reason and creates nothing
    Given Mei is viewing the "Sandbox" board in a real browser
    When Mei opens the new-issue dialog and submits it with an empty title
    Then the validation error "Title is required" is visible inside the dialog
    And the dialog stays open with the title field still focusable
    And no card was added to the board

  @needs-browser @slice1 @us-01 @error @edge
  Scenario: Fixing the error and resubmitting succeeds without a page reload
    # Proves the slot-only swap preserves the form + its _csrf (DESIGN cross-cutting): the
    # retry re-submits with a valid token and the modal closes on success.
    Given Mei has submitted the new-issue dialog with an empty title and sees "Title is required"
    When Mei types a title "Rate limit the gateway" and submits the dialog again
    Then the dialog closes
    And a card "Rate limit the gateway" appears in the Backlog column
    And the browser is still on the "Sandbox" board without a reload

  @needs-browser @slice1 @us-01 @error @scoped
  Scenario: A successful create is unaffected by the error handler
    # Guards the blast radius: the beforeSwap handler must only fire on 4xx.
    Given Mei is viewing the "Sandbox" board in a real browser
    When Mei opens the new-issue dialog and submits a valid title "Works first time"
    Then the dialog closes
    And a card "Works first time" appears in the Backlog column
    And no validation error is shown anywhere on the page

  # ------------------------------------------------ Slice 02 — fan out to the other htmx forms

  @needs-browser @slice2 @us-02 @error @contract
  Scenario: An invalid issue edit shows the reason in the edit dialog
    Given the "Sandbox" project has an issue "GEN-1" titled "Keep me"
    And Mei is viewing the "Sandbox" board in a real browser
    When Mei opens the edit dialog for "GEN-1", clears the title, and saves
    Then the validation error "Title is required" is visible inside the edit dialog
    And the edit dialog stays open
    And the "GEN-1" card still shows "Keep me"

  @pending @needs-browser @slice2 @us-02 @error @contract
  Scenario: An invalid comment edit shows the reason inline
    Given the "Sandbox" project has an issue "GEN-1" with a comment by Mei
    And Mei is viewing the "GEN-1" issue page in a real browser
    When Mei edits that comment to an empty body and saves
    Then the validation error is visible next to the comment
    And the comment still shows its original text

  # ------------------------------------------------ Slice 03 — the edges (DELIVER may defer)

  @pending @needs-browser @slice3 @us-03 @edge @drag
  Scenario: A rejected drag reverts the card AND says why
    # Today (board-dnd.js) the card snaps back with no reason. The contract gives it an
    # OOB #toast message.
    Given the "Sandbox" project has an issue "GEN-1" that Mei is not allowed to move
    And Mei is viewing the "Sandbox" board in a real browser
    When Mei drags "GEN-1" to another column and the move is refused
    Then the "GEN-1" card returns to its original column
    And a message explaining the move was not saved is visible

  @pending @needs-browser @slice3 @us-03 @edge @comment-create
  Scenario: An invalid new comment shows the reason instead of a blank page
    Given Mei is viewing the "GEN-1" issue page in a real browser
    When Mei submits a comment that the server rejects
    Then the validation error is visible on the issue page
    And the issue page is still shown (not replaced by a bare error)
