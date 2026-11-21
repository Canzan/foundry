# Feature: every page the operator lands on looks like Foundry and tells her
# what it is for.
#
# ---------------------------------------------------------------------------
# WHAT THIS LANE DRIVES
# ---------------------------------------------------------------------------
# One running application, through its own public HTTP surface. The body-shape,
# header, token and issue-content readings are real GETs on the shipped routes
# against a real per-scenario Postgres (@real-io). The look readings are the
# SAME routes opened in the REAL browser the @needs-browser lane drives: at the
# harness desk viewport and at the emulated 390x844 phone device, and once with
# a device that asks for the dark palette.
#
# ---------------------------------------------------------------------------
# WHY THE SHORTCUT LIST IS ASKED FOR FOUR WAYS
# ---------------------------------------------------------------------------
# One address answers two bodies, and which one it answers is decided from what
# the asking client DECLARES about its destination. So the same address is asked
# for four ways: as a page being opened, as a script fetching, as a frame
# loading, and declaring nothing at all. Three of those four must still be given
# the bare list the overlay assigns into the page, which is why serving the
# framed page to everybody fails here rather than passing.
#
# Every one of those four answers is also read for what it tells a cache it
# depends on. A cache that is told nothing is free to hand a page body to the
# overlay, or the bare list to a browser opening the address — and that defect
# shows up only at the cache, only for the OTHER reader, which is why the header
# is read on EVERY arm and not only on the new one.
#
# ---------------------------------------------------------------------------
# HOW THE CARD IS READ
# ---------------------------------------------------------------------------
# Not by class name: by what the browser PAINTS. The card is the frame drawn
# around what she came for — the first box above it that the stylesheet gives a
# surface, a border or a shadow of its own. The reading says how wide that box
# is, where it sits between the two edges of the window, and whether the product
# name is drawn above the page's heading inside it. A class name says none of
# that, and a page can carry the class and still run edge to edge.
#
# ---------------------------------------------------------------------------
# WHAT THIS LANE CANNOT SEE
# ---------------------------------------------------------------------------
# Whether a password manager really offers to fill or to save: that lives in the
# browser profile, not in the page. Only the intent the page DECLARES for each
# field is observable here. Nor does it judge the look the way an operator does;
# it reads geometry, type, paint and text. @needs-browser is excluded from the
# default fast lane, so evidence for this feature must name the lane that ran.

