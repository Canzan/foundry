# Feature: multi-workspace-provisioning — Slice 5 (US-MWT06): prove that a REAL
# pre-feature single-workspace install upgrades forward-only with ZERO data loss
# and ZERO change to how its users sign in and work; the existing workspace
# becomes workspace 1, its id unchanged, and existing sessions + machine tokens
# keep resolving to it. This is the deferred slice 5 of the shipped
# multi-workspace-tenancy isolation core, now its own feature. The `0009`
# guard-drop + `0010` active-workspace column ALREADY SHIPPED in the parent
# milestone; this slice is the upgrade-safety PROOF that those forward-only
# migrations (plus this feature's additive `0011_instance_admins.sql`) touch no
# existing data.
#
# Hypothesis (slices/slice-05-existing-install-migration.md, ADR-004): a real
# pre-feature single-workspace DB can upgrade forward-only with zero data loss
# and zero change to existing sessions/tokens/sign-in — and we KNOW we are right
# when a real pre-feature snapshot upgrades, every tenant row is present and
# unchanged, the workspace id is unchanged, and a carried-over session + machine
# token still resolve to workspace 1. DISPROVED if the migration rewrites,
# moves, or cross-wires any row, changes the workspace id, or breaks an existing
# session/token.
#
# DESIGN finding (ADR-004 / D4): there is NO backfill migration. The shipped
# `resolve_active_workspace` (foundry-store/src/lib.rs:419) already maps an
# upgraded user (`active_workspace_id IS NULL` + exactly one membership) ⇒ that
# one workspace deterministically, with NO value written. A backfill setting
# `active_workspace_id` would REWRITE rows OD-4 promises to leave untouched, for
# zero functional gain — so the slice-5 guarantee is a real-snapshot
# before/after-EQUALITY proof, NOT a row rewrite.
#
# Driving surface (this slice is migration-shaped, not a user-facing CLI/HTTP
# entry — the "actor" is the operator upgrading the binary). The upgrade is
# driven through the SHIPPED migration runner: the per-scenario harness stages
# the pre-feature migration history (`0001`..`0008`) in a tempfile::TempDir (the
# `support/test_migration.rs` `TestMigrationsDir` precedent), seeds
# representative tenant data via the real `Store`, then applies the canonical
# forward-only migrations (`0009`, `0010`, `0011`) via the SAME
# `run_migrations_from_dir` the production boot path uses. The user-observable
# outcomes (sign-in still works, session/token still resolves to workspace 1)
# are then exercised through the SHIPPED sign-in + `resolve_active_workspace`
# seam — the real driving ports a returning user actually hits.
#
# Driven adapters exercised (LAYER 3, @real-io): real Postgres (workspaces,
# users, workspace_memberships, teams, team_memberships, projects, issues,
# invites, machine_tokens, tower_sessions) via testcontainers + per-scenario
# schema; the real migration runner under its advisory-lock guard; the real
# sign-in path + `resolve_active_workspace`; the real Ed25519 machine-token
# verify path. No mocks at the acceptance level (the clock is the only fake, and
# only where time must advance).
#
# RED-state contract (DISTILL, ADR-025 / Mandate 7): the crate COMPILES (feature
# files are Gherkin text and do not affect compilation; no new undefined-symbol
# references are added to any .rs) → NOT BROKEN. Genuine RED is
# MISSING_FUNCTIONALITY at runtime against the real testcontainers PG16:
#   1. `0011_instance_admins.sql` does not exist yet — applying the canonical
#      forward-only set FAILS to find migration `0011` until DELIVER ships it.
#      That is the genuine RED for the additive-migration scenarios.
#   2. The real-snapshot before/after-equality harness (stage pre-0009 history,
#      snapshot tenant tables, apply 0009/0010/0011, re-snapshot, assert
#      equality) is NEW test infrastructure DELIVER must build — until it exists,
#      the proof scenarios assert against an unbuilt harness (MISSING_FUNCTIONALITY).
#   3. The shipped `resolve_active_workspace` ALREADY maps NULL-active +
#      sole-membership ⇒ workspace 1 (the no-backfill finding). So once the
#      snapshot harness exists, the resolution scenarios prove the SHIPPED
#      behaviour holds across the upgrade (green-by-inheritance behind the
#      harness gate) — they assert the contract, they do not require new
#      resolution code.
#
# Per the layered test discipline (Mandates 9 + 11): LAYER-3 real-adapter +
# real-migration scenarios → example-based (NOT property-based); every sad path
# is enumerated explicitly; no PBT machinery at this layer. Mandate 8 state-delta
# is a layers 1-3 requirement with a Python pilot port; no `state_delta.rs` Rust
# port exists (matching slices 1-4's precedent), so LAYER-3 assertions are
# traditional assertions over port-exposed observables: tenant-table row counts +
# per-row content equality, the workspace id, sign-in success, and the
# workspace `resolve_active_workspace` returns.
#
# Scope: SLICE 5 ONLY — the existing-install upgrade-safety guarantee. Workspace
# PROVISIONING (slice 6a) and rate-bucket eviction (slice 6b) are explicitly OUT
# — they live in us-mwt-slice-06-provision-and-prove.feature. The cross-tenant
# non-enumerability matrix (parent slices 1-4) is SHIPPED and NOT re-authored.
#
# All scenarios except the first @walking_skeleton one are @pending (one-at-a-
# time DELIVER cycle; DELIVER unskips one scenario per RED→GREEN→COMMIT cycle).

