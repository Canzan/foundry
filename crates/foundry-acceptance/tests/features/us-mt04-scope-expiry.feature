# machine-token-admin-ux — Slice 4 (US-MT04): choose scope + expiry within
# server-enforced bounds (DD8: default 90 days, cap 365 days; DD9: workspace vs
# team scope). Least-privilege, time-bounded grants.
@machine-token-admin @us-mt04 @real-io
Feature: An admin issues scoped, time-bounded tokens within server bounds

  Background:
    Given a workspace "Acme" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"

  Scenario: An admin issues a team-scoped, time-bounded token
    Given the admin is signed in to the token surface on an issuer-configured server
    When the admin issues a token labelled "CI bot" scoped to the "Backend" team for 30 days
    Then the issued token is limited to the "Backend" team
    And the issued token expires in 30 days
    And the token list shows the "Backend" scope for "CI bot"

  Scenario: An expiry exactly at the cap is accepted
    Given the admin is signed in to the token surface on an issuer-configured server
    When the admin issues a token labelled "Yearly bot" for 365 days
    Then the issued token value is shown exactly once with a copy affordance and an only-time warning

  @error
  Scenario: An expiry beyond the server cap is refused with the maximum stated
    Given the admin is signed in to the token surface on an issuer-configured server
    When the admin attempts to issue a token labelled "Forever bot" for 400 days
    Then issuance is refused with the maximum expiry stated
    And no token value is shown

  @error
  Scenario: A scope that is not part of the workspace is refused
    Given another workspace "Globex" owns a team "Outsiders"
    And the admin is signed in to the token surface on an issuer-configured server
    When the admin attempts to issue a token scoped to the "Outsiders" team
    Then issuance is refused as invalid
    And no token value is shown
