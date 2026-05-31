# Story: US-03 — Operator backs up and restores
# Slice: 3 (operator-grade)
# JTBD: outcome-2 (Data sovereignty — single-file backup includes everything)
#
# Driving port: shell-out to system `pg_dump` and `pg_restore` against the
# testcontainers Postgres exposed at its mapped host port. Contributors
# must have pg_dump/pg_restore on PATH (documented as a slice-3 prereq in
# CONTRIBUTING.md; the driver verifies presence and skips with a clear
# message if missing — F-004 anti-flake policy, no silent pass).
#
# Driven adapters exercised (Strategy C — all real):
#   - real testcontainers Postgres 16 source database
#   - real pg_dump -Fc -> backup file on tmp_path
#   - real pg_restore --clean --if-exists against a second freshly-booted
#     testcontainers Postgres (target DB; pet-per-scenario, not the shared
#     one — restore is destructive and would poison sibling scenarios)
#   - real attachment binary round-trip (sha256 invariant assertion)
#
# NFR coverage: NFR-DATA-01 (all-state-in-Postgres), NFR-DATA-02 (single
# pg_dump completeness), NFR-DATA round-trip integrity. The `foundry doctor
# backup-verify` CLI subcommand is exercised via assert_cmd; this is the
# slice-3 driving-adapter coverage for that CLI entry point (RCA-fix P1).
#
# Out of scope for slice 3 (deferred):
#   - Same-major-version constraint testing across Postgres 15/16/17 — the
#     architecture doc declares "use same major version" as operator
#     responsibility; we do not parametrize a cross-version Outline.
#   - WAL archiving / PITR / continuous backup (post-MVP per backup-restore.md).
#
# Gherkin discipline (CM-B): scenarios talk in operator language ("back up",
# "restore", "the database"). The specific tooling (pg_dump / pg_restore /
# bytea / sha256) lives in step-method bodies and the comment block above,
# not in the steps a stakeholder reads. The exception is the `foundry doctor
# backup-verify` subcommand name + its CLI output contract, because the
# subcommand IS the operator-facing contract under test.

@slice3 @us-03 @backup-restore @needs-pgclient
Feature: An operator captures a complete Foundry backup with a single command and restores it on a fresh database with every file intact
  All Foundry state — issues, comments, attachments, sessions, the outbox
  — lives in the Foundry database (NFR-DATA-01). A single backup
  operation produces one backup file. Restoring that file onto a
  freshly-booted database of the same major version reproduces a
  functionally identical Foundry: every issue is present, every
  attachment downloads byte-identically, and sequential issue keys
  continue from where the source instance left off. The `foundry doctor
  backup-verify` subcommand parses a backup file and reports row counts,
  total attachment bytes, and an OK/FAIL status.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"
    And a project "Auth v2" with key prefix "AUTH" exists in the "Backend" team

  @walking_skeleton @real-io @driving_adapter
  Scenario: The operator restores a fresh backup on a clean database and finds every workspace, issue, and attachment intact
    Given the workspace contains 5 issues with titles "AUTH-1" through "AUTH-5"
    And issue "AUTH-3" has an attachment "screenshot.png" of 256 kilobytes
    When the operator captures a complete backup of the running Foundry instance into a single backup file
    And the operator restores that backup file onto a freshly-booted database
    And the operator points a foundry-app replica at the restored database
    Then signing in as "devansh@acme.com" with the same password succeeds against the restored instance
    And the workspace "Acme Eng" contains the same 5 issues "AUTH-1" through "AUTH-5"
    And the attachment "screenshot.png" on "AUTH-3" downloads byte-identical to the original

  @real-io
  Scenario: Attachment binary content survives backup-and-restore byte-identically
    Given issue "AUTH-1" has 3 attachments of 100, 2000, and 8000 kilobytes respectively
    When the operator backs up and restores the database
    Then each of the 3 attachments on "AUTH-1" downloads from the restored instance byte-identical to the original
    And the Content-Type recorded for each attachment is preserved through the restore

  @real-io
  Scenario: Sequential issue keys continue from where the source instance left off after a restore
    Given the workspace contains issues "AUTH-1" through "AUTH-5"
    When the operator backs up and restores the database
    And Mei files a new issue against "Auth v2" with title "Post-restore issue creation" on the restored instance
    Then the new issue's key is "AUTH-6"

  @real-io @nfr-data-01
  Scenario: No Foundry state lives outside the database — the backup file alone reproduces the system
    Given the workspace contains 3 issues, 2 comments, 1 attachment, and 1 active session for Mei
    When the operator captures a backup and then drops every Foundry table from the source database
    And the operator restores the backup onto a clean database
    Then the restored instance contains all 3 issues, all 2 comments, and the 1 attachment downloads byte-identical to the original
    And Mei's session from before the backup is still recognised by the restored instance

  @real-io @driving_adapter @us-03-cli
  Scenario: The `foundry doctor backup-verify` CLI subcommand reports row counts and exits zero on a healthy backup
    Given the workspace contains 4 issues and 2 attachments
    And the operator has captured a backup of the database to a file
    When the operator runs `foundry doctor backup-verify <backup-file>` as a subprocess
    Then the exit code is 0
    And the stdout contains a row-count entry for the "issues" table with the value 4
    And the stdout contains a row-count entry for the "issue_attachments" table with the value 2
    And the stdout contains a "status: OK" line

  @real-io @driving_adapter @us-03-cli @error
  Scenario: The `foundry doctor backup-verify` CLI subcommand reports failure and exits non-zero on a truncated backup
    Given the operator has captured a backup of the database to a file
    And the backup file has been truncated to its first 1024 bytes
    When the operator runs `foundry doctor backup-verify <backup-file>` as a subprocess
    Then the exit code is non-zero
    And the stdout or stderr identifies the backup file as unreadable or truncated
