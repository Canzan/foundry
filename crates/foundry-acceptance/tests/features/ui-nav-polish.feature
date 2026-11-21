# Feature: ui-nav-polish — the left nav tells the operator where she is, offers
# only places she can open, and stays out of her way.
#
# ---------------------------------------------------------------------------
# WHAT THIS LANE DRIVES
# ---------------------------------------------------------------------------
# Two surfaces of ONE application. The mark-and-offer scenarios are real HTTP
# GETs on the authenticated routes of the running application carrying a real
# session cookie, against a real per-scenario Postgres (@real-io) read through
# the shipped fail-closed rights lookups — no route, lookup or admission
# decision is changed anywhere below. The geometry scenarios are the same GETs
# rendered in the REAL browser the @needs-browser lane drives: at the harness
# desk viewport, and at the emulated 390x844 phone device.
#
# ---------------------------------------------------------------------------
# WHY EVERY OFFERED LINK IS THEN OPENED
# ---------------------------------------------------------------------------
# "The nav offers her only what opens" cannot be read off the rail: a rail
# compared against the rail agrees with itself. So every href the rail offered
# is opened again with the SAME cookie the rail was rendered for, and the
# answer is read. A link that answers her own session with the refusal 404
# teaches her the application is broken and advertises a surface she may not
# use.
#
# ---------------------------------------------------------------------------
# THE RIGHTS DOMAIN IS CLOSED
# ---------------------------------------------------------------------------
# Four rows exhaust it — a member with no admin rights, a workspace admin, an
# instance super-admin who administers no workspace, and both — so every law
# below is enumerated rather than generated.
#
# ---------------------------------------------------------------------------
# THE BOUND ON THE PHONE READING
# ---------------------------------------------------------------------------
# The one-fifth bound is promised for phones 390 px wide and up. This lane
# drives ONE emulated device, 390x844, so the measured bound is a bar at or
# under 168.8 px there. A narrower phone may show a taller bar, and what that
# bar measures at 320 px is deliberately NOT asserted here: it is unbounded by
# any authority. What is asserted at every phone width is that no destination
# is hidden to make the bar fit.
#
# The no-script row is the one reading NOT taken on the emulated device: a
# browser that runs no page scripts is a session of its own here, held at the
# phone's own 390 px width, and its screen is the window's rather than the
# device's. Its bound is therefore a fifth of the screen it actually measured,
# and what it does not re-prove is the viewport declaration — that is read by
# the shipped mobile lane.
#
# ---------------------------------------------------------------------------
# WHAT THIS LANE CANNOT SEE
# ---------------------------------------------------------------------------
# A rights lookup that FAILS mid-render. The application answers such a render
# with a neutral identity that offers neither admin link, and no public surface
# or declared seam makes that failure happen, so it is not observed here.
# @needs-browser is also excluded from the default fast lane: a green gate on a
# host with no Docker observes NOTHING in the geometry scenarios, and evidence
# for this feature must name the lane that actually ran.

