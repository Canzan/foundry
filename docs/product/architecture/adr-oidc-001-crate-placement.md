# ADR-OIDC-001 — OIDC lives in its own crate, with its own algorithm pin

- Status: Accepted (2026-08-21)
- Feature: `keycloak-sso` (DDD-1, DDD-2, DDD-3)

## Context

foundry adds Keycloak sign-in. Keycloak signs ID tokens with **RS256**; foundry's
machine tokens are **EdDSA**, and `xtask/src/check_arch.rs::check_jwt_alg_pin` exists
to keep them that way. That guard scans `crates/foundry-auth/src`, fires on any file
constructing a `jsonwebtoken::Validation`, and requires an `algorithms` list that
mentions `EdDSA` and no other algorithm — `RS256` is in its explicit reject list.
Its helper `pins_algorithms_to_eddsa` inspects only the FIRST `algorithms` list it
finds in a file.

So RS256 validation inside `foundry-auth` fails `cargo xtask ci` by construction, and
the guard's file-scoped shape cannot express "EdDSA here, RS256 there".

## Decision

1. A new `crates/foundry-oidc` owns the OIDC protocol: discovery, JWKS cache, PKCE,
   token exchange, and RS256-pinned ID-token validation. It depends on neither
   `foundry-auth` nor `foundry-store`.
2. It is implemented over `reqwest` and `jsonwebtoken`, both already workspace
   dependencies. `DecodingKey::from_jwk` covers JWKS→key, so there is **no new runtime
   dependency**.
3. `check_arch` gains `check_oidc_alg_pin`, a sibling scanner over
   `crates/foundry-oidc/src` requiring `algorithms = [RS256]` and rejecting `none`,
   `HS*`, `ES*` and `PS*`.
4. `deny.toml` bans `foundry-oidc` outside `foundry-app` and `foundry-acceptance`.

## Alternatives

**Adopt the `openidconnect` crate.** Discovery, rotation, PKCE and validation arrive
audited, which is the strongest argument available and nearly decided it. Rejected
because validation would then happen inside a dependency, where no `check-arch` rule
can see the algorithm pin. foundry's whole defence against algorithm confusion is a
build-time scanner over first-party source; moving the credential check out of that
scanner's reach trades a loud failure mode for a silent one. The large transitive tree
behind the `cargo deny` gate is a secondary cost, not the reason.

**Widen `check_jwt_alg_pin` and keep OIDC in `foundry-auth`.** One crate, one guard.
Rejected because it edits the guard protecting machine tokens so that RS256 is
tolerated somewhere in the file — and given the first-`algorithms`-list heuristic, the
most likely implementation weakens the EdDSA pin rather than adding a second one.

**Put the handlers and protocol together in `foundry-app`.** Rejected: `foundry-app`
is not scanned by either pin, so the ID-token algorithm would be unpinned entirely.

## Consequences

Two credential classes carry two independent, per-class build-time pins that fail
independently. The cost is a hand-written JWKS fetch, cache and rotation path — real
work, and the place a bug is most likely. It is deliberately placed under a guard and
behind acceptance scenarios that mint wrong-algorithm and wrong-key tokens.