@ui-bare-pages
Feature: Every page the operator lands on looks like Foundry and tells her what it is for
  Opening the keyboard shortcut list, the operator reads each key beside what it
  does on a page framed like the rest of Foundry. Signed out, every page she can
  land on shows its form in one centred card under the product name, at desk and
  at phone width, and her password manager is told what each field holds. On an
  issue's own page she reads the issue itself: its key, its title, its
  description as she wrote it, and its status in Foundry's own chip.

  Background:
    Given a workspace "Acme" exists with a member "Mei" on team "Backend"
    And a project "Sandbox" with key prefix "GEN" exists under "Backend"

  # ==========================================================================
  # The shortcut list: one address, two bodies, one cache contract
  # ==========================================================================

  @real-io @driving_port
  Scenario Outline: What a client asking for the shortcut list is given
    # L1. RED-trigger: every client is given the bare list, so a browser opening
    # the address lands on an unstyled run of keys. The nearest wrong fix —
    # giving the framed page to everybody — is caught by the three rows below
    # the first, which must still be given the bare list the overlay assigns
    # into the live page.
    When a client asks for the keyboard shortcut list <declaring>
    Then it is given <body>
    And the answer tells a cache it depends on what the client declared

    Examples: the four things a client can declare about where the answer goes
      | declaring                           | body            |
      | as a page it is opening             | a framed page   |
      | as a script fetching it             | the bare list   |
      | as a frame loading it               | the bare list   |
      | declaring nothing at all            | the bare list   |

  @real-io @driving_port
  Scenario: The page and the overlay advertise the same shortcuts
    # L2. RED-trigger: the page arm advertises nothing at all today. The wrong
    # fix this catches is a page that writes its own list of keys: the two
    # answers are compared against the shipped list AND against each other, so
    # a key added to one and not the other fails, and a key added to neither
    # fails against the list below.
    When the shortcut list is asked for both as a page and as a script fetching it
    Then both answers advertise exactly these shortcuts, in this order
      | key   | what it does   |
      | c     | Create issue   |
      | /     | Search         |
      | j     | Next           |
      | k     | Previous       |
      | Enter | Open selected  |
      | ?     | Show this help |
      | Esc   | Close modal    |
    And the two answers advertise the identical shortcuts in the identical order

  @needs-browser @real-io @bare-pages-shortcuts
  Scenario: Opening the shortcut list from the nav, each key sits beside what it does
    # P1. RED-trigger: every rule that draws a key as a cap is written for the
    # overlay host alone, so the page she opens draws browser-default type with
    # each key on a line of its own and its meaning on the next.
    Given the operator opens the keyboard shortcut list from the nav
    Then each shortcut key is drawn on the same line as what it does
    And the instruction about choosing a card is drawn below the list

  @needs-browser @real-io @bare-pages-shortcuts
  Scenario: On her phone each shortcut key still sits beside what it does
    # P1 at the width where a pair breaks first: the longest meaning in the list
    # beside the widest key.
    Given the operator opens the keyboard shortcut list on her phone
    Then each shortcut key is drawn on the same line as what it does
    And the instruction about choosing a card is drawn below the list

  # ==========================================================================
  # Signed out: one centred card under the product name
  # ==========================================================================

  @needs-browser @real-io @bare-pages-card
  Scenario Outline: Signed out, what she came for sits in one centred card under the product name
    # L3. RED-trigger: the form runs from one edge of the window to the other
    # with no frame, no margin and no product name, so the first page an
    # evaluator ever sees looks like an unfinished form. The wrong fix this
    # catches is a card given to sign-in alone: every page she can land on
    # signed out is read here, and so is the shortcut list she can open without
    # a session.
    Given the operator is signed out, looking at <page> on <device>
    Then the page is one full document drawn with Foundry's own stylesheet
    And what she came for sits in one centred card under the product name
    And nothing of the page runs off the side of her screen
    And the page offers her no navigation rail and no palette control

    Examples: at desk width
      | page                                            | device  |
      | the sign-in page                                | a desk  |
      | the forgotten-password page                     | a desk  |
      | the check-your-email page                       | a desk  |
      | the claim-your-workspace page                   | a desk  |
      | the invitation page where she sets her password | a desk  |
      | the page where she chooses a new password        | a desk  |
      | the keyboard shortcut list                      | a desk  |

    Examples: on her phone
      | page                                            | device  |
      | the sign-in page                                | a phone |
      | the forgotten-password page                     | a phone |
      | the check-your-email page                       | a phone |
      | the claim-your-workspace page                   | a phone |
      | the invitation page where she sets her password | a phone |
      | the page where she chooses a new password       | a phone |
      | the keyboard shortcut list                      | a phone |

  @needs-browser @real-io @bare-pages-card
  Scenario: The sign-in card holds on a screen that asks for the dark palette
    # The bound on the law, asserted rather than assumed: the card takes a
    # surface of its own, and a surface that only exists in one palette would
    # leave the other palette's operator with no card at all.
    Given the operator is signed out, looking at the sign-in page on a desk that asks for the dark palette
    Then what she came for sits in one centred card under the product name
    And the page offers her no navigation rail and no palette control

  # ==========================================================================
  # What the browser is told each field holds
  # ==========================================================================

  @real-io @driving_port
  Scenario Outline: The browser is told what each sign-in and password field holds
    # L4. RED-trigger: not one of these fields declares anything, so her manager
    # neither fills the sign-in form nor offers to generate and save a password
    # where one is being created. The row that catches the likely wrong value is
    # the claim page: a form that CREATES a password and asks for the current one
    # makes the manager offer a stored secret instead of a new one.
    When a visitor is served <page>
    Then the page tells the browser that "<field>" holds <intent>

    Examples: the fields the promise names
      | page                                            | field                                | intent           |
      | the sign-in page                                | her email                            | her username     |
      | the sign-in page                                | her password                         | her current password |
      | the claim-your-workspace page                   | her email                            | her username     |
      | the claim-your-workspace page                   | her password                         | a password she is creating |
      | the invitation page where she sets her password | her new password                     | a password she is creating |
      | the invitation page where she sets her password | the confirmation of her new password | a password she is creating |
      | the page where she chooses a new password       | her new password                     | a password she is creating |
      | the page where she chooses a new password       | the confirmation of her new password | a password she is creating |

    Examples: the field no promise names, which must gain nothing
      | page                        | field     | intent  |
      | the forgotten-password page | her email | nothing |

  # ==========================================================================
  # An issue's own page reads as the issue
  # ==========================================================================

  @real-io @driving_port
  Scenario Outline: An issue's own page shows its key, its title, what she wrote and where it sits
    # L5. RED-trigger: the page shows the key and a bare Delete link and nothing
    # else, so she goes back to the board to learn what the issue even is. The
    # wrong fix this catches is a heading holding the key and the title together:
    # the heading is read for the key ALONE.
    Given an issue whose description runs over two lines, sitting in <lane>
    When the operator opens that issue's own page
    Then the page is headed by the issue's key alone
    And the page shows the issue's title
    And the page shows what she wrote, line breaks and all, as text
    And the page shows where the issue sits in Foundry's own chip, reading "<lane>"

    Examples: every place an issue of hers can sit
      | lane        |
      | backlog     |
      | todo        |
      | in_progress |
      | done        |

  @real-io @driving_port
  Scenario: An issue with nothing written down still has one place for it
    # L5. RED-trigger: there is no description area at all. The wrong fix this
    # catches is an area that disappears when nothing is stored, which leaves the
    # operator unable to tell an empty issue from a page that forgot to show it.
    Given an issue with nothing written down, beside one that has a description
    When the operator opens both issues' own pages
    Then both pages hold exactly one place for the description
    And the empty one tells her nothing is written down yet

  @real-io @driving_port
  Scenario: Deleting from an issue's own page still asks her to confirm
    # Preserved, not new: the Delete control takes a Foundry presentation and
    # keeps the address that is its whole mechanism. RED-trigger if a crafter
    # replaces the anchor with something a browser cannot follow.
    Given an issue whose description runs over two lines, sitting in backlog
    When the operator follows Delete from that issue's own page
    Then she is asked to confirm before anything is deleted

  @real-io @driving_port
  Scenario: Whatever she wrote in the description is shown as she wrote it
    # L6, over generated text from a recorded seed, with the four named texts
    # always included. RED-trigger: nothing of what she wrote reaches the page.
    # The wrong implementations this catches are a page that draws her text as
    # markup, one that shortens it, and one that flattens her line breaks.
    Given she has written descriptions of every shape, including these
      | what she wrote                            |
      | nothing at all                            |
      | something that looks like a script        |
      | two lines ended the way a browser ends them |
      | one unbroken word of four thousand letters  |
    When each of those issues' own pages is opened
    Then every page shows that description as text, whole, with its line breaks

  @needs-browser @real-io @bare-pages-issue
  Scenario: The other controls on an issue's own page are drawn like Foundry's own
    # P3. RED-trigger: Delete is a bare browser link and the control for
    # attaching a file is the browser's raw one, drawn in a type nothing else on
    # the page uses.
    Given the operator is looking at an issue's own page
    Then Delete is drawn as one of Foundry's controls, not as a bare link
    And the control for attaching a file is drawn in Foundry's own type
