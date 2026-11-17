# Feature: nav-collapse-toggle — the operator folds the left nav away and the app
# keeps her choice.
#
# ---------------------------------------------------------------------------
# WHAT THIS LANE DRIVES
# ---------------------------------------------------------------------------
# Every scenario below is driven in a REAL browser against the SAME in-process
# origin the HTTP lane uses: the Background seeds Acme / Mei / Backend / Sandbox
# through the shipped HTTP steps, and each Given opens a session onto that origin
# and signs Mei in through the real form. The stimulus is always the operator's
# own press, keystroke, reload or navigation; the observations are LAYOUT AND
# ACCESSIBILITY FACTS read from the live document — rendered widths, the marker on
# the document element, the toggle's accessible name and state, the keyboard order,
# the horizontal-overflow verdict and the remembered choice at the origin.
#
# ---------------------------------------------------------------------------
# THE ONE THING THIS LANE CANNOT SEE
# ---------------------------------------------------------------------------
# The pixels of the FIRST painted frame. This suite uses no paint-capture
# surface, so "no flash of the expanded nav" is observed in two layers, exactly
# as the theme lane observes the same promise:
#
#   (a) LOAD-BEARING, deterministic — the served document declares the script
#       that stamps the collapsed marker as a render-blocking head script,
#       carrying none of `defer`, `async`, `type="module"`. Each of those three
#       moves the stamp after the paint, and each does so while leaving every
#       settled-page assertion in this file green.
#   (b) SUPPORTING, measured — that script finished loading before the browser's
#       own first-contentful-paint entry. On a loopback origin (b) can pass by
#       luck even with a deferred script, so it never replaces (a).
#
# Whether the first frame of a collapsed operator's page is ever the expanded rail
# therefore stays an open question, recorded rather than papered over.
#
# ---------------------------------------------------------------------------
# WHY THE LANE MUST BE NAMED IN ANY CLOSE EVIDENCE
# ---------------------------------------------------------------------------
# These scenarios are @needs-browser: the default fast lane excludes them, and
# `cargo xtask ci` runs them only with Docker included. A green gate on a host
# with no Docker observes NOTHING in this file. Evidence for this feature must
# name the lane that actually ran.

