# Feature: project-name-rule — both doors that name a project (the team member's
# create form and the instance admin's rename) refuse the same names for the same
# reasons, in the same words, and every new project gets a board you can open.
#
# DISTILL 2026-10-06: every scenario is @pending (the repo convention: pending
# scenarios are excluded from every lane; DELIVER removes the tag as each one lands,
# slice by slice). The gate run that classified each one RED for the right reason is
# in docs/feature/project-name-rule/feature-delta.md (## Wave: DISTILL).
#
# The rule (D2, D3, D5): trim, then (rename only) the no-op, then empty, then control
# characters, then at most 256 Unicode scalars, then unique within the team. The
# trimmed value is what is stored. The refused set (D3) is exactly the workspace
# rule's: Unicode Cc, the bidi controls U+202A-202E and U+2066-2069, and U+2028/2029.
# Every other format character stays allowed (zero-width joiner, zero-width
# non-joiner, no-break space ...). "Unique within the team" (D6): no sibling has the
# same name in any letter case, and no sibling's address equals the name's derived
# address — unless the name derives no address at all. On create, the name is
# checked before the key prefix (D5). The copy (D4) is byte-identical at both doors:
#   "Project name must not be empty"
#   "Project name must not contain control characters"
#   "Project name must be at most 256 characters"
#   "Project name must be unique within the team"
#
# Addresses (D15). A project's address is minted once, at create: the name's own
# address when it has an ASCII letter or digit ("Homelab Ops" -> homelab-ops),
# otherwise the key prefix in lower case ("日本語ボード" with JP -> jp), taking the
# lowest free -2, -3 ... when the team already holds it. A rename never moves it.
#
# INVISIBLE CHARACTERS AND LONG NAMES IN THIS FILE. A name in quotes is exactly what
# is typed or pasted, except for these bracketed marks:
#   [TAB] tab, [NEWLINE] line feed, [NUL] the null character, [SPACE] a space (for
#   example tables, which drop edge spaces), [NBSP] no-break space U+00A0,
#   [ZWJ] zero-width joiner U+200D, [ZWNJ] zero-width non-joiner U+200C,
#   [U+XXXX] the character with that code point (e.g. [U+202E], right-to-left
#   override), and [N×c] the single character c written N times (e.g. [257×a] is
#   257 letters a; [256×日] is 256 CJK characters, three bytes each). Any other
#   bracketed text is literal.
#
# Oracles. Nothing is left behind by a refusal (D9): the steps capture every project
# (team, name, address, key prefix, issue counter) and the lanes of every project
# just before the attempt, and assert none of it moved. A success adds exactly one
# project with its lanes and changes nothing else. A rename changes only that
# project's name. Addresses asserted after a rename come from the database before
# the rename, never re-derived from a name (ADR-PROJECT-RENAME-001). Authz answers
# keep their precedence and stay byte-identical (D10). A NUL never becomes an
# internal error (KPI-3). Both doors give the same verdict and the same words for
# the same input (KPI-2, the parity outlines; input set per DESIGN OQ-D1).
#
# Harness: the SAME in-process axum router + real session/CSRF layers + real Postgres
# (shared testcontainer, per-scenario schema) every instance-admin and project
# scenario uses; @needs-browser scenarios drive a real headless Chrome. No fake:
# every port here is driving (HTTP) or driven-internal (Postgres).
#
# Shared vocabulary (cucumber steps are global): Priya, Marco, the Backend team with
# "Auth v2" (AUTH) and "Sandbox" (SBX), the never-existed answer, the rename refusal,
# and the dashboard in the browser are the iapr steps. Priya is the instance
# super-admin AND a member of every team the scenarios seed; Marco is a member of
# workspace "Canzan Labs" who is on no team and is not an instance admin.

