# Feature: team-membership, slice 06 (US-TM-07, OQ-5, OQ-6, D17) — a newcomer
# provisioned by a first Keycloak sign-in lands on the original workspace's default
# team. Linking an existing account adds no team. An unset or deleted default never
# refuses or fails the sign-in.
#
# Authored @pending in DISTILL 2026-10-09 (ADR-025 scaffolded RED); un-pended in
# DELIVER 06-01. The identity provider is the in-process Keycloak double over a real
# socket (the keycloak-sso lane's harness), with provisioning on for the
# "foundry-user" realm role. Canzan Labs is the instance's original (oldest)
# workspace, so provisioning joins it.
#
# Harness and oracles as in team-membership-01: see that file's header. Tomás is the
# newcomer here, so he is not in the roster.

@team-membership @tm-06 @real-io
Feature: A newcomer provisioned by a Keycloak sign-in lands on the default team

  Background:
    Given Canzan Labs is the instance's original workspace, with these teams:
      | team       | project           | key  |
      | General    | Sandbox           | SBX  |
      | Backend    | Identity Platform | AUTH |
      | Ops        | Runbooks          | OPS  |
      | Onboarding |                   |      |
    And these people belong to Canzan Labs:
      | person        | email                    | workspace role | teams                                            | called |
      | Priya Raman   | priya.raman@canzan.dev   | admin          | Backend as Member                                |        |
      | Jeff Bailey   | jeffabailey@gmail.com    | admin          | General as Lead, Ops as Lead, Onboarding as Lead | Jeff   |
      | Lena Park     | lena.park@canzan.dev     | member         | Backend as Lead, Ops as Member                   |        |
    And Globex is another workspace, whose team Payments is led by Hugo Weber (hugo.weber@globex.example)

  @us-tm-07 @driving_port @kpi @contract-shape:bounded-change
  Scenario: A newcomer's first Keycloak sign-in lands them on the default team
    Given Priya has set General as the default team for new members
    When Tomás Rivera signs in with Keycloak for the first time as tomas.rivera@canzan.dev
    Then Tomás Rivera has a new account in Canzan Labs and is a Member of General, and nothing else changed
    And Tomás Rivera sees Sandbox on his dashboard and opens its board

  @us-tm-07 @edge @contract-shape:unbounded-preservation
  Scenario: Signing in with Keycloak to an existing account adds no team
    Given Priya has set Ops as the default team for new members
    When Priya signs in with Keycloak using her own address
    Then Priya is signed in
    And no one's membership changed

  @us-tm-07 @error @contract-shape:bounded-change
  Scenario Outline: An unset or missing default never stops a first Keycloak sign-in
    Given <default>
    When Tomás Rivera signs in with Keycloak for the first time as tomas.rivera@canzan.dev
    Then Tomás Rivera has a new account in Canzan Labs and is on no team, and nothing else changed
    And Tomás Rivera is signed in

    Examples:
      | default                                                                  |
      | Canzan Labs has no default team                                          |
      | the Onboarding team, once Canzan Labs's default, has since been deleted by hand |
