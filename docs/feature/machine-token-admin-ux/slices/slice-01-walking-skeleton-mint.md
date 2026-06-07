# Slice 1 (Walking Skeleton) — Mint ONE token end-to-end, value shown once

## Outcome
A workspace admin issues a single machine token from the product and sees its value EXACTLY
ONCE; the token authenticates against `/api/v1`; only `jti` + metadata persist.

## Learning hypothesis
**We believe** an admin can be given a live, working bearer token from an in-product action
(over the shipped `MachineTokenSigner::mint`) **and** the one-time secret can be surfaced
safely — **and we will know we are right when** an admin mints a token in under 2 minutes,
uses it successfully, and no token value is ever persisted or logged.

## Riskiest assumption being validated
The **signing-key-in-AppState** posture (Q1/DM1): putting the Ed25519 private key live in the
running process so the product can issue. This is the one genuinely new, genuinely risky
thing — everything downstream is registry reads/flags over shipped store functions.

## Stories
- **US-MT00** (`@infrastructure`, folded) — `MachineTokenSigner` in `AppState` + forward-only
  `created_by` migration + `insert_machine_token(created_by)`.
- **US-MT01** — admin mints; value shown once; metadata persists; token works against the API.

## Reuses (shipped — do not rebuild)
- `MachineTokenSigner::mint(&MachineTokenClaims) -> SecretString` (foundry-auth).
- `insert_machine_token(...)` (foundry-store) — gains a `created_by` param.
- `is_workspace_admin(...)` for the admin gate (foundry-store).
- `MachineTokenVerifier` + per-request `jti` denylist (foundry-api) — makes "use it" already true.

## Done when
- An issuer-configured server lets an admin mint a token, shows the value once with a copy
  affordance + "only time you'll see this" warning, and shows `jti`/label/scope/expiry.
- The minted token authenticates against `/api/v1`.
- The value is absent from every later screen/endpoint, the DB, and logs.
- A verifier-only server reports "issuing not enabled" (no 500, no partial token).
- `created_by` is recorded on the new row.

## Key risks / guardrails
- Token-value leak (logs/error/half-page) → NFR-MT-SEC-01/02, NFR-MT-REL-01.
- Signing-key exposure → NFR-MT-SEC-04; **user must accept the posture (Q1) before BUILD**.

## Open questions touching this slice
- **Q1** signing-key posture (BLOCKING for BUILD, not for DISCUSS handoff).
- **Q6** surface (web screen vs `201` body) — slice is surface-neutral; DESIGN picks.
