# Story: US-R05 — Attachment surfaces render from templates
# Feature "Remaining-Surfaces Templating" — Slice 5
# JTBD: htmx-web-1 (restyle the attachment row + error/limit pages in templates)
#
# Driving adapter: the browser attachment routes served by foundry-app —
# POST .../issues/{n}/attachments (an empty/bad upload returns the upload-error
# fragment; an over-limit upload returns the 413 too-large page). See
# design/architecture.md §US-R05 (attachment_row.html is the ONE partial wrapped
# by the OOB wrapper; the upload-error reuses the shared error_fragment.html;
# payload_too_large.html extends base.html) and render-contract.md §US-R05.
# Driven adapters exercised: real Postgres + real Multipart (attachments bytea,
# memberships, sessions).
#
# RED contract (MOVE-ONLY feature): the attachment LISTING + the 413 STATUS are
# already COVERED by us-11-attachments.feature (the regression net — MUST stay
# green, NOT re-asserted here). The render-contract flags two surfaces as
# PARTIAL: no existing scenario asserts the data-hx-fragment="attachment-upload-error"
# marker, and the 413 page BODY copy is not asserted (only its status). This file
# pins:
#   - the upload-error fragment marker (byte-stable; a bare fragment), which
#     fails RED only if the move drops/renames it; it is the contract the move
#     must preserve.
#   - the 413 page styling: today attachments.rs::payload_too_large (:353) emits
#     a bare <!doctype><head> with NO <link>; "the too-large page links the
#     vendored stylesheet via the base layout" fails RED for MISSING_FUNCTIONALITY
#     until DELIVER moves it into payload_too_large.html extending base.html,
#     while the 413 status + "Upload too large" copy are the byte-stable contract.
# What DELIVER must wire is enumerated in
# docs/feature/remaining-surfaces-templating/distill/step-skeletons.md.

@remaining-surfaces @us-r05 @slice5 @driving_adapter @acme
Feature: A member sees byte-stable attachment errors and a styled too-large page
  When Mei's upload is rejected as a bad request she gets the same inline error
  carrying the byte-stable scraper marker as a bare fragment; when her file
  exceeds the limit she gets a styled too-large page that links the vendored
  stylesheet, keeps the "Upload too large" copy, and returns the unchanged
  over-limit status. The existing attachment listing scenarios stay green.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"
    And a project "Auth v2" with key prefix "AUTH" exists in the "Backend" team
    And the "Auth v2" project has issue AUTH-1 titled "Refresh token rotation" in the backlog

  @error @real-io
  Scenario: An upload with no file part returns the byte-stable attachment upload-error fragment
    Given Mei is signed in as a Backend member
    When Mei submits an upload to "AUTH-1" with no file attached
    Then the attachment upload-error fragment carries the marker "attachment-upload-error"
    And the attachment upload-error fragment is a bare fragment that is not wrapped in the base layout

  @error @real-io
  Scenario: An over-limit upload returns a styled too-large page with the unchanged status
    Given Mei is signed in as a Backend member
    When Mei uploads a file over the configured limit to "AUTH-1"
    Then the upload is refused with an over-limit status
    And the too-large page shows the literal copy "Upload too large"
    And the too-large page links the vendored stylesheet from the application's own static path
