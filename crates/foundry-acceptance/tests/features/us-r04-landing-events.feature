# Story: US-R04 — Dashboard landing and the events sign-in-required page extend base.html
# Feature "Remaining-Surfaces Templating" — Slice 4
# JTBD: htmx-web-2 (the first post-sign-in screen and a common dead-end look styled)
#
# Driving adapter: the browser landing + events routes served by foundry-app —
# GET / (signed-in renders the dashboard body; signed-out 303-redirects to
# /sign-in) and GET /team/{team}/project/{project}/events (signed-out returns a
# 401 sign-in-required page). See design/architecture.md §US-R04 (dashboard_root.html
# and events_signin_required.html both extend base.html; the signed-out 303 and
# the 401 status are handler control flow, UNCHANGED) and render-contract.md §US-R04.
# Driven adapters exercised: real Postgres (sessions, memberships); the vendored
# static-asset route for the asset reference.
#
# RED contract (MOVE-ONLY feature): both surfaces are GAP/PARTIAL per the
# render-contract — no existing scenario asserts the signed-in "/" body or the
# events 401-page body. Today signin.rs::dashboard_root (:243) and
# events.rs::unauthorized_response (:138) emit bare <!doctype><body> inline
# strings with NO <link> stylesheet, so:
#   - "the landing links the vendored stylesheet via the base layout" fails RED
#     for MISSING_FUNCTIONALITY until DELIVER moves the signed-in body into
#     dashboard_root.html extending base.html.
#   - the signed-out 303 redirect is a behaviour-UNCHANGED regression guard
#     (it passes GREEN today; it is here to prove the move does not touch the
#     handler control flow).
#   - the events 401 page asserts the styled body + the /sign-in link + the
#     preserved 401 status; the body-styling half fails RED until DELIVER moves
#     it into events_signin_required.html extending base.html, while the 401
#     status + /sign-in link are the byte-stable contract the move must keep.
# What DELIVER must wire is enumerated in
# docs/feature/remaining-surfaces-templating/distill/step-skeletons.md.

@remaining-surfaces @us-r04 @slice4 @driving_adapter @acme
Feature: A member sees a styled landing and a styled events sign-in page
  After signing in, Mei lands on "/" and sees a styled landing that links the
  vendored stylesheet instead of a bare unstyled page — while an unauthenticated
  request to "/" still redirects to sign-in exactly as before. When her session
  has expired and she hits the events stream, she gets a styled sign-in-required
  page with a working sign-in link, and the unauthorized status is unchanged.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"

  @real-io @driving_adapter
  Scenario: A signed-in member lands on a styled dashboard landing
    Given Mei is signed in as a Backend member
    When Mei opens the dashboard landing
    Then the dashboard landing links the vendored stylesheet from the application's own static path
    And the dashboard landing references no external origin

  @real-io
  Scenario: A signed-out request to the landing still redirects to sign-in
    Given Mei has no current browser session
    When Mei opens the dashboard landing
    Then the dashboard landing redirects to the sign-in page with no body change

  @error @real-io
  Scenario: A signed-out events request returns a styled sign-in-required page
    Given a project "Auth v2" with key prefix "AUTH" exists in the "Backend" team
    And Mei has no current browser session
    When Mei requests the events stream for "Auth v2" without a session
    Then the events page is refused with a sign-in-required status
    And the events page links the vendored stylesheet from the application's own static path
    And the events page offers a sign-in link
