# Feature: external-integration-plugins, operations (slices 01 and 03) — what
# the instance operator relies on: no credential in any log (SC-3,
# EIP-KPI-5), the secrets key's degraded start and refusals (DD-2, DDR-9,
# ADR-EIP-004), key rotation without anyone reconnecting (`foundry doctor
# rotate-secrets`, C-11), delivery counters with closed labels (DDR-22,
# SC-9), and correct delivery when Foundry runs as several replicas
# (DDR-4, DDR-5, DDR-23, ADR-EIP-002).
#
# Story mapping (@cross-cutting): each scenario also carries the story whose
# AC it backs: US-EIP-01 (SC-3 credentials, secrets key, settings, rotation),
# US-EIP-02 (SC-1/SC-2 delivery across replicas), US-EIP-05 (pacing, counters),
# US-EIP-06 (one admin email).
#
# Authored @pending in DISTILL 2026-10-21 (ADR-025 scaffolded RED).
#
# Lanes (DEVOPS environment matrix): `@eip-metrics` runs the real, prebuilt
# `foundry` binary as a subprocess (the in-process harness installs no
# metrics recorder) and scans its own stdout and stderr; rebuild and warm the
# binary first (C-2). `@eip-replica` runs two in-process replicas on one
# schema (support::multi_replica_harness) with the advanceable injected Clock,
# so a 45-second lease is crossed without waiting.
#
# The wording for an unavailable connection (K-19: "Slack is unavailable: this
# Foundry cannot read its saved connection. An instance operator needs to
# check the secrets key.") is DESIGN's UI-1 / DD-2, ratified by the user on
# 2026-10-22 (R-2). Metric names appear only inside the steps; the Gherkin speaks of
# "the operator's delivery counters".

# Copy and behaviour beyond D-13 that these scenarios assert were ratified by
# the user on 2026-10-22 (feature-delta DISTILL "[REF] User ratification
# (2026-10-22)", R-1..R-11, with the exact wording).

