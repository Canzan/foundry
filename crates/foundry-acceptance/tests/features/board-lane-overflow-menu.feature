# Feature: board-lane-overflow-menu — the board's column header stops arming a
# destructive control. One `⋯` menu per column carries Edit list, Insert list
# before, Insert list after and Delete list. Renaming touches a lane's label and
# nothing else; inserting places a working lane exactly where the operator asked,
# making lane deletion no longer one-way.
#
# SCAFFOLDED RED (DISTILL, ADR-025): every scenario below is authored `@pending`.
# DELIVER un-pends one at a time and never re-authors. The production scaffolds
# this suite drives are the DESIGN port signatures (component-boundaries.md):
# foundry-core::lane_slug, foundry-store::lanes (rename_lane, insert_lane_at),
# foundry-services::lanes (edit_lane_dialog, rename_lane, insert_lane_dialog,
# insert_lane), and the mounted foundry-app::lanes handlers (dialog GETs +
# confirm POSTs, all answering a clean 501 until DELIVER). Template changes are
# behaviour-changing and therefore DELIVER-owned, so the menu markup does not
# exist yet: every menu oracle fails structurally on the absent `⋯` trigger —
# MISSING_FUNCTIONALITY(markup), the honest RED for an affordance not yet built.
#
# NO WALKING SKELETON (DISCUSS D12). The predecessor needed one to swap the lane
# enum→data foundation under every board read/write. That foundation shipped and
# is guarded by a check-arch rule; every scenario here is UI plus one write port
# on top of it. There is deliberately ZERO @walking_skeleton tag in this file —
# its absence is a decision, not an omission.
#
# ORACLE DISCIPLINE (the house traps this file refuses):
#  - Lane expectations read lane rows BACK FROM THE DATABASE (slug, label,
#    position). The steps module holds NO static expected-lane list: one would go
#    green over the exact static-list consumers the check-arch rule exists to
#    forbid.
#  - The rename oracle asserts IDENTITY IS UNTOUCHED from the store, not from the
#    rendered page: `slug`, `position` and every `issues.state` are compared
#    before and after. A DOM-only assertion would pass over a rename that also
#    rewrote issue states.
#  - The insert oracle asserts CONTIGUITY explicitly (positions 0..n-1, unique).
#    Postgres does not enforce contiguity — only uniqueness — so a gap would be
#    invisible to the schema and merely cosmetic to `ORDER BY position`. If this
#    suite does not assert it, nothing does.
#  - Mutating scenarios snapshot the full board universe first (lane rows; every
#    issue's (lane, position); change-event and outbox counts) and assert the
#    declared delta fail-closed. A rename and an insert must each write ZERO
#    issue rows and ZERO change events — asserted, never assumed.
#  - Refusals are compared BYTE-IDENTICAL to a never-existed path, both verbs
#    (the non-enumerability idiom). An unrecognised insert `{side}` must be
#    indistinguishable from an unknown lane — never a 400.
#  - The zero-laneless guard query runs after every mutating scenario.
#
# THREE SCENARIOS ARRIVED IN DESIGN, NOT DISCUSS (feature-delta DD11/DD5), each
# because the D8 spike found a failure mode the acceptance criteria did not name:
#  1. "Two operators inserting at the same anchor both land" — the unguarded
#     path hands the loser a raw Postgres duplicate-key error. Measured.
#  2. "The menu survives the board refreshing underneath it" — a stored menu
#     handle is left detached by the out-of-band #board-columns swap, and Escape
#     would then no-op with a menu on screen (ADR-BOARD-LANE-005 rule 2).
#  3. "Escape peels one layer at a time" — the @layered scenario's fourth arm.
#
# CSRF HONESTY (inherited from fix-comment-delete-csrf): the HTTP lane injects
# the token, which is exactly how a real-browser 403 stayed hidden once before.
# The tokenless-refusal scenarios below pin the HTTP contract, and the browser
# lane drives a REAL confirm through htmx so a missing `_csrf` cannot pass twice.
#
# Grounding SSOT: docs/feature/board-lane-overflow-menu/feature-delta.md
# (DISCUSS D1-D14, US-BLO-01..03; DESIGN DD1-DD12), design/*.md, and
# docs/product/architecture/adr-board-lane-003/-004/-005.md.

