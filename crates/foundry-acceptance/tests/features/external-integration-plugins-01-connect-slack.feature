# Feature: external-integration-plugins, slice 01 (US-EIP-01) — a workspace
# admin connects the workspace's Slack in one approval, through the instance's
# own Slack app (OD-3, DDR-10, ADR-EIP-005).
#
# Authored @pending in DISTILL 2026-10-21 (ADR-025 scaffolded RED). DELIVER only
# un-pends. Copy is DISCUSS D-13 (locked); paths are DESIGN [REF] Driving Ports.
#
# Harness: the production router in process (InProcHarness) on the shared
# testcontainers Postgres 16, with the Slack HTTP double (support::slack_double,
# DEVOPS-2) reached through SLACK_API_BASE_URL / SLACK_AUTHORIZE_URL. "Slack's
# approval screen" is the double's authorize address: approving or cancelling
# there redirects back to Foundry's return address exactly as Slack does.
# Every outcome is observed through a door: Foundry's pages, what the Slack
# double was asked, and the Notifier recorder. The universe (support::
# external_integration_plugins::Universe) is read through the same doors.

# Copy and behaviour beyond D-13 that these scenarios assert were ratified by
# the user on 2026-10-22 (feature-delta DISTILL "[REF] User ratification
# (2026-10-22)", R-1..R-11, with the exact wording).

@external-integration-plugins @eip-01 @us-eip-01 @real-io
Feature: A workspace admin connects the workspace's Slack in one approval

  Background:
    Given the Canzan Labs workspace with these people
      | person       | workspace role | teams                         |
      | Priya Raman  | admin          | Backend as lead, Ops as lead  |
      | Lena Park    | member         | Backend as lead               |
      | Sam Okafor   | member         | Backend as member             |
      | Marco        | member         |                               |
    And Identity Platform (AUTH) belongs to Backend and Homelab Ops (OPS) belongs to Ops
    And it is 14:02 on 21 Oct 2026

  @pending @driving_port @kpi @contract-shape:bounded-change
  Scenario: A workspace admin connects Slack and sees the connection
    Given the instance has the Foundry Slack app set up
    And Canzan Labs has no Slack connection
    When Priya chooses Add to Slack and approves Foundry for the Slack workspace "Canzan Labs"
    Then Priya is back on the workspace integrations page and reads "Connected to Canzan Labs (canzanlabs.slack.com) · by Priya Raman · 21 Oct 2026"
    And the workspace's integration activity reads "21 Oct 2026 · Priya Raman connected Slack (Canzan Labs)"
    And nothing changed for Canzan Labs except its Slack connection and one activity line

  @pending @error @contract-shape:unbounded-preservation
  Scenario Outline: An instance that cannot hold a Slack connection says what is missing
    Given the instance has <setup>
    When Priya opens the workspace integrations page
    Then Priya reads "<message>"
    And Priya is offered no Add to Slack

    Examples:
      | setup                                           | message                                                                                                  |
      | no Foundry Slack app set up                     | Slack is not set up on this Foundry yet. An instance operator adds the Foundry Slack app once. How to set up Slack |
      | the Foundry Slack app set up but no secrets key | Slack cannot be connected on this Foundry yet: an instance operator needs to set the secrets key.        |

  @pending @error @contract-shape:unbounded-preservation
  Scenario: A cancelled approval connects nothing
    Given the instance has the Foundry Slack app set up
    And Canzan Labs has no Slack connection
    When Priya chooses Add to Slack and cancels on Slack's approval screen
    Then Priya reads "Slack was not connected. Nothing changed."
    And nothing changed for Canzan Labs

  @pending @error @contract-shape:unbounded-preservation
  Scenario Outline: A forged return from Slack connects nothing
    Given the instance has the Foundry Slack app set up
    And Priya holds a Slack return address with <state>
    When Priya opens that Slack return address
    Then Priya reads "Slack was not connected. Nothing changed."
    And nothing changed for Canzan Labs
    And Slack was never asked to exchange that return's code

    Examples:
      | state                               |
      | a state Foundry never issued        |
      | a state that was already used       |
      | a state older than 10 minutes       |
      | a state issued to another session   |

  @pending @error @contract-shape:unbounded-preservation
  Scenario Outline: Only a workspace admin reaches the workspace's Slack doors
    Given the instance has the Foundry Slack app set up
    When <person> opens <door>
    Then <person> gets the same not-found page any unknown address gets
    And nothing changed for Canzan Labs
    And Priya still finds the workspace integrations page

    Examples:
      | person                | door                                |
      | Lena                  | the workspace integrations page     |
      | Lena                  | the Slack setup page                |
      | Lena                  | Add to Slack                        |
      | Lena                  | the Slack return address            |
      | Marco                 | the workspace integrations page     |
      | Marco                 | Add to Slack                        |

  @pending @edge @contract-shape:pure-function
  Scenario: The Slack setup page gives the operator this instance's addresses and nothing secret
    Given the instance has the Foundry Slack app set up
    When Priya opens the Slack setup page
    Then the Slack app manifest names this instance's Slack return address and Slack events address
    And it asks Slack only for the bot permissions slices 01 to 03 need
    And no Slack credential appears on it

  @pending @error @kpi @contract-shape:unbounded-preservation
  Scenario Outline: The Slack credential never leaves the server
    Given Priya has connected Canzan Labs to Slack
    And Lena has routed New issue on Identity Platform to #auth-dev
    When <person> opens <page>
    Then no Slack credential appears anywhere in what Priya or Lena receives

    Examples:
      | person | page                                         |
      | Priya  | the workspace integrations page              |
      | Priya  | the Slack setup page                         |
      | Lena   | Identity Platform's integrations page        |
      | Lena   | a new Slack route on Identity Platform       |
