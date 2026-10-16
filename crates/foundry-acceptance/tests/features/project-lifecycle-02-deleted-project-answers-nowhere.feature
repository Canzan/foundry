# Feature: project-lifecycle, slice 02 (US-PL-03) — a deleted project answers
# nowhere, for anyone, on any door: every web and API address under it answers
# exactly as the same address of a project that never existed, every list drops
# it, a page opened before the delete cannot write into it, and a write racing
# the delete gets a normal answer, never a server error (D5, D9, D16, D17,
# INV-PL-2, INV-PL-3, Changed Assumptions 1 and 5).
#
# Authored @pending in DISTILL 2026-10-13 (ADR-025 scaffolded RED).
#
# THE DIFFERENTIAL OUTLINE (PL-KPI-3, DESIGN "the INV-PL-2 outline is
# differential, not expect 404"). For every door of the route inventory (DISCUSS
# [REF] Lifecycle inventory plus attachment upload and download, Changed
# Assumption 5, plus this feature's three new doors), every verb and every
# actor, the deleted project's answer is compared byte for byte (status,
# content type, body) with the same door, verb and actor on a slug no project
# ever had. Two more checks keep each row honest:
#   - "neither request changed anything" — the whole universe (teams, people,
#     tokens, every project, its holdings and records) is unchanged across the
#     two compared requests;
#   - the same door on Identity Platform, which is live, answers OTHERWISE, so a
#     row cannot pass because its request was malformed or its route unwired.
#     The live request runs last, after both compared requests.
# Every door names real things: SPIKE-2 (and AUTH-1 for the live control), the
# actor's own comment on it, its attachment, and the lane "Todo".
#
# The answer column pins OQ-D1 (re-verified at DISTILL): every web door's
# never-existed answer is the uniform 404 except the live updates, whose
# shipped refusal for an unknown project is a 403 text/plain "Not a member of
# team …" (events.rs; Changed Assumption 1). A deleted project gets exactly
# that refusal there. The script (API) doors answer the API's own not-found, except the
# two machine-token routes (`…/tokens`, `…/tokens/{jti}`), which never read the
# project: under any project address, live, deleted or never-existed, they
# answer the same (re-verified at DISTILL; a separate outline pins it).
#
# The 100-run delete-versus-issue-create guarantee is the store's
# (crates/foundry-store/tests/project_lifecycle.rs); the race here runs 5 times
# over HTTP (DESIGN Test Seams).
#
# Base-table reads (user decision U2, 2026-10-13, house precedent OQ-D6): the
# universe, "nothing erased" and the stored deleted marker are read with
# read-only fixture SQL on the base tables; whether a project is hidden is
# asserted only through the product's doors (pages, the API, the operator CLI).
# The two machine-token routes are exempt from INV-PL-2's not-found answer by
# user decision U1 (2026-10-13): they read no project and answer alike under
# every project address (DESIGN Changed Assumption 6). The universe ignores
# `machine_tokens.revoked_at` (it keys tokens by id, workspace and scope), so the
# revoke scenario asserts the revocation through the product's token list, not
# through the universe.
#
# Harness, universe and oracles as in project-lifecycle-01 (see its header).

