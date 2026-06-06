# Story: US-R01 — Project-create form + error fragment render from a template
# Feature "Remaining-Surfaces Templating" — Slice 1 (Walking Skeleton)
# JTBD: htmx-web-1 (edit markup in a template, not Rust) + htmx-web-2 (styled screen, offline)
#
# Driving adapter: the browser project-create routes served by foundry-app —
# GET /team/{team}/projects/new (the full-page form) and POST
# /team/{team}/projects (the create handler, which on a bad submit returns the
# project-create-error fragment). See design/architecture.md §"Surface → template
# map" (project_create.html extends base.html; the error div stays a bare
# fragment) and design/render-contract.md §US-R01.
# Driven adapters exercised: real Postgres (teams, projects, memberships,
# sessions) via testcontainers + per-scenario schema; the vendored static-asset
# route (ServeDir at /static) for the asset reference.
#
# RED contract (MOVE-ONLY feature): the EXISTING project-create scenarios in
# us-07-project-create.feature already pass for the current format! output and
# are the regression net (NFR-WEBB-COMPAT-01) — they MUST stay green and are NOT
# re-asserted here. This file asserts ONLY the genuine user-visible DELTAS, each
# of which fails RED for MISSING_FUNCTIONALITY today:
#   - the create form is a bare <!doctype><head> inline format! today
#     (projects.rs::render_create_form :466) with NO <link> stylesheet; the
#     "links the vendored stylesheet via the base layout" assertion fails until
#     DELIVER moves it into project_create.html extending base.html.
#   - the project-create-error fragment marker
#     (data-hx-fragment="project-create-error") is not explicitly asserted by
#     the existing suite (render-contract.md flags it PARTIAL); this file pins
#     it so the move cannot silently drop the scraper marker.
# What DELIVER must wire to flip these GREEN is enumerated in
# docs/feature/remaining-surfaces-templating/distill/step-skeletons.md.

@remaining-surfaces @us-r01 @slice1 @driving_adapter @acme
Feature: A member opens the project-create form and sees a styled, templated page
  Mei opens the create-project form at the same URL as always and types the
  same name and key prefix — but the page now renders from a template and links
  the vendored stylesheet the binary ships, so the form reads as a finished
  product instead of unstyled HTML. When she submits a bad key, the inline error
  still carries the byte-stable scraper marker so any Alpine hook still finds it.
  A contributor can reword the form by editing one template, and the existing
  project-create acceptance scenarios stay green.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"

  @walking_skeleton @real-io @driving_adapter
  Scenario: A member opens a styled, templated project-create form
    Given Mei is signed in as a Backend member
    When Mei opens the project-create form for the "Backend" team
    Then the project-create form links the vendored stylesheet from the application's own static path
    And the project-create form shows the project-name and key-prefix inputs and the hidden anti-forgery field
    And the project-create form references no external origin

  @error @real-io
  Scenario: An invalid submission returns the byte-stable project-create error fragment
    Given Mei is signed in as a Backend member
    When Mei submits the project-create form for "Backend" with name "Billing" and an empty key prefix
    Then the project-create error fragment carries the marker "project-create-error"
    And the project-create error fragment is a bare fragment that is not wrapped in the base layout
