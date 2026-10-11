# Feature: team-membership, slice 05 (US-TM-06, OQ-3 to OQ-8, D17) — a workspace
# admin picks a default team on the teams page, and an invited newcomer lands on it.
# A newcomer is never refused because of the default: with none, or with one that
# has since been deleted, they land on no team, which is today's behaviour. Setting
# a default never puts existing members on it (OQ-8).
#
# Authored @pending in DISTILL 2026-10-09 (ADR-025 scaffolded RED); un-pended in
# DELIVER 05-02. The invite is issued and accepted through the shipped doors (POST
# /workspace/invites, then GET and POST /invites/accept); only the team placement is
# new.
#
# Harness and oracles as in team-membership-01: see that file's header. Tomás is the
# newcomer here, so he is not in the roster.

@team-membership @tm-05 @real-io
Feature: A workspace admin picks a default team, and an invited newcomer lands on it

  Background:
    Given Canzan Labs is the instance's original workspace, with these teams:
      | team       | project           | key  |
      | General    | Sandbox           | SBX  |
      | Backend    | Identity Platform | AUTH |
      | Ops        | Runbooks          | OPS  |
      | Onboarding |                   |      |
    And these people belong to Canzan Labs:
      | person        | email                        | workspace role | teams                          | called                 |
      | Priya Raman   | priya.raman@canzan.dev       | admin          | Backend as Member              |                        |
      | Jeff Bailey   | jeffabailey@gmail.com        | admin          | General as Lead, Ops as Lead, Onboarding as Lead | Jeff |
      | Jeff Bailey   | jeff@unintelligent-design.us | member         |                                | Jeff's Keycloak account |
      | Lena Park     | lena.park@canzan.dev         | member         | Backend as Lead, Ops as Member |                        |
      | Marco Bianchi | marco.bianchi@canzan.dev     | member         | Ops as Member                  |                        |
    And Globex is another workspace, whose team Payments is led by Hugo Weber (hugo.weber@globex.example)

  @us-tm-06 @driving_port @contract-shape:bounded-change
  Scenario: A workspace admin picks the default team for new members, and no one already here is moved
    When Priya sets General as the default team for new members
    Then she sees "New members will join General."
    And Canzan Labs's default team is now General, and nothing else changed
    And the teams page shows General as the default team for new members
    And Jeff's Keycloak account is still on no team

  @us-tm-06 @edge @contract-shape:bounded-change
  Scenario: A workspace admin clears the default team
    Given Priya has set General as the default team for new members
    When Priya sets No team as the default team for new members
    Then she sees "New members join no team."
    And Canzan Labs's default team is now No team, and nothing else changed
    And the teams page shows No team as the default team for new members

  @us-tm-06 @kpi @contract-shape:bounded-change
  Scenario: An invited newcomer lands on the default team and opens its board
    Given Priya has set General as the default team for new members
    When Tomás Rivera accepts an invite to Canzan Labs at tomas.rivera@canzan.dev
    Then Tomás Rivera has a new account in Canzan Labs and is a Member of General, and nothing else changed
    And Tomás Rivera sees Sandbox on his dashboard and opens its board

  @us-tm-06 @edge @contract-shape:bounded-change
  Scenario: Without a default team, an invited newcomer lands as today
    Given Canzan Labs has no default team
    When Tomás Rivera accepts an invite to Canzan Labs at tomas.rivera@canzan.dev
    Then Tomás Rivera has a new account in Canzan Labs and is on no team, and nothing else changed
    And Tomás Rivera's dashboard says "You're not on any team yet — ask a workspace admin to add you."

  @us-tm-06 @error @contract-shape:bounded-change
  Scenario: A default team that no longer exists never stops a newcomer joining
    Given the Onboarding team, once Canzan Labs's default, has since been deleted by hand
    When Tomás Rivera accepts an invite to Canzan Labs at tomas.rivera@canzan.dev
    Then Tomás Rivera has a new account in Canzan Labs and is on no team, and nothing else changed
    And Tomás Rivera is signed in
    And the teams page shows No team as the default team for new members

  @us-tm-06 @error @kpi @contract-shape:unbounded-preservation
  Scenario Outline: Only a workspace admin can set the default, and only to one of Canzan Labs's teams
    Given Priya has set General as the default team for new members
    When <who> submits <team> as Canzan Labs's default team
    Then the answer is the page that never existed
    And no one's membership changed
    And the teams page shows General as the default team for new members

    Examples:
      | who                  | team                       |
      | a signed-out visitor | Backend                    |
      | Lena                 | Backend                    |
      | Marco                | Backend                    |
      | Priya                | Payments                   |
      | Priya                | a team that does not exist |

  @us-tm-06 @edge @contract-shape:unbounded-preservation
  Scenario: Only a workspace admin is offered the default team setting
    When Lena opens the teams page
    Then the teams page has no default team setting
    And Priya's teams page offers the default team setting
