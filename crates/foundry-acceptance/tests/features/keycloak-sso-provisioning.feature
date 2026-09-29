# Feature: keycloak-sso provisioning — a cluster identity holding the provision role
# gets a foundry account on its first sign-in.
#
# SUPERSEDES the "provision nothing" half of D3 (D3a, 2026-09-27): role-gated,
# opt-in provisioning. With FOUNDRY_OIDC_PROVISION_ROLE unset foundry behaves
# exactly as before — an identity with no foundry account is refused. With it set,
# a confirmed address whose ID token carries that Keycloak realm role
# (`realm_access.roles`) is given a `member` account in the instance's original
# workspace, with NO usable password, then signed in through the same seam as every
# other sign-in.
#
# Every refusal is the generic wrong-password refusal (D7): the callback is public,
# so neither account existence nor role membership may leak. "Turned away exactly as
# a wrong password is" is checked byte-for-byte against a real wrong-password answer.
#
# Harness: the SHIPPED foundry_oidc::OidcProvider talks over a real socket to the
# in-process RS256 provider double (support/oidc_issuer.rs). Postgres is REAL.
#
# Every scenario runs (DISTILL scaffolded them @pending, ADR-025; DELIVER un-pended
# them all). Scenarios 3-7 are GUARDS: they pin behaviour D3a must preserve and
# already passed against the link-only callback — they run alongside scenario 1 so
# they guard the provisioning code, not the absence of it.
# Blank-means-off for FOUNDRY_OIDC_PROVISION_ROLE is a configuration-parsing rule and
# is specified at unit level in foundry-oidc, not here.

