# Feature: team-membership, slice 01 (US-TM-01, US-TM-02) — the walking skeleton.
# A workspace admin finds a team, sees who is on it, and puts a workspace member on
# it; that member can then open the team's boards. This closes the incident
# recorded by fix-hide-unreachable-boards: an SSO-provisioned account on no team.
#
# DISTILL 2026-10-09 (ADR-025 scaffolded RED): every scenario is @pending, the
# walking skeleton included, because none of the routes exist yet (the house
# precedent: fix-hide-unreachable-boards' scenario was @pending in step 01-01 and
# un-pended by it). DELIVER step 01-01 un-pends the skeleton first. The RED
# classification of every scenario is in docs/feature/team-membership/distill/
# red-classification.md.
#
# Harness: the in-process router (real session, real double-submit CSRF), real
# Postgres (per-scenario schema), and the in-process Keycloak double over a real
# socket. Writes a person makes "from the page" submit the page's own form; writes
# a caller has no form for are the same request built by hand. Every When captures
# every team place, workspace place, account, team and default team first; "and
# nothing else changed" / "no one's membership changed" compare against it.
#
# Refusals are judged against a page that never existed, byte for byte (D5). Each
# refusal scenario also proves the team it hid exists ("Priya still finds …"), so a
# missing route can never pass for a refusal.

