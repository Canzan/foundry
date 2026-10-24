# Feature: external-integration-plugins, slice 02 (US-EIP-03) — a team lead
# chooses exactly which events a route carries: the four v1 chat events of D-2,
# an optional destination-lane filter on Issue moved, and the burst rule D-11
# (DDR-7, DDR-8, ADR-EIP-003).
#
# Authored @pending in DISTILL 2026-10-21 (ADR-025 scaffolded RED).
#
# Harness as in external-integration-plugins-01-connect-slack.feature. Every
# event is produced by the product's own board doors (file, move, reorder,
# comment, edit and delete a comment, delete an issue, rename and delete a
# lane); lanes the scenarios need are preconditions. A route receives only what
# happens after it is saved (DDR-27), so a Given's board action taken before
# the route is saved never reaches it.
#
# Burst copy for a lane delete that MOVES its cards ("7 issues moved to Todo in
# Identity Platform by Lena Park") is DESIGN's UI-3, ratified by the user
# on 2026-10-22 (R-10).

# Copy and behaviour beyond D-13 that these scenarios assert were ratified by
# the user on 2026-10-22 (feature-delta DISTILL "[REF] User ratification
# (2026-10-22)", R-1..R-11, with the exact wording).

@external-integration-plugins @eip-02 @us-eip-03 @real-io
Feature: A team lead chooses exactly which events a route carries

  Background:
    Given the Canzan Labs workspace with these people
      | person       | workspace role | teams                         |
      | Priya Raman  | admin          | Backend as lead, Ops as lead  |
      | Lena Park    | member         | Backend as lead               |
      | Sam Okafor   | member         | Backend as member             |
    And Identity Platform (AUTH) belongs to Backend and Homelab Ops (OPS) belongs to Ops
    And Identity Platform has the lanes Backlog, Todo, In progress, QA, Done and Spikes
    And Identity Platform holds these issues
      | key     | title                              | lane        | filed by   |
      | AUTH-7  | Move Grafana sign-in to Keycloak   | In progress | Sam Okafor |
      | AUTH-8  | Document the LDAP group mapping    | Todo        | Lena Park  |
      | AUTH-9  | Retire the old bind account        | Todo        | Lena Park  |
      | AUTH-10 | Enable MFA for admins              | In progress | Sam Okafor |
    And the next issue filed in Identity Platform becomes AUTH-12
    And it is 14:02 on 21 Oct 2026
    And the instance has the Foundry Slack app set up
    And Priya has connected Canzan Labs to Slack

  @kpi @contract-shape:bounded-change
  Scenario: A route limited to one lane posts only moves into that lane
    Given Lena has routed Issue moved into Done on Identity Platform to #auth-releases
    And Sam has moved AUTH-8 from Todo to In progress
    When Sam moves AUTH-7 from In progress to Done
    Then #auth-releases shows exactly one message, "AUTH-7 Move Grafana sign-in to Keycloak" / "Moved In progress → Done by Sam Okafor · Identity Platform"

  @kpi @contract-shape:bounded-change
  Scenario Outline: Each chosen event posts its own message
    Given Lena has routed <event> on Identity Platform to #auth-dev
    When <action>
    Then #auth-dev shows within 10 seconds a Foundry message "<first_line>" / "<second_line>" <link>

    Examples:
      | event         | action                                                   | first_line                               | second_line                                              | link                  |
      | New issue     | Sam files "Rotate LLDAP bind password" in Identity Platform | AUTH-12 Rotate LLDAP bind password    | New issue in Identity Platform by Sam Okafor · Backlog   | with Open in Foundry  |
      | Issue moved   | Sam moves AUTH-7 from In progress to Done                | AUTH-7 Move Grafana sign-in to Keycloak  | Moved In progress → Done by Sam Okafor · Identity Platform | with Open in Foundry  |
      | New comment   | Lena comments "Can you verify the Grafana login?" on AUTH-7 | AUTH-7 Move Grafana sign-in to Keycloak | Lena Park commented · Identity Platform                | with Open in Foundry  |
      | Issue deleted | Lena deletes AUTH-9                                      | AUTH-9 Retire the old bind account       | Deleted by Lena Park · Identity Platform                 | without a link        |

  @error @contract-shape:unbounded-preservation
  Scenario: Reordering cards within a lane posts nothing
    Given Lena has routed every chat event on Identity Platform to #auth-dev
    When Sam moves AUTH-9 above AUTH-8 inside Todo
    Then nothing is posted to #auth-dev within 10 seconds

  @error @contract-shape:unbounded-preservation
  Scenario Outline: Editing or deleting a comment posts nothing
    Given Lena has routed every chat event on Identity Platform to #auth-dev
    And Lena has commented "Can you verify the Grafana login?" on AUTH-7 and #auth-dev has shown it
    When Lena <changes> that comment
    Then #auth-dev shows no further message within 10 seconds

    Examples:
      | changes |
      | edits   |
      | deletes |

  @edge @contract-shape:pure-function
  Scenario: The route form offers exactly the chat events Foundry announces
    When Lena opens a new Slack route on Identity Platform
    Then the route form offers the events New issue, Issue moved, New comment and Issue deleted, and no other
    And Issue moved offers the lanes Backlog, Todo, In progress, QA, Done and Spikes to limit it to

  @error @contract-shape:unbounded-preservation
  Scenario: A route with no event chosen is not saved
    Given Lena is creating a Slack route from Identity Platform to #auth-dev with no event chosen
    When Lena saves the route
    Then Identity Platform still has no route

  @contract-shape:bounded-change
  Scenario: A lane filter follows its lane through a rename
    Given Lena has routed Issue moved into Done on Identity Platform to #auth-releases
    And Lena has renamed the lane Done to Shipped
    When Sam moves AUTH-10 from In progress to Shipped
    Then #auth-releases shows exactly one message, "AUTH-10 Enable MFA for admins" / "Moved In progress → Shipped by Sam Okafor · Identity Platform"

  @error @contract-shape:bounded-change
  Scenario: A route whose filtered lane is gone asks to be fixed
    Given Lena has routed Issue moved into QA or Done on Identity Platform to #auth-releases
    When Lena deletes the lane QA
    Then Lena's route to #auth-releases reads "Needs attention: the lane QA no longer exists. Choose lanes again."

  @error @contract-shape:unbounded-preservation
  Scenario: A route whose filtered lane is gone posts no moves until it is edited
    Given Lena has routed Issue moved into QA or Done on Identity Platform to #auth-releases
    And Lena has deleted the lane QA
    When Sam moves AUTH-7 from In progress to Done
    Then nothing is posted to #auth-releases within 10 seconds

  @contract-shape:bounded-change
  Scenario Outline: One action that deletes many cards posts one message once there are more than five
    Given Lena has routed Issue deleted on Identity Platform to #auth-dev
    And the lane Spikes holds <cards> cards
    When Lena deletes the lane Spikes and its cards
    Then #auth-dev shows <messages>

    Examples:
      | cards | messages                                                                   |
      | 5     | 5 messages, each "Deleted by Lena Park · Identity Platform"                |
      | 6     | exactly one message, "6 issues deleted from Identity Platform by Lena Park" |
      | 7     | exactly one message, "7 issues deleted from Identity Platform by Lena Park" |

  @contract-shape:bounded-change
  Scenario: Deleting a lane that moves its cards posts one message for the move
    Given Lena has routed Issue moved on Identity Platform to #auth-dev
    And the lane Spikes holds 7 cards
    When Lena deletes the lane Spikes and moves its cards to Todo
    Then #auth-dev shows exactly one message, "7 issues moved to Todo in Identity Platform by Lena Park"