@blo
Feature: Shaping a board's lanes from one unobtrusive menu

  Background:
    Given Priya is a Backend team member shaping her own boards

  # ========================================================================
  # US-BLO-01 — the menu replaces the armed × (slice 01)
  # ========================================================================

  @us-blo-01 @driving_port @real-io
  Scenario: The column header offers a menu, not an armed delete
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, In-Progress and Done
    When Priya views the "Homelab Ops" board
    Then every column header carries one lane menu trigger
    And no column header carries a lane delete control

  @us-blo-01 @driving_port
  Scenario: The menu lists exactly the four lane operations
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, In-Progress and Done
    When Priya views the "Homelab Ops" board
    Then the In-Progress column's menu offers exactly Edit list, Insert list before, Insert list after and Delete list

  @us-blo-01 @needs-browser @driving_port @real-io
  Scenario: Delete list reaches the shipped dialog unchanged
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, In-Progress and Done
    And OPS-3 and OPS-7 sit in In-Progress
    When Priya opens the In-Progress menu and chooses Delete list
    Then the delete-lane dialog opens naming In-Progress and its live count of 2
    And the dialog offers both the move fate and the permanent delete fate

  @us-blo-01 @needs-browser @edge @real-io
  Scenario: Escape closes the menu and returns focus, changing nothing
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, In-Progress and Done
    And Priya has opened the In-Progress column's menu
    When Priya presses Escape
    Then the menu is closed and focus has returned to the In-Progress menu trigger
    And the board renders exactly as it did before the menu was opened

  @us-blo-01 @needs-browser @edge
  Scenario: The menu is reachable and operable without a pointer
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, In-Progress and Done
    When Priya reaches the In-Progress menu trigger by keyboard and activates it
    Then the menu is open and each of its four items can be reached by keyboard in listed order

  @us-blo-01 @error @security
  Scenario: A non-member reaching the lane routes gets the uniform not-found
    Given Marco is signed in and is not a member of team Backend
    And "Homelab Ops" (OPS) is a board with lanes Backlog, In-Progress and Done
    When Marco requests every lane route for In-Progress on "Homelab Ops" directly
    Then each answer is byte-identical to a never-existed path, on both verbs
    And the "Homelab Ops" lane set is unchanged

  # ========================================================================
  # US-BLO-02 — Edit list: rename the label, touch nothing else (slice 02)
  # ========================================================================

  @us-blo-02 @driving_port @real-io
  Scenario: Renaming a lane changes the header and nothing else
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, In-Progress and Done
    And OPS-3 and OPS-7 sit in In-Progress top to bottom
    When Priya renames the In-Progress lane to "Doing"
    Then the column header reads Doing
    And OPS-3 and OPS-7 sit in that same column at the same positions
    And no issue row and no change event was written

  @us-blo-02 @edge @real-io
  Scenario: A rename never touches lane identity
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, In-Progress and Done
    And OPS-3 sits in In-Progress
    And the In-Progress lane has been renamed to "Doing"
    When Priya drags OPS-3 within the board and a machine client moves OPS-7 to the renamed lane
    Then both succeed against the lane slug in_progress
    And every lane slug, every lane position and every issue key is unchanged by the rename

  @us-blo-02
  Scenario: The edit dialog opens showing the lane's current name
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, In-Progress and Done
    And the In-Progress lane has been renamed to "Doing"
    When Priya opens the edit dialog for that lane
    Then the dialog's name field contains Doing
    And the dialog carries the board's matching token and a declarative close trigger

  @us-blo-02 @error
  Scenario: An empty or over-long lane name is refused inline
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, In-Progress and Done
    When Priya submits a rename of In-Progress to each of an empty name, a blank name and a 65-character name
    Then each is refused with a reason rendered into the dialog's error slot
    And the In-Progress lane still carries its original label

  @us-blo-02 @error @security
  Scenario: A rename is refused without the board's token and accepted with it
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, In-Progress and Done
    When a rename of In-Progress is submitted without the board's matching token, and then with it
    Then the tokenless rename was refused before the handler ran
    And the same rename carrying the token is accepted and takes effect

  @us-blo-02 @edge
  Scenario: Two lanes may carry the same label because labels are not identity
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, In-Progress and Done
    When Priya renames In-Progress to "Doing" and then renames Backlog to "Doing"
    Then both renames succeed and two columns read Doing
    And the two lanes still carry their distinct slugs

  # ========================================================================
  # US-BLO-03 — Insert list before / after (slice 03)
  # ========================================================================

  @us-blo-03 @driving_port @real-io
  Scenario: Inserting before a lane places the new lane immediately to its left
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, In-Progress and Done
    And OPS-3 sits in In-Progress and OPS-9 sits in Done
    When Priya inserts a lane named "Staging" before In-Progress
    Then the board's lanes read Backlog, Staging, In-Progress, Done in that order
    And the Staging column is empty and no existing card has moved
    And the lane positions are contiguous from zero and unique
    And no issue row and no change event was written

  @us-blo-03 @edge @real-io
  Scenario: Inserting after the last lane appends at the far right
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, In-Progress and Done
    When Priya inserts a lane named "Archive Box" after Done
    Then the board's lanes read Backlog, In-Progress, Done, Archive Box in that order
    And the lane positions are contiguous from zero and unique
    And a newly filed issue still lands in the leftmost lane

  @us-blo-03 @driving_port @real-io
  Scenario: An inserted lane is a fully working lane immediately
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, In-Progress and Done
    And OPS-3 sits in In-Progress
    And Priya has inserted a lane named "Staging" before In-Progress
    When Priya moves OPS-3 into Staging and opens its edit dialog
    Then the move succeeds and Staging appears among the dialog's Status options
    And a machine client may move an issue to the Staging lane's slug
    And every issue still has a lane its board renders

  @us-blo-03 @error
  Scenario: A name whose slug collides with an existing lane is refused inline
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, In-Progress and Done
    When Priya tries to insert a lane named "Done" before In-Progress
    Then the refusal names the conflict and renders into the dialog's error slot
    And the "Homelab Ops" lane set is unchanged

  @us-blo-03 @error @edge
  Scenario: A name with no usable characters is refused inline
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, In-Progress and Done
    When Priya tries to insert a lane named each of "...", "!!!" and a blank name
    Then each is refused with a reason asking for letters or numbers
    And the "Homelab Ops" lane set is unchanged

  @us-blo-03 @edge
  Scenario: A lane name that cannot start a slug still becomes a working lane
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, In-Progress and Done
    And OPS-3 sits in In-Progress
    When Priya inserts a lane named "2024 Review" after Done
    Then the column header reads "2024 Review"
    And the lane's slug satisfies the lane slug rule
    And a machine client may move an issue to that lane's slug

  @us-blo-03 @error @security
  Scenario: An unrecognised insert side is indistinguishable from an unknown lane
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, In-Progress and Done
    When Priya requests an insert dialog with a side that is neither before nor after
    Then the answer is byte-identical to a never-existed path
    And the "Homelab Ops" lane set is unchanged

  @us-blo-03 @error @security
  Scenario: A non-member cannot insert a lane
    Given Marco is signed in and is not a member of team Backend
    And "Homelab Ops" (OPS) is a board with lanes Backlog, In-Progress and Done
    When Marco sends the insert confirm for "Homelab Ops" directly
    Then the answer is byte-identical to a never-existed path, on both verbs
    And the "Homelab Ops" lane set is unchanged

  # ========================================================================
  # Scenarios that arrived in DESIGN — each pins a measured failure mode
  # (feature-delta DD11/DD5; adr-board-lane-003/-005)
  # ========================================================================

  @us-blo-03 @concurrency @real-io @adapter-integration
  Scenario: Two operators inserting at the same anchor both land
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, In-Progress and Done
    When two operators each insert a lane before Done at the same moment
    Then both lanes exist, neither operator saw a database error
    And the lane positions are contiguous from zero and unique
    And no issue row and no change event was written

  @us-blo-01 @needs-browser @layered @edge @real-io
  Scenario: The menu survives the board refreshing underneath it
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, In-Progress and Done
    And Priya has opened the In-Progress column's menu
    When the board's columns are refreshed out of band beneath the open menu
    And Priya presses Escape
    Then Escape is not a silent no-op and no menu is left on screen

  # ---------------------------------------------------------------------
  # REGRESSION — fix-lane-menu-clipped-mobile (2026-09-03)
  #
  # The menu opened but its items were UNREACHABLE on a phone. `.board` takes
  # `overflow-x: auto` below the 480px breakpoint (pwa-mobile-rendering), and an
  # element with non-visible overflow CLIPS its absolutely-positioned
  # descendants — so the menu was painted into a box the operator could not
  # touch. Measured at desktop width the identical menu IS reachable, which is
  # exactly why every existing browser scenario missed it: they all run wide.
  #
  # The oracle is a HIT TEST on the LAST item, not a visibility check. A clipped
  # menu still reports as "displayed" and still has a bounding rect; what it does
  # not have is a point the operator can actually hit. Asserting `is_displayed`
  # here would go green over the bug.
  # ---------------------------------------------------------------------
  @us-blo-01 @needs-browser @regression @mobile @edge @real-io
  Scenario: Every menu item is reachable on a phone-sized screen
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, In-Progress and Done
    And Priya is holding a phone
    When Priya opens the In-Progress column's menu on the phone
    Then every one of the six items can be touched, including the last

  @us-blo-01 @needs-browser @regression @mobile @edge
  Scenario: The menu trigger is visible at rest and big enough to touch
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, In-Progress and Done
    And Priya is holding a phone
    When Priya views the board on the phone without touching anything
    Then each menu trigger carries a visible edge before it is hovered or focused
    And each menu trigger is at least 44 by 44
    And no menu trigger overlaps the first card in its column

  @us-blo-01 @needs-browser @layered @edge
  Scenario: Escape peels one layer at a time with a menu open
    Given "Homelab Ops" (OPS) is a board with lanes Backlog, In-Progress and Done
    And Priya has opened the keyboard help overlay over an open lane menu
    When Priya presses Escape once
    Then only the help overlay has closed and the lane menu is still open
    And a second Escape closes the lane menu and leaves the board alone
