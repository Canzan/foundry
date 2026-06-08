# Feature: token-management-api — the machine-facing JSON counterpart to the
# shipped machine-token-admin-ux web UI. A bearer-authenticated /api/v1 adapter
# over the SHIPPED, mutation-hardened list_tokens / revoke_token use-cases.
# Ratified authz model (DISCUSS Q-AUTHZ -> option c, asymmetric): a machine
# bearer may LIST + REVOKE (incl. revoke-self / rotation), gated by the existing
# is_workspace_admin check on the bound user. MINT is NOT exposed via the API
# (no route); provisioning stays human-session-only via /admin/tokens.
#
# Driving adapter: the JSON API served by foundry-api, reached over real HTTP at
#   GET    /api/v1/teams/{team}/projects/{project}/tokens
#   DELETE /api/v1/teams/{team}/projects/{project}/tokens/{jti}
# authenticated by the SHIPPED MachinePrincipal bearer extractor, refusals via
# the SHIPPED status_for / ErrorBody envelope. See design/api-contract.md,
# design/no-mint-boundary.md, design/rate-guardrail.md.
#
# Driven adapters exercised (LAYER 3, @real-io): real Postgres (machine_tokens
# registry + jti denylist, workspaces, users, teams, projects, memberships) via
# testcontainers + per-scenario schema; the real Ed25519 verifier; the in-process
# axum router (the SAME InProcHarness the browser + Feature-A scenarios use).
#
# RED-state contract (DISTILL, ADR-025 / Mandate 7): the /api/v1/.../tokens
# routes are NOT yet merged into build_router (foundry-api has issue/comment
# routes only). Background + Given steps set up REAL preconditions (workspace,
# admin/member, seeded token rows, minted bearer) via the shipped helpers — they
# MUST succeed. When steps issue a REAL HTTP request to /api/v1/.../tokens and
# capture the response. Then steps assert the JSON outcome and FAIL RED (the
# route 404s today). This is MISSING_FUNCTIONALITY, not BROKEN. DELIVER unskips,
# merges foundry_api::routes(state) with the two token routes + the TokenJson
# shape + the rate guardrail, and the assertions flip GREEN.
#
# Per the layered test discipline: these are LAYER-3 real-adapter scenarios, so
# example-based (NOT property-based) and sad paths are enumerated explicitly
# (Mandate 9 + 11). No PBT machinery at this layer.
#
# All scenarios except the @walking_skeleton first one are @pending (one-at-a-
# time DELIVER cycle). The @rate-guardrail burst scenario is @pending pending
# the OD-TMA-1/OD-TMA-5 ratification of the bucket mechanism — see
# distill/upstream-issues.md (the one flagged open item).

