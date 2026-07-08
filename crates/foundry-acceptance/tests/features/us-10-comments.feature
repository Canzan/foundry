# Story: US-10 — User comments on an issue
# Slice: 2 (Realtime collaboration)
# JTBD: outcome-4 (In-Foundry discussion replaces Slack pings)
#
# Driving ports:
#   - POST /team/{team}/project/{project}/issues/{key}/comments (CSRF-
#     protected; markdown body)
#   - GET  /team/{team}/project/{project}/issues/{key} (issue detail page
#     with rendered comment thread)
#
# Driven adapters exercised:
#   - real Postgres comments table + outbox row in same transaction
#   - real pulldown-cmark markdown rendering
#   - real ammonia HTML sanitization
#   - real pg_notify + per-replica LISTEN + SSE fan-out (shared with US-09)
#
# Sanitization: ammonia strips <script>, event-handler attributes, and
# javascript: URLs per NFR-SEC-05. Allowed elements per design/auth.md +
# stories.md (US-08/US-10): inline emphasis, code (fenced + inline), bold,
# italic, links (rel="noopener" added), lists, headings up to h3.
#
# Soft-deletion (`deleted_at`) preserves history; deleted comments are
# hidden from the rendered thread. Edits set `updated_at` and surface an
# "edited" indicator (assert: an edited-marker class is present).

@slice2 @us-10 @comments
Feature: A team member discusses an issue inline with markdown that other viewers see in real time
  A signed-in member of a team can comment on issues in that team's
  projects. Comments support a safe subset of markdown, render in
  chronological order on the issue detail page, and propagate to other
  subscribed viewers within the realtime budget. Empty bodies are
  rejected with an htmx fragment; non-members receive 403; XSS payloads
  are sanitized away while legitimate markdown survives.

  Background:
    Given a workspace "Acme Eng" exists with admin "devansh@acme.com"
    And a member "mei@acme.com" belongs to the team "Backend"
    And a member "hiroshi@acme.com" belongs to the team "Backend"
    And a project "Auth v2" with key prefix "AUTH" exists in the "Backend" team
    And the "Auth v2" project already has issue AUTH-3
    And Mei is signed in

  @walking_skeleton @real-io @driving_adapter
  Scenario: Member comments on an issue with markdown and it renders sanitized HTML
    When Mei comments on "AUTH-3" with body "Looked into this — root cause is the **Set-Cookie SameSite=Lax** default change. See ``request.cookies`` and [this RFC](https://example.com)."
    Then the issue page for "AUTH-3" shows a comment by Mei containing a <strong> element with text "Set-Cookie SameSite=Lax"
    And the issue page for "AUTH-3" shows a comment by Mei containing a <code> element with text "request.cookies"
    And the issue page for "AUTH-3" shows a comment by Mei containing an <a> element whose href is "https://example.com" and whose rel attribute contains "noopener"
    And the comment is recorded as authored by Mei

  @real-io @csrf
  Scenario: The add-comment form posts with the CSRF token the issue page itself mints
    # Regression for comment-add-csrf 01-01: the token/cookie MUST come from the
    # real issue-detail GET (the double-submit issuance seam every other write
    # form uses), NOT from /sign-in. Pre-fix `show_issue` mints no `foundry_csrf`
    # cookie and renders no hidden `_csrf` field, so this fails on the missing
    # seam rather than posting successfully.
    When Mei posts a comment on "AUTH-3" with body "Real-page CSRF proves the double-submit seam." using only the CSRF cookie and token minted by the issue page
    Then the comment is recorded as authored by Mei

  @real-io @us-09 @realtime
  Scenario: A new comment appears in real time to another viewer on the same issue
    Given Hiroshi has an open subscription to events on "Auth v2"
    When Mei comments on "AUTH-3" with body "Looked into this — the redirect is wrong on Safari."
    Then within 2000 milliseconds Hiroshi observes a "CommentAdded" event for "AUTH-3" on "Auth v2"
    And the event payload's author email is "mei@acme.com"

  @real-io @nfr-sec-05
  Scenario: Malicious script in a comment body is sanitized while safe markdown survives
    When Mei comments on "AUTH-3" with body "<script>alert('xss')</script> but **bold** and *italic* survive."
    Then the issue page for "AUTH-3" shows a comment by Mei that does NOT contain any <script> element
    And the issue page for "AUTH-3" shows a comment by Mei containing a <strong> element with text "bold"
    And the issue page for "AUTH-3" shows a comment by Mei containing an <em> element with text "italic"

  @real-io @error
  Scenario: An empty comment body is rejected with an inline htmx fragment, not a full page
    When Mei comments on "AUTH-3" with body ""
    Then the response status is 400 or 422
    And the response is an htmx fragment containing "Comment cannot be empty"
    And the response is not a full HTML page
    And no comment is recorded on "AUTH-3"

  @real-io @error
  Scenario: A whitespace-only comment body is rejected the same way an empty body is
    When Mei comments on "AUTH-3" with body "   \n   \t  "
    Then the response status is 400 or 422
    And the response is an htmx fragment containing "Comment cannot be empty"
    And no comment is recorded on "AUTH-3"

  @real-io @error @nfr-sec-06
  Scenario: A workspace member not on the team cannot comment on that team's issue
    Given a member "rita@partners.acme.com" belongs to the team "Partners"
    And Rita is signed in
    When Rita comments on "AUTH-3" with body "I have a thought."
    Then the response status is 403
    And no comment is recorded on "AUTH-3"
