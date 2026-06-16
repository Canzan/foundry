# Per-workspace export — operator journey (DISCUSS-wave Gherkin).
# Persona: Devansh, the self-hosting operator. Surface: `foundry doctor` CLI.
# Scope: EXPORT ONLY (per-workspace restore deferred — DD-MWT-09).
# These scenarios are the discovery-level contract; acceptance-designer (DISTILL)
# formalizes them into executable specs against the foundry doctor surface.

Feature: Export a single workspace's data
  As Devansh, a self-hosting operator
  I want to export exactly one workspace's data into a portable, verifiable archive
  So that I can archive, migrate, hand off, or pre-deletion-snapshot just that tenant
  Without including any other tenant's data

  Background:
    Given a Foundry instance with workspaces "Acme Corp" (slug "acme") and "Globex LLC" (slug "globex")
    And "Globex LLC" has 7 members, 3 teams, 8 projects, 412 issues, 1893 comments, 2 invites, and 4 machine tokens
    And "Acme Corp" has its own separate members, teams, projects, issues, and comments

  # --- Step 1: identify the target -------------------------------------------

  Scenario: Operator sees every workspace's identity before exporting
    When Devansh runs "foundry doctor list-workspaces"
    Then the output lists "Acme Corp" with its id and slug "acme"
    And the output lists "Globex LLC" with its id and slug "globex"
    And the output ends with "status: OK"

  # --- Step 2: export (happy path + isolation crux) --------------------------

  Scenario: Operator exports exactly one tenant's data
    When Devansh runs "foundry doctor export-workspace globex /backups/globex-2026-06-16.dump"
    Then an archive is written to "/backups/globex-2026-06-16.dump"
    And the output reports a per-table row count for all 10 tenant tables
    And the reported "users" count is 7
    And the output prints a note that the archive contains password hashes and machine-token rows
    And the output ends with "status: OK"

  Scenario: The archive contains only the target workspace's data
    When Devansh exports "globex"
    Then every row in the archive belongs to the Globex workspace
    And no row in the archive belongs to the Acme workspace
    And the archive's member set is exactly Globex's 7 members, not any Acme member

  @property
  Scenario: An export of any single workspace contains no sibling data
    When any one workspace is exported and then verified
    Then the verification confirms zero rows resolve to any other workspace

  # --- Step 2: failure paths -------------------------------------------------

  Scenario: Exporting an unknown workspace is refused with guidance
    When Devansh runs "foundry doctor export-workspace nope /backups/x.dump"
    Then the command exits with code 2
    And the message tells Devansh to run "foundry doctor list-workspaces"
    And no archive file is created

  Scenario: A failed export never leaves a half-written archive
    Given the output path "/nope/x.dump" has a parent directory that does not exist
    When Devansh runs "foundry doctor export-workspace globex /nope/x.dump"
    Then the command exits with code 5
    And no file exists at "/nope/x.dump"
    And no partial archive can be mistaken for a complete one

  Scenario: Exporting the only workspace on the instance is valid and removes nothing
    Given a single-tenant instance whose only workspace is "Acme Corp"
    When Devansh runs "foundry doctor export-workspace acme /backups/acme.dump"
    Then an archive is written
    And the output notes that this is the only workspace on the instance
    And "Acme Corp" and all its data still exist on the instance unchanged
    And the output ends with "status: OK"

  Scenario: The export reports a clear error when the database is unreachable
    Given DATABASE_URL points at an unreachable database
    When Devansh runs "foundry doctor export-workspace globex /backups/globex.dump"
    Then the command exits with code 3
    And the message says it could not connect to the database

  # --- Step 3: verify (completeness + isolation) -----------------------------

  Scenario: Operator confirms an export is complete and isolation-clean
    Given a freshly exported archive of the Globex workspace at "/backups/globex-2026-06-16.dump"
    When Devansh runs "foundry doctor verify-export /backups/globex-2026-06-16.dump"
    Then the report confirms all 10 tenant tables are present
    And the report confirms every row belongs to the declared Globex workspace
    And the report confirms no row references a sibling workspace
    And the command exits with code 0

  Scenario: Verification detects an incomplete archive
    Given an archive that was truncated when the disk filled mid-export
    When Devansh runs "foundry doctor verify-export" on the truncated archive
    Then the command exits with code 4
    And the message says the archive is truncated or incomplete and to re-run the export

  Scenario: Verification fails loudly if a sibling row ever leaked into an archive
    Given an archive that wrongly contains one row belonging to the Acme workspace
    When Devansh runs "foundry doctor verify-export" on that archive
    Then the isolation check fails
    And the command exits with a non-zero code
    And the message identifies that a row resolves to a workspace other than the declared one
