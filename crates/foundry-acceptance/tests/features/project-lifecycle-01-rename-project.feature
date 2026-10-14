# Feature: project-lifecycle, slice 01 (US-PL-01) — a team's lead or a
# workspace admin renames a project from the team's members page; the slug, the
# key and every link stay, and every effective rename is on record, on this door
# and on the instance dashboard's (D2, D3, D8, D16, OQ-2, OQ-3, OQ-U1 = (b),
# INV-PL-7, INV-PL-8, INV-PL-9).
#
# Authored @pending in DISTILL 2026-10-13 (ADR-025 scaffolded RED, rigor profile
# adr-025-scaffolded-red); DELIVER un-pends one scenario at a time.
#
# Statuses (DESIGN [REF] Statuses, DDD-24, DDD-27): a rename that goes through
# is the 200 members page with "Renamed {old} to {new}."; a byte-equal name is
# the 200 members page with no notice and no record; a name refusal is a 422
# members page with the ProjectNameError copy in that project's row and the
# typed text kept; every refusal to someone without the right, and every
# unknown, foreign or deleted project, is the uniform 404 byte-identical to a
# page that never existed; a tokenless POST is the shared CSRF layer's 403.
#
# The Projects section (DDD-25, NFR-PL-1, pinned here): an h2 "Projects", one
# list item per live project, by name then key, whose text starts
# "{project} · {KEY}". The text is a link to the board ONLY when the viewer is on
# the team (OQ-U1 = (b)). For a `Manage` caller each row also holds a form that
# posts `name` to /team/{t}/project/{p}/rename (prefilled with the current name,
# labelled "Project name" plus the project's name, then a Rename button) and a
# "Delete project" link to /team/{t}/project/{p}/delete, in that keyboard order.
#
# Harness and oracles: the shipped in-process router with the real session and
# CSRF layers, real Postgres (per-scenario schema), the team-membership
# Background and its uniform-404 oracle. The universe every write is judged
# against (Mandate 8) is team-lifecycle's (teams, rows, tokens, team records)
# plus every project row with its deleted marker, a checksum of everything
# under each project (issues, comments, attachments, history, lanes, queued
# notifications) and every project lifecycle record. Preconditions are seeded
# (projects by SQL with their stored address, issues through the product); the
# outcome under test is always written by the product (No Fixture Theater).
#
# Base-table reads (user decision U2, 2026-10-13, house precedent OQ-D6): the
# universe, "nothing erased" and the stored deleted marker are read with
# read-only fixture SQL on the base tables; whether a project is hidden is
# asserted only through the product's doors (pages, the API, the operator CLI).
#
# Name marks (project-name-rule): [N×c] is the character c, N times; [TAB] and
# [U+XXXX] are the invisible characters they name.

