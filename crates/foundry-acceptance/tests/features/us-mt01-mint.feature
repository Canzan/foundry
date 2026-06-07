# machine-token-admin-ux — Slice 1 (Walking Skeleton): mint ONE token
# end-to-end, value shown exactly once. US-MT00 (signer-in-AppState +
# created_by migration, @infrastructure, folded) + US-MT01 (mint).
#
# Driving port: the browser admin surface at /admin/tokens (real HTTP through
# the in-process axum harness — same router build_router production uses). The
# minted token is cross-checked against the SHIPPED /api/v1 verify path to prove
# real signing (US-MT01 AC: "a token issued this way authenticates").
@machine-token-admin @us-mt01 @real-io
Feature: An admin issues a machine token and sees its value exactly once

  Background:
    Given a workspace "Acme" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"

  # THE walking skeleton — the riskiest, highest-value path: server-side Ed25519
  # signing surfaced safely through the admin surface, with the one-time secret
  # display, and the minted token genuinely working against the API.
  @walking_skeleton @us-mt00
  Scenario: An admin issues a working token and sees its value once
    Given the admin is signed in to the token surface on an issuer-configured server
    When the admin issues a token labelled "CI bot — files release issues"
    Then the issued token value is shown exactly once with a copy affordance and an only-time warning
    And the issued token shows its id, label, scope, and expiry
    And the issued token authenticates against the API

  @us-mt00
  Scenario: Issuing records who minted the token
    Given the admin is signed in to the token surface on an issuer-configured server
    When the admin issues a token labelled "Release bot"
    Then the token list attributes "Release bot" to the admin

  @error @us-mt00
  Scenario: Issuing is refused gracefully where it is not enabled
    Given the admin is signed in to the token surface on a verifier-only server
    When the admin opens the token surface
    Then issuing is reported as not enabled on this server
    And no mint form is offered
    And the server does not error

  @error @us-mt00
  Scenario: Issuing is refused gracefully when a mint is attempted on a verifier-only server
    Given the admin is signed in to the token surface on a verifier-only server
    When the admin attempts to issue a token labelled "Doomed bot"
    Then issuing is reported as not enabled on this server
    And no token value is shown
    And the server does not error

  @error
  Scenario: Issuing without a label is refused
    Given the admin is signed in to the token surface on an issuer-configured server
    When the admin attempts to issue a token with no label
    Then issuance is refused as invalid
    And no token value is shown
