# Feature: Invite Accept — a provisioned first-admin claims their account
#
# Persona: Priya Nair (priya.nair@northwind.example), first-admin of the freshly
# provisioned "Northwind" workspace. The invite link is the ONLY way she can
# establish a credential and sign in.
#
# Scope (v1): first-admin invites only. The uniform refusal body MUST be
# byte-identical across all invalid-link reasons (non-enumerable, security crux).

Feature: Invite accept for a provisioned first-admin
  As the first-admin of a freshly provisioned workspace
  I want to set my password from my invite link and be signed in
  So that I can actually get into my workspace instead of hitting a dead URL

  Background:
    Given a super-admin provisioned the "Northwind" workspace
    And Priya Nair was seeded as its first-admin with an invite link valid for 7 days

  # ---------------------------------------------------------------- Happy path

  Scenario: First-admin opens a valid invite and sees a set-password form
    Given Priya's invite has not expired and has not been used
    When Priya opens her invite link
    Then she sees a set-password form
    And the form names the "Northwind" workspace
    And no password has yet been set on her account

  Scenario: Setting a valid password consumes the invite and signs her in
    Given Priya has opened her valid invite for "Northwind"
    When she submits a password meeting the strength policy and confirms it
    Then she is signed in without a separate login step
    And she lands on the "Northwind" workspace dashboard
    And she sees no data from any other workspace

  Scenario: A consumed invite can never be used again
    Given Priya has already set her password via her invite link
    When Priya opens the same invite link again
    Then she sees the standard "invite is no longer valid" page
    And no new password is set and no session is created

  # ---------------------------------------------------------------- Sad paths

  Scenario: Expired invite is refused without leaking account existence
    Given Priya's invite expired 1 day ago
    When Priya opens her invite link
    Then she sees the standard "invite is no longer valid" page
    And the page does not reveal whether any account or workspace exists
    And the page advises asking the instance administrator to re-issue the invite

  Scenario: Tampered signature is refused
    Given Priya's invite is live
    But the signature in the link has been altered by one character
    When Priya opens the tampered link
    Then she sees the standard "invite is no longer valid" page
    And the response body is byte-identical to the expired-invite refusal

  Scenario: Unknown invite id is refused identically to every other invalid reason
    Given an invite id that was never issued
    When someone opens an accept link with that id
    Then they see the standard "invite is no longer valid" page
    And the response body is byte-identical to the expired-invite refusal
    And nothing reveals whether that id, account, or workspace exists

  Scenario: Weak password is corrected inline without consuming the invite
    Given Priya has opened her valid invite for "Northwind"
    When she submits a password below the strength policy
    Then she sees an inline error explaining the password requirement
    And her invite is still live and can still be used
    And no session is created

  Scenario: Mismatched confirmation is corrected inline without consuming the invite
    Given Priya has opened her valid invite for "Northwind"
    When her confirmation does not match her new password
    Then she sees an inline error that the passwords do not match
    And her invite is still live and can still be used

  # ---------------------------------------------------------------- Security properties

  @property
  Scenario: Concurrent accepts of one invite succeed exactly once
    Given Priya's invite is live
    When two accept submissions for the same invite arrive concurrently
    Then exactly one submission sets the password and signs in
    And the other receives the standard "invite is no longer valid" page
    And the invite is recorded as used exactly once

  @property
  Scenario: The accept POST is refused without a valid CSRF token
    Given a forged accept submission without a valid CSRF token
    When it reaches the accept endpoint
    Then it is refused by the request-forgery protection
    And no invite is consumed and no password is written

  @property
  Scenario: Invalid-link refusals are byte-identical across all reasons
    Given an expired invite, an already-used invite, a tampered-signature link, and an unknown-id link
    When each is opened
    Then all four produce a byte-identical user-visible refusal page
    And they differ only in internal logging, never in the observable response
