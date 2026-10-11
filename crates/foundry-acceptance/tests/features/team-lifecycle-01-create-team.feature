# Feature: team-lifecycle, slice 01 (US-TL-01) — the walking skeleton.
# A workspace admin creates a team, becomes its lead, and starts a project on it
# from the members page. This ends the dead end of every workspace provisioned
# from the instance's workspaces page: no team, so no project and no board.
#
# DISTILL 2026-10-11: every scenario @pending (ADR-025 scaffolded RED), the
# walking skeleton included, because none of the routes exists yet (the house
# precedent: team-membership's skeleton was @pending and un-pended by its first
# DELIVER step). The RED classification of each scenario is in
# docs/feature/team-lifecycle/distill/red-classification.md.
#
# Harness and oracles as in team-membership-01 (see that file's header): the
# in-process router with real sessions and the real double-submit CSRF, real
# Postgres per scenario. A write "from the page" submits the page's own form; a
# write the caller has no form for is the same request built by hand. Every When
# captures every team (name and address), team place, workspace place, account,
# default team, project, machine token and lifecycle record first; "nothing
# else changed" and "no team, project or membership changed" compare against it.
#
# Refusals are judged against a page that never existed, byte for byte (D5), and
# each refusal scenario also proves the thing it hid exists, so an unrouted path
# can never pass for a refusal. Name refusals are 422 with the input kept;
# writes that go through are 200 pages (DDD-18, DDD-22).

@team-lifecycle @tl-01 @real-io
Feature: A workspace admin creates a team, leads it, and starts a project on it

  Background:
    Given Canzan Labs is the instance's original workspace, with these teams:
      | team    | project           | key  |
      | General | Sandbox           | SBX  |
      | Backend | Identity Platform | AUTH |
      | Design  |                   |      |
    And these people belong to Canzan Labs:
      | person        | email                    | workspace role | teams                              | called |
      | Priya Raman   | priya.raman@canzan.dev   | admin          | Backend as Member                  |        |
      | Jeff Bailey   | jeffabailey@gmail.com    | admin          | General as Lead                    | Jeff   |
      | Lena Park     | lena.park@canzan.dev     | member         | Backend as Lead, Design as Lead    |        |
      | Sam Okafor    | sam.okafor@canzan.dev    | member         | Backend as Member                  |        |
      | Tomás Rivera  | tomas.rivera@canzan.dev  | member         | Design as Member                   |        |
      | Marco Bianchi | marco.bianchi@canzan.dev | member         |                                    |        |
      | Ines Duarte   | ines.duarte@canzan.dev   | member         |                                    |        |
    And Globex is another workspace, whose team Payments is led by Hugo Weber (hugo.weber@globex.example)
    And Ines Duarte is an instance admin of this foundry, but not a workspace admin

  # ------------------------------------------------------------- walking skeleton

  @walking_skeleton @driving_port @us-tl-01 @kpi @contract-shape:bounded-change @pending
  Scenario: Priya starts Homelab's first team, leads it, and opens its first board
    Given Homelab is a newly provisioned workspace with no teams, whose workspace admin is Priya Raman
    And Priya is working in Homelab
    When she follows Teams from her dashboard
    Then she sees "This workspace has no teams yet. Create one to start a project."
    When she creates the team "Ops" from the teams page
    Then she sees "Created Ops. You're its lead."
    And Homelab now has the team "Ops" at the address "ops", led by Priya Raman, and nothing else changed
    And Ops's members page lists "Priya Raman · priya.raman@canzan.dev · Lead"
    When she follows New project on Ops's members page and creates "Homelab Ops" with key OPS
    Then the Homelab Ops board opens, and her dashboard lists "OPS Homelab Ops"

  # ------------------------------------------------------------------- US-TL-01

  @us-tl-01 @driving_port @kpi @contract-shape:bounded-change @pending
  Scenario: A workspace admin adds a team next to the others and leads it, and only its people see it
    When Priya creates the team "Platform" from the teams page
    Then she sees "Created Platform. You're its lead."
    And Canzan Labs now has the team "Platform" at the address "platform", led by Priya Raman, and nothing else changed
    And Priya's teams page lists "Platform · 1 member"
    And Lena's teams page does not list "Platform"

  @us-tl-01 @edge @kpi @contract-shape:unbounded-preservation @pending
  Scenario: A new team's lead is sent from the dashboard to that team's project form, not to one she cannot use
    Given Homelab is a newly provisioned workspace with no teams, whose workspace admin is Priya Raman
    And Priya is working in Homelab
    And Priya has created the team "Ops"
    When she follows "create your first project" from her dashboard
    Then she gets Ops's new project form
    And no team, project or membership changed

  @us-tl-01 @edge @contract-shape:unbounded-preservation @pending
  Scenario: A member of a team with no projects is sent to that team's project form
    When Tomás follows "create your first project" from his dashboard
    Then he gets Design's new project form
    And no team, project or membership changed

  @us-tl-01 @edge @contract-shape:unbounded-preservation @pending
  Scenario Outline: Everyone on a team is offered New project, and an admin who is not on it is not
    When <who> opens <team>'s members page
    Then the members page <offers> New project

    Examples:
      | who   | team    | offers          |
      | Sam   | Backend | offers          |
      | Lena  | Backend | offers          |
      | Priya | Backend | offers          |
      | Jeff  | Backend | does not offer  |

  @us-tl-01 @error @contract-shape:unbounded-preservation @pending
  Scenario Outline: A name already used in the workspace is refused, and the text is kept
    When Priya creates the team "<typed>" from the teams page
    Then she is refused with "Team name must be unique within the workspace", and her text "<typed>" is kept
    And no team, project or membership changed

    Examples:
      | typed   |
      | backend |
      | GENERAL |

  @us-tl-01 @error @contract-shape:unbounded-preservation @pending
  Scenario: An empty name is refused
    When Priya creates the team "[SPACE][SPACE][SPACE]" from the teams page
    Then she is refused with "Team name must not be empty", and her text "[SPACE][SPACE][SPACE]" is kept
    And no team, project or membership changed

  @us-tl-01 @edge @kpi @contract-shape:unbounded-preservation @pending
  Scenario: Sending the create form a second time makes no second team
    Given Priya has created the team "Platform"
    When she sends the create form for "Platform" a second time
    Then she is refused with "Team name must be unique within the workspace", and her text "Platform" is kept
    And no team, project or membership changed

  @us-tl-01 @error @kpi @contract-shape:unbounded-preservation @pending
  Scenario Outline: Only a workspace admin can create a team
    When <who> submits a team create named "Platform"
    Then the answer is the page that never existed
    And no team, project or membership changed
    And Priya's teams page still offers creating a team

    Examples:
      | who                  |
      | a signed-out visitor |
      | Marco                |
      | Sam                  |
      | Lena                 |
      | Ines                 |
      | Hugo                 |

  @us-tl-01 @edge @contract-shape:unbounded-preservation @pending
  Scenario: An instance admin who is not a workspace admin is not offered the form either
    When Ines opens the teams page
    Then the teams page offers no way to create a team
    And Priya's teams page still offers creating a team

  @us-tl-01 @error @contract-shape:unbounded-preservation @pending
  Scenario: A create sent without the page's token is turned away, and nothing changes
    When Priya submits a team create named "Platform" without the page's token
    Then the answer is turned away as a forged form
    And no team, project or membership changed
    And the same create with the page's token creates "Platform"