@keycloak-sso @keycloak-sso-provisioning
Feature: A cluster identity holding the provision role is given a foundry account

  # 1
  @us-06 @driving_port @real-io
  Scenario: A newcomer holding the provision role is given an account and signed in
    Given foundry provisions holders of the "foundry-user" realm role
    And the instance also has a newer workspace named "Side Project"
    And a newcomer named "Nia Newcomer" is confirmed by the identity provider
    And the identity provider grants the newcomer the "foundry-user" realm role
    When the newcomer signs in through the identity provider
    Then the newcomer arrives signed in to the board
    And the newcomer is greeted as "Nia Newcomer"
    And the newcomer is an ordinary member of the instance's original workspace

  # 2
  @us-06 @driving_port @real-io
  Scenario Outline: A newcomer the provider gives no usable full name is still greeted by a name
    Given foundry provisions holders of the "foundry-user" realm role
    And a newcomer is confirmed by the identity provider as "<full_name>" with username "<username>"
    And the identity provider grants the newcomer the "foundry-user" realm role
    When the newcomer signs in through the identity provider
    Then the newcomer arrives signed in to the board
    And the newcomer is greeted as "<greeting>"

    Examples:
      | full_name                                                            | username | greeting     |
      |                                                                      | nia      | nia          |
      |                                                                      |          | nia.newcomer |
      | Anastasia Wilhelmina Theodora Nightingale-Featherstonehaugh Newcomer | nia      | nia          |

  # 3
  @us-06 @error @security @driving_port @real-io
  Scenario Outline: A newcomer without the provision role is turned away
    Given foundry provisions holders of the "foundry-user" realm role
    And a newcomer named "Nia Newcomer" is confirmed by the identity provider
    And the identity provider grants the newcomer <grant>
    When the newcomer signs in through the identity provider
    Then the newcomer is turned away exactly as a wrong password is
    And no foundry account exists for the newcomer

    Examples:
      | grant                            |
      | the "some-other-role" realm role |
      | no realm roles                   |

  # 4
  @us-06 @error @security @driving_port @real-io
  Scenario: With provisioning switched off a newcomer is turned away even holding the role
    Given foundry provisions nobody from the cluster identity provider
    And a newcomer named "Nia Newcomer" is confirmed by the identity provider
    And the identity provider grants the newcomer the "foundry-user" realm role
    When the newcomer signs in through the identity provider
    Then the newcomer is turned away exactly as a wrong password is
    And no foundry account exists for the newcomer

  # 5
  @us-06 @error @security @driving_port @real-io
  Scenario: A newcomer whose address the provider has not confirmed is turned away
    Given foundry provisions holders of the "foundry-user" realm role
    And a newcomer named "Nia Newcomer" is known to the identity provider at an unconfirmed address
    And the identity provider grants the newcomer the "foundry-user" realm role
    When the newcomer signs in through the identity provider
    Then the newcomer is turned away exactly as a wrong password is
    And no foundry account exists for the newcomer

  # 6
  @us-06 @error @security @driving_port @real-io
  Scenario: Before anyone has claimed the instance a newcomer holding the role is turned away
    Given foundry provisions holders of the "foundry-user" realm role on an instance nobody has claimed
    And a newcomer named "Nia Newcomer" is confirmed by the identity provider
    And the identity provider grants the newcomer the "foundry-user" realm role
    When the newcomer signs in through the identity provider
    Then the newcomer is turned away exactly as a wrong password is
    And no foundry account exists for the newcomer

  # 7
  @us-06 @driving_port @real-io
  Scenario: A member who already has an account keeps it when they hold the provision role
    Given foundry provisions holders of the "foundry-user" realm role
    And a member named "Pat Operator" already has a foundry account with a password
    And the identity provider confirms the member under a differently capitalised address as "Patricia Operator"
    And the identity provider grants the member the "foundry-user" realm role
    When the member signs in through the identity provider
    Then the member arrives signed in to the board
    And the member is greeted as "Pat Operator"
    And the member still has exactly one foundry account
    And the member's own password still lets them in

  # 8
  @us-06 @error @security @driving_port @real-io
  Scenario Outline: A provisioned account has no password to sign in with
    Given foundry provisions holders of the "foundry-user" realm role
    And a newcomer named "Nia Newcomer" is confirmed by the identity provider
    And the identity provider grants the newcomer the "foundry-user" realm role
    And the newcomer has been given an account through the identity provider
    When the newcomer tries the password form with "<password>"
    Then the newcomer is turned away exactly as a wrong password is

    Examples:
      | password                  |
      |                           |
      | a-guess-at-twelve-or-more |

  # 9
  @us-06 @error @security @driving_port @real-io
  Scenario: The password form takes as long to refuse a provisioned account as an unknown address
    Given foundry provisions holders of the "foundry-user" realm role
    And a newcomer named "Nia Newcomer" is confirmed by the identity provider
    And the identity provider grants the newcomer the "foundry-user" realm role
    And the newcomer has been given an account through the identity provider
    When password sign-in latency is sampled over 7 interleaved attempts for the newcomer and for an unknown address
    Then the median newcomer latency is within 150ms of the median unknown-address latency

  # 10
  @us-06 @driving_port @real-io
  Scenario: A provisioned member can choose a password of their own through a reset
    Given foundry provisions holders of the "foundry-user" realm role
    And a newcomer named "Nia Newcomer" is confirmed by the identity provider
    And the identity provider grants the newcomer the "foundry-user" realm role
    And the newcomer has been given an account through the identity provider
    When a visitor submits the forgot-password form with email "nia.newcomer@example.test"
    And the newcomer chooses the password "nia-chose-this-password" through the emailed reset link
    And the newcomer tries the password form with "nia-chose-this-password"
    Then the newcomer arrives signed in to the board
    And the newcomer is greeted as "Nia Newcomer"

  # 11 — pins today's deliberate behaviour while OD-10 (role revocation) stays open.
  @us-06 @driving_port @real-io
  Scenario: A member provisioned earlier still signs in after the provision role is withdrawn
    Given foundry provisions holders of the "foundry-user" realm role
    And a newcomer named "Nia Newcomer" is confirmed by the identity provider
    And the identity provider grants the newcomer the "foundry-user" realm role
    And the newcomer has been given an account through the identity provider
    And the identity provider no longer grants the newcomer the "foundry-user" realm role
    When the newcomer signs in through the identity provider
    Then the newcomer arrives signed in to the board
    And the newcomer still has exactly one foundry account
