# Story: US-W05c — Create and update issues and comments through the API
# Feature A "Programmatic Foundry" — Slice 2
# JTBD: jtbd-web-4 (an agent can DO work, not just observe it)
#
# Driving adapter: the JSON API write surface served by foundry-api —
#   POST  /api/v1/teams/{team}/projects/{project}/issues            (create issue)
#   PATCH /api/v1/teams/{team}/projects/{project}/issues/{number}   (change state)
#   POST  .../issues/{number}/comments                              (create comment)
#   PATCH .../issues/{number}/comments/{comment_id}                 (edit comment)
# See design/api-contract.md (resource shapes, status codes, error envelope)
# and design/architecture.md (writes reuse the foundry-services use-cases the
# browser handlers call — the SAME authz, validation, sanitization, outbox).
# Driven adapters exercised: real Postgres (issues, comments, outbox,
# memberships, machine_tokens), real markdown sanitizer in core, real SSE
# broadcast (an API-created change reaches a watching subscriber).
#
# The load-bearing requirement is RULE-PARITY (NFR-WEB-API-CON-02): an API
# write and the equivalent browser write produce the same acceptance/rejection
# and the same stored bytes. The scenarios pair API writes against UI rules.

@feature-a @us-w05c @driving_adapter
Feature: A machine creates and updates issues and comments through the API
  Holding a credential with write access, a machine files an issue, moves it
  through its states, and posts a comment — each governed by the same rules a
  member's browser action obeys: the same authorization, the same validation,
  the same sanitization of dangerous content, and the same realtime visibility.
  Invalid writes are rejected by the same rule the browser enforces, and a
  machine cannot do through the API what its bound principal could not do in
  the browser.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"
    And a project "Auth v2" with key prefix "AUTH" exists in the "Backend" team
    And the admin has granted a machine credential for "Devansh's agent" bound to Mei with write access to "Auth v2"

  @slice2-entry @real-io @driving_adapter
  Scenario: A machine files an issue through the API
    When the machine files an issue titled "Refresh token rotation broken on Safari" through the API
    Then a new issue is created with the next sequential key
    And the created issue is returned as data including its key and state
    And the new issue starts in the backlog
    And the answer contains no markup

  @real-io
  Scenario: An issue filed through the API appears to a member watching the board
    Given Mei is watching the "Auth v2" board in real time
    When the machine files an issue titled "Refresh token rotation broken on Safari" through the API
    Then the new issue appears on Mei's board
    And it was filed through the same core path a browser-filed issue travels

  @real-io
  Scenario: A machine moves an issue to a new state through the API
    Given the "Auth v2" project has issue AUTH-8 titled "Refresh token rotation" in the backlog
    When the machine moves AUTH-8 to "in progress" through the API
    Then AUTH-8's state becomes in progress
    And the updated issue is returned as data

  @real-io @nfr-web-api-con-02
  Scenario: A comment posted through the API is sanitized exactly as a browser comment
    Given the "Auth v2" project has issue AUTH-8 titled "Refresh token rotation" in the backlog
    When the machine posts a comment on AUTH-8 containing a script tag and a "javascript:" link through the API
    Then the comment is stored with the dangerous content removed
    And the stored comment matches what a browser-posted comment with the same text would store

  @error @real-io @nfr-web-api-con-02
  Scenario: An issue with an empty title is rejected by the same rule the browser enforces
    When the machine files an issue with an empty title through the API
    Then the write is rejected for a missing title
    And the rejection reason matches the browser's "Title is required" rule
    And the rejection is returned as data with no markup

  @real-io @nfr-web-api-con-02
  Scenario: The created issue is returned with the same trimmed title the store persists
    When the machine files an issue titled "  Refresh token rotation  " through the API
    Then a new issue is created with the next sequential key
    And the created issue is returned with the trimmed title "Refresh token rotation"

  @error @real-io
  Scenario: A write beyond the credential's authorization is refused
    Given the "Auth v2" project has a comment by Mei on issue AUTH-8
    And the admin has granted a second machine credential bound to a member who is not the comment's author and not an admin
    When that machine edits Mei's comment through the API
    Then the write is refused as not-allowed
    And the comment is left unchanged
