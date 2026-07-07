# Feature: issue-change-history — every change to an issue becomes a durable,
# attributable record, surfaced for humans, programs, and reports.
#
# Source SSOT for docs/feature/issue-change-history/distill/test-scenarios.md.
# ONE model (DESIGN ADR-001): an append-only `issue_change_events` table
# (actor · field · old → new · when), written in the SAME transaction as the
# mutation via a shared `record_issue_change` helper. Three read surfaces
# (ADR-002): a human timeline on a NEW issue-detail page, a program JSON feed at
# `GET /api/v1/.../issues/{n}/history`, and a project report + CSV export.
#
# Genesis = START EMPTY (ADR-001 / upstream UC-1): v1 records field CHANGES only
# — no backfill, no 'created' event; an unchanged issue's timeline is EMPTY.
#
# HARNESS NOTE: HTTP + store level (reqwest + scraper + real Postgres). Automated:
# the in-tx record contract (issue_change_events reads), the detail-page timeline
# render, the /api/v1 history JSON, the report + CSV, tenancy/non-enumerability.
# Plain-language phrasing polish + any live refresh are dogfood (converge on
# reload, no live push — mirrors card-ranking UC-1).
#
# EVERY scenario @pending until DELIVER wires + un-@pends (kept out of @all).

@issue-change-history @us-change-history @driving_port
Feature: A member sees, a program consumes, and a lead reports every change to an issue
  Each change to a tracked issue field (status, title, description, rank) is
  recorded as who/what/old→new/when, in the same transaction as the change and
  never edited or deleted — then read three ways.

  Background:
    Given a workspace "Acme" exists with a member "Mei" on team "Backend"
    And a project "Sandbox" (key "GEN") with issues:
      | key   | column  |
      | GEN-1 | Backlog |
      | GEN-2 | Todo    |
    And Mei is signed in

  # ---- Slice 01: status-change history + human timeline (issue-detail page) ----

  @us-01 @real-io @pending
  Scenario: A status change records one change event in the same transaction
    When Mei saves the edit dialog for "GEN-1" with status "Todo"
    Then a change event is recorded for "GEN-1": field "status", old "backlog", new "todo", by "Mei"

  @us-01 @real-io @pending
  Scenario: The issue-detail page renders the change timeline newest-first
    When Mei saves the edit dialog for "GEN-1" with status "Todo"
    And Mei saves the edit dialog for "GEN-1" with status "In-Progress"
    And Mei opens the detail page for "GEN-1"
    Then the "GEN-1" timeline shows a "status" change to "In Progress" by "Mei"
    And the "GEN-1" timeline lists the "in_progress" change above the "todo" change

  @us-01 @real-io @pending
  Scenario: An unchanged issue shows an empty timeline (no created event)
    When Mei opens the detail page for "GEN-2"
    Then the "GEN-2" timeline is empty

  @us-01 @real-io @pending
  Scenario: History is append-only — a later change adds an entry, earlier ones unchanged
    When Mei saves the edit dialog for "GEN-1" with status "Todo"
    And Mei saves the edit dialog for "GEN-1" with status "Done"
    Then "GEN-1" has 2 change events in the store
    And the earliest "GEN-1" change event still reads field "status", old "backlog", new "todo"

  @us-01 @real-io @pending
  Scenario: A same-value save records no change event
    When Mei saves the edit dialog for "GEN-2" with status "Todo"
    Then no change event is recorded for "GEN-2"

  @us-01 @real-io @error @pending
  Scenario: The timeline of a foreign issue is refused non-enumerably
    Given a foreign issue "ZZZ-9" exists in another workspace
    When Mei opens the detail page for "ZZZ-9"
    Then the response is a non-enumerable refusal

  @us-01 @real-io @pending
  Scenario: The board still opens the quick-edit modal, and the card links to the detail page
    When Mei fetches the "Sandbox" board
    Then each issue card still carries its edit-dialog control
    And each issue card links to its detail page

  # ---- Slice 02: record every editable field ----

  @us-02 @real-io @pending
  Scenario: A title edit records a title change (the previously-silent path now records)
    When Mei edits "GEN-1" title to "Login 500 on submit"
    Then a change event is recorded for "GEN-1": field "title", old "Seeded issue", new "Login 500 on submit", by "Mei"

  @us-02 @real-io @pending
  Scenario: A single save changing title and description records one event per changed field
    When Mei edits "GEN-1" title to "Auth bug" and description to "Repro on submit"
    Then "GEN-1" has a change event for field "title"
    And "GEN-1" has a change event for field "description"
    And "GEN-1" has no change event for field "status"

  @us-02 @real-io @pending
  Scenario: A reorder records a rank change
    When Mei drops "GEN-1" after "GEN-2" in "todo" as the drop handler would
    Then a change event is recorded for "GEN-1": field "rank", by "Mei"

  # ---- Slice 03: program JSON change feed ----

  @us-03 @real-io @pending
  Scenario: The history endpoint returns the issue's change events as JSON, oldest-first
    When Mei saves the edit dialog for "GEN-1" with status "Todo"
    And Mei edits "GEN-1" title to "Renamed"
    And a program requests the change history of "GEN-1"
    Then the history JSON lists the events oldest-first, each with actor, field, old, new, and a timestamp
    And the JSON events are the same as the stored change events for "GEN-1"

  @us-03 @real-io @error @pending
  Scenario: The history endpoint refuses a foreign issue non-enumerably
    Given a foreign issue "ZZZ-9" exists in another workspace
    When a program requests the change history of "ZZZ-9"
    Then the API response is a uniform non-enumerable refusal

  # ---- Slice 04: project change report + CSV export ----

  @us-04 @real-io @pending
  Scenario: The project report lists changes across issues and summarizes them
    When Mei saves the edit dialog for "GEN-1" with status "Todo"
    And Mei saves the edit dialog for "GEN-2" with status "In-Progress"
    And Mei opens the change report for project "Sandbox"
    Then the report lists change events across the project's issues, most recent first
    And the report summarizes status-flow transition counts and per-actor change counts

  @us-04 @real-io @pending
  Scenario: The project report exports to CSV with a stable column contract
    When Mei saves the edit dialog for "GEN-1" with status "Todo"
    And Mei exports the change report for project "Sandbox" as CSV
    Then the response is a CSV attachment with columns "issue,actor,field,old,new,at"

  @us-04 @real-io @error @pending
  Scenario: The report shows only the acting workspace's changes
    Given a foreign issue "ZZZ-9" exists in another workspace
    When Mei opens the change report for project "Sandbox"
    Then the report contains no "ZZZ-9" change events
