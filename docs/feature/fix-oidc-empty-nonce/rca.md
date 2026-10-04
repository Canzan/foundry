# RCA: `foundry-oidc` accepts an ID token with no nonce when the expected nonce is empty

**Date:** 2026-10-03. **Found by:** keycloak-sso phase 03, while killing named fault F10
(`0c2553d`). See `docs/feature/keycloak-sso/feature-delta.md:1340-1346` and
`docs/evolution/2026-09-27-keycloak-sso.md:366-370`.
**Investigator:** nw-troubleshooter. The RCA was not peer-reviewed. Production code is
unchanged. No fix direction has been approved yet.

## Symptom

`OidcProvider::exchange_code` returns `Ok(identity)` for an RS256-valid ID token that
carries no `nonce` claim, as long as the `AuthRequest` it was given has `nonce == ""`.

- `IdTokenClaims.nonce` is `#[serde(default)] String`
  (`crates/foundry-oidc/src/lib.rs:238-239`). A missing claim deserialises as `""`.
- The only nonce check is `if claims.nonce != req.nonce` (`lib.rs:428`). The check reads
  `"" != ""`, which is false, so the token passes.

**Reachability today: none by a legitimate or external path.** Only the app layer builds
the `AuthRequest`, and every route gives it a non-empty nonce:

- `start` calls `AuthRequest::generate()` (`crates/foundry-app/src/oidc.rs:139`).
- `generate()` sets `state`, `nonce` and `code_verifier` independently, each from
  `random_token()` (`lib.rs:163-169`). That is 32 bytes from `thread_rng`, encoded as
  43-character base64url (`lib.rs:178-183`). It is never empty.
- `callback` reads the challenge only through `read_challenge` → `unseal`
  (`oidc.rs:120-130`, `106-118`), and `unseal` requires an HMAC over `SESSION_SECRET`
  (`oidc.rs:108`). The only signer is `seal`, which serialises a `generate()` result
  (`oidc.rs:94-104`, `144`).
- If the cookie is missing or unsigned, the callback refuses before the exchange
  (`oidc.rs:174-176`).

The defect is a **latent fail-open at a library boundary**. It becomes live as soon as
any app-layer change lets an empty or synthesised challenge through. F10 was exactly
that change: "missing challenge cookie trusted" (`feature-delta.md:1327`). Then the only
guard against code injection and login CSRF is the challenge cookie at `oidc.rs:174`.

Compromising `SESSION_SECRET` is **not** made worse by this defect. An attacker with
the secret can seal any nonce, including the one in their own provider-begun flow. The
hardening below does not defend against that, and does not claim to.

## Root cause chain

### Branch A: the library compares, but never checks that there is anything to compare

1. **WHY 1 (symptom).** A token with no nonce is accepted against an empty expected
   nonce. [`lib.rs:428`: the only check is `claims.nonce != req.nonce`.]
2. **WHY 2.** "Absent" and "empty" cannot be told apart on either side.
   - Token side: `serde(default)` turns a missing claim into `""` (`lib.rs:238-239`).
   - Request side: `AuthRequest.nonce` is a plain `String` (`lib.rs:158`).
   - Two empties therefore compare equal, and "no challenge" matches "no claim".
3. **WHY 3.** `exchange_code` trusts its `AuthRequest` input entirely
   (`lib.rs:395-439`).
   - It checks no precondition on `state`, `nonce` or `code_verifier`.
   - It sends `code_verifier` as given, `""` included (`lib.rs:407`).
4. **WHY 4.** The invariant "each field is a fresh, non-empty random value" is set up by
   the constructor `generate()` (`lib.rs:163-169`), but the type does not enforce it.
   - All three fields are `pub` (`lib.rs:156-160`), so any caller can build an
     `AuthRequest { nonce: "".into(), .. }`.
   - `unseal` does exactly that from parsed JSON, with no emptiness check
     (`oidc.rs:113-117`).
5. **Root cause A.** `foundry-oidc` models the one-time challenge as an unvalidated
   struct with public fields. It also treats "nonce equal" as sufficient, when the rule
   is "the nonce is present and equal".
   - OIDC Core 1.0 §3.1.3.7 step 11 says that when a nonce was sent, the ID token's
     nonce claim MUST be present and MUST match. foundry always sends one
     (`lib.rs:344-350`), so for foundry the claim is mandatory.
   - The code implements only the "match" half, and an empty value satisfies it.

### Branch B: the app layer is the single effective guard, and nothing says so

1. **WHY 1.** Under F10 the forged arrival authenticates. F10 survived 29 of 29
   scenarios at 03-03 (`feature-delta.md:1327`). It was killed only after `0c2553d`
   seeded an account the forgery could reach.
