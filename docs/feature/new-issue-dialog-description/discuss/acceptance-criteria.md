# Acceptance Criteria — new-issue-dialog-description

Given/When/Then form for the DISTILL wave. `P1` = workspace member in a project; `P2` = machine-token client.

## US-01 — Description on the new-issue dialog (web)

```gherkin
Scenario: The new-issue dialog offers a Description field
  Given P1 is a member of team "acme" project "gen"
  When P1 opens the new-issue dialog
  Then the dialog shows a title input and a "description" textarea
  And the textarea is empty

Scenario: Filing an issue with a description persists it
  Given P1 has the new-issue dialog open
  When P1 submits title "Rate limit the gateway" and description "Return 429 with Retry-After."
  Then the issue is created in "backlog"
  And its description_md is "Return 429 with Retry-After."
  And the new card appears in the Backlog column
  And the dialog closes without a full-page navigation

Scenario: Description round-trips to the edit dialog
  Given P1 filed an issue with description "Return 429 with Retry-After."
  When P1 opens that issue's edit dialog
  Then the description textarea contains "Return 429 with Retry-After."

Scenario: Description is optional
  Given P1 has the new-issue dialog open
  When P1 submits a title and leaves the description empty
  Then the issue is created
  And its description_md is ""

Scenario: A typed description survives a title validation error
  # DESIGN-verified: htmx 2.0.4 does not swap the 400, so the modal + input persist for free.
  # The error message is NOT shown in-browser (pre-existing app-wide defect, deferred).
  Given P1 has the new-issue dialog open
  When P1 submits an empty title and description "Some detail I typed"
  Then the create is rejected with the "Title is required" error fragment (HTTP 400)
  And the open modal is not swapped, so the description field still contains "Some detail I typed"
  And no issue is created

Scenario: No-JS fallback creates with a description
  Given P1 has JavaScript disabled
  When P1 submits the full-page new-issue form with a title and a description
  Then the issue is created with that description
  And P1 lands on the board showing the new card

Scenario: Tenancy is unchanged
  Given P1 is NOT a member of team "other"
  When P1 requests the new-issue dialog for team "other" project "x"
  Then the response is 404
  And no issue is created
```

## US-02 — Description on the API create endpoint

```gherkin
Scenario: Creating an issue with a description over the API
  Given P2 holds a valid machine token for team "acme"
  When P2 POSTs {"title": "Rate limit", "description": "Return 429."} to the project's issues endpoint
  Then the response is 201 Created
  And a subsequent GET of that issue returns description "Return 429."

Scenario: Omitting description keeps existing clients working
  Given P2 holds a valid machine token for team "acme"
  When P2 POSTs {"title": "Rate limit"} with no description field
  Then the response is 201 Created
  And the issue's description_md is ""

Scenario: The API rejects an over-long description like the UI does
  Given P2 holds a valid machine token for team "acme"
  When P2 POSTs a title with a description exceeding the maximum length
  Then the response is 422
  And the message matches the copy the web dialog shows
  And no issue is created
```

## US-03 — Description length bound (create AND edit)

```gherkin
Scenario: Over-long description is refused on create
  Given P1 has the new-issue dialog open
  When P1 submits a valid title and a description exceeding the maximum length
  Then the dialog shows the description-too-long error
  And no issue is created

Scenario: Over-long description is refused on edit, leaving the issue untouched
  Given an issue exists with title "T" and description "D"
  When P1 saves the edit dialog with a description exceeding the maximum length
  Then the dialog shows the description-too-long error
  And the issue's title is still "T"
  And the issue's description_md is still "D"

Scenario Outline: The bound is inclusive at the boundary
  Given P1 has the new-issue dialog open
  When P1 submits a valid title and a description of <length> characters
  Then the create <outcome>

  Examples:
    | length     | outcome   |
    | 262144     | succeeds  |
    | 262145     | is refused|

Scenario: Multi-byte characters are counted as characters, not bytes
  Given P1 has the new-issue dialog open
  When P1 submits a valid title and a description of MAX multi-byte characters
  Then the issue is created
```

## Store-level scenarios (DISTILL to place)

- `insert_issue_with_outbox` persists a supplied description to `description_md`.
- `insert_issue_with_outbox` with an empty description writes `""` — byte-identical to today's behavior for
  every existing call-site.
- Workspace isolation on create is unchanged by the added parameter.

## Cross-feature invariant (issue-change-history)

```gherkin
Scenario: Creating with a description emits no change event
  Given the change-history feature records field CHANGES only (ODD-5, "start empty")
  When P1 files a new issue with a description
  Then the issue's timeline is empty
  And no "description" change event is recorded

Scenario: The first edit of a created description reports the created value as old_value
  Given P1 filed an issue with description "First"
  When P1 edits the description to "Second"
  Then a "description" change event records old_value "First" and new_value "Second"
```
