# Feature: project-lifecycle, slice 03 (US-PL-04) — the instance operator lists
# the deleted projects and restores one exactly as it was; while a project is
# deleted its name, address and key stay taken, so a restore never collides
# (D10, D11, D14, OQ-4, OQ-6, INV-PL-4, INV-PL-5, Changed Assumption 3).
#
# Authored @pending in DISTILL 2026-10-13 (ADR-025 scaffolded RED). Released
# together with slice 02 (DEVOPS R1).
#
# The operator's door is the real, prebuilt `foundry` binary (assert_cmd's
# cargo_bin, so the lane's profile: target/release/foundry under --release),
# run with DATABASE_URL pointed at the scenario's schema. Rebuild and warm it
# before the lane (DEVOPS P2). Exit codes and copy (D14, pinned):
#   list-deleted-projects: one block per deleted project, oldest deletion first,
#     blank line between blocks, lines `project-id`, `workspace`, `team`,
#     `name`, `key`, `issues`, `deleted-at` (RFC 3339, UTC), `deleted-by`;
#     `status: none` when there is none; exit 0.
#   restore-project: stdout `project-id: {uuid}` and `status: restored`, exit 0;
#     exit 4 with a stderr line `foundry doctor restore-project: project {uuid}
#     is not deleted — status: not restorable` or `… {uuid} not in database —
#     status: not restorable`; exit 2 on a malformed id and exit 3 on an
#     unreachable database, worded as restore-comment's.
#   restore-comment on a deleted comment inside a deleted project (DESIGN left
#     the wording to DISTILL; pinned here as a DISTILL decision): exit 4,
#     stderr `foundry doctor restore-comment: comment {uuid} is in deleted
#     project {project uuid} — status: not restorable`.
# Placeholders in braces ({Auth v2 Spike's id}) are resolved by the driver.
#
# "Exactly as it was" (INV-PL-4, PL-KPI-2) is judged against the holdings
# noted just before the delete, by fixture SQL on the base tables: every row
# under the project, by count and checksum, and its name, address, key and
# next issue number. Changed Assumption 3's carve-out (a comment awaiting
# cleanup whose 90-day window passes while the project is deleted is purged)
# is proven at store level, where time can be moved; here every tombstone is
# inside its window.
#
# Base-table reads (user decision U2, 2026-10-13, house precedent OQ-D6): the
# universe, "nothing erased", the stored deleted marker (to check the printed
# deleted-at) and the restore race's row-kept check are read with read-only
# fixture SQL on the base tables; whether a project is hidden is asserted only
# through the product's doors (pages, the API, the operator CLI).
#
# Harness, universe and oracles as in project-lifecycle-01 (see its header).