@project-lifecycle @pl-02 @real-io
Feature: A deleted project answers nowhere, for anyone, on any door

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
      | Marco Bianchi | marco.bianchi@canzan.dev | member         |                   |        |
      | Ines Duarte   | ines.duarte@canzan.dev   | member         |                   |        |
    And Globex is another workspace, whose team Payments is led by Hugo Brandt (hugo.brandt@globex.example)
    And Ines Duarte is an instance admin of this foundry, but not a workspace admin
    And these projects are live:
      | team    | project           | key   | address       |
      | Backend | Identity Platform | AUTH  | auth          |
      | Backend | Auth v2 Spike     | SPIKE | auth-v2-spike |
      | Ops     | Homelab Ops       | OPS   | homelab-ops   |

  # ------------------------------------------------------------ every door, every actor

  @us-pl-03 @kpi @contract-shape:unbounded-preservation
  Scenario Outline: Every web address of a deleted project answers as one that never existed, for everyone
    Given Auth v2 Spike has 3 issues, 2 comments and 1 attachment
    And Identity Platform has 1 issue, 1 comment and 1 attachment
    And <who> has commented on SPIKE-2 and on AUTH-1
    And Lena has deleted the project Auth v2 Spike
    When <who> tries <door> in Auth v2 Spike, and the same in a project that never existed
    Then both answers are exactly the same, and they are <answer>
    And neither request changed anything
    And the same door in Identity Platform, which is live, answers otherwise

    Examples: Lena, Backend's lead
      | who   | door                             | answer                            |
      | Lena  | the board                        | the uniform not-found page        |
      | Lena  | the report                       | the uniform not-found page        |
      | Lena  | the report as a spreadsheet      | the uniform not-found page        |
      | Lena  | filing an issue                  | the uniform not-found page        |
      | Lena  | the new-issue form               | the uniform not-found page        |
      | Lena  | searching the issues             | the uniform not-found page        |
      | Lena  | moving an issue to another state | the uniform not-found page        |
      | Lena  | an issue's page                  | the uniform not-found page        |
      | Lena  | an issue's edit form             | the uniform not-found page        |
      | Lena  | editing an issue                 | the uniform not-found page        |
      | Lena  | an issue's delete page           | the uniform not-found page        |
      | Lena  | deleting an issue                | the uniform not-found page        |
      | Lena  | commenting on an issue           | the uniform not-found page        |
      | Lena  | a comment                        | the uniform not-found page        |
      | Lena  | a comment's edit form            | the uniform not-found page        |
      | Lena  | editing a comment                | the uniform not-found page        |
      | Lena  | deleting a comment               | the uniform not-found page        |
      | Lena  | attaching a file to an issue     | the uniform not-found page        |
      | Lena  | downloading an attachment        | the uniform not-found page        |
      | Lena  | the live updates                 | the uniform not-found page        |
      | Lena  | a lane's delete dialog           | the uniform not-found page        |
      | Lena  | deleting a lane                  | the uniform not-found page        |
      | Lena  | a lane's edit dialog             | the uniform not-found page        |
      | Lena  | renaming a lane                  | the uniform not-found page        |
      | Lena  | a lane's insert dialog           | the uniform not-found page        |
      | Lena  | inserting a lane                 | the uniform not-found page        |
      | Lena  | moving a lane                    | the uniform not-found page        |
      | Lena  | renaming the project             | the uniform not-found page        |
      | Lena  | the project's delete page        | the uniform not-found page        |
      | Lena  | deleting the project             | the uniform not-found page        |

    Examples: Sam, a member of Backend
      | who   | door                             | answer                            |
      | Sam   | the board                        | the uniform not-found page        |
      | Sam   | the report                       | the uniform not-found page        |
      | Sam   | the report as a spreadsheet      | the uniform not-found page        |
      | Sam   | filing an issue                  | the uniform not-found page        |
      | Sam   | the new-issue form               | the uniform not-found page        |
      | Sam   | searching the issues             | the uniform not-found page        |
      | Sam   | moving an issue to another state | the uniform not-found page        |
      | Sam   | an issue's page                  | the uniform not-found page        |
      | Sam   | an issue's edit form             | the uniform not-found page        |
      | Sam   | editing an issue                 | the uniform not-found page        |
      | Sam   | an issue's delete page           | the uniform not-found page        |
      | Sam   | deleting an issue                | the uniform not-found page        |
      | Sam   | commenting on an issue           | the uniform not-found page        |
      | Sam   | a comment                        | the uniform not-found page        |
      | Sam   | a comment's edit form            | the uniform not-found page        |
      | Sam   | editing a comment                | the uniform not-found page        |
      | Sam   | deleting a comment               | the uniform not-found page        |
      | Sam   | attaching a file to an issue     | the uniform not-found page        |
      | Sam   | downloading an attachment        | the uniform not-found page        |
      | Sam   | the live updates                 | the uniform not-found page        |
      | Sam   | a lane's delete dialog           | the uniform not-found page        |
      | Sam   | deleting a lane                  | the uniform not-found page        |
      | Sam   | a lane's edit dialog             | the uniform not-found page        |
      | Sam   | renaming a lane                  | the uniform not-found page        |
      | Sam   | a lane's insert dialog           | the uniform not-found page        |
      | Sam   | inserting a lane                 | the uniform not-found page        |
      | Sam   | moving a lane                    | the uniform not-found page        |

    Examples: Priya, a workspace admin on Backend
      | who   | door                             | answer                            |
      | Priya | the board                        | the uniform not-found page        |
      | Priya | the report                       | the uniform not-found page        |
      | Priya | the report as a spreadsheet      | the uniform not-found page        |
      | Priya | filing an issue                  | the uniform not-found page        |
      | Priya | the new-issue form               | the uniform not-found page        |
      | Priya | searching the issues             | the uniform not-found page        |
      | Priya | moving an issue to another state | the uniform not-found page        |
      | Priya | an issue's page                  | the uniform not-found page        |
      | Priya | an issue's edit form             | the uniform not-found page        |
      | Priya | editing an issue                 | the uniform not-found page        |
      | Priya | an issue's delete page           | the uniform not-found page        |
      | Priya | deleting an issue                | the uniform not-found page        |
      | Priya | commenting on an issue           | the uniform not-found page        |
      | Priya | a comment                        | the uniform not-found page        |
      | Priya | a comment's edit form            | the uniform not-found page        |
      | Priya | editing a comment                | the uniform not-found page        |
      | Priya | deleting a comment               | the uniform not-found page        |
      | Priya | attaching a file to an issue     | the uniform not-found page        |
      | Priya | downloading an attachment        | the uniform not-found page        |
      | Priya | the live updates                 | the uniform not-found page        |
      | Priya | a lane's delete dialog           | the uniform not-found page        |
      | Priya | deleting a lane                  | the uniform not-found page        |
      | Priya | a lane's edit dialog             | the uniform not-found page        |
      | Priya | renaming a lane                  | the uniform not-found page        |
      | Priya | a lane's insert dialog           | the uniform not-found page        |
      | Priya | inserting a lane                 | the uniform not-found page        |
      | Priya | moving a lane                    | the uniform not-found page        |
      | Priya | renaming the project             | the uniform not-found page        |
      | Priya | the project's delete page        | the uniform not-found page        |
      | Priya | deleting the project             | the uniform not-found page        |

  @us-pl-03 @kpi @contract-shape:unbounded-preservation
  Scenario Outline: For someone who cannot reach Backend, a deleted project's address answers as one that never existed
    Given Auth v2 Spike has 3 issues, 2 comments and 1 attachment
    And Identity Platform has 1 issue, 1 comment and 1 attachment
    And Lena has commented on SPIKE-2 and on AUTH-1
    And Lena has deleted the project Auth v2 Spike
    When <who> tries <door> in Auth v2 Spike, and the same in a project that never existed
    Then both answers are exactly the same, as for any project out of reach
    And neither request changed anything
    And the same door in Identity Platform, which is live, answers them the same way

    Examples: a signed-out visitor
      | who                  | door                             |
      | a signed-out visitor | the board                        |
      | a signed-out visitor | the report                       |
      | a signed-out visitor | the report as a spreadsheet      |
      | a signed-out visitor | filing an issue                  |
      | a signed-out visitor | the new-issue form               |
      | a signed-out visitor | searching the issues             |
      | a signed-out visitor | moving an issue to another state |
      | a signed-out visitor | an issue's page                  |
      | a signed-out visitor | an issue's edit form             |
      | a signed-out visitor | editing an issue                 |
      | a signed-out visitor | an issue's delete page           |
      | a signed-out visitor | deleting an issue                |
      | a signed-out visitor | commenting on an issue           |
      | a signed-out visitor | a comment                        |
      | a signed-out visitor | a comment's edit form            |
      | a signed-out visitor | editing a comment                |
      | a signed-out visitor | deleting a comment               |
      | a signed-out visitor | attaching a file to an issue     |
      | a signed-out visitor | downloading an attachment        |
      | a signed-out visitor | the live updates                 |
      | a signed-out visitor | a lane's delete dialog           |
      | a signed-out visitor | deleting a lane                  |
      | a signed-out visitor | a lane's edit dialog             |
      | a signed-out visitor | renaming a lane                  |
      | a signed-out visitor | a lane's insert dialog           |
      | a signed-out visitor | inserting a lane                 |
      | a signed-out visitor | moving a lane                    |
      | a signed-out visitor | renaming the project             |
      | a signed-out visitor | the project's delete page        |
      | a signed-out visitor | deleting the project             |

    Examples: Marco, on no team
      | who                  | door                             |
      | Marco                | the board                        |
      | Marco                | the report                       |
      | Marco                | the report as a spreadsheet      |
      | Marco                | filing an issue                  |
      | Marco                | the new-issue form               |
      | Marco                | searching the issues             |
      | Marco                | moving an issue to another state |
      | Marco                | an issue's page                  |
      | Marco                | an issue's edit form             |
      | Marco                | editing an issue                 |
      | Marco                | an issue's delete page           |
      | Marco                | deleting an issue                |
      | Marco                | commenting on an issue           |
      | Marco                | a comment                        |
      | Marco                | a comment's edit form            |
      | Marco                | editing a comment                |
      | Marco                | deleting a comment               |
      | Marco                | attaching a file to an issue     |
      | Marco                | downloading an attachment        |
      | Marco                | the live updates                 |
      | Marco                | a lane's delete dialog           |
      | Marco                | deleting a lane                  |
      | Marco                | a lane's edit dialog             |
      | Marco                | renaming a lane                  |
      | Marco                | a lane's insert dialog           |
      | Marco                | inserting a lane                 |
      | Marco                | moving a lane                    |
      | Marco                | renaming the project             |
      | Marco                | the project's delete page        |
      | Marco                | deleting the project             |

    Examples: Ines, an instance admin who is not a workspace admin
      | who                  | door                             |
      | Ines                 | the board                        |
      | Ines                 | the report                       |
      | Ines                 | the report as a spreadsheet      |
      | Ines                 | filing an issue                  |
      | Ines                 | the new-issue form               |
      | Ines                 | searching the issues             |
      | Ines                 | moving an issue to another state |
      | Ines                 | an issue's page                  |
      | Ines                 | an issue's edit form             |
      | Ines                 | editing an issue                 |
      | Ines                 | an issue's delete page           |
      | Ines                 | deleting an issue                |
      | Ines                 | commenting on an issue           |
      | Ines                 | a comment                        |
      | Ines                 | a comment's edit form            |
      | Ines                 | editing a comment                |
      | Ines                 | deleting a comment               |
      | Ines                 | attaching a file to an issue     |
      | Ines                 | downloading an attachment        |
      | Ines                 | the live updates                 |
      | Ines                 | a lane's delete dialog           |
      | Ines                 | deleting a lane                  |
      | Ines                 | a lane's edit dialog             |
      | Ines                 | renaming a lane                  |
      | Ines                 | a lane's insert dialog           |
      | Ines                 | inserting a lane                 |
      | Ines                 | moving a lane                    |
      | Ines                 | renaming the project             |
      | Ines                 | the project's delete page        |
      | Ines                 | deleting the project             |

    Examples: Hugo, Globex's admin
      | who                  | door                             |
      | Hugo                 | the board                        |
      | Hugo                 | the report                       |
      | Hugo                 | the report as a spreadsheet      |
      | Hugo                 | filing an issue                  |
      | Hugo                 | the new-issue form               |
      | Hugo                 | searching the issues             |
      | Hugo                 | moving an issue to another state |
      | Hugo                 | an issue's page                  |
      | Hugo                 | an issue's edit form             |
      | Hugo                 | editing an issue                 |
      | Hugo                 | an issue's delete page           |
      | Hugo                 | deleting an issue                |
      | Hugo                 | commenting on an issue           |
      | Hugo                 | a comment                        |
      | Hugo                 | a comment's edit form            |
      | Hugo                 | editing a comment                |
      | Hugo                 | deleting a comment               |
      | Hugo                 | attaching a file to an issue     |
      | Hugo                 | downloading an attachment        |
      | Hugo                 | the live updates                 |
      | Hugo                 | a lane's delete dialog           |
      | Hugo                 | deleting a lane                  |
      | Hugo                 | a lane's edit dialog             |
      | Hugo                 | renaming a lane                  |
      | Hugo                 | a lane's insert dialog           |
      | Hugo                 | inserting a lane                 |
      | Hugo                 | moving a lane                    |
      | Hugo                 | renaming the project             |
      | Hugo                 | the project's delete page        |
      | Hugo                 | deleting the project             |

  @us-pl-03 @kpi @contract-shape:unbounded-preservation
  Scenario Outline: A deleted project cannot be reached by a script either, and nothing is written
    Given Auth v2 Spike has 3 issues, 2 comments and 1 attachment
    And Identity Platform has 1 issue, 1 comment and 1 attachment
    And Priya has commented on SPIKE-2 and on AUTH-1
    And a script holds a machine token of Priya's for Backend, and Priya has a spare machine token
    And Lena has deleted the project Auth v2 Spike
    When the script tries <door> in Auth v2 Spike, and the same in a project that never existed
    Then both answers are exactly the same, and they are the scripts' not-found answer
    And neither request changed anything
    And the same door in Identity Platform, which is live, answers otherwise

    Examples:
      | door                             |
      | listing the issues               |
      | filing an issue                  |
      | moving an issue to another state |
      | reading an issue's history       |
      | commenting on an issue           |
      | editing a comment                |

  @us-pl-03 @edge @contract-shape:unbounded-preservation
  Scenario: Listing machine tokens under a deleted project's address answers as under any address, because the list names no project
    Given Auth v2 Spike has 3 issues, 2 comments and 1 attachment
    And Identity Platform has 1 issue, 1 comment and 1 attachment
    And Priya has commented on SPIKE-2 and on AUTH-1
    And a script holds a machine token of Priya's for Backend, and Priya has a spare machine token
    And Lena has deleted the project Auth v2 Spike
    When the script tries listing the machine tokens in Auth v2 Spike, and the same in a project that never existed
    Then both answers are the machine-token answer, alike for both
    And neither request changed anything
    And the same door in Identity Platform, which is live, answers the same way

  @us-pl-03 @edge @contract-shape:bounded-change
  Scenario: Revoking a machine token under a deleted project's address revokes it, as under any address, because the token names no project
    Given a script holds a machine token of Priya's for Backend, and Priya has a spare machine token
    And Lena has deleted the project Auth v2 Spike
    When the script revokes Priya's spare machine token under Auth v2 Spike's address, and again under a project that never existed
    Then the first answer revoked the spare machine token, and nothing else changed
    And the second answer is the same as the first, and changed nothing

  @us-pl-03 @error @kpi @contract-shape:unbounded-preservation
  Scenario: A deleted project's real attachment cannot be downloaded by anyone
    Given Auth v2 Spike has 3 issues, 2 comments and 1 attachment
    And Identity Platform has 1 issue, 1 comment and 1 attachment
    And Lena has deleted the project Auth v2 Spike
    When Lena, Sam and Priya each download Auth v2 Spike's attachment
    Then each answer is the page that never existed
    And no project, issue or record changed
    And Lena can still download Identity Platform's attachment

  # ------------------------------------------------------------ lists

  @us-pl-03 @kpi @contract-shape:unbounded-preservation
  Scenario: A deleted project disappears from every list, and the others stay
    Given Lena has deleted the project Auth v2 Spike
    When Sam looks at his dashboard
    Then Sam's dashboard lists "Identity Platform" and not "Auth v2 Spike"
    And Sam's Board link opens Identity Platform
    And Backend's Projects section, as Lena sees it, lists only "Identity Platform · AUTH"
    And Ines's instance dashboard lists "Identity Platform" and "Homelab Ops" and not "Auth v2 Spike"
    And no project, issue or record changed

  @us-pl-03 @error @contract-shape:unbounded-preservation
  Scenario: The instance dashboard's rename does not find a deleted project
    Given Ines opened the instance dashboard earlier
    And Lena has deleted the project Auth v2 Spike
    When Ines renames the project Auth v2 Spike to "Spike Revived" from the dashboard she opened earlier
    Then the answer is the page that never existed
    And no project, issue or record changed
    And Ines can still rename the project Homelab Ops to "Homelab Operations" from the instance dashboard

  # ------------------------------------------------------------ stale pages, open views, races

  @us-pl-03 @error @contract-shape:unbounded-preservation
  Scenario: A change sent from a page opened before the delete is refused, and nothing changes
    Given Auth v2 Spike has 3 issues, 2 comments and 1 attachment
    And Identity Platform has 1 issue
    And Sam opened the edit form of SPIKE-2 earlier
    And Lena has deleted the project Auth v2 Spike
    When Sam saves the title "Revived" from the edit form he opened earlier
    Then the answer is the page that never existed
    And no project, issue or record changed
    And Sam can still save the title "Revived" on AUTH-1

  @us-pl-03 @edge @contract-shape:unbounded-preservation
  Scenario Outline: An open live view of a deleted project hears of no change made after the delete
    Given Auth v2 Spike has 3 issues, 2 comments and 1 attachment
    And Sam is watching Auth v2 Spike's live updates, and has heard of an issue Lena filed
    And Lena has deleted the project Auth v2 Spike
    When Lena tries to <write>
    Then the answer is the page that never existed
    And no project, issue or record changed
    And Sam's live view hears of no further change

    Examples:
      | write                         |
      | file an issue on Auth v2 Spike |
      | comment on SPIKE-2            |

  @us-pl-03 @error @kpi @concurrency @contract-shape:bounded-change
  Scenario: Filing an issue during a delete gets a normal answer, never a failure
    Given Backend has 5 fresh projects, Race 1 to Race 5
    When Lena deletes each fresh project at the same moment as Sam files an issue on it
    Then each time, either Sam's issue was kept and is hidden with the project, or Sam was answered as for a project that never existed
    And no answer in the race was a failure
    And no filed issue was lost
