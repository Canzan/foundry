# Journey: Keyboard Navigation — the seven advertised shortcuts, actually bound.
#
# Feature: keyboard-shortcut-bindings. Personas are the repo's own harness identities
# (us_12_keyboard_nav.rs:58-64): Mei Tanaka (mei@acme.com), Hiroshi Sato (hiroshi@acme.com).
#
# ============================ THE HARD RULE (NFR-1) ============================
# EVERY scenario below MUST FAIL on unmodified `main`.
#
# The shipped acceptance suite (crates/foundry-acceptance/src/steps/us_12_keyboard_nav.rs)
# is PORT-TO-PORT: it drives HTTP and parses HTML with `scraper`. It asserts the SERVER
# contracts the client would call, and it NEVER PRESSES A KEY — nothing in the harness can.
# That is why `GET /keyboard-help` is green while pressing `?` does nothing at all.
#
# Therefore every scenario here asserts KEY-PRESSED -> USER-OBSERVABLE OUTCOME in a real
# browser/DOM. A scenario that a port-to-port test could satisfy on unmodified `main` is,
# by construction, NOT a scenario for this feature and is rejected on sight.
#
# DISTILL note: these scenarios need a browser-capable driver. The existing reqwest+scraper
# harness CANNOT express them — that gap is the feature, not an inconvenience (see ODD-9).
# ==============================================================================

