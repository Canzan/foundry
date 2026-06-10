# Feature: multi-workspace-tenancy — Slice 3: propagate the isolation boundary to
# the JSON /api/v1 + machine-token + sign-in/session-resolution surfaces. Slice 1
# proved coexistence + resolution on ONE API read path (issues list); slice 2
# generalised the boundary to the WEB htmx tier (read/write/admin/refusal +
# switcher). This slice EXTENDS the boundary to the REMAINING /api/v1 surfaces
# (issue WRITE; token list/revoke) and asserts the session-resolution CONTRACT
# (US-MWT04) directly — that a session resolves to EXACTLY one acting workspace,
# fail-closed when none.
#
# Hypothesis (slices/slice-03-api-and-auth-boundary.md): the SAME boundary proven
# on the web tier holds for a machine-token bearer (workspace bound by
# `machine_tokens.workspace_id`, ADR-001 API leg) and for session resolution — an
# Acme-bound token cannot touch Globex (refused non-enumerably), and a session
# resolves to exactly one workspace (single-membership automatically;
# multi-membership to the chosen one; none → refused). Disproved if an Acme token
# reaches Globex, or a session is defaulted silently to an arbitrary tenant.
#
# Driving adapter: the JSON API served by foundry-api over real HTTP, reached at
#   GET    /api/v1/teams/{team}/projects/{project}/issues          (read, slice-1)
#   POST   /api/v1/teams/{team}/projects/{project}/issues          (WRITE, this slice)
#   GET    /api/v1/teams/{team}/projects/{project}/tokens          (token list)
#   DELETE /api/v1/teams/{team}/projects/{project}/tokens/{jti}    (token revoke)
# authenticated by the SHIPPED MachinePrincipal bearer extractor whose
# `token.workspace_id` is the acting workspace (ADR-001 API leg). The session
# leg is exercised at the resolution-contract level via the SHIPPED
# `resolve_active_workspace` seam (ADR-005), reusing the web sign-in path.
#
# Driven adapters exercised (LAYER 3, @real-io): real Postgres (workspaces, users,
# workspace_memberships, teams, team_memberships, projects, issues, machine_tokens)
# via testcontainers + per-scenario schema; the real Ed25519 verifier + per-request
# jti denylist; the in-process axum router (the SAME InProcHarness slices 1-2 and
# the token-management-api scenarios use). The `0002_multi_workspace.sql` migration
# runs as part of the per-scenario schema migration set.
#
# Refusal-status decision (ADR-003 / OD-MWT-D6, confirmed): on /api/v1, a request
# for a resource OUTSIDE the acting workspace returns the SHIPPED `status_for` 404
# JSON envelope, byte-identical to a never-existed id — generalising the shipped
# `find_*_in_workspace → None` idiom. A foreign-jti revoke is already the SHIPPED
# non-enumerable `NotFound` 404 (tokens.rs:267, reused as-is). Cross-tenant access
# NEVER 403s (a 403-vs-404 difference would be an existence oracle). The shipped
# bearer 401 (auth.md) and the non-enumerable sign-in error are UNCHANGED — this
# slice scopes WHICH workspace a principal acts on, not how a token is verified.
#
# Residual closure (NFR-MWT-TEST-01 / DM8 / docs/evolution 2026-06-08): the
# token-management-api `us-tma` scenarios test cross-workspace non-enumerability
# with a SYNTHETIC random uuid as the "foreign" jti (feature_token_management_api
# `credential_in_another_workspace`, because under `uniq_one_workspace` no real
# second workspace was insertable). Scenarios 5 + 6 below CONVERT that synthetic
# residual to REAL two-workspace fixtures: a real Globex-bound token's jti is the
# foreign target, and the Globex token is asserted STILL ACTIVE after the refused
# cross-tenant revoke.
#
# RED-state contract (DISTILL, ADR-025 / Mandate 7): the crate COMPILES (no
# import/collection error → not BROKEN). At runtime against the real testcontainers
# PG16, the genuine RED is MISSING_FUNCTIONALITY:
#   1. The Background seeds Acme then Globex; the SECOND `INSERT INTO workspaces`
#      FAILS on `uniq_one_workspace` (0001_init.sql:15) until DELIVER ships `0002`
#      (shared with slices 1-2 — GREEN by inheritance once `0002` ships).
#   2. The remaining /api/v1 scoping (issue WRITE via `create_issue` +
#      `insert_issue_with_outbox` bound to the acting workspace; token list/revoke
#      via `list_tokens(principal.workspace_id())` / `revoke_token`'s
#      `row.workspace_id != principal.workspace_id() ⇒ NotFound`) is ALREADY
#      shipped + 100%-mutation-hardened — it has simply never been exercised under
#      a genuinely-coexisting second workspace. So once `0002` lets two workspaces
#      coexist, the confinement scenarios prove the SHIPPED behaviour holds under
#      real cross-tenant fixtures (green-by-inheritance behind the `0002` gate).
#   3. US-MWT04 session resolution (`resolve_active_workspace`, ADR-005:
#      single-membership auto, multi-membership persisted-active, zero → None
#      fail-closed) is shipped at the store/sign-in seam by slices 1-2; these
#      scenarios assert that CONTRACT directly at the session/API level (not the
#      web switcher UI, which slice 2 covers).
#
# Per the layered test discipline (Mandates 9 + 11): LAYER-3 real-adapter
# scenarios → example-based (NOT property-based); every sad/evil-user path is
# enumerated explicitly; no PBT machinery at this layer. Mandate 8 state-delta is
# layers 1-3; at layer 3 traditional assertions over port-exposed observables
# (listed issue keys, listed token labels/jtis, HTTP refusal status + body
# identity, post-write workspace-scoped DB row presence, post-revoke
# `revoked_at` state) are used per the Layered Test Discipline table (matching
# slices 1-2's precedent; no `state_delta.rs` Rust port exists — Python is the
# canonical pilot).
#
# Scope: SLICE 3 ONLY — the JSON /api/v1 remaining surfaces + machine-token +
# sign-in/session-resolution contract. The full uniform non-enumerability matrix
# across ALL surfaces + the adversarial timing/shape matrix (Slice 4),
# migration-as-guarantee (Slice 5), and provisioning (Slice 6) are explicitly OUT
# — do not add them here. Slice 1 (API issues READ scoped by token.workspace_id)
# and slice 2 (web session resolution + switcher + uniform-404 + LAYER-1e guard)
# are NOT re-authored; this slice references them and proves the REMAINING surfaces.
#
# All scenarios except the first @walking_skeleton one are @pending (one-at-a-time
# DELIVER cycle; DELIVER unskips one scenario per RED→GREEN→COMMIT cycle).