2. **WHY 2.** After the cookie is gone, no later layer refuses the arrival:
   - The state check passes, because the synthesised challenge copies the arrival's
     `state` (`0c2553d` step doc: "a challenge synthesised from the arrival itself").
   - The nonce check passes as in Branch A.
   - PKCE adds nothing in the test lane. The issuer double never reads
     `code_verifier` (`crates/foundry-acceptance/src/support/oidc_issuer.rs:321-335`;
     the token handler checks only `spent_codes`).
3. **WHY 3.** The design calls the nonce "the independent second layer"
   (`lib.rs:391-394`, `feature-delta.md:630`, `:675`). That layer depends on the same
   cookie it is supposed to back up, because the expected nonce comes only from that
   cookie (`oidc.rs:174`, `187`). The two layers are not independent.
4. **WHY 4.** The design's threat analysis checks *mismatched* values, never *absent*
   ones.
   - The error-path list names "`state` absent or mismatched; `nonce` mismatched"
     (`feature-delta.md:91-93`), so the nonce has no "absent" case.
   - AC-3.3 says only "An ID token whose `nonce` does not match the cookie is refused"
     (`feature-delta.md:154`).
   - ADR-OIDC-002 rejects an *unsigned* cookie because "state and nonce only defend
     against forgery if the client cannot choose them"
     (`docs/product/architecture/adr-oidc-002-state-carrier.md`, Alternatives). It
     does not consider the server producing an empty value.
   - DDD-1..12 (`feature-delta.md:432-445`) and ADR-OIDC-001/003 do not mention nonce
     presence.
5. **Root cause B.** The design rests the "independence" of its defence layers on an
   assertion, without a stated per-layer invariant.
   - The app layer guards empty `state` explicitly: `q.state.is_empty() || q.state !=
     req.state` (`oidc.rs:177`).
   - The library has no equivalent for the nonce. The asymmetry went unnoticed because
     nothing tests the nonce layer with the cookie layer removed.

### Branch C: the test double cannot express the defect, so no test saw it

1. **WHY 1.** None of the 23 base scenarios in `keycloak-sso.feature` and none of the
   12 outlines in `keycloak-sso-provisioning.feature` fails today because of this
   defect.
2. **WHY 2.** Every arrival that reaches `exchange_code` goes through a real `start`,
   so the expected nonce is non-empty. Examples are `arrive_wrong_state`
   (`feature_keycloak_sso.rs:410-434`), `complete_minting` (`:460-463`) and replay
   (`:466-491`). "An arrival nobody started" (`arrive_without_starting`, `:385`) is refused at
   `oidc.rs:175`, before the exchange.
3. **WHY 3.** The double can only mint a token whose nonce claim is *present*.
   - `Claims.nonce` is a non-optional `String`, always serialised
     (`oidc_issuer.rs:310`).
   - Its value falls back to `""` when no `/authorize` was recorded
     (`oidc_issuer.rs:338-341`).
   - There is a `StaleNonce` variant (wrong nonce), but no "no nonce" variant.
4. **WHY 4.** DDD-12 places all protocol fidelity in the double
   (`feature-delta.md:445`, `:621`), and `foundry-oidc` has no test of
   `exchange_code`. Its unit tests cover config, PKCE derivation, claim extraction and
   the provision role only (`lib.rs:489-656`). `exchange_code` and `validate_id_token`
   are reachable only through HTTP, and no in-crate HTTP double exists. Its
   `Cargo.toml` has no `[dev-dependencies]`.
5. **Root cause C.** The library's validation rules have no test at their own
   boundary. Every check is tested only end to end, through an app layer that already
   filters out the degenerate input. So a library-level fail-open can only be seen by
   first breaking the app layer, which is what F10 did by accident.

### Backwards validation

- **A → symptom.** With `nonce: ""` and no claim, `"" != ""` is false, so the function
  returns `Ok`. Yes.
- **B → F10 survival.** With the cookie guard removed and no account in the world, the
  forgery is refused downstream for the wrong reason (`feature-delta.md:1327`). Yes.
- **C → late discovery.** No test supplies an empty `AuthRequest` or a nonce-less
  token, so the defect is invisible in green CI. Yes.
- The three branches do not contradict each other. A is the defect, B is why it matters
  and was not designed out, and C is why it shipped.

## Contributing factors

