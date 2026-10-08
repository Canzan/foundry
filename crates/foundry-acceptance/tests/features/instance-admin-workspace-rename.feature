# Feature: instance-admin-workspace-rename — the instance super-admin corrects a
# workspace's display name from the instance dashboard, the name every member
# reads at the top of the sidebar, and every effective correction leaves one
# append-only record of who renamed it, from what, to what, and when.
#
# DISTILL 2026-10-05: every scenario is @pending (the repo convention: pending
# scenarios are excluded from every lane; DELIVER removes the tag as each one
# lands). The gate run that classified each one RED for the right reason is in
# docs/feature/instance-admin-workspace-rename/feature-delta.md (## Wave: DISTILL).
#
# Display name only (D8): the workspace id, its memberships and every URL are
# unchanged by a rename; the sidebar monogram follows the name. No uniqueness rule
# (D7). The rule is: trim, non-empty, at most 24 Unicode scalars (D3); a trimmed
# value equal to the stored name is a quiet success that records nothing (D4) —
# and that no-op check runs BEFORE the length rule (DDD-3), so an over-long legacy
# name resubmitted untouched is a quiet success, not a refusal.
#
# The record oracle (D5, DDD-5): the rename record is read back from the store, one
# entry per effective rename, none for a no-op or a refusal. It is deliberately NOT
# part of the per-workspace backup (ADR-WORKSPACE-RENAME-002): a backup taken after
# a rename still holds exactly the ten workspace tables and still verifies.
#
# Authorization mirrors the shipped instance-admin surface (D1): signed-out and
# non-super-admin callers, and garbled or unknown workspace ids, all get the SAME
# uniform non-enumerable answer a never-existed address returns. A rename without
# the dashboard's matching token is refused by the middleware before the handler.
#
# The HTTP lane is byte-blind to the htmx swap, to form-errors.js routing a refusal
# into the row's message area, and to how the sidebar lays out; those are asserted
# by @needs-browser scenarios in a real headless Chrome (the precedent's lesson 1:
# a swap that consumes the message area shows one refusal and then goes silent).
#
# Grounding SSOT: docs/feature/instance-admin-workspace-rename/feature-delta.md
# (DISCUSS D1-D10 + US-IAWR-01; DESIGN DDD-1..12, OQ-D1..D5);
# ADR-WORKSPACE-RENAME-001/002. Step vocabulary shared with
# instance-admin-project-rename.feature where the meaning is identical (Background,
# Marco, the never-existed answer, the dashboard in the browser).
#
# Harness: the SAME in-process axum router + real session/CSRF layers + real
# Postgres (shared testcontainer, per-scenario schema) every instance-admin
# scenario uses; the backup scenario runs the real `foundry doctor` CLI. No fake:
# every port here is driving (HTTP, CLI) or driven-internal (Postgres).

