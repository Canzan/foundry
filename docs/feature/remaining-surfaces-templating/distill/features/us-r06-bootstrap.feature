# Story: US-R06 — Bootstrap, claim, invite, and the shared invalid_page extend base.html
# Feature "Remaining-Surfaces Templating" — Slice 6 (finishes the cut)
# JTBD: htmx-web-2 (the first-run screens + every not-found page look styled)
#
# Driving adapter: the browser bootstrap + landing routes served by foundry-app —
# GET /dashboard (bootstrap dashboard) and any GET that funnels through the
# shared invalid_page helper (e.g. a non-existent team slug → not-found page).
# See design/architecture.md §US-R06 (bootstrap_dashboard.html, bootstrap_claim.html,
# bootstrap_invite.html, and the high-leverage shared invalid_page.html all extend
# base.html; the /bootstrap CSRF exemption + signed invite URL UNCHANGED) and
# render-contract.md §US-R06.
# Driven adapters exercised: real Postgres (workspaces, sessions, memberships);
# the vendored static-asset route for the asset reference.
#
# RED contract (MOVE-ONLY feature): the claim FLOW + the invite URL are already
# COVERED by us-05-bootstrap.feature (the regression net — MUST stay green, NOT
# re-asserted here). The render-contract flags the bootstrap dashboard COPY and
# the shared invalid_page SHAPE as PARTIAL — no existing scenario asserts the
# "Workspace dashboard" body styling or one structural assertion on the shared
# invalid_page (reused across ~17 call sites). Today bootstrap.rs::dashboard (:205)
# and bootstrap.rs::invalid_page (:356) emit bare <!doctype><body> inline strings
# with NO <link> stylesheet, so:
#   - "the bootstrap dashboard links the vendored stylesheet via the base layout"
#     fails RED for MISSING_FUNCTIONALITY until DELIVER moves it into
#     bootstrap_dashboard.html extending base.html.
#   - the shared not-found page asserts the invalid_page heading/message shape
#     AND the stylesheet link; the styling half fails RED until DELIVER moves the
#     shared invalid_page into invalid_page.html extending base.html, which
#     restyles EVERY not-found/error path at once.
# What DELIVER must wire is enumerated in
# docs/feature/remaining-surfaces-templating/distill/step-skeletons.md.

@remaining-surfaces @us-r06 @slice6 @driving_adapter @acme
Feature: A self-hoster sees styled first-run and not-found pages
  A self-hoster's bootstrap dashboard and every not-found page across the app
  now render styled, linking the vendored stylesheet, instead of unstyled raw
  HTML — and because every not-found path funnels through one shared template,
  styling it once styles them all. The claim flow and the signed invite URL are
  unchanged, and the existing bootstrap acceptance scenarios stay green.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"

  @real-io @driving_adapter
  Scenario: The bootstrap dashboard is a styled page with its heading preserved
    Given Mei is signed in as a Backend member
    When Mei opens the bootstrap dashboard
    Then the bootstrap dashboard shows the literal copy "Workspace dashboard"
    And the bootstrap dashboard links the vendored stylesheet from the application's own static path

  @error @real-io
  Scenario: A request to a non-existent team renders the shared styled not-found page
    Given Mei is signed in as a Backend member
    When Mei opens the board for a team slug that does not exist
    Then the not-found page shows a heading and a message in the shared error-page shape
    And the not-found page links the vendored stylesheet from the application's own static path
