# Feature: name-db-checks — the database itself refuses a workspace or project name
# that every app door refuses, for new name writes only, and says which rule was
# broken. Names stored before the upgrade keep working: they list, render, take new
# issues, rename to a fit name, and survive a backup and restore.
#
# DISTILL 2026-10-08: every scenario is @pending (the repo convention: pending
# scenarios are excluded from every lane; DELIVER removes the tag as each one lands,
# slice by slice). The gate run that classified each one is in
# docs/feature/name-db-checks/feature-delta.md (## Wave: DISTILL).
#
# The rule (DISCUSS D1-D3, DESIGN DDD-1/2/6): on a name WRITE (a new row, or a row
# whose name changes) the database trims the value with exactly the app's whitespace
# set, then refuses, in the app's order: empty ("not_empty"), a refused character
# ("no_control_chars": Unicode Cc, U+202A-202E, U+2066-2069, U+2028, U+2029), more
# than 24 (workspaces) or 256 (projects) characters ("max_24_chars",
# "max_256_chars"). If all three pass but trimming changed the value, the reason is
# "trimmed": the database stores only what the app could have stored. Each refusal is
# the operator's familiar check-violation error, naming the rule as
# "<table>_name_<reason>" (for example workspaces_name_no_control_chars), so the
# operator at the prompt reads which rule was broken. Uniqueness stays in the app
# (D1): two projects may share a name when written by hand. A write that does not
# change the name (filing an issue bumps the project's issue counter; a rewrite of
# the same name) never meets the rule (D6, DDD-5).
#
# "At the database prompt" is Priya's psql session: the steps send the same INSERT
# or UPDATE her session would, as a parameterised statement on the scenario's own
# schema (DESIGN OQ-D2). "Stored before the upgrade" seeds a row through the one
# test-support legacy seam (DDD-10), the way history wrote it: with the rule absent.
#
# INVISIBLE CHARACTERS AND LONG NAMES IN THIS FILE. A name in quotes is exactly what
# is typed, except for these bracketed marks (the iwnr and pnr notation):
#   [TAB] tab, [NEWLINE] line feed, [SPACE] a space (example tables drop edge
#   spaces), [NBSP] no-break space U+00A0, [ZWJ] zero-width joiner U+200D,
#   [ZWNJ] zero-width non-joiner U+200C, [U+XXXX] the character with that code
#   point, and [N×c] the single character c written N times ([257×日] is 257 CJK
#   characters, three bytes each; [25×😀] is 25 four-byte characters). An empty
#   example cell is the empty name. Any other bracketed text is literal.
#
# Oracles. Before every write the steps capture every workspace (id, name), every
# project (name, address, key prefix, issue counter) and the issue count. A refusal
# moves NONE of it (fail-closed). An accepted rename changes exactly that one name;
# an accepted addition adds exactly one row under exactly the typed name. Filing an
# issue moves only that project's counter and adds one issue.
#
# Harness: the SAME in-process axum router + real session/CSRF layers + real Postgres
# (shared testcontainer, per-scenario schema migrated by the shipped migrations)
# every instance-admin scenario uses; the restore scenarios add the us-03 pg_dump /
# pg_restore client container and restore target, and the real `foundry` binary for
# backup-verify. No fake: every port here is driving (SQL, HTTP, CLI) or
# driven-internal (Postgres).
#
# Tags: @driving_port marks the FIRST scenario through each driving port (the
# operator's SQL session for each table, the board's new-issue door, restore), as in
# the precedents; every other scenario uses the same ports.
#
# Shared vocabulary (cucumber steps are global): Priya, the Canzan Labs / Backend /
# "Auth v2" (AUTH) / "Sandbox" (SBX) Background and the dashboard renames are the
# iapr, iawr and pnr steps; backing up, restoring and backup-verify are the us-03
# steps.

