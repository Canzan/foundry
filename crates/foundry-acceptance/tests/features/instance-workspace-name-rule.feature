# Feature: instance-workspace-name-rule — every door that names a workspace (the
# dashboard Provision form, the operator CLI, the first-run bootstrap claim, and the
# shipped rename) refuses the same names for the same reasons, in the same words.
#
# DISTILL 2026-10-05: every scenario is @pending (the repo convention: pending
# scenarios are excluded from every lane; DELIVER removes the tag as each one lands,
# slice by slice). The gate run that classified each one RED for the right reason is
# in docs/feature/instance-workspace-name-rule/feature-delta.md (## Wave: DISTILL).
#
# The rule (D1, D4, D5): trim, then (rename only) the no-op, then empty, then control
# characters, then at most 24 Unicode scalars. The trimmed value is what is stored.
# The refused set (D4): Unicode Cc (U+0000-001F, U+007F-009F), the bidi controls
# U+202A-202E and U+2066-2069, and U+2028/U+2029. Every other format character stays
# allowed (zero-width joiner, zero-width non-joiner, no-break space ...). Whitespace
# at either end, tabs and line breaks included, is trimmed before any check (OQ-D2).
# The copy (D3) is byte-identical on every door:
#   "Workspace name must not be empty"
#   "Workspace name must not contain control characters"
#   "Workspace name must be at most 24 characters"
#
# INVISIBLE CHARACTERS IN THIS FILE. A name in quotes is exactly what is typed or
# pasted, except for these bracketed marks, each standing for one character:
#   [TAB] tab, [NEWLINE] line feed, [NUL] the null character, [SPACE] a space (for
#   example tables, which drop edge spaces), [NBSP] no-break space U+00A0,
#   [ZWJ] zero-width joiner U+200D, [ZWNJ] zero-width non-joiner U+200C, and
#   [U+XXXX] the character with that code point (e.g. [U+202E], right-to-left
#   override). Any other bracketed text is literal.
#
# Oracles. Nothing is left behind by a refusal (D7, KPI-3): the steps capture every
# workspace by id and name, the row counts of users, memberships, teams, projects,
# lanes, invites, instance admins and rename records, and the count of live bootstrap
# links, before the attempt, and assert none of it moved. Authz and dead-link answers
# keep their precedence and stay byte-identical (D8). A NUL never becomes an internal
# error (KPI-4). The four doors give the same verdict and the same words for the same
# input (KPI-2, the parity outlines at the end; input set per OQ-D1).
#
# Harness: the SAME in-process axum router + real session/CSRF layers + real Postgres
# (shared testcontainer, per-scenario schema) every instance-admin and bootstrap
# scenario uses; the CLI scenarios run the real `foundry` binary as a subprocess
# against that schema; @needs-browser scenarios drive a real headless Chrome. No fake:
# every port here is driving (HTTP, CLI) or driven-internal (Postgres).
#
# Shared vocabulary (cucumber steps are global): Priya, Marco, the never-existed
# answer, the dashboard in the browser and the rename Thens are the iapr/iawr steps;
# the fresh instance, link minting, the dashboard redirect, the first instance admin
# and the unconsumed link are the us-05 / bootstrap-enum-oracle steps.

