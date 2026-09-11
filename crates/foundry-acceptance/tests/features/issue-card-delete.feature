# Feature: issue-card-delete — a card that should not exist can be removed from
# either surface that already opens it: the edit popup the board swaps into
# #modal-root, and the full issue page. One confirm dialog, one write port, one
# meaning of "deleted".
#
# AUTHORED BY DISTILL, DELIVERED (ADR-025). Every scenario below was scaffolded
# RED as `@pending` and un-pended one at a time by DELIVER, which never
# re-authored one; none is pending now. The seams they drive are the DESIGN port
# signatures (feature-delta DDD-1/DDD-7), all shipped:
# foundry-store::issue_delete (delete_issues_with_outbox, IssueDeleteContext,
# Store::delete_issue_with_outbox), foundry-services::issues (delete_issue,
# delete_issue_dialog, IssueDeleteView), the foundry-app::issues
# show_delete_form / submit_delete handlers, the two Delete controls and both
# dialog templates. This file is now the feature's living specification: it is
# the contract a change to any of those seams must keep, not a to-do list.
#
# NO WALKING SKELETON (DISCUSS D14). The services seam, CSRF middleware, the
# confirm-dialog contract, #modal-root, the OOB #board-columns refresh, the
# outbox->LISTEN->SSE topology and all three test lanes are shipped. There is
# deliberately ZERO @walking_skeleton tag in this file — its absence is a
# decision, not an omission, exactly as in board-lane-reorder.
#
# ORACLE DISCIPLINE (the house traps this file refuses):
#  - "The card is gone" is asserted from the DATABASE and from the RENDERED
#    BOARD, never from the response status alone. A 303 to the board proves the
#    handler ran, not that the row went.
#  - THE CASCADE ORACLE COUNTS CHILDREN, NEVER "no error was raised". A delete
#    that left comments, attachments or change events behind raises nothing:
#    the FKs are ON DELETE CASCADE, so an orphan is only reachable if the
#    cascade silently did not fire. Every mutating scenario runs the
#    orphaned-children guard query (LEFT JOIN issues WHERE issues.id IS NULL
#    across comments, issue_attachments and issue_change_events) and asserts 0.
#  - THE ANNOUNCEMENT ORACLE COUNTS ROWS BOTH WAYS. A missing IssueDeleted and a
#    spurious one are different bugs, and "the second board updated" cannot
#    distinguish them from a reload. Scenarios assert the outbox row count
#    (exactly N for N destroyed cards, exactly 0 for every refusal) AND the
#    payload's key, AND the second board's rendered cards.
#  - DELETE-CONTROL PLACEMENT IS A MARKUP ORACLE, NOT A STYLING ONE. AC-2.1 is
#    asserted as DOM containment — the control is not a descendant of the edit
#    <form> — because a Delete that merely *looks* far from Save is the exact
#    trap adr-modal-close-001 D-12 exists to prevent, and CSS cannot be trusted
#    to keep it.
#  - Refusals are compared BYTE-IDENTICAL to a never-existed path, so a delete
#    naming a vanished issue cannot be distinguished from one naming an issue on
#    another workspace's board.
#  - The zero-laneless guard query runs after every mutating scenario.
#
# FOUR SCENARIOS EXIST BECAUSE OF THE DESIGN, not the acceptance criteria:
#  1..3. The three lane-delete scenarios (@us-icd-03). DDD-2 folds lane delete
#     onto the same primitive, so lane delete stops being silent. That is a
#     behaviour change to SHIPPED code — ADR-BOARD-LANE-002 is amended in one
#     clause — and these three are what prove the amendment is one clause: the
#     fate=move arm must still emit IssueUpdated and NOT IssueDeleted, and the
#     last-lane refusal, DestinationNotFound refusal and bounded retry must all
#     come through unchanged.
#  4. "A comment filed while the dialog is open goes with the issue" — the D13
#     advisory-count boundary. The count is copy; the confirm binds. A scenario
#     that only ever deletes an issue whose counts never move cannot tell an
#     advisory read from a precondition.
#
# CSRF HONESTY (inherited from fix-comment-delete-csrf): the HTTP lane injects
# the token, which is exactly how a real-browser 403 stayed hidden once before.
# The tokenless-refusal scenario pins the HTTP contract; the browser lane drives
# a REAL popup delete through the live origin so a missing token cannot pass
# twice.
#
# NO-JS HONESTY (DDD-6, ADR-ISSUE-DELETE-002): the scripting-disabled scenario
# must follow a real <a href> to a real page and submit a real <form>. Asserting
# that the route merely answers 200 to a GET would pass over a bare unstyled
# fragment, which is the outcome that decision rejected.
#
# Grounding SSOT: docs/feature/issue-card-delete/feature-delta.md (DISCUSS
# D1-D14, US-ICD-01..03 incl. the DESIGN amendments AC-3.8..3.10; DESIGN
# DDD-1..DDD-12) and docs/product/architecture/adr-issue-delete-001/-002.md.

