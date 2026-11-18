# Feature: nav-collapse-icon-rail — the narrow left nav keeps every destination
# the operator had, drawn as icons.
#
# ---------------------------------------------------------------------------
# WHAT THIS LANE DRIVES
# ---------------------------------------------------------------------------
# Every scenario below is driven in a REAL browser against the SAME in-process
# origin the HTTP lane uses: the Background seeds Acme / Mei / Backend / Sandbox
# through the shipped HTTP steps, each Given grants the operator the rights its
# row names, opens a session onto that origin and signs her in through the real
# form. The stimulus is always her own press, keystroke, navigation or the width
# of her screen; the observations are LAYOUT AND ACCESSIBILITY FACTS read from
# the live document in ONE reading — which rail members are in the layout,
# visible and able to take the keyboard focus, the name the BROWSER computes for
# each of them, which one is marked as the page she is on, whether a focused icon
# shows its word in a box she can see, where each box sits inside the bar, and
# whether the page slides sideways.
#
# ---------------------------------------------------------------------------
# WHY THE NAME IS READ FROM THE BROWSER AND NOT FROM THE MARKUP
# ---------------------------------------------------------------------------
# An icon with no name is not a destination a screen-reader operator can choose,
# and the rail is her only route while it is narrow. A word taken out of the
# layout is still in the markup and is NOT in what the browser announces, so a
# name read from the markup would be green over a rail of silent icons. These
# scenarios read the browser's own computed name for each member.
#
# ---------------------------------------------------------------------------
# THE ONE THING THIS LANE CANNOT SEE
# ---------------------------------------------------------------------------
# What an assistive technology actually says. The name is read in one browser
# engine's accessibility tree, which is the closest observable thing to it, not
# the announcement itself. Whether a very long destination name stays legible in
# the label box at every supported width is likewise unobserved here.
#
# ---------------------------------------------------------------------------
# WHY THE LANE MUST BE NAMED IN ANY CLOSE EVIDENCE
# ---------------------------------------------------------------------------
# These scenarios are @needs-browser: the default fast lane excludes them, and
# `cargo xtask ci` runs them only with Docker included. A green gate on a host
# with no Docker observes NOTHING in this file. Evidence for this feature must
# name the lane that actually ran.

