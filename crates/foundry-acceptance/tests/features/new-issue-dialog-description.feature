# Feature: new-issue-dialog-description — add the Description field to the new-issue dialog.
#
# Source SSOT for docs/feature/new-issue-dialog-description/distill/test-scenarios.md.
# The new-issue dialog (opened by the `c` shortcut or the board "New issue" button)
# collects Title only; the shipped EDIT dialog collects Title + Description. This
# feature threads an OPTIONAL `description` through the create path at every layer
# (template → CreateIssueForm → services::create_issue → insert_issue_with_outbox),
# keeps web/JSON-API rule-parity (NFR-WEB-API-CON-02), and adds a shared app-level
# length bound matching the DB CHECK (262144). No migration (latest stays 0014);
# no new component — every change mirrors the shipped edit path (DESIGN ADR-001/002).
#
# HARNESS NOTE (as board-new-issue / issue-edit-dialog): the acceptance suite is
# HTTP-level (reqwest + scraper) + direct store assertions, NOT a JS browser. It
# pins (a) the wiring — the modal now carries a description textarea; (b) the create
# endpoint contract end-to-end — description persisted, OOB Backlog card, no-JS
# fallback; (c) the API rule-parity; (d) the length bound on BOTH create and edit.
#
# IMPORTANT — error visibility is NOT asserted at the DOM level. Under the vendored
# htmx 2.0.4 default config, a 4xx response is NOT swapped, so a validation error
# (empty title, or too-long description) is preserved-but-invisible in the browser.
# That is a pre-existing app-wide defect, deferred to its own bugfix (DESIGN
# upstream-changes.md). These scenarios therefore assert the HTTP RESPONSE
# (400/422 + fragment/body), never a visible on-screen message. Typed-input
# survival is a browser-dogfood item (walking-skeleton.md), because htmx's
# non-swap of the 400 is what preserves it — not observable over HTTP.
#
# EVERY scenario is @pending; acceptance.rs filter_run excludes @pending from every
# lane, so @all stays green until DELIVER wires the code and un-@pends per slice.