@nav-collapse-toggle @us-nav-collapse
Feature: The operator folds the left nav away and the app remembers
  An operator who wants the page wider folds the left nav into a narrow strip,
  and it stays folded on the next page and the next reload until she opens it
  again. The control answers the keyboard and says which way the nav is set. With
  scripting off there is no control and the nav is simply open, as before.

  Background:
    Given a workspace "Acme" exists with a member "Mei" on team "Backend"
    And a project "Sandbox" with key prefix "GEN" exists under "Backend"
    And Mei is signed in

  # ==========================================================================
  # Folding the nav away, and what the page does with the room
  # ==========================================================================

  @needs-browser @real-io @nav-collapse-geometry
  Scenario: Folding the left nav away gives its width to the page content
    # RED-trigger: the nav narrows but the content box does not grow into the
    # room it released — an empty band down the left of the page, which is the
    # whole value of the feature missing while the nav itself looks right.
    Given the operator has the dashboard open at desk width with the left nav open
    When she folds the left nav away with its control
    Then the left nav is a narrow strip and the page content is wider by the room the nav released
    And the left nav and the page content still fill the width of the shell
    And the browser remembers the folded-away choice

  @needs-browser @real-io @nav-collapse-persistence
  Scenario Outline: A folded-away left nav is still folded away when the operator comes back
    # RED-trigger: the choice is kept in the page instead of at the origin, so it
    # evaporates on the first reload or the first navigation.
    Given the operator has folded the left nav away on the dashboard
    When she <return>
    Then the left nav is still a narrow strip there

    Examples:
      | return                  |
      | reloads that page       |
      | opens the project board |

  @needs-browser @real-io @nav-collapse-no-flash
  Scenario: The folded-away nav is settled before the browser is allowed to paint
    # The two-layer observation this file's header describes. RED-trigger: the
    # stamping script is given `defer`, `async` or `type="module"`, or moved to the
    # foot of the body — every other scenario here stays green and the operator
    # sees the open nav flash on every single navigation.
    Given the operator has folded the left nav away on the dashboard
    When she opens the project board
    Then the page already carries the folded-away mark, from a script the browser may not defer past the first paint
    And that script finished loading before the screen first painted

  # ==========================================================================
  # Opening it again
  # ==========================================================================

  @needs-browser @real-io @nav-collapse-reopen
  Scenario: Opening the left nav again brings its items back
    # RED-trigger: the control only folds, so the operator cannot get the nav
    # back; or it reopens visually while the items stay out of reach.
    Given the operator has folded the left nav away on the dashboard
    When she opens the left nav again with its control
    Then the left nav is back at its full width with every nav item in sight and selectable
    And the browser remembers no nav choice at all

  @needs-browser @real-io @nav-collapse-reopen
  Scenario: A left nav the operator opened again stays open on the next load
    # RED-trigger: opening again is recorded as a word of its own rather than as
    # the absence of a choice, so the next load reads that word and folds the nav
    # away under her.
    Given the operator has folded the left nav away on the dashboard and opened it again
    When she reloads that page
    Then the left nav is still at its full width

  @needs-browser @real-io @nav-collapse-reopen
  Scenario Outline: Two presses leave the left nav and the remembered choice as they were
    # The whole state domain is two values, so these two rows are the whole law.
    # RED-trigger: a press that writes without reading, or reads without writing,
    # so the second press lands in the other state from the one the operator sees.
    Given the operator has the dashboard open with the left nav <start>
    When she presses the nav control twice
    Then the left nav is <start> again
    And the browser remembers <choice>

    Examples:
      | start       | choice                 |
      | open        | no nav choice at all   |
      | folded away | the folded-away choice |

  # ==========================================================================
  # One browser, more than one page
  # ==========================================================================

  @needs-browser @real-io @nav-collapse-two-tabs
  Scenario: A press in a second tab leaves the page the operator is reading alone
    # The admitted divergence, asserted rather than assumed. RED-trigger: a
    # cross-tab listener is added, which would move the nav under an operator who
    # is reading the other tab — and the second Then is what proves the press in
    # the other tab happened at all.
    Given the operator has the dashboard open in two tabs of one browser with the left nav open
    When she folds the left nav away in the second tab
    Then the tab she was reading still shows the open left nav
    And that tab shows the narrow strip once she reloads it

  @needs-browser @real-io @nav-collapse-persistence
  Scenario: Reading three pages without touching the control never rewrites the choice
    # RED-trigger: a page load normalises or rewrites the stored choice, which
    # would quietly erase a real folded-away choice the first time the origin's
    # storage was unreadable.
    Given the browser remembers the folded-away choice from an earlier visit
    When the operator opens the dashboard, the project board and the project change report
    Then every one of those pages shows the narrow strip
    And the remembered choice is byte-for-byte the one she left

  @needs-browser @real-io @nav-collapse-stored-choice
  Scenario Outline: The left nav follows the remembered choice, and only this app's own choice folds it
    # The whole finite domain of what the origin can hold: nothing, this app's own
    # choice, and four strings another tool, an extension or a later version could
    # leave behind. RED-trigger: an unrecognised string is applied verbatim, so the
    # operator gets a nav that matches no rule at all.
    Given the browser remembers <what> for the left nav
    When the operator opens the dashboard page
    Then the left nav renders <how>

    Examples:
      | what                  | how         |
      | nothing at all        | open        |
      | the choice "collapsed"| folded away |
      | the word "expanded"   | open        |
      | the word "Collapsed"  | open        |
      | the value "1"         | open        |
      | the value "null"      | open        |

  @needs-browser @real-io @nav-collapse-stored-choice
  Scenario: A mark another tool left with a value of its own still gives one of the two navs
    # RED-trigger: the rules are rewritten to select on the mark's VALUE, which
    # reopens a third nav width — neither folded nor open, and nothing an operator
    # could repair.
    Given the operator has the dashboard open at desk width with the left nav open
    When another tool stamps the page with a folded-away mark carrying a value of its own
    Then the left nav is exactly as narrow as the strip this app's own mark gives
    And the left nav and the page content still fill the width of the shell

  # ==========================================================================
  # The keyboard and what the control says
  # ==========================================================================

  @needs-browser @real-io @nav-collapse-keyboard
  Scenario Outline: The operator folds the left nav away from the keyboard alone
    # RED-trigger: the control is not a real button — a clickable span answers a
    # pointer and neither key, and takes the operator's focus nowhere.
    Given the operator has walked the keyboard focus to the nav control on the dashboard
    When she presses <key>
    Then the left nav is a narrow strip and the control still holds the focus, visibly

    Examples:
      | key   |
      | Enter |
      | Space |

  @needs-browser @real-io @nav-collapse-keyboard
  Scenario: The control says which way the left nav is set and what the next press does
    # RED-trigger: the name is written once at build time, so it describes the
    # state the nav USED to be in; or it is a bare glyph, which tells an operator
    # using assistive technology neither where she is nor what pressing will do.
    Given the operator has the dashboard open at desk width with the left nav open
    And the control reports the left nav open and names what the next press does
    When she folds the left nav away with its control
    Then the control reports the left nav folded away and names what the next press does
    And its wording is not the wording it carried before the press

  @needs-browser @real-io @nav-collapse-keyboard
  Scenario: A folded-away left nav leaves no hidden link in the keyboard order
    # RED-trigger: the items are hidden by width or by opacity, so they are
    # invisible and still focusable and still announced — the operator tabs into a
    # nav she cannot see.
    Given the operator has folded the left nav away on the dashboard
    When she walks the whole page with the Tab key
    Then no hidden nav item ever takes the keyboard focus
    And the monogram, the nav control and the theme control are still there, and the theme control still carries its name

  # ==========================================================================
  # When the mechanism is taken away
  # ==========================================================================

  @needs-browser @real-io @nav-collapse-degraded
  Scenario: With scripting off the left nav is open and there is no control at all
    # RED-trigger: the control is server-rendered, so with scripting off it sits
    # there looking live and does nothing — worse than not being there.
    Given a browser session with scripting switched off
    When the operator opens the dashboard page
    Then the left nav renders at its full width with every nav item usable
    And no control for the left nav exists in the document, and none can be reached with the keyboard

  @needs-browser @real-io @nav-collapse-degraded
  Scenario: With stored state refused the page still renders and nothing is reported
    # Driven on the sign-in screen by NECESSITY: refusing site data also refuses
    # the session cookie, so no signed-in screen exists under this condition.
    # RED-trigger: the first read of the remembered choice is unguarded, so the
    # script dies where it stands and takes the control down on every screen.
    Given a browser session whose browser refuses this site access to stored state
    When the operator opens the sign-in screen of this app
    Then the page renders, carries no folded-away mark, and reports no script error to the operator

  @needs-browser @real-io @nav-collapse-degraded
  Scenario: A screen with no left nav builds no control and reports nothing
    # The fifteen screens with no nav. RED-trigger: the script reaches into an
    # absent mount point and dies, which would be visible on every pre-auth screen
    # in the app.
    Given the browser remembers the folded-away choice from an earlier visit
    When the operator opens the sign-in screen of this app
    Then that screen offers no nav control, carries the mark no rule there consumes, and reports no script error

  # ==========================================================================
  # A phone, and a very long name
  # ==========================================================================

  @needs-browser @real-io @nav-collapse-phone
  Scenario Outline: A phone screen never scrolls sideways, whichever way the left nav is set
    # The promise is 480 px and below, so the boundary width itself and a width
    # well under it are both driven. RED-trigger: the folded-away rules reach
    # inside the phone's top-bar layout and apply a fixed rail width to a row,
    # which widens the document at every phone width.
    Given the browser remembers <what> for the left nav
    When the operator opens the dashboard on a phone <width> px wide
    Then the page does not scroll sideways, and the left nav still lays out as the top bar

    Examples:
      | what                   | width |
      | nothing at all         | 390   |
      | the choice "collapsed" | 390   |
      | nothing at all         | 480   |
      | the choice "collapsed" | 480   |
      | nothing at all         | 320   |
      | the choice "collapsed" | 320   |

  @needs-browser @real-io @nav-collapse-phone
  Scenario: On a phone the folded-away nav leaves the board columns scrolling inside their own strip
    # RED-trigger: the page takes the columns' overflow instead of the strip, so
    # the whole board page slides sideways under the operator's thumb.
    Given the browser remembers the folded-away choice from an earlier visit
    When the operator opens the project board on a phone 390 px wide
    Then that page does not scroll sideways
    And the board columns still scroll inside their own strip

  @needs-browser @real-io @nav-collapse-geometry
  Scenario: The longest workspace name the app allows never widens the folded-away nav
    # RED-trigger: the name is merely clipped rather than taken out of the layout,
    # so a long workspace name stretches the narrow strip and the page with it.
    # The length is the app's OWN limit, not an arbitrary one: workspace names are
    # refused above it, so this is the widest brand row an operator can create.
    Given the workspace is named with the longest name the app allows
    When the operator folds the left nav away on the dashboard
    Then the strip is still narrower than the open nav, with the workspace name not laid out at all
    And that page does not scroll sideways