@pnr
Feature: One rule for naming a project, at both doors

  # ======================================================================= US-PNR-01
  Rule: A pasted project name carrying invisible characters is refused at rename

    Background:
      Given Priya is the instance super-admin
      And workspace "Canzan Labs" has a team "Backend" with projects "Auth v2" (AUTH) and "Sandbox" (SBX)

    @us-pnr-01 @error @real-io @contract-shape:unbounded-preservation
    Scenario Outline: A project name with an invisible character inside it is refused inside the row
      When Priya renames project "Sandbox" to the pasted name "<pasted>"
      Then the rename is refused saying "Project name must not contain control characters"
      And no project changed and nothing was created

      Examples:
        | pasted               |
        | Sand[TAB]box         |
        | Sand[U+007F]box      |
        | Ops[U+202E]spoH      |
        | Ops[U+2066]Stacking  |
        | Reading[U+2028]List  |

    @us-pnr-01 @error @kpi @real-io @contract-shape:unbounded-preservation
    Scenario: A null character in the new name is refused with the reason, not an internal error
      When Priya renames project "Sandbox" to the pasted name "Identity[NUL]Platform"
      Then the rename is refused saying "Project name must not contain control characters"
      And no project changed and nothing was created

    @us-pnr-01 @edge @guard @real-io @contract-shape:bounded-change
    Scenario Outline: Joined, accented, long and padded names are accepted and the board stays where it was
      When Priya renames project "Sandbox" to the pasted name "<pasted>"
      Then project "Sandbox" is now named "<stored>", and its board still opens at its original address

      Examples:
        | pasted                            | stored                       |
        | Café Roadmap 👨[ZWJ]👩[ZWJ]👧      | Café Roadmap 👨[ZWJ]👩[ZWJ]👧 |
        | Mehr[ZWNJ]dad Board               | Mehr[ZWNJ]dad Board          |
        | [TAB]Sandbox Experiments[NEWLINE] | Sandbox Experiments          |
        | [256×日]                          | [256×日]                     |

    @us-pnr-01 @error @real-io @contract-shape:unbounded-preservation
    Scenario Outline: The reason given is the first one that applies
      When Priya renames project "Sandbox" to the pasted name "<pasted>"
      Then the rename is refused saying "<refusal>"
      And no project changed and nothing was created

      Examples:
        | pasted              | refusal                                           |
        | [150×a][TAB][150×b] | Project name must not contain control characters  |
        | Auth[TAB]V2         | Project name must not contain control characters  |
        | [SPACE][TAB][SPACE] | Project name must not be empty                    |
        | [257×a]             | Project name must be at most 256 characters       |
        | [257×日]            | Project name must be at most 256 characters       |
        | auth V2             | Project name must be unique within the team       |

    @us-pnr-01 @edge @guard @real-io @contract-shape:unbounded-preservation
    Scenario Outline: An untouched name from before the rule can be left as it is
      Given project "<legacy>" (<key>) was named before the rule existed
      When Priya renames project "<legacy>" to the pasted name "<legacy>"
      Then the row she gets back still shows "<legacy>" and carries no error
      And no project changed and nothing was created

      Examples:
        | legacy          | key |
        | Homelab[TAB]Ops | HLO |
        | Ops[U+202E]spoH | OPR |
        | [300×x]         | LNG |
        | sandbox         | SBL |

    @us-pnr-01 @error @security @guard @real-io @contract-shape:unbounded-preservation
    Scenario: A rename aimed at a project that does not exist is answered like a missing page, whatever the name
      When Priya sends a rename with the pasted name "Sand[TAB]box" aimed at a project id that matches nothing
      Then the answer is byte-identical to a never-existed address
      And no project changed and nothing was created

    @us-pnr-01 @error @security @guard @real-io @contract-shape:unbounded-preservation
    Scenario: A non-admin's rename is answered like a missing page before the name is looked at
      Given Marco is a signed-in member who is not an instance admin
      When Marco sends the rename for "Sandbox" with the pasted name "Sand[TAB]box"
      Then the answer is byte-identical to a never-existed address
      And no project changed and nothing was created

    @us-pnr-01 @needs-browser @error @real-io @contract-shape:unbounded-preservation
    Scenario: A pasted tab is explained inside the row on the real page
      Given Priya has the instance dashboard open in her browser
      When she pastes "Auth[TAB]Platform" over the "Auth v2" project name in her browser and submits it
      Then "Project name must not contain control characters" appears inside that row's message area
      And the rename form is still there for her to correct

  # ======================================================================= US-PNR-02
  Rule: Creating a project refuses a name the rename door would refuse, and creates nothing

    Background:
      Given Priya is the instance super-admin
      And workspace "Canzan Labs" has a team "Backend" with projects "Auth v2" (AUTH) and "Sandbox" (SBX)

    @us-pnr-02 @driving_port @edge @guard @real-io @contract-shape:bounded-change
    Scenario Outline: A name of up to 256 characters creates the project and opens its board
      When Priya creates a project named "<pasted>" with key prefix "OPS"
      Then she lands on the new project's board, headed "<stored>"

      Examples:
        | pasted               | stored     |
        | [256×a]              | [256×a]    |
        | A[255×日]            | A[255×日]  |
        | [SPACE][256×a][TAB]  | [256×a]    |

    @us-pnr-02 @driving_port @error @real-io @contract-shape:unbounded-preservation
    Scenario Outline: A name past 256 characters is refused, kept in the form, and nothing is created
      When Priya creates a project named "<pasted>" with key prefix "OPS"
      Then the create form is shown again saying "Project name must be at most 256 characters"
      And the create form still holds the name "<kept>" and the key prefix "OPS"
      And no project changed and nothing was created

      Examples:
        | pasted                 | kept       |
        | [257×a]                | [257×a]    |
        | A[256×日]              | A[256×日]  |
        | [SPACE][257×a][SPACE]  | [257×a]    |

    @us-pnr-02 @error @kpi @real-io @contract-shape:unbounded-preservation
    Scenario Outline: A blank or invisible-character name is refused and nothing is created
      When Priya creates a project named "<pasted>" with key prefix "OPS"
      Then the create form is shown again saying "<refusal>"
      And no project changed and nothing was created

      Examples:
        | pasted                | refusal                                           |
        | Homelab[TAB]Ops       | Project name must not contain control characters  |
        | Reading[U+2028]List   | Project name must not contain control characters  |
        | Ops[U+202E]spoH       | Project name must not contain control characters  |
        | Homelab[NUL]Ops       | Project name must not contain control characters  |
        | [SPACE][SPACE][SPACE] | Project name must not be empty                    |

    @us-pnr-02 @error @real-io @contract-shape:unbounded-preservation
    Scenario: A refusal sent from the page without reloading it comes back as the bare message
      When Priya creates a project named "[257×a]" with key prefix "OPS" from the page without reloading it
      Then the create refusal comes back as the bare message "Project name must be at most 256 characters"
      And no project changed and nothing was created

    @us-pnr-02 @driving_port @real-io @contract-shape:bounded-change
    Scenario: The corrected name succeeds with the same key prefix
      Given Priya's create of "[257×a]" with key prefix "OPS" was refused for its name
      When Priya creates a project named "Homelab Ops" with key prefix "OPS"
      Then she lands on the board at "/team/backend/project/homelab-ops" headed "Homelab Ops"

    @us-pnr-02 @error @security @guard @real-io @contract-shape:unbounded-preservation
    Scenario Outline: A caller who may not create in the team gets today's answer, whatever the name
      Given Marco is a signed-in member who is not an instance admin
      When <caller> sends creates to team "<team>" with unfit names and with an acceptable name
      Then each answer is byte-identical to the answer for the acceptable name
      And no project changed and nothing was created

      Examples:
        | caller               | team     |
        | Marco                | Backend  |
        | a signed-out visitor | Backend  |
        | Priya                | Research |

    @us-pnr-02 @error @real-io @contract-shape:unbounded-preservation
    Scenario Outline: A name problem is reported before a key problem
      When Priya creates a project named "<pasted>" with key prefix "<key>"
      Then the create form is shown again saying "<refusal>"
      And no project changed and nothing was created

      Examples:
        | pasted                | key | refusal                                           |
        | [300×a]               | ops | Project name must be at most 256 characters       |
        | Home[TAB]lab          | ops | Project name must not contain control characters  |
        | Sandbox               | ops | Project name must be unique within the team       |
        | [SPACE]               | ops | Project name must not be empty                    |

    @us-pnr-02 @needs-browser @error @real-io @contract-shape:unbounded-preservation
    Scenario: The refusal is shown in the create form on the real page, ready to correct
      Given Priya has the new-project form for team "Backend" open in her browser
      When she types the name "[257×a]" and the key prefix "OPS" into the form in her browser and submits it
      Then "Project name must be at most 256 characters" appears in the create form in her browser
      And the create form in her browser still holds the name "[257×a]" and the key prefix "OPS"

  # ======================================================================= US-PNR-03
  Rule: Creating a project refuses a name the team already uses, whatever its address

    Background:
      Given Priya is the instance super-admin
      And workspace "Canzan Labs" has a team "Backend" with projects "Auth v2" (AUTH) and "Sandbox" (SBX)
      And Priya has renamed project "Auth v2" to "Identity Platform"

    @us-pnr-03 @driving_port @error @real-io @contract-shape:unbounded-preservation
    Scenario: A name the team already uses under a different address is refused
      When Priya creates a project named "identity platform" with key prefix "IDP"
      Then the create form is shown again saying "Project name must be unique within the team"
      And no project changed and nothing was created
      And team "Backend" has exactly one project named "Identity Platform" in any letter case

    @us-pnr-03 @error @guard @real-io @contract-shape:unbounded-preservation
    Scenario: A name whose address the team already uses is still refused
      When Priya creates a project named "Auth V2!" with key prefix "AV2"
      Then the create form is shown again saying "Project name must be unique within the team"
      And no project changed and nothing was created

    @us-pnr-03 @edge @guard @real-io @contract-shape:bounded-change
    Scenario: The same name in another team is accepted
      Given workspace "Canzan Labs" also has a team "Frontend" with no projects
      When Priya creates a project named "Identity Platform" with key prefix "IDF" in team "Frontend"
      Then she lands on the board at "/team/frontend/project/identity-platform" headed "Identity Platform"

    @us-pnr-03 @error @real-io @contract-shape:unbounded-preservation
    Scenario: A long name that repeats a sibling's is refused for its length first
      Given project "[300×a]" (LNG) was named before the rule existed
      When Priya creates a project named "[300×A]" with key prefix "LNH"
      Then the create form is shown again saying "Project name must be at most 256 characters"
      And no project changed and nothing was created

    @us-pnr-03 @edge @real-io @contract-shape:bounded-change
    Scenario: A name with no address of its own is not a duplicate of an old project that has none either
      Given project "Ωμέγα" (OMG) was created before this fix and has no board address
      When Priya renames project "Sandbox" to the pasted name "🚀"
      Then project "Sandbox" is now named "🚀", and its board still opens at its original address

    @pending @us-pnr-01 @us-pnr-02 @us-pnr-03 @kpi @error @real-io @contract-shape:unbounded-preservation
    Scenario Outline: Both doors refuse the same name in the same words
      When Priya offers the project name "<pasted>" at both doors
      Then both doors refuse it saying "<refusal>"
      And no project changed and nothing was created

      Examples:
        | pasted                | refusal                                           |
        |                       | Project name must not be empty                    |
        | [SPACE][SPACE][SPACE] | Project name must not be empty                    |
        | [257×a]               | Project name must be at most 256 characters       |
        | A[256×日]             | Project name must be at most 256 characters       |
        | Sand[TAB]box          | Project name must not contain control characters  |
        | Ops[U+202E]spoH       | Project name must not contain control characters  |
        | Reading[U+2028]List   | Project name must not contain control characters  |
        | Homelab[NUL]Ops       | Project name must not contain control characters  |
        | [150×a][TAB][150×b]   | Project name must not contain control characters  |
        | identity platform     | Project name must be unique within the team       |
        | Auth V2!              | Project name must be unique within the team       |

    @pending @us-pnr-01 @us-pnr-02 @us-pnr-03 @kpi @edge @real-io @contract-shape:bounded-change
    Scenario Outline: Both doors accept the same name and store it the same way
      Given workspace "Canzan Labs" also has a team "Frontend" with no projects
      When Priya offers the project name "<pasted>" at both doors, creating it in team "Frontend"
      Then both doors accept it and store "<stored>"

      Examples:
        | pasted                         | stored                        |
        | [TAB]Kitchen Board[NEWLINE]    | Kitchen Board                 |
        | Café Roadmap 👨[ZWJ]👩[ZWJ]👧   | Café Roadmap 👨[ZWJ]👩[ZWJ]👧  |
        | [256×a]                        | [256×a]                       |
        | Ops[NBSP]Team                  | Ops[NBSP]Team                 |

  # ======================================================================= US-PNR-04
  Rule: A project named without Latin letters or digits gets a board you can open

    Background:
      Given Priya is the instance super-admin
      And workspace "Canzan Labs" has a team "Backend" with projects "Auth v2" (AUTH) and "Sandbox" (SBX)

    @pending @us-pnr-04 @driving_port @kpi @real-io @contract-shape:bounded-change
    Scenario: A project named in Japanese lands on its own board
      When Priya creates a project named "日本語ボード" with key prefix "JP"
      Then she lands on the board at "/team/backend/project/jp" headed "日本語ボード"
      And its change report opens at "/team/backend/project/jp/report" headed "日本語ボード"

    @pending @us-pnr-04 @driving_port @real-io @contract-shape:bounded-change
    Scenario: Two projects without Latin letters can live in one team
      Given Priya has created a project named "日本語ボード" with key prefix "JP"
      When Priya creates a project named "🚀" with key prefix "RKT"
      Then she lands on the board at "/team/backend/project/rkt" headed "🚀"

    @pending @us-pnr-04 @edge @real-io @contract-shape:bounded-change
    Scenario: The fallback address takes the next free number when its first choice is used
      Given Priya has created a project named "Ops" with key prefix "OPN"
      When Priya creates a project named "🛠" with key prefix "OPS"
      Then she lands on the board at "/team/backend/project/ops-2" headed "🛠"

    @pending @us-pnr-04 @edge @real-io @contract-shape:bounded-change
    Scenario Outline: The fallback address takes the lowest free number, never "-1"
      Given Priya has created a project named "<first>" with key prefix "<first_key>"
      And Priya has created a project named "<second>" with key prefix "<second_key>"
      When Priya creates a project named "日本語ボード" with key prefix "JP"
      Then she lands on the board at "<address>" headed "日本語ボード"

      Examples:
        | first | first_key | second | second_key | address                    |
        | JP    | JPX       | JP 2   | JPY        | /team/backend/project/jp-3 |
        | JP 2  | JPY       | Ops    | OPN        | /team/backend/project/jp   |

    @pending @us-pnr-04 @edge @guard @real-io @contract-shape:bounded-change
    Scenario Outline: Names with Latin letters or digits keep today's addresses
      When Priya creates a project named "<name>" with key prefix "<key>"
      Then she lands on the board at "<address>" headed "<name>"

      Examples:
        | name          | key | address                          |
        | Ωmega 2       | OMG | /team/backend/project/mega-2     |
        | Café Roadmap  | CAF | /team/backend/project/caf-roadmap |

    @pending @us-pnr-04 @real-io @contract-shape:bounded-change
    Scenario: Renaming such a project keeps its address
      Given Priya has created a project named "日本語ボード" with key prefix "JP"
      When Priya renames project "日本語ボード" to the pasted name "Japanese Board"
      Then project "日本語ボード" is now named "Japanese Board", and its board still opens at its original address
      And the board at "/team/backend/project/jp" is headed "Japanese Board"

    @pending @us-pnr-04 @edge @real-io @contract-shape:bounded-change
    Scenario: An old project with no address does not block a new one
      Given project "Ωμέγα" (OMG) was created before this fix and has no board address
      When Priya creates a project named "🚀" with key prefix "RKT"
      Then she lands on the board at "/team/backend/project/rkt" headed "🚀"

    @pending @us-pnr-04 @error @edge @real-io @contract-shape:unbounded-preservation
    Scenario: A name that spells a fallback address already taken is refused
      Given Priya has created a project named "日本語ボード" with key prefix "JP"
      When Priya creates a project named "JP" with key prefix "JPX"
      Then the create form is shown again saying "Project name must be unique within the team"
      And no project changed and nothing was created
