# Feature: per-workspace-backup (US-PWB-01/02/03): a self-hosting operator
# EXPORTS exactly one workspace's data to a portable, verifiable archive via the
# operator CLI, and PROVES — from the archive path alone — that the archive is
# complete (all 10 tenant tables) and isolation-clean (every row belongs to the
# declared workspace, no sibling's data rode along). EXPORT only; per-workspace
# restore/import is OUT for v1 (deferred — DD-MWT-09 sibling-clobber risk).
#
# Hypothesis (design/architecture.md §5-9, ADR-001..006): an operator can lift
# ONE tenant's full data set out of a multi-tenant instance into a single tar
# archive (manifest.json + tables/<table>.jsonl, whole-row to_jsonb) whose
# selection predicate IS the verify isolation predicate — and we KNOW we are
# right when, in a TWO-workspace fixture, an export of W contains EVERY W row
# across the 10 tables and NONE of any sibling's (including the transitively
# scoped team_memberships and the comment→issue→workspace cross-check), a
# multi-membership user IS included (legitimately, not as a leak), verify-export
# confirms completeness + isolation and exits 0, and a PLANTED sibling row makes
# verify-export RED (the falsifiability crux, US-PWB-02). DISPROVED if an export
# leaks a sibling row that verify accepts, omits a tenant table that verify calls
# complete, a failed export leaves a complete-looking half-archive, or a failure
# path exits with the wrong code.
#
# THREE BINDING DESIGN findings honoured here (design/upstream-changes.md):
#   1. DRIFT-1 (HIGH): `workspaces` has NO `slug` column (only id + name exist;
#      Store::list_workspaces returns (id, name)). The DISCUSS `globex`/`acme`
#      tokens are therefore SELECTOR tokens — accepted as an `<id>` OR an exact,
#      case-insensitive `<name>` match — NOT a `slug` column lookup. No schema
#      change, no `0012` migration. `list-workspaces` prints id + name.
#   2. DRIFT-2 (MEDIUM): `comments` HAS a denormalized direct `workspace_id`
#      (0004_comments.sql:21); only `team_memberships` is genuinely
#      transitive-only (via team_id). So export scopes `comments` directly, and
#      verify ADDITIONALLY walks comment.issue_id → issues.workspace_id as a
#      CROSS-CHECK (a denormalized workspace_id that disagrees with the issue's is
#      itself a corruption verify must catch — AC-02.3).
#   3. The `users` membership special case (OD-PWB-1 / ADR-001, RATIFIED): `users`
#      is a global identity table with no workspace_id. The export includes the
#      users that are MEMBERS of W (id IN memberships WHERE workspace_id=W). A
#      multi-membership user (member of Acme AND Globex) in a Globex archive is
#      NOT a sibling violation — isolation is strict "owned-by-W" for tables 1-9
#      and "member-of-W" for users (table 10). This is the ONE non-uniform place
#      in the predicate.
#
# Driving adapter: the operator CLI served by the `foundry` binary, reached at
#   foundry doctor list-workspaces
#   foundry doctor export-workspace <id|name-selector> <out-path>
#   foundry doctor verify-export <path>
# invoked as a REAL subprocess (assert_cmd::cargo_bin("foundry")) with
# DATABASE_URL pointing at the per-scenario testcontainers schema, reusing the
# allow-listed run_provision_workspace / run_backup_verify scaffold
# (thread-isolated tokio runtime, live DB via sqlx, structured key:value +
# terminating `status:` stdout, exit codes 0/2/3/4/5 mirroring admin_cli.rs).
# OFF the bearer API entirely (NFR-PWB-SURF-01; admin_cli is LAYER-1e allow-listed
# by file stem — ADR-006, no new allow-list line).
#
# Driven adapters exercised (LAYER 3, @real-io): real Postgres (the 10 TENANT_TABLES
# — workspaces, users, workspace_memberships, teams, team_memberships, projects,
# issues, invites, comments, machine_tokens) via testcontainers + per-scenario
# schema, seeded with TWO real coexisting workspaces ("Acme" + "Globex") each
# holding their own rows; the real Store::export_workspace scoped reader (10
# scoped SELECTs in ONE REPEATABLE READ tx — ADR-003); the tar archive written to
# a real filesystem path (tempfile::TempDir), atomically (`<out>.partial` → fsync
# → rename — NFR-PWB-ATOM-01); the real verify-export reader applying the SAME
# scope predicate to the archived rows offline. No mocks at the acceptance level.
#
# DECISIONS baked into the scenarios:
#   - Archive = tar of manifest.json (declared_workspace_id, declared_workspace_name,
#     tenant_tables[10], row_counts{}, format_version) + tables/<table>.jsonl
#     (whole-row to_jsonb JSONL — the slice-05 idiom). ADR-002.
#   - verify-export is PATH-ONLY (declared workspace read from the header, no
#     out-of-band arg — NFR-PWB-INT-01): completeness (10 tables present + per-table
#     JSONL line count == header count → exit 4 on truncation/short count) then
#     isolation (re-apply the predicate offline per archived row → any sibling →
#     non-zero, NAMES the foreign row). ADR-004.
#   - Exit codes (mirror admin_cli.rs): 0 OK · 2 unknown/invalid workspace (+ redirect
#     to list-workspaces) · 3 DB unreachable / mid-read error · 4 truncated/incomplete
#     archive (verify) · 5 output-path error (parent missing/unwritable, fails BEFORE
#     any DB read).
#   - Sensitivity: the archive contains users.password_hash + machine_tokens rows;
#     a successful export prints a one-line at-rest disclosure note (NFR-PWB-SEC-01).
#   - Completeness gold discipline (OD-PWB-2 / ADR-005): the acceptance side asserts
#     the archive COVERS all 10 TENANT_TABLES; the plant-a-row-PER-table gold test
#     (export count + verify completeness both see all 10) is a DELIVER build/unit
#     guard mirroring check_arch.rs's plant-a-violation discipline — noted in
#     distill/test-scenarios.md, not authored as a subprocess acceptance scenario.
#
# RED-state contract (DISTILL, ADR-025 / Mandate 7): the crate COMPILES — this file
# is Gherkin text and adds NO new undefined-symbol reference to any .rs; no step
# glue is authored in DISTILL and acceptance.rs is untouched → NOT BROKEN. Genuine
# RED is MISSING_FUNCTIONALITY at runtime against the real testcontainers PG16:
#   1. The `list-workspaces`, `export-workspace`, and `verify-export` doctor
#      subcommands do not exist yet — the main.rs doctor dispatch has no match arm
#      for them, so every scenario's subprocess invocation fails (unknown subcommand).
#      That is the genuine RED.
#   2. `Store::export_workspace(W) -> WorkspaceExport` (the 10 scoped SELECTs in one
#      REPEATABLE READ tx) does not exist yet in foundry-store; nor does the
#      admin_cli verify-export reader/isolation check, nor the owned `TENANT_TABLES`
#      constant + manifest writer. DELIVER builds the step glue
#      (crates/foundry-acceptance/src/steps/feature_per_workspace_backup.rs +
#      world.rs additions, registered in lib.rs, force-linked in acceptance.rs) and
#      the production code in the same RED→GREEN→COMMIT cycle.
#
# Per the layered test discipline (Mandates 9 + 11): LAYER-3 real-adapter +
# real-subprocess scenarios → EXAMPLE-BASED (NOT property-based); every sad /
# failure path is enumerated explicitly; no PBT machinery at this layer. The
# isolation invariant @property scenario is example-PINNED (a concrete
# two-workspace fixture exercised through the real CLI), not a Hypothesis/proptest
# generator — the generative exploration of the predicate belongs to layer 1-2
# unit/store tests in DELIVER. Mandate 8 state-delta is a layers 1-3 requirement
# with a Python pilot port; no `state_delta.rs` Rust port exists (matching slices
# 1-6's precedent), so LAYER-3 assertions are traditional assertions over
# port-exposed observables: the CLI exit code + stdout (per-table row counts,
# `status:` line, the sensitivity note, the verify check lines), the archive file's
# presence/absence at the output path, and the unchanged source-DB rows.
#
# Scope IN: per-workspace EXPORT + path-only VERIFY across the 10 tenant tables;
# id-or-name selector; the isolation crux + falsifiability; atomic write; the
# full 0/2/3/4/5 exit-code contract; the sensitivity disclosure. Scope OUT:
# per-workspace RESTORE / import (deferred v1 — DD-MWT-09); the web/bearer surface
# (off-bearer); a `workspaces.slug` column (DRIFT-1, NOT added); issue_attachments
# (NOT in the slice-05 10-table set for v1 — DRIFT-3 follow-up).
#
# All scenarios except the first @walking_skeleton one are @pending (one-at-a-time
# DELIVER cycle; DELIVER unskips one scenario per RED→GREEN→COMMIT cycle).

