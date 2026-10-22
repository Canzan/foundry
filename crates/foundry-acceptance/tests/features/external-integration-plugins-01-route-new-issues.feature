# Feature: external-integration-plugins, slice 01 (US-EIP-02) — a team lead
# sends a project's new issues to a Slack channel and sees the first one arrive;
# filing an issue is never slowed or refused by chat delivery (SC-1, SC-2,
# D-5, OD-7, DDR-3, DDR-25, DDR-27).
#
# Authored @pending in DISTILL 2026-10-21 (ADR-025 scaffolded RED).
#
# Harness as in external-integration-plugins-01-connect-slack.feature. Time:
# the harness's MockClock is the injected Clock (ADR-EIP-002 decision 11);
# "24 hours pass" advances it. Eventual states are bounded polls (10 s, the
# NFR-EIP-1 p95 bound), never fixed sleeps. Slack's moods (does not answer,
# answers again) are the double's script.
#
# The restart scenario runs the real, prebuilt `foundry` binary as a subprocess
# (warm-binary hazard P2; DEVOPS eip-restart-lane, `@slow`): it is killed with
# SIGKILL between attempts and started again.
#
# Page seams DISTILL pins for DELIVER (see feature-delta DISTILL Pre-requisites):
# one element per route carrying data-route-destination="<#channel>" whose text
# holds the K-9 status; the route form's fields kind, destination, events,
# lanes, content_level, shared_confirmed.

# Copy and behaviour beyond D-13 that these scenarios assert were ratified by
# the user on 2026-10-22 (feature-delta DISTILL "[REF] User ratification
# (2026-10-22)", R-1..R-11, with the exact wording).

