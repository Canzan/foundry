# machine-token-admin-ux — US-MT01 one-time display invariant (NFR-MT-SEC-01/02).
# The token value is shown EXACTLY ONCE at mint and is NEVER persisted, logged,
# or re-displayed. Chained narrative (Pillar 2): each scenario's Given reuses the
# mint of us-mt01.
@machine-token-admin @us-mt01 @real-io
Feature: A minted token value is never shown again

  Background:
    Given a workspace "Acme" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"

  Scenario: The token value is nowhere on the surface after issuance
    Given the admin has just issued a token labelled "Slack relay" and left the issuance view
    When the admin returns to the token surface
    Then the token value is nowhere on the surface
    And only the token's id, label, scope, expiry, and status are shown

  @error
  Scenario: Losing the token before copying has no recovery except reissue
    Given the admin has just issued a token labelled "Slack relay" and left the issuance view
    When the admin looks for the token value again
    Then the token value cannot be retrieved anywhere
    And the guidance says to revoke that token and issue a new one

  Scenario: The token value is never written to the registry
    Given the admin has just issued a token labelled "Slack relay" and left the issuance view
    When the registry record for that token is inspected
    Then the record holds only the token's id and metadata
    And the record holds no token value