@iawr
Feature: Correcting a stale workspace name from the instance dashboard, with the change on record

  Background:
    Given Priya is the instance super-admin
    And workspace "Canzan Labs" has a team "Backend" with projects "Auth v2" (AUTH) and "Sandbox" (SBX)
    And workspace "Bailey Family" exists with no projects
    And Dana is a member of workspace "Bailey Family"

  # ----------------------------------------------- correcting the name (happy path)

  @us-iawr-01 @driving_port @real-io @contract-shape:bounded-change
  Scenario: A stale workspace name is corrected from the dashboard
    When Priya renames workspace "Bailey Family" to "Household"
    Then the workspace row she gets back shows "Household"
    And reopening the instance dashboard shows workspace "Household" and no longer "Bailey Family"
    And no other workspace's name changed

  @us-iawr-01 @driving_port @real-io @contract-shape:pure-function
  Scenario: Members read the new name in their sidebar on their next page
    Given Priya has renamed workspace "Bailey Family" to "Household"
    When Dana opens a page in her workspace
    Then Dana's sidebar shows the workspace "Household" with monogram "H"
    And the workspace name's hover title reads "Household"
    And Dana is still a member of the same workspace

  @us-iawr-01 @driving_port @real-io @kpi @contract-shape:bounded-change
  Scenario: Every rename is on record with who, what, and when
    When Priya renames workspace "Bailey Family" to "Household"
    Then workspace "Household" has exactly 1 rename on record
    And the latest rename on record for workspace "Household" names Priya, from "Bailey Family" to "Household", at the time of the rename

  @us-iawr-01 @edge @real-io @contract-shape:bounded-change
  Scenario: Changing only the letter case is a real rename and goes on record
    Given Priya has renamed workspace "Bailey Family" to "Household"
    When Priya renames workspace "Household" to "household"
    Then the workspace row she gets back shows "household"
    And workspace "household" has exactly 2 renames on record
    And the latest rename on record for workspace "household" names Priya, from "Household" to "household", at the time of the rename

  @us-iawr-01 @edge @real-io @contract-shape:unbounded-preservation
  Scenario: Renaming a workspace to its current name is a quiet success
    Given Priya has renamed workspace "Bailey Family" to "Household"
    When Priya renames workspace "Household" to " Household "
    Then the workspace row she gets back shows "Household" and carries no error
    And no workspace changed and nothing new went on record

  @us-iawr-01 @edge @real-io @contract-shape:unbounded-preservation
  Scenario: An over-long name from before the limit can be left as it is
    Given workspace "Canzan Labs Platform Engineering and Site Reliability" was named before the rule existed
    When Priya renames workspace "Canzan Labs Platform Engineering and Site Reliability" to "Canzan Labs Platform Engineering and Site Reliability"
    Then the workspace row she gets back shows "Canzan Labs Platform Engineering and Site Reliability" and carries no error
    And no workspace changed and nothing new went on record

  @us-iawr-01 @edge @real-io @contract-shape:bounded-change
  Scenario: Two workspaces may share a name
    When Priya renames workspace "Bailey Family" to "Canzan Labs"
    Then the workspace row she gets back shows "Canzan Labs"
    And workspace "Canzan Labs" has exactly 1 rename on record

  # ------------------------------------------------ the 24-character rule (D3)

  @us-iawr-01 @edge @real-io @contract-shape:bounded-change
  Scenario Outline: A name of up to 24 characters is accepted
    When Priya renames workspace "Bailey Family" to "<submitted>"
    Then the workspace row she gets back shows "<stored>"
    And workspace "<stored>" has exactly 1 rename on record

    Examples:
      | submitted                    | stored                   |
      | Canzan Labs Platform Ops     | Canzan Labs Platform Ops |
      | Bailey Family Workspace      | Bailey Family Workspace  |
      | Ångström Øresund Société     | Ångström Øresund Société |

  @us-iawr-01 @edge @real-io @contract-shape:bounded-change
  Scenario: Spaces around a name are trimmed before the limit is counted
    When Priya renames workspace "Bailey Family" to "  Canzan Labs Platform Ops  "
    Then the workspace row she gets back shows "Canzan Labs Platform Ops"
    And the latest rename on record for workspace "Canzan Labs Platform Ops" names Priya, from "Bailey Family" to "Canzan Labs Platform Ops", at the time of the rename

  @us-iawr-01 @error @real-io @contract-shape:unbounded-preservation
  Scenario Outline: A name past 24 characters is refused with the limit stated
    When Priya renames workspace "Bailey Family" to "<submitted>"
    Then the workspace rename is refused saying "Workspace name must be at most 24 characters"
    And workspace "Bailey Family" is unchanged with no rename on record

    Examples:
      | submitted                 |
      | Canzan Labs Platform Team |
      | Ångström Øresund Sociétés |

  @us-iawr-01 @error @real-io @contract-shape:unbounded-preservation
  Scenario: An empty name is refused with the reason stated
    When Priya renames workspace "Bailey Family" to ""
    Then the workspace rename is refused saying "Workspace name must not be empty"
    And workspace "Bailey Family" is unchanged with no rename on record

  @us-iawr-01 @error @edge @real-io @contract-shape:unbounded-preservation
  Scenario: A name of only spaces counts as empty
    When Priya renames workspace "Bailey Family" to "   "
    Then the workspace rename is refused saying "Workspace name must not be empty"
    And workspace "Bailey Family" is unchanged with no rename on record

  @us-iawr-01 @error @security @real-io @contract-shape:bounded-change
  Scenario: A name with markup characters is shown exactly as typed
    When Priya renames workspace "Bailey Family" to "<b>Ops</b> & 'Co'"
    Then the workspace row she gets back shows "<b>Ops</b> & 'Co'"
    And Dana's sidebar shows the workspace "<b>Ops</b> & 'Co'" with monogram "<"
    And the workspace name's hover title reads "<b>Ops</b> & 'Co'"

  # --------------------------------------------------------- who may rename (D1)

  @us-iawr-01 @error @security @real-io @contract-shape:unbounded-preservation
  Scenario: Only the instance admin can rename a workspace
    Given Marco is a signed-in member who is not an instance admin
    When Marco sends the rename for workspace "Bailey Family" to "Household"
    Then the answer is byte-identical to a never-existed address
    And workspace "Bailey Family" is unchanged with no rename on record

  @us-iawr-01 @error @security @real-io @contract-shape:unbounded-preservation
  Scenario: A signed-out visitor cannot rename a workspace
    When a signed-out visitor sends a rename for workspace "Bailey Family"
    Then the answer is byte-identical to a never-existed address
    And workspace "Bailey Family" is unchanged with no rename on record

  @us-iawr-01 @error @security @real-io @contract-shape:unbounded-preservation
  Scenario: A workspace rename that does not carry the dashboard's matching token is refused
    When a rename for workspace "Bailey Family" is submitted without the dashboard's matching token
    Then the workspace rename is refused before any change is made
    And workspace "Bailey Family" is unchanged with no rename on record

  @us-iawr-01 @error @security @real-io @contract-shape:unbounded-preservation
  Scenario: A workspace rename aimed at a garbled workspace id is answered like a missing page
    When Priya sends a workspace rename aimed at the workspace id "not-a-uuid"
    Then the answer is byte-identical to a never-existed address
    And no workspace changed and nothing new went on record

  @us-iawr-01 @error @security @real-io @contract-shape:unbounded-preservation
  Scenario: A workspace rename aimed at a workspace that does not exist is answered like a missing page
    When Priya sends a workspace rename aimed at a workspace id that matches nothing
    Then the answer is byte-identical to a never-existed address
    And no workspace changed and nothing new went on record

  # ------------------------------------------- what a rename must leave alone (guards)

  @us-iawr-01 @guard @real-io @contract-shape:unbounded-preservation
  Scenario: Renaming a workspace leaves the projects listed under it exactly as they were
    Given workspace "Bailey Family" has a team "Home" with project "Chores" (CHR)
    And Priya has noted the project rows listed under workspace "Canzan Labs"
    And Priya has renamed workspace "Canzan Labs" to "Canzan Platform"
    When Priya opens the instance dashboard
    Then she sees "Auth v2" and "Sandbox" under "Canzan Platform" and "Chores" under "Bailey Family"
    And the project rows listed under workspace "Canzan Platform" are byte-identical to before

  @us-iawr-01 @guard @real-io @contract-shape:unbounded-preservation
  Scenario: A workspace backup taken after a rename carries the new name and no rename record
    Given Priya has renamed workspace "Bailey Family" to "Household"
    When Priya exports workspace "Household" to a backup
    Then the backup holds exactly the ten workspace tables and no rename record
    And the backup declares the workspace as "Household"
    And the backup passes verification

  # --------------------------------------------------- @needs-browser — the DOM oracle
  # The HTTP lane cannot see the htmx swap, the routing of a refusal into the row's
  # message area, or the sidebar's layout. These drive a REAL headless Chrome
  # against the same in-process origin.

  @us-iawr-01 @needs-browser @driving_port @real-io @contract-shape:bounded-change
  Scenario: The workspace row updates in place when the rename succeeds
    Given Priya has the instance dashboard open in her browser
    When she renames the "Bailey Family" workspace to "Household" in her browser
    Then that workspace's row shows "Household" without the page reloading
    And the projects listed under "Canzan Labs" are still on the page

  @us-iawr-01 @needs-browser @error @real-io @contract-shape:unbounded-preservation
  Scenario: Refused workspace renames explain themselves inside the row, every time
    Given Priya has the instance dashboard open in her browser
    When she blanks the "Bailey Family" workspace name in her browser
    Then "Workspace name must not be empty" appears inside that workspace row's message area
    When she renames the "Bailey Family" workspace to "Canzan Labs Platform Team" in her browser
    Then "Workspace name must be at most 24 characters" appears inside that workspace row's message area
    And the workspace rename form is still there for her to correct
    When she renames the "Bailey Family" workspace to "Household" in her browser
    Then that workspace's row shows "Household" without the page reloading

  @us-iawr-01 @needs-browser @edge @kpi @real-io @contract-shape:pure-function
  Scenario Outline: A long workspace name stays on one line in the sidebar
    Given workspace "Canzan Labs Platform Engineering and Site Reliability" was named before the rule existed
    And Lena is a member of workspace "Canzan Labs Platform Engineering and Site Reliability"
    When Lena opens a page in her workspace on a <screen> screen
    Then the sidebar shows the workspace name on one line ending in an ellipsis
    And hovering the workspace name shows "Canzan Labs Platform Engineering and Site Reliability"
    And the page does not scroll sideways

    Examples:
      | screen      |
      | desktop     |
      | phone-sized |
