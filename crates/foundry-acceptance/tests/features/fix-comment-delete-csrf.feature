# Feature: fix-comment-delete-csrf — the comment Delete button must carry a CSRF token.
#
# RCA: docs/feature/fix-comment-delete-csrf/rca/root-cause-analysis.md. The comment Delete
# control (comment_card.html:4) is a bare `hx-delete` with NO CSRF token. csrf_middleware
# requires a token for DELETE (is_safe_method allows only GET/HEAD/OPTIONS). A DELETE carries
# no urlencoded body to hold `_csrf`, this button has no `hx-headers` echo, and there is no
# global htmx CSRF injection — so in a REAL browser the DELETE reaches csrf_middleware with no
# token and is rejected 403 BEFORE the handler runs. Comment deletion is broken for real users.
#
# HARNESS NOTE — BROWSER-LANE ONLY. The gap is client-side: the shipped HTTP-lane comment-delete
# tests (us_10_comment_edit_delete) inject the CSRF token manually via reqwest, so they pass and
# never see the missing token. Only a real browser exercises the button as shipped. So this
# scenario is @needs-browser: it seeds a comment, clicks the shipped Delete button in a real
# Chrome, and asserts the card is REMOVED from the DOM — the DOM-level oracle the HTTP lane
# structurally cannot provide. RED today (403 → card stays), GREEN after the hx-headers fix.
#
# @needs-browser is IN the `all` lane (cargo xtask ci preflights chromedriver) and EXCLUDED from
# the fast default lane. This feature carries no @pending — the single scenario is live.

@fix-comment-delete-csrf @us-comment-delete-csrf @driving_port
Feature: The Delete button carries CSRF so a real-browser comment delete removes the card
  A member deleting their own comment from a real browser must have the DELETE accepted by
  csrf_middleware and the comment card removed from the page. The server response is unchanged;
  the button now supplies the cookie→header CSRF token a body-less DELETE cannot carry in a body.

  Background:
    Given a workspace "Acme" exists with a member "Mei" on team "Backend"
    And a project "Sandbox" with key prefix "GEN" exists under "Backend"
    And Mei is signed in

  @needs-browser @slice1 @us-01 @error @contract @real-io
  Scenario: A member can delete their comment from a real browser
    Given the "Sandbox" project has an issue "GEN-1" with a comment by Mei
    And Mei is viewing the "GEN-1" issue page in a real browser
    When Mei clicks the comment's Delete button
    Then the comment card is removed from the page
    And the comment is recorded as deleted in the store
