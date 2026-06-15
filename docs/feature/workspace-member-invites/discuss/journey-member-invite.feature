# Feature: Workspace Member Invites — an admin invites a teammate; the teammate joins
#
# Personas:
#   Dana Reyes (dana.reyes@northwind.example) — ADMIN of "Northwind", the inviter.
#   Sam Okafor (sam.okafor@northwind.example) — the invitee; has NO Foundry account yet.
#
# This generalizes the shipped first-admin invite-accept-flow to general members.
# Two capabilities: admin ISSUANCE (admin-gated, non-enumerable for non-admins) and
# invitee ACCEPTANCE that CREATES the account + adds a member-role membership + sets
# the password + auto-signs-in, all in ONE atomic transaction.
#
# Scope (v1): member role only. The uniform acceptance-refusal body MUST be
# byte-identical across all invalid-link reasons AND the email-already-a-user case
# (non-enumerable, the security crux — reused verbatim from the shipped flow).

Feature: Workspace member invites — issue and accept

  Background:
    Given the "Northwind" workspace exists
    And Dana Reyes is its admin
    And Sam Okafor has no Foundry account yet

  # ------------------------------------------------- Capability 1: Issuance (admin)

  Scenario: A workspace admin sends a member invite and gets a shareable link
    Given Dana is signed in as an admin of "Northwind"
    When Dana submits "sam.okafor@northwind.example" on the member-invite form
    Then an invite to "Northwind" is created for that email
    And Dana sees a confirmation with a shareable accept link valid for 7 days

  Scenario: A blank email is corrected inline without creating an invite
    Given Dana is on the member-invite form
    When Dana submits the form with an empty email
    Then she sees an inline error asking for an email address
    And no invite is created

  Scenario: A non-admin member cannot tell the issuance surface exists
    Given Sam is signed in as a plain member of "Northwind"
    When Sam opens the member-invite page
    Then he sees a generic "not found" response
    And nothing reveals that an issuance surface exists

  Scenario: A signed-out caller cannot tell the issuance surface exists
    Given no one is signed in
    When the member-invite page is opened
    Then the response is a generic "not found"
    And it is indistinguishable from the non-admin refusal

  # ----------------------------------------------- Capability 2: Acceptance (invitee)

  Scenario: An invitee with no account opens a live invite and sees a set-password form
    Given Dana issued Sam a live member invite for "Northwind" 2 hours ago
    When Sam opens his invite link
    Then he sees a set-password form naming the "Northwind" workspace
    And no account exists for him yet

  Scenario: Setting a valid password creates the account, joins the workspace, and signs in
    Given Sam has opened his valid member invite for "Northwind"
    When he submits a password meeting the strength policy and confirms it
    Then a new account is created for "sam.okafor@northwind.example"
    And he is added to "Northwind" as a member
    And he is signed in without a separate login step
    And he lands on the "Northwind" workspace dashboard
    And he sees no data from any other workspace

  Scenario: A newly joined member has member privileges only
    Given Sam has just joined "Northwind" via his invite
    When Sam opens the member-invite page
    Then he sees a generic "not found" response
    And he cannot issue invites

  Scenario: A consumed member invite can never be used again
    Given Sam has already joined "Northwind" via his invite link
    When Sam opens the same invite link again
    Then he sees the standard "invite is no longer valid" page
    And no second account is created and no session is created

  # ------------------------------------------------------------------ Sad paths

  Scenario: An expired invite is refused without leaking existence
    Given Sam's member invite expired 1 day ago
    When Sam opens his invite link
    Then he sees the standard "invite is no longer valid" page
    And the page does not reveal whether any account or workspace exists
    And the page advises asking the workspace administrator to re-issue the invite

  Scenario: A tampered signature is refused identically to an expired invite
    Given Sam's member invite is live
    But the signature in the link has been altered by one character
    When Sam opens the tampered link
    Then he sees the standard "invite is no longer valid" page
    And the response body is byte-identical to the expired-invite refusal

  Scenario: An unknown invite id is refused identically to every other reason
    Given an invite id that was never issued
    When someone opens an accept link with that id
    Then they see the standard "invite is no longer valid" page
    And the response body is byte-identical to the expired-invite refusal
    And nothing reveals whether that id, account, or workspace exists

  Scenario: An invite whose email already has an account is refused without leaking that fact
    Given Dana issued a member invite for an email that already has a Foundry account
    When that invitee opens the link and submits a valid password
    Then they see the standard "invite is no longer valid" page
    And the response body is byte-identical to the expired-invite refusal
    And no second account is created and the invite is not consumed

  Scenario: A weak password is corrected inline without creating an account
    Given Sam has opened his valid member invite for "Northwind"
    When he submits a password below the strength policy
    Then he sees an inline error explaining the password requirement
    And no account is created and his invite is still live
    And no session is created

  Scenario: A mismatched confirmation is corrected inline without creating an account
    Given Sam has opened his valid member invite for "Northwind"
    When his confirmation does not match his new password
    Then he sees an inline error that the passwords do not match
    And no account is created and his invite is still live

  # ------------------------------------------------------------ Security properties

  @property
  Scenario: Concurrent accepts of one member invite create the account exactly once
    Given Sam's member invite is live
    When two accept submissions for the same invite arrive concurrently
    Then exactly one submission creates the account, joins, and signs in
    And the other receives the standard "invite is no longer valid" page
    And exactly one user and one membership are created

  @property
  Scenario: The accept POST is refused without a valid CSRF token
    Given a forged member-accept submission without a valid CSRF token
    When it reaches the accept endpoint
    Then it is refused by the request-forgery protection
    And no invite is consumed, no account is created, and no password is written

  @property
  Scenario: The issuance POST is refused without a valid CSRF token
    Given a forged member-invite submission without a valid CSRF token
    When it reaches the issuance endpoint
    Then it is refused by the request-forgery protection
    And no invite is created

  @property
  Scenario: Invalid-link and email-already-a-user refusals are byte-identical
    Given an expired invite, an already-used invite, a tampered-signature link, an unknown-id link, and an email-already-a-user invite
    When each accept is attempted
    Then all five produce a byte-identical user-visible refusal page
    And they differ only in internal logging, never in the observable response

  @property
  Scenario: Issuance refusals are byte-identical to a generic not-found
    Given a non-admin member request and a signed-out request to the issuance surface
    When each reaches the issuance route
    Then both produce a response byte-identical to a generic not-found
    And neither reveals that an issuance surface exists
