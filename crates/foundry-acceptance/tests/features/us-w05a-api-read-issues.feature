# Story: US-W05a — Read the board's issues as machine-readable data
# Feature A "Programmatic Foundry" — Slice 1 (Walking Skeleton)
# JTBD: jtbd-web-4 (drive Foundry programmatically) + jtbd-web-2 (one neutral core)
#
# Driving adapter: the JSON API read surface served by foundry-api, reached
# over HTTP at GET /api/v1/teams/{team}/projects/{project}/issues — see
# design/api-contract.md (route surface) and design/architecture.md
# (foundry-api crate, foundry-services seam).
# Driven adapters exercised: real Postgres (teams, projects, issues, outbox,
# workspace/team memberships, sessions) via testcontainers + per-scenario schema.
#
# This is THE demo proof of the headline premise: a real machine consumer
# reads the same board the UI shows, as data, from one presentation-neutral
# core call, with no markup in the response. Per slice-01-json-read.md the
# read endpoint MAY accept the existing browser session in Slice 1; the
# machine-token credential becomes required in Slice 2 (us-w05b).

@feature-a @us-w05a @driving_adapter
Feature: An integrator reads a project's issues as machine-readable data
  An integrator points a script at a project's board and receives the same
  issues a member sees in the browser — as a data array, not a page — served
  from the one core data path the browser board also reads, with no markup
  in the response. An empty project answers with an empty list, and a request
  with no valid credential is refused without leaking any issue data.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"
    And a project "Auth v2" with key prefix "AUTH" exists in the "Backend" team

  @walking_skeleton @real-io @driving_adapter
  Scenario: An integrator reads the board's issues as data
    Given the "Auth v2" project has issue AUTH-2 titled "Refresh token rotation" in progress
    And the "Auth v2" project has issue AUTH-3 titled "Revoke on password change" in the backlog
    And Mei is signed in
    When Mei requests the "Auth v2" board's issues as machine-readable data
    Then the answer is a data list containing AUTH-2 and AUTH-3
    And each entry carries the issue's key, title, and state
    And AUTH-2 is reported in progress and AUTH-3 in the backlog
    And the answer contains no markup

  @real-io
  Scenario: An empty project answers with an empty list
    Given the "Auth v2" project has no issues
    And Mei is signed in
    When Mei requests the "Auth v2" board's issues as machine-readable data
    Then the answer is an empty data list
    And the request is reported as successful

  @real-io
  Scenario: The data answer and the browser board come from the same core path
    Given the "Auth v2" project has issue AUTH-2 titled "Refresh token rotation" in progress
    And the "Auth v2" project has issue AUTH-3 titled "Revoke on password change" in the backlog
    And Mei is signed in
    When Mei reads the "Auth v2" board as machine-readable data
    And Mei opens the "Auth v2" board in the browser
    Then both list exactly the same set of issues

  @error @real-io
  Scenario: A request with no valid credential is refused
    Given a caller presents no valid credential
    When the caller requests the "Auth v2" board's issues as machine-readable data
    Then the request is refused as unauthenticated
    And no issue data is returned
