# Feature: keycloak-sso — the operator signs in to foundry with the Keycloak identity
# they already use for the rest of the cluster.
#
# STRICTLY ADDITIVE. The local password door, the bootstrap claim, and the invite
# accept flow are unchanged and stay the break-glass route: Keycloak, LLDAP and
# foundry share a cluster, so an SSO-only tracker is unreachable exactly when the
# operator most needs the issue describing how to fix it (DISCUSS D2).
#
# Keycloak sign-in LINKS to a foundry account that already exists, matched on the
# UNIQUE `users.email_lower` and gated on the provider confirming the address. It
# provisions nothing (D3) — so a realm federating the whole directory cannot quietly
# populate the tracker.
#
# Every refusal is byte-identical to a wrong-password refusal (D7). The callback is
# publicly reachable, so a specific message would turn foundry into an
# account-existence oracle for the entire realm.
#
# Grounding SSOT: docs/feature/keycloak-sso/feature-delta.md (DISCUSS D1-D7 + US-01..05
# + 27 ACs; DESIGN DDD-1..DDD-12 + ADR-OIDC-001/002/003).
#
# Harness: the identity provider is an IN-PROCESS axum double bound on 127.0.0.1:0
# (support/oidc_issuer.rs), signing with a FIXED RSA test keypair — real RS256 crypto,
# fixture key material, exactly as the shipped machine-token keypair is handled. No
# request leaves the test process; the real Keycloak is exercised by slice 03's
# cluster e2e. Postgres is REAL (shared testcontainer + per-scenario schema).