- **PKCE in the forged scenario.** Under an F10-style synthesis the `code_verifier`
  is also `""`, and `exchange_code` sends `code_verifier=` (`lib.rs:407`).
  - Whether Keycloak refuses that depends on Keycloak, not on foundry.
    *Hypothesis, requires verification against the cluster:*
    - If the attacker's code was issued with a `code_challenge`, Keycloak requires a
      matching verifier and refuses.
    - If it was issued without one, Keycloak's handling of an empty or unexpected
      verifier is unverified. RFC 7636 §4.1 requires 43 to 128 characters, so a
      conformant server should refuse `""`.
  - foundry relies on that refusal implicitly, and the issuer double does not model it
    (`oidc_issuer.rs:321-335`).
  - No Keycloak client config (`pkce.code.challenge.method`) is checked into this repo
    (`git grep` finds nothing). Whether PKCE is *required* on the client is unknown.
- **The comparison is not constant-time** (`lib.rs:428`, `String` `!=`). This is not a
  material risk. The nonce travels in the front channel (`lib.rs:350`), and a timing
  oracle would need provider-signed tokens with chosen nonces. Note it, but take no
  action.
- **"Second layer" wording.** It appears in a doc comment (`lib.rs:394`) and in two
  design sections (`feature-delta.md:630`, `:675`). The repetition made the
  independence look established.

## Proposed fix (minimal, `foundry-oidc` only)

All refusals return `OidcError::Untrusted`. The app maps every `exchange_code` error to
`refuse(&state, &headers, &format!("exchange: {err}"))` (`oidc.rs:187-190`). That path
gives the D7/DDD-11 generic refusal, byte-identical to a wrong password, and writes the
existing `oidc sign-in refused` reason log line (`oidc.rs:60-61`). No app-layer change is
needed.

1. **Precondition guard: refuse an incomplete challenge before any network I/O.** At the
   top of `exchange_code`, before `self.discovery()` (`lib.rs:400`), add:

   ```rust
   if req.state.is_empty() || req.nonce.is_empty() || req.code_verifier.is_empty() {
       return Err(OidcError::Untrusted(
           "the sign-in answers no challenge we issued".to_string(),
       ));
   }
   ```

   Ideally make this a pure `fn` on `AuthRequest`, e.g.
   `fn ensure_issued(&self) -> Result<(), OidcError>`, so it can be unit-tested without
   HTTP. Refusing before the exchange also means a degenerate request never spends the
   victim's code at the provider and never sends `code_verifier=`.
2. **Presence-and-match nonce check.** Replace `lib.rs:428-432` with a pure helper, for
   example:

   ```rust
   fn ensure_nonce(expected: &str, presented: &str) -> Result<(), OidcError> {
       if expected.is_empty() || presented.is_empty() || presented != expected { Err(Untrusted(..)) } else { Ok(()) }
   }
   ```

   Keep the existing message, "the identity answers a challenge we did not issue". This
   implements OIDC Core §3.1.3.7(11) in full: present AND equal. Keep
   `serde(default)` on `nonce` (`lib.rs:238`) so an absent claim still deserialises and
   is refused by this rule, rather than failing as an unreadable token. Either way the
   result is a generic refusal. Keeping one reason makes the log line more useful.
3. **Doc comment.** At `lib.rs:391-394`, state the invariant that is now enforced: the
   nonce must be present on both sides. Drop the word "independent", or qualify it to
   say the layer is independent of the cookie's *contents* but not of its *existence*.

**Deliberately not in the minimal fix (optional follow-ups):**

- Make `AuthRequest` fields private, with a validating constructor that `unseal` would
  use. This enforces root cause A by type and is the stronger permanent fix. It touches
  `oidc.rs:113-117` and every `pub` field reader, so it is a larger change.
- Make `unseal` return `None` for empty fields (`oidc.rs:113-117`), as defence in depth
  at the app layer.
- Add PKCE verification to the issuer double, so the lane models Keycloak's PKCE
  refusal.

## Files affected

- `crates/foundry-oidc/src/lib.rs`: the guard in `exchange_code` (around `:395-400`),
  the nonce rule (`:428-432`), the doc comment (`:391-394`), and new unit tests in
  `mod tests` (`:489-`).
- `crates/foundry-oidc/Cargo.toml`: only if the optional HTTP-level test (below) is
  added. It needs `[dev-dependencies]` for `axum` (or `tokio` `TcpListener` with
  hand-rolled HTTP) and a fixture RSA key.
- No changes to `crates/foundry-app`, migrations, feature files or acceptance steps.

## Risk assessment

- **Real Keycloak: low risk, but unverified in this repo.**
  - foundry always sends a non-empty `nonce` (`lib.rs:344-350`) and an S256
    `code_challenge` (`:351`).
  - Under OIDC Core §3.1.2.1 and §3.1.3.7(11), the provider must echo the nonce in the
    ID token when the request carried one, and Keycloak does this for the code flow.
  - So no legitimate Keycloak sign-in is refused by the presence rule.
  - This repo holds no recorded real-Keycloak run (DDD-12 defers that to the cluster
    e2e). After the fix, do one manual sign-in against the cluster.
