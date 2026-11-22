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
# WHY THE SHORTCUT LIST IS ASKED FOR IN SO MANY WAYS
# ---------------------------------------------------------------------------
# One address answers two kinds of body, and the page body comes in two frames.
# WHICH BODY follows what the asking client DECLARES about its destination: a
# client opening a page is given a page, and a script fetching, a frame loading
# and a client declaring nothing at all are each still given the bare list the
# overlay writes into the live page. WHICH FRAME follows WHO IS ASKING: the
# operator with a session keeps the rest of Foundry around the list, and a
# reader with no session, or with a session whose account is gone, is given the
# centred card. So the address is read as each of those readers, and the page
# she opens signed in is read once more by SIGNING OUT from it, because a frame
# assembled without its anti-forgery token looks right and refuses the one
# press on it.
#
# Every one of those answers is also read for what it tells a cache. A cache
# that is told nothing is free to hand a page body to the overlay, the bare list
# to a browser opening the address, or one operator's framed page — which names
# her workspace — to the next reader of the same address. That defect shows up
# only at the cache, only for the OTHER reader, which is why both headers are
# read on EVERY answer and not only on the new one.
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
  Scenario: Signed in, she opens the shortcut list and keeps the rest of Foundry around it
    # L1 L3 L8. RED-trigger: she is given the centred card, which for her is a
    # page with no rail, no workspace and no way back — exactly what the first
    # examination refused. The wrong fix this catches is a frame that marks no
    # place in the rail, or marks two: the marking is read for EXACTLY one, and
    # for which one.
    Given the operator has a session of her own
    When she asks for the keyboard shortcut list as a page she is opening
    Then she is given the rest of Foundry around the list
    And exactly one place in the rail is marked as where she is, and it is her home
    And the list is still the one a reader with no scripting finds
    And the answer tells a cache it depends on what was declared and on who asked
    And the answer tells every cache to keep no copy of it

  @real-io @driving_port
  Scenario: She can sign out from the shortcut page she opened signed in
    # L1. RED-trigger: the page she is given carries no rail and so nothing to
    # sign out with. The wrong fix this catches is a frame assembled without its
    # anti-forgery mint: the page then LOOKS right and the one press on it is
    # refused, which no reading of the markup alone would notice.
    Given the operator has a session of her own
    And she has opened the keyboard shortcut list as a page she is opening
    When she signs out from that page
    Then she is signed out, and her session opens her workspace no more

  @real-io @driving_port
  Scenario: A reader whose account is gone is given the card, not a refusal and not a nameless frame
    # L1. RED-trigger: the header set this reading needs is not answered yet.
    # The wrong fix this catches is a frame drawn for an identity the store can
    # no longer answer for: it would greet her as "there" in "your workspace",
    # which is worse than the card and reads as a defect of her account.
    Given the operator's account was removed while her session was open
    When she asks for the keyboard shortcut list as a page she is opening
    Then she is given the centred card, with no rail and no workspace named
    And the answer tells a cache it depends on what was declared and on who asked
    And the answer tells every cache to keep no copy of it

  @real-io @driving_port
  Scenario: With no session at all the shortcut list is still a page she can read
    # L1 L3 L8. RED-trigger: the header set is not answered yet. The body is
    # already the card, and that is what this reading holds in place while the
    # signed-in frame is added beside it.
    Given the operator has no session at all
    When she asks for the keyboard shortcut list as a page she is opening
    Then she is given the centred card, with no rail and no workspace named
    And the list is still the one a reader with no scripting finds
    And the answer tells a cache it depends on what was declared and on who asked
    And the answer tells every cache to keep no copy of it

  @real-io @driving_port
  Scenario Outline: A client that is not opening a page is still given the bare list
    # L1 L8. RED-trigger: the answers name only what was declared, not who
    # asked, so a shared cache may hand one reader's answer to another. The
    # nearest wrong fix — giving the page body to everybody — is caught here,
    # because each of these must still be given the body the overlay writes into
    # the live page, and these answers stay keepable by a cache.
    Given the operator has no session at all
    When she asks for the keyboard shortcut list <declaring>
    Then she is given the bare list
    And the answer tells a cache it depends on what was declared and on who asked
    And a cache may keep the answer

    Examples: every way of asking that is not opening a page
      | declaring                |
      | as a script fetching it  |
      | as a frame loading it    |
      | declaring nothing at all |

  @real-io @driving_port
  Scenario: The page and the overlay advertise the same shortcuts
    # L2. This reading is GREEN today and is meant to be: both ways of asking
    # get the same single body, so the two lists agree because they are one
    # list. It is a drift guard that holds the line from the moment the page arm
    # exists. The wrong fix it catches is a page that writes its own list of
    # keys: the two answers are compared against the shipped list AND against
    # each other, so a key added to one and not the other fails, and a key added
    # to neither fails against the list below.
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

  @real-io @driving_port
  Scenario: No field beyond those eight claims to hold her credentials
    # L4, the count rather than the identities: eight fields are told what they
    # hold and no ninth is. The wrong fix this catches is a token spreading to a
    # field nobody promised — a search box told it holds a username, or a
    # one-time confirmation told it holds a password — which teaches her manager
    # to offer secrets where none belong.
    When every page this work touches is served to her
    Then exactly eight of their fields are told what they hold, and no other

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
    And each place is one the stylesheet can reach
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

  @needs-browser @real-io @bare-pages-issue
  Scenario Outline: Whichever palette her screen asks for, the status and the attach control are readable
    # L5. RED-trigger: the status is bare text that resolves no surface, no edge
    # and no corner at all, and the button inside the attach control is the
    # browser's own. The wrong fixes this catches are a status painted on a
    # see-through tint, which a reader's eye and a measurement both fall
    # straight through, and either control's words left too faint to read on the
    # surface behind them.
    Given the operator is looking at an issue's own page on a screen that asks for the <palette> palette
    Then the status is drawn on a surface of its own, with an edge and rounded corners
    And the status reads clearly against that surface
    And the button that chooses a file is drawn as one of Foundry's quiet controls
    And that button's word reads clearly, and its edge is visible against the page

    Examples: the two palettes a screen can ask for
      | palette |
      | light   |
      | dark    |
