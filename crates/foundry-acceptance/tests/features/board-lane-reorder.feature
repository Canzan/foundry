# Feature: board-lane-reorder — the board's lane order stops being fixed at
# creation. A lane moves left or right by dragging its column header, or from
# the same `⋯` menu that already renames, inserts and deletes. A move relocates
# the lane and nothing else: no card changes lane, no lane changes identity.
#
# SCAFFOLDED RED (DISTILL, ADR-025): every scenario below is authored `@pending`.
# DELIVER un-pends one at a time and never re-authors. The production scaffolds
# this suite drives are the DESIGN port signatures (feature-delta DDD-5/DDD-8):
# foundry-store::lanes (move_lane_before, LaneMoveOutcome), foundry-services::
# lanes (move_lane, MoveLaneError), and the mounted foundry-app::lanes
# submit_move_lane handler (answering a clean 501 until DELIVER). The two new
# menu items and the drag surface are behaviour-changing template/JS work and
# therefore DELIVER-owned, so they do not exist yet: every menu and drag oracle
# fails structurally — MISSING_FUNCTIONALITY(markup) — the honest RED for an
# affordance not yet built.
#
# NO WALKING SKELETON (DISCUSS D13). The lanes-as-data foundation, the position
# machinery, the `⋯` menu and the OOB #board-columns refresh are all shipped.
# There is deliberately ZERO @walking_skeleton tag in this file — its absence is
# a decision, not an omission, exactly as in the predecessor wave.
#
# ORACLE DISCIPLINE (the house traps this file refuses):
#  - Lane expectations read lane rows BACK FROM THE DATABASE (slug, label,
#    position). This suite holds NO static expected-lane list — one would go
#    green over the exact static-list consumers check-arch exists to forbid.
#  - THE CONCURRENCY ORACLE ASSERTS THE RESULTING ORDER, NEVER "no error was
#    raised" (feature-delta DDD-4). This is the single most important line in
#    this file. ADR-BOARD-LANE-006 Finding 4 MEASURED that two unlocked
#    concurrent moves raise NO error, keep every invariant intact — contiguous,
#    no duplicates, zero laneless issues — and still leave the board arranged as
#    nobody asked, with a lane neither operator mentioned shoved past another.
#    An "it did not throw" assertion is GREEN on that corrupt case. So is a
#    contiguity assertion. Only the order distinguishes them.
#  - A move must write ZERO issue rows, ZERO change events and ZERO outbox rows,
#    and must leave every lane's slug and label byte-identical. Snapshot the
#    universe before, snapshot after, assert the delta fail-closed.
#  - Contiguity (positions 0..n-1, unique) is asserted explicitly. Postgres
#    enforces uniqueness only; a gap is invisible to the schema and merely
#    cosmetic to ORDER BY position.
#  - Refusals are compared BYTE-IDENTICAL to a never-existed path, so a move
#    naming a vanished lane cannot be distinguished from one naming a lane on
#    another workspace's board.
#  - The zero-laneless guard query runs after every mutating scenario.
#
# TWO SCENARIOS EXIST BECAUSE OF THE DESIGN SPIKE, not the acceptance criteria:
#  1. "Two operators moving lanes at once leave the board as asked" — the
#     silent-race case above. Measured 5/5, ADR-BOARD-LANE-006 Finding 4.
#  2. "Dragging a card still moves the card" — the ADR-BOARD-LANE-007 boundary.
#     The shipped card-drag scenarios must ALSO pass unmodified; this one asserts
#     the boundary from the lane side.
#
# CSRF HONESTY (inherited from fix-comment-delete-csrf): the HTTP lane injects
# the token, which is exactly how a real-browser 403 stayed hidden once before.
# The tokenless-refusal scenario pins the HTTP contract; the browser lane drives
# a REAL menu move and a REAL drop through the live origin so a missing token
# cannot pass twice.
#
# Grounding SSOT: docs/feature/board-lane-reorder/feature-delta.md (DISCUSS
# D1-D16, US-BLR-01..03; DESIGN DDD-1..DDD-14) and
# docs/product/architecture/adr-board-lane-006/-007.md.