@project-lifecycle @pl-03 @real-io
Feature: The instance operator restores a deleted project exactly as it was

  Background:
    Given Canzan Labs is the instance's original workspace, with these teams:
      | team    | project | key |
      | Backend |         |     |
      | Ops     |         |     |
    And these people belong to Canzan Labs:
      | person        | email                    | workspace role | teams             | called |
      | Priya Raman   | priya.raman@canzan.dev   | admin          | Backend as Member |        |
      | Lena Park     | lena.park@canzan.dev     | member         | Backend as Lead   |        |
      | Sam Okafor    | sam.okafor@canzan.dev    | member         | Backend as Member |        |
      | Tomás Rivera  | tomas.rivera@canzan.dev  | member         | Ops as Lead       |        |
    And these projects are live:
      | team    | project           | key   | address       |
      | Backend | Identity Platform | AUTH  | auth          |
      | Backend | Auth v2 Spike     | SPIKE | auth-v2-spike |
      | Ops     | Reading List      | READ  | reading-list  |

  @us-pl-04 @driving_port @kpi @pending @contract-shape:unbounded-preservation
  Scenario: The operator finds a deleted project on the list
    Given Auth v2 Spike has 3 issues, 2 comments and 1 attachment
    And Lena has deleted the project Auth v2 Spike
    When the operator lists the deleted projects
    Then the operator reads exactly these blocks, and the command succeeded:
      | project-id | {Auth v2 Spike's id}   |
      | workspace  | Canzan Labs            |
      | team       | backend                |
      | name       | Auth v2 Spike          |
      | key        | SPIKE                  |
      | issues     | 3                      |
      | deleted-at | {when it was deleted}  |
      | deleted-by | lena.park@canzan.dev   |
    And no project, issue or record changed

  @us-pl-04 @driving_port @kpi @pending @contract-shape:bounded-change
  Scenario: The operator restores a deleted project with everything under it
    Given Auth v2 Spike has 3 issues, 2 comments and 1 attachment
    And Sam has noted how every page of Auth v2 Spike answers him
    And Lena has deleted the project Auth v2 Spike
    When the operator restores {Auth v2 Spike's id}
    Then the operator reads "project-id: {Auth v2 Spike's id}" then "status: restored", and the command succeeded
    And Auth v2 Spike is restored, one restore record names the operator, and nothing else changed
    And Backend's Projects section, as Lena sees it, lists "Auth v2 Spike · SPIKE" again
    And SPIKE-2's page opens for Sam at its old address, with its 2 comments and its attachment
    And every page of Auth v2 Spike answers Sam as it did before the delete
    And the next issue Lena files on Auth v2 Spike is SPIKE-4

  @us-pl-04 @kpi @pending @contract-shape:bounded-change
  Scenario: A restored project holds exactly what it held when it was deleted
    Given Auth v2 Spike has 3 issues, 2 comments and 1 attachment
    And one more comment on SPIKE-2 was deleted and awaits cleanup
    And Lena has deleted the project Auth v2 Spike
    When the operator restores {Auth v2 Spike's id}
    Then Auth v2 Spike is restored, one restore record names the operator, and nothing else changed
    And Auth v2 Spike holds exactly what it held just before it was deleted, under the same name, address, key and next issue number

  @us-pl-04 @edge @pending @contract-shape:unbounded-preservation
  Scenario: The operator is told when there is nothing to restore
    When the operator lists the deleted projects
    Then the operator reads "status: none", and the command succeeded
    And no project, issue or record changed

  @us-pl-04 @edge @pending @contract-shape:unbounded-preservation
  Scenario: Every deleted project is listed, the oldest deletion first
    Given Auth v2 Spike has 3 issues, 2 comments and 1 attachment
    And Priya has deleted the project Reading List
    And Lena has deleted the project Auth v2 Spike
    When the operator lists the deleted projects
    Then the operator reads exactly these blocks, and the command succeeded:
      | project-id | {Reading List's id}    |
      | workspace  | Canzan Labs            |
      | team       | ops                    |
      | name       | Reading List           |
      | key        | READ                   |
      | issues     | 0                      |
      | deleted-at | {when it was deleted}  |
      | deleted-by | priya.raman@canzan.dev |
      |            |                        |
      | project-id | {Auth v2 Spike's id}   |
      | workspace  | Canzan Labs            |
      | team       | backend                |
      | name       | Auth v2 Spike          |
      | key        | SPIKE                  |
      | issues     | 3                      |
      | deleted-at | {when it was deleted}  |
      | deleted-by | lena.park@canzan.dev   |
    And no project, issue or record changed

  @us-pl-04 @error @pending @contract-shape:unbounded-preservation
  Scenario Outline: A restore that cannot be done is refused with its reason, and nothing changes
    When the operator restores <target>
    Then the command exits <code>, saying "<message>"
    And no project, issue or record changed

    Examples:
      | target                   | code | message                                                                                                      |
      | {Identity Platform's id} | 4    | foundry doctor restore-project: project {Identity Platform's id} is not deleted — status: not restorable     |
      | {an id no project has}   | 4    | foundry doctor restore-project: project {an id no project has} not in database — status: not restorable      |
      | not-a-uuid               | 2    | foundry doctor restore-project: invalid UUID "not-a-uuid"                                                    |

  @us-pl-04 @edge @pending @contract-shape:unbounded-preservation
  Scenario: Restoring the same project twice restores it once
    Given Lena has deleted the project Auth v2 Spike
    And the operator has restored {Auth v2 Spike's id}
    When the operator restores {Auth v2 Spike's id}
    Then the command exits 4, saying "foundry doctor restore-project: project {Auth v2 Spike's id} is not deleted — status: not restorable"
    And no project, issue or record changed

  @us-pl-04 @error @pending @contract-shape:unbounded-preservation
  Scenario: A restore attempted while foundry's records cannot be reached changes nothing
    Given Lena has deleted the project Auth v2 Spike
    When the operator restores {Auth v2 Spike's id} while foundry's records cannot be reached
    Then the command exits 3, saying "foundry doctor restore-project: could not connect to DATABASE_URL"
    And no project, issue or record changed

  @us-pl-04 @error @pending @contract-shape:unbounded-preservation
  Scenario: A deleted project's name stays taken for a new project until it is restored
    Given Lena has deleted the project Auth v2 Spike
    When Lena follows New project on Backend's members page and creates "auth v2 spike" with key SPOKE
    Then she is refused with "Project name must be unique within the team", and her text "auth v2 spike" is kept
    And no project, issue or record changed

  @us-pl-04 @error @pending @contract-shape:unbounded-preservation
  Scenario: A deleted project's key stays taken in the workspace until it is restored
    Given Lena has deleted the project Auth v2 Spike
    When Lena follows New project on Backend's members page and creates "Spike Two" with key SPIKE
    Then she is told "Project key is already in use in this workspace"
    And no project, issue or record changed

  @us-pl-04 @error @pending @contract-shape:unbounded-preservation
  Scenario: A deleted project's name stays taken for a rename until it is restored
    Given Lena has deleted the project Auth v2 Spike
    When Lena renames the project Identity Platform to "Auth v2 Spike" on Backend's members page
    Then she is refused with "Project name must be unique within the team" in Identity Platform's row, and her text "Auth v2 Spike" is kept
    And no project, issue or record changed

  @us-pl-04 @error @pending @contract-shape:unbounded-preservation
  Scenario: A deleted comment inside a deleted project cannot be brought back on its own
    Given Auth v2 Spike has 3 issues, 2 comments and 1 attachment
    And one more comment on SPIKE-2 was deleted and awaits cleanup
    And Lena has deleted the project Auth v2 Spike
    When the operator brings back that deleted comment
    Then the command exits 4, saying "foundry doctor restore-comment: comment {that comment's id} is in deleted project {Auth v2 Spike's id} — status: not restorable"
    And no project, issue or record changed
