# Story: US-B01 — Render the issue board from a template
# Feature B "htmx Web Tier" — Slice 1 (Walking Skeleton)
# JTBD: htmx-web-1 (restyle a screen without touching Rust) + htmx-web-2 (styled first screen)
#
# Driving adapter: the browser board route served by foundry-app at
# GET /team/{team}/project/{project} — see design/architecture.md
# (the `views` module + templates/board.html) and design/render-contract.md
# (selector-and-substring-identical, move-only).
# Driven adapters exercised: real Postgres (teams, projects, issues, memberships,
# sessions) via testcontainers + per-scenario schema; the vendored static-asset
# route (ServeDir at /static) for the asset references.
#
# RED contract (move-only feature): the EXISTING board scenarios in
# us-08-file-issue.feature already pass for the current `format!` board and are
# the regression net (NFR-WEBB-COMPAT-01) — they MUST stay green and are NOT
# re-asserted here. This file asserts ONLY the genuine user-visible delta:
# the board now REFERENCES the vendored static assets (a `<link>` stylesheet
# and the htmx/Alpine `<script>` tags under /static), which today's
# `render_board` emits NONE of. That reference is absent today, so the
# walking-skeleton scenario fails RED for MISSING_FUNCTIONALITY until DELIVER
# adds templates/board.html + base.html (referencing /static assets) and the
# vendored blobs. What DELIVER must wire is enumerated in
# docs/feature/htmx-web-tier/distill/step-skeletons.md.

@feature-b @us-b01 @slice1 @driving_adapter @acme
Feature: A member opens the board and sees a styled, templated product surface
  Mei opens her team's project board at the same URL as always and sees the
  same issues in the same columns — but the page now renders from a template
  and links the vendored stylesheet and scripts the binary ships, so the board
  reads as a finished product instead of unstyled HTML. A contributor can
  change the board's wording in the template alone, and the existing board
  acceptance scenarios stay green.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"
    And a project "Auth v2" with key prefix "AUTH" exists in the "Backend" team

  @walking_skeleton @real-io @driving_adapter
  Scenario: A member opens a styled board that still shows the same issues
    Given the "Auth v2" project has issue AUTH-2 titled "Refresh token rotation" in progress
    And the "Auth v2" project has issue AUTH-3 titled "Revoke on password change" in the backlog
    And Mei is signed in as a Backend member
    When Mei opens the "Auth v2" board in her browser
    Then the board still shows the columns "Backlog", "Todo", "In-Progress", "Done"
    And the board still shows the cards for AUTH-2 and AUTH-3 in their columns
    And the board links the vendored stylesheet from the application's own static path
    And the board loads the vendored htmx and Alpine scripts from the application's own static path
    And the board references no external origin

  @real-io
  Scenario: An empty board shows an inviting, templated empty state
    Given the "Auth v2" project has no issues
    And Mei is signed in as a Backend member
    When Mei opens the "Auth v2" board in her browser
    Then the board still shows the columns "Backlog", "Todo", "In-Progress", "Done"
    And the board shows guidance explaining how to file the first issue
    And the board links the vendored stylesheet from the application's own static path

  @real-io
  Scenario: The board preserves the hidden keyboard-navigation order
    Given the "Auth v2" project has issue AUTH-2 titled "Refresh token rotation" in progress
    And the "Auth v2" project has issue AUTH-3 titled "Revoke on password change" in the backlog
    And Mei is signed in as a Backend member
    When Mei opens the "Auth v2" board in her browser
    Then the board carries the keyboard-navigation list with AUTH-2 before AUTH-3

  @error @real-io
  Scenario: A board template that fails to render returns a clean error, not a half-page
    Given the board template is configured to fail rendering
    And Mei is signed in as a Backend member
    When Mei opens the "Auth v2" board in her browser
    Then the board responds with a clean server error
    And the response is not a partially rendered page
