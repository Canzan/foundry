# Story: US-R03 — Issue-create-error and state-change fragments render from templates
# Feature "Remaining-Surfaces Templating" — Slice 3
# JTBD: htmx-web-1 (reword the error / restyle the state chip in templates, not Rust)
#
# Driving adapter: the browser issue routes served by foundry-app —
# POST /team/{team}/project/{project}/issues (a no-title submit returns the
# issue-create-error fragment) and POST .../issues/{n}/state (returns the
# state-change <span>). See design/architecture.md §US-R03 (the error reuses the
# shared error_fragment.html; the chip moves to partials/state_chip.html) and
# design/render-contract.md §US-R03. Both stay BARE fragments.
# Driven adapters exercised: real Postgres (issues, projects, memberships,
# sessions).
#
# RED contract (MOVE-ONLY feature): the issue-create and board behaviour is
# already COVERED by us-08-file-issue.feature (the regression net — MUST stay
# green, NOT re-asserted here). The render-contract flags these two markers as
# PARTIAL — no existing scenario explicitly asserts the
# data-hx-fragment="issue-create-error" marker + the literal "Title is required"
# copy, nor the data-state value on the state chip. This file pins both so the
# move cannot silently drop a scraper marker or reword the error. They fail RED
# for MISSING_FUNCTIONALITY only if the DELIVER move drops/renames a marker;
# until DELIVER routes these through templates the asserts hold against the
# current format! output and are the byte-stable contract the move must preserve.
# What DELIVER must wire is enumerated in
# docs/feature/remaining-surfaces-templating/distill/step-skeletons.md.

@remaining-surfaces @us-r03 @slice3 @driving_adapter @acme
Feature: A member sees byte-stable issue error and state fragments
  When Mei files an issue without a title she gets the same inline error carrying
  the byte-stable scraper marker and the literal "Title is required" copy; when
  she changes an issue's state she gets the same state chip carrying its
  byte-stable data-state value. Both are bare fragments htmx swaps in place, and
  the existing issue-create acceptance scenarios stay green.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"
    And a project "Auth v2" with key prefix "AUTH" exists in the "Backend" team

  @error @real-io
  Scenario: A missing-title submission returns the byte-stable issue-create error fragment
    Given Mei is signed in as a Backend member
    When Mei files an issue on "Auth v2" with an empty title
    Then the issue-create error fragment carries the marker "issue-create-error"
    And the issue-create error fragment shows the literal copy "Title is required"
    And the issue-create error fragment is a bare fragment that is not wrapped in the base layout

  @real-io
  Scenario: A state change returns the byte-stable state chip
    Given the "Auth v2" project has issue AUTH-1 titled "Refresh token rotation" in the backlog
    And Mei is signed in as a Backend member
    When Mei moves "AUTH-1" to the "in-progress" state from the board
    Then the state chip carries the data-state value "in_progress"
    And the state chip is a bare fragment that is not wrapped in the base layout