@icd
Feature: Removing a card that should not be on the board

  Background:
    Given Priya is a Backend team member tidying her own boards

  # ========================================================================
  # US-ICD-01 — delete from the full page: the whole write path (slice 01)
  # ========================================================================

  @us-icd-01 @driving_port
  Scenario: The confirm names the issue and counts what goes with it
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    And AUTH-42 carries three comments and one attachment
    When Priya asks to delete AUTH-42 from its own page
    Then she is asked to confirm deleting AUTH-42
    And the confirmation says its three comments and one attachment go with it
    And the confirmation says this cannot be undone
    And AUTH-42 is still on the board

  @us-icd-01 @driving_port
  Scenario: The confirm says so when nothing goes with the issue
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    And AUTH-41 carries no comments and no attachments
    When Priya asks to delete AUTH-41 from its own page
    Then she is asked to confirm deleting AUTH-41
    And the confirmation says nothing else goes with it
    And the confirmation says this cannot be undone

  @us-icd-01 @driving_port @real-io
  Scenario: Confirming removes the card and returns her to the board
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    When Priya deletes AUTH-42 from its own page
    Then she is looking at the "Identity Platform" board
    And the board holds AUTH-41 and AUTH-43 only

  @us-icd-01 @real-io
  Scenario: Everything hanging off the card goes with it
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    And AUTH-42 carries three comments and one attachment
    And AUTH-42 has been edited twice
    When Priya deletes AUTH-42 from its own page
    Then nothing of AUTH-42 remains anywhere
    And no comment, attachment or change record is left behind without its card

  @us-icd-01 @real-io
  Scenario: The cards beside it are untouched
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    And AUTH-41 and AUTH-43 sit in the same lane as AUTH-42
    When Priya deletes AUTH-42 from its own page
    Then nothing of AUTH-42 remains anywhere
    And AUTH-41 and AUTH-43 are in the same lane and the same order as before
    And every comment and attachment on AUTH-41 and AUTH-43 is still there

  @us-icd-01 @real-io
  Scenario: Deleting a card is not a change to the board's lanes
    Given "Identity Platform" (AUTH) is a board whose lanes are Backlog, In-Progress and Done
    When Priya deletes AUTH-42 from its own page
    Then nothing of AUTH-42 remains anywhere
    And the board still reads Backlog, In-Progress, Done
    And every lane still has the slug and label it had
    And no issue is left without a lane

  @us-icd-01 @real-io
  Scenario: A card filed by someone else can be deleted by any team member
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    And AUTH-42 was filed by another Backend team member
    When Priya deletes AUTH-42 from its own page
    Then the board holds AUTH-41 and AUTH-43 only

  @us-icd-01 @real-io
  Scenario: A comment filed while the confirmation is open goes with the issue
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    And AUTH-42 carries three comments and one attachment
    And Priya has been asked to confirm deleting AUTH-42
    When another team member comments on AUTH-42 and Priya then confirms
    Then nothing of AUTH-42 remains anywhere
    And no comment, attachment or change record is left behind without its card

  @us-icd-01 @error
  Scenario: Deleting a card on a board she is not a member of is refused indistinguishably
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    When Marco asks to delete AUTH-42
    Then the refusal is byte-identical to a card that never existed
    And AUTH-42 is still on the board

  @us-icd-01 @error
  Scenario: A non-member is refused at the confirmation as well as at the delete
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    When Marco asks to confirm deleting AUTH-42
    Then the refusal is byte-identical to a card that never existed
    And AUTH-42 is still on the board

  @us-icd-01 @error
  Scenario: A signed-out delete is refused the same way
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    When someone who is not signed in asks to delete AUTH-42
    Then AUTH-42 is still on the board
    And no comment, attachment or change record is left behind without its card

  @us-icd-01 @error
  Scenario: Deleting a card that is already gone is refused indistinguishably
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    And AUTH-42 has already been deleted
    When Priya confirms deleting AUTH-42 a second time
    Then the refusal is byte-identical to a card that never existed
    And the board holds AUTH-41 and AUTH-43 only

  @us-icd-01 @error
  Scenario: Deleting a card that never existed is refused the same way
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    When Priya asks to delete a card that was never filed
    Then the refusal is byte-identical to a card that never existed
    And the board holds AUTH-41, AUTH-42 and AUTH-43

  @us-icd-01 @error
  Scenario: A delete that carries no token is refused before anything is written
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    When Priya's delete of AUTH-42 arrives without its token
    Then AUTH-42 is still on the board
    And nothing was announced to anyone

  @us-icd-01 @needs-browser @real-io
  Scenario: The whole path works with scripting switched off
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    And Priya is browsing with scripting switched off
    When she follows the delete link on AUTH-42's page and confirms
    Then she is looking at the "Identity Platform" board
    And the board holds AUTH-41 and AUTH-43 only

  @us-icd-01 @needs-browser
  Scenario: The confirmation is a readable page when scripting is switched off
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    And Priya is browsing with scripting switched off
    When she follows the delete link on AUTH-42's page
    Then she is looking at a full page asking her to confirm deleting AUTH-42
    And the page carries the site's own heading and navigation

  # ---------------------------------------------------------------- REGRESSION
  # The full page with scripting ON. Until these existed, EVERY full-page
  # scenario above was either HTTP-lane (reqwest, which never executes htmx or
  # keyboard.js and so cannot observe either defect) or `@needs-browser` with
  # scripting switched OFF (where htmx is absent by construction, the plain
  # `href` is followed, and the confirm's close control is inert by design and
  # unasserted). That left the surface an operator actually uses — a real
  # browser, scripting on — covered by nothing, and two defects shipped green
  # through 632 passing tests:
  #
  #   1. `issue.html`'s Delete carried `hx-target="#modal-root"`, but
  #      `#modal-root` is declared ONLY in `board.html`. htmx claimed the click,
  #      failed to resolve the target (`htmx:targetError`), and never followed
  #      the href — so the full page could not delete at all.
  #   2. `delete_issue_modal_page.html` includes the SAME partial the popup
  #      renders (DDD-6), whose only close affordance is the popup-only
  #      `[data-action="close-modal"]` trigger — `closeModal()` empties
  #      `#modal-root`, which that page does not have. The x rendered and did
  #      nothing; Back was the only way out of a destructive dialog.
  #
  # Both are the SAME root cause: a board-scoped host behind an affordance on a
  # non-board surface. The oracles below are therefore deliberately about
  # ARRIVAL, not markup — "she got somewhere" is the one thing neither defect
  # could fake.

  @us-icd-01 @needs-browser @real-io @regression @icd-page-js
  Scenario: Delete on the full page reaches the confirmation with scripting on
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    When Priya clicks Delete on AUTH-42's page in a real browser
    Then she is looking at a full page asking her to confirm deleting AUTH-42
    And AUTH-42 is still on the board

  @us-icd-01 @needs-browser @real-io @regression @icd-page-js
  Scenario: The confirmation's close control is a way out with scripting on
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    When Priya clicks Delete on AUTH-42's page in a real browser and closes the confirmation
    Then she is looking at AUTH-42's own page
    And AUTH-42 is still on the board

  # ========================================================================
  # US-ICD-02 — delete from the popup, board updates in place (slice 02)
  # ========================================================================

  @us-icd-02 @driving_port
  Scenario: The card's own dialog offers Delete where it cannot be mistaken for Save
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    When Priya opens AUTH-42 from the board
    Then the dialog offers Delete
    And Delete cannot submit the edit she is looking at
    And Save is still the only way to save

  @us-icd-02 @driving_port
  Scenario: Choosing Delete asks before it acts
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    And AUTH-42 carries three comments and one attachment
    When Priya opens AUTH-42 from the board and chooses Delete
    Then she is asked to confirm deleting AUTH-42
    And AUTH-42 is still on the board

  @us-icd-02 @error
  Scenario: Choosing Delete does not save a half-typed edit
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    When Priya opens AUTH-42, retitles it without saving, and chooses Delete
    Then AUTH-42 still has the title it had
    And she is asked to confirm deleting AUTH-42

  @us-icd-02 @driving_port @real-io
  Scenario: Confirming from the board removes the card without leaving the board
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    When Priya opens AUTH-42 from the board and deletes it
    Then she is still looking at the "Identity Platform" board
    And the board holds AUTH-41 and AUTH-43 only
    And nothing is asking for her attention

  @us-icd-02 @real-io
  Scenario: The refreshed board keeps every other card and every lane operation
    Given "Identity Platform" (AUTH) is a board whose lanes are Backlog, In-Progress and Done
    And AUTH-41, AUTH-42 and AUTH-43 are spread across those lanes
    When Priya opens AUTH-42 from the board and deletes it
    Then the board still reads Backlog, In-Progress, Done
    And AUTH-41 and AUTH-43 are in the lanes and the order they were in
    And each column still offers all six lane operations

  @us-icd-02 @needs-browser @error
  Scenario: Backing out of the confirmation leaves the card alone
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    When Priya opens AUTH-42 from the board, chooses Delete, then dismisses the confirmation
    Then the board holds AUTH-41, AUTH-42 and AUTH-43
    And nothing was announced to anyone

  @us-icd-02 @needs-browser @driving_port @real-io
  Scenario: A real browser carries the delete through
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    When Priya deletes AUTH-42 from the board in a real browser
    Then the board holds AUTH-41 and AUTH-43 only

  @us-icd-02 @real-io
  Scenario: Saving an edit from the card's dialog still works exactly as before
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    When Priya opens AUTH-42 from the board and saves a new title
    Then the card on the board shows the new title
    And the board holds AUTH-41, AUTH-42 and AUTH-43

  @us-icd-02 @error
  Scenario: A non-member deleting from the board is refused indistinguishably
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    When Marco deletes AUTH-42 from the board
    Then the refusal is byte-identical to a card that never existed
    And AUTH-42 is still on the board

  # ========================================================================
  # US-ICD-03 — the card leaves every open board (slice 03)
  # ========================================================================

  @us-icd-03 @needs-browser @real-io
  Scenario: A deleted card leaves a board someone else is looking at
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    And the same board is open in another window
    When Priya deletes AUTH-42 from the board
    Then the other window stops showing AUTH-42 without being reloaded
    And the other window still shows AUTH-41 and AUTH-43

  @us-icd-03 @real-io
  Scenario: The announcement names the card that went
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    And someone is listening to the "Identity Platform" board
    When Priya deletes AUTH-42 from its own page
    Then exactly one card was announced as deleted
    And the announcement names AUTH-42

  @us-icd-03 @error
  Scenario: Another project's board hears nothing
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    And someone is listening to the "Homelab Ops" board instead
    When Priya deletes AUTH-42 from its own page
    Then exactly one card was announced as deleted
    And nothing was announced to that listener

  @us-icd-03 @error
  Scenario: A refused delete announces nothing
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    And someone is listening to the "Identity Platform" board
    When Marco deletes AUTH-42 from the board
    Then the refusal is byte-identical to a card that never existed
    And nothing was announced to anyone
    And AUTH-42 is still on the board

  @us-icd-03 @real-io
  Scenario: Comments on other cards still arrive alongside
    Given "Identity Platform" (AUTH) is a board holding AUTH-41, AUTH-42 and AUTH-43
    And someone is listening to the "Identity Platform" board
    When another team member comments on AUTH-41 and Priya then deletes AUTH-42
    Then the listener heard the comment on AUTH-41 and the deletion of AUTH-42
    And each was announced exactly once

  # --- the DDD-2 consequence: lane delete stops being silent ---------------

  @us-icd-03 @real-io
  Scenario: Emptying a lane by deleting it announces every card that went
    Given "Identity Platform" (AUTH) is a board whose lanes are Backlog, In-Progress and Done
    And AUTH-41, AUTH-42 and AUTH-43 all sit in Done
    And someone is listening to the "Identity Platform" board
    When Priya deletes the Done lane and its cards with it
    Then exactly three cards were announced as deleted
    And the announcements name AUTH-41, AUTH-42 and AUTH-43
    And no comment, attachment or change record is left behind without its card

  @us-icd-03 @real-io
  Scenario: Moving a lane's cards still announces them as moved, never as deleted
    Given "Identity Platform" (AUTH) is a board whose lanes are Backlog, In-Progress and Done
    And AUTH-41, AUTH-42 and AUTH-43 all sit in Done
    And someone is listening to the "Identity Platform" board
    When Priya deletes the Done lane and moves its cards to Backlog
    Then exactly three cards were announced as moved
    And no card was announced as deleted
    And AUTH-41, AUTH-42 and AUTH-43 are all in Backlog

  @us-icd-03 @error
  Scenario: A lane delete that is refused still announces nothing
    Given "Identity Platform" (AUTH) is a board with a single lane holding AUTH-41
    And someone is listening to the "Identity Platform" board
    When Priya tries to delete the only lane the board has
    Then she is told a board needs at least one lane
    And nothing was announced to anyone
    And AUTH-41 is still on the board

  @us-icd-03 @needs-browser @real-io
  Scenario: A second board loses every card of a lane that was deleted
    Given "Identity Platform" (AUTH) is a board whose lanes are Backlog, In-Progress and Done
    And AUTH-41, AUTH-42 and AUTH-43 all sit in Done
    And the same board is open in another window
    When Priya deletes the Done lane and its cards with it
    Then the other window stops showing AUTH-41, AUTH-42 and AUTH-43 without being reloaded
