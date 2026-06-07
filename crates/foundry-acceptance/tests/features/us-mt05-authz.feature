# machine-token-admin-ux — Slice 4 (US-MT05): mint/list/revoke are
# workspace-admin-only (is_workspace_admin), non-enumerable refusal. The
# adversarial boundary the whole feature stands on (NFR-MT-SEC-03).
@machine-token-admin @us-mt05 @real-io
Feature: Only workspace admins can manage machine tokens

  Background:
    Given a workspace "Acme" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"

  Scenario: A workspace admin can open the token surface
    Given the admin is signed in to the token surface on an issuer-configured server
    When the admin opens the token surface
    Then the token surface is shown

  @error
  Scenario: A non-admin member is refused without learning the surface exists
    Given the member is signed in to the token surface on an issuer-configured server
    When the member opens the token surface
    Then the member is refused without revealing whether the surface exists

  @error
  Scenario: A non-admin member cannot issue a token
    Given the member is signed in to the token surface on an issuer-configured server
    When the member attempts to issue a token labelled "Sneaky bot"
    Then the member is refused without revealing whether the surface exists
    And no token value is shown

  @error
  Scenario: A non-admin member cannot revoke a token
    Given the workspace "Acme" has an issued token labelled "Acme bot"
    And the member is signed in to the token surface on an issuer-configured server
    When the member tries to revoke the "Acme bot" token
    Then the member is refused without revealing whether the surface exists
    And the row for "Acme bot" remains active
