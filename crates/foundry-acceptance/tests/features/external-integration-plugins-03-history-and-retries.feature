# Feature: external-integration-plugins, slice 03 (US-EIP-05) — a team lead
# sees every delivery, and transient failures fix themselves: per-route
# history, backoff with "retry after", per-route order, a 24-hour give-up, and
# Foundry's own words for every reason (OD-7, DDR-5, DDR-19, ADR-EIP-002).
#
# Authored @pending in DISTILL 2026-10-21 (ADR-025 scaffolded RED).
#
# Harness as in external-integration-plugins-01-connect-slack.feature. Time is
# the MockClock (the injected Clock): waits are advanced, never slept.
#
# Slack's answers come from the double's closed list (DEVOPS-2), one per
# classifier row DESIGN names: rate limited with Retry-After, no answer, an
# HTML error page, an `ok: false` code Foundry does not know, and the named
# route- and connection-level codes. The words a route shows for codes no
# DISCUSS example quotes (channel_not_found; an unknown code) are the product
# owner's to confirm (DESIGN UI-7): those rows assert only the status's first
# word.

@external-integration-plugins @eip-03 @us-eip-05 @real-io
Feature: A team lead sees every delivery, and transient failures fix themselves

  Background:
    Given the Canzan Labs workspace with these people
      | person       | workspace role | teams                         |
      | Priya Raman  | admin          | Backend as lead, Ops as lead  |
      | Lena Park    | member         | Backend as lead               |
      | Sam Okafor   | member         | Backend as member             |
      | Marco        | member         |                               |
    And Identity Platform (AUTH) belongs to Backend and Homelab Ops (OPS) belongs to Ops
    And Identity Platform holds these issues
      | key    | title                              | lane        | filed by   |
      | AUTH-7 | Move Grafana sign-in to Keycloak   | In progress | Sam Okafor |
    And the next issue filed in Identity Platform becomes AUTH-12
    And it is 14:02 on 21 Oct 2026
    And the instance has the Foundry Slack app set up
    And Priya has connected Canzan Labs to Slack
    And Lena has routed every chat event on Identity Platform to #auth-dev

  @pending @kpi @contract-shape:pure-function
  Scenario: Every delivery is listed with its outcome
    Given Sam has filed "Rotate LLDAP bind password" in Identity Platform and #auth-dev has shown it
    When Lena opens the history of the route to #auth-dev
    Then the route's history shows "14:02 · New issue · AUTH-12 · Delivered"

  @pending @error @kpi @contract-shape:bounded-change
  Scenario: A rate-limited delivery waits and then arrives
    Given Slack asked Foundry to retry Sam's comment on AUTH-7 after 30 seconds
    When 30 seconds pass
    Then #auth-dev shows AUTH-7's comment message
    And the history of the route to #auth-dev reads "New comment · AUTH-7 · Delivered after 2 attempts (Slack asked Foundry to slow down)"

  @pending @error @contract-shape:bounded-change
  Scenario: Messages delayed by an outage arrive in the order they happened
    Given Slack does not answer
    And Sam has filed "Pin the Keycloak realm export" in Identity Platform
    And Sam has moved AUTH-12 from Backlog to In progress
    And 2 hours have passed
    When Slack answers again
    Then #auth-dev shows AUTH-12's New issue message before its Moved message

  @pending @error @contract-shape:pure-function
  Scenario: After 24 hours of failure Foundry stops and says so
    Given Slack does not answer
    And Sam has filed "Expire stale LDAP sessions" in Identity Platform
    And 24 hours have passed without Slack answering
    When Lena opens the history of the route to #auth-dev
    Then the route's history shows "New issue · AUTH-12 · Failed: gave up after 24 hours (Slack did not answer)"

  @pending @error @contract-shape:bounded-change
  Scenario Outline: What Foundry does with each answer Slack can give
    Given Slack will answer Foundry's next post to #auth-dev with <answer>
    And Sam has filed "Rotate LLDAP bind password" in Identity Platform
    When 2 minutes pass
    Then Lena's route to #auth-dev reads "<status>"
    And Slack was asked to post AUTH-12's message <attempts>

    Examples: transient answers are retried
      | answer                                  | status                                          | attempts                         |
      | no answer                               | Retrying: Slack did not answer                  | more than once                   |
      | a request to slow down for 30 seconds   | Delivering                                      | twice                            |

    Examples: refusals that concern the channel stop that route
      | answer                                  | status                                                                    | attempts |
      | the channel is archived                 | Needs attention: Foundry can no longer post to #auth-dev (the channel was archived) | once |
      | Foundry is not in the channel           | Needs attention: Foundry is no longer in #auth-dev. Invite @Foundry back, then Resume. | once |

    Examples: refusals that concern the connection pause every route
      | answer                                  | status                         | attempts |
      | the token was revoked                   | Paused: connection lost        | once     |
      | the Slack workspace is inactive         | Paused: connection lost        | once     |
      | the credential is not valid             | Paused: connection lost        | once     |

  @pending @error @contract-shape:bounded-change
  Scenario Outline: Answers Foundry cannot read are retried, never treated as final
    Given Slack will answer Foundry's next post to #auth-dev with <answer>
    When Sam files "Rotate LLDAP bind password" in Identity Platform
    Then Lena's route to #auth-dev reads a status starting "Retrying: "

    Examples:
      | answer                                  |
      | an error page instead of an answer      |
      | a refusal Foundry does not know         |

  @pending @error @contract-shape:bounded-change
  Scenario: A channel Slack cannot find stops that route only
    Given Lena has routed every chat event on Identity Platform to #auth-releases
    And Slack will answer Foundry's next post to #auth-dev with the channel was not found
    When Sam files "Rotate LLDAP bind password" in Identity Platform
    Then Lena's route to #auth-dev reads a status starting "Needs attention: "
    And #auth-releases shows AUTH-12's message

  @pending @error @kpi @contract-shape:unbounded-preservation
  Scenario: History speaks Foundry's words, never Slack's
    Given Slack will answer Foundry's next post to #auth-dev with a refusal carrying its own raw text
    And Sam has filed "Rotate LLDAP bind password" in Identity Platform
    When Lena opens the history of the route to #auth-dev
    Then the history shows none of the raw Slack text and no Slack credential

  @pending @error @contract-shape:unbounded-preservation
  Scenario Outline: Only those who manage the route see its history
    When <person> opens the history of the route to #auth-dev
    Then <person> gets the same not-found page any unknown address gets
    And Lena still finds the history of the route to #auth-dev

    Examples:
      | person               |
      | Sam                  |
      | Marco                |

  @pending @edge @contract-shape:bounded-change
  Scenario Outline: Beyond the newest 100, the history keeps a delivery for 30 days
    Given Sam has filed "The oldest delivered issue" in Identity Platform and #auth-dev has shown it
    And <age> days have passed
    And Sam has filed 100 issues in Identity Platform and #auth-dev has shown them
    When 2 hours pass
    Then the history of the route to #auth-dev <lists> AUTH-12

    Examples:
      | age | lists           |
      | 29  | still lists     |
      | 31  | no longer lists |