@nav-collapse-icon-rail @us-nav-collapse
Feature: The operator works from the narrow left nav without losing a destination
  An operator who narrowed the left nav still goes everywhere she went before:
  every destination her rights give her, and Sign out, is an icon in the strip,
  each one named with the place it opens, with the page she is on marked. She
  signs out of it in one press and finds the nav as she left it next time. On a
  phone the icons stay inside the bar and the page never slides sideways.

  Background:
    Given a workspace "Acme" exists with a member "Mei" on team "Backend"
    And a project "Sandbox" with key prefix "GEN" exists under "Backend"
    And Mei is signed in

  # ==========================================================================
  # Nothing becomes unreachable when the rail narrows
  # ==========================================================================

  @needs-browser @real-io @icon-rail-reachability
  Scenario Outline: Every destination the operator had stays one press away in the narrow rail
    # The whole value of the slice. RED-trigger: the narrow rail takes its
    # destinations out of the layout, so the operator is left in a strip with a
    # monogram and two controls and no way to anywhere — or it keeps them in the
    # layout and loses their names, which leaves a screen-reader operator with a
    # row of silent glyphs.
    Given the operator is <rights>, with the dashboard open at desk width and the left nav open
    When she narrows the left nav to its icon rail
    Then every destination she had, and Sign out, is still in the rail, named and ready to press
    And the workspace name and her own name are out of the narrow rail

    Examples:
      | rights                                 |
      | a member with no admin rights          |
      | a workspace admin                      |
      | an instance admin                      |
      | both a workspace and an instance admin |

  @needs-browser @real-io @icon-rail-degraded
  Scenario Outline: With no scripting the operator gets every destination in the open nav
    # The bound on the promise, asserted rather than assumed: the narrow rail only
    # exists where scripting ran, so with scripting off she gets the open nav she
    # has always had — with all of her destinations in it. RED-trigger: a
    # destination is moved behind a script, so a browser that runs none shows the
    # operator a nav with holes in it.
    Given the operator is <rights> and her browser runs no page scripts
    When she opens the dashboard with no scripting
    Then every destination she had, and Sign out, is in the open left nav, named and ready to press

    Examples:
      | rights                        |
      | a member with no admin rights |
      | a workspace admin             |
      | an instance admin             |

  # ==========================================================================
  # Going somewhere from the narrow rail, and knowing where she is
  # ==========================================================================

  @needs-browser @real-io @icon-rail-journey
  Scenario: The operator opens her board from the narrow rail and sees it marked there
    # RED-trigger: the icons are decoration — they draw a glyph and open nothing —
    # or the strip marks nothing, so she presses an icon, lands somewhere and
    # cannot tell from the rail where she ended up.
    Given the operator is both a workspace and an instance admin, with the left nav narrowed to its icon rail on the dashboard
    When she presses the Board icon in the narrow rail
    Then the board opens with the rail still narrow, and Board alone is marked as the page she is on

  @needs-browser @real-io @icon-rail-journey
  Scenario: Walking the narrow rail from the keyboard shows the operator each destination's name
    # The pointer operator reads the same box on hover; the keyboard is the surface
    # that can be driven. RED-trigger: the word is taken out of the layout instead
    # of being clipped and revealed, so an operator who cannot name the glyphs has
    # to press one to find out where it goes.
    Given the operator is both a workspace and an instance admin, with the left nav narrowed to its icon rail on the dashboard
    When she walks the whole page with the Tab key, stopping on each icon in the rail
    Then each icon she stops on shows the name of the place it opens, in a box she can see

  # ==========================================================================
  # Leaving, and coming back
  # ==========================================================================

  @needs-browser @real-io @icon-rail-sign-out
  Scenario: One press of Sign out in the narrow rail ends the operator's session
    # RED-trigger: Sign out is dropped from the strip, so the operator has to open
    # the nav again to leave — or it grows a confirmation step in the strip that
    # the open nav does not have.
    Given the operator is both a workspace and an instance admin, with the left nav narrowed to its icon rail on the dashboard
    When she presses Sign out in the narrow rail, once
    Then her session is over with nothing else asked of her, and the app opens no page of hers until she signs in again

  @needs-browser @real-io @icon-rail-sign-out
  Scenario Outline: The nav the operator left is the nav she comes back to
    # Signing out is not opening the nav again: the choice is kept per browser, as
    # the theme choice is. RED-trigger: signing out clears the remembered choice,
    # so the operator who narrowed the rail finds it open at her next sign-in — or
    # the rail is drawn from the rights of the session that ended.
    Given the operator left the left nav <geometry> and signed out from it in the same browser
    When she signs in again <rights>
    Then the left nav is <geometry> again, with the destinations her rights give her now

    Examples:
      | geometry | rights                                    |
      | narrow   | with the same rights                      |
      | open     | with the same rights                      |
      | narrow   | with her instance-wide rights taken away  |

  @needs-browser @real-io @icon-rail-degraded
  Scenario: Signing out with no scripting leaves the choice she made earlier untouched
    # The no-script arm of the same law. She cannot narrow the nav in a browser that
    # runs no scripts, and what she chose earlier has to be waiting for her next
    # visit in one that does. RED-trigger: signing out reaches into the remembered
    # choice and clears it, so an operator whose browser stopped running scripts for
    # one visit comes back to a nav she did not choose.
    Given the operator is both a workspace and an instance admin, her browser runs no page scripts, and she signed out of the left nav after it remembered the narrow nav she chose earlier
    When she signs in again with no page scripts
    Then the left nav is open with every destination in it, and the choice she made earlier is byte-for-byte the one she left

  # ==========================================================================
  # A phone
  # ==========================================================================

  @needs-browser @real-io @icon-rail-phone
  Scenario Outline: On a phone the narrow bar shows every destination inside itself
    # The promise is 480 px and below, so the boundary width itself and two widths
    # under it are driven. RED-trigger: the icons are laid out in one row that
    # reaches past the edge of the bar, so the narrowest phones scroll sideways —
    # or a destination is dropped below the breakpoint to make the row fit.
    Given the operator is both a workspace and an instance admin
    When she opens the dashboard with the left nav narrowed on a phone <width> px wide
    Then every destination she has, and Sign out, is laid out inside the top bar, and that page does not slide sideways

    Examples:
      | width |
      | 320   |
      | 390   |
      | 480   |
