# Feature: instance-admin-project-rename — the instance super-admin sees every
# project in the instance and corrects a stale display name in place, without a
# production psql session.
#
# Rename changes the DISPLAY NAME ONLY (D1): the stored slug, every board/report
# address, the key prefix, and existing issue keys are byte-identical before and
# after. The load-bearing oracle is the BOARD-SURVIVAL scenario: today
# `build_board_page` re-derives slugs from NAMES at render time (D2 — the defect
# ADR-PROJECT-RENAME-001 fixes), so a naive rename would break every issue-card
# action while the board URL itself still resolved. Its steps capture the STORED
# slug from the database BEFORE the rename and never re-derive it from the new
# name — a test-local slugify(new_name) would assert the wrong URL and go green
# over the exact bug this feature exists to fix.
#
# Authorization mirrors the shipped instance-admin surface (D5): signed-out and
# non-super-admin callers — and garbled or unknown project ids — are all answered
# with the SAME uniform non-enumerable 404 a never-existed path returns. The
# rename form is an htmx mutating trigger and carries the double-submit `_csrf`
# field; a POST without a valid pair is refused by the middleware before the
# handler runs.
#
# Refused renames are 422 + a bare error fragment routed into the submitting
# row's `[data-error-slot]` by form-errors.js (D6). The HTTP lane asserts status
# + fragment bytes; the DOM swap itself is invisible to it (the form-errors RCA),
# so two @needs-browser scenarios use a real headless Chrome as the oracle for
# the in-place row swap and the inline error.
#
# DELIVER COMPLETE: every scenario has been un-pended, one at a time (the repo
# convention: pending scenarios are excluded from every lane; DELIVER removes
# the tag as each lands). The DISTILL-era production scaffolds
# (foundry-core::slugify, the four foundry-store queries,
# foundry_services::projects::rename_project, the mounted
# submit_project_rename handler) are all live production code now.
#
# Grounding SSOT: docs/feature/instance-admin-project-rename/feature-delta.md
# (DISCUSS D1-D7 + US-IAPR-01..03; DESIGN component-boundaries.md port
# signatures; ADR-PROJECT-RENAME-001/002).
#
# Harness: the SAME in-process axum router + real session/CSRF layers + real
# Postgres (shared testcontainer, per-scenario schema) every instance-admin
# scenario already uses. No new harness; no fake — every port here is driving
# (HTTP) or driven-internal (Postgres), so everything is real per the ATDD
# infrastructure policy.