@team-membership @tm-01 @real-io
Feature: A workspace admin sees who is on a team and puts a workspace member on it

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

  # ------------------------------------------------------------- walking skeleton

  @walking_skeleton @driving_port @us-tm-02 @kpi @contract-shape:bounded-change
  Scenario: Jeff puts his own Keycloak account on General, and it opens the Sandbox board
    Given Jeff's Keycloak account "jeff@unintelligent-design.us" has signed in for the first time
    And Jeff's Keycloak account is on no team and cannot open the Sandbox board
    When Jeff follows Teams from his dashboard
    And he opens General from the teams page
    And he adds "Jeff Bailey — jeff@unintelligent-design.us" to General
    Then he sees "Added Jeff Bailey (jeff@unintelligent-design.us) to General as a member."
    And Jeff's Keycloak account is now a Member of General, and nothing else changed
    And General's members page lists "Jeff Bailey · jeff@unintelligent-design.us · Member"
    And Jeff's Keycloak account, still signed in, sees Sandbox on its dashboard and opens its board

  # ------------------------------------------------------------------- US-TM-01

  @us-tm-01 @driving_port @contract-shape:unbounded-preservation
  Scenario: A workspace admin finds a team and sees who is on it, leads first
    When Priya follows Teams from her dashboard
    Then the teams page lists exactly:
      | General · 1 member  |
      | Backend · 3 members |
      | Ops · 3 members     |
    When she opens Backend from the teams page
    Then Backend's members page lists, in this order:
      | Lena Park · lena.park@canzan.dev · Lead       |
      | Priya Raman · priya.raman@canzan.dev · Member |
      | Sam Okafor · sam.okafor@canzan.dev · Member   |
    And no one's membership changed

  @us-tm-01 @edge @contract-shape:unbounded-preservation
  Scenario: A workspace admin sees the members of a team she is not on, but still cannot open its board
    When Priya opens Ops's members page
    Then Ops's members page lists, in this order:
      | Jeff Bailey · jeffabailey@gmail.com · Lead        |
      | Lena Park · lena.park@canzan.dev · Member         |
      | Marco Bianchi · marco.bianchi@canzan.dev · Member |
    And the page offers adding, changing roles and removing
    And Priya's visit to the Runbooks board gets the page that never existed

  @us-tm-01 @edge @contract-shape:unbounded-preservation
  Scenario: A team member sees their own team, without any way to change it
    When Sam opens the teams page
    Then the teams page lists exactly:
      | Backend · 3 members |
    When he opens Backend from the teams page
    Then Backend's members page lists "Sam Okafor · sam.okafor@canzan.dev · Member"
    And the page offers no way to add, change a role or remove anyone

  @us-tm-01 @edge @contract-shape:unbounded-preservation
  Scenario: Someone on no team is told so on the teams page
    When Tomás opens the teams page
    Then he sees "You're not on any team yet — ask a workspace admin to add you."
    And the teams page lists no team

  @us-tm-01 @error @kpi @contract-shape:unbounded-preservation
  Scenario Outline: Someone who may not see a team's members gets the page that never existed
    When <who> asks for the members page of <team>
    Then the answer is the page that never existed
    And no one's membership changed
    And Priya still finds <team>'s members page

    Examples:
      | who                  | team    |
      | a signed-out visitor | Backend |
      | Marco                | Backend |
      | Lena                 | General |
      | Hugo                 | Backend |

  @us-tm-01 @error @kpi @contract-shape:unbounded-preservation
  Scenario: Another workspace's team and a team that does not exist look the same from Canzan Labs
    When Priya asks for the members page of Payments
    Then the answer is the page that never existed
    And Hugo still finds Payments's members page
    When Priya asks for the members page of a team that does not exist
    Then the answer is the page that never existed

  @us-tm-01 @error @contract-shape:unbounded-preservation
  Scenario: A signed-out visitor learns nothing from the teams page
    When a signed-out visitor opens the teams page
    Then the answer is the page that never existed
    And Priya's teams page still lists Backend

  # ------------------------------------------------------------------- US-TM-02

  @us-tm-02 @kpi @contract-shape:bounded-change
  Scenario: A workspace admin puts a member on a team, and the member can open the team's board
    When Priya adds Tomás Rivera to Backend
    Then she sees "Added Tomás Rivera (tomas.rivera@canzan.dev) to Backend as a member."
    And Tomás Rivera is now a Member of Backend, and nothing else changed
    And Tomás Rivera sees Identity Platform on his dashboard and opens its board

  @us-tm-02 @edge @contract-shape:unbounded-preservation @pending
  Scenario: Only workspace members who are not on the team are offered, each by name and address
    Given Jeff's Keycloak account "jeff@unintelligent-design.us" has signed in for the first time
    When Jeff opens General's members page
    Then Add a member on General offers exactly:
      | Jeff Bailey — jeff@unintelligent-design.us |
      | Lena Park — lena.park@canzan.dev           |
      | Marco Bianchi — marco.bianchi@canzan.dev   |
      | Priya Raman — priya.raman@canzan.dev       |
      | Sam Okafor — sam.okafor@canzan.dev         |
      | Tomás Rivera — tomas.rivera@canzan.dev     |

  @us-tm-02 @edge @contract-shape:unbounded-preservation @pending
  Scenario: When everyone is already on the team, there is nobody to add
    Given everyone in Canzan Labs is on Ops
    When Priya opens Ops's members page
    Then Add a member is replaced by "Everyone in this workspace is already on Ops."

  @us-tm-02 @edge @contract-shape:unbounded-preservation @pending
  Scenario: Adding someone already on the team, from an older page, changes nothing and says so
    Given Priya opened Backend's members page earlier
    And Priya has added Tomás Rivera to Backend
    When Priya adds Tomás Rivera to Backend from the page she opened earlier
    Then she sees "Tomás Rivera (tomas.rivera@canzan.dev) is already on Backend."
    And no one's membership changed
    And Backend's members page lists "Tomás Rivera · tomas.rivera@canzan.dev · Member" once

  @us-tm-02 @error @kpi @contract-shape:unbounded-preservation @pending
  Scenario Outline: Someone who is not a Canzan Labs member cannot be added
    When Priya submits an add of <target> to Backend
    Then the answer is the page that never existed
    And no one's membership changed
    And Priya still finds Backend's members page

    Examples:
      | target                    |
      | Hugo Weber                |
      | someone who never existed |
      | a garbled name            |

  @us-tm-02 @error @kpi @contract-shape:unbounded-preservation @pending
  Scenario Outline: Someone without the right cannot add anyone to a team
    When <who> submits an add of Tomás Rivera to <team>
    Then the answer is the page that never existed
    And no one's membership changed
    And Priya still finds <team>'s members page

    Examples:
      | who                  | team    |
      | a signed-out visitor | General |
      | Marco                | General |
      | Sam                  | Backend |
      | Lena                 | General |
      | Hugo                 | Backend |

  @us-tm-02 @error @kpi @contract-shape:unbounded-preservation @pending
  Scenario: An add aimed at another workspace's team or at a team that does not exist is refused the same way
    When Priya submits an add of Tomás Rivera to Payments
    Then the answer is the page that never existed
    And no one's membership changed
    And Hugo still finds Payments's members page
    When Priya submits an add of Tomás Rivera to a team that does not exist
    Then the answer is the page that never existed
    And no one's membership changed

  @us-tm-02 @error @contract-shape:unbounded-preservation @pending
  Scenario: Someone who left the workspace cannot be added from a page opened before they left
    Given Priya opened Backend's members page earlier
    And Tomás Rivera has since left Canzan Labs
    When Priya adds Tomás Rivera to Backend from the page she opened earlier
    Then the answer is the page that never existed
    And no one's membership changed

  @us-tm-02 @error @contract-shape:unbounded-preservation @pending
  Scenario: An add sent without the page's token is turned away, and nothing changes
    When Priya submits an add of Tomás Rivera to Backend without the page's token
    Then the answer is turned away as a forged form
    And no one's membership changed
    And the same add with the page's token puts Tomás Rivera on Backend