@ui-nav-polish
Feature: The left nav says where the operator is and offers only what opens
  Wherever she is, the operator reads her own place off the left nav: the item
  that owns the page she is on is marked there, and nothing else is. She is
  offered the places her rights let her into and no others, every one of them
  opens for her, and she loses nothing she could already reach. On a long page
  the nav stays with her, on a phone the bar across the top stays short with
  every destination inside it, and the two controls at the foot of the nav read
  as quietly as the links beside them.

  Background:
    Given a workspace "Acme" exists with a member "Mei" on team "Backend"
    And a project "Sandbox" with key prefix "GEN" exists under "Backend"
    And Mei is signed in

  # ==========================================================================
  # Where she is
  # ==========================================================================

  @real-io @driving_port
  Scenario Outline: The nav marks the page the operator is on and no other
    # RED-trigger: Teams, Invite a member and Machine tokens mark nothing, so
    # Home claims to be the page she is on while she reads her machine tokens.
    # The nearest wrong implementation — marking the owner and leaving Home
    # marked too — is caught by the second observation, not the first.
    Given she holds the rights of <rights>
    When she opens "<page>"
    Then the left nav marks "<marked>" as the page she is on, and nothing else
    And the item the nav marks is one of the places it offers her

    Examples: the places that own an item in the nav
      | rights                                 | page                            | marked          |
      | a member with no admin rights          | /                               | Home            |
      | a member with no admin rights          | /team/backend/project/sandbox   | Board           |
      | a member with no admin rights          | /teams                          | Teams           |
      | a workspace admin                      | /team/backend/members           | Teams           |
      | a workspace admin                      | /workspace/invites              | Invite a member |
      | a workspace admin                      | /admin/tokens                   | Machine tokens  |

    Examples: a place no item in the nav owns
      | rights                                 | page                            | marked          |
      | a member with no admin rights          | /account/settings               | Home            |
      | both a workspace and an instance admin | /account/settings               | Home            |

  # ==========================================================================
  # What she is offered, and whether it opens
  # ==========================================================================

  @real-io @driving_port
  Scenario Outline: An operator the two admin pages refuse is offered neither of them, and keeps everything else
    # RED-trigger: Invite a member and Machine tokens are offered to every
    # signed-in operator, so a member with no rights presses one and the
    # application answers her nowhere. The instance-admin row is the second
    # cause: being an admin of SOMETHING is not the fact those two pages read.
    Given she holds the rights of <rights>
    When she opens "/team/backend/project/sandbox"
    Then the left nav offers her exactly these places
      | word  | opens                         |
      | Home  | /                             |
      | Board | /team/backend/project/sandbox |
      | Teams | /teams                        |
    And every place the left nav offers her opens for her

    Examples:
      | rights                        |
      | a member with no admin rights |
      | an instance admin             |

  @real-io @driving_port
  Scenario Outline: An operator those pages let in is offered all five places, and every one opens
    # The other half of the same law, and the catch for the wrong fix: hiding the
    # two links from EVERYONE also stops a plain member pressing a refusal, and
    # it fails here.
    Given she holds the rights of <rights>
    When she opens "/team/backend/project/sandbox"
    Then the left nav offers her exactly these places
      | word            | opens                         |
      | Home            | /                             |
      | Board           | /team/backend/project/sandbox |
      | Teams           | /teams                        |
      | Invite a member | /workspace/invites            |
      | Machine tokens  | /admin/tokens                 |
    And every place the left nav offers her opens for her

    Examples:
      | rights                                 |
      | a workspace admin                      |
      | both a workspace and an instance admin |

  @real-io @driving_port
  Scenario: The two admin pages still refuse an operator the nav no longer offers them to
    # The nav hiding a link neither adds nor removes an admission decision: she
    # can still type the address, and the answer is the same non-enumerable
    # refusal it has always been. RED-trigger: hiding the link is mistaken for
    # the gate and the page starts letting her in.
    Given she holds the rights of a member with no admin rights
    When she types the address "/workspace/invites" herself
    Then the application answers her nowhere
    When she types the address "/admin/tokens" herself
    Then the application answers her nowhere

  # ==========================================================================
  # The nav stays with her on a long page
  # ==========================================================================

  @needs-browser @real-io @nav-polish-sticky
  Scenario Outline: The whole nav is still in view at the bottom of a long page
    # RED-trigger: the rail stretches to the height of the document, so by the
    # time she reaches the bottom of a long board the nav is a screen and a half
    # above her and she scrolls back up to go anywhere.
    Given the operator is <geometry> on a page taller than her window at desk width
    When she scrolls that page to its bottom
    Then the whole left nav is in view, with every place she can press

    Examples:
      | geometry                              |
      | looking at the open nav               |
      | looking at the nav narrowed to icons  |

  @needs-browser @real-io @nav-polish-sticky
  Scenario Outline: On a page with nothing to scroll the whole nav fits her window
    # The other half of the law. A short page does change for her: with the open
    # nav it measured 0 to 769 px in a 757 px window before, because the rail
    # stretched to the height of the whole shell and ran past the bottom of what
    # she could see; held at the height of the window it fits. RED-trigger: the
    # rail is pinned by a rule that lifts it out of the shell, and a page that
    # fits now draws the nav over its own content or caps it so a place falls
    # outside the box.
    Given the operator is <geometry> on a page that fits in her window at desk width
    Then the whole left nav is in view, with every place she can press

    Examples:
      | geometry                              |
      | looking at the open nav               |
      | looking at the nav narrowed to icons  |

  # ==========================================================================
  # The narrowed nav over her board
  # ==========================================================================
  # Holding the nav in view must not cost the operator the one thing the narrow
  # nav gives her: the word each glyph stands for, shown beside the strip while
  # she walks it from the keyboard. Over her board that word is drawn across a
  # lane, so it is read WHERE IT IS PAINTED and not where its box says it is:
  # the box is untouched both by an ancestor that clips the word out of view and
  # by page content drawn on top of it, so a reading of the box alone is green
  # over a word the operator cannot see at all.

  @needs-browser @real-io @nav-polish-flyout
  Scenario Outline: The word the narrowed nav reveals can be read over her board
    # RED-trigger, two causes and one reading: the narrowed strip scrolls its own
    # content, which clips the word out of view sideways with nothing to scroll it
    # back; and the nav takes no paint level of its own, so a lane of the board
    # drawn later covers the word even where it is not clipped. Either way she
    # walks a strip of glyphs she cannot name.
    Given the operator has narrowed the nav to icons on her board, taller than her window and scrolled to its bottom
    When she walks the keyboard onto "<place>" in the nav
    Then the word that place shows her lies over a lane of her board and can be read there

    Examples: one place from the nav's main group, one from the group at its foot
      | place              |
      | Invite a member    |
      | Keyboard shortcuts |

  @needs-browser @real-io @nav-polish-flyout
  Scenario: The shortcut list she opens covers the narrowed nav
    # The other end of the band: the nav is drawn above the board and below
    # everything the operator opens over the page. RED-trigger: the nav is lifted
    # over the board by a level above the overlays, so the list she asked for is
    # cut in half by a strip of glyphs.
    Given the operator has narrowed the nav to icons on her board, taller than her window and scrolled to its bottom
    When she opens the list of keyboard shortcuts
    Then the list she opened is drawn over the nav

  @needs-browser @real-io @nav-polish-flyout
  Scenario: With the nav open no word hangs over her board
    # The bound on the law, asserted rather than assumed: the open nav shows every
    # word in its own column, so there is no revealed word to read beside it and
    # the nav is free to scroll its own content. RED-trigger: the open nav starts
    # drawing a flyout as well, and then a word the operator never asked for hangs
    # over her work.
    Given the operator has the nav open on her board, taller than her window and scrolled to its bottom
    When she walks the keyboard onto "Invite a member" in the nav
    Then that place shows its word inside the nav itself, with nothing of it over her board

  # ==========================================================================
  # A phone
  # ==========================================================================

  @needs-browser @real-io @nav-polish-phone
  Scenario Outline: On her phone the bar above the page is short and keeps every place
    # RED-trigger: the bar takes about a third of the screen, so she reads her
    # board through a letterbox. Capping its height with a scroll box is the
    # wrong fix and is caught by the same reading: a place she cannot see while
    # the keyboard can still reach it fails here.
    Given she holds the rights of <rights>
    When she opens her phone on the dashboard <how>
    Then the bar above the page takes at most a fifth of her screen
    And every place in that bar can be seen and pressed, each a thumb's width high

    Examples:
      | rights                                 | how                                 |
      | a member with no admin rights          | as it comes                         |
      | both a workspace and an instance admin | as it comes                         |
      | both a workspace and an instance admin | having narrowed the nav earlier     |
      | both a workspace and an instance admin | with no page scripts running        |

  # ==========================================================================
  # The foot of the nav
  # ==========================================================================

  @needs-browser @real-io @nav-polish-quiet
  Scenario Outline: Sign out and the theme control read as quietly as the links beside them
    # RED-trigger: Sign out is a filled black slab and the theme control a
    # bordered chip, so the two heaviest things in the nav are the two the
    # operator uses least.
    Given the operator is looking at the foot of the nav in the <palette> palette
    Then Sign out and the theme control are drawn as quietly as the links beside them

    Examples:
      | palette |
      | light   |
      | dark    |

  @needs-browser @real-io @nav-polish-quiet
  Scenario: One press of Sign out still ends the operator's session
    # Quieter is not weaker. RED-trigger: the submit loses its presentation and
    # its behaviour with it.
    Given the operator is looking at the foot of the nav in the light palette
    When she presses Sign out, once
    Then her session is over and the application asks her to sign in again

  @needs-browser @real-io @nav-polish-quiet
  Scenario: One press of the theme control still changes the palette
    # RED-trigger: the control is quietened by replacing it, and the press that
    # used to change the palette does nothing.
    Given the operator is looking at the foot of the nav in the light palette
    When she presses the theme control, once
    Then the application changes its palette

  @needs-browser @real-io @nav-polish-quiet
  Scenario: With no page scripts there is no theme control and Sign out alone is quiet
    # The bound on the promise, asserted rather than assumed: the theme control
    # is built by a script, so a browser that runs none has none at all and the
    # law rests on the sign-out submit. RED-trigger: the quiet presentation is
    # applied by a script, so an operator with scripting off keeps the slab.
    Given the operator is looking at the foot of the nav with no page scripts running
    Then there is no theme control at all
    And Sign out is drawn as quietly as the links beside it
