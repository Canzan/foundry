# Story: US-R02 — New-issue modal renders from a template/partial
# Feature "Remaining-Surfaces Templating" — Slice 2
# JTBD: htmx-web-1 (restyle the modal in one partial, not two Rust blocks)
#
# Driving adapter: the browser new-issue route served by foundry-app —
# GET /team/{team}/project/{project}/issues/new. With the hx-request header it
# returns the BARE modal fragment (htmx-swapped, already COVERED by
# us-12-keyboard-nav.feature); WITHOUT it (a no-JS browser) it returns the
# FULL-PAGE fallback. See design/architecture.md §US-R02 (partials/new_issue_modal.html
# is the ONE partial; new_issue_modal_page.html extends base.html and includes it)
# and design/render-contract.md §US-R02.
# Driven adapters exercised: real Postgres (teams, projects, memberships,
# sessions); the vendored static-asset route for the asset reference.
#
# RED contract (MOVE-ONLY feature): the modal FRAGMENT path is already COVERED —
# us_12_keyboard_nav asserts input[name="title"][autofocus] on the htmx GET, and
# that scenario is the regression net for the fragment (it MUST stay green and is
# NOT re-asserted here). The render-contract flags ONE genuine GAP: the no-JS
# FULL-PAGE fallback has no scenario exercising it. Today render_modal_full_page
# (keyboard.rs:124) emits a bare <!doctype><head> with NO <link> stylesheet, so
# the "full page links the vendored stylesheet via the base layout" assertion
# fails RED for MISSING_FUNCTIONALITY until DELIVER moves it into
# new_issue_modal_page.html extending base.html and including the shared partial.
# What DELIVER must wire is enumerated in
# docs/feature/remaining-surfaces-templating/distill/step-skeletons.md.

@remaining-surfaces @us-r02 @slice2 @driving_adapter @acme
Feature: A member without scripting opens the new-issue page and sees a styled fallback
  When Mei has scripting disabled and navigates straight to the new-issue URL,
  the page she lands on is a full styled page that links the vendored stylesheet,
  not a bare unstyled form — and it carries the same dialog, anti-forgery field,
  and autofocused title input the htmx modal does, posting to the identical
  action. The htmx modal fragment and its regression net stay green and unchanged.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"
    And a project "Auth v2" with key prefix "AUTH" exists in the "Backend" team

  @real-io @driving_adapter
  Scenario: The no-script new-issue page is a styled full page sharing the modal form
    Given Mei is signed in as a Backend member
    When Mei opens the new-issue page for "Auth v2" without scripting
    Then the new-issue page links the vendored stylesheet from the application's own static path
    And the new-issue page carries the new-issue dialog with the autofocused title input and the hidden anti-forgery field
    And the new-issue page references no external origin
