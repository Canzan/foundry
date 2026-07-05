# Acceptance Criteria (Given/When/Then) — board-new-issue

Driving port: the browser (signed-in session) on a project board. The create endpoints are already
port-tested (`us-08-file-issue`, `us-12-keyboard-nav`); these scenarios cover the **button interaction** that
was never wired. DELIVER authors the step glue driving a real htmx-capable flow (or asserting the wired
attributes + the shipped OOB contract, per the harness's htmx capability).

```gherkin
Background:
  Given a workspace "Acme" with a member "Mei" on team "Backend"
  And a project "Sandbox" with key prefix "GEN" exists under "Backend"
  And Mei is signed in and viewing the "Sandbox" board

Scenario: The New issue button opens the modal
  When Mei activates the "New issue" button
  Then a GET for the new-issue modal is issued
  And the modal shows a title field and a "Create" button

Scenario: Filing a titled issue drops a card into Backlog and closes the modal
  Given the new-issue modal is open
  When Mei submits the modal with title "Wire the button"
  Then a POST to the issues collection is issued carrying the _csrf token
  And the response is an out-of-band card appended to the "backlog" column
  And the new card shows the key "GEN-1" and the title "Wire the button"
  And the modal is dismissed
  And the page did not fully navigate

Scenario: An empty title is rejected inside the open modal
  Given the new-issue modal is open
  When Mei submits the modal with an empty title
  Then the response is the "Title is required" error rendered inside the modal
  And no card is added to any column
  And the board is not replaced

Scenario: No-JS fallback still files the issue
  Given htmx is unavailable
  When Mei submits the modal form with title "Fallback works"
  Then the plain POST creates the issue
  And the board reloads showing "Fallback works" in Backlog
```

## Wiring assertions (source-level, complementary)

- The "New issue" button carries `hx-get` to `…/issues/new` and targets a modal container.
- The modal fragment's form carries `hx-post` to its `action` (so submit is an htmx request) and keeps the
  hidden `_csrf`.
- A modal container element exists in `board.html` for the modal fragment to swap into.
