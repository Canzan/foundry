# Story: US-06 — User signs in with email and password
# Slice: 1 (Walking Skeleton)
# JTBD: outcome-4 (daily-usable, low-friction sign-in is part of the Linear-feel promise)
#
# Driving port: HTTP form POST to /sign-in (auth) — see auth.md § Sessions,
# § Password Handling, § Brute-force protection.
# Driven adapters exercised: real Postgres (users, signin_attempts,
# tower_sessions), FakeClock (brute-force window + delay recording),
# FakeEmailSender (password-reset email).
# NFR-SEC-02 (brute-force delay) verified via recorded sleep duration, not
# wall-clock — see atdd-infrastructure-policy.md § Driven external.

@slice1 @us-06 @driving_port
Feature: A returning member signs in and keeps a stable session
  A registered user can sign in with email and password, receive a
  server-validated session cookie, and access protected pages. Wrong
  passwords produce non-enumerable errors; sustained failures trigger an
  artificial delay rather than a lockout; signing out invalidates the
  server-side session row.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" is registered with password "correct horse battery staple"

  @walking_skeleton @real-io @nfr-sec-03
  Scenario: Member signs in successfully and receives a secure session cookie
    Given Mei has no current session
    When Mei submits the sign-in form via "/sign-in" with email "mei@acme.com" and password "correct horse battery staple"
    Then the response redirects Mei to "/"
    And the response sets a session cookie named "foundry_session"
    And that cookie is HttpOnly and SameSite=Lax and Secure
    And the session is recorded as valid for 30 days
    And requesting a protected page with that cookie returns a successful response

  @error @real-io
  Scenario: Wrong password produces a non-enumerable error
    When Mei submits the sign-in form with email "mei@acme.com" and password "wrong-password"
    Then the response status is 401 or shows an inline error
    And the response body contains "Invalid email or password"
    And no session cookie is set

  @error @real-io
  Scenario: Unknown email produces the same error as wrong password
    When a visitor submits the sign-in form with email "ghost@acme.com" and password "anything"
    Then the response body contains "Invalid email or password"
    And no session cookie is set

  @nfr-sec-03 @real-io
  Scenario: Sign-in timing does not reveal whether an email is registered
    # Username-enumeration side-channel guard. Production runs one argon2id
    # verify on both the real-user and unknown-email paths (the latter against
    # a known-bad hash), so the symmetry is genuine. The test samples the two
    # arms interleaved and compares medians — robust to the spawn_blocking-pool
    # contention that made a single-sample comparison flake under @all.
    When sign-in latency is sampled over 7 interleaved unknown-email and wrong-password attempts
    Then the median unknown-email latency is within 150ms of the median wrong-password latency

  @nfr-sec-02 @error @real-io
  Scenario: The sixth failed attempt within 15 minutes is delayed by at least 5 seconds
    Given Mei has failed sign-in 5 times in the last 15 minutes
    When Mei submits a sixth failed sign-in attempt
    Then the handler records a scheduled delay of at least 4500 milliseconds before responding
    And the response otherwise contains "Invalid email or password"

  @real-io
  Scenario: Sign-out invalidates the server-side session row
    Given Mei is signed in with an active session
    When Mei posts to "/sign-out"
    Then the server-side session row for Mei's session id no longer exists
    And presenting Mei's prior cookie to a protected page returns an anonymous-redirect response

  @real-io
  Scenario: Password-reset email is sent when SMTP is configured and email exists
    Given the SMTP transport is configured
    When a visitor submits the forgot-password form with email "mei@acme.com"
    Then the response body contains "If that email is on file, a reset link has been sent"
    And exactly one email is recorded as sent to "mei@acme.com"
    And the recorded email body contains a reset link valid for 1 hour
