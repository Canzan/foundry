# Feature: team-lifecycle, slice 03 (US-TL-04) — every team name follows one
# rule, and a name in any script gets a working address (D6, D7, OQ-2, OQ-3,
# OQ-7, INV-TL-4).
#
# DISTILL 2026-10-11: every scenario @pending (ADR-025 scaffolded RED). The
# create races repeat 5 times over HTTP; the 100-run proofs are the use-case's
# (crates/foundry-services/tests/team_lifecycle_use_cases.rs), and the database
# rule's arms, legacy survival and parity are the store's
# (crates/foundry-store/tests/team_lifecycle_migrations.rs).
#
# Corrections applied from DESIGN's Changed Assumptions (recorded in the
# feature-delta's DISTILL section): two concurrent creates of the SAME name give
# one team and one uniqueness refusal, while two concurrent creates of DIFFERENT
# names with no Latin letters both succeed as "team" and "team-2" (1); the
# database refusal is shown with U+0001, U+202E and 65 characters, because
# Postgres refuses U+0000 in text (22021) before any rule runs, while the app
# door refuses a pasted NUL as a control character (2).
#
# Name marks, as in project-name-rule and name-db-checks: [N×c] is the character
# c N times; [TAB], [NUL], [SPACE] and [U+XXXX] are that one character. "At the
# database prompt" is the operator's psql session: the steps send the same
# INSERT or UPDATE a person would type.
#
# Harness and oracles as in team-lifecycle-01: see that file's header.

@team-lifecycle @tl-03 @real-io
Feature: Every team name follows one rule, and a name in any script gets a working address

  Background:
    Given Canzan Labs is the instance's original workspace, with these teams:
      | team    | project           | key  |
      | General | Sandbox           | SBX  |
      | Backend | Identity Platform | AUTH |
    And these people belong to Canzan Labs:
      | person        | email                    | workspace role | teams             | called |
      | Priya Raman   | priya.raman@canzan.dev   | admin          | Backend as Member |        |
      | Jeff Bailey   | jeffabailey@gmail.com    | admin          | General as Lead   | Jeff   |
      | Lena Park     | lena.park@canzan.dev     | member         | Backend as Lead   |        |

  @us-tl-04 @driving_port @contract-shape:bounded-change @pending
  Scenario: A name with no Latin letters gets a working team page
    When Priya creates the team "インフラ" from the teams page
    Then she sees "Created インフラ. You're its lead."
    And Canzan Labs now has the team "インフラ" at the address "team", led by Priya Raman, and nothing else changed
    And the members page at the address "team" opens for Priya

  @us-tl-04 @contract-shape:bounded-change @pending
  Scenario: A second name with no Latin letters gets the next free address
    Given Priya has created the team "インフラ"
    When Priya creates the team "運用" from the teams page
    Then she sees "Created 運用. You're its lead."
    And Canzan Labs now has the team "運用" at the address "team-2", led by Priya Raman, and nothing else changed
    And the members page at the address "team-2" opens for Priya

  @us-tl-04 @edge @contract-shape:bounded-change @pending
  Scenario: The fallback address is the lowest free one
    Given Canzan Labs also has these teams, made before team create existed:
      | team        | address |
      | Team        | team    |
      | Squad Three | team-3  |
    When Priya creates the team "運用" from the teams page
    Then Canzan Labs now has the team "運用" at the address "team-2", led by Priya Raman, and nothing else changed

  @us-tl-04 @error @contract-shape:unbounded-preservation @pending
  Scenario Outline: A name that breaks the rule is refused with its reason, and nothing is created
    When Priya creates the team "<typed>" from the teams page
    Then she is refused with "<reason>", and her text "<typed>" is kept
    And no team, project or membership changed

    Examples:
      | typed                | reason                                         |
      | Platform[U+202E]Ops  | Team name must not contain control characters  |
      | Platform[TAB]Ops     | Team name must not contain control characters  |
      | Platform[NUL]Ops     | Team name must not contain control characters  |
      | [65×a]               | Team name must be at most 64 characters        |
      | [65×日]              | Team name must be at most 64 characters        |
      | [SPACE][TAB]         | Team name must not be empty                    |

  @us-tl-04 @edge @contract-shape:bounded-change @pending
  Scenario Outline: A name of exactly 64 characters is accepted
    When Priya creates the team "<typed>" from the teams page
    Then she sees "Created <typed>. You're its lead."
    And Canzan Labs now has the team "<typed>" at the address "<address>", led by Priya Raman, and nothing else changed

    Examples:
      | typed   | address |
      | [64×a]  | [64×a]  |
      | [64×日] | team    |

  @us-tl-04 @error @contract-shape:unbounded-preservation @pending
  Scenario Outline: A name whose address or letters would collide is refused
    When Priya creates the team "<typed>" from the teams page
    Then she is refused with "Team name must be unique within the workspace", and her text "<typed>" is kept
    And no team, project or membership changed

    Examples:
      | typed               |
      | Back-End            |
      | BACKEND             |
      | [SPACE]backend[TAB] |

  @us-tl-04 @error @kpi @concurrency @contract-shape:bounded-change @pending
  Scenario: Two admins creating the same name at once get one team
    When Priya and Jeff create the team "Infra" at the same moment, 5 times over
    Then each time, exactly one "Infra" team exists and the other admin was refused with "Team name must be unique within the workspace"
    And no answer was a server error

  @us-tl-04 @error @concurrency @contract-shape:bounded-change @pending
  Scenario: Two admins creating the same name with no Latin letters at once get one team
    When Priya and Jeff create the team "インフラ" at the same moment, 5 times over
    Then each time, exactly one "インフラ" team exists and the other admin was refused with "Team name must be unique within the workspace"
    And no answer was a server error

  @us-tl-04 @edge @concurrency @contract-shape:bounded-change @pending
  Scenario: Two admins creating different names with no Latin letters at once both get a team
    When Priya creates the team "インフラ" at the same moment as Jeff creates "運用", 5 times over
    Then each time, both teams were created, at the addresses "team" and "team-2"
    And no answer was a server error

  @us-tl-04 @error @adapter-integration @contract-shape:unbounded-preservation @pending
  Scenario Outline: A team name typed at the database prompt that breaks the rule is refused under the rule's name
    When the operator adds a team named "<typed>" to Canzan Labs at the database prompt
    Then the database refuses the team name under the rule "<rule>"
    And no team, project or membership changed

    Examples:
      | typed               | rule                         |
      | Platform[U+0001]Ops | teams_name_no_control_chars  |
      | Platform[U+202E]Ops | teams_name_no_control_chars  |
      | [65×a]              | teams_name_max_64_chars      |
      | [SPACE]             | teams_name_not_empty         |
      | [SPACE]Platform     | teams_name_trimmed           |

  @us-tl-04 @error @adapter-integration @contract-shape:unbounded-preservation @pending
  Scenario Outline: A team renamed at the database prompt to a name that breaks the rule is refused
    When the operator renames team Backend to "<typed>" at the database prompt
    Then the database refuses the team name under the rule "<rule>"
    And no team, project or membership changed

    Examples:
      | typed               | rule                         |
      | Backend[U+202E]     | teams_name_no_control_chars  |
      | [65×a]              | teams_name_max_64_chars      |

  @us-tl-04 @edge @adapter-integration @contract-shape:bounded-change @pending
  Scenario: A team name the app would accept is stored at the database prompt exactly as typed
    When the operator adds a team named "[64×日]" to Canzan Labs at the database prompt
    Then the database stores the team name exactly as typed, and nothing else changed
