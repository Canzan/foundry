# Story: US-10 — Author edits/deletes own comment; admin deletes any
# Slice: 5 (comment-edit-delete)
# JTBD: outcome-4 (In-Foundry discussion replaces Slack pings; revising
#       and removing comments is part of the discussion surface)
#
# Inheritance (slice 2 — foundry-realtime-collab):
#   - POST /team/{team}/project/{project}/issues/{n}/comments
#   - GET  /team/{team}/project/{project}/issues/{n}
#   - SSE  channel issue_events (CommentAdded already shipped)
#   These exist and are GREEN. Slice 5 ADDS edit + delete + admin-delete
#   + the SSE event types for the new verbs.
#
# Slice-5 driving ports (per architecture.md "Route Additions"):
#   - GET    /team/{team}/project/{project}/issues/{n}/comments/{id}/edit
#   - PATCH  /team/{team}/project/{project}/issues/{n}/comments/{id}
#   - DELETE /team/{team}/project/{project}/issues/{n}/comments/{id}
#
# Driven adapters exercised (all reused from slice 2 — zero new
# infrastructure per the slice-5 Reuse Analysis):
#   - real Postgres comments table + outbox row in same transaction
#     (per ADR-007 soft-delete + ADR-008 SSE event shape)
#   - real pulldown-cmark markdown rendering + ammonia sanitization
#     (re-run on every edit per architecture.md "Inheritance"
#     paragraph 4)
#   - real pg_notify + per-replica LISTEN + SSE fan-out (the slice-2
#     trigger fires the same way; slice-5 just adds two new event_type
#     values per ADR-008)
#
# Layer / PBT mode declaration (per nw-test-design-mandates Mandate 9):
#   - Layer 3 (subprocess-or-equivalent HTTP + real Postgres).
#   - Example-only. No proptest. Sad paths are enumerated explicitly
#     (Mandate 11). PBT belongs at layers 1-2 (unit), which is
#     DELIVER's responsibility.
#
# Authorization rule (architecture.md "Authorization is HTTP-verb-
# uniform" constraint, ADR-006 "Always editable"):
#   - PATCH:  comment.author_id == actor.user_id (admin NOT exercised
#             for PATCH in this slice; admin-edit is a follow-on per
#             ADR-006 § Decision paragraph 1)
#   - DELETE: comment.author_id == actor.user_id || actor.role == admin
#   - GET edit-form: same as PATCH (probe-the-substrate-lie that
#     authorization is uniform across HTTP verbs)
#
# 404-vs-410 disambiguation (architecture.md "404-vs-410 Handler
# Logic", per ADR-008 + DESIGN-D6 = B):
#   - 404: random UUID, or wrong workspace
#   - 410: row exists but is soft-deleted (deleted_at IS NOT NULL)
#   - 403: row exists, is live, but actor is not authorized
#
# UX wording for 410-Gone (DISTILL D4 recommendation = A, terse):
#   The 410 response body contains the literal substring "This comment
#   has been deleted". Assertion uses substring match so a v0.2 copy
#   polish does not red the suite.
#
# Soft-delete invariant (ADR-007 + wave-decisions.md Constraint 1):
#   The issue-page GET MUST filter `WHERE deleted_at IS NULL`. The
#   "soft-delete invariant" scenario at the bottom proves this
#   behaviourally — insert one live + one soft-deleted comment, GET
#   the issue page, assert only the live one renders.

