# Feature: project-lifecycle, slice 02 (US-PL-02) — a team's lead or a
# workspace admin deletes a project through a confirm page that says what
# happens; the delete is soft: the project's deleted marker and one record are
# written, and nothing under it is erased (D-A, D-B, D6, D7, OQ-3, OQ-5,
# INV-PL-1, INV-PL-7, INV-PL-8).
#
# Authored @pending in DISTILL 2026-10-13 (ADR-025 scaffolded RED). Slice 02
# ships with slice 03 (DISCUSS release rule; DEVOPS R1): a delete is never
# offered without the operator's restore.
#
# Statuses (DDD-24, DDD-27): the confirm page is a 200 full page whose h1 is
# "Delete {project}?", followed by the count line and the nothing-erased line,
# a form posting to /team/{t}/project/{p}/delete with a "Delete project" button,
# and a "Cancel" link back to /team/{t}/members; it asks for no typed name
# (OQ-5). A delete that goes through is the 200 members page with
# "Deleted {project}.". Every refusal is the uniform 404 on both verbs; a
# tokenless POST is the CSRF layer's 403.
#
# "Nothing erased" (PL-KPI-2, OQ-D6) is read from the store by fixture SQL on
# the base tables: per project, the count and a checksum of its issues,
# comments (live and awaiting cleanup), attachments, history, lanes and queued
# notifications, plus its next issue number. "Hidden" is only ever observed
# through the product's doors (feature file 02b).
#
# Base-table reads (user decision U2, 2026-10-13, house precedent OQ-D6): the
# universe, "nothing erased" and the stored deleted marker are read with
# read-only fixture SQL on the base tables; whether a project is hidden is
# asserted only through the product's doors (pages, the API, the operator CLI).
#
# Harness, universe and oracles as in project-lifecycle-01 (see its header).