@keyboard @browser-observable
Feature: The seven advertised keyboard shortcuts work in the browser

  The help overlay advertises seven shortcuts — c, /, j, k, Enter, ?, Esc
  (SHORTCUTS, crates/foundry-app/src/keyboard.rs:48-56). None of them are bound.
  The entire client-side keyboard layer was never written. Mei reads the promise,
  presses c, and nothing happens.

  Background:
    Given Mei is signed in to the "acme" workspace
    And Mei is a member of the "acme" team
    And the "AUTH" project exists with issues AUTH-1, AUTH-2, AUTH-3 and AUTH-4
    And AUTH-2 is titled "Session cookie not cleared on sign-out"

  # ===========================================================================
  # US-01 — ? shows the shortcut list as an overlay, right where Mei is (K3)
  # ===========================================================================

  @us-01 @help @global
  Scenario: Pressing the help key shows the shortcut list over the current page
    Given Mei is viewing the AUTH project board
    When Mei presses "?"
    Then the keyboard shortcut list appears as an overlay over the board
    And the board is still visible behind it
    And the browser did not navigate away from the board

  @us-01 @help
  Scenario: The help overlay lists every advertised shortcut
    Given Mei has opened the help overlay
    When Mei reads it
    Then it lists a description for each of "c", "/", "j", "k", "Enter", "?" and "Esc"

  @us-01 @help @global
  Scenario: The help overlay is available away from the board
    Given Mei is viewing the dashboard
    When Mei presses "?"
    Then the keyboard shortcut list appears as an overlay over the dashboard

  @us-01 @help
  Scenario: Dismissing the help returns Mei exactly where she was
    Given Mei has the help overlay open over the AUTH board
    When Mei presses "Esc"
    Then the help overlay closes
    And Mei is still on the AUTH board with nothing else changed

  # ===========================================================================
  # US-02 — the guards (K2). THE CLIFF: a layer that eats keystrokes is
  # STRICTLY WORSE than no layer at all. This is the highest-risk requirement.
  # ===========================================================================

  @us-02 @guard @critical
  Scenario: Typing shortcut letters into a title inserts them instead of firing shortcuts
    Given Mei has the new-issue modal open on the AUTH board
    When Mei types "cache invalidation on login" into the title field
    Then the title field contains "cache invalidation on login"
    And no additional modal was opened
    And no card selection changed

  # PAIRED ASSERTION — do NOT split these halves into separate scenarios.
  # "typing c opens no modal" is trivially true on main (nothing is bound) and would
  # ALSO pass on a build that binds without guarding. The first Then proves the layer
  # is LIVE; the second proves the guard suppresses it. Only both together have teeth.
  # This is a revert-reds-it regression guard, not a reds-on-main scenario.
  @us-02 @guard @critical @property @paired-assertion
  Scenario: No shortcut ever fires from a text-entry context, while still firing outside one
    Given Mei is viewing the AUTH board with no text field focused
    When Mei presses "c"
    Then the new-issue modal opens, proving the shortcut layer is live
    When Mei types the characters "cjk/?" into the title field
    Then exactly those characters are entered into the field
    And no additional modal opens, no search is focused, and no selection moves

  @us-02 @guard @critical
  Scenario: A copy chord copies instead of creating an issue
    Given Mei is viewing the AUTH board with the text "AUTH-2" selected
    When Mei presses "Cmd+C"
    Then the text is copied
    And the new-issue modal does not open

  @us-02 @guard
  Scenario: Shortcuts work again once Mei leaves the text field
    Given Mei has finished typing in the title field and has left it
    When Mei presses "c"
    Then the new-issue modal opens

  # ===========================================================================
  # US-03 — c files an issue without the mouse (A1)
  # ===========================================================================

  @us-03 @create
  Scenario: Pressing the create key opens the new-issue modal on the board
    Given Mei is viewing the AUTH project board
    When Mei presses "c"
    Then the new-issue modal opens over the board
    And the title field is focused and ready for typing

  @us-03 @create
  Scenario: Mei files an issue entirely from the keyboard
    Given Mei has opened the new-issue modal by pressing "c"
    When Mei types "Session cookie not cleared on sign-out" and submits the form
    Then a new issue with that title appears on the AUTH board

  @us-03 @create @scope
  Scenario: The create key does nothing where there is no project
    Given Mei is viewing the dashboard
    When Mei presses "c"
    Then no modal opens
    And the browser does not navigate away

  @us-03 @no-js
  Scenario: Filing without a mouse leaves the no-JS path working
    Given scripting is disabled in Mei's browser
    When Mei activates the "New issue" button on the AUTH board
    Then the full-page new-issue form is shown
    And submitting it creates the issue

  # ===========================================================================
  # US-04 — / searches without typing a slash (A2)
  # ===========================================================================

  @us-04 @search
  Scenario: Pressing the search key focuses the search box without typing a slash
    Given Mei is viewing the AUTH project board
    When Mei presses "/"
    Then the search input is focused
    And the search input is empty

  @us-04 @search
  Scenario: Mei finds an issue by typing part of its title
    Given Mei has focused the search box by pressing "/"
    When Mei types "session"
    Then the results list shows the issue "Session cookie not cleared on sign-out"

  @us-04 @search
  Scenario: Mei finds an issue by its exact key
    Given Mei has focused the search box by pressing "/"
    When Mei types "AUTH-2"
    Then the results list shows exactly the issue AUTH-2

  @us-04 @search
  Scenario: A search that matches nothing says so
    Given Mei has focused the search box by pressing "/"
    When Mei types "zzz"
    Then the results list shows an empty state indicating nothing matched

  # ===========================================================================
  # US-05 — j/k walk the VISIBLE cards (M1).
  # NOTE: this RETIRES the hidden #kb-items carrier (board.html:12) and deletes
  # its two currently-GREEN assertions (us_12_keyboard_nav.rs:334-360,
  # feature_b_web_tier.rs:568-572). Deliberate — see ODD-1 / Risk R1.
  # ===========================================================================

  @us-05 @selection
  Scenario: The next key selects the first visible card and highlights it
    Given Mei is viewing the AUTH board showing issues AUTH-3, AUTH-2 and AUTH-1
    When Mei presses "j"
    Then the first visible card is highlighted as selected

  @us-05 @selection @kb-items-collision
  Scenario: Next and previous walk the cards in the order Mei sees them
    Given Mei has selected the first visible card on the AUTH board
    When Mei presses "j" and then "k"
    Then the selection moves to the second visible card and back to the first
    And the selection order matches the order the cards appear on screen

  @us-05 @selection
  Scenario: A selection below the fold scrolls into view
    Given Mei is viewing the AUTH board with more cards than fit on screen
    When Mei presses "j" repeatedly until the selection passes the bottom of the viewport
    Then the selected card is scrolled into view and its highlight is visible

  @us-05 @selection
  Scenario: Moving previous from the first card stays put
    Given Mei has the first visible card selected
    When Mei presses "k"
    Then the first card remains selected
    And no error occurs

  @us-05 @selection @drag-coexistence
  Scenario: Dragging a card with the mouse leaves selection coherent
    Given Mei has a card selected on the AUTH board
    When Hiroshi drags that card into another column with the mouse
    Then the drag completes as it does today
    And no stale highlight is left behind

  @us-05 @selection @a11y
  Scenario: Moving the selection is announced to assistive technology
    Given Mei is using a screen reader on the AUTH board
    When Mei presses "j" to move the selection to AUTH-2
    Then the newly selected issue is announced
    And the highlight does not rely on colour alone

  # ===========================================================================
  # US-06 — Enter opens the selected issue (M2)
  # ===========================================================================

  @us-06 @open
  Scenario: Pressing enter opens the selected issue
    Given Mei is viewing the AUTH board and has selected AUTH-2 with the "j" key
    When Mei presses "Enter"
    Then the issue modal for AUTH-2 opens over the board

  @us-06 @open
  Scenario: Enter with nothing selected does nothing
    Given Mei is viewing the AUTH board and has not selected any card
    When Mei presses "Enter"
    Then no modal opens
    And the browser does not navigate away

  @us-06 @open @guard
  Scenario: Enter inside a form still submits the form
    Given Mei has the new-issue modal open with a title typed into it
    When Mei presses "Enter" in the title field
    Then the form is submitted
    And no issue card is opened behind the modal

  @us-06 @open @selection
  Scenario: Closing the opened issue leaves the selection intact
    Given Mei has opened AUTH-2 by pressing "Enter"
    When Mei presses "Esc"
    Then the modal closes
    And AUTH-2 is still selected on the board

  # ===========================================================================
  # US-07 — Esc gets Mei out of anything (K4). Esc is what makes the other six
  # safe to press: every one of them opens something.
  # ===========================================================================

  @us-07 @escape
  Scenario: Escape closes the new-issue modal and returns to the board
    Given Mei has opened the new-issue modal by pressing "c"
    When Mei presses "Esc"
    Then the modal closes
    And Mei is back on the AUTH board with nothing else changed

  @us-07 @escape
  Scenario: Escape closes one layer at a time
    Given Mei has the new-issue modal open and has pressed "?" to show the help overlay
    When Mei presses "Esc"
    Then the help overlay closes
    And the new-issue modal is still open

  @us-07 @escape
  Scenario: Escape with nothing open does nothing
    Given Mei is viewing the AUTH board with no modal or overlay open
    When Mei presses "Esc"
    Then nothing happens
    And the browser does not navigate away

  @us-07 @escape @selection
  Scenario: Escape does not throw away the selection
    Given Mei has selected AUTH-2 and opened it by pressing "Enter"
    When Mei presses "Esc"
    Then the modal closes
    And AUTH-2 is still selected so "j" moves to the next card

  @us-07 @escape @search
  Scenario: Escape leaves search and restores the board
    Given Mei has focused search by pressing "/" and typed a query
    When Mei presses "Esc"
    Then search closes and the board is restored

  # ===========================================================================
  # Cross-cutting properties
  # ===========================================================================

  @property @contract
  Scenario: The bound shortcuts are exactly the advertised ones
    Given Mei is viewing the AUTH project board
    When Mei consults the shortcut help
    Then every shortcut it lists is bound and does something
    And no shortcut outside that list is bound

  @property @no-js
  Scenario: With scripting disabled the app behaves exactly as it does today
    Given scripting is disabled in Mei's browser
    When Mei uses the AUTH board and the sidebar "Keyboard shortcuts" link
    Then the full-page keyboard help is shown
    And the "New issue" button still opens the full-page form
    And no advertised action is reachable only by keyboard

  @property @htmx-swap
  Scenario: Shortcuts keep working after the page content is swapped
    Given Mei has filed an issue by pressing "c" and submitting the form
    When Mei presses "j" and then "Enter"
    Then the selection moves and the selected issue opens
    And no page reload was required
