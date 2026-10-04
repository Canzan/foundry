# Evolution: fix-oidc-empty-nonce (the nonce check could not tell "absent" from "equal")

**Finalized:** 2026-10-04.
**Commit:** DELIVER `51b62cd`, one DES-monitored step.
**Origin:** found by keycloak-sso phase 03 (OD-11) while killing named fault F10 (`0c2553d`), then
run through `/nw-bugfix`. The RCA is `docs/feature/fix-oidc-empty-nonce/rca.md`. The user approved
fix direction (a)+(b)+(c) on 2026-10-04. Trunk-based, no PR. DES integrity exit 0.

## Defect: latent, not reachable today

`OidcProvider::exchange_code` checked that the nonces were *equal* (`claims.nonce != req.nonce`), not
that both were *present*. `IdTokenClaims.nonce` is `#[serde(default)]`, so an ID token with no nonce
claim reads as `""`. Given an `AuthRequest` whose nonce was also `""`, the comparison `"" != ""` failed
to refuse, and the identity was accepted.

No legitimate or external path reaches this:
- Every challenge comes from `AuthRequest::generate()`, which uses 43 random base64url characters.
- The challenge is sealed with an HMAC over `SESSION_SECRET`.
- A missing or unsigned cookie is refused before the exchange.

It became live only when F10 seeded "trust a missing challenge cookie". At that point the cookie check
was the sole guard against code injection and login CSRF, despite the docs calling the nonce an
"independent second layer". It is not independent: the expected nonce comes only from that cookie.

## Fix: `crates/foundry-oidc/src/lib.rs` only

- **(a) `AuthRequest::ensure_issued`:** the first line of `exchange_code`, before discovery or any
  provider call. It refuses an empty `state`, `nonce` or `code_verifier`. A blank challenge never
  spends the code at the provider and never sends an empty PKCE verifier.
- **(b) `ensure_nonce(expected, presented)`:** refuses when either is empty or they differ. It
  replaces the inline comparison. `serde(default)` is kept so that a missing claim reaches this check.
- **(c) The doc comment** now calls the nonce a second layer that depends on the challenge cookie.

Both refuse with the existing `OidcError::Untrusted`, which the app routes through its single
`refuse()`. Refusals stay byte-identical to a wrong password (D7). There is no new error variant, no
`foundry-app` change and no Gherkin change.

## Regression tests

Three unit tests are in `foundry-oidc`'s `mod tests`, the boundary where the defect is directly
reachable:
- `an_unissued_challenge_is_refused_before_the_exchange`: empty state, nonce or verifier against a
  dead issuer. It was RED with `Transport` (it reached the network); it is now `Untrusted`.
- `only_a_present_and_matching_nonce_is_answered`: a table that includes `("","")`, the defect.
- `an_identity_with_no_nonce_claim_answers_no_challenge`: a real Keycloak-shaped payload with the
  `nonce` key removed.

foundry-oidc: 14/14. Named faults N1–N4 (guard removed, equality-only, presence-only, nonce-only guard):
4/4 killed.

There is no acceptance scenario, by design. With the cookie check intact the defect cannot be reached
end to end, so a new scenario would pass before the fix too. The keycloak-sso lanes (38/38) and
provisioning lanes (15/15) are the no-regression guard.

## Gates

- keycloak-sso 38/38 and keycloak-sso-provisioning 15/15.
- fmt, clippy `-D warnings` and check-arch pass.
- smoke is green on the second run. The first hit the known testcontainers `PortNotExposed` flake in
  `foundry-services`, which passed 7/7 alone.
- Full `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci` on `51b62cd`: **GREEN**, all gates, 903/903 scenarios and 6268/6268 steps, browser lane run. The first attempt
  stopped at the workspace test gate on the same testcontainers `PortNotExposed` flake, this time in
  `foundry-services` `revoke_and_list_use_cases`. The full rerun was green.

## Owed and follow-ups

- **Owed by the user:** one manual sign-in through the real Keycloak after v0.6.2 deploys. The new
  nonce check is unverified against real Keycloak in this repo, though Keycloak echoes the nonce it
  is sent.
- **Not approved, left as options:**
  - Make `AuthRequest`'s fields private behind a validating constructor.
  - Have `unseal` reject empty fields.
  - Add a PKCE verifier check and a `NoNonce` variant to the test issuer, plus a DISTILL example
    beside the stale-challenge scenario.
- **Unknown:** whether Keycloak's foundry client *requires* PKCE. No client setting is checked into
  the repo. Foundry no longer sends an empty verifier either way.
