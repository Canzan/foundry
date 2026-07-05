# Acceptance Criteria (Given/When/Then) — issue-edit-dialog

Driving port: the browser (signed-in session) on a project board + the store (integration) for the new
update method. As with board-new-issue, the HTTP acceptance suite (reqwest + scraper) pins the WIRING (card
`hx-get`, the edit-dialog fragment, the save endpoint contract), the SAVE end-to-end at the store/endpoint
level, tenancy/CSRF/validation, and the no-JS fallback; the live click→dialog→save→card-updates interaction
is browser-dogfooded. DESIGN finalizes endpoint verbs (ODD-1) and the save-response swap (ODD-2).

```gherkin
Background:
  Given a workspace "Acme" with a member "Mei" on team "Backend"
  And a project "Sandbox" (key "GEN") with an issue "GEN-1" titled "Old title" described "old body"
  And Mei is signed in and viewing the "Sandbox" board

Scenario: The card is wired to open the edit dialog
  When Mei fetches the board
  Then the "GEN-1" card carries an hx-get to the issue-edit endpoint targeting the modal container

Scenario: The edit dialog is pre-filled with the issue's current values
  When Mei opens the edit dialog for "GEN-1"
  Then the dialog title field contains "Old title"
  And the dialog description field contains "old body"
  And the form carries the hidden "_csrf" field

Scenario: Saving edited title + description updates the issue and the board card
  Given the edit dialog for "GEN-1" is open
  When Mei saves the dialog with title "New title" and description "new body"
  Then the issue "GEN-1" now has title "New title" and description "new body" in the store
  And the response replaces the "GEN-1" board card in place, now showing "New title"
  And the dialog is dismissed
  And the page did not fully navigate

Scenario: An empty title is rejected in the dialog
  Given the edit dialog for "GEN-1" is open
  When Mei saves the dialog with an empty title
  Then the response is the "Title is required" error rendered in the dialog
  And "GEN-1" still has its previous title in the store

Scenario: Editing a foreign issue is refused non-enumerably
  Given an issue "GEN-9" exists in a DIFFERENT workspace
  When Mei requests the edit dialog for that issue's path
  Then the response is the uniform not-found page (no echoed title)
  And no update is possible

Scenario: No-JS fallback saves the edit
  Given htmx is unavailable
  When Mei submits the edit form for "GEN-1" as a plain form with title "Plain edit"
  Then the issue is updated and the board reloads showing "Plain edit" on the card
```

## Store-integration scenarios (foundry-store)

| Scenario | Assertion |
|----------|-----------|
| `update_issue_details_with_outbox` persists both fields | title + description_md updated, `updated_at` bumped |
| tenant isolation | an issue in another workspace is not updated by a scoped call |
| validation bounds | title 1–256 enforced (or enforced at the service layer — DESIGN decides where) |
