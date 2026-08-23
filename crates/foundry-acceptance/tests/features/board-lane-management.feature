# Feature: board-lane-management — lanes become the project's own data. New
# projects start with Backlog, In-Progress and Done; any lane can be deleted
# from the board, and deleting a lane that holds cards asks the operator — in a
# dialog — whether to move those cards to another lane or delete them.
#
# SCAFFOLDED RED (DISTILL, ADR-025): every scenario below carries @pending.
# DELIVER un-pends one at a time; it never re-authors. The production scaffolds
# this suite drives are the DESIGN port signatures (component-boundaries.md):
# foundry-store::lanes (list_project_lanes, delete_lane_with_fate),
# foundry-services::lanes (delete_lane_dialog, delete_lane, classify_lane_delete),
# foundry_services::issues::validate_project_lane, foundry_services::board::board_view,
# and the mounted foundry-app::lanes handlers (dialog GET + confirm POST, both
# answering a clean 501 until DELIVER). Migration 0015 is DELIVER-owned
# (behaviour-changing) — pre-0015 the `lanes` relation does not exist, so every
# Given that seeds lane rows fails structurally: MISSING_FUNCTIONALITY(schema),
# the honest RED for a feature whose storage does not exist yet.
#
# ORACLE DISCIPLINE (the house traps this file refuses):
#  - Board-render assertions read the expected lane list BACK FROM THE DATABASE
#    (lanes rows: slug, label, position) — never from DEFAULT_COLUMNS, which
#    slice 01 deletes. A test-local static lane list would go green over the
#    exact static-list consumers D8 exists to remove.
#  - The move-fate ordering oracle captures the destination lane's card order
#    BEFORE the confirm and asserts the appended order AFTER, from both the
#    stored rows and the rendered column. Never recomputed, never assumed.
#  - Mutating scenarios snapshot the FULL board universe first (lane rows;
#    every issue's (lane, position); change-event and outbox counts) and assert
#    the declared delta fail-closed: on a lane-delete move fate ONLY the moved
#    cards' (lane, position) may change; any other drift is a violation.
#  - Refusals are compared BYTE-IDENTICAL to a never-existed path (the
#    non-enumerability idiom). Note the DELIBERATE asymmetry (DESIGN
#    refinement 4): the board PAGE shows a same-workspace non-member a 403;
#    the lane routes answer the SAME principal with the uniform 404 (D10's
#    explicit pin). The authz scenario pins the lane routes' 404, both verbs.
#  - The zero-laneless guard query (architecture-design.md §4) runs after every
#    mutating scenario: no issue may reference a lane its project does not have.
#
# RACE HONESTY (US-BLM-04 scenario 5): the in-lane race leg exercises the
# "card committed into the dying lane BEFORE the confirm POST" interleaving —
# deterministic from the driving ports (dialog GET → machine filing → confirm).
# The narrower interleaving (a card committing between the transaction's
# membership snapshot and its lane-row DELETE) is not deterministically
# reachable from any port: it requires pausing delete_lane_with_fate mid-
# transaction. The composite FK (ADR-BOARD-LANE-001) makes that window a retry,
# not a strand, and the PINNED OBSERVABLE — zero laneless issues after every
# mutation, provable by query — holds across all interleavings. That guard is
# asserted here; the mid-transaction pause itself is documented as untestable
# in-lane (feature-delta.md, Wave: DISTILL).
#
# RETIREMENT NOTE: keyboard-shortcut-bindings' "@named-edge" scenario ("Enter
# is a no-op for a found issue that the board does not render") was premised on
# `cancelled` being findable-but-cardless (UNRENDERED_STATE). Slice 01 destroys
# that premise BY DESIGN (KPI 2: zero invisible issues, permanently — the FK
# makes a cardless state unreachable). It is retired in this wave with its
# rationale recorded in the delta; its successor coverage is the
# "long-invisible cancelled issue gets a visible lane" scenario below plus the
# existing FR-9 no-selection no-op scenarios, which survive unchanged.
#
# Grounding SSOT: docs/feature/board-lane-management/feature-delta.md (DISCUSS
# D1-D11 + US-BLM-01..04; DESIGN component-boundaries.md port signatures +
# markup contracts; ADR-BOARD-LANE-001/002; brief.md lanes invariant).
#
# Harness: the SAME in-process axum router + real session/CSRF layers + real
# Postgres (shared testcontainer, per-scenario schema) as every board scenario.
# The machine-client legs mint a REAL registered EdDSA bearer (fixed test key,
# the Feature-A idiom). The migration oracle stages 0001..0014 into a TempDir
# (test_migration precedent), seeds pre-0015-shaped data, then runs the
# canonical dir twice through the SAME run_migrations_from_dir the production
# boot path uses. The two @needs-browser scenarios drive a REAL headless
# Chrome (fantoccini) because the HTTP lane is byte-blind to the htmx dialog
# swap, the fate-button submitter inclusion, and the OOB column refresh.

