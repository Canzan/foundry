# Story: US-05 — Admin bootstraps a workspace and invites teammates
# Slice: 1 (Walking Skeleton)
# JTBD: outcome-1 (extends "stood up" to "team is using it")
#
# Driving port: HTTP form POST to /bootstrap (claim) — see architecture.md
# § Component Diagram, and auth.md § Bootstrap Token Flow.
# Driven adapters exercised: real Postgres (workspaces, users,
# workspace_memberships, teams, projects, bootstrap_tokens), FakeClock
# (token expiry), FakeEmailSender (only the email-invite scenario).

@slice1 @us-05 @driving_port
Feature: An admin claims a fresh Foundry and invites teammates
  An admin who follows the bootstrap URL from US-01 can claim the workspace,
  create the admin account, and produce an invite link in a single sitting.
  The bootstrap token is single-use and time-bounded; replays and expired
  links are rejected with explanatory pages, not partial-state failures.

  Background:
    Given a fresh Foundry instance with no workspace and no users
    And the bootstrap token "valid-token-001" was minted 1 minute ago with a 30-minute TTL

  @walking_skeleton @real-io @nfr-sec-01 @nfr-sec-03
  Scenario: Admin claims the workspace via the bootstrap URL and is signed in
    When the admin submits the bootstrap claim form via "/bootstrap?token=valid-token-001" with
      | email          | devansh@acme.com               |
      | password       | correct horse battery staple   |
      | display_name   | Devansh                        |
      | workspace_name | Acme Eng                       |
    Then the response redirects the admin to the workspace dashboard
    And the response sets a session cookie named "foundry_session"
    And that cookie is HttpOnly and SameSite=Lax and Secure
    And the workspace "Acme Eng" exists with Devansh as its only admin
    And a default team named "General" exists in that workspace
    And a default project named "Sandbox" exists in the General team

  @error @real-io
  Scenario: Replayed bootstrap token is rejected after the admin is claimed
    Given the admin has already claimed the workspace using "valid-token-001"
    When a second visitor opens the bootstrap URL "/bootstrap?token=valid-token-001"
    Then the bootstrap link is refused with the uniform non-enumerable page
    And no second workspace is created

  @error @real-io
  Scenario: Expired bootstrap token is rejected
    Given the bootstrap token "stale-token-002" was minted 31 minutes ago with a 30-minute TTL
    When a visitor opens the bootstrap URL "/bootstrap?token=stale-token-002"
    Then the bootstrap link is refused with the uniform non-enumerable page
    And no workspace, user, or session is created

  # SECURITY regression (enumeration oracle): the claim flow must NOT tell a prober
  # WHY a token is invalid. An already-used, an expired, and an unknown/forged token
  # must all be refused with a BYTE-IDENTICAL response (status AND full body) so the
  # prober cannot distinguish used vs expired vs never-existed. Mirrors the shipped
  # invite-accept-flow `invite_refusal_page()` posture (ADR-002). Before the fix the
  # three bodies diverged ("already used" / "expired" / "not recognised"): genuine RED.
  @error @real-io @nfr-sec-01 @security-regression
  Scenario: Used, expired, and unknown bootstrap tokens are refused indistinguishably
    Given the admin has already claimed the workspace using "valid-token-001"
    And the bootstrap token "stale-token-003" was minted 31 minutes ago with a 30-minute TTL
    When a visitor submits the bootstrap claim for the already-used token "valid-token-001"
    And a visitor submits the bootstrap claim for the expired token "stale-token-003"
    And a visitor submits the bootstrap claim for the unknown token "never-minted-999"
    Then the three bootstrap refusals are byte-identical in status and body
    And none of the refusals reveals whether the token was used, expired, or unknown

  @real-io @us-05
  Scenario: Admin generates a shareable invite link that contains a signed token
    Given the admin has claimed "Acme Eng" and is signed in
    When the admin opens the invite-teammates panel and requests a shareable link
    Then the response contains an invite URL
    And the invite URL carries a signed token parameter
    And the invite is recorded as valid for 7 days

  @real-io @smtp
  Scenario: Email invite delivers one message via the configured SMTP transport
    Given the admin has claimed "Acme Eng" and is signed in
    And the SMTP transport is configured
    When the admin sends an email invite to "mei@acme.com"
    Then exactly one email is recorded as sent to "mei@acme.com"
    And the recorded email body contains a signed invite link
