# Story: US-02 — Operator scales to multiple replicas
# Slice: 3 (operator-grade)
# JTBD: outcome-6 (Multi-replica operability with no sticky-session tax)
#
# Driving port: HTTP through the in-process round-robin proxy in front of
# N foundry-app replicas (Option A, see distill/wave-decisions.md §US-02).
# A single `@docker-compose` scenario also exercises the production-shaped
# Caddy + multi-replica compose stack.
#
# Driven adapters exercised (Strategy C — all real):
#   - real reqwest::Client -> in-process round-robin proxy -> N real axum binaries
#   - one shared real Postgres (testcontainers-rs), one per-scenario schema
#   - real tower-sessions-sqlx-store rows (sessions survive replica switches)
#   - real PgListener task per replica + real pg_notify post-commit
#   - real `/readyz` health endpoint observed via the proxy's health probe
#
# NFR coverage: NFR-AVAIL-01 (no sticky), NFR-AVAIL-02 (graceful shutdown
# drains in <=10s), NFR-OBS-02 (/readyz flips to 503 on DB outage), NFR-
# PERF-04 (per-replica pool stays <= 10 connections). Latency assertions
# use a 200ms+ budget per F-004 to stay non-flaky under parallel CI load.
#
# Out of scope for slice 3 (deferred):
#   - K8s manifests applied to a real kind/k3d cluster (would be `@k8s`);
#     the K8s YAML is reviewed-by-eye + driven by the same docker-compose
#     scenario contract. See wave-decisions.md §US-02-deferred.
#   - SSE "old replica keeps serving old SQL during migration" — covered
#     by US-04 (rolling-upgrade), not here.
#
# Gherkin discipline (CM-B): the OPERATOR is the user for this story.
# /readyz, SIGTERM, and HTTP status codes ARE the operator-facing
# contracts under test — they remain in the scenarios because the
# operator literally reads /readyz output, sends SIGTERM, and watches
# the status code distribution in dashboards. Implementation details
# below the operator's surface (pg_notify channel names, tokio task
# IDs, sqlx pool internals) stay in step-method bodies.

@slice3 @us-02 @multi-replica
Feature: An operator runs three foundry replicas behind a load balancer and survives single-replica failures without user-visible logouts
  A signed-in member's browser is routed by a round-robin load balancer
  across N=3 foundry-app replicas that share one database. Sessions live
  in the database so any cookie validates on any replica. Realtime
  streams stay on their landing replica but auto-reconnect to a healthy
  replica when the landing replica dies. The load balancer removes any
  replica whose /readyz returns 503 (database outage, graceful drain)
  within ten seconds.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"
    And a member "hiroshi@acme.com" belongs to the team "Backend"
    And a project "Auth v2" with key prefix "AUTH" exists in the "Backend" team
    And the operator runs 3 foundry replicas behind a round-robin load balancer

  @walking_skeleton @real-io @driving_adapter
  Scenario: A member's session is recognised by every replica regardless of which one the load balancer routes her to
    Given Mei is signed in
    When Mei makes 6 requests through the load balancer that visit each of the 3 replicas at least once
    Then every request observes Mei as signed in
    And no request prompts Mei to re-authenticate
    And the workspace dashboard renders Mei's display name on every response

  @real-io
  Scenario: A member files an issue on one replica and another member observes it through a different replica within two seconds
    Given Mei is signed in
    And Hiroshi is signed in
    And Mei has an open realtime subscription on "Auth v2" that landed on a specific replica
    When Hiroshi files an issue against "Auth v2" with title "Multi-replica fan-out works" via a different replica than Mei's subscription
    Then within 2000 milliseconds Mei observes an "IssueCreated" event for "AUTH-1" on "Auth v2"
    And the event was produced by a different replica than the one serving Mei's subscription

  @real-io @nfr-avail-03
  Scenario: A member's realtime stream auto-reconnects to a healthy replica when its landing replica is stopped
    Given Mei is signed in
    And Mei has an open realtime subscription on "Auth v2" that landed on a specific replica
    When the replica serving Mei's subscription is stopped
    Then within 10000 milliseconds Mei's client has reconnected to a different healthy replica
    And subsequent issue events on "Auth v2" are delivered to Mei within 2000 milliseconds of being produced

  @real-io @error @nfr-obs-02
  Scenario: All replicas flip /readyz to 503 within ten seconds when the database becomes unreachable, and the load balancer removes them from rotation
    Given all 3 replicas report ready through their /readyz endpoint
    When the database becomes unreachable from every replica
    Then within 10000 milliseconds every replica's /readyz endpoint returns 503
    And the load balancer removes every replica from rotation
    And a subsequent request through the load balancer receives an upstream-unavailable response

  @real-io @nfr-avail-02
  Scenario: A replica receiving SIGTERM finishes in-flight requests and flips to draining within the grace window
    Given Mei is signed in
    And Mei has just submitted a long-running request that is being served by a specific replica
    When the replica serving Mei's request receives SIGTERM
    Then Mei's in-flight request completes successfully
    And the replica's /readyz endpoint returns 503 before its in-flight request completes
    And the replica exits within 15 seconds of receiving SIGTERM

  @real-io @nfr-perf-04
  Scenario: Per-replica connection pool stays below the configured ceiling under sustained traffic across all replicas
    Given Mei is signed in
    When Mei issues 30 requests through the load balancer back-to-back over 3 seconds
    Then no replica's database pool ever exceeds 10 active connections
    And every request returns a successful response

  @docker-compose @us-02 @real-io @manual-trigger
  Scenario: The production-shaped Caddy + 3-replica docker-compose stack serves the session-survives-replica-switch scenario
    Given the docker-compose multi-replica stack is up with Caddy in front of 3 foundry-app replicas
    And an admin has bootstrapped a workspace "Acme Eng" with member "mei@acme.com"
    And Mei is signed in through the Caddy load balancer
    When Mei makes 6 requests through Caddy that visit each replica at least once
    Then every request observes Mei as signed in
    And no request prompts Mei to re-authenticate
    And the Caddy access log shows requests distributed across all 3 replica upstreams
