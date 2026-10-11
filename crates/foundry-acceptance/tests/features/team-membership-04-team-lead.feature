# Feature: team-membership, slice 04 (US-TM-05, OQ-2, OQ-D8) — a team lead manages
# their own team, and only their own team. Leads are peers: a lead may change or
# remove any lead or member of her team, herself and a workspace admin included
# (DDD-23), unless that leaves the team without a lead (INV-4). Authority is read
# per request (INV-6): a page opened as a lead proves nothing after a demotion.
#
# Authored @pending in DISTILL 2026-10-09 (ADR-025 scaffolded RED); un-pended in
# DELIVER 04-01.
#
# Harness and oracles as in team-membership-01: see that file's header.

@team-membership @tm-04 @real-io
Feature: A team lead manages her own team, and only her own team

  Background:
    Given Canzan Labs is the instance's original workspace, with these teams:
      | team    | project           | key  |
      | General | Sandbox           | SBX  |
      | Backend | Identity Platform | AUTH |
      | Ops     | Runbooks          | OPS  |
    And these people belong to Canzan Labs:
      | person        | email                    | workspace role | teams                          | called |
      | Priya Raman   | priya.raman@canzan.dev   | admin          | Backend as Member              |        |
      | Jeff Bailey   | jeffabailey@gmail.com    | admin          | General as Lead, Ops as Lead   | Jeff   |
      | Lena Park     | lena.park@canzan.dev     | member         | Backend as Lead, Ops as Member |        |
      | Sam Okafor    | sam.okafor@canzan.dev    | member         | Backend as Member              |        |
      | Tomás Rivera  | tomas.rivera@canzan.dev  | member         |                                |        |
      | Marco Bianchi | marco.bianchi@canzan.dev | member         | Ops as Member                  |        |
    And Globex is another workspace, whose team Payments is led by Hugo Weber (hugo.weber@globex.example)

  @us-tm-05 @driving_port @kpi @contract-shape:bounded-change
  Scenario: A team lead puts a workspace member on her own team
    When Lena opens Backend's members page
    Then the page offers adding, changing roles and removing
    When Lena adds Tomás Rivera to Backend
    Then she sees "Added Tomás Rivera (tomas.rivera@canzan.dev) to Backend as a member."
    And Tomás Rivera is now a Member of Backend, and nothing else changed

  @us-tm-05 @contract-shape:bounded-change
  Scenario: A team lead hands leadership over and steps down
    Given Lena has made Sam Okafor a lead of Backend
    When Lena makes herself a member of Backend
    Then she sees "Lena Park (lena.park@canzan.dev) is now a member of Backend."
    And Lena Park is now a Member of Backend, and nothing else changed
    And the page offers no way to add, change a role or remove anyone

  @us-tm-05 @contract-shape:bounded-change
  Scenario: A team lead takes a teammate off her team
    When Lena removes Sam Okafor from Backend
    Then she sees "Removed Sam Okafor (sam.okafor@canzan.dev) from Backend."
    And Sam Okafor is no longer on Backend, and nothing else changed

  @us-tm-05 @error @kpi @contract-shape:unbounded-preservation
  Scenario Outline: A team's only lead cannot step down or leave it
    Given Lena Park is Backend's only lead
    When Lena <action> Backend
    Then she is told "Backend needs at least one lead. Make someone else a lead first."
    And no one's membership changed

    Examples:
      | action                    |
      | makes herself a member of |
      | removes herself from      |

  @us-tm-05 @edge @contract-shape:bounded-change
  Scenario: A team lead can take a workspace admin off her team
    When Lena removes Priya Raman from Backend
    Then she sees "Removed Priya Raman (priya.raman@canzan.dev) from Backend."
    And Priya Raman is no longer on Backend, and nothing else changed

  @us-tm-05 @edge @contract-shape:bounded-change
  Scenario: A team lead can make a workspace admin on her team a member again
    Given Lena has made Priya Raman a lead of Backend
    When Lena makes Priya Raman a member of Backend
    Then she sees "Priya Raman (priya.raman@canzan.dev) is now a member of Backend."
    And Priya Raman is now a Member of Backend, and nothing else changed

  @us-tm-05 @error @kpi @contract-shape:unbounded-preservation
  Scenario Outline: A team lead cannot change a team she does not lead
    When Lena submits <request>
    Then the answer is the page that never existed
    And no one's membership changed
    And Priya still finds <team>'s members page

    Examples:
      | request                            | team    |
      | an add of Tomás Rivera to Ops      | Ops     |
      | making Marco Bianchi a lead of Ops | Ops     |
      | removing Marco Bianchi from Ops    | Ops     |
      | an add of Tomás Rivera to General  | General |
      | making Jeff a member of General    | General |
      | removing Jeff from General         | General |

  @us-tm-05 @edge @contract-shape:unbounded-preservation
  Scenario: A team lead sees a team she is only a member of without any way to change it
    When Lena opens Ops's members page
    Then the page offers no way to add, change a role or remove anyone

  @us-tm-05 @error @contract-shape:unbounded-preservation
  Scenario: A lead who has since been made a member cannot act from a page she opened as a lead
    Given Lena opened Backend's members page earlier
    And Priya has made Sam Okafor a lead of Backend
    And Priya has made Lena Park a member of Backend
    When Lena adds Tomás Rivera to Backend from the page she opened earlier
    Then the answer is the page that never existed
    And no one's membership changed
