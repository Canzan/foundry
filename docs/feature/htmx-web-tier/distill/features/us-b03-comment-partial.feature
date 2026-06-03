# Story: US-B03 — Move the issue detail and comment thread to one card partial
# Feature B "htmx Web Tier" — Slice 2
# JTBD: htmx-web-1 (restyle the comment thread in one partial, not four Rust sites)
#
# Driving adapter: the browser issue/comment routes served by foundry-app —
# GET /team/{team}/project/{project}/issues/{n} (full page) and the htmx
# comment-post path that returns the OOB-swap fragment. See
# design/render-contract.md §"Issue page + comments" and §"the one-partial rule",
# and design/wave-decisions.md DD10 (one partial; OOB wrapper includes the SAME
# partial, fixing comments.rs:841).
# Driven adapters exercised: real Postgres (issues, comments, memberships,
# sessions); markdown sanitization stays in foundry_core (NFR-WEBB-BND-03).
#
# RED contract — THE bug made observable: today render_comment_card_oob
# (comments.rs ~:828-858) deliberately ELIDES the Edit/Delete buttons ("for
# simplicity"), so a live htmx-appended comment card structurally DIFFERS from
# the same card after a full page reload (which DOES carry the affordances).
# The live-vs-reloaded structural-parity scenario fails RED on that divergence
# until DELIVER routes the OOB path through the shared comment_card partial with
# the same affordance flags. The existing comment scenarios in
# us-10-comments.feature / us-10-comment-edit-delete.feature are the regression
# net for the unchanged behaviour (authz gating, 403/410 copy, sanitization) and
# MUST stay green — they are NOT re-asserted here.
# See docs/feature/htmx-web-tier/distill/step-skeletons.md.

@feature-b @us-b03 @slice2 @driving_adapter @acme
Feature: A live-posted comment card matches the same card after a reload
  When Mei posts a comment, the card that appears live via htmx looks exactly
  like the same card after Hiroshi reloads the page — including the Edit and
  Delete affordances, which today the live card wrongly omits. Every comment
  render path uses one card definition, so the live card and the reloaded card
  can no longer drift apart, while who-can-edit and markdown sanitization stay
  decided in the handler and core.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"
    And a project "Auth v2" with key prefix "AUTH" exists in the "Backend" team
    And the "Auth v2" project has issue AUTH-3 titled "Revoke on password change" in the backlog

  @walking_skeleton @real-io @driving_adapter
  Scenario: A live-posted comment card carries the same affordances as a reloaded one
    Given Mei is signed in as a Backend member
    When Mei posts the comment "Looked into this — SameSite default change" on AUTH-3
    And Mei reopens the AUTH-3 issue page after a full reload
    Then the live-appended comment card and the reloaded comment card are structurally identical
    And the live-appended comment card offers Mei the edit affordance
    And the live-appended comment card offers Mei the delete affordance

  @real-io
  Scenario: The issue page and its comment cards render with author and body
    Given Mei is signed in as a Backend member
    And Mei has posted the comment "First note" on AUTH-3
    When Mei opens the AUTH-3 issue page
    Then the comment card by Mei shows her as the author
    And the comment card by Mei shows the rendered comment body "First note"

  @real-io
  Scenario: The edited marker appears on a comment the author has edited
    Given Mei is signed in as a Backend member
    And Mei has posted the comment "Initial wording" on AUTH-3
    When Mei edits her comment on AUTH-3 to read "Revised wording"
    Then the comment card by Mei shows the rendered comment body "Revised wording"
    And the comment card by Mei shows the edited marker

  @error @real-io
  Scenario: A reader who is neither author nor admin sees no edit or delete affordance
    Given Mei is signed in as a Backend member
    And Mei has posted the comment "Mei's note" on AUTH-3
    And a member "hiroshi@acme.com" belongs to the team "Backend"
    And Hiroshi is signed in as a Backend member
    When Hiroshi opens the AUTH-3 issue page
    Then the comment card by Mei offers Hiroshi no edit affordance
    And the comment card by Mei offers Hiroshi no delete affordance

  @real-io
  Scenario: A workspace admin sees delete but not edit on another member's comment
    Given Mei is signed in as a Backend member
    And Mei has posted the comment "Mei's note" on AUTH-3
    And the workspace admin Devansh also belongs to the Backend team
    And Devansh is signed in as the workspace admin
    When Devansh opens the AUTH-3 issue page
    Then the comment card by Mei offers Devansh the delete affordance
    And the comment card by Mei offers Devansh no edit affordance

  @error @real-io
  Scenario: A dangerous comment is sanitized in core before the card renders
    Given Mei is signed in as a Backend member
    When Mei posts the comment "see <script>alert(1)</script> and [x](javascript:alert(2))" on AUTH-3
    And Mei opens the AUTH-3 issue page
    Then the comment card by Mei contains no script element
    And the comment card by Mei contains no javascript link
