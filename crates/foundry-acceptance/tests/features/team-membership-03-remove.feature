# Feature: team-membership, slice 03 (US-TM-04, OQ-9) — a workspace admin takes
# someone off a team without leaving it leaderless; removing someone from the
# workspace also takes them off its teams, and is refused for a team's last lead.
#
# DISTILL 2026-10-09: every scenario @pending (ADR-025 scaffolded RED). The
# workspace removal is the shipped scripted door POST /workspace/members/remove (no
# UI form): its refusal is a 409 in its own plain-text family (DDD-8), and it sends
# no removal notice. Board access is read per request, so a removed member's very
# next board visit is the page that never existed (no session refresh).
#
# Harness and oracles as in team-membership-01: see that file's header.

@team-membership @tm-03 @real-io
Feature: A workspace admin takes someone off a team without leaving it leaderless

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

  @us-tm-04 @driving_port @kpi @contract-shape:bounded-change @pending
  Scenario: A workspace admin takes a member off a team, and the member loses its board at once
    Given Priya has added Tomás Rivera to Backend
    And Tomás Rivera can see Identity Platform on his dashboard and open its board
    When Priya removes Tomás Rivera from Backend
    Then she sees "Removed Tomás Rivera (tomas.rivera@canzan.dev) from Backend."
    And Tomás Rivera is no longer on Backend, and nothing else changed
    And Tomás Rivera's next visit to the Identity Platform board gets the page that never existed
    And Tomás Rivera's dashboard says "You're not on any team yet — ask a workspace admin to add you."

  @us-tm-04 @contract-shape:bounded-change @pending
  Scenario: Taking one of two leads off a team leaves the other in charge
    Given Priya has made Sam Okafor a lead of Backend
    When Priya removes Lena Park from Backend
    Then she sees "Removed Lena Park (lena.park@canzan.dev) from Backend."
    And Lena Park is no longer on Backend, and nothing else changed
    And Backend's members page lists "Sam Okafor · sam.okafor@canzan.dev · Lead"

  @us-tm-04 @error @kpi @contract-shape:unbounded-preservation @pending
  Scenario: The last lead cannot be taken off the team
    Given Lena Park is Backend's only lead
    When Priya removes Lena Park from Backend
    Then she is told "Backend needs at least one lead. Make someone else a lead first."
    And no one's membership changed
    And Backend's members page lists "Lena Park · lena.park@canzan.dev · Lead"

  @us-tm-04 @edge @contract-shape:unbounded-preservation @pending
  Scenario: Removing someone who has already left the team, from an older page, changes nothing
    Given Priya opened Backend's members page earlier
    And Priya has removed Sam Okafor from Backend
    When Priya removes Sam Okafor from Backend from the page she opened earlier
    Then she gets Backend's members page back, with nothing to report
    And no one's membership changed

  @us-tm-04 @error @kpi @concurrency @contract-shape:bounded-change @pending
  Scenario: A removal and a demotion at the same moment leave the team with exactly one lead
    Given Lena Park and Sam Okafor are Backend's only leads
    When Priya removes Lena Park and makes Sam Okafor a member at the same moment, 5 times over
    Then each time, exactly one went through and the other was told "Backend needs at least one lead. Make someone else a lead first."
    And each time, Backend was left with exactly one lead

  @us-tm-04 @error @kpi @contract-shape:unbounded-preservation @pending
  Scenario Outline: Someone without the right cannot take anyone off a team
    When <who> submits removing Sam Okafor from Backend
    Then the answer is the page that never existed
    And no one's membership changed
    And Priya still finds Backend's members page

    Examples:
      | who                  |
      | a signed-out visitor |
      | Marco                |
      | Sam                  |
      | Hugo                 |

  @us-tm-04 @error @kpi @contract-shape:unbounded-preservation @pending
  Scenario Outline: A removal naming someone outside the workspace, or no one, is refused
    When Priya submits removing <target> from Backend
    Then the answer is the page that never existed
    And no one's membership changed
    And Priya still finds Backend's members page

    Examples:
      | target                    |
      | Hugo Weber                |
      | someone who never existed |
      | a garbled name            |

  @us-tm-04 @driving_port @contract-shape:bounded-change @pending
  Scenario: Removing someone from the workspace also takes them off its teams
    When Priya removes Sam Okafor from Canzan Labs
    Then Sam Okafor is no longer in Canzan Labs or on any of its teams, and nothing else changed
    And Backend's members page no longer lists Sam Okafor

  @us-tm-04 @edge @contract-shape:bounded-change @pending
  Scenario: A lead who shares the lead with someone else can be removed from the workspace
    Given Priya has made Sam Okafor a lead of Backend
    When Priya removes Lena Park from Canzan Labs
    Then Lena Park is no longer in Canzan Labs or on any of its teams, and nothing else changed

  @us-tm-04 @error @kpi @contract-shape:unbounded-preservation @pending
  Scenario: The last lead of a team cannot be removed from the workspace
    Given Lena Park is Backend's only lead
    When Priya removes Lena Park from Canzan Labs
    Then she is told "Lena Park (lena.park@canzan.dev) is the last lead of Backend. Make someone else a lead first."
    And no one's membership changed
    And Lena Park is sent no notice that she was removed