@multi-workspace-provisioning @mwt-slice-05 @real-io @driving_adapter
Feature: An existing single-workspace install upgrades to workspace 1 with no data loss
  Sasha runs a pre-feature single-workspace Foundry: one workspace, its users,
  teams, projects, issues, invites, a live session, and a valid machine token.
  She upgrades the binary; the forward-only migrations run. Afterward every
  tenant row is present and byte-for-byte unchanged, the existing workspace IS
  workspace 1 with its identity unchanged, her users sign in exactly as before,
  and a session and machine token carried across the upgrade still resolve to
  workspace 1. The upgrade drops a guard and adds an empty role table — it
  rewrites nothing. Proven against a REAL pre-feature database snapshot, not an
  assumed migration contract.

  Background:
    Given a pre-feature single-workspace install of "Acme" with admin "ops@acme.com"
    And "Acme" has members, teams, projects, issues, and invites
    And "Acme" has a live signed-in session and a valid machine token

  # ----------------------------------------------------------------------------
  # 1. Walking skeleton — the demo-able upgrade-safety proof.
  #    "Sasha upgrades her install; her workspace becomes workspace 1 with all its
  #    data intact, and her users sign in and work exactly as before."
  #    This is the thinnest end-to-end cut that answers the riskiest question:
  #    does the upgrade lose or change any data, or break sign-in?
  # ----------------------------------------------------------------------------
  @walking_skeleton @wiring_e2e @us-mwt06
  Scenario: Upgrading a single-workspace install keeps it working as workspace 1
    When the install is upgraded to multi-workspace support
    Then the existing workspace becomes the first workspace with its identity unchanged
    And all of its tenant data is present and unchanged
    And "ops@acme.com" signs in and works exactly as before

  # ----------------------------------------------------------------------------
  # 2. No tenant data is lost or changed — the row-level before/after equality
  #    proof across every tenant table (NFR-MWT-DATA-01). This is the data-safety
  #    core: snapshot before, upgrade, snapshot after, assert equality.
  # ----------------------------------------------------------------------------
  @us-mwt06
  Scenario: No tenant data is lost or changed by the upgrade
    Given a recorded snapshot of all the workspace's data before the upgrade
    When the install is upgraded to multi-workspace support
    Then every tenant row is present and unchanged afterward
    And the existing workspace's identity is unchanged

  # ----------------------------------------------------------------------------
  # 3. Carried-over session + machine token still resolve (NFR-MWT-DATA-02).
  #    A session and a token that predate the upgrade keep working and resolve to
  #    workspace 1 — proving the NULL-active + sole-membership resolution path.
  # ----------------------------------------------------------------------------
  @us-mwt06
  Scenario: Existing sessions and machine tokens still resolve after the upgrade
    Given an active session and a valid machine token from before the upgrade
    When the install is upgraded to multi-workspace support
    Then the carried session still resolves to the first workspace
    And the carried machine token still acts on the first workspace

  # ----------------------------------------------------------------------------
  # 4. No backfill — the upgraded user's active workspace stays UNWRITTEN, and
  #    resolution still maps them to workspace 1 (D4 / ADR-004 the no-backfill
  #    finding made observable). This proves the guarantee is achieved WITHOUT a
  #    row rewrite: the active-workspace value remains as the upgrade left it.
  # ----------------------------------------------------------------------------
  @us-mwt06
  Scenario: An upgraded user resolves to workspace 1 without their active workspace being written
    Given a user whose active workspace was never chosen before the upgrade
    When the install is upgraded to multi-workspace support
    Then that user resolves to the first workspace
    And their active-workspace choice remains unwritten

  # ----------------------------------------------------------------------------
  # 5. Re-running the upgrade is a no-op (idempotent in effect). Applying the
  #    forward-only migrations a second time neither duplicates nor alters the
  #    workspace or any tenant row.
  # ----------------------------------------------------------------------------
  @us-mwt06 @error
  Scenario: Re-running the upgrade does not duplicate or alter anything
    Given the install has already been upgraded to multi-workspace support
    When the upgrade is applied a second time
    Then the workspace is neither duplicated nor altered
    And every tenant row remains exactly as it was after the first upgrade

  # ----------------------------------------------------------------------------
  # 6. The existing auth + workspace behaviour stays green across the upgrade
  #    (NFR-MWT-REL-02). The single-workspace behaviours are the one-membership
  #    special case of multi-workspace; nothing a returning user does regresses.
  # ----------------------------------------------------------------------------
  @us-mwt06 @regression
  Scenario: Existing sign-in and workspace behaviour is unchanged after the upgrade
    When the install is upgraded to multi-workspace support
    Then an existing member signs in and lands on the first workspace
    And the existing member sees exactly the issues and projects they saw before
    And nothing about the single-workspace experience has changed