@iapr
Feature: Correcting a stale project name from the instance dashboard

  Background:
    Given Priya is the instance super-admin
    And workspace "Canzan Labs" has a team "Backend" with projects "Auth v2" (AUTH) and "Sandbox" (SBX)
    And workspace "Bailey Family" exists with no projects

  # ------------------------------------------------ US-IAPR-01 seeing every project

  @us-iapr-01 @driving_port @real-io
  Scenario: Every project in the instance is listed under its workspace
    Given workspace "Bailey Family" has a team "Home" with project "Chores" (CHR)
    When Priya opens the instance dashboard
    Then she sees "Auth v2" and "Sandbox" under "Canzan Labs" and "Chores" under "Bailey Family"
    And each project row shows its name, key prefix, and owning team
    And within a workspace the projects are ordered by name

  @us-iapr-01 @edge
  Scenario: A workspace with no projects says so
    When Priya opens the instance dashboard
    Then the "Bailey Family" section says "No projects yet."

  @us-iapr-01 @error @security
  Scenario: The project list is invisible to anyone who is not the instance admin
    Given Marco is a signed-in member who is not an instance admin
    When Marco requests the instance dashboard
    Then the answer is byte-identical to a never-existed address
    And a signed-out visitor requesting the instance dashboard is answered identically

  # -------------------------------------- US-IAPR-02 renaming without breaking anything

  @us-iapr-02 @driving_port @real-io
  Scenario: A stale project name is corrected from the dashboard
    When Priya renames "Auth v2" to "Identity Platform"
    Then the row she gets back shows "Identity Platform" with key prefix "AUTH"
    And reopening the instance dashboard shows "Identity Platform" and no longer "Auth v2"

  @us-iapr-02 @driving_port @real-io
  Scenario: Boards, addresses, and issue keys survive a rename
    Given issue AUTH-7 "Refresh token rotation" exists on the "Auth v2" board
    And Priya has noted where the "Auth v2" board lives
    When Priya renames "Auth v2" to "Identity Platform"
    Then the board still answers at its old address, now titled "Identity Platform"
    And issue AUTH-7 keeps its key and its card actions still answer at the old address
    And the change report at the old address shows the new name
    And the project's stored address and key prefix are byte-identical to before

  @us-iapr-02 @edge
  Scenario: Renaming a project to its current name is a quiet success
    When Priya renames "Sandbox" to "Sandbox"
    Then the row she gets back shows "Sandbox" with key prefix "SBX" and carries no error
    And the stored project record is untouched

  @us-iapr-02 @error @security
  Scenario: Only the instance admin can rename a project
    Given Marco is a signed-in member who is not an instance admin
    When Marco sends the rename for "Auth v2" to "Marco's Project"
    Then the answer is byte-identical to a never-existed address
    And the project is still named "Auth v2" everywhere

  @us-iapr-02 @error @security
  Scenario: A rename that does not carry the dashboard's matching token is refused
    When a rename for "Auth v2" is submitted without the dashboard's matching token
    Then the rename is refused before any change is made
    And the project is still named "Auth v2" everywhere

  @us-iapr-02 @error @security
  Scenario: A signed-out visitor cannot rename anything
    When a signed-out visitor sends a rename for "Auth v2"
    Then the answer is byte-identical to a never-existed address
    And the project is still named "Auth v2" everywhere

  # -------------------------------------------- US-IAPR-03 refusals explain themselves

  @us-iapr-03 @error
  Scenario: An empty name is refused with the reason stated
    When Priya renames "Auth v2" to ""
    Then the rename is refused saying "Project name must not be empty"
    And the project is still named "Auth v2" everywhere

  @us-iapr-03 @error @edge
  Scenario: A name of only spaces counts as empty
    When Priya renames "Auth v2" to "   "
    Then the rename is refused saying "Project name must not be empty"

  @us-iapr-03 @error
  Scenario: A name past the length limit is refused with the limit stated
    When Priya renames "Auth v2" to a 300-character name
    Then the rename is refused saying "Project name must be at most 256 characters"
    And the project is still named "Auth v2" everywhere

  @us-iapr-03 @edge
  Scenario: A name of exactly the length limit is accepted
    When Priya renames "Auth v2" to a 256-character name
    Then the row she gets back shows that exact name

  @us-iapr-03 @error
  Scenario: A name another project in the team already uses is refused
    When Priya renames "Auth v2" to "Sandbox"
    Then the rename is refused saying "Project name must be unique within the team"
    And both projects keep their names

  @us-iapr-03 @error
  Scenario: Changing the letter case does not dodge the uniqueness rule
    When Priya renames "Auth v2" to "sandbox"
    Then the rename is refused saying "Project name must be unique within the team"
    And both projects keep their names

  @us-iapr-03 @error
  Scenario: A name that collides with another project's address is refused
    When Priya renames "Auth v2" to "Sandbox!"
    Then the rename is refused saying "Project name must be unique within the team"
    And both projects keep their names

  @us-iapr-03 @edge
  Scenario: Re-casing a project's own name is a valid rename
    When Priya renames "Sandbox" to "SANDBOX"
    Then the row she gets back shows "SANDBOX" with key prefix "SBX"
    And the project's stored address is unchanged

  @us-iapr-03 @error @security
  Scenario: A rename aimed at a garbled project id is answered like a missing page
    When Priya sends a rename aimed at the project id "not-a-uuid"
    Then the answer is byte-identical to a never-existed address

  @us-iapr-03 @error @security
  Scenario: A rename aimed at a project that does not exist is answered like a missing page
    When Priya sends a rename aimed at a project id that matches nothing
    Then the answer is byte-identical to a never-existed address

  # ------------------------------------------------- @needs-browser — the DOM oracle
  # The HTTP lane is byte-blind to the htmx row swap and to form-errors.js routing
  # the 422 fragment into the row's slot (the form-errors RCA). These two
  # scenarios drive a REAL headless Chrome against the same in-process origin.

  @us-iapr-02 @needs-browser @driving_port @real-io
  Scenario: The row updates in place when the rename succeeds
    Given Priya has the instance dashboard open in her browser
    When she types "Identity Platform" into the "Auth v2" row and submits it
    Then that row shows "Identity Platform" without the page reloading

  @us-iapr-03 @needs-browser @error @real-io
  Scenario: A refused rename explains itself inside the row being edited
    Given Priya has the instance dashboard open in her browser
    When she blanks the name in the "Auth v2" row and submits it
    Then "Project name must not be empty" appears inside that row's message area
    And the rename form is still there for her to correct
    When she types "Identity Platform" into the "Auth v2" row and submits it
    Then that row shows "Identity Platform" without the page reloading