@slice5 @us-10 @comment-edit-delete
Feature: A team member edits or deletes their own comment, and a workspace admin removes any comment; viewers see edits and removals in real time
  A signed-in comment author may edit their own comment at any time
  (ADR-006: always editable) or delete it (ADR-007: soft tombstone).
  Workspace admins may delete any comment as a moderation action.
  Edits surface an "edited" indicator next to the timestamp;
  deletions remove the comment card from the rendered thread. Both
  fan out to other viewers via the slice-2 SSE channel with two new
  event types (ADR-008: CommentEdited, CommentDeleted). PATCH on a
  non-author returns 403; PATCH or DELETE on an already-soft-deleted
  row returns 410 Gone (ADR-008 + architecture.md "404-vs-410").

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"
    And a member "hiroshi@acme.com" belongs to the team "Backend"
    And a project "Auth v2" with key prefix "AUTH" exists in the "Backend" team
    And the "Auth v2" project already has issue AUTH-3
    And Mei is signed in

  @walking_skeleton @real-io @driving_adapter @comment-edit
  Scenario: Comment author edits their own comment and the updated text replaces the original in the thread
    Given Mei has previously posted a comment on "AUTH-3" with body "Looked into this — root cause is the **Set-Cookie SameSite=Lax** default change."
    When Mei requests the edit form for her comment on "AUTH-3"
    Then the response is an htmx fragment containing a textarea whose value is the raw markdown source of her comment
    When Mei submits an edit to her comment on "AUTH-3" with body "Updated — the root cause is the **Set-Cookie SameSite=Strict** policy applied to third-party iframes."
    Then the issue page for "AUTH-3" shows a comment by Mei containing a <strong> element with text "Set-Cookie SameSite=Strict"
    And the issue page for "AUTH-3" shows a comment by Mei with an "edited" indicator
    And the issue page for "AUTH-3" does NOT show a comment by Mei containing the text "SameSite=Lax"

  @real-io @error @comment-edit @nfr-sec-06
  Scenario: A non-author cannot edit someone else's comment
    Given Mei has previously posted a comment on "AUTH-3" with body "Looked into this."
    And Hiroshi is signed in
    When Hiroshi submits an edit to Mei's comment on "AUTH-3" with body "Sabotaged text."
    Then the response status is 403
    And the issue page for "AUTH-3" still shows a comment by Mei containing the text "Looked into this."

  @real-io @comment-delete @admin
  Scenario: Workspace admin deletes any comment and remaining viewers see it disappear from the thread
    Given Mei has previously posted a comment on "AUTH-3" with body "Probably can close this as a dup."
    And Devansh is signed in
    When Devansh deletes Mei's comment on "AUTH-3"
    Then the response status is 200
    And the issue page for "AUTH-3" no longer shows a comment by Mei

  @real-io @comment-delete
  Scenario: Comment author deletes their own comment
    Given Mei has previously posted a comment on "AUTH-3" with body "Wait, never mind."
    When Mei deletes her own comment on "AUTH-3"
    Then the response status is 200
    And the issue page for "AUTH-3" no longer shows a comment by Mei

  @real-io @comment-edit @realtime @nfr-perf-03
  Scenario: An open subscriber receives a CommentEdited event when another viewer edits an existing comment
    Given Mei has previously posted a comment on "AUTH-3" with body "First take."
    And Hiroshi has an open subscription to events on "Auth v2"
    When Mei submits an edit to her comment on "AUTH-3" with body "Revised take."
    Then within 2000 milliseconds Hiroshi observes a "CommentEdited" event for "AUTH-3" on "Auth v2"
    And the event payload's comment author email is "mei@acme.com"

  @real-io @comment-delete @realtime @nfr-perf-03
  Scenario: An open subscriber receives a CommentDeleted event when another viewer deletes a comment
    Given Mei has previously posted a comment on "AUTH-3" with body "Throwaway thought."
    And Hiroshi has an open subscription to events on "Auth v2"
    When Mei deletes her own comment on "AUTH-3"
    Then within 2000 milliseconds Hiroshi observes a "CommentDeleted" event for "AUTH-3" on "Auth v2"

  @real-io @error @gone
  Scenario: PATCH on a comment that has already been soft-deleted returns 410 Gone with an htmx fragment
    Given Mei has previously posted a comment on "AUTH-3" with body "Will reconsider."
    And Mei has deleted her own comment on "AUTH-3"
    When Mei submits an edit to her soft-deleted comment on "AUTH-3" with body "Trying to undo my delete."
    Then the response status is 410
    And the response is an htmx fragment containing "This comment has been deleted"

  @real-io @error @gone
  Scenario: DELETE on a comment that has already been soft-deleted returns 410 Gone
    Given Mei has previously posted a comment on "AUTH-3" with body "Done with this."
    And Mei has deleted her own comment on "AUTH-3"
    When Mei deletes her own comment on "AUTH-3" again
    Then the response status is 410
    And the response is an htmx fragment containing "This comment has been deleted"

  @real-io @soft-delete-invariant
  Scenario: The issue page lists only non-deleted comments
    Given Mei has previously posted a comment on "AUTH-3" with body "First comment — kept."
    And Mei has previously posted a comment on "AUTH-3" with body "Second comment — to be removed."
    When Mei deletes her own "Second comment — to be removed." comment on "AUTH-3"
    Then the issue page for "AUTH-3" shows a comment by Mei containing the text "First comment — kept."
    And the issue page for "AUTH-3" does NOT show a comment by Mei containing the text "Second comment — to be removed."

  @real-io @comment-edit @cancel
  Scenario: Author cancels the edit and the original card is returned by the server
    Given Mei has previously posted a comment on "AUTH-3" with body "Initial body."
    When Mei requests the edit form for her comment on "AUTH-3"
    And Mei cancels the edit on her comment on "AUTH-3"
    Then the response is an htmx fragment containing the text "Initial body."
    And the response is an htmx fragment that does NOT contain a <textarea> element