@ndc
Feature: The database refuses a name every app door refuses, and leaves older names alone

  Background:
    Given Priya is the instance super-admin
    And workspace "Canzan Labs" has a team "Backend" with projects "Auth v2" (AUTH) and "Sandbox" (SBX)
    And workspace "Household" exists with no projects

  # ======================================================================= US-NDC-01
  Rule: A hand-typed workspace name that breaks the rule is refused by the database, which names the rule

    @us-ndc-01 @driving_port @error @real-io @contract-shape:unbounded-preservation
    Scenario Outline: A hand-typed workspace rename that breaks the rule is refused under the rule's name, and nothing changes
      When the operator renames workspace "Household" to "<typed>" at the database prompt
      Then the database refuses it under the rule "<rule>"
      And no workspace or project changed

      Examples:
        | typed                                  | rule                             |
        | House[TAB]Hold                         | workspaces_name_no_control_chars |
        | Ops[U+202E]gnikcatS                    | workspaces_name_no_control_chars |
        | Kit[U+009F]chen                        | workspaces_name_no_control_chars |
        | Canzan Labs Platform Engineering Group | workspaces_name_max_24_chars     |
        | [25×日]                                | workspaces_name_max_24_chars     |
        | [SPACE]Globex                          | workspaces_name_trimmed          |
        | Globex[NBSP]                           | workspaces_name_trimmed          |
        | [U+3000]Globex                         | workspaces_name_trimmed          |
        |                                        | workspaces_name_not_empty        |

    @us-ndc-01 @error @real-io @contract-shape:unbounded-preservation
    Scenario Outline: The reason given is the first rule the name breaks, in the app's order
      When the operator renames workspace "Household" to "<typed>" at the database prompt
      Then the database refuses it under the rule "<rule>"
      And no workspace or project changed

      Examples:
        | typed                                  | rule                             |
        | [SPACE][TAB][SPACE]                    | workspaces_name_not_empty        |
        | [SPACE]Glo[TAB]bex                     | workspaces_name_no_control_chars |
        | Canzan Labs Platform[TAB]Engineering   | workspaces_name_no_control_chars |
        | [U+2028]Globex                         | workspaces_name_trimmed          |
        | Glo[U+2028]bex                         | workspaces_name_no_control_chars |
        | [24×x][SPACE]                          | workspaces_name_trimmed          |

    @us-ndc-01 @error @real-io @contract-shape:unbounded-preservation
    Scenario Outline: A hand-added workspace whose name breaks the rule is refused, and nothing is added
      When the operator adds a workspace named "<typed>" at the database prompt
      Then the database refuses it under the rule "<rule>"
      And no workspace or project changed

      Examples:
        | typed                 | rule                             |
        | Globex[SPACE]         | workspaces_name_trimmed          |
        | [25×😀]               | workspaces_name_max_24_chars     |
        | Glo[NEWLINE]bex       | workspaces_name_no_control_chars |
        |                       | workspaces_name_not_empty        |

    @us-ndc-01 @error @real-io @contract-shape:bounded-change
    Scenario: After a refused hand-typed name, the corrected name lands
      Given the operator's rename of workspace "Household" to "[SPACE]Globex" at the database prompt was refused
      When the operator renames workspace "Household" to "Globex" at the database prompt
      Then the database stores exactly "Globex" and nothing else changed

    @us-ndc-01 @edge @guard @real-io @contract-shape:bounded-change
    Scenario Outline: A workspace name the app would accept is stored exactly as typed
      When the operator renames workspace "Household" to "<typed>" at the database prompt
      Then the database stores exactly "<typed>" and nothing else changed

      Examples:
        | typed                    |
        | 👨[ZWJ]👩[ZWJ]👧 Bailey   |
        | Ångström Øresund Société |
        | Canzan[U+200B]Labs       |
        | [U+FEFF]Globex           |
        | Glo[NBSP]bex             |
        | [24×😀]                  |
        | Canzan Labs              |

    @us-ndc-01 @edge @guard @real-io @contract-shape:bounded-change
    Scenario Outline: A hand-added workspace with a fit name is stored exactly as typed
      When the operator adds a workspace named "<typed>" at the database prompt
      Then the database stores exactly "<typed>" and nothing else changed

      Examples:
        | typed   |
        | Globex  |
        | [24×日] |

    @us-ndc-01 @edge @guard @real-io @contract-shape:unbounded-preservation
    Scenario: A workspace stored before the upgrade keeps its name when it is rewritten unchanged by hand
      Given workspace "Canzan Labs Platform Engineering Group" was stored before the upgrade
      When the operator renames workspace "Canzan Labs Platform Engineering Group" to "Canzan Labs Platform Engineering Group" at the database prompt
      Then the database accepts it and nothing changed

    @us-ndc-01 @error @real-io @contract-shape:unbounded-preservation
    Scenario: A workspace stored before the upgrade cannot be renamed by hand to another name that breaks the rule
      Given workspace "Canzan[TAB]Labs" was stored before the upgrade
      When the operator renames workspace "Canzan[TAB]Labs" to "Canzan[TAB]Labs Ops" at the database prompt
      Then the database refuses it under the rule "workspaces_name_no_control_chars"
      And no workspace or project changed

    @us-ndc-01 @edge @real-io @contract-shape:bounded-change
    Scenario: A workspace stored before the upgrade is renamed on the dashboard to a fit name, and the rename is on record
      Given workspace "Canzan Labs Platform Engineering Group" was stored before the upgrade
      When Priya renames workspace "Canzan Labs Platform Engineering Group" to "Canzan Platform"
      Then the workspace row she gets back shows "Canzan Platform"
      And workspace "Canzan Platform" has exactly 1 rename on record

  # ======================================================================= US-NDC-02
  Rule: A hand-typed project name that breaks the rule is refused, and older projects still take new issues

    @us-ndc-02 @driving_port @error @real-io @contract-shape:unbounded-preservation
    Scenario Outline: A hand-typed project rename that breaks the rule is refused under the rule's name, and nothing changes
      When the operator renames project "Sandbox" to "<typed>" at the database prompt
      Then the database refuses it under the rule "<rule>"
      And no workspace or project changed

      Examples:
        | typed              | rule                           |
        | Sand[TAB]box       | projects_name_no_control_chars |
        | Ops[U+2066]Board   | projects_name_no_control_chars |
        | [257×x]            | projects_name_max_256_chars    |
        | [257×日]           | projects_name_max_256_chars    |
        | Homelab Ops[SPACE] | projects_name_trimmed          |
        | [U+205F]Sandbox    | projects_name_trimmed          |
        |                    | projects_name_not_empty        |
        | [SPACE][NBSP]      | projects_name_not_empty        |

    @us-ndc-02 @error @real-io @contract-shape:unbounded-preservation
    Scenario Outline: A hand-added project whose name breaks the rule is refused, and nothing is added
      When the operator adds project "<typed>" (HLO) to team "Backend" at the database prompt
      Then the database refuses it under the rule "<rule>"
      And no workspace or project changed

      Examples:
        | typed              | rule                           |
        | Homelab Ops[SPACE] | projects_name_trimmed          |
        | [257×a]            | projects_name_max_256_chars    |
        | Home[U+0085]lab    | projects_name_no_control_chars |

    @us-ndc-02 @error @real-io @contract-shape:unbounded-preservation
    Scenario: A hand-typed project rename that also moves the issue counter is refused as a whole
      When the operator renames project "Sandbox" to "Sand[TAB]box" and moves its issue counter on by 5 in the same statement at the database prompt
      Then the database refuses it under the rule "projects_name_no_control_chars"
      And no workspace or project changed

    @us-ndc-02 @edge @guard @real-io @contract-shape:bounded-change
    Scenario Outline: A project name the app would accept is stored exactly as typed, even beside a sibling of the same name
      When the operator renames project "Sandbox" to "<typed>" at the database prompt
      Then the database stores exactly "<typed>" and nothing else changed

      Examples:
        | typed          |
        | [256×日]       |
        | [256×😀]       |
        | Sand[ZWNJ]box  |
        | auth v2        |

    @us-ndc-02 @edge @guard @real-io @contract-shape:bounded-change
    Scenario Outline: A hand-added project with a fit name is stored exactly as typed
      When the operator adds project "<typed>" (HLO) to team "Backend" at the database prompt
      Then the database stores exactly "<typed>" and nothing else changed

      Examples:
        | typed       |
        | Homelab Ops |
        | [256×a]     |

    @us-ndc-02 @driving_port @edge @guard @kpi @real-io @contract-shape:bounded-change
    Scenario: A project stored before the upgrade with a tab in its name still takes new issues
      Given project "Homelab[TAB]Ops" (OPS) was stored before the upgrade, with OPS-7 its last issue
      When Priya files "Replace UPS battery" on the "Homelab[TAB]Ops" board
      Then the issue is created as OPS-8 and shows on that board
      And project "Homelab[TAB]Ops" kept its name, address and key prefix

    @us-ndc-02 @edge @guard @real-io @contract-shape:unbounded-preservation
    Scenario: A project stored before the upgrade can be left as it is at the dashboard
      Given project "Homelab[TAB]Ops" (OPS) was stored before the upgrade, with OPS-7 its last issue
      When Priya renames project "Homelab[TAB]Ops" to the pasted name "Homelab[TAB]Ops"
      Then the row she gets back still shows "Homelab[TAB]Ops" and carries no error
      And no project changed and nothing was created

    @us-ndc-02 @edge @real-io @contract-shape:bounded-change
    Scenario: A project stored before the upgrade is renamed on the dashboard to a fit name
      Given project "Homelab[TAB]Ops" (OPS) was stored before the upgrade, with OPS-7 its last issue
      When Priya renames project "Homelab[TAB]Ops" to the pasted name "Homelab Ops"
      Then project "Homelab[TAB]Ops" is now named "Homelab Ops", and its board still opens at its original address

    @us-ndc-02 @edge @guard @real-io @contract-shape:unbounded-preservation
    Scenario: A project stored before the upgrade keeps its name when it is rewritten unchanged by hand
      Given project "Homelab[TAB]Ops" (OPS) was stored before the upgrade, with OPS-7 its last issue
      When the operator renames project "Homelab[TAB]Ops" to "Homelab[TAB]Ops" at the database prompt
      Then the database accepts it and nothing changed

    @us-ndc-02 @error @real-io @contract-shape:unbounded-preservation
    Scenario: A project stored before the upgrade cannot be renamed by hand to another name that breaks the rule
      Given project "[300×x]" (LNG) was stored before the upgrade, with LNG-1 its last issue
      When the operator renames project "[300×x]" to "[299×x]" at the database prompt
      Then the database refuses it under the rule "projects_name_max_256_chars"
      And no workspace or project changed

  # ======================================================================= US-NDC-03
  Rule: A backup that holds names from before the upgrade still restores and verifies

    @us-ndc-03 @needs-pgclient @driving_port @edge @guard @real-io @contract-shape:unbounded-preservation
    Scenario: A backup holding names from before the upgrade restores, and the rule still holds afterwards
      Given workspace "Canzan Labs Platform Engineering Group" was stored before the upgrade
      And project "Homelab[TAB]Ops" (OPS) was stored before the upgrade, with OPS-7 its last issue
      When the operator backs up and restores the database
      Then the restored instance holds workspace "Canzan Labs Platform Engineering Group" and project "Homelab[TAB]Ops" byte for byte
      And the name rule is switched on in the restored instance
      And renaming workspace "Household" to "House[TAB]Hold" by hand in the restored instance is refused under the rule "workspaces_name_no_control_chars"

    @us-ndc-03 @needs-pgclient @driving_adapter @edge @guard @real-io @contract-shape:unbounded-preservation
    Scenario: Verifying a backup that holds names from before the upgrade reports it healthy
      Given workspace "Canzan Labs Platform Engineering Group" was stored before the upgrade
      And project "Homelab[TAB]Ops" (OPS) was stored before the upgrade, with OPS-7 its last issue
      And the operator has captured a backup of the database to a file
      When the operator runs `foundry doctor backup-verify <backup-file>` as a subprocess
      Then the exit code is 0
      And the stdout contains a "status: OK" line
