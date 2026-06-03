# Story: US-B04 — Move sign-in and forgot-password to the shared base layout
# Feature B "htmx Web Tier" — Slice 3
# JTBD: htmx-web-2 (the first-impression auth screen looks trustworthy)
#
# Driving adapter: the browser sign-in / forgot-password routes served by
# foundry-app — GET /sign-in, GET /forgot-password — see
# design/render-contract.md §"Sign-in + forgot" and design/wave-decisions.md
# DD12 (template emits ONLY the hidden _csrf field; csrf.rs/cookie/header/
# brute-force/non-enumerable error are invariants, DB7).
# Driven adapters exercised: real Postgres (users, tower_sessions); FakeClock
# (brute-force delay) — both UNCHANGED.
#
# RED contract: the genuine user-visible delta is that the sign-in / forgot
# pages now render from the SHARED base layout and REFERENCE the vendored
# stylesheet from /static — today signin.rs::render_signin_form emits a bare
# <head> with no stylesheet link. The asset-reference assertions fail RED until
# DELIVER moves these pages onto base.html. The security contracts (non-
# enumerable error, CSRF cookie+field, 30-day session cookie attrs) are
# UNCHANGED — the scenarios below ASSERT they still hold (they are GREEN-staying
# regression guards living beside the new delta, mirroring Feature A's
# "browser path unchanged" guard). The existing us-06-signin.feature scenarios
# remain the primary regression net. See step-skeletons.md.

@feature-b @us-b04 @slice3 @driving_port @acme
Feature: The sign-in and forgot-password pages render from the shared styled layout
  A returning member and a first-time evaluator land on a sign-in page that is
  styled from the shared base layout and the vendored stylesheet — the same
  look as the board — while the page still sets the same 30-day session cookie,
  shows the same non-enumerable error, and preserves the same anti-forgery
  contract as before the markup moved.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" is registered with password "correct horse battery staple"

  @walking_skeleton @real-io @driving_port
  Scenario: The sign-in page renders from the shared layout and still signs the member in
    Given Mei has no current browser session
    When Mei opens the sign-in page
    Then the sign-in page links the vendored stylesheet from the application's own static path
    And the sign-in page renders from the shared base layout
    When Mei submits valid credentials on the sign-in page
    Then Mei is signed in and lands on the dashboard
    And her browser holds a session cookie that is HttpOnly and Secure and SameSite=Lax and valid for 30 days

  @error @real-io @nfr-sec-05
  Scenario: Invalid credentials show the unchanged non-enumerable error in the styled form
    When Mei submits a wrong password on the sign-in page
    Then the styled sign-in form shows "Invalid email or password"
    When an unknown visitor submits an unregistered email on the sign-in page
    Then the styled sign-in form shows "Invalid email or password"

  @real-io @nfr-sec-04
  Scenario: The anti-forgery contract is preserved on the templated sign-in form
    Given Mei has no current browser session
    When Mei opens the sign-in page
    Then the sign-in page sets an anti-forgery cookie
    And the sign-in form carries a matching hidden anti-forgery field
    When a sign-in is submitted without a valid anti-forgery token
    Then the sign-in submission is refused

  @real-io
  Scenario: The forgot-password page renders from the shared layout
    When Mei opens the forgot-password page
    Then the forgot-password page links the vendored stylesheet from the application's own static path
    And the forgot-password page renders from the shared base layout