@token-management-api @real-io @driving_adapter
Feature: A machine manages its workspace's tokens over the JSON API
  An integrator, a rotation job, or an audit pipeline points a bearer credential
  at the token-management surface and inventories, revokes, and rotates the
  workspace's machine tokens as data — without a browser, a session, or database
  access. A management-capable caller (bound to a workspace admin) is allowed; a
  non-management caller is refused without leaking the registry; minting a new
  credential is not possible from a bearer at all. The surface is non-enumerable,
  workspace-confined, and rate-guarded against a revoke storm.

  Background:
    Given a workspace "Acme" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"

  # ======================================================================
  # Slice 1 — Walking Skeleton: prove the authz model on the SAFEST op (LIST)
  # US-TMA00 (route group + authz-gate seam) + US-TMA01 (GET list)
  # ======================================================================

  @walking_skeleton @us-tma00 @us-tma01
  Scenario: An audit pipeline lists the workspace's tokens as data
    Given the workspace "Acme" has a managed token "ci-issue-filer" used 4 minutes ago
    And the workspace "Acme" has a managed token "slack-relay" never used
    And an audit pipeline holds a management-capable bearer for "Acme"
    When the pipeline requests the token list over the API
    Then the answer is a token list containing "ci-issue-filer" and "slack-relay"
    And each listed token carries its label, scope, expiry, status, last-used, and who minted it
    And no listed token carries a token value

  @us-tma01 @error
  Scenario: An empty registry answers with an empty list, not an error
    Given the workspace "Acme" has no managed tokens
    And an audit pipeline holds a management-capable bearer for "Acme"
    When the pipeline requests the token list over the API
    Then the answer is an empty token list
    And the token request is reported as successful

  @us-tma01 @error
  Scenario: A non-management caller is refused without leaking the registry
    Given the workspace "Acme" has a managed token "ci-issue-filer" used 4 minutes ago
    And a caller holds a non-management bearer for "Acme"
    When the caller requests the token list over the API
    Then the token request is refused as not allowed
    And no token data is returned by the API

  @us-tma01 @us-tma00 @error
  Scenario: A request with no credential is refused before any token logic runs
    When a caller requests the token list over the API with no bearer credential
    Then the token request is refused as unauthorized
    And no token data is returned by the API

  @us-tma01 @error
  Scenario: The list never exposes a token value
    Given the workspace "Acme" has a managed token "ci-issue-filer" used 4 minutes ago
    And an audit pipeline holds a management-capable bearer for "Acme"
    When the pipeline requests the token list over the API
    Then no field in the token list carries a token, secret, or hash value

  # ======================================================================
  # Slice 2 — Revoke + Rotate: hands-free credential lifecycle
  # US-TMA02 (revoke) + US-TMA03 (revoke-self / rotation)
  # ======================================================================

  @us-tma02 @pending
  Scenario: A rotation job revokes a credential and it is dead on its next call
    Given a credential "leaked-ci" is active in workspace "Acme"
    And a rotation job holds a management-capable bearer for "Acme"
    When the job revokes "leaked-ci" over the API
    Then the revoke is reported as succeeded with no content
    And the next API call made with "leaked-ci" is refused as unauthorized

  @us-tma02 @error @pending
  Scenario: Revoking an already-revoked credential is a harmless success
    Given a credential "old-triage" in workspace "Acme" is already revoked
    And a rotation job holds a management-capable bearer for "Acme"
    When the job revokes "old-triage" over the API again
    Then the revoke is reported as succeeded with no content

  @us-tma02 @error @pending
  Scenario: Revoking a credential from another workspace reveals nothing
    Given a credential exists in another workspace
    And a rotation job holds a management-capable bearer for "Acme"
    When the job attempts to revoke that other workspace's credential over the API
    Then the revoke is refused as not found
    And the refusal is identical to revoking an id that exists nowhere

  @us-tma02 @error @pending
  Scenario: A non-management caller cannot revoke
    Given a credential "leaked-ci" is active in workspace "Acme"
    And a caller holds a non-management bearer for "Acme"
    When the caller attempts to revoke "leaked-ci" over the API
    Then the token request is refused as not allowed
    And the credential "leaked-ci" remains active

  @us-tma03 @pending
  Scenario: A rotation job retires its own credential after promoting a new one
    Given a rotation job holds its own management-capable bearer "rotating-bot" for "Acme"
    When the job revokes its own credential "rotating-bot" over the API
    Then the revoke is reported as succeeded with no content
    And the next API call made with "rotating-bot" is refused as unauthorized

  @us-tma03 @error @pending
  Scenario: Re-running rotation against an already-retired credential is harmless
    Given a credential "rotating-bot" in workspace "Acme" is already revoked
    And a rotation job holds a management-capable bearer for "Acme"
    When the job revokes "rotating-bot" over the API again
    Then the revoke is reported as succeeded with no content

  # ======================================================================
  # Slice 3 — Trust the contract: stable codes + non-enumerable boundary
  # US-TMA04 (stable contract) + US-TMA05 (evil-caller boundary + no-mint + rate)
  # ======================================================================

  @us-tma04 @pending
  Scenario: A listed token reflects its revocation on the next read
    Given the workspace "Acme" has a managed token "slack-relay" never used
    And an audit pipeline holds a management-capable bearer for "Acme"
    When the pipeline revokes "slack-relay" and then lists the tokens again over the API
    Then the listed token "slack-relay" now shows as revoked
    And every other field of "slack-relay" is unchanged from the previous read

  @us-tma04 @error @pending
  Scenario: Every token-route refusal carries a stable machine-readable code
    Given a caller holds a non-management bearer for "Acme"
    When the caller requests the token list over the API
    Then the refusal carries a stable error code and the conventional status
    And the code can be branched on without parsing prose

  @us-tma05 @error @pending
  Scenario: Cross-workspace and unknown ids are indistinguishable
    Given a credential exists in another workspace
    And a rotation job holds a management-capable bearer for "Acme"
    When the job attempts to revoke that other workspace's credential over the API
    And the job attempts to revoke an id that exists nowhere over the API
    Then both attempts return the identical not-found refusal

  @us-tma05 @error @pending
  Scenario: An invalid or revoked credential is refused identically
    Given a caller holds a credential the workspace never issued
    When the caller requests the token list over the API
    Then the token request is refused as unauthorized
    And no token data is returned by the API

  @us-tma05 @error @pending
  Scenario: A credential signed with a disallowed algorithm is refused
    Given a caller holds a token-management credential signed with an algorithm the server does not accept
    When the caller requests the token list over the API
    Then the token request is refused as unauthorized
    And no token data is returned by the API

  @us-tma05 @error @pending
  Scenario: There is no programmatic mint surface to escalate through
    Given an audit pipeline holds a management-capable bearer for "Acme"
    When the caller attempts to mint a token over the API
    Then no programmatic mint route exists
    And no token value is returned by the API

  # The rate guardrail's MECHANISM (in-process per-principal token bucket keyed
  # by bound user_id, C/R values, the adapter-local 429, and the test-only clock-
  # advance affordance the bucket reads) is OD-TMA-1 / OD-TMA-5 — OPEN, awaiting
  # ratification at the post-roadmap checkpoint. This scenario is authored
  # DETERMINISTIC-BY-DESIGN (it drives the SHIPPED state.clock / MockClock to
  # exercise refill — NO wall-clock sleep, NO real-time flake) but is held
  # @pending until the bucket mechanism is ratified, so DELIVER does not race a
  # half-specified guardrail. See distill/upstream-issues.md (the one open item).
  @us-tma05 @rate-guardrail @pending
  Scenario: A burst of revocations beyond the guardrail is throttled
    Given a rotation job holds a management-capable bearer for "Acme"
    And the workspace "Acme" has a managed token for every revoke in the burst
    When the job issues a burst of revocations beyond the per-principal guardrail over the API
    Then the revocations within the guardrail succeed
    And the revocations beyond the guardrail are refused as too many requests
    And the per-principal mutation rate is observable as a guardrail metric
