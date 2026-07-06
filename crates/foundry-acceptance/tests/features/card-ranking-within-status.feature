# Feature: card-ranking-within-status — rank issue cards within a status, and
# place them precisely when moving across statuses.
#
# Source SSOT for docs/feature/card-ranking-within-status/distill/test-scenarios.md.
# ONE write path (DESIGN ADR-002): the shipped POST /issues/{n}/state gains an
# optional `after` neighbour key. board-dnd.js ALWAYS sends state + after — a
# within-status reorder is "same state + new position"; a cross-status drop is
# "new state + new position", atomic. Rank = a contiguous `position` per
# (project, state) (ADR-001), read `ORDER BY position ASC, number DESC`.
#
# HARNESS NOTE: the HTTP suite pins the persist contract (POST /state with `after`
# → store position + state), the ordered board read, the zero-shuffle default,
# the new-issue slot, tenancy/non-enumerability, and progressive enhancement. The
# live DRAG GESTURE + optimistic client move/revert are browser-dogfooded (JS the
# HTTP harness can't drive) — same split as issue-status-move. v1 realtime is
# state-only (ADR-002 / upstream UC-1): NO live two-client SSE-position scenario;
# cross-viewer convergence is verified as a persisted re-read (a fresh board GET).
#
# EVERY scenario @pending until DELIVER wires the glue + un-@pends (kept out of @all).

@card-ranking @us-card-ranking @driving_port
Feature: A member ranks issue cards within a status
  A member drags a card to an exact slot — within its status column or into
  another — and that order persists for every viewer, through the shipped
  change_issue_state path (now carrying a position), with tenancy/CSRF intact and
  drag as a progressive enhancement.

  Background:
    Given a workspace "Acme" exists with a member "Mei" on team "Backend"
    And a project "Sandbox" (key "GEN") with issues:
      | key   | column |
      | GEN-2 | Todo    |
      | GEN-3 | Backlog |
      | GEN-4 | Todo    |
    And Mei is signed in

  # ---- Slice 01: reorder within a status ----

  @us-01 @real-io
  Scenario: A column with no manual order shows issues newest-first (zero-shuffle)
    When Mei fetches the "Sandbox" board
    Then the "todo" column shows cards in order "GEN-4, GEN-2"

  @us-01 @real-io
  Scenario: Reordering within a column persists the new order
    When Mei drops "GEN-4" after "GEN-2" in "todo" as the drop handler would
    Then "GEN-4" is ranked after "GEN-2" in the "todo" column in the store
    And the "todo" column shows cards in order "GEN-2, GEN-4"

  @us-01 @real-io
  Scenario: Reordering to the top of a column (no neighbour) persists position 0
    When Mei drops "GEN-4" at the top of "todo" as the drop handler would
    Then the "todo" column shows cards in order "GEN-4, GEN-2"

  @us-01 @real-io @error
  Scenario: An unknown neighbour key is refused without changing the order
    When a drop posts an unknown neighbour "GEN-404" for "GEN-4" in "todo"
    Then the response is a non-enumerable refusal
    And the "todo" column shows cards in order "GEN-4, GEN-2"

  @us-01 @real-io @error
  Scenario: A reorder targeting a foreign issue is refused non-enumerably
    Given a foreign issue "ZZZ-9" exists in another workspace
    When Mei drops "ZZZ-9" at the top of "todo" as the drop handler would
    Then the response is a non-enumerable refusal

  @us-01 @real-io
  Scenario: A newly filed issue lands at the top of Backlog
    When Mei files a new issue titled "Fresh triage"
    Then the newest issue is first in the "backlog" column

  @us-01 @real-io
  Scenario: The ranked order is served in the board HTML without JavaScript
    When Mei drops "GEN-4" after "GEN-2" in "todo" as the drop handler would
    And Mei fetches the "Sandbox" board
    Then the "todo" column shows cards in order "GEN-2, GEN-4"
    And the board loads the drag-and-drop script

  # ---- Slice 02: cross-status positional drop (state + rank, atomic) ----

  @us-02 @real-io @pending
  Scenario: Dropping a card into another column at a slot sets state AND rank
    When Mei drops "GEN-3" after "GEN-4" in "todo" as the drop handler would
    Then "GEN-3" has state "todo" in the store
    And "GEN-3" is ranked after "GEN-4" in the "todo" column in the store
    And the "todo" column shows cards in order "GEN-4, GEN-3, GEN-2"

  @us-02 @real-io @pending
  Scenario: A cross-status drop to the top of the target column
    When Mei drops "GEN-3" at the top of "todo" as the drop handler would
    Then "GEN-3" has state "todo" in the store
    And the "todo" column shows cards in order "GEN-3, GEN-4, GEN-2"

  @us-02 @real-io @error @pending
  Scenario: A rejected cross-status drop changes neither state nor rank
    When a drop posts an invalid state for "GEN-3"
    Then the response is a validation error
    And "GEN-3" has state "backlog" in the store
    And the "backlog" column shows cards in order "GEN-3"
