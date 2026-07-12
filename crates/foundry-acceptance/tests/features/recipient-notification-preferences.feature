# Feature: recipient-notification-preferences (v1 = recipient unsubscribe) — a signed,
# per-(email, workspace) opt-out for the two SUPPRESSIBLE notification events
# (workspace_invite, member_invite), with the three MANDATORY security events
# (password_reset, password_changed, member_removed) held structurally exempt.
#
# Source SSOT for docs/feature/recipient-notification-preferences/distill/test-scenarios.md.
# Requirements SSOT: ../discuss/ (US-01..07, AC per story, BR/NFR, the never-suppress
# @property). Design SSOT: ../design/ (architecture.md + adr-001..006; the "Handoff to
# DISTILL" + "Constraints for DISTILL/DELIVER" this file pins). Resolved contracts:
#   - UnsubscribeToken = HMAC over "unsub|v1|{email_lower}|{workspace_id}" via
#     foundry_auth::sign/verify, keyed on SESSION_SECRET, constant-time, NO expiry (ADR-001).
#   - Suppressible = {workspace_invite, member_invite}; MANDATORY-exempt =
#     {password_reset, password_changed, member_removed}, structural via
#     NotificationEvent::is_suppressible() (ADR-003).
#   - Suppression runs INSIDE the infallible Notifier::notify behind a SuppressionPolicy
#     port, bounded + FAIL-OPEN on lookup error; Notification gains workspace_id:
#     Option<Uuid>; default AllowAllSuppression = delivery byte-for-byte unchanged (ADR-003).
#   - Route: non-destructive GET /unsubscribe?t=..&sig=.. (state-aware confirm page, NO
#     state change, prefetch/scanner-safe) -> CSRF POST /unsubscribe flips state; bad/
#     tampered/unknown token -> the uniform non-enumerable refusal (ADR-002).
#   - State: 0014_notification_unsubscribes(email_lower, workspace_id, unsubscribed_at),
#     composite PK, FK workspaces ON DELETE CASCADE; absence-of-row = subscribed (ADR-004).
#   - Metric: sibling foundry_notification_suppressions_total{event} — event-only bounded
#     label, PII-free, register-at-0 over the full catalog (ADR-005).
#   - Signed-in status + resubscribe for account holders; token-undo resubscribe on the
#     confirm page for account-less recipients (ADR-006).
#
# Driving ports (Mandate 1 — every scenario enters through one, never an internal fn):
#   1. The signed unsubscribe LINK in the suppressible email body (minted at the emit site).
#   2. The public GET /unsubscribe (confirm page, non-destructive) + CSRF POST /unsubscribe.
#   3. A real shipped emit flow — the bootstrap/member invites, forgot-password, remove-
#      member, password-change — each emitting ONE notification through notify().
#   4. The signed-in GET /account/notifications + CSRF POST /account/notifications/resubscribe.
#   5. The recording provider double (was-it-delivered?) + the /metrics sidecar (suppressions).
#
# HARNESS BOUNDARY (distill/acceptance-review.md): the app + Postgres are REAL (the shipped
# in-process axum harness + testcontainers, @real-io), mirroring the predecessor
# notification-delivery-providers feature. The 0014 table, the SuppressionPolicy port +
# StoreSuppression + the /unsubscribe + /account/notifications routes are exercised REAL
# through the composition root. The DELIVERY TRANSPORTS stay in-process recording doubles
# (the shipped notify_recorder providers) so a `Then` can observe "delivered vs suppressed"
# without a real SMTP/webhook call. The register-at-0 + bounded-label metric scenarios drive
# a REAL `foundry` subprocess + scrape its /metrics sidecar (the in-process harness installs
# no recorder — same split the predecessor used).
#
# EVERY scenario is @pending, PER-SCENARIO (never a feature-level @pending): DELIVER removes
# the tag one slice at a time as it authors the token, table, route, suppression gate, and
# metric seam and turns each GREEN (Outside-In). @pending is excluded from EVERY lane
# (acceptance.rs filter_run, `!has("pending")`), so this file keeps the @all lane green until
# DELIVER unskips slice-by-slice. Run one slice with `FOUNDRY_ACCEPTANCE_TAGS=recipient-unsubscribe`
# (the feature-specific tag, chosen to avoid the @us-0N cross-feature tag collisions). Slice
# order = US-01 (walking skeleton) -> US-02 (mandatory exempt) -> US-03 (non-enumerable +
# prefetch-safe) -> US-04 (member_invite) -> US-05 (signed-in status) -> US-06 (resubscribe)
# -> US-07 (observability); the fail-open + workspace-cascade edges land with US-01/US-04.