@blr
Feature: Putting a board's lanes in the order the work travels

  Background:
    Given Priya is a Backend team member ordering her own board lanes

  # ========================================================================
  # US-BLR-01 — move from the menu: the whole write path (slice 01)
  # ========================================================================

  @us-blr-01 @driving_port
  Scenario: The menu offers both move directions alongside the shipped operations
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, Done, Staging and In-Progress
    When Priya opens the "Homelab Ops" board to reorder it
    Then the Staging column's menu offers exactly Edit list, Insert list before, Insert list after, Move list left, Move list right and Delete list

  @us-blr-01 @driving_port @real-io
  Scenario: Moving a lane left puts it before its neighbour
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, Done, Staging and In-Progress
    When Priya moves the Staging lane left
    Then the board reads Backlog, Staging, Done, In-Progress
    And every lane keeps the slug and label it had

  @us-blr-01 @driving_port @real-io
  Scenario: Moving a lane right puts it after its neighbour
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, Done, Staging and In-Progress
    When Priya moves the Done lane right
    Then the board reads Backlog, Staging, Done, In-Progress
    And every lane keeps the slug and label it had

  @us-blr-01 @driving_port @real-io
  Scenario: Moving a lane leaves every card exactly where it was
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, Done, Staging and In-Progress
    And OPS-3, OPS-7 and OPS-9 sit in Done
    When Priya moves the Done lane right
    Then the board reads Backlog, Staging, Done, In-Progress
    And no card changed lane or order
    And no change event and no outbox row was written

  @us-blr-01 @real-io
  Scenario: Lane positions stay contiguous after a move
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, Done, Staging and In-Progress
    When Priya moves the In-Progress lane left
    Then the board reads Backlog, Done, In-Progress, Staging
    And the lane positions are contiguous from zero with no duplicates

  @us-blr-01 @driving_port
  Scenario: The leftmost lane cannot be moved further left
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, Done, Staging and In-Progress
    When Priya opens the "Homelab Ops" board to reorder it
    Then the Backlog column offers a disabled Move list left
    And the Backlog column still offers all six operations

  @us-blr-01 @driving_port
  Scenario: The rightmost lane cannot be moved further right
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, Done, Staging and In-Progress
    When Priya opens the "Homelab Ops" board to reorder it
    Then the In-Progress column offers a disabled Move list right
    And the In-Progress column still offers all six operations

  @us-blr-01 @real-io @error
  Scenario: Moving a lane onto its own position writes nothing
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, Done, Staging and In-Progress
    When Priya moves the Done lane to the position it already holds
    Then the move was accepted
    And the board reads Backlog, Done, Staging, In-Progress
    And no change event and no outbox row was written

  @us-blr-01 @error
  Scenario: Moving a lane that is already gone is refused indistinguishably
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, Done, Staging and In-Progress
    When Priya moves a lane that no longer exists
    Then the refusal is byte-identical to a board that never existed
    And the board reads Backlog, Done, Staging, In-Progress

  @us-blr-01 @error
  Scenario: Moving a lane beside a neighbour that is already gone is refused the same way
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, Done, Staging and In-Progress
    When Priya moves the Done lane beside a lane that no longer exists
    Then the refusal is byte-identical to a board that never existed
    And the board reads Backlog, Done, Staging, In-Progress

  @us-blr-01 @error
  Scenario: An outsider cannot reorder someone else's board
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, Done, Staging and In-Progress
    And Marco is signed in without membership of team Backend
    When Marco moves the Staging lane left
    Then the refusal is byte-identical to a board that never existed
    And the board reads Backlog, Done, Staging, In-Progress

  @us-blr-01 @error
  Scenario: A signed-out visitor cannot reorder a board
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, Done, Staging and In-Progress
    When a signed-out visitor moves the Staging lane left
    Then the refusal is byte-identical to a board that never existed
    And the board reads Backlog, Done, Staging, In-Progress

  @us-blr-01 @error
  Scenario: A move without the request token is refused before anything is written
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, Done, Staging and In-Progress
    When Priya moves the Staging lane left without the request token
    Then the move is refused before the handler runs
    And the board reads Backlog, Done, Staging, In-Progress

  @us-blr-01 @real-io @concurrency
  Scenario: Two operators reordering at once leave the board as they asked
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, Done, Staging and In-Progress
    When Priya moves In-Progress before Done while another member moves Staging before Backlog
    Then the board reads Staging, Backlog, In-Progress, Done
    And the lane positions are contiguous from zero with no duplicates
    And no card changed lane or order

  @us-blr-01 @needs-browser @driving_port @real-io
  Scenario: Choosing Move list left from the real menu reorders the board
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, Done, Staging and In-Progress
    When Priya opens the Staging column's menu and chooses Move list left
    Then the board on screen reads Backlog, Staging, Done, In-Progress
    And the menu is closed

  @us-blr-01 @needs-browser
  Scenario: The move items are reachable and operable by keyboard alone
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, Done, Staging and In-Progress
    When Priya reaches the Staging menu by keyboard and activates Move list left
    Then the board on screen reads Backlog, Staging, Done, In-Progress

  # ========================================================================
  # US-BLR-02 — dragging a column header (slice 02)
  # ========================================================================

  @us-blr-02 @needs-browser @driving_port @real-io
  Scenario: Dragging a column header past its neighbour reorders the board
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, Done, Staging and In-Progress
    When Priya drags the Done column past Staging and releases
    Then the board on screen reads Backlog, Staging, Done, In-Progress
    And the board reads Backlog, Staging, Done, In-Progress

  @us-blr-02 @needs-browser
  Scenario: Pressing a column header without moving still opens its menu
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, Done, Staging and In-Progress
    When Priya presses the Staging column's menu trigger without moving the pointer
    Then the Staging column's menu is open
    And the Staging column header is a drag surface
    And the board reads Backlog, Done, Staging, In-Progress

  @us-blr-02 @needs-browser @error
  Scenario: Escape during a drag returns the column and writes nothing
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, Done, Staging and In-Progress
    And Priya has begun dragging the Done column
    When Priya presses Escape to cancel the drag
    Then the board on screen reads Backlog, Done, Staging, In-Progress
    And the board reads Backlog, Done, Staging, In-Progress
    And no change event and no outbox row was written

  @us-blr-02 @needs-browser @error
  Scenario: A refused drop returns the column to exactly where it started
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, Done, Staging and In-Progress
    And the next move will be refused
    When Priya drags the Done column past Staging and releases
    Then the board on screen reads Backlog, Done, Staging, In-Progress
    And the board reads Backlog, Done, Staging, In-Progress

  @us-blr-02 @needs-browser @mobile @real-io
  Scenario: A lane can be moved by touch
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, Done, Staging and In-Progress
    When Priya drags the Done column past Staging with a touch pointer
    Then the board reads Backlog, Staging, Done, In-Progress

  @us-blr-02 @needs-browser @error
  Scenario: Dragging a card still moves the card and never the lane
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, Done, Staging and In-Progress
    And OPS-3 sits in Done
    When Priya drags OPS-3 from Done into Staging
    Then OPS-3 sits in Staging
    And the board reads Backlog, Done, Staging, In-Progress

  # ========================================================================
  # US-BLR-03 — reaching an off-screen destination (slice 03)
  # ========================================================================

  @us-blr-03 @needs-browser @mobile @real-io
  Scenario: Dragging to the edge of a narrow board carries the lane off screen
    Given "Homelab Ops" (OPS) is a board with eight lanes on a narrow screen
    When Priya drags the leftmost column to the right edge and holds until the board scrolls
    Then the board reads with the first lane moved to the far right

  @us-blr-03 @needs-browser @mobile
  Scenario: The board stops scrolling at its own edge
    Given "Homelab Ops" (OPS) is a board with eight lanes on a narrow screen
    When Priya drags a column to the right edge and holds past the end of the board
    Then the board has scrolled no further than its own end
    And the page itself has not scrolled

  @us-blr-03 @needs-browser @error
  Scenario: The drop indicator never outlives the drag
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, Done, Staging and In-Progress
    And Priya has begun dragging the Done column
    When Priya presses Escape to cancel the drag
    Then no drop indicator remains on the board
