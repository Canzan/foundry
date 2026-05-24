# Story: US-08 — User files an issue (the JTBD hot path)
# Slice: 1 (Walking Skeleton)
# JTBD: outcome-4 (Linear-feel speed for the most-frequent user action)
#
# Driving port: HTTP form POST to /issues (htmx-driven) — see
# architecture.md § "End-to-end Trace: User files an issue".
# Driven adapters exercised: real Postgres (issues, projects.next_issue_number,
# outbox), in-process Publisher → pg_notify (verified via outbox row).
# NFR-PERF-01: per the budget in `nw-ad-critique-dimensions` F-004, the
# performance scenario uses a 200ms budget (NOT the 50ms internal stretch)
# to avoid flake under parallel CI load.

@slice1 @us-08 @driving_port
Feature: A team member files an issue and sees it in the project board
  A signed-in member of a team can file an issue against one of that team's
  projects, providing only a title; everything else defaults sanely. The
  issue is assigned a per-project sequential key (AUTH-1, AUTH-2, ...) and
  appears in the Backlog column. Empty titles are rejected with an inline
  error; cross-team or anonymous attempts are forbidden.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"
    And a project "Auth v2" with key prefix "AUTH" exists in the "Backend" team
    And Mei is signed in

  @walking_skeleton @real-io @us-08
  Scenario: Member files an issue with only a title and sees AUTH-1 on the board
    When Mei files an issue against "Auth v2" with title "Refresh token rotation broken on Safari"
    Then the new issue is assigned the key "AUTH-1"
    And the issue's state is "backlog"
    And the issue's priority is "medium"
    And the issue's author is Mei
    And the response contains a fragment showing AUTH-1 in the Backlog column
    And opening "/team/backend/project/auth-v2" lists AUTH-1 in the Backlog column

  @real-io
  Scenario: Issue keys are sequential per project
    Given the "Auth v2" project already has issues AUTH-1 through AUTH-5
    When Mei files a new issue against "Auth v2" with title "Sixth issue"
    Then the new issue is assigned the key "AUTH-6"

  @real-io
  Scenario: Issue keys are scoped per project, not per workspace
    Given a project "Web App" with key prefix "WEB" exists in the "Backend" team
    And the "Auth v2" project already has issue AUTH-1
    When Mei files an issue against "Web App" with title "First web issue"
    Then the new issue is assigned the key "WEB-1"

  @error @real-io
  Scenario: An empty title is rejected with an inline htmx error fragment, not a full page
    When Mei files an issue against "Auth v2" with title ""
    Then the response status is 400 or 422
    And the response is an htmx fragment containing "Title is required"
    And the response is not a full HTML page
    And no issue is created in "Auth v2"

  @error @real-io @nfr-sec-06
  Scenario: A workspace member not on the team cannot file an issue against that team's project
    Given Hiroshi is a workspace member but not a member of the "Backend" team
    And Hiroshi is signed in
    When Hiroshi files an issue against "Auth v2" with title "Unauthorized attempt"
    Then the response status is 403 Forbidden
    And no issue is created in "Auth v2"

  @nfr-perf-01 @real-io
  Scenario: Sequential issue creation has P95 latency under 200ms
    # NFR-PERF-01: P95 server-render latency <= 200ms (the measurable ceiling;
    # 50ms is documented as internal stretch only). Budget 200ms per F-004
    # guidance (flake-tolerant under parallel CI load).
    When Mei files 100 issues against "Auth v2" sequentially, each with a unique title
    Then all 100 issues are persisted with sequential keys AUTH-1 through AUTH-100
    And the P95 server-side response time across those 100 requests is at most 200 milliseconds

  @property @real-io
  Scenario Outline: Title length boundary handling
    When Mei files an issue against "Auth v2" with title of length <length>
    Then the file-issue outcome is "<outcome>"

    Examples: boundary
      | length | outcome  |
      | 1      | accepted |
      | 256    | accepted |
      | 0      | rejected |
      | 257    | rejected |
