# Feature: external-integration-plugins, slice 03 (US-EIP-06) — a broken
# connection is flagged at once and fixed with one reconnect: connection-level
# refusals pause every route and email the workspace admins once (D-15,
# DDR-11, DDR-23, EIP-KPI-6); channel-level refusals mark one route; Slack's
# signed uninstall events are believed only when they are Slack's (ADR-EIP-005
# decision 6).
#
# Authored @pending in DISTILL 2026-10-21 (ADR-025 scaffolded RED).
#
# Harness as in external-integration-plugins-01-connect-slack.feature. The
# admin email goes through the shipped Notifier (a new suppressible account
# event, DDR-23) and is read from the harness's delivery recorder. Its subject
# ("Foundry lost its Slack connection for Canzan Labs") is DESIGN's
# UI-5, ratified by the user on 2026-10-22 (R-10). "Slack tells Foundry"
# is the double posting a signed Events API envelope to Foundry's public
# events address with the test signing secret (real HMAC, DEVOPS-2).

# Copy and behaviour beyond D-13 that these scenarios assert were ratified by
# the user on 2026-10-22 (feature-delta DISTILL "[REF] User ratification
# (2026-10-22)", R-1..R-11, with the exact wording).

@external-integration-plugins @eip-03 @us-eip-06 @real-io
Feature: A broken connection is flagged at once and fixed with one reconnect

  Background:
    Given the Canzan Labs workspace with these people
      | person       | workspace role | teams                         |
      | Priya Raman  | admin          | Backend as lead, Ops as lead  |
      | Lena Park    | member         | Backend as lead               |
      | Sam Okafor   | member         | Backend as member             |
    And Identity Platform (AUTH) belongs to Backend and Homelab Ops (OPS) belongs to Ops
    And the next issue filed in Identity Platform becomes AUTH-12
    And it is 14:02 on 21 Oct 2026
    And the instance has the Foundry Slack app set up
    And Priya has connected Canzan Labs to Slack
    And Lena has routed every chat event on Identity Platform to #auth-dev
    And Lena has routed every chat event on Identity Platform to #auth-releases
    And Priya has routed every chat event on Homelab Ops to #ops-alerts

  @pending @error @kpi @contract-shape:bounded-change
  Scenario: A removed Slack app pauses the routes and tells the workspace admins
    Given the Foundry app has been removed from Canzan Labs' Slack without Foundry being told
    When Sam files "Rotate LLDAP bind password" in Identity Platform
    Then the workspace integrations page tells Priya "Foundry lost its Slack connection for Canzan Labs" and "Routes are paused. Reconnect"
    And the routes to #auth-dev, #auth-releases and #ops-alerts each read "Paused: connection lost"
    And Priya receives one email "Foundry lost its Slack connection for Canzan Labs" within 5 minutes
    And Lena receives no email about it

  @pending @error @kpi @contract-shape:bounded-change
  Scenario: Slack telling Foundry the app was uninstalled pauses the routes at once
    When Slack tells Foundry the Foundry app was uninstalled from Canzan Labs
    Then the workspace integrations page reads "Foundry lost its Slack connection for Canzan Labs: the Foundry app was removed from Slack. Routes are paused. Reconnect"
    And the routes to #auth-dev, #auth-releases and #ops-alerts each read "Paused: connection lost"
    And Priya receives one email "Foundry lost its Slack connection for Canzan Labs" within 5 minutes

  @pending @error @contract-shape:unbounded-preservation
  Scenario Outline: Messages to Foundry's Slack events address that are not Slack's change nothing
    When <sender> posts <message> to Foundry's Slack events address
    Then Foundry <answers> the message
    And the workspace integrations page reads "Connected to Canzan Labs"
    And Priya receives no email about the Slack connection

    Examples:
      | sender  | message                                                        | answers                               |
      | someone | an uninstall of Canzan Labs that is not signed                 | refuses as unauthorised                |
      | someone | an uninstall of Canzan Labs signed with the wrong secret       | refuses as unauthorised                |
      | Slack   | an uninstall of Canzan Labs signed more than 5 minutes ago     | refuses as unauthorised                |
      | Slack   | an uninstall of another Slack workspace                        | acknowledges without acting on         |
      | Slack   | an uninstall of another Slack app in Canzan Labs               | acknowledges without acting on         |

  @pending @edge @contract-shape:bounded-change
  Scenario: Slack telling Foundry twice still sends one email
    Given Slack has told Foundry the Foundry app was uninstalled from Canzan Labs
    When Slack tells Foundry the Foundry app was uninstalled from Canzan Labs
    Then Priya has received exactly one email "Foundry lost its Slack connection for Canzan Labs"

  @pending @edge @contract-shape:pure-function
  Scenario: Slack can confirm Foundry's events address
    When Slack verifies Foundry's Slack events address
    Then Slack's confirmation of Foundry's events address succeeds

  @pending @kpi @contract-shape:bounded-change
  Scenario: Reconnecting resumes the routes and delivers what was missed
    Given Slack has told Foundry the Foundry app was uninstalled from Canzan Labs
    And Sam has filed "Pin the Keycloak realm export" in Identity Platform
    And Sam has filed "Expire stale LDAP sessions" in Identity Platform
    And 2 hours have passed
    When Priya reconnects Slack and approves Foundry for the Slack workspace "Canzan Labs"
    Then the routes to #auth-dev, #auth-releases and #ops-alerts each read "Delivering"
    And #auth-dev shows AUTH-12's message before AUTH-13's message

  @pending @error @contract-shape:unbounded-preservation
  Scenario: Reconnecting to the wrong Slack workspace changes nothing
    Given Slack has told Foundry the Foundry app was uninstalled from Canzan Labs
    When Priya reconnects Slack and approves Foundry for the Slack workspace "Canzan Personal"
    Then Priya reads "This is a different Slack workspace (Canzan Personal). Routes stay paused. Reconnect to Canzan Labs, or disconnect."
    And the routes to #auth-dev, #auth-releases and #ops-alerts each read "Paused: connection lost"

  @pending @error @contract-shape:bounded-change
  Scenario Outline: A problem with one channel affects only its own route
    Given <problem>
    When Sam files "Rotate LLDAP bind password" in Identity Platform
    Then Lena's route to #auth-dev reads "<status>"
    And #auth-releases shows AUTH-12's message
    And Lena's route to #auth-releases reads "Delivering" with the time of that delivery

    Examples:
      | problem                                         | status                                                                                  |
      | #auth-dev has been archived in Slack            | Needs attention: Foundry can no longer post to #auth-dev (the channel was archived)      |
      | Foundry has been removed from #auth-dev in Slack | Needs attention: Foundry is no longer in #auth-dev. Invite @Foundry back, then Resume. |

  @pending @error @contract-shape:bounded-change
  Scenario Outline: Resuming a route checks its channel again
    Given Foundry has been removed from #auth-dev in Slack
    And Sam has filed "Rotate LLDAP bind password" in Identity Platform
    And Lena's route to #auth-dev needs attention
    And Foundry <invited> #auth-dev in Slack
    When Lena resumes the route to #auth-dev
    Then <outcome>

    Examples:
      | invited                       | outcome                                                                                   |
      | has been invited back to      | #auth-dev shows AUTH-12's message                                                        |
      | has still not been invited to | Lena's route to #auth-dev reads "Needs attention: Foundry is no longer in #auth-dev. Invite @Foundry back, then Resume." |

  @pending @edge @contract-shape:bounded-change
  Scenario: A muted workspace admin is not emailed, and the page still says what broke
    Given Priya has muted Canzan Labs
    When Slack tells Foundry the Foundry app was uninstalled from Canzan Labs
    Then Priya receives no email about the Slack connection
    And the workspace integrations page reads "Foundry lost its Slack connection for Canzan Labs: the Foundry app was removed from Slack. Routes are paused. Reconnect"