- **The 38 keycloak-sso and 15 provisioning scenario runs: no outcome changes.**
  - Every path to `exchange_code` starts at `/auth/oidc/start`, so state, nonce and
    verifier are non-empty, and the double echoes the recorded nonce
    (`oidc_issuer.rs:284-286`, `338-341`).
  - `StaleNonce` still refuses, with the same reason string.
  - "An arrival nobody started" is still refused at `oidc.rs:175`, before the
    exchange.
  - The refusal sweep in "Every refusal looks identical" is unaffected, because the
    response comes from the same `refuse()`.
- **Mutation:** the new conditions add mutants in `lib.rs`, such as dropping an
  `is_empty()` disjunct or flipping `||` to `&&`. The unit tests below must kill each
  one to meet the 80% per-feature gate.
- **Behaviour change for direct library callers:** there are none outside
  `foundry-app` (`git grep exchange_code`: only `oidc.rs:187`).

## Regression tests

1. **Unit tests in `crates/foundry-oidc/src/lib.rs` `mod tests` (required).** These
   are hermetic, have no network, and are fast.
   - `an_unissued_challenge_is_refused_before_the_exchange`: for each of empty
     `state`, empty `nonce` and empty `code_verifier`, the guard returns
     `Err(Untrusted)`.
     - If you test through `exchange_code`, point the provider at an unroutable issuer
       (as in `provider_with_provision_role`, `lib.rs:625-636`) and assert
       **`Untrusted`, not `Transport`**. Today the call reaches `discovery()` and
       returns `Transport`, so the test fails.
     - If you test the pure helper, the test fails to compile today. That counts as
       RED.
   - `an_identity_with_no_nonce_is_refused`: check `ensure_nonce` against a table:
     `("", "")` → Err, `("n", "")` → Err, `("", "n")` → Err, `("n", "m")` → Err,
     `("n", "n")` → Ok. The `("", "")` row is the defect. Today `"" != ""` passes, so
     the equivalent of the current code returns Ok.
   - Optionally, deserialise a payload with **no** `nonce` key into `IdTokenClaims`,
     using the existing `identity_from` / `keycloak_payload` pattern at `lib.rs:558-575`
     with the key removed. Assert that `ensure_nonce(&req.nonce, &claims.nonce)` refuses
     it. This pins the `serde(default)` interaction directly.
2. **Optional HTTP-level integration test, `crates/foundry-oidc/tests/exchange_code.rs`.**
   This is the strongest proof of the defect at the boundary where it lives.
   - Run a loopback issuer that serves discovery, JWKS and a token endpoint, minting an
     RS256 ID token that **omits** the nonce key. The acceptance double cannot omit the
     key (`oidc_issuer.rs:310`).
   - Call `exchange_code("c", &AuthRequest{ nonce: "", .. })`. Today it returns
     `Ok(identity)`. After the fix it returns `Err(Untrusted)`.
   - Cost: dev-dependencies and a fixture keypair. Weigh this against the unit tests,
     which already kill every new mutant.
3. **Acceptance (DISTILL-owned; flagged, not written).**
   - The defect cannot be reached end to end while the cookie guard is intact. A new
     scenario through the real round-trip would **pass today**, so it is not a
     regression test for this defect.
   - Its only value is killing a future mutant that skips the nonce compare when the
     token carries none. That needs an `oidc_issuer.rs` `Variant::NoNonce` that omits
     the claim, which in turn needs `Claims.nonce` to become `Option<String>` with
     `skip_serializing_if`.
   - Recommendation: no new Gherkin is needed for this fix. DISTILL may add an
     "identity answering no challenge is refused" example beside "stale challenge"
     (`keycloak-sso.feature:102`) if it wants that mutant covered at the acceptance
     level.
   - F10's scenario (`0c2553d`) continues to pin the cookie guard on its own.

## Resolution (2026-10-04)

The user approved fix direction **(a)+(b)+(c)** through `/nw-bugfix` Phase 2. The optional follow-ups
were not approved. DELIVER step 01-01 shipped it as `51b62cd`
(`fix(oidc): refuse an empty or absent nonce, state or verifier`):

- `AuthRequest::ensure_issued` is the first check in `exchange_code`.
- A pure `ensure_nonce` replaces the inline comparison.
- The doc comment is corrected.

Three regression unit tests were RED before the fix and pass now. Named faults N1–N4 are all killed.
The keycloak lanes pass: keycloak-sso 38/38 and provisioning 15/15. Record:
`docs/evolution/2026-10-04-fix-oidc-empty-nonce.md`.
