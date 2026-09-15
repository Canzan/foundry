# Feature: card-drag-drop-feedback — the board's card drag keeps working after the
# board refreshes itself in place, shows which lane and which slot will take the
# card before Priya lets go, and keeps every lane's "No issues yet" line truthful,
# while anything dragged in from outside the page does nothing at all.
#
# AUTHORED BY DISTILL (ADR-025). Every scenario is scaffolded RED as `@pending`
# and DELIVER un-pends them slice by slice (01 -> 04), never re-authoring one.
# No production seam is scaffolded: DESIGN DDD-10 keeps the server untouched, and
# the change lives in board-dnd.js, the stylesheet and partials/board_columns.html,
# which DELIVER owns.
#
# NO WALKING SKELETON (DISCUSS D1). The card drag, the positional persist, the
# in-place #board-columns refresh and the browser lane are all shipped. There is
# deliberately no @walking_skeleton tag in this file.
#
# ORACLE DISCIPLINE (the traps this file refuses; DESIGN DDD-8):
#  - NO RELOAD between an in-place refresh and the drag (D2). The step module
#    stamps window.__cdfMark when the board opens and every drag asserts it is
#    still there. "after a reload" only ever appears as an ORACLE, after the drag.
#  - A refresh is PROVEN: the Given waits until #board-columns is a different
#    node. A menu click that silently did nothing would leave the old, still-wired
#    lanes on screen and the regression would pass over them.
#  - "accepts the drag" is the synthetic dragover being defaultPrevented; the
#    drag kit fires drop only on a claimed target, as a real browser does.
#  - "no placeholder" means NOT DISPLAYED (computed style), not absent from the
#    DOM (DESIGN DDD-5): the server renders it in every lane.
#  - Negative oracles carry a positive control ("… straight afterwards does light
#    it"), so they cannot pass only because the feedback does not exist yet.
#  - "returns to its slot" first proves the move was sent and refused.
#  - Feel (flicker, oscillation, a real Finder drop) is each slice's manual
#    dogfood check (D12); synthetic events prove wiring only.
#
# THE SHIPPED CARD-DRAG SCENARIOS ARE NOT TOUCHED (KPI 8, AC-1.7):
# board-lane-reorder.feature "Dragging a card still moves the card and never the
# lane", and keyboard-shortcut-bindings' AUTH-2 mouse drag.
#
# Grounding SSOT: docs/feature/card-drag-drop-feedback/feature-delta.md (DISCUSS
# D1-D16, US-CDF-01..04, KPIs 1-8; DESIGN DDD-1..12 incl. the DDD-9 M1-M9 named
# faults; DEVOPS environment matrix) and adr-board-card-001/002/003.

