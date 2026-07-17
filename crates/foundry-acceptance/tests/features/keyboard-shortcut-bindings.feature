# Story: keyboard-shortcut-bindings — the missing client keyboard layer
# Feature-id: keyboard-shortcut-bindings   ·   Slices 01..05 (see docs/feature/…/slices/)
# JTBD: fast-keyboard-issue-flow. Personas are the repo's own harness identities
# (us_12_keyboard_nav.rs:58-64): Mei Tanaka (mei@acme.com, keyboard-first, Japanese IME)
# and Hiroshi Sato (hiroshi@acme.com, pointer user).
#
# ======================= THE HARD RULE (NFR-1 / D9) =======================
# Every scenario below asserts KEY-PRESSED -> USER-VISIBLE OUTCOME in a real
# browser and MUST FAIL on unmodified `main`. The shipped port-to-port suite
# (us_12_keyboard_nav.rs, reqwest+scraper) proves the SERVER contracts and is
# GREEN today while this feature is 100% ABSENT — it never presses a key. A
# scenario a port-to-port test could satisfy on `main` is, by construction, NOT
# a scenario for this feature.
#
# ONE deliberate exception (D15): the US-02 @paired-assertion guard scenario is a
# REVERT-REDS-IT regression guard, not a reds-on-`main` scenario. It first proves
# the shortcut fires OUTSIDE a text field (the layer is live) THEN that it does
# NOT fire inside one. The two halves live in ONE scenario. DISTILL/DELIVER MUST
# NOT split them — split, the guard half passes vacuously on any build where the
# key is unbound (including `main` and a broken half-shipped layer).
#
# ================= HARNESS / LANE (ADR-007 — the root-cause fix) =================
# These scenarios need a browser-capable driver; the reqwest+scraper harness
# cannot express them (ODD-9 — that gap IS the feature). DELIVER builds the
# @needs-browser lane: BrowserHarness = InProcHarness (unchanged, already serves a
# real TCP origin at base_url()) + a fantoccini session -> chromedriver. The lane
# is EXCLUDED from the fast loop but INCLUDED in `all` (= what `cargo xtask ci`
# runs); a missing/skewed chromedriver is a HARD FAILURE with an install hint —
# it probes then refuses, NEVER silently skips. See test-scenarios.md for the
# exact xtask + acceptance.rs wiring DELIVER must execute.
#
# EVERY scenario carries @pending, per-scenario (house convention, mirrors
# recipient-notification-preferences). @pending is excluded from EVERY lane
# (acceptance.rs `!has("pending")`), so this file keeps `cargo test` and the
# `all` lane GREEN until DELIVER unskips one slice at a time. DELIVER removes
# @pending from a slice's scenarios as it implements them.
# Two honest limits are named where they bite: the IME clause is SIMULATED via a
# JS-dispatched CompositionEvent (WebDriver send_keys cannot compose), and the
# copy chord asserts NON-ACTIVATION (clipboard is unassertable headless).
# ================================================================================