@new-issue-dialog-description @us-new-issue-description @driving_port
Feature: A member writes a description while filing an issue from the dialog
  The new-issue dialog offers a Description field beside Title, mirroring the
  shipped edit dialog. Filing persists the description; it round-trips to the
  edit dialog; it is optional; over-long descriptions are refused with a clean
  validation error on both create and edit — reusing the shipped create path,
  CSRF, OOB card, and no-JS fallback, with tenancy intact.

  Background:
    Given a workspace "Acme" exists with a member "Mei" on team "Backend"
    And a project "Sandbox" with key prefix "GEN" exists under "Backend"
    And Mei is signed in

  # ---------------------------------------------------------------- US-01 (web)

  @real-io @slice1 @us-01
  Scenario: The new-issue dialog offers a Description field
    When Mei fetches the new-issue dialog for "Sandbox"
    Then the new-issue modal form carries a "description" textarea beside the title input
    And the new-issue "description" textarea is empty

  @real-io @slice1 @us-01
  Scenario: Filing an issue with a description persists it and returns the Backlog card
    When Mei files a new issue titled "Rate limit the gateway" described "Return 429 with Retry-After." to "Sandbox" as an htmx request
    Then the created "Sandbox" issue "GEN-1" has description "Return 429 with Retry-After." in the store
    And the response is an out-of-band fragment targeting the "backlog" column
    And it renders a card showing the key "GEN-1" and the title "Rate limit the gateway"

  @real-io @slice1 @us-01
  Scenario: A filed description round-trips to the edit dialog
    Given Mei has filed an issue titled "Rate limit" described "Return 429 with Retry-After." to "Sandbox"
    When Mei opens the edit dialog for "GEN-1"
    Then the dialog description field contains "Return 429 with Retry-After."

  @real-io @slice1 @us-01
  Scenario: Description is optional — a title-only create stores an empty description
    When Mei files a new issue titled "Just a title" described "" to "Sandbox" as an htmx request
    Then the created "Sandbox" issue "GEN-1" has description "" in the store
    And it renders a card showing the key "GEN-1" and the title "Just a title"

  @real-io @slice1 @us-01 @error
  Scenario: An empty title is rejected and no issue is created, even with a typed description
    # HTTP-observable only: the 400 fragment + no row. Input SURVIVAL (htmx does not
    # swap the 400, so the modal keeps the typed description) is a browser-dogfood item.
    When Mei files a new issue with an empty title described "Some detail I typed" to "Sandbox" as an htmx request
    Then the response is the "Title is required" error fragment
    And no issue exists in the "Sandbox" project

  @real-io @slice1 @us-01 @no-js
  Scenario: No-JS fallback — the full-page form carries the field and files with a description
    When Mei fetches the full-page new-issue form for "Sandbox"
    Then the full-page new-issue form carries a "description" textarea
    When Mei files a new issue titled "Fallback body" described "typed without JS" to "Sandbox" as a plain form
    Then the response redirects to the "Sandbox" board
    And the created "Sandbox" issue "GEN-1" has description "typed without JS" in the store

  @real-io @slice1 @us-01 @security
  Scenario: Requesting the new-issue dialog for a foreign project is refused non-enumerably
    Given a project "Secret" with key prefix "SEC" exists in a DIFFERENT workspace from Mei
    When Mei requests the new-issue dialog for that project's path
    Then the response is the uniform not-found page

  # ---------------------------------------------------------------- US-02 (API)

  @real-io @slice2 @us-02 @driving_adapter @nfr-web-api-con-02
  Scenario: A machine files an issue with a description through the API
    Given the admin has granted a machine credential for "api-writer" bound to Mei with write access to "Sandbox"
    When the machine files an issue titled "Rate limit" described "Return 429." to "Sandbox" through the API
    Then the write is accepted with the next sequential key
    And reading that issue back returns the description "Return 429."

  @real-io @slice2 @us-02 @driving_adapter
  Scenario: Omitting the description over the API keeps existing clients working
    Given the admin has granted a machine credential for "api-writer" bound to Mei with write access to "Sandbox"
    When the machine files an issue titled "No body" through the API
    Then the write is accepted with the next sequential key
    And reading that issue back returns an empty description

  # ------------------------------------------------ US-03 (bound: create + edit)

  @real-io @slice3 @us-03 @error
  Scenario: An over-long description is refused on create and no issue is created
    When Mei files a new issue titled "Paste bomb" with a description of 262145 characters to "Sandbox" as an htmx request
    Then the response is the "Description is too long" error fragment
    And no issue exists in the "Sandbox" project

  @pending @real-io @slice3 @us-03 @error
  Scenario: An over-long description is refused on edit, leaving the issue untouched
    # Today this produces a DB-CHECK 500; after slice 03 it is a clean validation refusal.
    Given a project "Sandbox" issue "GEN-1" titled "Keep" described "keep body" exists
    When Mei saves the edit dialog for "GEN-1" with title "Keep" and a description of 262145 characters
    Then the response is the "Description is too long" error fragment
    And the issue "GEN-1" still has title "Keep" and description "keep body" in the store

  @real-io @slice3 @us-03
  Scenario: A description exactly at the maximum is accepted
    When Mei files a new issue titled "At the bound" with a description of 262144 characters to "Sandbox" as an htmx request
    Then the created "Sandbox" issue "GEN-1" has a description of 262144 characters in the store

  @real-io @slice3 @us-03
  Scenario: Length is counted in characters, not bytes — multi-byte content at the bound is accepted
    When Mei files a new issue titled "Multibyte" with a description of 262144 multi-byte characters to "Sandbox" as an htmx request
    Then the created "Sandbox" issue "GEN-1" is created with a 262144-character description

  @pending @real-io @slice3 @us-03 @driving_adapter @nfr-web-api-con-02
  Scenario: The API refuses an over-long description by the same rule the browser enforces
    Given the admin has granted Mei a machine credential with write access to "Sandbox"
    When the machine files an issue titled "API paste bomb" with a description of 262145 characters through the API
    Then the API write is rejected as unprocessable for a too-long description
    And the rejection reason matches the browser's "Description is too long" rule

  # ------------------------------------------- Cross-feature: issue-change-history

  @real-io @slice1 @us-01 @cross-feature
  Scenario: Creating an issue with a description emits no change-history event
    # issue-change-history ODD-5 "start empty": v1 records CHANGES, not creation.
    When Mei files a new issue titled "Fresh" described "created with a body" to "Sandbox" as an htmx request
    Then the change-history timeline for "GEN-1" is empty

  @real-io @slice1 @us-01 @cross-feature
  Scenario: The first edit of a created description reports the created value as the old value
    Given Mei has filed an issue titled "Fresh" described "First" to "Sandbox"
    When Mei saves the edit dialog for "GEN-1" with title "Fresh" and description "Second"
    Then a "description" change event for "GEN-1" records old value "First" and new value "Second"