@keycloak-sso
Feature: Signing in to foundry with a cluster identity

  # ---------------------------------------------------------------- US-01 arrival

  @us-01 @walking_skeleton @driving_port @real-io @pending
  Scenario: The operator signs in with their cluster identity and reaches their board
    Given foundry is connected to the cluster identity provider
    And the operator has a foundry account for a confirmed address
    When the operator chooses to sign in with their cluster identity
    And they authenticate with the identity provider
    Then they arrive at their board signed in as themselves

  @us-01 @driving_port @real-io @pending
  Scenario: The sign-in page offers the cluster identity when it is available
    Given foundry is connected to the cluster identity provider
    When a visitor opens the sign-in page
    Then they are offered a way to sign in with their cluster identity

  @us-01 @driving_port @real-io @pending
  Scenario: Each sign-in attempt carries a fresh single-use challenge
    Given foundry is connected to the cluster identity provider
    When the operator begins signing in with their cluster identity twice
    Then each attempt carries a different challenge

  @us-01 @driving_port @real-io @pending
  Scenario: A cluster identity grants exactly what a password grants
    Given the operator has signed in with their cluster identity
    When they file an issue
    Then the issue is recorded as authored by them

  @us-01 @real-io @pending
  Scenario: The challenge is discarded once the sign-in finishes
    Given the operator has signed in with their cluster identity
    Then no challenge remains held by their browser

  # ------------------------------------------------- US-02 who is allowed through

  @us-02 @error @security @driving_port @real-io @pending
  Scenario: An identity with no foundry account is turned away
    Given foundry is connected to the cluster identity provider
    And a person known to the identity provider has no foundry account
    When they authenticate with the identity provider
    Then they are returned to the sign-in page and told nothing more
    And no foundry account has been created for them

  @us-02 @error @security @driving_port @real-io @pending
  Scenario: An unconfirmed address is turned away even when it matches an account
    Given foundry is connected to the cluster identity provider
    And the operator has a foundry account for an address the provider has not confirmed
    When they authenticate with the identity provider
    Then they are returned to the sign-in page and told nothing more

  @us-02 @error @security @driving_port @real-io @pending
  Scenario: A person who belongs to no workspace is turned away
    Given foundry is connected to the cluster identity provider
    And a person has a foundry account but belongs to no workspace
    When they authenticate with the identity provider
    Then they are returned to the sign-in page and told nothing more

  # ------------------------------------------------ US-03 forged and stale arrivals

  @us-03 @error @security @driving_port @real-io @pending
  Scenario: An arrival nobody started is refused
    Given foundry is connected to the cluster identity provider
    When someone arrives claiming to have signed in, having never begun
    Then they are returned to the sign-in page and told nothing more

  @us-03 @error @security @driving_port @real-io @pending
  Scenario: An arrival that does not match the challenge it answers is refused
    Given the operator has begun signing in with their cluster identity
    When they arrive answering a different challenge
    Then they are returned to the sign-in page and told nothing more

  @us-03 @error @security @driving_port @real-io @pending
  Scenario: An identity answering a stale challenge is refused
    Given the operator has begun signing in with their cluster identity
    When the identity provider vouches for them against an earlier challenge
    Then they are returned to the sign-in page and told nothing more

  @us-03 @error @security @driving_port @real-io @pending
  Scenario: An identity signed by an unknown key is refused
    Given the operator has begun signing in with their cluster identity
    When an identity signed by a key the provider does not publish arrives
    Then they are returned to the sign-in page and told nothing more

  @us-03 @error @security @driving_port @real-io @pending
  Scenario: An identity vouched for by a different provider is refused
    Given the operator has begun signing in with their cluster identity
    When an identity naming a different provider arrives
    Then they are returned to the sign-in page and told nothing more

  @us-03 @error @security @driving_port @real-io @pending
  Scenario: An identity that has already expired is refused
    Given the operator has begun signing in with their cluster identity
    When an identity whose validity has already lapsed arrives
    Then they are returned to the sign-in page and told nothing more

  # AC-3.5 — the property is NOT provided by discarding the challenge (a client may
  # keep it); it is provided by the provider accepting each authorisation once. The
  # scenario therefore replays a GENUINE completed sign-in, so it exercises the
  # mechanism that actually holds (feature-delta.md § Changed Assumptions).
  @us-03 @error @security @driving_port @real-io @pending
  Scenario: Replaying a completed sign-in is refused
    Given the operator has signed in with their cluster identity
    When that same sign-in is presented a second time
    Then they are returned to the sign-in page and told nothing more
    And their original session is untouched

  @us-03 @error @driving_port @real-io @pending
  Scenario: An unreachable provider refuses the sign-in rather than breaking
    Given foundry is connected to the cluster identity provider
    And the identity provider cannot be reached
    When the operator tries to sign in with their cluster identity
    Then they are returned to the sign-in page and told nothing more
    And foundry keeps serving every other page

  @us-02 @us-03 @security @driving_port @real-io @pending
  Scenario: Every refusal looks identical, whoever is refused
    Given foundry is connected to the cluster identity provider
    When each way of being turned away is attempted in turn
    Then every one of them is answered identically
    And a wrong password is answered identically too

  # ------------------------------------------------------- US-04 the door that stays

  @us-04 @driving_port @real-io @pending
  Scenario: The password door still opens while cluster identity is available
    Given foundry is connected to the cluster identity provider
    And the operator has a foundry account for a confirmed address
    When they sign in with their foundry password
    Then they arrive at their board signed in as themselves

  @us-04 @driving_port @real-io @pending
  Scenario: A fresh instance can still be claimed with the provider unreachable
    Given foundry is connected to the cluster identity provider
    And the identity provider cannot be reached
    When the first operator claims the instance
    Then they arrive at their board signed in as themselves

  @us-04 @driving_port @real-io @pending
  Scenario: Either door leads to the same person
    Given the operator has signed in with their cluster identity
    When they sign out and sign in again with their foundry password
    Then they arrive at their board signed in as themselves

  # --------------------------------------------- US-05 running without a provider

  @us-05 @driving_port @real-io @pending
  Scenario: With no provider configured foundry serves as it always did
    Given foundry is not connected to any cluster identity provider
    When a visitor opens the sign-in page
    Then they are not offered a way to sign in with a cluster identity
    And foundry reports itself healthy and ready

  @us-05 @error @driving_port @real-io @pending
  Scenario: Asking for cluster identity when none is configured is refused
    Given foundry is not connected to any cluster identity provider
    When someone asks to sign in with a cluster identity
    Then they are returned to the sign-in page and told nothing more

  @us-05 @error @pending
  Scenario: A half-configured provider stops foundry from starting
    Given foundry is given a provider address but no credential for it
    When foundry starts
    Then it refuses to start and names the missing credential
