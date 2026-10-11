# Feature: team-lifecycle, slice 04 (US-TL-05) — a workspace admin, or the team's
# own lead, renames a team; its address and every link to it stay (D4, D6, D15,
# OQ-1, OQ-6, INV-TL-5, INV-TL-6).
#
# DISTILL 2026-10-11: every scenario @pending (ADR-025 scaffolded RED). The
# rename's atomic record, its locked old name under concurrent renames, and
# rename versus delete are the store's (crates/foundry-store/tests/team_rename.rs).
#
# Statuses (DDD-18): a rename that goes through, and a byte-equal no-op, are the
# 200 members page (the no-op with no notice and no record); a name refusal is
# 422 with the input kept; every refusal to someone without the right is the
# uniform 404; a tokenless POST is the CSRF layer's 403.
#
# Harness, oracles and name marks as in team-lifecycle-01 and -03.

@team-lifecycle @tl-04 @real-io
Feature: A team is renamed, and every link to it keeps working

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
      | Marco Bianchi | marco.bianchi@canzan.dev | member         | Ops as Member                  |        |
      | Ines Duarte   | ines.duarte@canzan.dev   | member         |                                |        |
    And Globex is another workspace, whose team Payments is led by Hugo Weber (hugo.weber@globex.example)
    And Ines Duarte is an instance admin of this foundry, but not a workspace admin

  @us-tl-05 @driving_port @kpi @contract-shape:bounded-change @pending
  Scenario: A workspace admin renames a team and every link to it keeps working
    When Priya renames Backend to "Platform Engineering"
    Then she sees "Renamed Backend to Platform Engineering."
    And Backend is now called "Platform Engineering" at the same address, one rename record names Priya Raman, and nothing else changed
    And Priya's teams page lists "Platform Engineering · 3 members"
    And the members page at the address "backend" opens for Priya
    And the Identity Platform board still opens for Lena

  @us-tl-05 @kpi @contract-shape:bounded-change @pending
  Scenario: A team's lead renames her own team
    When Lena renames Backend to "Platform Engineering"
    Then she sees "Renamed Backend to Platform Engineering."
    And Backend is now called "Platform Engineering" at the same address, one rename record names Lena Park, and nothing else changed

  @us-tl-05 @edge @contract-shape:bounded-change @pending
  Scenario: A change of letter case is a real rename, and a team never collides with itself
    When Priya renames Backend to "backend"
    Then she sees "Renamed Backend to backend."
    And Backend is now called "backend" at the same address, one rename record names Priya Raman, and nothing else changed

  @us-tl-05 @edge @contract-shape:unbounded-preservation @pending
  Scenario: Submitting the same name changes nothing and records nothing
    When Priya submits Backend's rename form unchanged
    Then she gets Backend's members page back with no notice
    And no team, project or membership changed

  @us-tl-05 @error @contract-shape:unbounded-preservation @pending
  Scenario Outline: A rename to a name already in use is refused, and the team keeps its name
    When Priya renames Ops to "<typed>"
    Then she is refused with "Team name must be unique within the workspace", and her text "<typed>" is kept
    And no team, project or membership changed

    Examples:
      | typed    |
      | general  |
      | Backend! |

  @us-tl-05 @error @contract-shape:unbounded-preservation @pending
  Scenario Outline: A rename that breaks the rule is refused with the same words as a create
    When Priya renames Ops to "<typed>"
    Then she is refused with "<reason>", and her text "<typed>" is kept
    And no team, project or membership changed

    Examples:
      | typed            | reason                                         |
      | [SPACE]          | Team name must not be empty                    |
      | Ops[U+202E]Team  | Team name must not contain control characters  |
      | [65×a]           | Team name must be at most 64 characters        |

  @us-tl-05 @edge @contract-shape:unbounded-preservation @pending
  Scenario: A renamed team's old address still holds its old name's place
    Given Priya has renamed General to "Platform"
    When Priya creates the team "General" from the teams page
    Then she is refused with "Team name must be unique within the workspace", and her text "General" is kept
    And no team, project or membership changed

  @us-tl-05 @edge @contract-shape:unbounded-preservation @pending
  Scenario Outline: Rename is offered to the team's lead and to a workspace admin; Delete team only to the admin
    When <who> opens Backend's members page
    Then the members page <rename> Rename
    And the members page <delete> Delete team

    Examples:
      | who   | rename         | delete         |
      | Priya | offers         | offers         |
      | Lena  | offers         | does not offer |
      | Sam   | does not offer | does not offer |

  @us-tl-05 @error @kpi @contract-shape:unbounded-preservation @pending
  Scenario Outline: Someone without the right cannot rename a team
    When <who> submits a rename of <team> to "Platform"
    Then the answer is the page that never existed
    And no team, project or membership changed
    And Priya still finds <team>'s rename form

    Examples:
      | who                  | team    |
      | a signed-out visitor | Backend |
      | Marco                | Backend |
      | Sam                  | Backend |
      | Lena                 | General |
      | Ines                 | Backend |
      | Hugo                 | Backend |

  @us-tl-05 @error @kpi @contract-shape:unbounded-preservation @pending
  Scenario: A rename aimed at another workspace's team or at a team that does not exist is refused the same way
    When Priya submits a rename of Payments to "Platform"
    Then the answer is the page that never existed
    And no team, project or membership changed
    And Hugo still finds Payments's rename form
    When Priya submits a rename of a team that does not exist to "Platform"
    Then the answer is the page that never existed

  @us-tl-05 @error @contract-shape:unbounded-preservation @pending
  Scenario: A lead who has since been made a member cannot rename from a page she opened as a lead
    Given Lena opened Backend's members page earlier
    And Priya has made Sam Okafor a lead of Backend
    And Priya has made Lena Park a member of Backend
    When Lena renames Backend to "Platform" from the page she opened earlier
    Then the answer is the page that never existed
    And no team, project or membership changed

  @us-tl-05 @error @contract-shape:unbounded-preservation @pending
  Scenario: A rename sent without the page's token is turned away, and nothing changes
    When Priya submits a rename of Backend to "Platform" without the page's token
    Then the answer is turned away as a forged form
    And no team, project or membership changed
    And the same rename with the page's token renames Backend to "Platform"
