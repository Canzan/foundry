# machine-token-admin-ux — Slice 2 (US-MT02): list the workspace's issued
# machine tokens. Read-only over list_machine_tokens (shipped), newest first;
# no secret shown; workspace-isolated; inviting empty state.
@machine-token-admin @us-mt02 @real-io
Feature: An admin sees the workspace's issued machine tokens

  Background:
    Given a workspace "Acme" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"

  Scenario: A reviewer sees the workspace's issued tokens newest first
    Given the workspace "Acme" has three issued tokens, one of them revoked
    And the admin is signed in to the token surface on an issuer-configured server
    When the admin opens the token surface
    Then the surface lists all three tokens, newest first
    And each row shows its label, scope, expiry, and status
    And no token value appears anywhere in the list

  # Single-workspace constraint (uniq_one_workspace, see distill/upstream-issues.md):
  # a real second workspace cannot be seeded in slice 1, so registry isolation is
  # exercised at the read boundary — the list query is scoped to the acting
  # workspace_id, and a credential bound to a DIFFERENT (synthetic) workspace_id
  # never appears. The behaviour under test is "the list is workspace-scoped".
  @error
  Scenario: The list is scoped to the acting workspace
    Given the workspace "Acme" has an issued token labelled "Acme bot"
    And a registry credential exists bound to a different workspace
    And the admin is signed in to the token surface on an issuer-configured server
    When the admin opens the token surface
    Then the surface lists "Acme bot"
    And the surface lists only the acting workspace's tokens

  Scenario: An empty workspace shows guidance, not a blank table
    Given the workspace "Acme" has no issued tokens
    And the admin is signed in to the token surface on an issuer-configured server
    When the admin opens the token surface
    Then a clear empty state invites issuing the first token

  Scenario: A revoked token still appears in the list as revoked
    Given the workspace "Acme" has an issued token labelled "Old triage agent" that is revoked
    And the admin is signed in to the token surface on an issuer-configured server
    When the admin opens the token surface
    Then the row for "Old triage agent" shows status revoked
