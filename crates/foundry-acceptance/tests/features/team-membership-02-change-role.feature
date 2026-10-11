# Feature: team-membership, slice 02 (US-TM-03) — a workspace admin changes who leads
# a team, and a team can never lose its last lead (D6, INV-1 to INV-4).
#
# Authored @pending in DISTILL 2026-10-09 (ADR-025 scaffolded RED); un-pended in
# DELIVER 02-02. The race is repeated 5 times over HTTP (OQ-D3); the 100-run proof
# of the same guarantee is the store's (crates/foundry-store/tests/team_lead_guard.rs).
# A last-lead refusal is a stated reason (409, the members page re-rendered with the
# D15 copy), never a 404, because the actor may manage the team (DDD-8).
#
# Harness and oracles as in team-membership-01: see that file's header.

@team-membership @tm-02 @real-io
Feature: A workspace admin changes who leads a team, and a team never loses its last lead

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

  @us-tm-03 @driving_port @contract-shape:bounded-change
  Scenario: A workspace admin makes a member a lead
    When Priya makes Sam Okafor a lead of Backend
    Then she sees "Sam Okafor (sam.okafor@canzan.dev) is now a lead of Backend."
    And Sam Okafor is now a Lead of Backend, and nothing else changed
    And Backend's members page lists, in this order:
      | Lena Park · lena.park@canzan.dev · Lead       |
      | Sam Okafor · sam.okafor@canzan.dev · Lead     |
      | Priya Raman · priya.raman@canzan.dev · Member |

  @us-tm-03 @contract-shape:bounded-change
  Scenario: A workspace admin hands leadership over by making the previous lead a member
    Given Priya has made Sam Okafor a lead of Backend
    When Priya makes Lena Park a member of Backend
    Then she sees "Lena Park (lena.park@canzan.dev) is now a member of Backend."
    And Lena Park is now a Member of Backend, and nothing else changed

  @us-tm-03 @error @kpi @contract-shape:unbounded-preservation
  Scenario: The last lead cannot be made a member
    Given Lena Park is Backend's only lead
    When Priya makes Lena Park a member of Backend
    Then she is told "Backend needs at least one lead. Make someone else a lead first."
    And no one's membership changed
    And Backend's members page lists "Lena Park · lena.park@canzan.dev · Lead"

  @us-tm-03 @edge @contract-shape:unbounded-preservation
  Scenario: Making a lead a lead again changes nothing and says so
    When Priya makes Lena Park a lead of Backend
    Then she sees "Lena Park (lena.park@canzan.dev) is now a lead of Backend."
    And no one's membership changed

  @us-tm-03 @edge @contract-shape:unbounded-preservation
  Scenario: A role change for a workspace member who is not on the team changes nothing
    When Priya submits making Tomás Rivera a lead of Backend
    Then she gets Backend's members page back, with nothing to report
    And no one's membership changed

  @us-tm-03 @error @kpi @concurrency @contract-shape:bounded-change
  Scenario: Two leads made members at the same moment leave the team with exactly one lead
    Given Lena Park and Sam Okafor are Backend's only leads
    When Priya makes Lena Park and Sam Okafor members at the same moment, 5 times over
    Then each time, exactly one went through and the other was told "Backend needs at least one lead. Make someone else a lead first."
    And each time, Backend was left with exactly one lead

  @us-tm-03 @edge @contract-shape:bounded-change
  Scenario: A team that has no lead can be given one
    Given Canzan Labs also has a team Design, on which Sam Okafor and Tomás Rivera are both Members
    When Priya makes Tomás Rivera a lead of Design
    Then she sees "Tomás Rivera (tomas.rivera@canzan.dev) is now a lead of Design."
    And Tomás Rivera is now a Lead of Design, and nothing else changed

  @us-tm-03 @error @kpi @contract-shape:unbounded-preservation
  Scenario Outline: Someone without the right cannot change a role
    When <who> submits making Sam Okafor a lead of Backend
    Then the answer is the page that never existed
    And no one's membership changed
    And Priya still finds Backend's members page

    Examples:
      | who                  |
      | a signed-out visitor |
      | Marco                |
      | Sam                  |
      | Hugo                 |

  @us-tm-03 @error @kpi @contract-shape:unbounded-preservation
  Scenario Outline: A role change naming someone outside the workspace, or no one, is refused
    When Priya submits making <target> a lead of Backend
    Then the answer is the page that never existed
    And no one's membership changed
    And Priya still finds Backend's members page

    Examples:
      | target                    |
      | Hugo Weber                |
      | someone who never existed |
      | a garbled name            |

  @us-tm-03 @error @contract-shape:unbounded-preservation
  Scenario: A role that does not exist is refused, and nothing changes
    When Priya submits making Sam Okafor an owner of Backend
    Then the answer is the page that never existed
    And no one's membership changed
    And Priya still finds Backend's members page

  @us-tm-03 @error @contract-shape:unbounded-preservation
  Scenario: A role change aimed at another workspace's team is refused the same way
    When Priya submits making Hugo Weber a member of Payments
    Then the answer is the page that never existed
    And no one's membership changed
    And Hugo still finds Payments's members page
