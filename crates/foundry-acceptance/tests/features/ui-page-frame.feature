# Feature: every page of the app is framed the same way, and its forms read as
# one ordered set of sections.
#
# ---------------------------------------------------------------------------
# WHAT THIS LANE DRIVES
# ---------------------------------------------------------------------------
# One running application, through its own public HTTP surface, opened in the
# REAL browser the @needs-browser lane drives: at the harness desk viewport and
# at the emulated 390x844 phone, and once on a screen that asks for each
# palette. Every geometry here is read from the LIVE layout — the box the
# browser painted — never from a class name. A page can carry every class this
# work adds and still start its content somewhere else.
#
# ---------------------------------------------------------------------------
# WHERE A PAGE "STARTS", EXACTLY
# ---------------------------------------------------------------------------
# The reading walks down from the one box the frame owns, taking the first child
# that is drawn at each step, until it reaches the first element holding words of
# its own — the page's own heading. That box is where the operator sees the page
# begin. The walk is what makes the defect visible: a page that wraps itself in
# another padded box, or centres itself in a column of its own, reports a start
# further in than the frame's, and the reading names every box on the path it
# took. Two pages agree only when BOTH their inline start and their block start
# agree, and the frame's own padding box is read beside them, so a run of pages
# that all drifted by the same amount cannot read as agreement.
#
# ---------------------------------------------------------------------------
# WHY A PAGE OUTSIDE THE FRAME IS READ TOO
# ---------------------------------------------------------------------------
# Four pages of this application put their content next to the window with no
# frame around it, and they pad themselves. The quickest way to give every framed
# page one edge is to take that padding away from everything — which leaves those
# four pages pressed against the side of the screen. So one of them is opened in
# the same session and its own padding is read there.
#
# ---------------------------------------------------------------------------
# WHAT THIS LANE CANNOT SEE
# ---------------------------------------------------------------------------
# How loud a control looks to an operator: it reads the fill, the edge, the face
# and the measured contrast a control resolves, not its visual weight. Nor the
# reading order assistive technology announces beyond document order, nor an
# operator's own font size or zoom. @needs-browser is excluded from the default
# fast lane, so evidence for this feature must name the lane that ran.

