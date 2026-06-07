# machine-token-admin-ux — Slice 4 (US-MT06): the list shows "minted by {admin}"
# (created_by) and "last used" (last_used_at) for audit + staleness triage.
@machine-token-admin @us-mt06 @real-io
Feature: The token list attributes each token and shows whether it is still used

  Background:
    Given a workspace "Acme" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"

  Scenario: The list attributes each token to who issued it
    Given the admin issued the "CI bot" token and another admin "dana@acme.com" issued the "Old triage agent" token
    And the admin is signed in to the token surface on an issuer-configured server
    When the admin opens the token surface
    Then "CI bot" shows it was minted by "devansh@acme.com"
    And "Old triage agent" shows it was minted by "dana@acme.com"

  Scenario: The list shows whether a token is still being used
    Given the "CI bot" token was used recently and a freshly issued token has never been used
    And the admin is signed in to the token surface on an issuer-configured server
    When the admin opens the token surface
    Then "CI bot" shows a recent last-used time
    And the freshly issued token shows it has never been used

  @error
  Scenario: A token issued before issuer attribution shows an unknown issuer
    Given the workspace "Acme" has a token with no recorded issuer
    And the admin is signed in to the token surface on an issuer-configured server
    When the admin opens the token surface
    Then that token shows its issuer as unknown
