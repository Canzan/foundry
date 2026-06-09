# Feature: multi-workspace-tenancy — Slice 1 (Walking Skeleton): two workspaces
# coexist + request→workspace resolution. This is the load-bearing abstraction
# every later slice (2-6) depends on, so it ships FIRST.
#
# Hypothesis (slice-01-walking-skeleton-coexist.md): two real workspaces can
# coexist in one Foundry instance and a request resolves to EXACTLY its own
# workspace end-to-end on ONE read path, over the per-table `workspace_id`
# scoping that already ships — and a request resolving to NO workspace is
# refused, not defaulted.
#
# Chosen read path (distill/slice-01-wave-decisions.md): the JSON API
#   GET /api/v1/teams/{team}/projects/{project}/issues
# This is the cheapest, most load-bearing surface for the skeleton per ADR-001:
# `token.workspace_id` resolution + the `MachinePrincipal{workspace_id}`
# extractor + the per-table `workspace_id` issue scoping ALL already ship, so
# two machine tokens bound to Acme vs Globex exercise the resolution seam
# end-to-end with mostly-shipped machinery plus only the NEW `0002` migration.
# The web-board session leg (the SessionUser active-workspace EXTEND + switcher,
# ADR-005) is Slice 3 and is explicitly OUT of this slice.
#
# Driving adapter: the JSON API served by foundry-api over real HTTP, reached at
#   GET /api/v1/teams/{team}/projects/{project}/issues
# authenticated by the SHIPPED MachinePrincipal bearer extractor whose
# `token.workspace_id` is the acting workspace (ADR-001).
#
# Driven adapters exercised (LAYER 3, @real-io): real Postgres (workspaces,
# users, teams, projects, issues, workspace_memberships, machine_tokens) via
# testcontainers + per-scenario schema; the real Ed25519 verifier; the
# in-process axum router (the SAME InProcHarness the Feature-A + token-management
# scenarios use). The `0002_multi_workspace.sql` migration runs as part of the
# per-scenario schema migration set.
#
# RED-state contract (DISTILL, ADR-025 / Mandate 7): the `0002` migration that
# DROPS `uniq_one_workspace` does NOT exist yet, AND the application-level 409
# guard in `create_workspace` (bootstrap.rs:289, see design/upstream-changes.md
# Finding 1) still forbids a second workspace. So today a SECOND
# `INSERT INTO workspaces` FAILS on the unique index. The two-workspace seeding
# Given steps therefore FAIL RED on the second insert until DELIVER ships the
# migration — this is MISSING_FUNCTIONALITY, not BROKEN. Once `0002` drops the
# guard, the second insert succeeds and the isolation assertions become the
# behaviour under test.
#
# Per the layered test discipline (Mandates 9 + 11): these are LAYER-3
# real-adapter scenarios, so example-based (NOT property-based) and any sad path
# is enumerated explicitly. No PBT machinery at this layer. Mandate 8 state-delta
# is layers 1-3; at layer 3 (real subprocess/HTTP) traditional assertions over
# port-exposed observables are used per the Layered Test Discipline table.
#
# Scope: SLICE 1 ONLY. Cross-tenant evil-user refusal hardening (Slice 2-4),
# the session/web leg (Slice 3), migration-as-user-guarantee (Slice 5), and
# provisioning (Slice 6) are explicitly OUT — do not add them here.
#
# All scenarios except the @walking_skeleton first one are @pending (one-at-a-
# time DELIVER cycle; DELIVER unskips one scenario per RED→GREEN→COMMIT cycle).

@multi-workspace-tenancy @mwt-slice-01 @real-io @driving_adapter
Feature: Two workspaces coexist in one instance and each request sees only its own data
  Sasha runs two independent tenants — "Acme" and "Globex" — in one Foundry
  instance. A member of Acme who lists a project's issues as data sees ONLY
  Acme's issues; a member of Globex sees ONLY Globex's; a brand-new workspace
  starts empty rather than inheriting a neighbour's data; and a request that
  resolves to no workspace is refused rather than served against an arbitrary
  one. Proven with REAL coexisting two-workspace fixtures (real members, real
  tokens, real issues), not synthetic ids.

  Background:
    Given workspace "Acme" exists with admin "ops@acme.com"
    And workspace "Globex" exists with admin "ops@globex.com"

  @walking_skeleton @wiring_e2e @us-mwt01
  Scenario: A member of one workspace lists only their own workspace's issues
    Given "Acme" has a member "marco@acme.com" in team "Backend" with project "Auth" prefix "ACME"
    And "Globex" has a member "lucia@globex.com" in team "Platform" with project "Core" prefix "GLOBEX"
    And the "Acme" project "Auth" has issues ACME-1 and ACME-2
    And the "Globex" project "Core" has issues GLOBEX-1 and GLOBEX-2
    And a machine credential is bound to "marco@acme.com" in workspace "Acme"
    When the Acme-bound credential lists the "Auth" project's issues as data
    Then the answer lists only the "Acme" issues ACME-1 and ACME-2
    And no "Globex" issue appears in the answer

  @us-mwt01 @pending
  Scenario: Each workspace's members see a disjoint set of data
    Given "Acme" has a member "marco@acme.com" in team "Backend" with project "Auth" prefix "ACME"
    And "Globex" has a member "lucia@globex.com" in team "Platform" with project "Core" prefix "GLOBEX"
    And the "Acme" project "Auth" has issues ACME-1 and ACME-2
    And the "Globex" project "Core" has issues GLOBEX-1 and GLOBEX-2
    And a machine credential is bound to "marco@acme.com" in workspace "Acme"
    And a machine credential is bound to "lucia@globex.com" in workspace "Globex"
    When the Acme-bound credential lists the "Auth" project's issues as data
    And the Globex-bound credential lists the "Core" project's issues as data
    Then the Acme answer contains only "Acme" issues
    And the Globex answer contains only "Globex" issues
    And neither answer contains any of the other workspace's issues

  @us-mwt00 @coexistence @pending
  Scenario: A second workspace can be created where none could before
    Given an instance that already has the workspace "Acme"
    When the workspace "Globex" is created alongside it
    Then both workspaces exist on the instance
    And neither creation is blocked by a single-workspace limit

  @us-mwt01 @pending
  Scenario: A brand-new workspace starts empty, not populated from a neighbour
    Given "Acme" has a member "marco@acme.com" in team "Backend" with project "Auth" prefix "ACME"
    And the "Acme" project "Auth" has issues ACME-1 and ACME-2
    And "Globex" has a member "lucia@globex.com" in team "Platform" with project "Core" prefix "GLOBEX"
    And a machine credential is bound to "lucia@globex.com" in workspace "Globex"
    When the Globex-bound credential lists the "Core" project's issues as data
    Then the answer is an empty data list for the new workspace
    And no other workspace's issues appear

  @us-mwt00 @error @pending
  Scenario: A request that resolves to no workspace is refused, not defaulted
    Given "Acme" has a member "marco@acme.com" in team "Backend" with project "Auth" prefix "ACME"
    And the "Acme" project "Auth" has issues ACME-1 and ACME-2
    And a credential whose holder belongs to no workspace
    When that credential lists the "Auth" project's issues as data
    Then the request is refused
    And it is not served against any workspace's data

  @us-mwt00 @migration @no-rewrite @pending
  Scenario: Dropping the single-workspace guard leaves the existing workspace's data unchanged
    Given the existing workspace "Acme" with its issues recorded before the guard is dropped
    When the single-workspace guard is dropped so a second workspace becomes possible
    Then the "Acme" workspace's identity is unchanged
    And every "Acme" issue recorded beforehand is present and unchanged afterward
