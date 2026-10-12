# Feature: team-lifecycle, slice 02 (US-TL-02, US-TL-03) — a workspace admin
# deletes a team through a confirm page that says what goes with it, and a team
# that has a project is never deleted, even when the project appears while the
# confirm page is open (D8, D9, D10, OQ-4 (a), INV-TL-1 to INV-TL-3).
#
# Authored @pending in DISTILL 2026-10-11 (ADR-025 scaffolded RED); un-pended in
# DELIVER 02-02, 02-03 and 02-04. The race is repeated 5 times over HTTP
# (DESIGN Test Seams); the 100-run proof of the same guarantee, and the two negative controls that show why the lock and the
# check must be separate statements, are the store's
# (crates/foundry-store/tests/team_delete.rs).
#
# Statuses (DDD-18): the confirm page and the "still has projects" page are 200
# pages; a delete sent while the team has a project is a 409 with the same
# words ("is told"); a delete that goes through is the 200 teams page with
# "Deleted {team}."; every refusal to someone without the right is the uniform
# 404; a tokenless POST is the CSRF layer's 403.
#
# Harness and oracles as in team-lifecycle-01: see that file's header.

@team-lifecycle @tl-02 @real-io
Feature: A workspace admin deletes a team, told first what goes, and never loses a project

  Background:
    Given Canzan Labs is the instance's original workspace, with these teams:
      | team    | project           | key  |
      | General | Sandbox           | SBX  |
      | Backend | Identity Platform | AUTH |
      | Ops     |                   |      |
      | Bakcend |                   |      |
      | Design  |                   |      |
    And these people belong to Canzan Labs:
      | person        | email                    | workspace role | teams                              | called |
      | Priya Raman   | priya.raman@canzan.dev   | admin          | Backend as Member                  |        |
      | Jeff Bailey   | jeffabailey@gmail.com    | admin          | General as Lead, Ops as Lead       | Jeff   |
      | Lena Park     | lena.park@canzan.dev     | member         | Backend as Lead, Design as Lead    |        |
      | Sam Okafor    | sam.okafor@canzan.dev    | member         | Bakcend as Member                  |        |
      | Tomás Rivera  | tomas.rivera@canzan.dev  | member         | Bakcend as Member, Ops as Member   |        |
      | Marco Bianchi | marco.bianchi@canzan.dev | member         |                                    |        |
      | Ines Duarte   | ines.duarte@canzan.dev   | member         |                                    |        |
    And Globex is another workspace, whose team Payments is led by Hugo Weber (hugo.weber@globex.example)
    And Ines Duarte is an instance admin of this foundry, but not a workspace admin

  # ------------------------------------------------------------------- US-TL-02

  @us-tl-02 @driving_port @contract-shape:unbounded-preservation
  Scenario: The delete page says exactly what goes with the team, and changes nothing
    When Priya follows Delete team on Bakcend's members page
    Then the delete page asks "Delete Bakcend?"
    And the delete page says "Bakcend has 2 member(s) and no projects. They lose access to Bakcend. This can't be undone."
    And the delete page says nothing about the default team or machine tokens
    And the delete page offers Delete team and Cancel
    And no team, project or membership changed

  @us-tl-02 @kpi @contract-shape:bounded-change
  Scenario: A workspace admin deletes a team that has no projects
    Given Priya opened Bakcend's delete page earlier
    When she confirms the delete from the page she opened earlier
    Then she sees "Deleted Bakcend."
    And Bakcend is gone with everyone's place on it, one delete record names Priya Raman, and nothing else changed
    And Priya's teams page does not list "Bakcend"

  @us-tl-02 @edge @contract-shape:unbounded-preservation
  Scenario: A member whose only team was deleted lands on the no-team page, and one with another team keeps it
    Given Priya has deleted Bakcend
    Then Sam Okafor's dashboard says "You're not on any team yet — ask a workspace admin to add you."
    And Tomás's teams page still lists Ops

  @us-tl-02 @kpi @contract-shape:bounded-change
  Scenario: Deleting the default team warns first and leaves no default
    Given Priya has set Ops as the default team for new members
    When Priya follows Delete team on Ops's members page
    Then the delete page says "Ops is the default team for new members. After it is deleted, new members will join no team."
    When she confirms the delete
    Then she sees "Deleted Ops."
    And Ops is gone with everyone's place on it, Canzan Labs has no default team, one delete record names Priya Raman, and nothing else changed
    And the teams page shows No team as the default team for new members

  @us-tl-02 @contract-shape:bounded-change
  Scenario: A team's machine tokens are named on the delete page and go with the team
    Given Design has 1 machine token scoped to it
    When Priya follows Delete team on Design's members page
    Then the delete page says "1 machine token(s) scoped to Design will stop working."
    When she confirms the delete
    Then she sees "Deleted Design."
    And Design is gone with everyone's place on it and its 1 machine token, one delete record names Priya Raman, and nothing else changed

  @us-tl-02 @edge @contract-shape:unbounded-preservation
  Scenario: Cancel goes back to the team, and nothing changes
    When Priya follows Delete team on Bakcend's members page
    And she follows Cancel
    Then she is back on Bakcend's members page
    And no team, project or membership changed

  @us-tl-02 @edge @contract-shape:bounded-change
  Scenario: After a team is deleted its name and address can be used again, and only the new team's people reach it
    Given Priya has deleted Bakcend
    When Priya creates the team "Bakcend" from the teams page
    Then she sees "Created Bakcend. You're its lead."
    And Canzan Labs now has the team "Bakcend" at the address "bakcend", led by Priya Raman, and nothing else changed
    When Sam asks for the members page of Bakcend
    Then the answer is the page that never existed

  @us-tl-02 @error @kpi @contract-shape:unbounded-preservation
  Scenario Outline: Only a workspace admin may open a team's delete page
    When <who> asks for the delete page of <team>
    Then the answer is the page that never existed
    And no team, project or membership changed
    And Priya still finds <team>'s delete page

    Examples:
      | who                  | team    |
      | a signed-out visitor | Bakcend |
      | Marco                | Bakcend |
      | Sam                  | Bakcend |
      | Lena                 | Design  |
      | Ines                 | Bakcend |
      | Hugo                 | Bakcend |

  @us-tl-02 @error @kpi @contract-shape:unbounded-preservation
  Scenario Outline: Only a workspace admin may delete a team
    When <who> submits a delete of <team>
    Then the answer is the page that never existed
    And no team, project or membership changed
    And Priya still finds <team>'s delete page

    Examples:
      | who                  | team    |
      | a signed-out visitor | Bakcend |
      | Marco                | Bakcend |
      | Sam                  | Bakcend |
      | Lena                 | Design  |
      | Ines                 | Bakcend |
      | Hugo                 | Bakcend |

  @us-tl-02 @error @kpi @contract-shape:unbounded-preservation
  Scenario: Another workspace's team and a team that does not exist look the same from Canzan Labs
    When Priya asks for the delete page of Payments
    Then the answer is the page that never existed
    And Hugo still finds Payments's delete page
    When Priya submits a delete of Payments
    Then the answer is the page that never existed
    And no team, project or membership changed
    When Priya asks for the delete page of a team that does not exist
    Then the answer is the page that never existed
    When Priya submits a delete of a team that does not exist
    Then the answer is the page that never existed

  @us-tl-02 @error @contract-shape:unbounded-preservation
  Scenario: A delete sent without the page's token is turned away, and nothing changes
    When Priya submits a delete of Bakcend without the page's token
    Then the answer is turned away as a forged form
    And no team, project or membership changed
    And the same delete with the page's token deletes Bakcend

  @us-tl-02 @edge @contract-shape:unbounded-preservation
  Scenario: A confirm sent again for a team already deleted finds nothing, and changes nothing
    Given Priya opened Bakcend's delete page earlier
    And Priya has deleted Bakcend
    When she confirms the delete from the page she opened earlier
    Then the answer is the page that never existed
    And no team, project or membership changed

  @us-tl-02 @error @contract-shape:unbounded-preservation
  Scenario: An add sent from a members page opened before the team was deleted finds nothing
    Given Priya opened Bakcend's members page earlier
    And Priya has deleted Bakcend
    When Priya adds Marco Bianchi to Bakcend from the page she opened earlier
    Then the answer is the page that never existed
    And no team, project or membership changed

  # ------------------------------------------------------------------- US-TL-03

  @us-tl-03 @driving_port @error @kpi @contract-shape:unbounded-preservation
  Scenario: A team with a project shows why it cannot be deleted
    When Priya follows Delete team on Backend's members page
    Then the delete page says "Backend still has 1 project(s): Identity Platform (AUTH). A team with projects can't be deleted."
    And the delete page offers no Delete team
    And no team, project or membership changed

  @us-tl-03 @edge @contract-shape:unbounded-preservation
  Scenario: Every project of the team is named, by name and key
    Given Ops has these projects:
      | project      | key  |
      | Reading List | READ |
      | Homelab Ops  | OPS  |
    When Priya follows Delete team on Ops's members page
    Then the delete page says "Ops still has 2 project(s): Homelab Ops (OPS), Reading List (READ). A team with projects can't be deleted."
    And the delete page offers no Delete team

  @us-tl-03 @error @kpi @contract-shape:unbounded-preservation
  Scenario: A delete sent for a team that has a project is refused, and nothing is lost
    When Priya submits a delete of Backend
    Then she is told "Backend still has 1 project(s): Identity Platform (AUTH). A team with projects can't be deleted."
    And no team, project or membership changed
    And the Identity Platform board still opens for Lena

  @us-tl-03 @error @kpi @contract-shape:unbounded-preservation
  Scenario: A confirm sent from a page opened before a project appeared is refused
    Given Priya opened Design's delete page earlier
    And Lena has since created the project "Brand Refresh" with key BRAND on Design
    When Priya confirms the delete from the page she opened earlier
    Then she is told "Design still has 1 project(s): Brand Refresh (BRAND). A team with projects can't be deleted."
    And no team, project or membership changed
    And the Brand Refresh board still opens for Lena

  @us-tl-03 @error @kpi @concurrency @contract-shape:bounded-change
  Scenario: A delete racing a project create never destroys the project
    When Priya deletes Design at the same moment as Lena creates "Brand Refresh" with key BRAND on it, 5 times over
    Then each time, either the project was kept and the delete was refused, or the team was deleted and the create found no team
    And no answer was a server error
    And no project was ever lost