@recipient-unsubscribe @driving_port
Feature: A recipient silences a workspace's invitation emails without losing security-critical mail
  A notification recipient — often an account-less invitee identified only by email — can stop a
  specific workspace's suppressible invitation emails with one click from the email itself, safely
  (non-enumerable, prefetch-safe) and reversibly, while the three security-critical events are
  guaranteed to always reach them; operators can watch opt-out volume without ever seeing who.

  Background:
    Given Foundry is serving with recipient unsubscribe enabled

  # ── Slice 01 — US-01 walking skeleton: link -> GET confirm -> CSRF POST -> row -> suppress ──

  @us-01 @walking_skeleton @driving_port @real-io
  Scenario: One click from the invite email stops that workspace's invitations
    Given Sam has a workspace-invite email for "Northwind" carrying a signed unsubscribe link
    When Sam opens the unsubscribe link and confirms unsubscribing from "Northwind"
    Then Sam sees a confirmation that "Northwind" invitations are stopped
    And a subsequent workspace-invite for Sam from "Northwind" is not delivered
    And one suppression is counted for the "workspace_invite" event

  @us-01 @real-io
  Scenario: Unsubscribing from one workspace leaves another untouched
    Given Sam has confirmed unsubscribing from "Northwind"
    And Sam also has an invite for workspace "Contoso"
    When a workspace-invite for "Contoso" is issued to Sam
    Then the "Contoso" invitation is delivered to Sam

  @us-01 @error @real-io
  Scenario: Confirming an unsubscribe twice is a harmless no-op
    Given Sam has confirmed unsubscribing from "Northwind"
    When Sam confirms unsubscribing from "Northwind" a second time
    Then Sam sees that he is already unsubscribed from "Northwind"
    And Sam sees the same confirmation both times with no error

  @us-01 @real-io
  Scenario: With no opt-out on record, a workspace-invite is delivered unchanged
    Given Sam has a workspace-invite email for "Northwind" carrying a signed unsubscribe link
    When a workspace-invite for "Northwind" is issued to Sam
    Then the workspace-invite for Sam from "Northwind" is delivered unchanged

  # ── Slice 02 — US-02: mandatory security events are NEVER suppressed (the crux invariant) ──

  @us-02 @property @real-io
  Scenario: A password reset reaches an unsubscribed recipient
    Given Sam has confirmed unsubscribing from "Northwind"
    When Sam requests a password reset
    Then the password-reset notification is delivered to Sam
    And it is not counted as suppressed

  @us-02 @real-io
  Scenario: A removal notice reaches an unsubscribed recipient
    Given Sam has confirmed unsubscribing from "Northwind"
    When an admin removes Sam from "Northwind"
    Then the member-removed notification is delivered to Sam
    And it is not counted as suppressed

  @us-02 @property @real-io
  Scenario: No mandatory event is ever suppressed for an unsubscribed recipient
    Given Sam is unsubscribed from every workspace he belongs to
    When a password reset, a password change, and a removal each fire for Sam
    Then every one of those notifications is delivered
    And none of them is counted as suppressed

  # ── Slice 03 — US-03: non-enumerable refusal + prefetch-safe GET + CSRF on the POST ──────

  @us-03 @security @real-io
  Scenario: A tampered token is refused exactly like an invalid one
    Given Sam's unsubscribe link for "Northwind" has a tampered token
    When the tampered unsubscribe link is opened
    Then the uniform non-enumerable refusal page is shown
    And no unsubscribe is recorded

  @us-03 @security @real-io
  Scenario: The response does not reveal whether an address exists
    Given an unsubscribe request for a real recipient carries an invalid token
    And an unsubscribe request for a non-existent address carries an invalid token
    When both unsubscribe links are opened
    Then both requests return a byte-identical refusal
    And neither response reveals whether the address, workspace, or account exists

  @us-03 @security @real-io
  Scenario: Prefetching the link does not unsubscribe anyone
    Given Sam has a valid unsubscribe link for "Northwind" he has not confirmed
    When an automated client fetches the unsubscribe link without confirming
    Then a subsequent workspace-invite to Sam in "Northwind" is still delivered
    And Sam remains subscribed to "Northwind" until he explicitly confirms

  @us-03 @security @error @real-io
  Scenario: An unsubscribe confirm without a valid CSRF token is refused
    Given Sam has a valid unsubscribe link for "Northwind" he has not confirmed
    When the unsubscribe confirm is posted without a valid CSRF token
    Then the confirm is refused and no opt-out state changes

  @us-03 @security @real-io
  Scenario: A refused unsubscribe request leaks no token or recipient email
    Given Sam's unsubscribe link for "Northwind" has a tampered token
    When the tampered unsubscribe link is opened
    Then no unsubscribe token or recipient email appears in the logs

  # ── Slice 04 — US-04: the same one-click opt-out covers member_invite too ────────────────

  @us-04 @real-io
  Scenario: One opt-out covers member-invite emails for a workspace
    Given Sam has unsubscribed from "Northwind" via a workspace-invite link
    When a member-invite for "Northwind" is issued to Sam
    Then the member-invite for Sam from "Northwind" is not delivered
    And one suppression is counted for the "member_invite" event

  @us-04 @real-io
  Scenario: The member-invite email carries its own unsubscribe link
    Given Sam has a member-invite email for "Northwind" carrying a signed unsubscribe link
    When Sam opens the unsubscribe link and confirms unsubscribing from "Northwind"
    Then both member-invite and workspace-invite emails from "Northwind" are suppressed

  @us-04 @property @real-io
  Scenario: Unsubscribing via a member-invite still leaves security mail intact
    Given Sam has unsubscribed from "Northwind" via a member-invite link
    When an admin removes Sam from "Northwind"
    Then the member-removed notification is delivered to Sam
    And it is not counted as suppressed

  # ── Slice 05 — US-05: the signed-in per-workspace status page (least-privilege) ──────────

  @pending @us-05 @real-io
  Scenario: The settings page shows per-workspace subscription status
    Given Maria is signed in and belongs to "Northwind", "Contoso", and "Initech"
    And Maria has confirmed unsubscribing from "Northwind"
    When Maria opens the notification settings page
    Then "Northwind" is shown as muted
    And "Contoso" is shown as subscribed
    And "Initech" is shown as subscribed

  @pending @us-05 @security @real-io
  Scenario: A request cannot be steered to another recipient's status
    Given Maria is signed in and belongs to "Northwind", "Contoso", and "Initech"
    When a request attempts to view notification status for another recipient's email
    Then only Maria's own status is returned
    And only workspaces Maria belongs to are listed

  # ── Slice 06 — US-06: resubscribe (signed-in toggle AND account-less token undo) ─────────

  @pending @us-06 @real-io
  Scenario: Resubscribing a muted workspace restores its notifications
    Given Maria is signed in and belongs to "Northwind", "Contoso", and "Initech"
    And Maria has confirmed unsubscribing from "Northwind"
    When Maria resubscribes to "Northwind"
    Then "Northwind" is shown as subscribed again
    And a subsequent invitation for "Northwind" is delivered to Maria

  @pending @us-06 @security @error @real-io
  Scenario: A resubscribe without a valid CSRF token is rejected
    Given Maria is signed in and belongs to "Northwind", "Contoso", and "Initech"
    And Maria has confirmed unsubscribing from "Northwind"
    When a cross-site request attempts to resubscribe Maria to "Northwind" without a valid CSRF token
    Then the resubscribe is refused and Maria's subscription state is unchanged

  @pending @us-06 @real-io
  Scenario: An account-less recipient resubscribes from the confirm page
    Given Sam has confirmed unsubscribing from "Northwind"
    When Sam opens his unsubscribe link and confirms resubscribing to "Northwind"
    Then "Northwind" is shown as subscribed again
    And a subsequent workspace-invite for Sam from "Northwind" is delivered to Sam

  @pending @us-06 @error @real-io
  Scenario: Resubscribing an already-subscribed workspace is harmless
    Given Maria is signed in and belongs to "Northwind", "Contoso", and "Initech"
    When Maria submits a resubscribe for "Northwind" twice from a stale page
    Then Maria sees the same resubscribe confirmation both times with no error
    And a subsequent invitation for "Northwind" is delivered to Maria

  # ── Slice 07 — US-07: PII-free suppression observability on the /metrics sidecar ─────────

  @pending @us-07 @real-io
  Scenario: Suppressed deliveries are visible as a count on the metrics endpoint
    Given several suppressible deliveries to unsubscribed recipients have been suppressed
    When Olivia scrapes the metrics endpoint
    Then a suppression count is present split by event
    And the counts reflect how many suppressible deliveries were suppressed

  @pending @us-07 @property @security @real-io
  Scenario: The suppression metric exposes no recipient PII
    Given several suppressible deliveries to unsubscribed recipients have been suppressed
    When Olivia scrapes the metrics endpoint
    Then no recipient email or unsubscribe token appears in any metric label or line

  @pending @us-07 @property @real-io
  Scenario: Mandatory events never appear as suppressed
    Given Olivia boots Foundry with recipient unsubscribe enabled
    When Olivia scrapes the metrics endpoint
    Then the suppression metric is registered at zero for every event
    And the suppressed count for every mandatory event is zero

  # ── Cross-cutting NFR/edge litmuses (fail-open + await-bounded notify(); cascade) ────────

  @nfr @property @error @real-io
  Scenario: A failing suppression lookup still delivers the notification (fail-open)
    Given Sam has confirmed unsubscribing from "Northwind"
    And the suppression lookup is failing
    When a workspace-invite for "Northwind" is issued to Sam
    Then the workspace-invite for Sam from "Northwind" is delivered unchanged
    And the emit completes without stalling

  @nfr @property @error @real-io
  Scenario: A slow suppression lookup does not stall the emit (await-bounded, fail-open)
    Given Sam has confirmed unsubscribing from "Northwind"
    And the suppression lookup is slow
    When a workspace-invite for "Northwind" is issued to Sam
    Then the workspace-invite for Sam from "Northwind" is delivered unchanged
    And the emit completes without stalling

  @edge @real-io
  Scenario: Deleting a workspace clears its opt-out rows
    Given a workspace "Northwind" with an unsubscribed recipient is scheduled for deletion
    When the "Northwind" workspace is deleted
    Then deleting the workspace succeeds
    And a previously-unsubscribed recipient of that workspace resumes delivery
    And no orphaned suppression state remains
