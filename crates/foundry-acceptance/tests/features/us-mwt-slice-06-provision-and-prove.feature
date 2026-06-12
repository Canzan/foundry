# Feature: multi-workspace-provisioning — Slice 6a (US-MWT07) + the provisioning
# leg of US-MWT08: an instance super-admin provisions a NEW workspace via the
# operator CLI and seeds its first admin; the newly-provisioned workspace is a
# real coexisting tenant that honours the SHIPPED isolation boundary; creating it
# touches no existing workspace; and a non-super-admin is refused, fail-closed
# and non-enumerably. This is the deferred slice 6 of the shipped
# multi-workspace-tenancy isolation core, now its own feature.
#
# Hypothesis (slices/slice-06-provision-and-prove.md, ADR-001/002/003): an
# operator can create a second workspace + first admin (reusing the bootstrap
# seeding idiom) WITHOUT touching existing tenants, gated by a NEW instance
# super-admin authority — and we KNOW we are right when provisioning "Globex"
# leaves "Acme" untouched, Globex's first admin signs in and acts only on Globex,
# a member of Globex cannot reach Acme (and vice-versa, the SHIPPED boundary),
# and a non-super-admin's provisioning attempt is refused. DISPROVED if
# provisioning leaks across tenants, the new workspace is reachable by the wrong
# people, or a non-super-admin can create a workspace.
#
# TWO BINDING DESIGN findings honoured here (upstream-changes.md):
#   1. CLI-FIRST provisioning (ADR-002 / D2, RATIFIED — REVISES parent ADR-004's
#      web-first sketch). The v1 surface is the operator CLI
#      `foundry doctor provision-workspace --name <name> --admin-email <addr>
#      [--as <super-admin-email>]`. The web `/admin/instance/…` flow is DEFERRED
#      and OUT OF SCOPE. Every provisioning scenario drives the CLI port, never a
#      web POST.
#   2. The `bootstrap.rs:~301` `create_workspace` 409 guard is STILL PRESENT
#      (the `0009` index drop did NOT remove the application-level 409 handler).
#      For v1 the CLI is the active provisioning surface; the web `create_workspace`
#      409 is the deferred-web-flow EXTEND point and is NOT exercised here. These
#      scenarios do NOT assume the guard is gone and do NOT touch the parent docs.
#
# Authority model (ADR-001 / ADR-003, RATIFIED): the operator who CLAIMS the
# bootstrap (creating workspace 1 + its admin) ALSO becomes the first
# `instance_admins` row (the first super-admin). Provisioning is gated by the NEW
# `is_instance_admin(user_id)` authz (an `EXISTS (SELECT 1 FROM instance_admins
# WHERE user_id=$1)` shape, instance-scoped — mirrors `is_workspace_admin` but
# takes no workspace arg, so it cannot trip the LAYER-1e tenant guard). An
# upgraded install grants the operator via `foundry doctor grant-super-admin
# --email <addr>` (idempotent `ON CONFLICT DO NOTHING`).
#
# Driving adapter: the operator CLI served by the `foundry` binary, reached at
#   foundry doctor provision-workspace --name <name> --admin-email <addr> [--as <addr>]
#   foundry doctor grant-super-admin   --email <addr>
#   foundry doctor revoke-super-admin  --email <addr>
# invoked as a real subprocess (assert_cmd::Command::cargo_bin("foundry")) with
# DATABASE_URL pointing at the per-scenario testcontainers schema, reusing the
# allow-listed run_restore_comment scaffold (thread-isolated tokio runtime, live
# DB via sqlx, structured exit codes). The ISOLATION PROOF leg then drives the
# SHIPPED sign-in + resolution + scoped-read ports (the in-process axum router /
# resolve_active_workspace seam slices 1-4 use) — proving the provisioned tenant
# obeys the already-shipped boundary.
#
# Driven adapters exercised (LAYER 3, @real-io): real Postgres (workspaces,
# users, workspace_memberships, teams, team_memberships, projects, issues,
# invites, instance_admins) via testcontainers + per-scenario schema; the real
# bootstrap/invite seeding transaction (provision_workspace mirrors
# create_initial_workspace's atomic shape); the real is_instance_admin authz; the
# real sign-in + resolve_active_workspace seam; the in-process axum router for
# the scoped-read isolation proof. No mocks at the acceptance level.
#
# Refusal / non-enumerability decision (ADR-003 + the SHIPPED uniform-404 idiom
# OD-MWT-D6): a non-super-admin's provisioning attempt is refused FAIL-CLOSED via
# the structured CLI exit-code contract (a distinct non-zero "not authorized"
# exit, mirroring run_restore_comment's exit-code discipline); the authz failure
# does NOT leak whether the target workspace name or admin email already exists
# (no existence oracle — "not authorized" is observationally independent of
# whether the target exists). The provisioned tenant's cross-tenant refusals
# (Globex member reaching Acme, and vice-versa) reuse the SHIPPED non-enumerable
# uniform-404 — a foreign resource is refused IDENTICALLY to a never-existed one;
# NEVER a 403-vs-404 existence oracle.
#
# RED-state contract (DISTILL, ADR-025 / Mandate 7): the crate COMPILES (feature
# files are Gherkin text and do not affect compilation; no new undefined-symbol
# references added to any .rs) → NOT BROKEN. Genuine RED is MISSING_FUNCTIONALITY
# at runtime against the real testcontainers PG16:
#   1. `0011_instance_admins.sql`, the `instance_admins` table, `is_instance_admin`,
#      the `provision_workspace` tx, and the `provision-workspace` /
#      `grant-super-admin` / `revoke-super-admin` CLI subcommands do not exist yet
#      — every provisioning scenario fails because the subcommand is unknown / the
#      table is absent. That is the genuine RED.
#   2. The ISOLATION leg rides the SHIPPED slice-1..4 boundary
#      (resolve_active_workspace, the per-table workspace_id scoping, the
#      attachments.rs-derived uniform-404). Once a workspace can be provisioned at
#      all, the isolation scenarios prove the SHIPPED behaviour holds for the NEW
#      tenant (green-by-inheritance behind the provisioning gate) — they assert
#      the boundary contract, they do not require new isolation code.
#
# Per the layered test discipline (Mandates 9 + 11): LAYER-3 real-adapter +
# real-subprocess scenarios → example-based (NOT property-based); every sad /
# evil-user / unauthorized path is enumerated explicitly; no PBT machinery at
# this layer. Mandate 8 state-delta is layers 1-3 with a Python pilot port; no
# `state_delta.rs` Rust port exists (matching slices 1-4), so LAYER-3 assertions
# are traditional assertions over port-exposed observables: the CLI exit code +
# stdout (new workspace id, invite link), the post-provision DB row presence
# scoped by workspace, the unchanged Acme snapshot, sign-in success, and the
# scoped-read result sets.
#
# Scope: SLICE 6a (provisioning) + the US-MWT08 PROVISIONING-ISOLATION proof leg
# ONLY. The slice-6b rate-bucket eviction (NFR-MWT-PERF-01, residual F2) is a
# unit/property test at the rate_limit module (layers 1-2), NOT an acceptance
# scenario — it is documented in distill/test-scenarios.md and lives in
# crates/foundry-app/src/rate_limit.rs tests, not here. The slice-5 migration
# guarantee is us-mwt-slice-05-migration-guarantee.feature. The web provisioning
# flow is DEFERRED (D2) and explicitly OUT.
#
# All scenarios except the first @walking_skeleton one are @pending (one-at-a-
# time DELIVER cycle; DELIVER unskips one scenario per RED→GREEN→COMMIT cycle).