@project-lifecycle @pl-01 @real-io
Feature: A team's lead or a workspace admin renames a project from the team's page, and every link keeps working

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

  # ------------------------------------------------------------ renames that go through

  @us-pl-01 @driving_port @kpi @pending @contract-shape:bounded-change
  Scenario: A team's lead renames a project and every link to it keeps working
    Given Identity Platform has 7 issues
    When Lena renames the project Identity Platform to "Identity & Access" on Backend's members page
    Then she sees "Renamed Identity Platform to Identity & Access."
    And Backend's Projects section lists, in this order:
      | Auth v2 Spike · SPIKE    |
      | Identity & Access · AUTH |
    And Identity Platform is now called "Identity & Access" at the address "auth", one rename record names Lena Park, and nothing else changed
    And Identity & Access's board and its issue AUTH-7 still open for Lena at their old addresses

  @us-pl-01 @kpi @pending @contract-shape:bounded-change
  Scenario: After a rename every page of the project answers as it did before
    Given Identity Platform has 1 issue, 1 comment and 1 attachment
    And Lena has noted how every page of Identity Platform answers her
    When Lena renames the project Identity Platform to "Identity & Access" on Backend's members page
    Then Identity Platform is now called "Identity & Access" at the address "auth", one rename record names Lena Park, and nothing else changed
    And every page of Identity Platform answers Lena as it did before the rename

  @us-pl-01 @kpi @pending @contract-shape:bounded-change
  Scenario: A workspace admin renames a project of a team she is not on
    When Priya renames the project Homelab Ops to "Homelab Operations" on Ops's members page
    Then she sees "Renamed Homelab Ops to Homelab Operations."
    And Homelab Ops is now called "Homelab Operations" at the address "homelab-ops", one rename record names Priya Raman, and nothing else changed

  @us-pl-01 @kpi @pending @contract-shape:bounded-change
  Scenario: A rename made from the instance dashboard is on record too
    When Ines renames the project Reading List to "Reading Queue" from the instance dashboard
    Then Reading List is now called "Reading Queue" at the address "reading-list", one rename record names Ines Duarte, and nothing else changed

  @us-pl-01 @edge @pending @contract-shape:bounded-change
  Scenario: A change of letter case is a real rename, and a project never collides with itself
    When Lena renames the project Identity Platform to "identity platform" on Backend's members page
    Then she sees "Renamed Identity Platform to identity platform."
    And Identity Platform is now called "identity platform" at the address "auth", one rename record names Lena Park, and nothing else changed

  @us-pl-01 @edge @pending @contract-shape:bounded-change
  Scenario: A name of exactly 256 characters is accepted
    When Lena renames the project Identity Platform to "[256×a]" on Backend's members page
    Then she sees the project renamed from "Identity Platform" to "[256×a]"
    And Identity Platform is now called "[256×a]" at the address "auth", one rename record names Lena Park, and nothing else changed

  @us-pl-01 @edge @pending @contract-shape:unbounded-preservation
  Scenario: Submitting the same name changes nothing and records nothing
    When Lena submits Identity Platform's project rename unchanged on Backend's members page
    Then she gets Backend's members page back with no notice
    And no project, issue or record changed

  # ------------------------------------------------------------ the rule and its copy

  @us-pl-01 @error @pending @contract-shape:unbounded-preservation
  Scenario: A name already used in the team is refused, and the text is kept
    When Priya renames the project Reading List to "homelab ops" on Ops's members page
    Then she is refused with "Project name must be unique within the team" in Reading List's row, and her text "homelab ops" is kept
    And no project, issue or record changed

  @us-pl-01 @error @pending @contract-shape:unbounded-preservation
  Scenario Outline: A name that breaks the project-name rule is refused with its reason, and the text is kept
    When Lena renames the project Identity Platform to "<name>" on Backend's members page
    Then she is refused with "<reason>" in Identity Platform's row, and her text "<name>" is kept
    And no project, issue or record changed

    Examples:
      | name             | reason                                           |
      |                  | Project name must not be empty                   |
      | [3× ]            | Project name must not be empty                   |
      | Identity[TAB]Hub | Project name must not contain control characters |
      | [257×a]          | Project name must be at most 256 characters      |

  # ------------------------------------------------------------ who sees what

  @us-pl-01 @edge @pending @contract-shape:unbounded-preservation
  Scenario: The Projects section names each project by name then key, and each field says which project it is for
    When Lena opens Backend's members page
    Then Backend's Projects section lists, in this order:
      | Auth v2 Spike · SPIKE    |
      | Identity Platform · AUTH |
    And each project's name field is labelled "Project name" with the project's name
    And within each project row the keyboard reaches the name field, then Rename, then Delete project
    And every project row's rename and delete controls work as plain page forms

  @us-pl-01 @edge @pending @contract-shape:unbounded-preservation
  Scenario Outline: Everyone on the team sees its projects, and only its lead or a workspace admin gets the controls
    When <who> opens <team>'s members page
    Then each project row <link>
    And each project row <controls>

    Examples:
      | who   | team    | link                | controls                                         |
      | Lena  | Backend | links to its board  | offers Project name, Rename and Delete project   |
      | Sam   | Backend | links to its board  | offers no Project name, Rename or Delete project |
      | Priya | Backend | links to its board  | offers Project name, Rename and Delete project   |
      | Priya | Ops     | is not a link       | offers Project name, Rename and Delete project   |
      | Tomás | Ops     | links to its board  | offers Project name, Rename and Delete project   |

  @us-pl-01 @edge @pending @contract-shape:unbounded-preservation
  Scenario: A team with no projects says so
    When Tomás opens Labs's members page
    Then he sees "Labs has no projects yet."

  # ------------------------------------------------------------ refusals

  @us-pl-01 @error @kpi @pending @contract-shape:unbounded-preservation
  Scenario Outline: Only the team's lead or a workspace admin can rename a project
    When <who> submits a rename of the project <project> to "Renamed by mistake"
    Then the answer is the page that never existed
    And no project, issue or record changed
    And Priya still finds the rename form of <project>

    Examples:
      | who                  | project           |
      | a signed-out visitor | Identity Platform |
      | Marco                | Identity Platform |
      | Sam                  | Identity Platform |
      | Lena                 | Reading List      |
      | Ines                 | Identity Platform |
      | Hugo                 | Identity Platform |

  @us-pl-01 @error @kpi @pending @contract-shape:unbounded-preservation
  Scenario Outline: A rename aimed at another workspace's project or at a project that does not exist is refused the same way
    When Priya submits a rename of <target> to "Renamed by mistake"
    Then the answer is the page that never existed
    And no project, issue or record changed
    And Hugo still finds the rename form of Ledger

    Examples:
      | target                        |
      | the project Ledger            |
      | a project that does not exist |

  @us-pl-01 @error @pending @contract-shape:unbounded-preservation
  Scenario: A lead who has since been made a member cannot rename from a page she opened as a lead
    Given Lena opened Backend's members page earlier
    And Priya has made Sam a lead of Backend
    And Priya has made Lena a member of Backend
    When Lena renames the project Identity Platform to "Identity & Access" from the page she opened earlier
    Then the answer is the page that never existed
    And no project, issue or record changed
    And Sam still finds the rename form of Identity Platform

  @us-pl-01 @error @pending @contract-shape:unbounded-preservation
  Scenario: A rename sent without the page's token is turned away, and nothing changes
    When Lena submits a rename of the project Identity Platform to "Identity & Access" without the page's token
    Then the answer is turned away as a forged form
    And no project, issue or record changed
    And the same project rename with the page's token renames Identity Platform to "Identity & Access"
