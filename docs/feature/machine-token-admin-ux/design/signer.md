# Signing-Key Posture — `MachineTokenSigner` in AppState

DESIGN (Propose) for the security-posture change ratified as Q1/DM1: the running process becomes a
token ISSUER. This document specifies WHERE the signer lives, HOW it is loaded, the HARDENING, the
signer-absent behaviour, and the Earned-Trust boot probe. Decisions: ADR-MT01 (`wave-decisions.md`).

## Where the signer lives

`AppState` gains ONE field, mirroring the existing verifier (`foundry-app/src/lib.rs:70`):

```
// foundry-app/src/lib.rs — AppState (NEW field, beside machine_token_verifier)
pub machine_token_signer: Option<Arc<foundry_auth::MachineTokenSigner>>,
```

- `Some(signer)` ⇒ this binary is an **issuer**; the mint surface is offered.
- `None` ⇒ this binary is **verifier-only**; the mint surface is not offered (graceful, DD2).

The verifier stays exactly as today (`Arc<MachineTokenVerifier>`, always present — every binary
verifies). The signer is the optional, NEW issuer capability.

`MachineTokenSigner` does NOT implement `FromRef` from `AppState` (unlike the verifier, which
foundry-api extracts). The signer is read directly by the `admin_tokens` handler from
`State<AppState>` and passed to `services.mint_token(signer, …)` (DD4) — it is confined to the
mint call path and never leaves `foundry-app`/`foundry-services`.

## How it is loaded (mirror the verifier)

In `main.rs`, the signing key is ALREADY read for the boot self-test
(`foundry-app/src/main.rs:199-216`). DESIGN EXTENDS that exact block to RETAIN the signer on
success instead of dropping it:

```
// foundry-app/src/main.rs — EXTEND the existing self-test block (~199)
let machine_token_signer: Option<Arc<MachineTokenSigner>> =
    match std::env::var("MACHINE_TOKEN_SIGNING_KEY") {
        Ok(raw) => {
            // SHIPPED \n-normalization, identical to the public-key path (main.rs:171).
            let pem = SecretString::new(raw.replace("\\n", "\n").into());
            let signer = MachineTokenSigner::from_pkcs8_pem(&pem)
                .map_err(/* health.startup.refused {reason:'machine_token_key'} — as today */)?;
            // Earned-Trust gate (UNCHANGED): keypair must round-trip in THIS env.
            machine_token_verifier.self_test(&signer)
                .map_err(/* health.startup.refused — as today, main.rs:205-215 */)?;
            Some(Arc::new(signer))   // <-- the ONLY new line: retain after the probe passes
        }
        Err(_) => None,             // verifier-only binary — graceful
    };
// ... then place `machine_token_signer` into AppState alongside machine_token_verifier.
```

Key points:
- **Reuses `from_pkcs8_pem`** (`foundry-auth:113`) and the **SHIPPED `\n`-normalization** the public
  keys already use (`main.rs:171`) — env transport (`.env`/compose/subprocess) encodes PEM newlines
  as literal `\n`.
- **Reuses the existing self-test** (`MachineTokenVerifier::self_test`, `foundry-auth:201`) as the
  wire-then-probe-then-use gate. No new probe is invented.
- The signing key string is wrapped in a `SecretString` as early as practical. Two transient,
  non-zeroizing heap allocations of the key bytes precede that wrap and persist until the allocator
  reclaims them: (1) the plain `String` returned by `env::var`, bound to `raw`, which lives for the
  whole match arm; and (2) the intermediate `String` produced by `raw.replace("\\n", "\n")` for the
  `\n`-normalization. Neither is a `SecretString`, so neither zeroizes on drop. This is an ACCEPTED
  residual (alongside the parsed-key residual below): the key bytes already arrive in plaintext via
  the process environment, so these short-lived copies do not widen the exposure beyond what the
  environment already grants. We describe it honestly rather than claim a single-expression consume.

## Hardening (NFR-MT-SEC-01/04, the risk register)

