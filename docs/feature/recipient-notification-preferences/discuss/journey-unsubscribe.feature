# Feature: Recipient Unsubscribe — one click from the email, security mail always through
#
# Personas:
#   Sam Okafor (sam.okafor@acme.example) — RECIPIENT / invitee with no account; wants to stop unwanted
#     workspace-invite + member-invite emails from the email itself, without losing security-critical alerts.
#   Maria Santos (maria.santos@acme.example) — account-holding member; wants a signed-in per-workspace status
#     page and a resubscribe control.
#   Ops/Compliance Olivia (olivia.okonkwo@acme.example) — wants opt-out honored + observable (no PII) and
#     security events provably never suppressed.
#   Malicious Mallory — the adversary the token must defeat (enumeration, prefetch, tamper).
#
# This carves recipient preferences out of the shipped notification-delivery-providers feature. It builds the
# unsubscribe MECHANISM end-to-end and proves it on the two suppressible events (workspace_invite,
# member_invite), with the security events (password_reset, password_changed, member_removed) explicitly exempt.
#
# Core invariants:
#   - MANDATORY > unsubscribe: security events are NEVER suppressed, even for a fully-unsubscribed recipient.
#   - NON-ENUMERABLE: a tampered/unknown token yields a fixed, byte-identical refusal that leaks no existence,
#     and records nothing.
#   - PREFETCH-SAFE: a bare GET never mutates state; only an explicit confirm (POST / RFC 8058 one-click) does.
#   - LEAST-PRIVILEGE: a signed-in member views/mutates only their own state, from the session identity.
#   - PII-FREE: the suppression metric never carries a recipient email or token.
#   - ADDITIVE: with no unsubscribe rows, delivery is byte-for-byte as today.
#
# Scope (v1 = US-01..US-04): token + public route + 0014 table + suppression filter + security invariant +
# non-enumerable/prefetch-safety, on both invite events. US-05..US-07 (signed-in status, resubscribe, operator
# visibility) fast-follow. Per-category / digests / quiet-hours are OUT OF SCOPE.