@multi-workspace-provisioning @mwt-slice-06 @real-io @driving_adapter
Feature: An instance super-admin provisions a new isolated workspace from the operator CLI
  Sasha claimed her Foundry instance at bootstrap, so she is both workspace 1's
  admin and the first instance super-admin. From the operator shell she runs
  `foundry doctor provision-workspace` to create "Globex" and seed Priya as its
  first admin; the command prints Globex's id and Priya's invite link. Priya
  signs in, lands on Globex, and acts only on Globex — she cannot reach Acme, and
  no Acme member can reach Globex (the shipped boundary holds for the brand-new
  tenant). Creating Globex leaves Acme byte-for-byte untouched. A regular member
  who tries to provision is refused, fail-closed, without learning whether the
  target already exists. Proven with REAL coexisting workspaces, not synthetic
  ids.

  Background:
    Given an instance claimed by super-admin "ops@acme.com" with workspace "Acme"
    And "Acme" has a member "marco@acme.com" with issues in "Acme"

  # ----------------------------------------------------------------------------
  # 1. Walking skeleton — the demo-able provisioning proof, end-to-end through the
  #    operator CLI. "The super-admin provisions a new isolated workspace and its
  #    first admin; the new admin signs in and acts on it." This is the headline
  #    user value of the whole feature and the thinnest cut that proves the CLI
  #    port wires through to a real, reachable, isolated tenant.
  # ----------------------------------------------------------------------------
  @walking_skeleton @wiring_e2e @us-mwt07
  Scenario: A super-admin provisions a new isolated workspace with a first admin
    When the super-admin provisions workspace "Globex" with first admin "priya@globex.com"
    Then the new workspace "Globex" exists and is isolated from all others
    And the command reports the new workspace and a first-admin invite link
    And "priya@globex.com" signs in and acts on "Globex"

  # ----------------------------------------------------------------------------
  # 2. Creating a new workspace does not touch existing ones (NFR-MWT-REL-01).
  #    Snapshot Acme before, provision Globex, assert Acme is byte-for-byte
  #    unchanged and Globex starts empty.
  # ----------------------------------------------------------------------------
  @us-mwt07
  Scenario: Provisioning a new workspace leaves existing workspaces untouched
    Given a recorded snapshot of "Acme" and its data and members
    When the super-admin provisions workspace "Globex" with first admin "priya@globex.com"
    Then "Acme" and all its data and members are unchanged
    And "Globex" starts empty and isolated

  # ----------------------------------------------------------------------------
  # 3. The provisioned tenant honours the SHIPPED isolation boundary — Globex's
  #    member sees only Globex; a member of Acme cannot reach Globex
  #    (NFR-MWT-SEC-01, proven for the NEW tenant). Green-by-inheritance behind
  #    the provisioning gate.
  # ----------------------------------------------------------------------------
  @us-mwt07 @us-mwt08
  Scenario: The provisioned workspace is a real coexisting tenant that sees only its own data
    Given the super-admin has provisioned workspace "Globex" with first admin "priya@globex.com"
    And "Globex" has issues that belong to "Globex"
    When "priya@globex.com" lists her issues
    Then she sees only "Globex" issues
    And no "Acme" issue appears

  # ----------------------------------------------------------------------------
  # 4. Cross-tenant refusal between the provisioned tenant and the existing one
  #    is non-enumerable (evil-user, NFR-MWT-SEC-02) — an Acme member reaching a
  #    real Globex resource is refused IDENTICALLY to a never-existed one. Proves
  #    the SHIPPED uniform-404 idiom extends to the freshly-provisioned tenant.
  # ----------------------------------------------------------------------------
  @pending @us-mwt07 @us-mwt08 @error
  Scenario: A member of the existing workspace cannot reach the provisioned one non-enumerably
    Given the super-admin has provisioned workspace "Globex" with first admin "priya@globex.com"
    And an issue belongs to "Globex"
    When "marco@acme.com" requests that "Globex" issue by its real address
    And "marco@acme.com" requests an issue that never existed
    Then the two responses are refused identically
    And nothing reveals that the "Globex" issue exists

  # ----------------------------------------------------------------------------
  # 5. A non-super-admin cannot provision (authz core, evil-user,
  #    NFR-MWT-SEC-04). A regular workspace member's provisioning attempt is
  #    refused fail-closed via the CLI exit-code contract.
  # ----------------------------------------------------------------------------
  @us-mwt07 @error
  Scenario: A non-super-admin cannot provision a workspace
    Given "marco@acme.com" is a regular member and not a super-admin
    When "marco@acme.com" attempts to provision workspace "Sneaky" with first admin "mallory@sneaky.test"
    Then the attempt is refused as not authorized
    And no new workspace was created

  # ----------------------------------------------------------------------------
  # 6. The authz refusal does not leak existence (non-enumerable authz,
  #    NFR-MWT-SEC-02 applied to provisioning). A non-super-admin attempting to
  #    provision an EXISTING name and a NEVER-existed name is refused identically
  #    — the refusal carries no oracle for whether the target already exists.
  # ----------------------------------------------------------------------------
  @pending @us-mwt07 @error
  Scenario: An unauthorized provisioning attempt does not reveal whether the target exists
    Given "marco@acme.com" is a regular member and not a super-admin
    When "marco@acme.com" attempts to provision a workspace named like an existing one
    And "marco@acme.com" attempts to provision a workspace named like one that never existed
    Then the two attempts are refused identically as not authorized
    And neither refusal reveals whether the target already exists

  # ----------------------------------------------------------------------------
  # 7. First super-admin comes from the bootstrap claim (ADR-001 / D1). The
  #    operator who claimed the instance is the first super-admin and can
  #    provision; an operator who never claimed cannot.
  # ----------------------------------------------------------------------------
  @us-mwt07
  Scenario: The bootstrap-claiming operator is the first super-admin and can provision
    Given "ops@acme.com" claimed the instance at bootstrap
    When "ops@acme.com" provisions workspace "Globex" with first admin "priya@globex.com"
    Then the provisioning succeeds
    And "Globex" exists and is isolated from all others

  # ----------------------------------------------------------------------------
  # 8. Upgraded installs gain a super-admin via grant (ADR-001 / D1). An install
  #    with no super-admin (an upgraded one) grants the operator via
  #    `grant-super-admin`; the grant is idempotent; the granted operator can then
  #    provision.
  # ----------------------------------------------------------------------------
  @us-mwt07
  Scenario: An upgraded install grants its first super-admin and can then provision
    Given an upgraded instance with workspace "Acme" and no super-admin yet
    When "ops@acme.com" is granted super-admin
    And "ops@acme.com" is granted super-admin a second time
    Then the grant is recorded exactly once
    And "ops@acme.com" can then provision workspace "Globex" with first admin "priya@globex.com"

  # ----------------------------------------------------------------------------
  # 9. Provisioning is not exposed on the bearer surface (NFR / api≠mint). A
  #    machine token — even a workspace-1-bound one — cannot reach a provisioning
  #    path on /api/v1; provisioning is off the bearer surface entirely.
  # ----------------------------------------------------------------------------
  @pending @us-mwt07 @error @verify-path-unchanged
  Scenario: Provisioning is unreachable from the bearer API surface
    Given a machine token is bound to "Acme"
    When a caller uses it to attempt workspace provisioning over /api/v1
    Then no provisioning path is reachable on the bearer surface
    And no new workspace was created