@external-integration-plugins @cross-cutting @real-io
Feature: The operator keeps integrations safe and observable

  Background:
    Given the Canzan Labs workspace with these people
      | person       | workspace role | teams                         |
      | Priya Raman  | admin          | Backend as lead, Ops as lead  |
      | Lena Park    | member         | Backend as lead               |
      | Sam Okafor   | member         | Backend as member             |
    And Identity Platform (AUTH) belongs to Backend and Homelab Ops (OPS) belongs to Ops
    And the next issue filed in Identity Platform becomes AUTH-12
    And it is 14:02 on 21 Oct 2026

  @us-eip-01 @eip-01 @eip-metrics @error @kpi @contract-shape:unbounded-preservation
  Scenario: No credential appears in anything Foundry writes to its logs
    Given Foundry runs as the operator's process with the Foundry Slack app set up
    And Priya has connected Canzan Labs to Slack
    And Lena has routed New issue on Identity Platform to #auth-dev
    And Slack will answer Foundry's next post to #auth-dev with the token was revoked
    When Sam files "Rotate LLDAP bind password" in Identity Platform
    Then Foundry's output never contains any Slack credential or the secrets key

  @us-eip-01 @eip-01 @eip-metrics @error @contract-shape:unbounded-preservation
  Scenario: A missing secrets key keeps the board up and says what it cannot open
    Given Foundry runs as the operator's process with the Foundry Slack app set up
    And Priya has connected Canzan Labs to Slack
    And Foundry has been started again without its secrets key
    When Priya opens the workspace integrations page
    Then Priya reads "Slack is unavailable: this Foundry cannot read its saved connection. An instance operator needs to check the secrets key."
    And the board of Identity Platform still opens for Sam
    And the operator's health signals report the secrets key as degraded
    And Canzan Labs' Slack is not marked lost and Priya receives no email about it

  @us-eip-01 @eip-01 @eip-metrics @error @contract-shape:unbounded-preservation
  Scenario Outline: A setting that cannot be right stops Foundry from starting, and says which
    When the operator starts Foundry with <setting>
    Then Foundry refuses to start, naming <name>
    And Foundry's output never contains any Slack credential or the secrets key

    Examples:
      | setting                                              | name                    |
      | a secrets key that is not 32 bytes                   | the secrets key         |
      | only two of the three Foundry Slack app settings     | the Slack app settings  |
      | a delivery lease shorter than 35 seconds             | the delivery lease      |

  @us-eip-01 @eip-01 @eip-metrics @driving_port @contract-shape:bounded-change
  Scenario: The operator rotates the secrets key without anyone reconnecting
    Given Foundry runs as the operator's process with the Foundry Slack app set up
    And Priya has connected Canzan Labs to Slack
    And Lena has routed New issue on Identity Platform to #auth-dev
    And Foundry has been started again with a new secrets key and the old one as the previous key
    When the operator runs foundry doctor rotate-secrets
    Then the command reports 1 credential sealed again under the new key

  @pending @us-eip-05 @eip-03 @eip-metrics @kpi @contract-shape:bounded-change
  Scenario: The operator's delivery counters start at zero and count each outcome
    Given Foundry runs as the operator's process with the Foundry Slack app set up
    And the operator's delivery counters all read zero
    And Priya has connected Canzan Labs to Slack
    And Lena has routed New issue on Identity Platform to #auth-dev
    When Sam files "Rotate LLDAP bind password" in Identity Platform
    Then the operator's delivery counters show one Slack New issue delivered
    And the delivery counters name no channel, room, project, workspace, address or person

  @us-eip-02 @eip-01 @eip-replica @kpi @contract-shape:bounded-change
  Scenario: Two replicas deliver each event once, in the order it happened
    Given Foundry runs as two replicas on one database with the Foundry Slack app set up
    And Priya has connected Canzan Labs to Slack
    And Lena has routed every chat event on Identity Platform to #auth-dev
    And Sam has filed "Rotate LLDAP bind password" in Identity Platform
    When Sam moves AUTH-12 from Backlog to Done
    Then #auth-dev shows AUTH-12's New issue message, "New issue in Identity Platform by Sam Okafor · Backlog", before its Moved message, each exactly once

  @us-eip-02 @eip-01 @eip-replica @error @kpi @contract-shape:bounded-change
  Scenario: A message whose sender stopped in the middle of a send is not lost
    Given Foundry runs as two replicas on one database with the Foundry Slack app set up
    And Priya has connected Canzan Labs to Slack
    And Lena has routed New issue on Identity Platform to #auth-dev
    And Slack holds Foundry's post without answering
    And Sam has filed "Rotate LLDAP bind password" in Identity Platform
    And every replica stopped in the middle of sending AUTH-12's message and a fresh replica started
    And Slack answers again
    When 45 seconds pass
    Then #auth-dev shows AUTH-12's message
    And the history of the route to #auth-dev reads "New issue · AUTH-12 · Delivered after 2 attempts"

  @pending @us-eip-06 @eip-03 @eip-replica @error @kpi @contract-shape:bounded-change
  Scenario: Two replicas email each workspace admin once when the connection is lost
    Given Foundry runs as two replicas on one database with the Foundry Slack app set up
    And Priya has connected Canzan Labs to Slack
    And Lena has routed every chat event on Identity Platform to #auth-dev
    And Lena has routed every chat event on Identity Platform to #auth-releases
    And the Foundry app has been removed from Canzan Labs' Slack without Foundry being told
    When Sam files "Rotate LLDAP bind password" in Identity Platform
    Then Priya has received exactly one email "Foundry lost its Slack connection for Canzan Labs"

  @pending @us-eip-05 @eip-03 @eip-replica @edge @contract-shape:bounded-change
  Scenario: Two routes to one channel never post more than once a second
    Given Foundry runs as two replicas on one database with the Foundry Slack app set up
    And Priya has connected Canzan Labs to Slack
    And Lena has routed every chat event on Identity Platform to #auth-dev
    And Priya has routed every chat event on Identity Platform to #auth-dev with Link only
    When Sam files 3 issues in Identity Platform at once
    Then #auth-dev receives 6 messages, no two of them less than one second apart

  @us-eip-01 @eip-01 @eip-metrics @driving_port @contract-shape:bounded-change
  Scenario: After a rotation Foundry opens the connection with only the new key
    Given Foundry runs as the operator's process with the Foundry Slack app set up
    And Priya has connected Canzan Labs to Slack
    And Lena has routed New issue on Identity Platform to #auth-dev
    And Foundry has been started again with a new secrets key and the old one as the previous key
    When the operator finishes the key rotation: runs foundry doctor rotate-secrets, then starts Foundry with only the new secrets key
    Then the operator's health signals report the secrets key as healthy
    And the workspace integrations page reads "Connected to Canzan Labs"

  @us-eip-02 @eip-01 @eip-replica @error @contract-shape:bounded-change
  Scenario: A late answer from a replica whose claim expired is recorded once
    Given Foundry runs as two replicas on one database with the Foundry Slack app set up
    And Priya has connected Canzan Labs to Slack
    And Lena has routed New issue on Identity Platform to #auth-dev
    And Slack holds Foundry's post without answering
    And Sam has filed "Rotate LLDAP bind password" in Identity Platform
    And 45 seconds have passed
    When Slack answers again
    Then the history of the route to #auth-dev reads "New issue · AUTH-12 · Delivered after 2 attempts"
    And the history of the route to #auth-dev lists AUTH-12 exactly once
    And both of Slack's posts for AUTH-12 carried the same message

  @us-eip-02 @eip-01 @eip-replica @kpi @contract-shape:bounded-change
  Scenario: Two replicas deliver a burst of events once each
    Given Foundry runs as two replicas on one database with the Foundry Slack app set up
    And Priya has connected Canzan Labs to Slack
    And Lena has routed New issue on Identity Platform to #auth-dev
    When Sam files 5 issues in Identity Platform at once
    Then #auth-dev shows each of AUTH-12, AUTH-13, AUTH-14, AUTH-15 and AUTH-16 exactly once

  # Single-replica failover (DIST-22, DELIVER 01-11, gate G4): only the replica
  # holding AUTH-12's claim stops, without hearing Slack's answer; the replica
  # that kept running takes the delivery over once the 45-second lease lapses.
  # Scenario 66 stops every replica and 123 keeps both running; this one is
  # the survivor taking over. The seam that names the claiming replica is
  # DELIVER 01-11's to build.
  @us-eip-02 @eip-01 @eip-replica @error @kpi @contract-shape:bounded-change
  Scenario: The replica that kept running delivers a message its stopped partner was sending
    Given Foundry runs as two replicas on one database with the Foundry Slack app set up
    And Priya has connected Canzan Labs to Slack
    And Lena has routed New issue on Identity Platform to #auth-dev
    And Slack holds Foundry's post without answering
    And Sam has filed "Rotate LLDAP bind password" in Identity Platform
    And the replica sending AUTH-12's message stopped before hearing back, and the other replica kept running
    And Slack answers again
    When 45 seconds pass
    Then #auth-dev shows AUTH-12's message exactly once
    And both of Slack's posts for AUTH-12 carried the same message
    And the history of the route to #auth-dev reads "New issue · AUTH-12 · Delivered after 2 attempts"
    And the history of the route to #auth-dev lists AUTH-12 exactly once

  # Live claim (DISTILL addendum, 2026-10-24; seeded fault A): both replicas
  # keep running and Slack holds the first post, so the replica sending
  # AUTH-12's message still holds its claim while it waits for Slack. The other
  # replica must not send the message too. 131 cannot show this: its claimant
  # stops, and the survivor's take-over reads the same whether it waited for
  # the 45-second lease or not.
  @us-eip-02 @eip-01 @eip-replica @error @contract-shape:bounded-change
  Scenario: The other replica never sends a message a running replica is still sending
    Given Foundry runs as two replicas on one database with the Foundry Slack app set up
    And Priya has connected Canzan Labs to Slack
    And Lena has routed New issue on Identity Platform to #auth-dev
    And Slack holds Foundry's post without answering
    When Sam files "Rotate LLDAP bind password" in Identity Platform
    Then Slack is asked to post AUTH-12's message only once within 10 seconds