@iwnr
Feature: One rule for naming a workspace, at every door

  # ======================================================================= US-WNR-01
  Rule: A pasted name carrying invisible characters is refused at rename

    Background:
      Given Priya is the instance super-admin
      And workspace "Canzan Labs" has a team "Backend" with projects "Auth v2" (AUTH) and "Sandbox" (SBX)
      And workspace "Household" exists with no projects

    @us-wnr-01 @error @real-io @contract-shape:unbounded-preservation
    Scenario Outline: A name with an invisible character inside it is refused inside the row
      When Priya renames workspace "Household" to the pasted name "<pasted>"
      Then the workspace rename is refused saying "Workspace name must not contain control characters"
      And workspace "Household" is unchanged with no rename on record

      Examples:
        | pasted               |
        | House[TAB]hold       |
        | House[U+007F]hold    |
        | Ops[U+202E]gnikcatS  |
        | Ops[U+2066]Stacking  |
        | Ops[U+2028]Platform  |

    @us-wnr-01 @error @kpi @real-io @contract-shape:unbounded-preservation
    Scenario: A name with a null character is refused with the reason, not an internal error
      When Priya renames workspace "Household" to the pasted name "Bailey[NUL]Family"
      Then the workspace rename is refused saying "Workspace name must not contain control characters"
      And workspace "Household" is unchanged with no rename on record

    @us-wnr-01 @edge @real-io @contract-shape:bounded-change
    Scenario Outline: Joined, accented and spaced names are still accepted
      When Priya renames workspace "Household" to the pasted name "<pasted>"
      Then the pasted name "<pasted>" is stored exactly, with one rename on record

      Examples:
        | pasted                   |
        | 👨[ZWJ]👩[ZWJ]👧 Bailey   |
        | Mehr[ZWNJ]dad Household  |
        | Ops[NBSP]Team            |
        | Ångström Øresund Société |

    @us-wnr-01 @edge @real-io @contract-shape:bounded-change
    Scenario: Tabs and line breaks at either end are trimmed, not refused
      When Priya renames workspace "Household" to the pasted name "[TAB]Kitchen[NEWLINE]"
      Then the workspace row she gets back shows "Kitchen"
      And the latest rename on record for workspace "Kitchen" names Priya, from "Household" to "Kitchen", at the time of the rename

    @us-wnr-01 @edge @error @real-io @contract-shape:unbounded-preservation
    Scenario: A name of only spaces and tabs counts as empty
      When Priya renames workspace "Household" to the pasted name "[SPACE][TAB][SPACE]"
      Then the workspace rename is refused saying "Workspace name must not be empty"
      And workspace "Household" is unchanged with no rename on record

    @us-wnr-01 @error @real-io @contract-shape:unbounded-preservation
    Scenario: A long name with a tab is refused for the tab first
      When Priya renames workspace "Household" to the pasted name "Canzan Labs Platform[TAB]Engineering"
      Then the workspace rename is refused saying "Workspace name must not contain control characters"
      And workspace "Household" is unchanged with no rename on record

    @us-wnr-01 @edge @guard @real-io @contract-shape:unbounded-preservation
    Scenario: An untouched name with a tab from before the rule can be left as it is
      Given workspace "Canzan[TAB]Labs" was named before the rule existed
      When Priya renames workspace "Canzan[TAB]Labs" to the pasted name "Canzan[TAB]Labs"
      Then the workspace row she gets back shows the pasted name "Canzan[TAB]Labs" and carries no error
      And no workspace changed and nothing new went on record

    @us-wnr-01 @needs-browser @error @real-io @contract-shape:unbounded-preservation
    Scenario: A pasted tab is explained inside the row on the real page
      Given Priya has the instance dashboard open in her browser
      When she pastes "House[TAB]hold" over the "Household" workspace name in her browser and submits it
      Then "Workspace name must not contain control characters" appears inside that workspace row's message area
      And the workspace rename form is still there for her to correct

  # ======================================================================= US-WNR-02
  Rule: A first-run claim with an unfit workspace name is refused without burning the link

    Background:
      Given a fresh Foundry instance with no workspace and no users
      And the bootstrap token "claim-001" was minted 1 minute ago with a 30-minute TTL

    @us-wnr-02 @driving_port @error @real-io @contract-shape:unbounded-preservation
    Scenario: An over-long workspace name is refused and the link still works
      When Priya claims the instance through link "claim-001" naming the workspace "Raman Household Operations Center"
      Then the claim page is shown again saying "Workspace name must be at most 24 characters"
      And her email, display name and workspace name are still filled in, and her password is not
      And nothing was created or changed
      And the bootstrap token "claim-001" remains unconsumed

    @us-wnr-02 @driving_port @real-io @contract-shape:bounded-change
    Scenario: The corrected claim with the same link succeeds
      Given Priya's claim through link "claim-001" was refused for the workspace name "Raman Household Operations Center"
      When Priya claims the instance through link "claim-001" naming the workspace "Raman Household"
      Then the response redirects the admin to the workspace dashboard
      And the workspace "Raman Household" exists with a first instance admin

    @us-wnr-02 @error @kpi @real-io @contract-shape:unbounded-preservation
    Scenario Outline: A blank or invisible-character workspace name is refused on the claim page
      When Priya claims the instance through link "claim-001" naming the workspace "<pasted>"
      Then the claim page is shown again saying "<refusal>"
      And nothing was created or changed
      And the bootstrap token "claim-001" remains unconsumed

      Examples:
        | pasted                   | refusal                                             |
        | [SPACE][SPACE][SPACE]    | Workspace name must not be empty                    |
        | Raman[NEWLINE]Household  | Workspace name must not contain control characters  |
        | Raman[NUL]Household      | Workspace name must not contain control characters  |

    @us-wnr-02 @edge @guard @real-io @contract-shape:bounded-change
    Scenario: Spaces around the workspace name are trimmed
      When Priya claims the instance through link "claim-001" naming the workspace "  Raman Household  "
      Then the response redirects the admin to the workspace dashboard
      And the workspace "Raman Household" exists with a first instance admin

    @us-wnr-02 @error @security @guard @real-io @contract-shape:unbounded-preservation
    Scenario: A dead link answers exactly as before, whatever the name
      Given the admin has already claimed the workspace using "claim-001"
      And the bootstrap token "stale-002" was minted 31 minutes ago with a 30-minute TTL
      When a visitor posts claims with unfit workspace names through the links "claim-001", "stale-002" and "never-issued-003"
      Then each link answers every unfit name byte-identically to its answer for an acceptable name
      And nothing was created or changed

  # ======================================================================= US-WNR-03
  Rule: Provisioning from the dashboard refuses a name the sidebar cannot hold, and creates nothing

    Background:
      Given Priya is the instance super-admin
      And workspace "Canzan Labs" has a team "Backend" with projects "Auth v2" (AUTH) and "Sandbox" (SBX)

    @us-wnr-03 @driving_port @edge @guard @real-io @contract-shape:bounded-change
    Scenario: A name of exactly 24 characters is provisioned
      When Priya provisions workspace "Canzan Labs Platform Ops" for first admin "dana@canzan.net" from the dashboard
      Then the dashboard confirms workspace "Canzan Labs Platform Ops" was provisioned for first admin "dana@canzan.net"

    @us-wnr-03 @driving_port @error @real-io @contract-shape:unbounded-preservation
    Scenario: A name past 24 characters is refused, kept in the form, and nothing is created
      When Priya provisions workspace "Canzan Labs Platform Engineering" for first admin "dana@canzan.net" from the dashboard
      Then the dashboard refuses the provision saying "Workspace name must be at most 24 characters"
      And the provision form still holds the name "Canzan Labs Platform Engineering" and the first admin "dana@canzan.net"
      And nothing was created or changed

    @us-wnr-03 @error @kpi @real-io @contract-shape:unbounded-preservation
    Scenario Outline: A blank or invisible-character name is refused and nothing is created
      When Priya provisions workspace "<pasted>" for first admin "dana@canzan.net" from the dashboard
      Then the dashboard refuses the provision saying "<refusal>"
      And nothing was created or changed

      Examples:
        | pasted                    | refusal                                             |
        | [SPACE][SPACE][SPACE]     | Workspace name must not be empty                    |
        | Canzan Labs Platform Team | Workspace name must be at most 24 characters        |
        | Globex[U+202E]            | Workspace name must not contain control characters  |
        | Globex[NUL]Labs           | Workspace name must not contain control characters  |

    @us-wnr-03 @driving_port @real-io @contract-shape:bounded-change
    Scenario: The corrected name provisions with the same first-admin email
      Given Priya's dashboard provision of "Canzan Labs Platform Engineering" for first admin "dana@canzan.net" was refused for its name
      When Priya provisions workspace "Canzan Platform Eng" for first admin "dana@canzan.net" from the dashboard
      Then the dashboard confirms workspace "Canzan Platform Eng" was provisioned for first admin "dana@canzan.net"
      And the instance dashboard lists workspace "Canzan Platform Eng"

    @us-wnr-03 @edge @guard @real-io @contract-shape:bounded-change
    Scenario: Spaces around the name are trimmed before it is stored and shown
      When Priya provisions workspace "  Globex  " for first admin "dana@canzan.net" from the dashboard
      Then the dashboard confirms workspace "Globex" was provisioned for first admin "dana@canzan.net"

    @us-wnr-03 @error @security @guard @real-io @contract-shape:unbounded-preservation
    Scenario: A non-admin is refused before the name is looked at
      Given Marco is a signed-in member who is not an instance admin
      When Marco posts a dashboard provision of a 40-character workspace name
      Then the answer is byte-identical to a never-existed address
      And nothing was created or changed

    @us-wnr-03 @needs-browser @error @real-io @contract-shape:unbounded-preservation
    Scenario: The refusal is shown in the Provision form on the real page, ready to correct
      Given Priya has the instance dashboard open in her browser
      When she provisions workspace "Canzan Labs Platform Engineering" for first admin "dana@canzan.net" in her browser
      Then "Workspace name must be at most 24 characters" appears in the Provision form in her browser
      And the Provision form in her browser still holds the name "Canzan Labs Platform Engineering" and the first admin "dana@canzan.net"

  # ======================================================================= US-WNR-04
  Rule: The CLI refuses an unfit name before touching the database

    Background:
      Given Priya is the instance super-admin
      And workspace "Canzan Labs" has a team "Backend" with projects "Auth v2" (AUTH) and "Sandbox" (SBX)

    @us-wnr-04 @driving_port @error @real-io @pending @contract-shape:unbounded-preservation
    Scenario Outline: A name the rule refuses exits 2 with the reason and creates nothing
      When Priya runs the provisioning command naming the workspace "<pasted>" for first admin "dana@canzan.net"
      Then the command exits 2 saying "<refusal>" with nothing on standard output
      And nothing was created or changed

      Examples:
        | pasted                           | refusal                                             |
        | Canzan Labs Platform Engineering | Workspace name must be at most 24 characters        |
        | [SPACE][SPACE][SPACE]            | Workspace name must not be empty                    |
        |                                  | Workspace name must not be empty                    |
        | Globex[U+202E]                   | Workspace name must not contain control characters  |

    @us-wnr-04 @error @security @kpi @real-io @pending @contract-shape:unbounded-preservation
    Scenario: A name with a line break cannot forge output lines
      When Priya runs the provisioning command naming the workspace "Globex[NEWLINE]status: refused" for first admin "dana@canzan.net"
      Then the command exits 2 saying "Workspace name must not contain control characters" with nothing on standard output
      And neither output carries the forged line "status: refused"
      And nothing was created or changed

    @us-wnr-04 @edge @real-io @pending @contract-shape:bounded-change
    Scenario: Spaces around the name are trimmed in storage and in the output
      When Priya runs the provisioning command naming the workspace "  Globex  " for first admin "dana@canzan.net"
      Then the command exits 0 and reports "workspace-name: Globex"
      And the provisioned workspace is stored as "Globex"

    @us-wnr-04 @error @real-io @pending @contract-shape:unbounded-preservation
    Scenario Outline: A name mistake is caught without a database
      Given <situation>
      When Priya runs the provisioning command naming the workspace "Canzan Labs Platform Engineering" for first admin "dana@canzan.net"
      Then the command exits 2 saying "Workspace name must be at most 24 characters" with nothing on standard output

      Examples:
        | situation                                       |
        | no database is configured for the command       |
        | the command's database cannot be reached        |

    @us-wnr-04 @error @guard @real-io @pending @contract-shape:unbounded-preservation
    Scenario: Leaving the name out entirely still gets the usage line
      When Priya runs the provisioning command without a workspace name for first admin "dana@canzan.net"
      Then the command exits 2 with its usage line and no name-rule message
      And nothing was created or changed

  # ========================================================== KPI-2 — one rule, four doors
  Rule: The same name gets the same verdict and the same words at every door

    Background:
      Given Priya is the instance super-admin
      And workspace "Canzan Labs" has a team "Backend" with projects "Auth v2" (AUTH) and "Sandbox" (SBX)
      And workspace "Household" exists with no projects

    @us-wnr-01 @us-wnr-02 @us-wnr-03 @us-wnr-04 @kpi @error @real-io @pending @contract-shape:unbounded-preservation
    Scenario Outline: Every door refuses the same name in the same words
      When Priya offers the workspace name "<pasted>" at every door
      Then every door refuses it saying "<refusal>"
      And nothing was created or changed

      Examples:
        | pasted                               | refusal                                             |
        |                                      | Workspace name must not be empty                    |
        | [SPACE][SPACE][SPACE]                | Workspace name must not be empty                    |
        | Canzan Labs Platform Team            | Workspace name must be at most 24 characters        |
        | Ångström Øresund Sociétés            | Workspace name must be at most 24 characters        |
        | House[TAB]hold                       | Workspace name must not contain control characters  |
        | Ops[U+202E]gnikcatS                  | Workspace name must not contain control characters  |
        | Ops[U+2028]Platform                  | Workspace name must not contain control characters  |
        | Canzan Labs Platform[TAB]Engineering | Workspace name must not contain control characters  |

    @us-wnr-01 @us-wnr-02 @us-wnr-03 @us-wnr-04 @kpi @edge @real-io @pending @contract-shape:bounded-change
    Scenario Outline: Every door accepts the same name and stores it the same way
      When Priya offers the workspace name "<pasted>" at every door
      Then every door accepts it and stores "<stored>"

      Examples:
        | pasted                   | stored                   |
        | Canzan Labs Platform Ops | Canzan Labs Platform Ops |
        | Ångström Øresund Société | Ångström Øresund Société |
        | [TAB]Kitchen[NEWLINE]    | Kitchen                  |
        | 👨[ZWJ]👩[ZWJ]👧 Bailey   | 👨[ZWJ]👩[ZWJ]👧 Bailey   |
        | Ops[NBSP]Team            | Ops[NBSP]Team            |

    @us-wnr-01 @us-wnr-02 @us-wnr-03 @kpi @error @real-io @pending @contract-shape:unbounded-preservation
    Scenario: A null character is refused in the same words at every web door, never as an internal error
      When Priya offers the workspace name "Bailey[NUL]Family" at every web door
      Then every door refuses it saying "Workspace name must not contain control characters"
      And nothing was created or changed