@kb @browser-observable
Feature: The seven advertised keyboard shortcuts work in the browser

  The help overlay advertises seven shortcuts — c, /, j, k, Enter, ?, Esc
  (SHORTCUTS, crates/foundry-app/src/keyboard.rs:48-56). None are bound: the
  entire client keyboard layer (static/js/keyboard.js) was never written. This
  feature binds them through one document-delegated, guarded dispatch layer, and
  builds the browser lane that would have caught the gap. Zero new routes, zero
  endpoints, zero migrations.

  Background:
    Given Mei is signed into a real browser on the "acme" workspace, a member of the "Backend" team
    And the "AUTH" project exists with issues AUTH-1, AUTH-2, AUTH-3 and AUTH-4
    And AUTH-2 is titled "Session cookie not cleared on sign-out"

  # ===========================================================================
  # SLICE 01 — the dispatch layer, the help overlay (US-01), Esc (US-07)
  # ADR-001 (vanilla IIFE, [data-kb-ready], drop Alpine) · ADR-003 (kb-overlay-root,
  # DOM-derived Esc stack, keep no-JS links). ADR-007 lands FIRST or nothing here
  # is assertable — the lane is the precondition for every slice.
  # ===========================================================================

  # THE WALKING-SKELETON-EQUIVALENT: not a product skeleton (server contracts are
  # shipped, D8) but the proof the INSTRUMENT works end-to-end. chromedriver up ->
  # InProcHarness served on its real port -> navigate signed-in -> [data-kb-ready]
  # appears -> the Secure-cookie-over-plain-HTTP probe (sign in, assert STILL
  # signed in — harness.rs:401-406 emits `Secure` over HTTP and a real browser,
  # unlike reqwest, may refuse it) -> press `?` -> the help overlay appears. If
  # this cannot be made to run, no other scenario is worth writing.
  @needs-browser @slice1 @us-01 @lane-probe @walking_skeleton @driving_port @real-io
  Scenario: The browser lane can drive a real key against the served app end to end
    Given the browser lane has started chromedriver and navigated to the AUTH board
    Then the page reports the keyboard layer is ready
    And Mei is still signed in after the browser accepts the session cookie over plain HTTP
    When Mei presses "?"
    Then the keyboard shortcut list appears as an overlay over the board

  @needs-browser @slice1 @us-01 @help
  Scenario: Pressing the help key shows the shortcut list over the current page
    Given Mei is viewing the AUTH project board
    When Mei presses "?"
    Then the keyboard shortcut list appears as an overlay over the board
    And the board is still visible behind it
    And the browser did not navigate away from the board

  # bound == advertised (BR-1, KPI-5): both derive from SHORTCUTS. @property here
  # is an EXAMPLE-BASED invariant litmus — layer-3 browser scenario, NOT a
  # PBT-generated one (Mandate 11: layer 3+ sad paths / properties stay example-only).
  @needs-browser @slice1 @us-01 @help @property @contract
  Scenario: The overlay lists exactly the seven advertised shortcuts and each is bound
    Given Mei has opened the help overlay on the AUTH board
    Then it lists a description for each of "c", "/", "j", "k", "Enter", "?" and "Esc"
    And every shortcut it lists is bound and does something
    And no shortcut outside that list is bound

  @needs-browser @slice1 @us-01 @help @global @edge
  Scenario: The help overlay is available away from the board where there is no modal mount
    Given Mei is viewing the dashboard, a page with no modal mount point
    When Mei presses "?"
    Then the keyboard shortcut list appears as an overlay over the dashboard

  @needs-browser @slice1 @us-01 @us-07 @escape
  Scenario: Dismissing the help returns Mei exactly where she was
    Given Mei has the help overlay open over the AUTH board
    When Mei presses "Esc"
    Then the help overlay closes
    And Mei is still on the AUTH board with nothing else changed

  # no-JS path (NFR-4, ODD-8, AC-X.3): with scripting OFF the overlay does not
  # exist and the sidebar full-page link is the only way to read the list. Driven
  # by launching the browser with JavaScript disabled.
  @needs-browser @slice1 @us-01 @no-js @property
  Scenario: With scripting disabled the full-page keyboard help still works
    Given scripting is disabled in Mei's browser on the AUTH board
    When Mei follows the sidebar "Keyboard shortcuts" link
    Then the full-page keyboard help is shown
    And no advertised action is reachable only by keyboard

  # ===========================================================================
  # SLICE 02 — the guards (US-02). ADR-002 (the crux). No new key is bound here;
  # capability is deliberately ZERO — the value is a harm avoided (typing works).
  # ===========================================================================

  # THE most important scenario in the feature (AC-02.2). PAIRED ASSERTION — the
  # two When/Then halves are ONE scenario ON PURPOSE (D15). The first proves the
  # layer is LIVE (press `c` outside a field -> modal opens); the second proves
  # the guard (type into the field -> only characters, nothing fires). Split, the
  # guard half is vacuously true on `main`. This is revert-reds-it: it reds when
  # the guard is removed from a BOUND layer, not when the layer is absent. The
  # [data-kb-ready] marker is the concrete "layer is live" hook. DO NOT SPLIT.
  @needs-browser @slice2 @us-02 @guard @critical @property @paired-assertion
  Scenario: No shortcut ever fires from a text-entry context, while still firing outside one
    Given Mei is viewing the AUTH board with no text field focused
    When Mei presses "c"
    Then the new-issue modal opens, proving the shortcut layer is live
    When Mei types the characters "cjk/?" into the title field
    Then exactly those characters are entered into the field
    And no additional modal opens, no search is focused, and no selection moves

  @needs-browser @slice2 @us-02 @guard @edge
  Scenario: Typing shortcut letters into a title inserts them instead of firing shortcuts
    Given Mei has the new-issue modal open on the AUTH board
    When Mei types "cache invalidation on login" into the title field
    Then the title field contains "cache invalidation on login"
    And no additional modal was opened
    And no card selection changed

  # Cmd+C / Ctrl+C reshaped for headless (ADR-007, upstream-changes §4): clipboard
  # contents are UNASSERTABLE headless, and Linux CI's copy chord is Ctrl not Meta.
  # So assert NON-ACTIVATION (no modal) + defaultPrevented === false for BOTH
  # modifiers — never "the text was copied".
  @needs-browser @slice2 @us-02 @guard @modifier @error
  Scenario: A copy chord does not create an issue and is left for the browser to handle
    Given Mei is viewing the AUTH board with the text "AUTH-2" selected on the page
    When Mei presses the copy chord with Ctrl and again with Cmd
    Then the new-issue modal does not open for either modifier
    And the keydown default was not prevented for either modifier

  @needs-browser @slice2 @us-02 @guard @shift
  Scenario: Shift is not a suppressor so the help key still fires
    Given Mei is viewing the AUTH board with no text field focused
    When Mei presses "?" which the browser produces as Shift and "/"
    Then the keyboard shortcut list appears as an overlay over the board

  # IME clause (ADR-002 guard 1, ADR-007 honest limit): WebDriver send_keys CANNOT
  # produce composition. This is SIMULATED by dispatching a CompositionEvent plus a
  # KeyboardEvent{isComposing:true, keyCode:229} for `c` via client.execute(). It
  # exercises our predicate truthfully (listeners fire for untrusted events) but is
  # NOT a real IME — a real-IME regression could still reach Mei (see @manual).
  @needs-browser @slice2 @us-02 @guard @ime @edge
  Scenario: A key delivered mid IME composition does not fire a shortcut
    Given Mei's Japanese IME is composing text in the title field
    When a "c" key arrives while composition is in progress
    Then no additional modal opens
    And the composing character is left to the input method

  # Unblocked by UI-3's ratification (deliver/upstream-issues.md, 2026-07-16):
  # guard 4's DOMAIN is narrowed to the keys a text-entry context can consume, so
  # "Esc" — which produces no character and which a text input does nothing with —
  # is dispatched from the autofocused title field and can close the modal. Not a
  # per-shortcut carve-out: the predicate never names "Escape", and there is still
  # exactly one chain evaluated once before dispatch is reachable.
  @needs-browser @slice2 @us-02 @guard
  Scenario: Leaving the text field re-enables the shortcuts immediately
    Given Mei has typed in the title field and then pressed "Esc" to leave it
    When Mei presses "c"
    Then the new-issue modal opens

  # ===========================================================================
  # SLICE 03 — c files an issue (US-03) + Esc closes the modal (US-07)
  # Reuses board.html:6's own hx-get (never reconstructed); ZERO client CSRF work
  # (keyboard.rs:94 mints it, new_issue_modal.html:4 carries it). If DELIVER writes
  # CSRF code here, something is wrong (DESIGN wave-decisions).
  # ===========================================================================

  @needs-browser @slice3 @us-03 @create
  Scenario: Pressing the create key opens the new-issue modal on the board
    Given Mei is viewing the AUTH project board
    When Mei presses "c"
    Then the new-issue modal opens over the board
    And the title field is focused and ready for typing

  @needs-browser @slice3 @us-03 @create
  Scenario: Mei files an issue entirely from the keyboard
    Given Mei has opened the new-issue modal by pressing "c"
    When Mei types "Session cookie not cleared on sign-out" and submits the form
    Then a new issue with that title appears on the AUTH board

  @needs-browser @slice3 @us-03 @create @scope @edge
  Scenario: The create key does nothing where there is no project
    Given Mei is viewing the dashboard, a page with no team or project
    When Mei presses "c"
    Then no modal opens
    And the browser does not navigate away

  @needs-browser @slice3 @us-03 @us-07 @escape
  Scenario: Escape closes the new-issue modal and returns to the board
    Given Mei has opened the new-issue modal by pressing "c"
    When Mei presses "Esc"
    Then the modal closes
    And Mei is back on the AUTH board with nothing else changed

  # ADR-003 layered-Esc proof — the scenario that reds if anyone collapses the two
  # hosts (#kb-overlay-root over #modal-root) back into one. Help must close OVER a
  # still-open modal.
  @needs-browser @slice3 @us-07 @escape @layered @critical
  Scenario: Escape closes one layer at a time, help before the modal beneath it
    Given Mei has the new-issue modal open and has pressed "?" to show the help overlay
    When Mei presses "Esc"
    Then the help overlay closes
    And the new-issue modal is still open
    When Mei presses "Esc" a second time
    Then the new-issue modal closes

  @needs-browser @slice3 @us-07 @escape @edge
  Scenario: Escape with nothing open is a harmless no-op
    Given Mei is viewing the AUTH board with no modal or overlay open
    When Mei presses "Esc"
    Then nothing happens
    And the browser does not navigate away

  # ===========================================================================
  # SLICE 04 — / reveals + focuses the board search panel (US-04)
  # ADR-005: board-only; JS-injected panel + a pointer-clickable control; `/`
  # preventDefault()s its own slash; shipped GET …/search?q= fragment honoured
  # as-is (exact-key, substring, data-empty).
  # ===========================================================================

  # The classic bug (FR-7): the field must be focused AND EMPTY — no stray "/".
  @needs-browser @slice4 @us-04 @search
  Scenario: Pressing the search key focuses the search box without typing a slash
    Given Mei is viewing the AUTH project board
    When Mei presses "/"
    Then the search input is focused
    And the search input is empty

  @needs-browser @slice4 @us-04 @search
  Scenario: Mei finds an issue by typing part of its title
    Given Mei has focused the board search box by pressing "/"
    When Mei types "session" into the search box
    Then the results list shows the issue "Session cookie not cleared on sign-out"

  @needs-browser @slice4 @us-04 @search @edge
  Scenario: Mei finds an issue by its exact key
    Given Mei has focused the board search box by pressing "/"
    When Mei types "AUTH-2" into the search box
    Then the results list shows exactly the issue AUTH-2

  @needs-browser @slice4 @us-04 @search @error
  Scenario: A search that matches nothing shows the empty state
    Given Mei has focused the board search box by pressing "/"
    When Mei types "zzz" into the search box
    Then the results list shows an empty state indicating nothing matched

  @needs-browser @slice4 @us-04 @search @guard @edge
  Scenario: A slash typed into the focused search box is inserted literally
    Given Mei has focused the board search box by pressing "/"
    When Mei types "and/or" into the search box
    Then the search box contains "and/or"
    And search focus was not grabbed again

  @needs-browser @slice4 @us-04 @search @us-07 @escape
  Scenario: Escape leaves search and restores the board
    Given Mei has focused the board search box by pressing "/" and typed a query
    When Mei presses "Esc"
    Then the search panel closes and the board is restored

  # ===========================================================================
  # SLICE 05 — j/k walk the VISIBLE cards (US-05) + Enter opens (US-06)
  # ADR-004 (selection is a KEY) · ADR-005 (modal navigation, Enter-via-board-card)
  # · ADR-006 (aria-activedescendant composite) · ADR-008 (retire #kb-items).
  # ===========================================================================

  @needs-browser @slice5 @us-05 @selection
  Scenario: The next key selects the first visible card and highlights it
    Given Mei is viewing the AUTH board showing issues AUTH-3, AUTH-2 and AUTH-1
    When Mei presses "j"
    Then the first visible card is highlighted as selected

  # Selection follows the EYES, not the retired #kb-items ASC-by-number carrier —
  # the visible board is column-grouped and DESC-within-column (ADR-008).
  @needs-browser @slice5 @us-05 @selection @kb-items-collision
  Scenario: Next and previous walk the cards in the order Mei sees them
    Given Mei has selected the first visible card on the AUTH board
    When Mei presses "j" and then "k"
    Then the selection moves to the second visible card and back to the first
    And the selection order matches the order the cards appear on screen

  # scrollIntoView (AC-05.3) — deterministic only under a FIXED window size (ADR-007).
  @needs-browser @slice5 @us-05 @selection @edge
  Scenario: A selection below the fold scrolls into view
    Given Mei is viewing the AUTH board with more cards than fit on screen
    When Mei presses "j" repeatedly until the selection passes the bottom of the viewport
    Then the selected card is scrolled into view and its highlight is visible

  @needs-browser @slice5 @us-05 @selection @edge
  Scenario: Moving previous from the first card stays put
    Given Mei has the first visible card selected
    When Mei presses "k"
    Then the first card remains selected
    And no error occurs

  # ADR-004 index-vs-key proof: drag moves the same NODE; the ring rides the node
  # by key, not by slot. Reds if anyone switches selection to an index. board-dnd.js
  # is untouched (NFR-8).
  @needs-browser @slice5 @us-05 @selection @drag-coexistence @edge
  Scenario: Dragging the selected card to another column leaves selection coherent
    Given Mei has selected AUTH-2 on the AUTH board
    When Hiroshi drags AUTH-2 into another column with the mouse
    Then the drag completes as it does today
    And the ring is still on AUTH-2, not on whatever now occupies the old slot

  # a11y (ADR-006, AC-05.7): aria-activedescendant on a focusable composite; the
  # ring never relies on colour alone. KPI-4 is met CONDITIONALLY — "once the board
  # is focused" (the AT user Tabs to the board once). Slice 05 also owes the help
  # copy the instruction "Tab to the board, then j/k".
  @needs-browser @slice5 @us-05 @a11y
  Scenario: Once the board is focused, moving the selection is exposed to assistive technology
    Given Mei has Tabbed to focus the AUTH board as a screen-reader user would
    When Mei presses "j" to move the selection to AUTH-2
    Then the board's active descendant is the AUTH-2 card
    And the AUTH-2 card is marked selected for assistive technology
    And the selection highlight does not rely on colour alone
    And the help overlay tells the user to Tab to the board, then press "j" or "k"

  @needs-browser @slice5 @us-06 @open
  Scenario: Pressing enter opens the selected issue
    Given Mei is viewing the AUTH board and has selected AUTH-2 with the "j" key
    When Mei presses "Enter"
    Then the issue modal for AUTH-2 opens over the board

  @needs-browser @slice5 @us-06 @open @edge
  Scenario: Enter with nothing selected does nothing
    Given Mei is viewing the AUTH board and has not selected any card
    When Mei presses "Enter"
    Then no modal opens
    And the browser does not navigate away

  @needs-browser @slice5 @us-06 @open @guard
  Scenario: Enter inside a form still submits the form
    Given Mei has the new-issue modal open with a title typed into it
    When Mei presses "Enter" in the title field
    Then the form is submitted
    And no issue card is opened behind the modal

  @needs-browser @slice5 @us-06 @us-07 @selection
  Scenario: Closing the opened issue leaves the selection intact
    Given Mei has opened AUTH-2 by pressing "Enter"
    When Mei presses "Esc"
    Then the modal closes
    And AUTH-2 is still selected so "j" moves to the next card

  # ADR-005 one-open-path: `/` -> AUTH-2 -> j -> Enter opens THE SAME modal a
  # pointer click on AUTH-2's board card produces. Search-result rows carry NO
  # hx-get (search_results.html:4), so Enter resolves selectedKey -> the board card.
  #
  # RESOLVED (UI-7, ratified 2026-07-16). This was blocked at 05-04: `/` focuses the
  # search box, so `j` was delivered to a TEXT-ENTRY CONTEXT and ADR-002 guard 4 made
  # it inert — the box read "AUTH-2j" and the press never reached the dispatch table.
  # ADR-005 §3 and guard 4 could not both hold. The resolution leaves guard 4 UNTOUCHED
  # and reaches the results by an explicit Tab out of the box (the cost ADR-006 already
  # ratified for the board, and which the help overlay states). Blur-on-arrival was
  # tried first and rejected: at human typing speed it strands the query at one
  # character — and the batched lane ran GREEN over that defect, which is why the probe
  # now types at 150ms/char. See deliver/upstream-issues.md UI-7.
  @needs-browser @slice5 @us-06 @open @one-open-path @critical
  Scenario: Enter from the search results opens the same modal as clicking the board card
    Given Mei has searched the board for "AUTH-2" and selected the result with "j"
    When Mei presses "Enter"
    Then the modal that opens is the same one a pointer click on the AUTH-2 board card produces

  # UI-7's ratified cost, written down where the USER can find it. `/` leaves the
  # caret in the search box, so guard 4 correctly holds j/k/Enter until focus leaves
  # it — `Tab` is the way out. Mei cannot GUESS that, and a results list that
  # silently ignores `j` is indistinguishable from one that is not navigable at all,
  # which is the exact disease this feature exists to cure. Same mechanism and same
  # source of truth as ADR-006's board Tab (keyboard.rs SELECTION_INSTRUCTION).
  @needs-browser @slice5 @us-06 @open @a11y @help @discoverability
  Scenario: The help overlay says how to reach the search results
    Given Mei is viewing the AUTH project board
    When Mei presses "?"
    Then the help overlay tells Mei to press Tab from the search box to reach the results

  # ADR-005 named edge: the board renders only {backlog,todo,in_progress,done};
  # search returns every issue. An issue in another state is findable but has NO
  # card -> Enter is a no-op (consistent with "no selection => no-op", FR-9).
  #
  # RESOLVED (UI-7). Its own Given ("AUTH-9 exists in a state the board does not
  # display") was always green: AUTH-9 seeds in `cancelled`, search finds it, the board
  # renders no card for it. Only the SHARED "selected the result with j" Given was
  # unreachable, which UI-7's Tab resolution fixed.
  @needs-browser @slice5 @us-06 @open @edge @named-edge
  Scenario: Enter is a no-op for a found issue that the board does not render
    Given AUTH-9 exists in a state the board does not display
    And Mei has searched the board for "AUTH-9" and selected the result with "j"
    When Mei presses "Enter"
    Then no modal opens
    And the browser does not navigate away

  # ===========================================================================
  # CROSS-CUTTING — delegation survives htmx swaps (AC-X.5), and the #kb-items
  # retirement regression (AC-05.6, ADR-008 incl. the trap-B vacuity guard).
  # ===========================================================================

  # ADR-004 key-survives-swap + document-delegation (NFR-6): filing via `c` swaps
  # #modal-root and re-renders cards; j/Enter still work with no reload.
  @needs-browser @slice5 @property @htmx-swap
  Scenario: Shortcuts keep working after the page content is swapped
    Given Mei has filed an issue by pressing "c" and submitting the form
    When Mei presses "j" and then "Enter"
    Then the selection moves and the selected issue opens
    And no page reload was required

  # NOT a browser scenario — a SOURCE-TREE litmus (ADR-008, AC-05.6). Reds on
  # `main` (the carrier is present). DELIVER implements it as a filesystem grep
  # over crates/ (or a `cargo xtask check-arch` litmus), NOT via the browser, so it
  # carries no @needs-browser tag. It also guards TRAP B: projects.rs:1110's
  # `html.split("id=\"kb-items\"").next()` must be repointed at the full HTML, or
  # `each_issue_lands_in_exactly_its_state_column` passes vacuously.
  @slice5 @us-05 @kb-items-retirement @grep-litmus @real-io
  Scenario: The hidden keyboard-navigation carrier is gone from the source tree
    Given the retirement of the "#kb-items" carrier has landed
    Then a search for "kb-items" or "kb_items" under crates returns zero hits
    And no test slices the board HTML on the string "id=\"kb-items\"" before asserting column placement

  # ===========================================================================
  # MANUAL — the two honest limits the browser lane can only SIMULATE (ADR-007).
  # Supersedes the retired us-12-keyboard-nav.feature @manual drill. Excluded by
  # every lane (@manual + @pending). Paste into release-checklist.md.
  # ===========================================================================

  @pending @manual @us-02 @us-05 @ime @a11y
  Scenario: Manual UAT — a real IME and a real screen reader, the substrates CI cannot drive
    # 1. With a REAL Japanese IME: open the new-issue modal (c), begin composing a
    #    word starting with "c" (e.g. "ちゃんと"). Confirm NO second modal opens and
    #    the composed text lands in the title — the automated @ime scenario only
    #    SIMULATES this via a dispatched CompositionEvent.
    # 2. With a REAL screen reader (NVDA/JAWS) in browse mode: land on the AUTH
    #    board, Tab once to focus it, then press j/k. Confirm each move announces the
    #    selected issue's key + title, and that j/k did nothing BEFORE the Tab
    #    (browse-mode interception — the ADR-006 accepted cost).
    Given a human reviewer has a real IME and a real screen reader
    When the reviewer follows the documented keyboard drill
    Then the reviewer signs off on IME composition and screen-reader announcement for this release
