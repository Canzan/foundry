# Story: US-09 — Realtime issue updates via SSE
# Slice: 2 (Realtime collaboration)
# JTBD: outcome-4 (Linear-feel realtime — multi-user issue board feels live)
#
# Driving port: HTTP GET to /team/{team}/project/{project}/events as an SSE
# stream. Slice 1 already writes outbox rows in the same transaction as the
# issue insert (us-08); slice 2 turns that outbox row into a pg_notify and
# fans it out to local SSE subscribers via a per-replica LISTEN connection
# + tokio::sync::broadcast (see design/system/realtime-infrastructure.md).
#
# Driven adapters exercised:
#   - real Postgres LISTEN connection (dedicated per replica)
#   - real pg_notify('issue_events', payload) post-commit
#   - in-process broadcast fan-out filtered by project_id + membership
#   - real SSE response (axum::response::sse::Sse, heartbeat comments)
#
# NFR-PERF-03: median client-to-client latency ≤1s; P99 ≤2s. Per F-004
# (>=200ms budget for timing assertions), we measure with a 2000ms ceiling
# (the NFR-published P99) and a 1500ms median ceiling — comfortably above
# the median target, comfortably below the P99.
#
# Reconnect-from-Last-Event-Id is OUT of scope (deferred to v0.4 per
# realtime-infrastructure.md). Heartbeat interval defaults to 25s in
# production; tests configure a shorter interval via the SSE_HEARTBEAT_MS
# env var so the heartbeat scenario completes in <1s.

@slice2 @us-09 @realtime
Feature: A team member sees teammates' issue changes appear on the project board within one second
  A signed-in member on a project board has an open SSE stream to that
  project's event channel. When any teammate creates or updates an issue
  on the same project (from any replica), every subscribed member sees
  the change appear within the realtime budget. Subscribers never receive
  events from projects they cannot read, and unauthenticated subscriptions
  are refused.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"
    And a member "hiroshi@acme.com" belongs to the team "Backend"
    And a project "Auth v2" with key prefix "AUTH" exists in the "Backend" team
    And Mei is signed in

  @walking_skeleton @real-io @driving_adapter
  Scenario: Member subscribed to a project sees a teammate's new issue within two seconds
    Given Mei has an open subscription to events on "Auth v2"
    When Hiroshi files an issue against "Auth v2" with title "Refresh token rotation broken on Safari"
    Then within 2000 milliseconds Mei observes an "IssueCreated" event for "AUTH-1" on "Auth v2"
    And the event's project key is "AUTH"

  @real-io
  Scenario: Subscriber does not receive events from a project they are not viewing
    Given a project "Mobile App" with key prefix "MOB" exists in the "Backend" team
    And Mei has an open subscription to events on "Auth v2"
    When Hiroshi files an issue against "Mobile App" with title "Unrelated edit on a different project"
    Then within 2500 milliseconds Mei has received zero events on her "Auth v2" subscription

  @real-io @error @nfr-sec-06
  Scenario: Subscriber outside the team cannot subscribe to that team's project events
    Given a member "rita@partners.acme.com" belongs to the team "Partners"
    And Rita is signed in
    When Rita attempts to subscribe to events on "Auth v2"
    Then the subscription is refused with status 403
    And Rita receives no events on a closed stream

  @real-io @error
  Scenario: Anonymous subscriber cannot open an event stream
    Given Mei is signed out
    When an anonymous request attempts to subscribe to events on "Auth v2"
    Then the subscription is refused with status 401
    And the response body contains a sign-in prompt

  @real-io
  Scenario: A quiet stream emits heartbeat comments so load balancers do not idle-kill it
    Given the heartbeat interval is configured to 200 milliseconds for this scenario
    And Mei has an open subscription to events on "Auth v2"
    When 700 milliseconds pass with no issue activity on "Auth v2"
    Then Mei's stream has received at least 2 keepalive heartbeats

  @real-io
  Scenario: Subscriber receives events for issue updates as well as creations
    Given the "Auth v2" project already has issue AUTH-1
    And Mei has an open subscription to events on "Auth v2"
    When Hiroshi changes the state of "AUTH-1" to "in-progress"
    Then within 2000 milliseconds Mei observes an "IssueUpdated" event for "AUTH-1" on "Auth v2"
    And the event payload reports state "in-progress"

  @real-io @nfr-perf-03
  Scenario: Sequential issue creations all fan out within the NFR-PERF-03 budget
    # NFR-PERF-03 ceiling: P99 <= 2 seconds, median <= 1 second. We assert
    # each of 10 events arrives within 2000ms (P99), and that the median
    # arrival time across the 10 is <= 1500ms (median + safety margin per
    # F-004 to avoid flake under parallel CI load).
    Given Mei has an open subscription to events on "Auth v2"
    When Hiroshi files 10 issues against "Auth v2" sequentially, each with a unique title, pausing 100 milliseconds between
    Then Mei receives 10 "IssueCreated" events whose keys are "AUTH-1" through "AUTH-10"
    And every per-event arrival latency is at most 2000 milliseconds
    And the median per-event arrival latency is at most 1500 milliseconds
