# Feature: project-lifecycle, slice 04 (US-PL-05, OQ-1 (b)) — a team whose
# projects are all deleted can itself be deleted; its deleted projects are kept,
# detached from the gone team, and cannot be restored; only live projects keep a
# team from being deleted (D14, INV-PL-6, INV-TL-1 as amended, ADR-PROJECT-
# LIFECYCLE-004, amends team-lifecycle OUT-25).
#
# Authored @pending in DISTILL 2026-10-13 (ADR-025 scaffolded RED).
#
# The team delete's shipped pages and steps (team-lifecycle-02) are reused: the
# confirm page, its refusal while a LIVE project exists (a 200 page on GET, a
# 409 on POST), "Deleted {team}." on the teams page. New here: the D14 line
# "{team} also has {k} deleted project(s): {list}. They are kept, but can't be
# restored after {team} is deleted, and their keys stay taken in {workspace}.",
# and the operator's refusal to restore a detached project, pinned as
# `foundry doctor restore-project: project {uuid}: its team {team_slug} was
# deleted — status: not restorable` (D14 gives "… its team {team_slug} was
# deleted — status: not restorable"; DISTILL joins it to the uuid with ": ").
# A detached project stays on the operator's list, its `team` line naming the
# deleted team's address (DISTILL decision; D14 did not cover it).
#
# The 100-run restore-versus-team-delete guarantee is the store's
# (crates/foundry-store/tests/team_delete.rs); the race here runs 5 times,
# the operator's restore through the real binary against Priya's delete
# through the real router.
#
# Base-table reads (user decision U2, 2026-10-13, house precedent OQ-D6): the
# universe, "nothing erased", the stored deleted marker and the restore race's
# row-kept check are read with read-only fixture SQL on the base tables;
# whether a project is hidden is asserted only through the product's doors.
#
# Harness, universe and oracles as in project-lifecycle-01 (see its header).

@project-lifecycle @pl-04 @real-io
Feature: A team whose projects are all deleted can be deleted, and its deleted projects are kept

  Background:
    Given Canzan Labs is the instance's original workspace, with these teams:
      | team    | project | key |
      | Backend |         |     |
      | Ops     |         |     |
      | Labs    |         |     |
    And these people belong to Canzan Labs:
      | person        | email                    | workspace role | teams                       | called |
      | Priya Raman   | priya.raman@canzan.dev   | admin          | Backend as Member           |        |
      | Lena Park     | lena.park@canzan.dev     | member         | Backend as Lead             |        |
      | Tomás Rivera  | tomas.rivera@canzan.dev  | member         | Ops as Lead, Labs as Member |        |
    And these projects are live:
      | team    | project             | key   | address             |
      | Backend | Identity Platform   | AUTH  | auth                |
      | Ops     | Homelab Ops         | OPS   | homelab-ops         |
      | Ops     | Reading List        | READ  | reading-list        |
      | Labs    | Prototype Graveyard | PROTO | prototype-graveyard |

  @us-pl-05 @driving_port @pending @contract-shape:unbounded-preservation
  Scenario: The team delete page names the deleted project it will keep
    Given Prototype Graveyard has 12 issues
    And Priya has deleted the project Prototype Graveyard
    When Priya follows Delete team on Labs's members page
    Then the delete page asks "Delete Labs?"
    And the delete page says "Labs has 1 member(s) and no projects. They lose access to Labs. This can't be undone."
    And the delete page says "Labs also has 1 deleted project(s): Prototype Graveyard (PROTO). They are kept, but can't be restored after Labs is deleted, and their keys stay taken in Canzan Labs."
    And the delete page offers Delete team and Cancel
    And the delete page works as a plain page form
    And no project, issue or record changed

  @us-pl-05 @driving_port @kpi @pending @contract-shape:bounded-change
  Scenario: A team whose only project is deleted can be deleted, and the project's work is kept
    Given Prototype Graveyard has 12 issues
    And Priya has deleted the project Prototype Graveyard
    And Priya opened Labs's delete page earlier
    When Priya confirms the team delete from the page she opened earlier
    Then she sees "Deleted Labs."
    And Labs is gone with everyone's place on it, one delete record names Priya Raman, Prototype Graveyard is kept apart from any team with its 12 issues unchanged, and nothing else changed

  @us-pl-05 @error @kpi @pending @contract-shape:unbounded-preservation
  Scenario: Only live projects keep a team from being deleted, and only they are named
    Given Priya has deleted the project Reading List
    When Priya follows Delete team on Ops's members page
    Then the delete page says "Ops still has 1 project(s): Homelab Ops (OPS). A team with projects can't be deleted."
    And the delete page offers no Delete team
    And no project, issue or record changed

  @us-pl-05 @error @kpi @pending @contract-shape:unbounded-preservation
  Scenario: A delete sent for a team that still has a live project is refused, naming only the live one
    Given Priya has deleted the project Reading List
    When Priya submits a delete of Ops
    Then she is told "Ops still has 1 project(s): Homelab Ops (OPS). A team with projects can't be deleted."
    And no project, issue or record changed

  @us-pl-05 @error @pending @contract-shape:unbounded-preservation
  Scenario: A deleted project whose team is gone cannot be restored, and is not lost
    Given Prototype Graveyard has 12 issues
    And Priya has deleted the project Prototype Graveyard
    And Priya has deleted Labs
    When the operator restores {Prototype Graveyard's id}
    Then the command exits 4, saying "foundry doctor restore-project: project {Prototype Graveyard's id}: its team labs was deleted — status: not restorable"
    And no project, issue or record changed

  @us-pl-05 @edge @pending @contract-shape:unbounded-preservation
  Scenario: A project kept from a deleted team is still on the operator's list
    Given Prototype Graveyard has 12 issues
    And Priya has deleted the project Prototype Graveyard
    And Priya has deleted Labs
    When the operator lists the deleted projects
    Then the operator reads exactly these blocks, and the command succeeded:
      | project-id | {Prototype Graveyard's id} |
      | workspace  | Canzan Labs                |
      | team       | labs                       |
      | name       | Prototype Graveyard        |
      | key        | PROTO                      |
      | issues     | 12                         |
      | deleted-at | {when it was deleted}      |
      | deleted-by | priya.raman@canzan.dev     |
    And no project, issue or record changed

  @us-pl-05 @edge @pending @contract-shape:unbounded-preservation
  Scenario: A deleted project's key stays taken after its team is deleted
    Given Priya has deleted the project Prototype Graveyard
    And Priya has deleted Labs
    When Tomás follows New project on Ops's members page and creates "Prototype Two" with key PROTO
    Then he is told "Project key is already in use in this workspace"
    And no team, project or membership changed

  @us-pl-05 @error @kpi @concurrency @pending @contract-shape:bounded-change
  Scenario: A restore racing a team delete never loses a project
    Given 5 fresh teams each hold one project with one issue, which Priya has deleted
    When the operator restores each of those projects at the same moment as Priya deletes its team
    Then each time, either the project came back and the team delete was refused naming it, or the team was deleted and the restore was refused
    And no project and no issue was ever removed