@multi-workspace-tenancy @mwt-slice-03 @real-io @driving_adapter
Feature: A machine token or API caller bound to one workspace acts only on that workspace
  Marco's Acme-bound machine token reads and writes only Acme's resources over
  /api/v1; a call targeting a Globex resource is refused identically to a
  never-existed one. The Acme token lists only Acme's tokens and can revoke only
  Acme's — a real Globex jti is refused as not-found and the Globex token stays
  active. A signed-in session resolves to exactly one acting workspace
  (single-membership automatically, multi-membership to the chosen one) and is
  refused, never defaulted, when none resolves. The shipped verify path — iss/aud
  EdDSA pinning + the per-request jti denylist — is unchanged. Proven with REAL
  coexisting Acme/Globex fixtures (real members, real tokens, real issues), not
  synthetic ids.

  Background:
    Given workspace "Acme" exists with admin "ops@acme.com"
    And workspace "Globex" exists with admin "ops@globex.com"
    And "Acme" has a member "marco@acme.com" in team "Backend" with project "Auth" prefix "ACME"
    And "Globex" has a member "lucia@globex.com" in team "Platform" with project "Core" prefix "GLOBEX"
    And the "Acme" project "Auth" has issues ACME-1 and ACME-2
    And the "Globex" project "Core" has issues GLOBEX-1 and GLOBEX-2

  # ----------------------------------------------------------------------------
  # 1. Walking skeleton — the demo-able confinement proof on the API WRITE path.
  #    "An Acme-bound token files an issue; it lands in Acme and only Acme."
  #    (Slice 1 proved the READ path; this is the first WRITE-path confinement.)
  # ----------------------------------------------------------------------------
  @walking_skeleton @wiring_e2e @us-mwt03
  Scenario: A workspace-bound token's write lands only in its own workspace
    Given a machine credential is bound to "marco@acme.com" in workspace "Acme"
    When the Acme-bound credential files issue "Rotate signing keys" in the "Auth" project over the API
    Then the write is reported as created
    And the new issue exists only in "Acme"
    And no issue was created in "Globex"

  # ----------------------------------------------------------------------------
  # 2. Confined READ — the Acme token lists only Acme issues over the API.
  #    (References slice-1's read proof; here it rides the slice-3 Background so
  #    every surface shares one coexisting fixture.)
  # ----------------------------------------------------------------------------
  @us-mwt03
  Scenario: A workspace-bound token reads only its own workspace's issues over the API
    Given a machine credential is bound to "marco@acme.com" in workspace "Acme"
    When the Acme-bound credential lists the "Auth" project's issues as data
    Then the answer lists only the "Acme" issues ACME-1 and ACME-2
    And no "Globex" issue appears in the answer

  # ----------------------------------------------------------------------------
  # 3. Cross-tenant READ refusal (evil-user, the security core) — an Acme token
  #    reaching a real Globex project is refused IDENTICALLY to a never-existed one.
  # ----------------------------------------------------------------------------
  @us-mwt03 @error
  Scenario: A cross-workspace API read is refused non-enumerably
    Given a machine credential is bound to "marco@acme.com" in workspace "Acme"
    When the Acme-bound credential lists the "Core" project's issues over the API by its real address
    And the Acme-bound credential lists a project's issues that never existed over the API
    Then the two API responses are refused identically
    And nothing in the API response reveals the "Globex" project exists

  # ----------------------------------------------------------------------------
  # 4. Cross-tenant WRITE refusal (evil-user) — an Acme token writing into a real
  #    Globex project is refused identically to a never-existed one, and no Globex
  #    issue is created.
  # ----------------------------------------------------------------------------
  @us-mwt03 @error
  Scenario: A cross-workspace API write is refused non-enumerably
    Given a machine credential is bound to "marco@acme.com" in workspace "Acme"
    When the Acme-bound credential files issue "Sneaky" in the "Core" project over the API by its real address
    And the Acme-bound credential files issue "Sneaky" in a project that never existed over the API
    Then the two API responses are refused identically
    And no issue was created in "Globex"

  # ----------------------------------------------------------------------------
  # 5. Token LIST confined to the acting workspace (RESIDUAL CLOSURE) — an Acme
  #    token listing tokens sees ONLY Acme's, never Globex's. Replaces the
  #    synthetic-uuid `us-tma` proof with a REAL two-workspace fixture.
  # ----------------------------------------------------------------------------
  @us-mwt03 @error
  Scenario: A workspace-bound token lists only its own workspace's tokens
    Given a machine credential is bound to "ops@acme.com" in workspace "Acme"
    And a managed token "acme-ci" exists in workspace "Acme"
    And a managed token "globex-ci" exists in workspace "Globex"
    When the Acme-bound credential lists the workspace's tokens over the API
    Then the token list contains "acme-ci"
    And the token list does not contain "globex-ci"

  # ----------------------------------------------------------------------------
  # 6. Token REVOKE confined to the acting workspace (RESIDUAL CLOSURE, the KEY
  #    item) — an Acme token revoking a REAL Globex jti is refused as not-found
  #    identically to an id that exists nowhere, and the Globex token STAYS ACTIVE.
  # ----------------------------------------------------------------------------
  @us-mwt03 @error
  Scenario: A workspace-bound token cannot revoke another workspace's token
    Given a machine credential is bound to "ops@acme.com" in workspace "Acme"
    And a managed token "globex-ci" exists in workspace "Globex"
    When the Acme-bound credential revokes the "Globex" token "globex-ci" over the API
    And the Acme-bound credential revokes a token id that exists nowhere over the API
    Then the two API revoke responses are refused identically as not found
    And the "Globex" token "globex-ci" remains active

  # ----------------------------------------------------------------------------
  # 7. Session resolution — single membership auto-resolves (US-MWT04 sc 1).
  #    Asserted at the resolution-contract level: a single-membership user's
  #    session acts on their one workspace with no choice step.
  # ----------------------------------------------------------------------------
  @us-mwt04
  Scenario: A single-membership session resolves to exactly one workspace automatically
    Given "marco@acme.com" belongs to exactly one workspace "Acme"
    When his session's acting workspace is resolved
    Then the session resolves to exactly the workspace "Acme"
    And no workspace choice was required

  # ----------------------------------------------------------------------------
  # 8. Session resolution — multi-membership resolves to exactly the chosen one
  #    (US-MWT04 sc 2). Distinct from slice-2's web SWITCH scenarios: here the
  #    invariant under test is "resolution yields EXACTLY one", at the seam.
  # ----------------------------------------------------------------------------
  @us-mwt04
  Scenario: A multi-membership session resolves to exactly the chosen workspace
    Given "dana@contract.dev" is also a member of "Acme" in team "Backend" with project "Auth" prefix "ACME"
    And "dana@contract.dev" is also a member of "Globex" in team "Platform" with project "Core" prefix "GLOBEX"
    And "dana@contract.dev" has chosen "Globex" as their active workspace
    When her session's acting workspace is resolved
    Then the session resolves to exactly the workspace "Globex"
    And her session is scoped to exactly one workspace

  # ----------------------------------------------------------------------------
  # 9. Session resolution — fail-closed when none resolves (US-MWT04 sc 3).
  #    A user who belongs to no workspace resolves to NO workspace; the session is
  #    refused, never defaulted to an arbitrary tenant.
  # ----------------------------------------------------------------------------
  @us-mwt04 @error
  Scenario: A session that resolves to no workspace is refused, not defaulted
    Given "evicted@nowhere.test" belongs to no workspace
    When their session's acting workspace is resolved
    Then no workspace is resolved
    And the session is not scoped to any workspace

  # ----------------------------------------------------------------------------
  # 10. Verify-path-unchanged regression (NFR invariant) — multi-workspace did NOT
  #     weaken token verification: a token whose jti is on the per-request denylist
  #     (revoked) is refused 401, and a token signed with a disallowed algorithm is
  #     refused 401, exactly as before this feature. The iss/aud/EdDSA pinning +
  #     the jti denylist still hold under two coexisting workspaces.
  # ----------------------------------------------------------------------------
  @us-mwt03 @error @verify-path-unchanged
  Scenario: The shipped token verify path and jti denylist are unchanged under multi-workspace
    Given a machine credential is bound to "marco@acme.com" in workspace "Acme"
    And that credential has been revoked
    When the revoked credential lists the "Auth" project's issues as data
    Then the request is refused as unauthorized by the verify path
    And a credential signed with a disallowed algorithm is also refused as unauthorized