@cdf
Feature: Card drag-and-drop feedback
  Priya drags cards on her boards many times a session. A drag must keep working
  after the board refreshes in place, show which lane and which slot will take
  the card before she lets go, and leave every lane's "No issues yet" line
  truthful, while anything dragged in from outside the page does nothing.

  Background:
    Given Priya works two boards: Identity Platform with lanes Backlog, In-Progress and Done, and Homelab Ops with lanes Backlog, Staging, In-Progress and Done
    And Identity Platform's Backlog holds AUTH-41, AUTH-42 and AUTH-43, its In-Progress holds AUTH-3, AUTH-12 and AUTH-19 in that order, and its Done holds AUTH-7
    And Homelab Ops's Backlog holds OPS-3, its Staging is empty, its In-Progress holds only OPS-7, and its Done holds OPS-9

  # ========================================================================
  # US-CDF-01 — drops keep working after the board refreshes in place (slice 01)
  # ========================================================================

  # Written first. RED on HEAD because the dragover on the lane now on screen is
  # not defaultPrevented and no request is sent (AC-1.2).
  @us-cdf-01 @needs-browser @driving_port @real-io @kpi
  Scenario: A card can still be dropped after the board rearranges itself in place
    Given Homelab Ops is open in a browser
    And Priya has deleted OPS-9 from its popup without reloading
    And Priya has moved Staging right from its lane menu without reloading
    When Priya drags OPS-7 from In-Progress over Done
    Then Done accepts the drag
    And after she drops it, OPS-7 is in Done
    And after a reload OPS-7 is still in Done

  @us-cdf-01 @needs-browser @driving_port @real-io @kpi
  Scenario Outline: Every way the board refreshes in place leaves every lane accepting drops
    Given the Identity Platform board is open in a browser
    And Priya <refresh_in_place> without reloading
    When Priya drags AUTH-41 into <destination> and drops it
    Then AUTH-41 is in <destination>
    And after a reload AUTH-41 is still in <destination>

    Examples:
      | refresh_in_place                                     | destination |
      | deletes AUTH-42 from its popup                       | In-Progress |
      | renames Done to "Shipped" from its lane menu         | Shipped     |
      | inserts a list "Review" after In-Progress            | Review      |
      | deletes the Done list, moving its cards into Backlog | In-Progress |

  # A GUARD, NOT A REFRESH. Dragging a lane header moves the existing lane nodes
  # and replaces nothing (board-lane-dnd.js pointerup never calls applyBoard), so
  # this proves card drops also hold when the board is merely rearranged. Green on
  # HEAD by design; kept as a robustness guard (DISTILL Upstream Issue #1).
  @us-cdf-01 @needs-browser @driving_port @real-io @kpi
  Scenario: Rearranging the lanes by dragging a header leaves every lane accepting drops
    Given the Identity Platform board is open in a browser
    And Priya drags the Done header to the left of In-Progress without reloading
    When Priya drags AUTH-41 into Done and drops it
    Then AUTH-41 is in Done
    And after a reload AUTH-41 is still in Done

  @us-cdf-01 @needs-browser @real-io
  Scenario: A card dropped at an exact slot after a refresh keeps that slot
    Given the Identity Platform board is open in a browser
    And Priya has deleted AUTH-42 from its popup without reloading
    When Priya drags AUTH-41 between AUTH-3 and AUTH-12 and drops it
    Then the move request names AUTH-3 as the card above
    And after a reload In-Progress reads AUTH-3, AUTH-41, AUTH-12, AUTH-19

  @us-cdf-01 @needs-browser @real-io @kpi
  Scenario: A freshly loaded board drags exactly as it did before
    Given the Identity Platform board has just been loaded
    When Priya drags AUTH-41 to the top of In-Progress and drops it
    Then the move request carries the destination lane and names no card above
    And after a reload AUTH-41 is first in In-Progress

  @us-cdf-01 @needs-browser @error @kpi
  Scenario Outline: Something dragged in from outside the page is swallowed by the board
    Given the Identity Platform board is open in a browser, <board_state>
    When <something> is dragged in from outside the page and dropped on <where>
    Then no card moves and no move request is sent
    And the tab still shows the Identity Platform board
    And the board shows that it will not take the drop

    Examples:
      | board_state                                | something                         | where                                   |
      | freshly loaded                             | a file "keys.png"                 | Done                                    |
      | freshly loaded                             | a file "keys.png"                 | the gap between Backlog and In-Progress |
      | after Priya deleted AUTH-42 from its popup | a file "keys.png"                 | Done                                    |
      | after Priya deleted AUTH-42 from its popup | a text selection from another app | the gap between Backlog and In-Progress |
      | freshly loaded                             | a text selection from another app | the empty space below AUTH-7 in Done    |

  # Modelled as a drag with no dragstart on this page, carrying the card's key as
  # plain text exactly as a card drag from another foundry tab does.
  @us-cdf-01 @needs-browser @error @kpi
  Scenario: A card dragged in from another tab moves nothing in this one
    Given the Identity Platform board is open in two tabs
    When Priya drags AUTH-41 out of the second tab and drops it on Done in the first
    Then no card moves in either tab and no move request is sent

  @us-cdf-01 @needs-browser @error
  Scenario: A cancelled card drag is not mistaken for the next drag
    Given the Identity Platform board is open in a browser
    And Priya started dragging AUTH-41 and cancelled it with Escape
    When a file "keys.png" is then dragged in from outside the page and dropped on In-Progress
    Then no card moves and no move request is sent
    And AUTH-41 is still in its slot in Backlog

  @us-cdf-01 @needs-browser @error @real-io
  Scenario: A drop the server refuses after a refresh puts the card back
    Given the Identity Platform board is open in a browser
    And Priya has deleted AUTH-42 from its popup without reloading
    And another operator deleted AUTH-43 after Priya's board loaded
    When Priya drags AUTH-43 into Done and drops it
    Then AUTH-43 returns to its original slot in Backlog

  # ========================================================================
  # US-CDF-02 — the lane under a dragged card activates (slice 02)
  # ========================================================================

  @us-cdf-02 @needs-browser @kpi
  Scenario: The lane under a dragged card lights up, and only that lane
    Given Homelab Ops is open in a browser
    When Priya drags OPS-3 over Done
    Then Done is shown as activated
    And no other lane is shown as activated

  @us-cdf-02 @needs-browser
  Scenario: The highlight follows the card from lane to lane
    Given Priya is dragging OPS-3 over Done
    When she moves it over Staging
    Then Staging is shown as activated and Done is not

  # Read BETWEEN the dragleave and the next dragover (DESIGN DDD-8f): that is the
  # only moment a clear-on-every-leave implementation shows its flicker.
  @us-cdf-02 @needs-browser @edge
  Scenario: A lane stays lit while the card passes over the cards inside it
    Given Priya is dragging OPS-3 over Done
    When the pointer passes over OPS-9 inside Done
    Then Done is still shown as activated

  @us-cdf-02 @needs-browser @edge
  Scenario Outline: Nothing is lit while the card is over no lane
    Given Priya is dragging OPS-3 over Done
    When she <leaving>
    Then no lane is shown as activated

    Examples:
      | leaving                              |
      | moves it over the page header        |
      | carries it out of the browser window |

  @us-cdf-02 @needs-browser @error @kpi
  Scenario Outline: Every way a drag ends leaves no lane lit
    Given Priya is dragging OPS-3 over Done
    When <drag_end>
    Then no lane is shown as activated

    Examples:
      | drag_end                                             |
      | she drops it on Done                                 |
      | she presses Escape                                   |
      | she releases it over the page header                 |
      | she drops it on Done and the server refuses the move |

  @us-cdf-02 @needs-browser @error @kpi
  Scenario: Something dragged in from outside the page lights nothing
    Given Homelab Ops is open in a browser
    When a file "keys.png" is dragged in from outside the page over Done
    Then no lane is shown as activated
    And a card dragged over Done straight afterwards does light it

  @us-cdf-02 @needs-browser @real-io
  Scenario: Lanes still light up after the board refreshes in place
    Given Homelab Ops is open in a browser
    And Priya has inserted a list "Review" after In-Progress from its lane menu, without reloading
    When Priya drags OPS-3 over Review
    Then Review is shown as activated

  @us-cdf-02 @needs-browser
  Scenario Outline: The activated lane is legible in both palettes and moves nothing
    Given the device is set to the <palette> palette
    And Homelab Ops is open in a browser
    When Priya drags OPS-3 over Done
    Then the activated lane's boundary measures at least 3:1 against the page
    And no card and no column has moved or changed size

    Examples:
      | palette |
      | light   |
      | dark    |

  # ========================================================================
  # US-CDF-03 — a marker shows exactly where the card will land (slice 03)
  # ========================================================================

  @us-cdf-03 @needs-browser
  Scenario: A marker shows the slot between two cards
    Given the Identity Platform board is open in a browser
    When Priya drags AUTH-41 over In-Progress between AUTH-3 and AUTH-12
    Then a marker shows between AUTH-3 and AUTH-12
    And it is the only marker on the board

  @us-cdf-03 @needs-browser @driving_port @real-io @kpi
  Scenario: The card lands exactly where the marker showed, and a reload agrees
    Given the Identity Platform board is open in a browser
    And the marker shows between AUTH-3 and AUTH-12 while Priya drags AUTH-41
    When she drops it
    Then In-Progress reads AUTH-3, AUTH-41, AUTH-12, AUTH-19
    And the move request names AUTH-3 as the card above
    And after a reload In-Progress reads AUTH-3, AUTH-41, AUTH-12, AUTH-19

  @us-cdf-03 @needs-browser @real-io @edge
  Scenario Outline: The marker reaches both ends of a lane
    Given the Identity Platform board is open in a browser
    When Priya drags AUTH-41 over In-Progress <where>
    Then the marker shows <marker>
    And after she drops it and reloads, AUTH-41 is <position> in In-Progress

    Examples:
      | where                      | marker        | position |
      | above the middle of AUTH-3 | above AUTH-3  | first    |
      | below AUTH-19              | below AUTH-19 | last     |

  @us-cdf-03 @needs-browser @real-io @edge
  Scenario: Reordering inside a lane never offers the card's own slot
    Given the Identity Platform board is open in a browser
    When Priya drags AUTH-19 up from its own slot to between AUTH-3 and AUTH-12
    Then the marker shows between AUTH-3 and AUTH-12
    And the marker was never shown above AUTH-19 itself while she dragged it
    And after she drops it and reloads, In-Progress reads AUTH-3, AUTH-19, AUTH-12

  @us-cdf-03 @needs-browser @edge
  Scenario: A still pointer keeps the marker in one place
    Given the Identity Platform board is open in a browser
    And the marker shows between AUTH-3 and AUTH-12 while Priya drags AUTH-41
    When the drag reports the same pointer position several more times
    Then the marker still shows between AUTH-3 and AUTH-12

  @us-cdf-03 @needs-browser
  Scenario: The marker moves with the card to another lane
    Given the Identity Platform board is open in a browser
    And the marker shows between AUTH-3 and AUTH-12 while Priya drags AUTH-41
    When she moves the card over Done below AUTH-7
    Then the only marker on the board shows below AUTH-7

  @us-cdf-03 @needs-browser @error @kpi
  Scenario Outline: The marker never outlives the drag
    Given the Identity Platform board is open in a browser
    And the marker shows between AUTH-3 and AUTH-12 while Priya drags AUTH-41
    When <drag_end>
    Then no marker shows anywhere on the board
    And AUTH-41 is <auth_41_ends_up>

    Examples:
      | drag_end                                     | auth_41_ends_up             |
      | she drops it                                 | between AUTH-3 and AUTH-12  |
      | she presses Escape                           | back in its slot in Backlog |
      | she releases it over the page header         | back in its slot in Backlog |
      | she drops it and the server refuses the move | back in its slot in Backlog |

  @us-cdf-03 @needs-browser @error @kpi
  Scenario: Something dragged in from outside the page shows no marker
    Given the Identity Platform board is open in a browser
    When a file "keys.png" is dragged in from outside the page over In-Progress
    Then no marker shows anywhere on the board
    And a card dragged over In-Progress straight afterwards does show a marker

  @us-cdf-03 @needs-browser @real-io
  Scenario: The marker works on a board that refreshed in place
    Given the Identity Platform board is open in a browser
    And Priya has deleted AUTH-42 from its popup without reloading
    When Priya drags AUTH-41 over In-Progress between AUTH-12 and AUTH-19
    Then a marker shows between AUTH-12 and AUTH-19
    And after she drops it and reloads, In-Progress reads AUTH-3, AUTH-12, AUTH-41, AUTH-19

  @us-cdf-03 @needs-browser
  Scenario Outline: The marker is legible in both palettes
    Given the device is set to the <palette> palette
    And the Identity Platform board is open in a browser
    When Priya drags AUTH-41 over In-Progress between AUTH-3 and AUTH-12
    Then the marker measures at least 3:1 against the lane behind it

    Examples:
      | palette |
      | light   |
      | dark    |

  # ========================================================================
  # US-CDF-04 — empty lanes read truthfully (slice 04)
  # "The placeholder" is the server's own "No issues yet — press c to file the
  # first one." line, and "shows" means displayed. A reload is the oracle.
  # ========================================================================

  @us-cdf-04 @needs-browser @real-io
  Scenario: Dropping into an empty lane removes its placeholder
    Given Homelab Ops is open in a browser and Staging shows the placeholder
    When Priya drags OPS-3 from Backlog into Staging and drops it
    Then Staging shows OPS-3 and no placeholder
    And after a reload Staging looks exactly the same

  @us-cdf-04 @needs-browser @real-io
  Scenario: A lane emptied by a drag shows the placeholder
    Given Homelab Ops is open in a browser
    When Priya drags OPS-7, the only card in In-Progress, into Done and drops it
    Then In-Progress shows the placeholder, with the same words and markup a freshly loaded empty lane shows
    And after a reload In-Progress looks exactly the same

  @us-cdf-04 @needs-browser @driving_port @real-io @kpi
  Scenario: One drag updates both lanes at once
    Given Homelab Ops is open in a browser
    When Priya drags OPS-7 from In-Progress into the empty Staging lane and drops it
    Then Staging shows OPS-7 and no placeholder
    And In-Progress shows the placeholder
    And after a reload both lanes look exactly the same

  @us-cdf-04 @needs-browser @error @real-io
  Scenario: A refused drop puts both lanes back as they were
    Given Homelab Ops is open in a browser
    And another operator deleted OPS-7 after Priya's board loaded
    When Priya drags OPS-7 from In-Progress into Staging and drops it
    Then OPS-7 is back in In-Progress and In-Progress shows no placeholder
    And Staging shows its placeholder again

  @us-cdf-04 @needs-browser @error
  Scenario Outline: Only a drop changes a placeholder
    Given Homelab Ops is open in a browser and Staging shows the placeholder
    When <happening>
    Then Staging still shows its placeholder, unchanged
    And once Priya drops OPS-3 on Staging its placeholder is no longer displayed

    Examples:
      | happening                                                                   |
      | Priya drags OPS-3 over Staging and presses Escape                           |
      | Priya drags OPS-3 over Staging and releases it over the page header         |
      | a file "keys.png" is dragged in from outside the page and dropped on Staging |

  @us-cdf-04 @needs-browser @real-io @kpi
  Scenario: A lane emptied by a delete in another tab shows the placeholder
    Given Homelab Ops is open in two tabs
    And In-Progress in the second tab does not show the placeholder while it holds OPS-7
    When Priya deletes OPS-7, the only card in In-Progress, from its popup in the first tab
    Then the second tab drops OPS-7 without a reload
    And In-Progress in the second tab shows the placeholder
    And every other lane in the second tab is unchanged

  @us-cdf-04 @needs-browser @real-io @edge
  Scenario: A delete in another tab that leaves cards behind adds no placeholder
    Given Homelab Ops's Done also holds OPS-11
    And Homelab Ops is open in two tabs
    When Priya deletes OPS-9 from its popup in the first tab
    Then Done in the second tab shows OPS-11 and no placeholder

  @us-cdf-04 @needs-browser @real-io
  Scenario: Placeholders stay truthful on a board that refreshed in place
    Given Homelab Ops is open in a browser
    And Priya has moved Staging right from its lane menu without reloading
    When Priya drags OPS-7 from In-Progress into Staging and drops it
    Then Staging shows OPS-7 and no placeholder
    And In-Progress shows the placeholder