@external-integration-plugins @eip-01 @us-eip-02 @real-io
Feature: A team lead sends a project's new issues to a Slack channel and sees the first one arrive

  Background:
    Given the Canzan Labs workspace with these people
      | person       | workspace role | teams                                |
      | Priya Raman  | admin          | Backend as lead, Ops as lead         |
      | Lena Park    | member         | Backend as lead                      |
      | Sam Okafor   | member         | Backend as member, Ops as member     |
      | Marco        | member         |                                      |
    And Identity Platform (AUTH) belongs to Backend and Homelab Ops (OPS) belongs to Ops
    And Identity Platform holds these issues
      | key    | title                              | lane        | filed by   |
      | AUTH-7 | Move Grafana sign-in to Keycloak   | In progress | Sam Okafor |
      | AUTH-8 | Document the LDAP group mapping    | Todo        | Lena Park  |
    And the next issue filed in Identity Platform becomes AUTH-12
    And it is 14:02 on 21 Oct 2026
    And the instance has the Foundry Slack app set up

  @pending @walking_skeleton @driving_port @kpi @contract-shape:bounded-change
  Scenario: Lena sees the first new issue arrive in the chosen Slack channel
    Given Priya has connected Canzan Labs to Slack
    And Lena has routed New issue on Identity Platform to #auth-dev
    When Sam files "Rotate LLDAP bind password" in Identity Platform
    Then #auth-dev shows within 10 seconds a Foundry message "AUTH-12 Rotate LLDAP bind password" / "New issue in Identity Platform by Sam Okafor · Backlog" with Open in Foundry
    And Open in Foundry opens AUTH-12 for Sam and gives Marco the same not-found page any unknown address gets
    And Lena's route to #auth-dev reads "Delivering" with the time of that delivery

  @pending @contract-shape:bounded-change
  Scenario: A test message confirms the route before it is saved
    Given Priya has connected Canzan Labs to Slack
    And Lena is creating a Slack route from Identity Platform to #auth-dev
    When Lena chooses Send test
    Then #auth-dev shows "Test message from Foundry for Identity Platform (AUTH). This route will post here."
    And Identity Platform still has no route

  @pending @edge @contract-shape:bounded-change
  Scenario: Saving a route sends nothing and puts it on record
    Given Priya has connected Canzan Labs to Slack
    And Lena is creating a Slack route from Identity Platform to #auth-dev
    When Lena saves the route
    Then Identity Platform's integrations page lists the route to #auth-dev reading "Delivering"
    And Slack has received no message for #auth-dev
    And the workspace's integration activity gains the line "21 Oct 2026 · Lena Park created the route to #auth-dev"

  @pending @edge @contract-shape:bounded-change
  Scenario: Test messages are limited to six a minute for each person
    Given Priya has connected Canzan Labs to Slack
    And Lena is creating a Slack route from Identity Platform to #auth-dev
    And Lena has sent 6 test messages in the last minute
    When Lena chooses Send test
    Then Lena reads "Too many test messages. Try again in a minute."
    And #auth-dev holds exactly 6 test messages
    And Identity Platform still has no route

  @pending @error @kpi @contract-shape:bounded-change
  Scenario: Filing an issue is never slowed or refused by chat delivery
    Given Priya has connected Canzan Labs to Slack
    And Lena has routed New issue on Identity Platform to #auth-dev
    And Slack does not answer
    When Sam files "Audit Keycloak client scopes" in Identity Platform
    Then AUTH-12 is created and shown on the board within 2 seconds
    And Lena's route to #auth-dev reads "Retrying: Slack did not answer"

  @pending @error @slow @eip-restart @kpi @contract-shape:bounded-change
  Scenario: A message waiting for Slack survives a restart of Foundry
    Given Foundry runs as the operator's process with the Foundry Slack app set up
    And Priya has connected Canzan Labs to Slack
    And Lena has routed New issue on Identity Platform to #auth-dev
    And Slack does not answer
    And Sam has filed "Audit Keycloak client scopes" in Identity Platform
    And Foundry has been killed and started again while Slack was still down
    When Slack answers again
    Then #auth-dev shows AUTH-12's message exactly once

  @pending @error @contract-shape:bounded-change
  Scenario: A message Slack never accepts within a day is flagged on the route
    Given Priya has connected Canzan Labs to Slack
    And Lena has routed New issue on Identity Platform to #auth-dev
    And Slack does not answer
    And Sam has filed "Expire stale LDAP sessions" in Identity Platform
    When 24 hours pass without Slack answering
    Then Lena's route to #auth-dev reads "Needs attention: AUTH-12's message could not be delivered within 24 hours"

  @pending @error @contract-shape:unbounded-preservation
  Scenario: A message given up on is never posted later
    Given Priya has connected Canzan Labs to Slack
    And Lena has routed New issue on Identity Platform to #auth-dev
    And Slack does not answer
    And Sam has filed "Expire stale LDAP sessions" in Identity Platform
    And 24 hours have passed without Slack answering
    When Slack answers again
    Then nothing is posted to #auth-dev within 10 seconds

  @pending @error @contract-shape:unbounded-preservation
  Scenario: A lead without a chat connection is told who can add one
    Given Canzan Labs has no Slack or Matrix connection
    When Lena chooses New route on Identity Platform's integrations page
    Then Lena reads "This workspace has no chat connection yet. Ask a workspace admin to connect Slack or Matrix."

  @pending @error @contract-shape:unbounded-preservation
  Scenario Outline: Only the team's leads and workspace admins configure a project's routes
    Given Priya has connected Canzan Labs to Slack
    When <person> <attempt> on <project>
    Then <person> gets the same not-found page any unknown address gets
    And <project> still has no route
    And <owner> still finds <project>'s integrations page

    Examples:
      | person               | attempt                                 | project           | owner |
      | Sam                  | opens the integrations page             | Identity Platform | Lena  |
      | Sam                  | opens a new Slack route                 | Identity Platform | Lena  |
      | Sam                  | submits a new route to #auth-dev        | Identity Platform | Lena  |
      | Marco                | opens the integrations page             | Identity Platform | Lena  |
      | Marco                | submits a new route to #auth-dev        | Identity Platform | Lena  |
      | Lena                 | opens the integrations page             | Homelab Ops       | Priya |
      | Lena                 | submits a new route to #ops-alerts      | Homelab Ops       | Priya |

  @pending @contract-shape:bounded-change
  Scenario: A workspace admin configures routes on any project of the workspace
    Given Priya has connected Canzan Labs to Slack
    And Priya is creating a Slack route from Homelab Ops to #ops-alerts
    When Priya saves the route
    Then Homelab Ops's integrations page lists the route to #ops-alerts reading "Delivering"

  @pending @error @kpi @contract-shape:unbounded-preservation
  Scenario: Another project's issues never reach the route
    Given Priya has connected Canzan Labs to Slack
    And Lena has routed New issue on Identity Platform to #auth-dev
    When Sam files "Renew cluster certificates" in Homelab Ops
    Then nothing is posted to #auth-dev within 10 seconds

  @pending @edge @contract-shape:unbounded-preservation
  Scenario: A route carries only what happens after it is saved
    Given Priya has connected Canzan Labs to Slack
    And Sam has filed "Rotate LLDAP bind password" in Identity Platform
    And Lena is creating a Slack route from Identity Platform to #auth-dev
    When Lena saves the route
    Then nothing is posted to #auth-dev within 10 seconds

  @pending @edge @contract-shape:pure-function
  Scenario Outline: The channel picker lists the channels Foundry can post to
    Given Priya has connected Canzan Labs to Slack
    And Foundry <membership> the private channel #backend-private
    When Lena opens a new Slack route on Identity Platform
    Then the channel picker offers #auth-dev, #auth-releases and #general
    And the channel picker <offers> #backend-private
    And Lena reads "Private channels appear here after someone runs /invite @Foundry in them."

    Examples:
      | membership          | offers         |
      | is not in           | does not offer |
      | has been invited to | offers         |
