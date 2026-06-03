# Story: US-B05 — Normalize htmx directives and upgrade to a pinned htmx 2
# Feature B "htmx Web Tier" — Slice 4
# JTBD: htmx-web-3 (a future htmx upgrade is "swap the vendored file, run the suite")
#
# Driving adapter: the static-asset route (the vendored htmx blob) + the
# browser htmx-driven interactions (create-card OOB swap, comment edit/delete/
# cancel) served by foundry-app. See design/htmx2-migration.md (DD6/DD7: direct
# normalize-and-bump, latest stable htmx 2.0.x; data-* markers byte-stable) and
# design/wave-decisions.md.
# Driven adapters exercised: real filesystem (the vendored htmx 2.x blob via
# ServeDir); real Postgres for the interaction regression.
#
# RED contract: today static/ is empty and htmx is unvendored/unpinned at any
# version — so "the served htmx asset reports version 2.x" fails RED until
# DELIVER vendors the pinned htmx 2.0.x blob. The interaction-still-works and
# data-*-markers-byte-stable scenarios are the regression net DB4 requires; they
# go green once the bump lands AND the existing hx-driven scenarios stay green.
# Per Mandate 11 the layer-3 interaction checks are example-based, NOT property-
# generated. See docs/feature/htmx-web-tier/distill/step-skeletons.md.

@feature-b @us-b05 @slice4 @driving_adapter @acme
Feature: htmx is vendored at one pinned version 2 and every interaction still works
  After the normalization slice, the binary ships exactly one htmx file, pinned
  at a 2.x version, and every existing htmx-driven interaction — the create-card
  swap, posting a comment, editing, deleting, and cancelling an edit — behaves
  exactly as before, while the passive scraper markers the acceptance suite
  reads are left byte-for-byte unchanged.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"
    And a project "Auth v2" with key prefix "AUTH" exists in the "Backend" team

  @walking_skeleton @real-io @driving_adapter
  Scenario: The served htmx asset is a single pinned version 2
    Given the foundry binary is running
    When a browser requests the vendored htmx script from the static path
    Then the vendored htmx script reports a version in the 2 series
    And exactly one htmx file is vendored under the static path

  @real-io
  Scenario: Filing an issue still appends its card to the backlog after the upgrade
    Given the "Auth v2" project has no issues
    And Mei is signed in as a Backend member
    When Mei files an issue on "Auth v2" titled "Refresh token rotation broken on Safari"
    Then the returned fragment appends the new card to the backlog column
    And the new card carries the issue key

  @real-io
  Scenario: Posting and editing a comment still swap correctly after the upgrade
    Given the "Auth v2" project has issue AUTH-3 titled "Revoke on password change" in the backlog
    And Mei is signed in as a Backend member
    When Mei posts the comment "Working on it" on AUTH-3
    And Mei edits her comment on AUTH-3 to read "Fixed it"
    Then the comment card by Mei shows the rendered comment body "Fixed it"

  @real-io @nfr-web-compat-02
  Scenario: The render-contract data markers are left byte-unchanged by the normalization
    Given the "Auth v2" project has issue AUTH-2 titled "Refresh token rotation" in progress
    And Mei is signed in as a Backend member
    When Mei opens the "Auth v2" board in her browser
    Then the board carries the column marker for the backlog column
    And the board carries the issue-key markers on its cards
    When Mei opens the AUTH-2 issue page
    Then the issue page carries the comment-list marker