@per-workspace-backup @real-io @driving_adapter
Feature: An operator exports one workspace's data to a verifiable, isolation-clean archive
  Devansh runs a multi-tenant Foundry instance hosting "Acme Corp" and
  "Globex LLC". He can only back up the whole instance today; to archive a
  churned tenant or hand a departing customer its own data he would have to do
  manual surgery on a combined dump. From the operator shell he runs
  `foundry doctor list-workspaces` to see each workspace's id and name, then
  `foundry doctor export-workspace globex <path>` to write a single tar archive
  of exactly Globex's data across the 10 tenant tables, with a per-table row
  count report ending `status: OK`. He runs `foundry doctor verify-export <path>`
  and the archive confirms — from the path alone — that all 10 tables are present
  and every row belongs to Globex with no Acme row riding along. A multi-membership
  user who belongs to both Acme and Globex is legitimately included, not flagged
  as a leak. If a buggy export ever slipped one Acme row into the Globex archive,
  verify would catch it and refuse. Failures are guided, not cryptic: a typo'd
  name redirects to list-workspaces (exit 2), an unwritable path fails before any
  data is read (exit 5), a disk-full export leaves no complete-looking file, and a
  truncated archive is rejected (exit 4). Proven with REAL coexisting workspaces
  holding real seeded rows, not synthetic ids.

  Background:
    Given an instance with workspaces "Acme Corp" and "Globex LLC"
    And "Globex LLC" has its own members, teams, projects, issues, and comments
    And "Acme Corp" has its own members, teams, projects, issues, and comments

  # ============================================================================
  # US-PWB-01 — Export one workspace's data to a portable archive
  # ============================================================================

  # ----------------------------------------------------------------------------
  # 1. Walking skeleton — the demo-able export proof, end-to-end through the
  #    operator CLI. "Devansh exports one workspace and gets a verifiable archive
  #    reporting all 10 tenant tables, ending status: OK." This is the headline
  #    user value and the thinnest cut that proves the CLI port wires through to
  #    the scoped store reader, the tar writer on a real filesystem, and back to
  #    structured stdout + exit 0. (US-PWB-01 / AC-01.2)
  # ----------------------------------------------------------------------------
  @walking_skeleton @wiring_e2e @us-pwb01
  Scenario: An operator exports one workspace to a verifiable archive reporting all ten tables
    When Devansh exports "globex" to a backup path
    Then an archive file exists at that path
    And the output reports a row count for all 10 tenant tables
    And the output ends with "status: OK"
    And the command exits with code 0

  # ----------------------------------------------------------------------------
  # 2. list-workspaces shows each workspace's identity so the operator can name a
  #    target. DRIFT-1: prints id + name (no slug column exists). (AC-01.1)
  # ----------------------------------------------------------------------------
  @us-pwb01
  Scenario: An operator sees every workspace's identity before exporting
    When Devansh runs "foundry doctor list-workspaces"
    Then the output lists each workspace's id and name
    And both "Acme Corp" and "Globex LLC" appear
    And the output ends with "status: OK"
    And the command exits with code 0

  # ----------------------------------------------------------------------------
  # 3. The selector resolves a workspace by its id, not only by name. DRIFT-1:
  #    the id-or-name selector is ONE resolution fn feeding the archive header.
  #    (AC-01.3)
  # ----------------------------------------------------------------------------
  @us-pwb01
  Scenario: An operator exports a workspace selected by its id
    When Devansh exports the workspace whose id is Acme Corp's to a backup path
    Then the selector resolves to "Acme Corp"
    And an archive of "Acme Corp" exists at that path
    And the output ends with "status: OK"

  # ----------------------------------------------------------------------------
  # 4. The export is read-only — exporting never mutates the source instance.
  #    (System constraint: read-only; reinforced by AC-03.4)
  # ----------------------------------------------------------------------------
  @us-pwb01
  Scenario: Exporting a workspace removes nothing from the instance
    When Devansh exports "globex" to a backup path
    Then "Globex LLC" and all its data still exist on the instance unchanged
    And "Acme Corp" and all its data still exist on the instance unchanged

  # ============================================================================
  # US-PWB-02 — Prove the export contains only this tenant's data (the crux)
  # ============================================================================

  # ----------------------------------------------------------------------------
  # 5. The archive contains EVERY Globex row across the tables and NO Acme row —
  #    the completeness + isolation core in a real two-workspace fixture. The
  #    member set is exactly Globex's members. (AC-02.1)
  # ----------------------------------------------------------------------------
  @us-pwb02
  Scenario: The archive contains every target row and no sibling row
    When Devansh exports "globex" to a backup path
    Then every row in the archive belongs to "Globex LLC"
    And no row in the archive belongs to "Acme Corp"
    And the archive's member set is exactly the members of "Globex LLC"

  # ----------------------------------------------------------------------------
  # 6. verify-export confirms completeness AND isolation from the path alone and
  #    exits 0 on a clean archive. The declared workspace is read from the header,
  #    not passed as an argument (NFR-PWB-INT-01). (AC-02.2)
  # ----------------------------------------------------------------------------
  @us-pwb02
  Scenario: An operator confirms an export is complete and isolation-clean
    Given Devansh has exported "globex" to a backup path
    When Devansh runs "foundry doctor verify-export" on that archive
    Then the report confirms all 10 tenant tables are present
    And the report confirms every row belongs to the declared workspace
    And the report confirms no row references a sibling workspace
    And the command exits with code 0

  # ----------------------------------------------------------------------------
  # 7. Transitively-scoped rows are isolation-checked through the FK chain too —
  #    not only the direct-workspace_id tables. team_memberships reaches the
  #    workspace only via team_id; comments are cross-checked via
  #    comment.issue_id → issues.workspace_id (DRIFT-2: comments has a direct
  #    workspace_id, so this is a corruption cross-check). (AC-02.3)
  # ----------------------------------------------------------------------------
  @us-pwb02
  Scenario: Transitively-scoped rows are isolation-checked through the foreign-key chain
    Given Devansh has exported "globex" to a backup path
    When Devansh runs "foundry doctor verify-export" on that archive
    Then each team membership is resolved to its owning workspace through its team
    And each comment is cross-checked against its issue's owning workspace
    And every transitively-scoped row is confirmed to belong to "Globex LLC"

  # ----------------------------------------------------------------------------
  # 8. The users membership special case (OD-PWB-1 / ADR-001): a user who is a
  #    member of BOTH Acme and Globex is legitimately included in the Globex
  #    archive and is NOT treated as a sibling leak. Verify confirms each archived
  #    user is a member of Globex; it does not fail because the user also belongs
  #    elsewhere. (AC-02.1 + the OD-PWB-1 ratified semantics)
  # ----------------------------------------------------------------------------
  @us-pwb02
  Scenario: A user who belongs to two workspaces is legitimately included and not flagged as a leak
    Given a user is a member of both "Acme Corp" and "Globex LLC"
    And Devansh has exported "globex" to a backup path
    When Devansh runs "foundry doctor verify-export" on that archive
    Then that shared user appears in the archive as a member of "Globex LLC"
    And verification does not flag that shared user as a sibling-workspace row
    And the command exits with code 0

  # ----------------------------------------------------------------------------
  # 9. The isolation check BITES — the falsifiability crux (NFR-PWB-ISO-01,
  #    US-PWB-02). A planted Acme row inside a Globex archive makes verify-export
  #    RED: it exits non-zero and the message NAMES the foreign row resolving to a
  #    workspace other than the declared one. This is the security guarantee that
  #    a leak can never pass verification. (AC-02.4)
  # ----------------------------------------------------------------------------
  @us-pwb02 @error
  Scenario: Verification fails loudly when a sibling row is planted in an archive
    Given Devansh has exported "globex" to a backup path
    And one row belonging to "Acme Corp" is planted into that archive
    When Devansh runs "foundry doctor verify-export" on that archive
    Then the isolation check fails
    And the command exits with a non-zero code
    And the message identifies a row resolving to a workspace other than the declared one

  # ----------------------------------------------------------------------------
  # 10. The isolation invariant, example-pinned (LAYER-3 — NOT a generator). In a
  #     two-workspace fixture, exporting then verifying EITHER workspace confirms
  #     zero rows resolve to the other. The @property tag flags the universal
  #     invariant for DELIVER to amplify with a generative store-level (layer 1-2)
  #     property test; here it is pinned to the concrete Acme/Globex fixture
  #     exercised through the real CLI. (AC-02.5)
  # ----------------------------------------------------------------------------
  @us-pwb02 @property
  Scenario Outline: An export of any single workspace contains no sibling data
    When Devansh exports "<target>" to a backup path
    And Devansh runs "foundry doctor verify-export" on that archive
    Then verification confirms zero rows resolve to any workspace other than "<target>"
    And the command exits with code 0

    Examples:
      | target     |
      | globex     |
      | acme       |

  # ============================================================================
  # US-PWB-03 — Survive every failure path without surprising or burning the operator
  # ============================================================================

  # ----------------------------------------------------------------------------
  # 11. Unknown workspace → exit 2 with a redirect to list-workspaces, no archive
  #     created. The selector matched neither an id nor a name. (AC-03.1)
  # ----------------------------------------------------------------------------
  @us-pwb03 @error
  Scenario: Exporting an unknown workspace is refused with guidance
    When Devansh exports "nope" to a backup path
    Then the command exits with code 2
    And the message tells Devansh to run "foundry doctor list-workspaces"
    And no archive file is created at that path

  # ----------------------------------------------------------------------------
  # 12. Output-path error → exit 5, failing BEFORE any tenant data is read. The
  #     parent directory does not exist; the pre-flight stage catches it before
  #     opening the DB read snapshot. (AC-03.2)
  # ----------------------------------------------------------------------------
  @pending @us-pwb03 @error
  Scenario: A failed export never leaves a half-written archive when the path is unwritable
    When Devansh exports "globex" to a path whose parent directory does not exist
    Then the command exits with code 5
    And no file exists at that path
    And the failure happened before any tenant data was read

  # ----------------------------------------------------------------------------
  # 13. Atomic write — a disk-full / killed export leaves no complete-looking file
  #     at the final path (NFR-PWB-ATOM-01: <out>.partial → fsync → rename). A
  #     later verify-export on the final path finds no archive to accept. (AC-03.3)
  # ----------------------------------------------------------------------------
  @pending @us-pwb03 @error
  Scenario: A disk-full export leaves no complete-looking archive
    Given an export of "globex" fails mid-write because the disk fills
    Then no file exists at the final output path
    And at most a discardable partial file remains
    And a later verify-export on the final path finds no archive to accept

  # ----------------------------------------------------------------------------
  # 14. Truncated archive → verify exits 4 with an actionable message. The
  #     completeness count tripwire (manifest row_counts vs JSONL line count)
  #     catches a short archive before the isolation pass. (AC-03.5)
  # ----------------------------------------------------------------------------
  @pending @us-pwb03 @error
  Scenario: Verification detects an incomplete archive
    Given Devansh has exported "globex" to a backup path
    And that archive was truncated when the disk filled mid-export
    When Devansh runs "foundry doctor verify-export" on the truncated archive
    Then the command exits with code 4
    And the message says the archive is truncated or incomplete and to re-run the export

  # ----------------------------------------------------------------------------
  # 15. DB unreachable → exit 3 with an actionable message, mirroring the shipped
  #     scaffold's DB/infra failure code. (AC-01.4)
  # ----------------------------------------------------------------------------
  @us-pwb03 @error
  Scenario: The export reports a clear error when the database is unreachable
    Given the database is unreachable
    When Devansh exports "globex" to a backup path
    Then the command exits with code 3
    And the message says it could not connect to the database

  # ----------------------------------------------------------------------------
  # 16. Sole-workspace export is valid and read-only — a single-tenant install can
  #     take a pre-migration snapshot. The output notes it is the only workspace;
  #     nothing is deleted. (AC-03.4)
  # ----------------------------------------------------------------------------
  @us-pwb03
  Scenario: Exporting the only workspace is valid and removes nothing
    Given a single-tenant instance whose only workspace is "Acme Corp"
    When Devansh exports "acme" to a backup path
    Then an archive file exists at that path
    And the output notes that this is the only workspace on the instance
    And "Acme Corp" and all its data still exist on the instance unchanged

  # ----------------------------------------------------------------------------
  # 17. At-rest sensitivity disclosure (NFR-PWB-SEC-01) — a successful export
  #     prints a note that the archive holds password hashes and machine-token
  #     rows and advises treating it as sensitive at rest. (AC-03.6)
  # ----------------------------------------------------------------------------
  @us-pwb03
  Scenario: The operator is warned about sensitive at-rest contents on a successful export
    When Devansh exports "globex" to a backup path
    Then the output prints a note that the archive contains password hashes and machine-token rows
    And the note advises treating the archive as sensitive at rest
    And the command exits with code 0
