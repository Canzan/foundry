# Story: US-W05b — Authenticate programmatic clients with a machine token
# Feature A "Programmatic Foundry" — Slice 2
# JTBD: jtbd-web-4 (drive Foundry programmatically, unattended)
#
# Driving adapter: the JSON API, authenticated by a machine token presented
# as a bearer credential — see design/auth.md (JWT/Ed25519, jti denylist,
# alg pinned to EdDSA, fail-closed verification) and design/api-contract.md
# (Authorization: Bearer header; 401/403 status conventions).
# Driven adapters exercised: real Postgres (machine_tokens registry/denylist,
# teams, projects, issues, memberships), the Ed25519 verifier in core.
#
# Auth is mostly sad paths by nature: a valid credential is one row of the
# truth table; missing / malformed / forged / expired / revoked / wrong-alg /
# out-of-scope are all refusals. Error/edge ratio for this feature is high by
# design (NFR-WEB-API-SEC-01..03, US-W05b scenarios 2-5).
#
# NON-ENUMERABLE refusal: every credential failure is refused the same way,
# with no detail distinguishing why (design/error-and-observability.md). The
# scenarios assert the refusal and the no-data-leak, never an enumerable reason.

@feature-a @us-w05b @driving_adapter
Feature: A machine authenticates to the API with its own credential
  A workspace admin grants a machine its own credential, distinct from any
  human's browser session. A request bearing a valid credential is treated as
  that machine and may read what its bound principal may read. A request whose
  credential is missing, malformed, forged, expired, or revoked is refused as
  unauthenticated; a request whose credential is valid but reaches beyond its
  granted scope is refused as not-allowed. The browser sign-in path is
  unaffected by the new credential surface.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"
    And a project "Auth v2" with key prefix "AUTH" exists in the "Backend" team
    And the "Auth v2" project has issue AUTH-2 titled "Refresh token rotation" in progress

  @slice2-entry @real-io @driving_adapter
  Scenario: A machine reads with its granted credential
    Given the admin has granted a machine credential for "Devansh's dashboard" bound to Mei
    When the machine requests the "Auth v2" board's issues with that credential
    Then the request is authenticated as the machine
    And the board's issues are returned as data

  @real-io
  Scenario: A machine credential needs no browser session and no anti-forgery token
    Given the admin has granted a machine credential for "Devansh's dashboard" bound to Mei
    When the machine requests the board's issues carrying only its credential and no session and no anti-forgery token
    Then the request succeeds

  @error @real-io
  Scenario: A request with no credential is refused
    When a caller requests the board's issues carrying no credential
    Then the request is refused as unauthenticated
    And no issue data is returned

  @error @real-io
  Scenario: A malformed credential is refused
    When a caller requests the board's issues carrying a malformed credential
    Then the request is refused as unauthenticated
    And no issue data is returned

  @error @real-io
  Scenario: A forged credential the registry never issued is refused
    Given a caller holds a credential the workspace never issued
    When the caller requests the board's issues with that credential
    Then the request is refused as unauthenticated
    And no issue data is returned

  @error @real-io
  Scenario: An expired credential is refused
    Given the admin granted a machine credential bound to Mei that has since expired
    When the machine requests the board's issues with that expired credential
    Then the request is refused as unauthenticated
    And no issue data is returned

  @error @real-io
  Scenario: A revoked credential is refused on its next use
    Given the admin has granted a machine credential for "Devansh's dashboard" bound to Mei
    And the admin revokes that credential
    When the machine next requests the board's issues with that credential
    Then the request is refused as unauthenticated
    And no issue data is returned

  @error @real-io @nfr-web-api-sec-02
  Scenario: A credential signed with a disallowed algorithm is refused
    Given a caller holds a credential signed with an algorithm the server does not accept
    When the caller requests the board's issues with that credential
    Then the request is refused as unauthenticated
    And no issue data is returned

  @error @real-io @nfr-web-api-sec-02
  Scenario: A credential cannot reach beyond the team it was scoped to
    Given the team "Platform" owns a project "Billing" with key prefix "BILL"
    And the admin has granted a machine credential bound to Mei scoped to the "Backend" team
    When the machine requests the "Billing" board's issues with that credential
    Then the request is refused as not-allowed
    And no issue data is returned

  @real-io @nfr-web-api-sec-01
  Scenario: The browser sign-in path is unchanged by the machine credential surface
    Given a member account for "mei@acme.com" with password "correct horse battery staple"
    When Mei signs in through the browser with her email and password
    Then she receives a session cookie as before
    And her browser session still requires an anti-forgery token on a mutating request
