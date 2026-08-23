# Feature: issue-edit-modal-close-icon — a close (×) in the edit dialog's top right.
#
# Source SSOT: docs/feature/issue-edit-modal-close-icon/feature-delta.md (DISCUSS
# US-01 + AC-1.1..1.6 + UAT S1-S4; DESIGN D-10..D-16) and
# docs/product/architecture/adr-modal-close-001-declarative-close-trigger.md.
# STRICTLY ADDITIVE: one close mechanism (keyboard.js::closeModal() emptying
# #modal-root), two triggers — Esc (shipped, closeTopLayer(), BR-4 single owner)
# and the new declarative [data-action="close-modal"] click trigger. Esc, Save,
# and the "Open full page" link are regression surface only (S4).
#
# HARNESS NOTE — BROWSER LANE ONLY (@needs-browser, the shipped fantoccini +
# chromedriver lane). The behaviour is DOM+JS: a delegated click listener closing
# a dialog. The HTTP lane (reqwest + scraper) never runs JS — it could see the
# button render but never observe the close, the exact green-over-nothing hole the
# form-error-display RCA closed. Every scenario drives a real browser and, per
# D-15 (Earned Trust), waits on the shipped [data-kb-ready] marker before
# interacting: the wiring lands in keyboard.js, so kb-ready IS the attachment
# probe — present means the delegated listeners are live, never merely that the
# file parsed. The no-save oracle (S2) asserts BOTH ways: no save request
# observed while the dialog closed AND the stored fields unchanged (read back at
# the store, the shipped issue-edit-dialog oracle).
#
# FOCUS IS PINNED AS ESC-PARITY (DESIGN D-11 + open question 1): after a ×-close,
# assert the same observable today's Esc close produces — focus rests on a live
# element (the focused field went away with the host, activeElement falls to
# body) and the document-delegated shortcuts (j / k / c) still act. Do NOT assert
# focus returns to the originating card: restore is DEFERRED, and if ever added
# it lands inside closeModal() so BOTH triggers get it.
#
# DELIVERED: every scenario is live (no @pending left) — the button (D-12), the
# delegated listener (D-10), and the CSS + re-hash (D-13/D-14) shipped, and the
# S4 regression guards were un-@pended green one at a time. NO new walking
# skeleton: the edit-dialog e2e path was already shipped and green
# (issue-edit-dialog.feature + form-error-display slice 02); the × rides it.

@issue-edit-modal-close-icon @us-close-modal @driving_port
Feature: The operator closes the issue edit dialog without saving
  Opening an issue just to read it, or starting an edit and thinking better of it,
  ends with one click on a visible, conventional close control in the dialog's top
  right — no hunting for the unadvertised Esc key, no saving a change that was
  never meant. Esc keeps working exactly as before; the × is a second trigger for
  the same close, never a second mechanism.

  Background:
    Given a workspace "Acme" exists with a member "Mei" on team "Backend"
    And a project "Sandbox" with key prefix "GEN" exists under "Backend"
    And the "Sandbox" project has an issue "GEN-1" titled "Keep me"
    And Mei is signed in

  # ------------------------------------------------ S1 — the way out is visible and works

  @needs-browser @us-01 @real-io
  Scenario: The edit dialog offers a visible way out in its top right
    Given Mei is viewing the "Sandbox" board in a real browser
    When Mei opens the edit dialog for "GEN-1"
    Then the dialog shows a close control in the top right of its header
    And the close control is named "Close" for assistive technology
    And the close control's click target is at least 24 by 24 pixels

  @needs-browser @us-01 @real-io
  Scenario: One click on the close control returns Mei to the board
    Given Mei has opened the edit dialog for "GEN-1" in a real browser
    When Mei clicks the close control
    Then the dialog closes
    And the board is interactive again
    And the "GEN-1" card still shows "Keep me"

  # ------------------------------------------------ S2 — a dismissal saves nothing

  @needs-browser @us-01 @real-io @edge
  Scenario: A discarded edit saves nothing
    Given Mei has opened the edit dialog for "GEN-1" in a real browser
    And Mei has typed "Rename identity platform" into the title field without saving
    When Mei clicks the close control
    Then the dialog closes
    And no save request was sent while the dialog closed
    And the issue "GEN-1" still has title "Keep me" in the store
    And its description and status are unchanged in the store

  # ------------------------------------------------ S3 — no mouse required

  @needs-browser @us-01 @real-io @a11y
  Scenario: Mei reaches the close control with Tab and activates it with Enter
    Given Mei has opened the edit dialog for "GEN-1" in a real browser
    When Mei moves focus to the close control with the Tab key
    Then the close control shows a visible focus indicator
    When Mei presses "Enter"
    Then the dialog closes
    And the board is interactive again

  @needs-browser @us-01 @real-io @a11y
  Scenario: Space activates the close control exactly as Enter does
    # Native-button freebies are asserted, never assumed (DESIGN open question 2):
    # BOTH keys get their own activation proof.
    Given Mei has moved focus to the close control of the open "GEN-1" edit dialog
    When Mei presses "Space"
    Then the dialog closes
    And the board is interactive again

  # ------------------------------------------------ Esc-parity of the after-state (AC-1.5)

  @needs-browser @us-01 @real-io
  Scenario: Closing with a click leaves Mei exactly where Esc would
    Given Mei has opened the edit dialog for "GEN-1" in a real browser
    When Mei clicks the close control
    Then focus rests on a live element, just as an Esc close leaves it
    And pressing "j" and then "k" still moves the card selection
    And pressing "c" still opens the new-issue dialog

  # ------------------------------------------------ S4 — the existing exits are untouched

  @needs-browser @us-01 @real-io @scoped
  Scenario: Esc still closes the dialog in a single keypress
    # Blast-radius guard (BR-4): the new click trigger must not add a second
    # Escape listener; one press still closes exactly one layer.
    Given Mei has opened the edit dialog for "GEN-1" in a real browser
    When Mei presses "Esc"
    Then the dialog closes
    And the board is interactive again

  @needs-browser @us-01 @real-io @scoped
  Scenario: Saving from the dialog still works with the close control present
    Given Mei has opened the edit dialog for "GEN-1" in a real browser
    When Mei changes the title to "Renamed on purpose" and saves
    Then the dialog closes
    And the "GEN-1" card shows "Renamed on purpose"

  @needs-browser @us-01 @real-io @error
  Scenario: The close control still works from the validation-error state
    # form-errors.js routes the 4xx reason into the open dialog; the × must not
    # trap Mei in that state (AC-1.4).
    Given Mei has opened the edit dialog for "GEN-1" in a real browser
    And Mei has saved it with an empty title and sees "Title is required" inside the dialog
    When Mei clicks the close control
    Then the dialog closes
    And the "GEN-1" card still shows "Keep me"

  @needs-browser @us-01 @real-io @scoped
  Scenario: The full page link beside the close control still navigates
    Given Mei has opened the edit dialog for "GEN-1" in a real browser
    When Mei follows the "Open full page" link
    Then the browser shows the "GEN-1" issue page