@blm
Feature: Shaping a board's lanes to match how the work actually flows

  Background:
    Given Priya is a member of team Backend in workspace Canzan Labs

  # ========================================================================
  # US-BLM-01 — lanes become data (slice 01, walking skeleton)
  # ========================================================================

  @us-blm-01 @walking_skeleton @driving_port @real-io
  Scenario: Existing boards render unchanged when lanes become data
    Given "Identity Platform" (AUTH) is a grandfathered board with its four working lanes
    And AUTH-7 sits in Backlog, AUTH-12, AUTH-15 and AUTH-18 sit in Todo top to bottom, AUTH-3 sits in In-Progress and AUTH-1 sits in Done
    When Priya opens the "Identity Platform" board
    Then the columns are exactly the board's own lanes, in the board's own order
    And every card sits in the same column at the same position as before the upgrade
    And no Cancelled column appears

  @us-blm-01 @driving_port @real-io
  Scenario: A long-invisible cancelled issue gets a visible lane
    Given "Homelab Ops" (OPS) is a grandfathered board granted a Cancelled lane, holding OPS-9 "Replace UPS battery" in Cancelled
    When Priya opens the "Homelab Ops" board
    Then a Cancelled column renders after Done, holding OPS-9

  @us-blm-01
  Scenario: The edit dialog offers exactly the board's lanes
    Given "Homelab Ops" (OPS) is a grandfathered board granted a Cancelled lane, holding OPS-9 "Replace UPS battery" in Cancelled
    When Priya opens the edit dialog for OPS-9
    Then the Status options are exactly the board's five lanes, in board order

  @us-blm-01 @error
  Scenario: A write to a lane the board does not have is refused
    Given "Identity Platform" (AUTH) is a grandfathered board with its four working lanes
    And AUTH-7 sits in Backlog
    When a machine client moves AUTH-7 to "cancelled"
    Then the move is refused as invalid and AUTH-7 has not moved
    And no issue on any board is without a lane

  @us-blm-01 @real-io
  Scenario: Drag-and-drop still lands cards exactly where they are dropped
    Given "Identity Platform" (AUTH) is a grandfathered board with its four working lanes
    And AUTH-12 sits in Todo and AUTH-3 sits in In-Progress
    When Priya drags AUTH-12 to the top of In-Progress
    Then AUTH-12 renders at the top of In-Progress and stays there on reload
    And the change report records AUTH-12's move

  @us-blm-01 @real-io @adapter-integration
  Scenario: The upgrade grandfathers every existing board and can run twice safely
    Given a database from before the upgrade holds "Identity Platform" with issues in its four working states and "Homelab Ops" with one cancelled issue
    When the upgrade migrations run, and then run again
    Then "Identity Platform" has exactly the lanes Backlog, Todo, In-Progress and Done, in that order
    And "Homelab Ops" additionally has a Cancelled lane after Done
    And no issue row was rewritten by the upgrade
    And the store structurally refuses an issue without a lane

  # ========================================================================
  # US-BLM-02 — new projects start with Backlog, In-Progress, Done (slice 02)
  # ========================================================================

  @us-blm-02 @driving_port @real-io
  Scenario: A new project's board opens with the three default lanes
    Given Priya creates project "Reading List" in team Backend
    When she opens the "Reading List" board
    Then the columns are exactly Backlog, In-Progress and Done, in that order

  @us-blm-02
  Scenario: The first issue lands in the leftmost lane
    Given the fresh "Reading List" board
    When Priya files READ-1 "Dune"
    Then READ-1 appears as a card in Backlog

  @us-blm-02 @edge
  Scenario: The filing reply names the lane the issue actually landed in
    Given the fresh "Reading List" board
    When a machine client files "Children of Time" into "Reading List"
    Then the reply says the new issue landed in Backlog
    And the board shows it there

  @us-blm-02
  Scenario: The edit dialog of a new project offers exactly the three lanes
    Given the fresh "Reading List" board
    And Priya files READ-1 "Dune"
    When Priya opens the edit dialog for READ-1
    Then the Status options are exactly Backlog, In-Progress and Done

  @us-blm-02 @error
  Scenario: The board's lanes bound what any client may set
    Given the fresh "Reading List" board
    And Priya files READ-1 "Dune"
    When a machine client moves READ-1 to "in_progress" and then to "todo"
    Then the first move succeeds and the second is refused as invalid, with READ-1 still In-Progress

  # ========================================================================
  # US-BLM-03 — delete an empty lane (slice 03)
  # ========================================================================

  @us-blm-03 @driving_port @real-io
  Scenario: An empty lane disappears after an explicit confirm
    Given "Homelab Ops" (OPS) is a grandfathered board whose Todo lane holds no issues
    When Priya asks to delete the Todo lane and confirms in the dialog
    Then the Todo column is gone without a full page reload and remains gone on reload
    And the edit dialog no longer offers Todo and a client can no longer move a card there

  # DESIGN refinement 2 pinned: the delete TRIGGER is a safe read — fetching
  # the dialog mutates nothing at all. The state-delta here declares the full
  # board universe with every entry unchanged.
  @us-blm-03 @edge
  Scenario: Asking for the confirm dialog changes nothing by itself
    Given "Homelab Ops" (OPS) is a grandfathered board whose Todo lane holds no issues
    When Priya opens the delete dialog for the Todo lane
    Then the dialog states the lane holds no issues and that this cannot be undone
    And the board, its lanes and every card are untouched

  @us-blm-03 @error
  Scenario: The last lane cannot be deleted
    Given project "Scratch" (SCR) has exactly one lane, Done
    When Priya asks to delete Done and confirms
    Then she is refused with the reason "A board needs at least one lane" and the lane remains

  @us-blm-03 @edge
  Scenario: New issues follow the leftmost surviving lane
    Given the fresh "Reading List" board
    And Priya deleted the empty Backlog lane, leaving In-Progress and Done
    When her automation files "Children of Time" into "Reading List"
    Then the reply says the new issue landed in In-Progress and the board shows it there

  # The 404-vs-403 asymmetry is DELIBERATE (DESIGN refinement 4): the board
  # page maps a same-workspace non-member to a 403 page; the lane routes map
  # the SAME principal to the uniform non-enumerable 404, on BOTH verbs.
  @us-blm-03 @error @security
  Scenario: Only team members can delete a lane
    Given "Homelab Ops" (OPS) is a grandfathered board whose Todo lane holds no issues
    And Marco is signed in to Canzan Labs but is not a member of team Backend
    When Marco sends the lane-delete confirm for Todo on "Homelab Ops" directly
    Then the answer is byte-identical to a never-existed address
    And Marco asking for the delete dialog is answered identically
    And the Todo lane is still on the board

  @us-blm-03 @error @security
  Scenario: A delete that does not carry the board's matching token is refused
    Given "Homelab Ops" (OPS) is a grandfathered board whose Todo lane holds no issues
    When a lane-delete confirm for Todo is submitted without the board's matching token
    Then the delete is refused before any change is made
    And the Todo lane is still on the board

  # ========================================================================
  # US-BLM-04 — deleting a full lane asks: move the cards, or delete them
  # ========================================================================

  @us-blm-04 @driving_port @real-io
  Scenario: Cards move to the chosen lane and keep their order
    Given "Identity Platform" (AUTH) is a grandfathered board with its four working lanes
    And AUTH-7 sits in Backlog and AUTH-12, AUTH-15 and AUTH-18 sit in Todo top to bottom
    When Priya deletes the Todo lane choosing to move all 3 to Backlog
    Then the Todo column is gone and AUTH-12, AUTH-15 and AUTH-18 sit at the bottom of Backlog in that order
    And the change report shows a move from Todo to Backlog for each of the three, attributed to Priya

  @us-blm-04 @real-io
  Scenario: Cards are deleted only by an explicit, counted, permanent choice
    Given project "Scratch" (SCR) has lanes Backlog and Done, with SCR-2 and SCR-5 in Done
    And SCR-2 carries a comment and an attachment
    When Priya deletes the Done lane, reading that it holds 2 issues and cannot be undone, choosing to delete all 2 permanently
    Then the lane and both cards are gone from the board and neither issue is findable in search
    And nothing of SCR-2 remains, neither its comment nor its attachment

  @us-blm-04 @edge
  Scenario: The prompt offers only surviving lanes as destinations
    Given "Identity Platform" (AUTH) is a grandfathered board with its four working lanes
    And AUTH-12, AUTH-15 and AUTH-18 sit in Todo top to bottom
    When Priya opens the delete dialog for the Todo lane
    Then the dialog states the lane holds 3 issues
    And the destination picker lists exactly Backlog, In-Progress and Done with Backlog preselected

  @us-blm-04 @edge
  Scenario: Walking away from the prompt changes nothing
    Given "Identity Platform" (AUTH) is a grandfathered board with its four working lanes
    And AUTH-12, AUTH-15 and AUTH-18 sit in Todo top to bottom
    And Priya has the delete dialog for the Todo lane in front of her
    When she walks away without confirming
    Then the Todo lane, all three cards and the change history are untouched

  @us-blm-04 @error @edge @real-io
  Scenario: A card filed mid-decision is still accounted for
    Given "Identity Platform" (AUTH) is a grandfathered board with its four working lanes
    And AUTH-12, AUTH-15 and AUTH-18 sit in Todo top to bottom
    And Priya has the delete dialog for the Todo lane in front of her, reading 3 issues
    When her automation lands one more issue in Todo before she confirms moving all to Backlog
    Then all four cards that were in Todo at confirm time sit in Backlog
    And no issue on any board is without a lane

  # ========================================================================
  # @needs-browser — the DOM oracle. The HTTP lane is byte-blind to the htmx
  # dialog swap into #modal-root, to WHICH fate button's name/value the
  # browser submits (Earned Trust: htmx submitter inclusion), and to the
  # out-of-band board-column refresh. These two scenarios drive a REAL
  # headless Chrome against the same in-process origin.
  # ========================================================================

  @us-blm-04 @needs-browser @driving_port @real-io @pending
  Scenario: Deleting a full lane from the board is one visible, counted decision
    Given "Identity Platform" (AUTH) is a grandfathered board with its four working lanes
    And AUTH-7 sits in Backlog and AUTH-12, AUTH-15 and AUTH-18 sit in Todo top to bottom
    And Priya has the "Identity Platform" board open in her browser
    When she clicks the delete control on the Todo column
    Then a dialog appears stating the lane holds 3 issues
    When she confirms moving all 3 to Backlog
    Then the Todo column disappears without the page reloading
    And the three cards appear at the bottom of the Backlog column

  # ADR-MODAL-CLOSE-001 pinned: the dialog closes through the declarative
  # data-action="close-modal" trigger and the single Escape owner — template-
  # only wiring, no new listeners.
  @us-blm-03 @needs-browser @edge @real-io @pending
  Scenario: The delete dialog closes like every other dialog, leaving the board alone
    Given "Homelab Ops" (OPS) is a grandfathered board whose Todo lane holds no issues
    And Priya has the "Homelab Ops" board open in her browser
    When she clicks the delete control on the Todo column
    Then the delete dialog appears
    When she dismisses it with the close control
    Then the dialog is gone, the Todo column is still on the board, and the page did not reload
    When she reopens the dialog and presses Esc
    Then the dialog is gone again with the board untouched
