# Feature: card-pointer-drag — a card on the board drags on Pointer Events, so
# Priya can move it with a finger or a pen as well as with the mouse. A press
# and hold lifts it on a phone, a quick swipe still scrolls, a tap still opens
# the card, and a mouse drag feels exactly as it did.
#
# AUTHORED BY DISTILL (ADR-025). Every scenario is scaffolded RED as `@pending`
# and DELIVER un-pends them slice by slice (01 -> 03), never re-authoring one.
# No production seam is scaffolded: DESIGN DDD-16/21 put the whole change in
# board-dnd.js, keyboard.js and the stylesheet, which DELIVER owns.
#
# NO WALKING SKELETON (DISCUSS D8). The move request, ranking, revert, lane
# light, slot marker and placeholders are all shipped (card-drag-drop-feedback).
#
# THE SHIPPED CARD-DRAG GHERKIN IS NOT TOUCHED (D4). The ~37 shipped scenarios
# are this feature's regression proof; their DRIVER is re-pointed in DELIVER
# slice 01 (feature-delta `## Wave: DISTILL`, "Driver re-pointing plan"). This
# file adds only the NEW behaviour.
#
# DRIVER (DESIGN DDD-12): every card gesture here is TRUSTED browser input,
# W3C WebDriver Actions for the mouse, the pen and the keyboard, and Chrome's
# own touch input (Input.dispatchTouchEvent through the driver) for a finger,
# because the driver's W3C touch cannot continue a gesture across calls
# (measured, see the feature-delta). Only a drag that comes from OUTSIDE the
# page (a file) is a synthetic DragEvent: nothing on this page has a pointer
# for it.
#
# ORACLE DISCIPLINE (the traps this file refuses):
#  - The page event recorder is armed before every gesture. A "nothing lifted"
#    oracle first proves trusted pointer input ARRIVED; otherwise a driver that
#    delivered nothing would pass it.
#  - "No carried card remains" / "no lane is lit" / "no marker shows" are only
#    asserted after the scenario has PROVEN the carried card, the lit lane and
#    the marker existed (the board-lane-reorder drop-indicator lesson).
#  - "Back in its exact slot" compares the card's neighbours with the ones read
#    at the lift, not merely its lane.
#  - A negative oracle carries a positive control in the same scenario ("...
#    straight afterwards does lift it"), so it cannot pass only because the
#    pointer drag does not exist yet.
#  - Order oracles read the whole lane, and every drop that should persist is
#    re-read after a reload.
#  - "Lifted" is DESIGN's observable (DDD-7, DDD-17): the page marks a drag in
#    flight, the origin card stays in its slot marked lifted, and a carried copy
#    of the card is visible under the pointer.
#  - Feel (hold length, iOS callout, real momentum scroll) is each slice's
#    real-device dogfood (D19, DDD-13); WebDriver proves wiring.
#
# Grounding SSOT: docs/feature/card-pointer-drag/feature-delta.md (DISCUSS
# D1-D20, US-CPD-01..03; DESIGN DDD-1..22), spike/findings.md, and
# adr-board-card-004-pointer-events-card-drag.md.

