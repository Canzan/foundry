# Story: US-W06 — Lock the web/api boundary with a structural guard
# Feature A "Programmatic Foundry" — Slice 2 (@infrastructure, folded in)
# JTBD: jtbd-web-2 (the boundary cannot erode)
#
# Driving adapter: the boundary guard, exercised as the maintainer runs it —
# a subprocess invocation of `cargo xtask check-arch` (the AST layer) and the
# `cargo-deny` crate-graph rule (the dependency-direction layer). See
# design/boundary-guard.md (three orthogonal layers: AST source walk for
# api-emits-no-markup + the JWT algorithm pin, cargo-deny for dependency
# direction, an injected-violation gold test that proves the guard bites).
#
# These scenarios run a real subprocess against the real source tree (and,
# for the injected cases, against a throwaway copy with a violation planted),
# NOT the foundry binary. They are example-based (layer 4+, per the layered
# test discipline): each violation is one named example, never generated.
# Tagged @boundary-guard so the runner can shard them onto the lint lane.

@feature-a @us-w06 @infrastructure @boundary-guard
Feature: The web/api boundary is enforced as a check, not a review chore
  The maintainer runs one check that fails the build when the data-API source
  constructs a page, when an adapter reaches the database directly instead of
  going through the shared service layer, or when the credential verifier
  would accept a disallowed signing algorithm. On a clean tree the check
  passes with no manual steps. A deliberately planted violation makes the
  check fail, proving the guard actually bites.

  @real-io @boundary-guard
  Scenario: A clean tree passes the boundary check
    Given the project tree has no boundary violations
    When the maintainer runs the boundary check
    Then the check passes

  @error @real-io @boundary-guard
  Scenario: A page constructed in the data-API tier fails the check
    Given a copy of the tree in which a data-API handler is changed to build a page
    When the maintainer runs the boundary check on that copy
    Then the check fails
    And it names the handler that builds a page

  @error @real-io @boundary-guard
  Scenario: An adapter reaching the database directly fails the check
    Given a copy of the tree in which the data-API adapter declares a direct dependency on the persistence layer
    When the maintainer runs the boundary check on that copy
    Then the check fails
    And it names the forbidden dependency

  @error @real-io @boundary-guard @nfr-web-api-sec-02
  Scenario: A credential verifier that would accept a disallowed algorithm fails the check
    Given a copy of the tree in which the credential verifier is changed to accept any signing algorithm
    When the maintainer runs the boundary check on that copy
    Then the check fails
    And it reports the credential verifier no longer pins the single allowed algorithm

  @error @real-io @boundary-guard @us-tma05
  Scenario: A data-API mint surface fails the check
    Given a copy of the tree in which a data-API handler is changed to mint a token
    When the maintainer runs the boundary check on that copy
    Then the check fails
    And it names the handler that mints a token

  @error @real-io @boundary-guard @us-tma05
  Scenario: A multi-line POST on the tokens collection cannot evade the check
    Given a copy of the tree in which a multi-line route block registers a POST on the tokens collection
    When the maintainer runs the boundary check on that copy
    Then the check fails
    And it names the handler that registers a mint POST