Feature: Recipient unsubscribe — stop a workspace's invitations, keep security mail

  Background:
    Given Foundry is deployed for the "Acme" organization
    And "sam.okafor@acme.example" is an invitee with no account
    And "maria.santos@acme.example" is a member of "Northwind", "Contoso", and "Initech"

  # ---------------------------------------- Capability 1: Unsubscribe from the email (US-01, walking skeleton)

  Scenario: A workspace-invite email carries a per-workspace unsubscribe link
    Given a workspace-invite for "Northwind" is issued to "sam.okafor@acme.example"
    When the notification is built
    Then it includes a signed unsubscribe link bound to Sam's email and the Northwind workspace
    And a password-reset email for Sam would include no unsubscribe link

  Scenario: One click from the invite email stops that workspace's invitations
    Given Sam has a workspace-invite email for "Northwind" containing an unsubscribe link
    When Sam opens the link and confirms unsubscribing
    Then Sam sees a confirmation that Northwind invitations are stopped
    And the next workspace-invite for Sam from Northwind is not delivered

  Scenario: Unsubscribing from one workspace leaves another untouched
    Given Sam has unsubscribed from "Northwind"
    And Sam is also invited to "Contoso"
    When a workspace-invite for "Contoso" is issued to Sam
    Then the Contoso invitation is delivered normally

  Scenario: Confirming an unsubscribe twice is a harmless no-op
    Given Sam has already unsubscribed from "Northwind"
    When Sam opens the same unsubscribe link and confirms again
    Then Sam sees that he is already unsubscribed from Northwind
    And no error occurs and no duplicate record is created

  # -------------------------------------------------- Capability 2: Security mail is never suppressed (US-02)

  Scenario: A password reset reaches an unsubscribed recipient
    Given Sam has unsubscribed from "Northwind"
    When Sam requests a password reset
    Then the password-reset notification is delivered to Sam
    And it is not suppressed

  Scenario: A removal notice reaches an unsubscribed recipient
    Given Sam has unsubscribed from "Northwind"
    When an admin removes Sam from "Northwind"
    Then the member-removed notification is delivered to Sam
    And it is not suppressed

  # ------------------------------------------ Capability 3: A safe, non-enumerable, prefetch-proof link (US-03)

  Scenario: A tampered token is refused exactly like an invalid one
    Given an unsubscribe link whose token has been altered
    When the link is opened
    Then the uniform "no longer valid" refusal page is shown
    And no unsubscribe is recorded

  Scenario: The response does not reveal whether an address exists
    Given one request for a real recipient with an invalid token
    And another request for a non-existent address with an invalid token
    When both links are opened
    Then both return an identical refusal response
    And neither reveals whether the address, workspace, or account exists

  Scenario: Prefetching the link does not unsubscribe anyone
    Given Sam has a valid unsubscribe link he has not yet confirmed
    When an automated client fetches the link without confirming
    Then no unsubscribe is recorded
    And Sam remains subscribed until he explicitly confirms

  Scenario: A confirm without a valid CSRF token changes nothing
    Given a cross-site request tries to confirm an unsubscribe without a valid CSRF token
    When the request is submitted
    Then it is rejected
    And no opt-out is recorded

  # ------------------------------------------ Capability 4: The mechanism covers both invite events (US-04)

  Scenario: One opt-out covers both invite events for a workspace
    Given Sam has unsubscribed from "Northwind" via a workspace-invite link
    When an admin re-adds Sam and a member-invite for Northwind fires
    Then the member-invite is not delivered to Sam

  Scenario: The member-invite email carries its own unsubscribe link
    Given Sam's first contact from "Northwind" is a member-invite email
    When Sam opens its unsubscribe link and confirms
    Then Sam is unsubscribed from Northwind
    And both member-invite and workspace-invite emails from Northwind are suppressed

  # ------------------------------------------ Capability 5: Signed-in per-workspace status (US-05)

  Scenario: The settings page shows per-workspace subscription status
    Given Maria is signed in and is unsubscribed from "Northwind"
    When Maria opens the notification settings page
    Then she sees "Northwind" as muted
    And she sees "Contoso" and "Initech" as subscribed

  Scenario: The page shows only the signed-in member's own workspaces
    Given Maria is signed in
    When Maria opens the notification settings page
    Then only workspaces Maria belongs to are listed
    And no other recipient's status is shown

  Scenario: A request cannot be steered to another recipient's status
    Given Maria is signed in
    When a request attempts to view notification status for another user's email
    Then only Maria's own status is returned

  # ------------------------------------------ Capability 6: Signed-in resubscribe (US-06)

  Scenario: Resubscribing a muted workspace restores its notifications
    Given Maria is signed in and "Northwind" shows as muted
    When Maria clicks Resubscribe for Northwind
    Then Northwind shows as subscribed
    And the next Northwind invitation is delivered to Maria again

  Scenario: Resubscribing an already-subscribed workspace is harmless
    Given Maria is signed in and "Contoso" shows as subscribed
    When Maria submits a resubscribe for Contoso
    Then Contoso remains subscribed
    And no error occurs

  Scenario: A resubscribe without a valid CSRF token is rejected
    Given a cross-site request attempts to resubscribe Maria to "Northwind" without a valid CSRF token
    When the request is submitted
    Then it is rejected
    And Maria's subscription state is unchanged

  # ------------------------------------------ Capability 7: Operator visibility, no PII (US-07)

  Scenario: Suppressed deliveries are visible as a PII-free count on /metrics
    Given several workspace-invite and member-invite deliveries have been suppressed
    When Olivia scrapes /metrics
    Then a suppression count is present, split by event
    And no recipient email address or unsubscribe token appears in any label

  Scenario: Mandatory events never appear as suppressed
    Given recipients are unsubscribed and mandatory events have fired for them
    When Olivia inspects the suppression metric
    Then the suppressed count for password_reset, password_changed, and member_removed is zero

  # ------------------------------------------------------------ Security / operability properties

  @property
  Scenario: A mandatory security event is never suppressed
    Given a recipient unsubscribed from every workspace they belong to
    When a password reset, a password change, or a removal fires for them
    Then every one of those notifications is delivered
    And none is recorded as suppressed

  @property
  Scenario: An invalid unsubscribe token never leaks existence and never mutates state
    Given a tampered token, an unknown token, and a valid token that is only fetched (not confirmed)
    When each is presented to the unsubscribe endpoint
    Then the invalid ones return an identical refusal that reveals no existence
    And none of the three records an unsubscribe

  @property
  Scenario: A signed-in member can only ever view or change their own subscription state
    Given a signed-in member and another recipient's subscription state
    When the member views or submits any notification-settings action
    Then only the member's own state, scoped to their own workspaces, is ever read or written

  @property
  Scenario: The suppression metric never carries recipient PII
    Given suppressions have occurred for known recipients
    When /metrics is scraped and the delivery logs are inspected
    Then no recipient email address or unsubscribe token appears in any label or line
    And the label domains stay within their bounded sets

  @property
  Scenario: With no unsubscribe records, delivery is unchanged from before this feature
    Given the notification-unsubscribes table is empty
    When any existing notification (invite, reset, removal) fires
    Then delivery behaves byte-for-byte as it did before this feature
</content>