- **`SecretString` end-to-end.** The key PEM is wrapped in `SecretString` before parsing
  (`from_pkcs8_pem` takes `&SecretString`); `MachineTokenSigner` holds only the parsed
  `EncodingKey` (no `Debug`/`Display` that prints key bytes). The minted token is a `SecretString`
  (`mint` returns it). **Never** log the key or the minted token (error-and-observability.md
  §"no-secret rule").
- **No `Debug` leak.** `AppState`'s `Debug` impl is already `finish_non_exhaustive()` and lists only
  non-secret fields (`lib.rs:168-175`); the signer field is NOT added to it — so a `{:?}` of
  `AppState` can never print the signer.
- **Zeroize where practical.** `EncodingKey` (jsonwebtoken) does not expose a zeroizing drop; the
  `SecretString` wrapping the PEM zeroizes on drop (secrecy crate). The realistic residual is the
  parsed key in `EncodingKey` for the process lifetime — the ACCEPTED, ratified posture for a
  self-hosted issuer. We document it rather than pretend it away (Earned Trust: honest about the
  faith we are asking the operator to make).
- **The minted value's lifetime is bounded to ONE render.** `mint_token` returns the `SecretString`
  to the handler, which exposes it exactly once into `TokenMintedPage`'s field and drops it after
  `render()` (DD7). No `Arc`, no cache, no session storage.
- **Crash-dump / in-process-attacker delta.** An attacker who reaches the issuer process can mint
  (the ratified, accepted risk, Q1). Mitigation is operational (self-hosted boundary) + the audit
  trail (`created_by`, `last_used_at`) that makes abuse attributable after the fact. Verifier-only
  replicas carry NO signing key, so the key-exposure surface is exactly the issuer binaries.

## Behaviour when `MACHINE_TOKEN_SIGNING_KEY` is ABSENT (OD1 — RECOMMENDED: graceful)

`machine_token_signer == None`. The binary boots normally (no refuse-to-start). The admin surface
detects the absence and degrades gracefully (US-MT00 scenario 2, US-MT01 scenario 3):

- `GET /admin/tokens`: the list still renders (verifier-only binaries can still SEE issued tokens),
  but the **mint form is replaced by an "Issuing tokens is not enabled on this server" notice**.
  No mint button, no partial form.
- `POST /admin/tokens`: returns a clean 403-style "issuing not enabled" page/fragment — **never a
  500, never a partial token** (the handler checks `state.machine_token_signer.is_none()` BEFORE
  building any claims).
- This is a per-handler check on the `Option`, NOT a route-mount decision — the route exists on
  every binary; only the issuer capability differs. (Rejected alternative: conditionally mounting
  the mint route, which would make verifier-only return 404 and leak the configuration shape.)

Rejected: hard-require the key at boot (breaks the ratified verifier-only deployment); a separate
issuer-mode flag (the key's presence already IS the mode). See ADR-MT01.

## Earned-Trust posture (principle 12)

The signer is a dependency on external key material in a specific runtime environment. The
invariant **wire → probe → use** is satisfied by the EXISTING boot self-test:

1. **Wire**: `from_pkcs8_pem` parses the configured private key.
2. **Probe**: `MachineTokenVerifier::self_test(&signer)` (foundry-auth:201) signs a throwaway claim
   set and verifies it under THIS binary's configured public keys — proving the keypair round-trips
   *in this environment* (catches the classic "signing key and public key don't correspond" lie that
   would otherwise 401 every minted token in production).
3. **Use**: the signer is retained in `AppState` ONLY if the probe passed; otherwise the binary
   emits `health.startup.refused {probe:'machine_token', reason:'machine_token_key'}` and exits
   (the SHIPPED behaviour, main.rs:205-215 — unchanged).

The probe contract is enforced by the boot path itself (an issuer cannot serve a mint surface
without a signer that survived `self_test`). No new probe code is required for this feature; DD1's
single added line (`Some(Arc::new(signer))`) sits AFTER the probe, so the type "we have a usable
signer" is only constructible post-probe.
