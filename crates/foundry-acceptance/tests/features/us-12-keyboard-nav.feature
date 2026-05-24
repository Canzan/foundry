# Story: US-12 — User navigates with keyboard shortcuts
# Slice: 2 (Realtime collaboration)
# JTBD: outcome-4 (Keyboard-driven flow is *the* Linear differentiator)
#
# Driving ports (the server-side contracts the alpine.js handlers depend
# on):
#   - GET  /team/{team}/project/{project}/issues/new   — htmx modal fragment
#   - GET  /team/{team}/project/{project}/search?q=... — htmx search fragment
#   - GET  /keyboard-help                              — shortcut overlay
#   - GET  /team/{team}/project/{project}              — project board page
#     (must expose data-issue-key attributes the alpine.js j/k/Enter
#     handler walks)
#
# Driven adapters exercised:
#   - real Postgres (issues, projects)
#   - real askama template rendering with data-attribute injection
#
# Pure browser interaction (the actual `c` / `/` / `j` / `k` / `Enter` /
# `?` key handling, the modal focus management, the highlight after a
# realtime swap) lives in alpine.js and is OUT of automated scope per
# the JTBD-backend-MVP no-Playwright decision. The @manual scenario at
# the bottom is the QA drill script (precedent: US-01's @manual entry
# in slice 1).

@slice2 @us-12 @keyboard
Feature: A team member's keyboard shortcuts are backed by stable server contracts
  Linear-feel keyboard shortcuts (`c` create, `/` search, `j`/`k` navigate,
  `Enter` open, `?` help) are implemented in alpine.js on the client. The
  server's responsibility is to provide the endpoints those handlers call
  and to render data attributes the handlers walk. This feature pins those
  server contracts so the client-side wiring cannot silently rot.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"
    And a project "Auth v2" with key prefix "AUTH" exists in the "Backend" team
    And the "Auth v2" project already has issues AUTH-1 through AUTH-3
    And Mei is signed in

  # Server-contract scenario (NOT a walking skeleton): the alpine.js
  # j/k handler reads `data-issue-key` from the DOM. This scenario pins
  # that the server emits the attribute; the actual user-keyboard-to-
  # selection journey is the @manual browser drill below (lines 83-).
  @real-io @driving_adapter
  Scenario: Project board markup carries the data-issue-key attribute that the j/k navigation handler walks
    When Mei opens the project board for "Auth v2"
    Then the rendered page contains an element with attribute data-issue-key="AUTH-1"
    And the rendered page contains an element with attribute data-issue-key="AUTH-2"
    And the rendered page contains an element with attribute data-issue-key="AUTH-3"
    And the data-issue-key elements appear in the document in ascending issue-number order

  @real-io
  Scenario: The new-issue modal endpoint returns a modal-shaped fragment when called as an htmx request
    When Mei requests the new-issue modal for "Auth v2" as an htmx request
    Then the response is an htmx fragment containing a form posting to "/team/backend/project/auth-v2/issues"
    And the response is not a full HTML page
    And the response contains an input named "title"
    And the response marks the title input as autofocused

  @real-io
  Scenario: The search endpoint filters issues by title substring as an htmx fragment
    Given the "Auth v2" project already has an issue titled "Refresh token rotation broken on Safari"
    And the "Auth v2" project already has an issue titled "OIDC support for v0.3"
    When Mei searches "Auth v2" for the query "refresh"
    Then the response is an htmx fragment
    And the response lists exactly one matching issue whose title contains "Refresh token rotation"
    And the response does NOT list the issue titled "OIDC support for v0.3"

  @real-io
  Scenario: The search endpoint matches an issue by its exact key
    When Mei searches "Auth v2" for the query "AUTH-2"
    Then the response is an htmx fragment
    And the response lists exactly one matching issue whose key is "AUTH-2"

  @real-io
  Scenario: The keyboard-help overlay enumerates every shortcut that ships in MVP
    When Mei requests the keyboard-help overlay
    Then the response is a valid HTML fragment
    And the response describes the "c" shortcut as "Create issue"
    And the response describes the "/" shortcut as "Search"
    And the response describes the "j" shortcut as "Next"
    And the response describes the "k" shortcut as "Previous"
    And the response describes the "Enter" shortcut as "Open selected"
    And the response describes the "?" shortcut as "Show this help"
    And the response describes the "Esc" shortcut as "Close modal"

  @manual @us-12
  Scenario: Manual UAT — full keyboard-driven create flow in a real browser
    # This scenario is not automated. The backend contracts above pin
    # the server side; the alpine.js handlers are exercised manually
    # before each release. Precedent: US-01's @manual entry.
    #
    # Drill script (paste into release-checklist.md):
    # 1. Sign in as Mei.
    # 2. Visit the Auth v2 project board.
    # 3. Press `?` — confirm the help overlay opens listing every shortcut.
    # 4. Press `Esc` — confirm the overlay closes and focus returns to the board.
    # 5. Press `c` — confirm the new-issue modal opens with focus in the title field.
    # 6. Type "Manual UAT smoke" and press Cmd-Enter (Ctrl-Enter on Linux).
    # 7. Confirm the modal closes and the new card appears in the Backlog column.
    # 8. Press `j` — confirm the first card gains a selection ring.
    # 9. Press `j` twice more — confirm selection moves down two cards.
    # 10. Press `k` — confirm selection moves up one card.
    # 11. Press `Enter` — confirm the selected issue's detail page opens.
    # 12. Go back. Press `/` — confirm the search input gains focus.
    # 13. Type "smoke" and confirm the matching issue from step 6 appears.
    Given a human reviewer is performing the keyboard-drill checklist
    When the reviewer follows the documented steps
    Then the reviewer signs off on the keyboard-shortcut behaviour for this release
