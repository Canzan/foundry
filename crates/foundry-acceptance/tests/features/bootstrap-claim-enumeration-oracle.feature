# Feature: bootstrap-claim-enumeration-oracle
#
# Closes the SECOND (downstream) enumeration oracle on the bootstrap claim POST:
# after the single-use token is claimed, an email-uniqueness collision on the
# users insert currently surfaces as a 500 — distinguishable from the 303 success,
# so a token holder can probe which emails already map to an account. It also burns
# the token on a legitimate collision. This feature applies the shipped
# `create_member_and_consume` idiom (one-tx claim+create, 23505 caught specifically,
# rollback ⇒ token UNCONSUMED, uniform refusal) to the bootstrap claim.
#
# Driving port: HTTP form POST to /bootstrap (claim) — the SAME real endpoint the
# US-05 walking skeleton drives. Driven adapters: real Postgres (workspaces, users,
# bootstrap_tokens, instance_admins), FakeClock (token expiry).
#
# This file DELIBERATELY does NOT re-assert the US-05 net (token unknown/used/expired
# refusal, happy-path claim). It adds ONLY the email-collision arm of that same
# non-enumerable posture, plus the token-reusability (atomic-rollback) proof.
# The shared token-refusal steps are reused from us_05_bootstrap.rs (Pillar 1/2).

@bootstrap-enum-oracle @driving_port
Feature: A colliding bootstrap-claim email is refused without leaking that the email exists
  A bootstrap-token holder who submits an email that already maps to an account
  learns nothing about whether that email exists: the refusal is byte-identical to
  an unknown/expired-token refusal, never a 500. And because the claim and the
  account create commit atomically, a colliding submit leaves the token reusable —
  a retry with a fresh email still succeeds.

  Background:
    Given a fresh Foundry instance with no workspace and no users
    And the bootstrap token "valid-token-001" was minted 1 minute ago with a 30-minute TTL

  # US-03 regression (GREEN guard): the fresh-email claim still seeds the whole
  # workspace AND the first instance admin, landing at the dashboard exactly as
  # today. D1 requires this seed to stay green through the store rewire.
  @real-io @us-03 @regression
  Scenario: A fresh-email claim still seeds the workspace and the first instance admin
    When the admin submits the bootstrap claim form via "/bootstrap?token=valid-token-001" with
      | email          | devansh@acme.com               |
      | password       | correct horse battery staple   |
      | display_name   | Devansh                        |
      | workspace_name | Acme Eng                       |
    Then the response redirects the admin to the workspace dashboard
    And the workspace "Acme Eng" exists with a first instance admin

  # US-01 (the security crux): a colliding email must be refused BYTE-IDENTICALLY to
  # an expired and an unknown token — same status AND full body — so there is no
  # 500-vs-303 (or any) oracle distinguishing "email exists" from "email is fresh".
  # RED today: the collision arm returns 500 INTERNAL_SERVER_ERROR, so its status
  # diverges from the 200 token refusals.
  @real-io @us-01 @error @nfr-sec-01 @security-regression
  Scenario: Colliding email, expired token, and unknown token are refused indistinguishably
    Given the admin has already claimed the workspace using "valid-token-001"
    And the bootstrap token "second-token-002" was minted 1 minute ago with a 30-minute TTL
    And the bootstrap token "stale-token-003" was minted 31 minutes ago with a 30-minute TTL
    When a visitor submits the bootstrap claim for "second-token-002" using the already-registered email "devansh@acme.com"
    And a visitor submits the bootstrap claim for the expired token "stale-token-003"
    And a visitor submits the bootstrap claim for the unknown token "never-minted-999"
    Then the three bootstrap refusals are byte-identical in status and body
    And none of the refusals reveals whether the token was used, expired, or unknown

  # US-02 (atomic rollback, observable half 1): after a collision refusal the token
  # was NOT consumed — the claim+create rolled back together. RED today: the token
  # is claimed before the create runs, so the collision burns it (used_at is set).
  @real-io @us-02 @error
  Scenario: A colliding submit leaves the bootstrap token unconsumed
    Given the admin has already claimed the workspace using "valid-token-001"
    And the bootstrap token "second-token-002" was minted 1 minute ago with a 30-minute TTL
    And a visitor submits the bootstrap claim for "second-token-002" using the already-registered email "devansh@acme.com"
    Then the bootstrap token "second-token-002" remains unconsumed

  # US-02 (atomic rollback, observable half 2 — the user-facing recovery): because
  # the collision left the token reusable, retrying it with a fresh email succeeds
  # and lands signed in with a real workspace. RED today: the token is already burned,
  # so the retry is refused instead of redirecting.
  @real-io @us-02 @error
  Scenario: After a collision the token is reusable with a corrected email
    Given the admin has already claimed the workspace using "valid-token-001"
    And the bootstrap token "second-token-002" was minted 1 minute ago with a 30-minute TTL
    And a visitor submits the bootstrap claim for "second-token-002" using the already-registered email "devansh@acme.com"
    When the visitor retries "second-token-002" with the fresh email "mei@acme.com" and workspace "Mei Space"
    Then the response redirects the admin to the workspace dashboard
    And the workspace "Mei Space" exists with a first instance admin