@ui-page-frame
Feature: Every app page is framed the same way and its forms read as ordered sections
  Moving between Home, her board, Teams, invites, tokens and settings, the
  operator sees the content hold its place: it never jumps sideways and never
  starts at a different height. Each stacked form reads as a section under its
  own heading with its own action beside its fields, and every field is as wide
  as what it holds while every list, number and file control is drawn like
  Foundry's own. In the issue dialog Delete reads as the quiet control beside
  Save and still asks her to confirm, and the board header reads as one tidy row
  with the project key as a labelled value.

  Background:
    Given a workspace "Acme" exists with a member "Mei" on team "Backend"
    And a project "Sandbox" with key prefix "GEN" exists under "Backend"
    And Mei holds every seat her workspace and the instance offer
    And a teammate she can add to a team, and an issue she can open on her board

  # ==========================================================================
  # B1 — one frame owns where every page starts
  # ==========================================================================

  @needs-browser @real-io @page-frame
  Scenario Outline: Every page she opens starts its content at the one frame edge
    # L1. RED-trigger on the shipped build: Home starts further in than the rest
    # and in a column of its own, and Teams, tokens, settings, the instance
    # dashboard and the change report each add a second padded box, so the six
    # starts disagree with the board's. The reading names each page and the boxes
    # on its path, so the first failure says WHICH page and WHICH box. The
    # painted top of the first heading is read on its own: subtracting that
    # heading's margin would report the frame's edge while the line she sees
    # sits lower on some pages than on others.
    Given she is signed in and reading Foundry on <device>
    When she opens each page of the app in turn
    Then every page starts its content at the one edge the frame owns
    And every page starts its content at the one height the frame owns
    And the first heading on every page sits at the height the frame owns
    And exactly one padded box stands between the frame and a page's own words
    And nothing on any of those pages runs off the side of her screen
    And the page that is not framed keeps the page padding of its own

    Examples: the two screens she reads Foundry on
      | device    |
      | a desk    |
      | her phone |

  @needs-browser @real-io @page-frame
  Scenario: Widening her window never moves where a page starts
    # L1, the half a single-page reading cannot catch: a centred column is a
    # function of the WINDOW, so it holds still only while the window does. The
    # wrong fix this catches is a frame that pads correctly while Home keeps its
    # own centred column, and a frame given a maximum content width, which would
    # centre every page's start instead of only Home's.
    Given she is signed in and reading Foundry on a desk
    And she has opened each page of the app in turn
    When she opens her window wider
    Then every page still starts its content where it did before
    And every page still starts at the one edge the frame owns
    And her board still uses the width of the window

  @needs-browser @real-io @page-frame
  Scenario Outline: Her board stays on the screen with no sideways scroll
    # L1. A page can keep the document from scrolling while its lanes run past
    # the frame and the board strip scrolls on its own. This reading measures
    # the lanes and that strip, not only the document.
    Given she is signed in and reading Foundry on <device>
    When she opens her board
    Then every lane of her board is fully on the screen
    And she does not have to scroll the board sideways

    Examples: the two screens she reads her board on
      | device    |
      | a desk    |
      | her phone |

  @needs-browser @real-io @page-frame
  Scenario Outline: Each project card on Home spans the frame
    # L1. Home can start at the frame's left edge and still lay each project
    # card in a narrow track of a wider grid. One card is the only track that
    # holds anything, so it spans the frame even when empty tracks collapse,
    # and two cards still sit in narrow tracks. Home holds two projects, and
    # this reading measures each card against the frame, not the grid.
    Given a project "Ledger" with key prefix "LDG" exists under "Backend"
    And she is signed in and reading Foundry on <device>
    When she opens Home
    Then each project card spans the width of the frame

    Examples: the two screens she reads Home on
      | device    |
      | a desk    |
      | her phone |

  @real-io @driving_port
  Scenario: Every place the rail offers her is a page this reading covers
    # Y1: the frame stays the only padded box only for the pages somebody reads.
    # A page added to the rail later is NOT covered until it is enumerated here,
    # so this guard fails the moment the rail offers a destination the all-pages
    # reading does not open. Her board is matched by the exact route this
    # reading already opens, /team/backend/project/sandbox, not by a prefix.
    # The shortcut list and the integrations pages are the only places never
    # asked. Another place under /team/ is not her board and fails this guard.
    Given the operator has her own session of the application
    When the pages the rail offers her are listed beside the pages this reading opens
    Then the reading opens every page the rail offers her

  # ==========================================================================
  # B2 — a field is as wide as what it holds; the controls are Foundry's own
  # ==========================================================================

  @needs-browser @real-io @page-frame
  Scenario: Whatever she has stored, a field stays as wide as its own size and no page scrolls sideways
    # L2, over generated stored names from a recorded seed, with the named
    # extremes always included. RED-trigger: a team name field, a rename field
    # and an email field each run the whole width of the frame today. The wrong
    # fixes this catches are a width instead of a cap, which lets one unbroken
    # name push the page wider; a cap applied everywhere, which narrows the
    # sign-in card's own field; and a cap that reaches into a dialog, which would
    # leave the title field stopping short of the description under it.
    Given she is signed in and reading Foundry on a desk
    And she has stored names of every shape, including these
      | what she stored                                |
      | a project name of 256 letters and no spaces    |
      | a workspace name of 24 of the widest letters   |
      | a team name of one letter                      |
      | a team name of 64 letters                      |
      | an email of 254 letters                        |
    When each page that shows those names is opened, and her issue dialog beside them
    Then every field inside the frame stays within the cap that applies where it sits
    And no page she opened runs off the side of her screen
    And in a dialog the title field is exactly as wide as the description under it
    And on the sign-in card her password field keeps the width of the card

  @needs-browser @real-io @page-frame
  Scenario Outline: Whichever palette her screen asks for, every list, number and file control is drawn as Foundry's own
    # L3. RED-trigger: the default-team list, the member list, the expiry number
    # and the two dialog lists are the browser's own boxes, in the browser's own
    # face, beside fields Foundry draws. The wrong fix this catches is a control
    # drawn with no edge at all, which this reading measures against the page and
    # refuses, and one whose arrow is taken away with nothing drawn in its place.
    Given she is signed in and reading Foundry on a screen that asks for the <palette> palette
    When she opens every form of the app and both of its dialogs
    Then every list, number and file control is drawn as one of Foundry's own
    And every one of those controls has an edge she can see against the page
    And the words in every one of those controls read clearly
    And a list still shows the arrow her own browser draws

    Examples: the two palettes a screen can ask for
      | palette |
      | light   |
      | dark    |

  # ==========================================================================
  # B3 — a stacked form is a section with a heading and its own action
  # ==========================================================================

  @needs-browser @real-io @page-frame
  Scenario: Each page of stacked forms reads as sections, each under its own heading
    # L4. RED-trigger: the Teams Save sits at the foot of the page under no
    # heading, the instance rename forms sit in indented lists of bullets, and
    # the settings groups are drawn with the browser's own ridged box. A section
    # that holds one heading and several forms still fails: each form needs a
    # heading no other form shares, which a shared Workspaces heading does not.
    # The wrong fix this catches is a section that pads itself, which would undo
    # the one frame edge one page at a time — so the content edge is read again
    # here.
    Given she is signed in and reading Foundry on a desk
    When she opens Teams, the instance dashboard and her account settings
    Then every form sits in a section holding exactly one heading and its own action
    And each form sits under a heading that no other form shares
    And the rename rows show no bullet and no indent of the browser's own
    And a project row can still be told from its workspace
    And the grouped choices are drawn as Foundry's own groups
    And none of those sections moved the page's content edge

  @needs-browser @real-io @page-frame
  Scenario: Renaming from the instance dashboard still swaps the row, and a refused name still answers in its own place
    # The swap contract the sectioned rows must survive: both renames are swapped
    # by the row's own outer element, and a refusal lands in that row's own error
    # place. RED-trigger if the row's element kind, its markers or its error place
    # change with the presentation.
    Given she is signed in and reading Foundry on a desk
    And she has opened the instance dashboard
    When she renames a project and a workspace, once with a name each takes and once with one each refuses
    Then each row she renamed shows its new name in place
    And each refusal is answered inside the row it was refused in

  # ==========================================================================
  # D1 — Delete is the quiet control of the issue dialog
  # ==========================================================================

  @needs-browser @real-io @page-frame
  Scenario Outline: In the issue dialog Delete is the quiet control and Save alone is the main action
    # L5. RED-trigger: Delete resolves no rule of its own, so it inherits the
    # page's primary fill and reads as the main action of the dialog beside Save.
    # The wrong fix this catches is a Delete with no fill and no edge: this
    # reading measures its edge against the dialog and refuses a control with
    # nothing to measure.
    Given she is signed in and reading Foundry on a screen that asks for the <palette> palette
    When she opens the edit dialog of a card on her board
    Then Delete is drawn as one of Foundry's quiet controls and Save as the main action
    And Delete has an edge she can see against the dialog
    And the status list and the title field of that dialog are Foundry's own

    Examples: the two palettes a screen can ask for
      | palette |
      | light   |
      | dark    |

  @needs-browser @real-io @page-frame
  Scenario: Pressing the quiet Delete still asks her to confirm
    # L5. The presentation change must not cost the step that protects her: the
    # control keeps the address it fetches and the place it puts the answer.
    Given she is signed in and reading Foundry on a desk
    And she has opened the edit dialog of a card on her board
    When she presses Delete in that dialog
    Then she is asked to confirm before the issue in that dialog is deleted

  # ==========================================================================
  # D2 — the board header is one row and the key is a labelled value
  # ==========================================================================

  @needs-browser @real-io @page-frame
  Scenario: The board header and the change report each name the project key as a labelled value
    # L6. RED-trigger: both headers show the key as a run of text with no element
    # of its own, and the two board links are run together with an underline and
    # no gap between them. The wrong fix this catches is a key element added to
    # the board alone, which would leave the change report as a second idiom for
    # the same fact: both pages are read.
    Given she is signed in and reading Foundry on a desk
    When she opens her board and its change report
    Then each page names the project key as a labelled value under "Key prefix"
    And the two board controls are drawn as quiet controls with a gap between them
    And neither board control is underlined
    And the change report is still reached by the address the live board reads

  @real-io @driving_port
  Scenario: A board she may not manage still reads as one row with the key beside its label
    # L6, the other board state: one control instead of two. The wrong fix this
    # catches is a header row built around two controls, which leaves a lone
    # control sitting somewhere else.
    Given a teammate who may read her board but may not manage it
    When that teammate opens the board
    Then the board names the project key as a labelled value under "Key prefix"
    And only the change report is offered to her, in the row of board controls

  @needs-browser @real-io @page-frame
  Scenario: Her open board drops a card that was deleted somewhere else
    # L6. The live board derives its address from the change report control.
    # The refresh that control keeps alive is the one the board ships: a card
    # deleted somewhere else leaves the open board, and the column that held
    # the card stays that column. A reload, or a replacement of that column,
    # is not that refresh. A new card appearing on its own is a different
    # feature, and this reading does not ask for it.
    Given she is signed in and reading Foundry on a desk
    And she has opened her board
    When the issue on her board is deleted from somewhere else
    Then that card leaves her open board without her reloading it

  @needs-browser @real-io @page-frame
  Scenario: Pressing slash on her board still opens the search panel in its own place
    # Y2. The search panel is put in beside the New issue control, inside that
    # control's own parent, so a header row built around that control would lay
    # the panel out as one more item in the row.
    Given she is signed in and reading Foundry on a desk
    And she has opened her board
    When she presses the slash key
    Then the search panel is drawn in its own block beside the New issue control