@cpd
Feature: Card drag on any pointer
  Priya drags cards at her desk with the mouse and, away from it, on her phone.
  A card must lift, travel and land exactly where she means on either, while a
  swipe still scrolls the board, a tap or click still opens the card, and an
  interrupted drag changes nothing.

  Background:
    Given Priya keeps two boards for pointer dragging: Identity Platform with lanes Backlog, In-Progress and Done, and Homelab Ops with lanes Backlog, Staging, In-Progress and Done
    And on Identity Platform, Backlog holds AUTH-41, AUTH-42 and AUTH-43, In-Progress holds AUTH-3, AUTH-12 and AUTH-19 in that order, and Done holds AUTH-7
    And on Homelab Ops, Backlog holds OPS-3, Staging is empty, In-Progress holds OPS-7 and Done holds OPS-9

  # ========================================================================
  # Slice 01 — US-CPD-01: the mouse drag, unchanged to the hand, on Pointer
  # Events (desk, 1280x900)
  # ========================================================================

  # The probe scenario (OQ-9): RED on HEAD at the drag, with the recorder
  # showing trusted mouse input DID reach the page and no card lifted.
  @us-cpd-01 @needs-browser @pending @driving_port @real-io @kpi
  Scenario: A card dragged with the mouse lands exactly where it is released
    Given the Identity Platform board is open at the desk
    When Priya drags AUTH-41 with the mouse between AUTH-3 and AUTH-12 and releases it
    Then In-Progress now reads AUTH-3, AUTH-41, AUTH-12, AUTH-19
    And the move request is exactly the one the board has always sent, naming AUTH-3 as the card above
    And no edit dialog opens
    And a reload shows In-Progress as AUTH-3, AUTH-41, AUTH-12, AUTH-19

  @us-cpd-01 @needs-browser @pending @driving_port @real-io
  Scenario: A card dropped at the top of a lane names no card above it
    Given the Identity Platform board is open at the desk
    When Priya drags AUTH-41 with the mouse to the top of In-Progress and releases it
    Then In-Progress now reads AUTH-41, AUTH-3, AUTH-12, AUTH-19
    And the move request is exactly the one the board has always sent, naming no card above
    And a reload shows In-Progress as AUTH-41, AUTH-3, AUTH-12, AUTH-19

  @us-cpd-01 @needs-browser @pending
  Scenario: The card being dragged is carried under the mouse without hiding the lane beneath it
    Given the Identity Platform board is open at the desk
    And Priya has lifted AUTH-41 with the mouse
    When she carries it over In-Progress between AUTH-3 and AUTH-12
    Then a carried AUTH-41 has followed the pointer
    And In-Progress is the one lane lit up
    And exactly one marker shows, between AUTH-3 and AUTH-12
    And AUTH-41 still shows in its own slot in Backlog, marked as lifted
    And once she releases it no carried card remains

  # Spike Q6: a drag released on the card it started from fires a click on it.
  @us-cpd-01 @needs-browser @pending @edge
  Scenario: A drag released back over its own card does not open it
    Given the Identity Platform board is open at the desk
    And Priya has lifted AUTH-41 with the mouse
    When she releases it back over AUTH-41
    Then no edit dialog opens
    And Backlog now reads AUTH-41, AUTH-42, AUTH-43
    And no carried card remains

  # The click guard resets on the next press (DDD-6). A guard that waited for
  # a click to consume would eat this one.
  @us-cpd-01 @needs-browser @pending
  Scenario: Right after a drag, a press that barely moves is still a click
    Given the Identity Platform board is open at the desk
    And Priya has just dragged AUTH-43 into Done with the mouse
    When she presses AUTH-41, moves the mouse 3 pixels and releases
    Then AUTH-41's edit dialog opens
    And no further move request is sent

  @us-cpd-01 @needs-browser @pending @error
  Scenario: Escape during a card drag puts the card back and peels only that layer
    Given the Identity Platform board is open at the desk
    And Priya is dragging AUTH-41 over Done with the mouse
    When she presses Escape on the keyboard
    Then AUTH-41 is back in its exact slot in Backlog
    And no lane is lit, no marker shows, no carried card remains and no move request is sent
    And releasing the mouse over Done afterwards moves nothing
    And pressing Escape again changes nothing on the board

  @us-cpd-01 @needs-browser @pending @error
  Scenario: Only the primary mouse button drags a card
    Given the Identity Platform board is open at the desk
    When Priya presses AUTH-41 with the right mouse button and moves it into In-Progress
    Then AUTH-41 has not lifted and Backlog now reads AUTH-41, AUTH-42, AUTH-43
    And no move request is sent for it
    And the same move with the primary button does lift AUTH-41

  # The card side of the gesture boundary (D13). The lane side is the next
  # scenario, and the shipped board-lane-reorder guard stays the standing proof.
  @us-cpd-01 @needs-browser @pending @error
  Scenario: A drag begun on a card never moves a lane
    Given Homelab Ops is open at the desk
    When Priya drags OPS-3 with the mouse from Backlog across the In-Progress header and releases it in Done
    Then Done now reads OPS-9, OPS-3
    And the lanes still read Backlog, Staging, In-Progress, Done
    And a reload shows Done as OPS-9, OPS-3

  @us-cpd-01 @needs-browser @pending @error
  Scenario: A drag begun on a lane header never lifts a card
    Given the Identity Platform board is open at the desk
    When Priya drags the Done header with the mouse to the left of In-Progress
    Then the lanes now read Backlog, Done, In-Progress
    And no card was lifted, every card is still where it was and no move request is sent
    And a card dragged straight afterwards still lifts

  # DDD-19: cards keep draggable="true" (issue-status-move.feature:49), and the
  # board declines the browser's own drag of them, which would otherwise take
  # the pointer away mid-drag (spike Q1).
  @us-cpd-01 @needs-browser @pending @error
  Scenario: A card never starts the browser's own drag, though it is still marked draggable
    Given the Identity Platform board is open at the desk
    When Priya presses AUTH-41 with the mouse and moves it 30 pixels
    Then the browser never gets to start its own drag, so the pointer stays with the board
    And AUTH-41 is still marked draggable

  @us-cpd-01 @needs-browser @pending @error
  Scenario: A file from the desktop is still swallowed right after a pointer drag
    Given the Identity Platform board is open at the desk
    And Priya has just dragged AUTH-43 into Done with the mouse
    When a file "keys.png" from the desktop is dropped on In-Progress
    Then nothing else moves and no further move request is sent
    And the board still shows Identity Platform in the same tab

  @us-cpd-01 @needs-browser @pending @real-io
  Scenario: A card can still be dragged after the board refreshes in place
    Given the Identity Platform board is open at the desk
    And Priya has deleted AUTH-42 from its popup and the board refreshed without reloading
    When Priya drags AUTH-41 with the mouse into Done and releases it
    Then Done now reads AUTH-7, AUTH-41
    And a reload shows Done as AUTH-7, AUTH-41

  # ========================================================================
  # Slice 02 — US-CPD-02: press and hold to lift on a phone (390x844 touch),
  # swipe and tap unchanged. Gated on the DDD-13 device checklist.
  # ========================================================================

  @us-cpd-02 @needs-browser @pending @mobile @driving_port @real-io @kpi
  Scenario: Holding a card lifts it and it can be dropped at an exact slot by touch
    Given the Identity Platform board is open on a phone
    And Priya has lifted AUTH-41 by holding it with a touch pointer
    When she carries it between AUTH-3 and AUTH-12 and lifts her finger
    Then In-Progress now reads AUTH-3, AUTH-41, AUTH-12, AUTH-19
    And the move request is exactly the one the board has always sent, naming AUTH-3 as the card above
    And no edit dialog opens
    And a reload shows In-Progress as AUTH-3, AUTH-41, AUTH-12, AUTH-19

  @us-cpd-02 @needs-browser @pending @mobile
  Scenario: While lifted by touch, the lane under the finger lights, the marker shows the slot and nothing scrolls
    Given the Identity Platform board is open on a phone
    And Priya has lifted AUTH-41 by holding it with a touch pointer
    When she carries it over In-Progress between AUTH-3 and AUTH-12
    Then In-Progress is the one lane lit up
    And exactly one marker shows, between AUTH-3 and AUTH-12
    And a carried AUTH-41 has followed the pointer
    And neither the board nor the page has scrolled while she carried it

  @us-cpd-02 @needs-browser @pending @mobile @error @kpi
  Scenario: A touch that moves before the hold completes scrolls the board and lifts nothing
    Given Homelab Ops is open on a phone
    When Priya puts a touch pointer on OPS-3 and swipes left before the hold completes
    Then the board has scrolled towards Done
    And OPS-3 has not lifted, no lane is lit, no marker shows and no move request is sent
    And holding OPS-3 still straight afterwards does lift it

  # A touch drag past the slop produces no click at all (spike Q6), so a click
  # guard that waits for one would eat this tap.
  @us-cpd-02 @needs-browser @pending @mobile @kpi
  Scenario: A tap on a card still opens it, even right after a touch drag
    Given the Identity Platform board is open on a phone
    And Priya has just carried AUTH-43 into In-Progress by touch
    When she taps AUTH-41
    Then AUTH-41's edit dialog opens
    And no further move request is sent

  @us-cpd-02 @needs-browser @pending @mobile @edge
  Scenario: A card lifted by touch and released where it lifted opens nothing and stays put
    Given the Identity Platform board is open on a phone
    And Priya has lifted AUTH-41 by holding it with a touch pointer
    When she lifts her finger without moving it
    Then no edit dialog opens
    And Backlog now reads AUTH-41, AUTH-42, AUTH-43
    And no carried card remains and no move request is sent

  @us-cpd-02 @needs-browser @pending @mobile @error
  Scenario: A second finger during a touch drag does not take the card
    Given the Identity Platform board is open on a phone
    And Priya has lifted AUTH-41 by holding it with a touch pointer
    And she has carried it over In-Progress between AUTH-3 and AUTH-12
    When a second finger goes down on Backlog and moves
    Then AUTH-41 is still carried and In-Progress is still the one lane lit up
    And lifting the first finger lands AUTH-41 between AUTH-3 and AUTH-12

  # The journey's `refused` path on touch: the revert is the shipped one
  # (AC-2.6), and a store-level delete announces nothing, so the board still
  # shows the card when Priya lifts it.
  @us-cpd-02 @needs-browser @pending @mobile @error @real-io
  Scenario: A touch drop the server refuses puts the card back exactly
    Given the Identity Platform board is open on a phone
    And AUTH-41 was deleted elsewhere after Priya's board loaded
    And Priya has lifted AUTH-41 by holding it with a touch pointer
    When she carries it between AUTH-3 and AUTH-12 and lifts her finger
    Then the move was sent and refused
    And AUTH-41 is back in its exact slot in Backlog
    And no lane is lit, no marker shows and no carried card remains

  @us-cpd-02 @needs-browser @pending
  Scenario: A pen lifts a card by holding, exactly as a finger does
    Given the Identity Platform board is open for a pen
    And Priya has lifted AUTH-19 by holding it with a pen
    When she carries it to the top of In-Progress and lifts the pen
    Then In-Progress now reads AUTH-19, AUTH-3, AUTH-12
    And the move request is exactly the one the board has always sent, naming no card above

  # ========================================================================
  # Slice 03 — US-CPD-03: carry a card off-screen, and never strand it
  # ========================================================================

  @us-cpd-03 @needs-browser @pending @mobile @driving_port @real-io @kpi
  Scenario: Holding a carried card at the board's edge scrolls to an off-screen lane
    Given Homelab Ops has eight lanes and is open on a phone
    And Priya has lifted OPS-3 by holding it with a touch pointer
    When she holds it at the right edge of the board until Done is in view and releases it over Done
    Then Done now reads OPS-9, OPS-3
    And OPS-3 landed exactly where the marker showed just before she let go
    And a reload shows Done as OPS-9, OPS-3

  @us-cpd-03 @needs-browser @pending @mobile @edge
  Scenario: Auto-scroll stops at the board's end
    Given Homelab Ops has eight lanes and is open on a phone
    And Priya has lifted OPS-3 by holding it with a touch pointer
    And she has carried it to the right edge until the board scrolled to its end
    When she keeps holding it at the edge
    Then the board scrolls no further and the page has not scrolled sideways
    And a marker still shows in the lane under her finger

  @us-cpd-03 @needs-browser @pending @mobile @kpi
  Scenario: Holding a carried card near the bottom reaches the end of a long lane
    Given Identity Platform's Backlog runs on from AUTH-43 through AUTH-60, twenty cards in all
    And the Identity Platform board is open on a phone
    And Priya has lifted AUTH-43 by holding it with a touch pointer
    When she holds it near the bottom of the screen until AUTH-60 is in view and lets go below it
    Then the page had scrolled down to reach AUTH-60
    And AUTH-43 landed exactly where the marker showed just before she let go
    And a reload shows AUTH-43 last in Backlog, after AUTH-60

  @us-cpd-03 @needs-browser @pending @mobile @error @kpi
  Scenario: The system taking the pointer mid-drag puts the card back and leaves nothing behind
    Given the Identity Platform board is open on a phone
    And Priya is carrying AUTH-41 over In-Progress with a touch pointer
    When an incoming call takes the touch away from Priya
    Then AUTH-41 is back in its exact slot in Backlog
    And no lane is lit, no marker shows, no carried card remains and no move request is sent

  @us-cpd-03 @needs-browser @pending @mobile @error
  Scenario: A hold the system interrupts before the card lifts leaves nothing to undo
    Given the Identity Platform board is open on a phone
    And Priya has put a touch pointer on AUTH-41 without holding it long enough to lift
    When an incoming call takes the touch away from Priya
    Then AUTH-41 has still not lifted once the hold time has passed, and nothing is lit or sent
    And holding AUTH-41 again straight afterwards does lift it

  @us-cpd-03 @needs-browser @pending @mobile @error @kpi
  Scenario: Releasing a carried card off every lane changes nothing
    Given the Identity Platform board is open on a phone
    And Priya is carrying AUTH-41 over In-Progress with a touch pointer
    When she lifts her finger over the page header
    Then AUTH-41 is back in its exact slot in Backlog
    And no lane is lit, no marker shows, no carried card remains and no move request is sent
    And no edit dialog opens

  @us-cpd-03 @needs-browser @pending
  Scenario: A mouse drag in a narrow window reaches an off-screen lane too
    Given Homelab Ops has eight lanes and is open in a narrow window at the desk
    And Priya has lifted OPS-3 with the mouse
    When she holds it at the right edge of the board until Done is in view and releases it over Done
    Then Done now reads OPS-9, OPS-3
    And OPS-3 landed exactly where the marker showed just before she let go