@project-lifecycle @pl-02 @real-io
Feature: A team's lead or a workspace admin deletes a project, told first what happens, and nothing under it is erased

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
      | Sam Okafor    | sam.okafor@canzan.dev    | member         | Backend as Member           |        |
      | Tomás Rivera  | tomas.rivera@canzan.dev  | member         | Ops as Lead, Labs as Member |        |
      | Marco Bianchi | marco.bianchi@canzan.dev | member         |                             |        |
      | Ines Duarte   | ines.duarte@canzan.dev   | member         |                             |        |
    And Globex is another workspace, whose team Payments is led by Hugo Brandt (hugo.brandt@globex.example)
    And Ines Duarte is an instance admin of this foundry, but not a workspace admin
    And these projects are live:
      | team     | project           | key   | address       |
      | Backend  | Identity Platform | AUTH  | auth          |
      | Backend  | Auth v2 Spike     | SPIKE | auth-v2-spike |
      | Ops      | Homelab Ops       | OPS   | homelab-ops   |
      | Ops      | Reading List      | READ  | reading-list  |
      | Payments | Ledger            | LEDG  | ledger        |

  # ------------------------------------------------------------ deletes that go through

  @us-pl-02 @driving_port @contract-shape:unbounded-preservation
  Scenario: The delete page says what happens before anything does
    Given Auth v2 Spike has 3 issues, 2 comments and 1 attachment
    When Lena follows Delete project on Auth v2 Spike's row of Backend's members page
    Then the delete page asks "Delete Auth v2 Spike?"
    And the delete page says "Auth v2 Spike (SPIKE) has 3 issue(s). After it is deleted, nobody can open its board, its issues or its report, and links to SPIKE issues stop working."
    And the delete page says "Nothing is erased. The instance operator can restore Auth v2 Spike with every issue, comment and attachment."
    And the delete page offers Delete project and Cancel, and asks for no typed name
    And the delete page works as a plain page form
    And no project, issue or record changed

  @us-pl-02 @driving_port @kpi @pending @contract-shape:bounded-change
  Scenario: A team's lead deletes a project after reading what happens
    Given Auth v2 Spike has 3 issues, 2 comments and 1 attachment
    And Lena opened the delete page of the project Auth v2 Spike earlier
    When Lena confirms the project delete from the page she opened earlier
    Then she sees "Deleted Auth v2 Spike."
    And Backend's Projects section lists, in this order:
      | Identity Platform · AUTH |
    And Auth v2 Spike is deleted by Lena Park, one delete record names Lena Park, and nothing else changed

  @us-pl-02 @kpi @pending @contract-shape:bounded-change
  Scenario: Deleting a project erases none of its work
    Given Auth v2 Spike has 3 issues, 2 comments and 1 attachment
    And one more comment on SPIKE-2 was deleted and awaits cleanup
    When Lena deletes the project Auth v2 Spike from Backend's members page
    Then Auth v2 Spike still holds its 3 issues, 3 comments of which 1 awaits cleanup, 1 attachment, 4 lanes, their history and their queued notifications, unchanged
    And Auth v2 Spike's next issue is still SPIKE-4
    And Auth v2 Spike is deleted by Lena Park, one delete record names Lena Park, and nothing else changed

  @us-pl-02 @edge @contract-shape:unbounded-preservation
  Scenario: The delete page of a project with no issues says so
    When Priya follows Delete project on Reading List's row of Ops's members page
    Then the delete page asks "Delete Reading List?"
    And the delete page says "Reading List (READ) has 0 issue(s). After it is deleted, nobody can open its board, its issues or its report, and links to READ issues stop working."
    And no project, issue or record changed

  @us-pl-02 @pending @contract-shape:bounded-change
  Scenario: A workspace admin deletes a project with no issues on a team she is not on
    Given Priya opened the delete page of the project Reading List earlier
    When Priya confirms the project delete from the page she opened earlier
    Then she sees "Deleted Reading List."
    And Reading List is deleted by Priya Raman, one delete record names Priya Raman, and nothing else changed

  @us-pl-02 @edge @contract-shape:unbounded-preservation
  Scenario: Cancel goes back to the team, and nothing changes
    Given Lena opened the delete page of the project Auth v2 Spike earlier
    When Lena cancels the delete page she opened earlier
    Then she is back on Backend's members page
    And no project, issue or record changed
    And Backend's Projects section lists, in this order:
      | Auth v2 Spike · SPIKE    |
      | Identity Platform · AUTH |

  @us-pl-02 @edge @pending @contract-shape:unbounded-preservation
  Scenario: A delete confirmed a second time finds nothing, and deletes once
    Given Lena opened the delete page of the project Auth v2 Spike earlier
    And Lena has deleted the project Auth v2 Spike
    When Lena confirms the project delete from the page she opened earlier
    Then the answer is the page that never existed
    And no project, issue or record changed
    And Lena still finds the delete page of Identity Platform

  # ------------------------------------------------------------ refusals

  @us-pl-02 @error @kpi @contract-shape:unbounded-preservation
  Scenario Outline: Only the team's lead or a workspace admin may open a project's delete page
    When <who> asks for the delete page of the project <project>
    Then the answer is the page that never existed
    And no project, issue or record changed
    And Priya still finds the delete page of <project>

    Examples:
      | who                  | project           |
      | a signed-out visitor | Auth v2 Spike     |
      | Marco                | Auth v2 Spike     |
      | Sam                  | Auth v2 Spike     |
      | Lena                 | Reading List      |
      | Ines                 | Auth v2 Spike     |
      | Hugo                 | Auth v2 Spike     |

  @us-pl-02 @error @kpi @pending @contract-shape:unbounded-preservation
  Scenario Outline: Only the team's lead or a workspace admin may delete a project
    When <who> submits a delete of the project <project>
    Then the answer is the page that never existed
    And no project, issue or record changed
    And Priya still finds the delete page of <project>

    Examples:
      | who                  | project           |
      | a signed-out visitor | Auth v2 Spike     |
      | Marco                | Auth v2 Spike     |
      | Sam                  | Auth v2 Spike     |
      | Lena                 | Reading List      |
      | Ines                 | Auth v2 Spike     |
      | Hugo                 | Auth v2 Spike     |

  @us-pl-02 @error @kpi @pending @contract-shape:unbounded-preservation
  Scenario Outline: Another workspace's project and a project that does not exist look the same from Canzan Labs
    When Priya <request>
    Then the answer is the page that never existed
    And no project, issue or record changed
    And Hugo still finds the delete page of Ledger

    Examples:
      | request                                                     |
      | asks for the delete page of the project Ledger              |
      | submits a delete of the project Ledger                      |
      | asks for the delete page of a project that does not exist   |
      | submits a delete of a project that does not exist           |

  @us-pl-02 @error @pending @contract-shape:unbounded-preservation
  Scenario: A lead who has since been made a member cannot delete from a page she opened as a lead
    Given Lena opened the delete page of the project Auth v2 Spike earlier
    And Priya has made Sam a lead of Backend
    And Priya has made Lena a member of Backend
    When Lena confirms the project delete from the page she opened earlier
    Then the answer is the page that never existed
    And no project, issue or record changed
    And Sam still finds the delete page of Auth v2 Spike

  @us-pl-02 @error @pending @contract-shape:unbounded-preservation
  Scenario: A delete sent without the page's token is turned away, and nothing changes
    When Lena submits a delete of the project Auth v2 Spike without the page's token
    Then the answer is turned away as a forged form
    And no project, issue or record changed
    And the same project delete with the page's token deletes Auth v2 Spike
