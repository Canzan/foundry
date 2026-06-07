# machine-token-admin-ux — Slice 3 (US-MT03): revoke a token so it is refused
# on its next API use. The kill-switch proof cross-checks the SHIPPED per-request
# jti denylist (us-w05b behaviour): revoke → the next /api/v1 call is refused.
@machine-token-admin @us-mt03 @real-io
Feature: An admin revokes a machine token and it dies on its next use

  Background:
    Given a workspace "Acme" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"

  Scenario: A revoked token is refused on its very next API call
    Given the admin has issued a token labelled "Slack relay" that an integration is using
    And the admin is signed in to the token surface on an issuer-configured server
    When the admin revokes the "Slack relay" token
    Then the row for "Slack relay" shows status revoked
    And the integration's next API call with that token is refused

  Scenario: Revoking warns it is immediate and irreversible before it happens
    Given the admin has issued a token labelled "Slack relay" that an integration is using
    And the admin is signed in to the token surface on an issuer-configured server
    When the admin opens the revoke confirmation for "Slack relay"
    Then the confirmation warns the revoke is immediate and cannot be undone

  Scenario: Revoking an already-revoked token is harmless
    Given the workspace "Acme" has an issued token labelled "Old triage agent" that is revoked
    And the admin is signed in to the token surface on an issuer-configured server
    When the admin revokes the "Old triage agent" token again
    Then the revoke succeeds without error
    And the row for "Old triage agent" shows status revoked

  @error
  Scenario: An admin cannot revoke a token outside their workspace
    Given another workspace "Globex" has an issued token labelled "Globex bot" that is active
    And the admin is signed in to the token surface on an issuer-configured server
    When the admin tries to revoke the "Globex" workspace's token
    Then the revoke is refused without revealing whether that token exists
    And the "Globex" workspace's token remains active

  @error
  Scenario: A revoke without a valid anti-forgery token is refused
    Given the admin has issued a token labelled "Slack relay" that an integration is using
    And the admin is signed in to the token surface on an issuer-configured server
    When the admin submits a revoke for "Slack relay" with no anti-forgery token
    Then the revoke is refused as forbidden
    And the row for "Slack relay" remains active
