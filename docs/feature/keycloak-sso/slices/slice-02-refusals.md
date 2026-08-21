# Slice 02 — Refusals are safe and silent

## Goal
Every way the OIDC path can fail refuses identically, and never creates an account.

## Learning hypothesis
**Disproves if it fails**: that the new sign-in path can be made
enumeration-silent — that `GENERIC_SIGNIN_ERROR` plus the shipped timing-symmetry
posture generalises from the password flow to a federated one. If a refusal branch
is distinguishable, `us-06-timing-symmetry-redesign`'s oracle was password-specific
and the enumeration work must be redone at the sign-in surface, not per-path.
**Confirms if it succeeds**: the callback is safe to publish through the tunnel, and
enabling SSO cannot leak who has a foundry account.

## IN scope
- Unknown email → refused, no user/membership/session created.
- `email_verified` false or absent → refused, even when the email matches.
- User exists but belongs to no workspace → the existing fail-closed branch.
- Missing OIDC cookie, `state` mismatch, `nonce` mismatch → refused.
- ID token failing signature / `iss` / `aud` / `exp` → refused.
- Replayed callback → refused (cookie is single-use).
- Token endpoint unreachable or slow → refused and logged, never a 500.
- All of the above return the same status and body as a bad password (D7).

## OUT of scope
- Rate limiting or lockout on the OIDC path (foundry has none on `/sign-in` either;
  adding it here would be a new commitment, not a refusal).
- Cluster wiring — slice 03.

## Acceptance criteria
AC-2.1 … AC-2.5 and AC-3.1 … AC-3.7 (`feature-delta.md`, US-02 and US-03), plus the
US-04 regressions AC-4.1, AC-4.2, AC-4.4 asserting the password path and its
timing-symmetry oracle stay green alongside the new path.

## Dependencies
Slice 01.

## Effort
~0.5 day. Reference class: `bootstrap-claim-enumeration-oracle` — the shipped
feature that established how this repo asserts non-enumerability, and whose oracle
this slice should reuse rather than reinvent.

## Taste-test note
Ships no new component — only branches on slice 01's handler plus scenarios. Passes
the thinness tests. Its value is user-visible per the elevator pitches in US-02/US-03
(a valid Keycloak login that creates nothing; a hand-crafted callback that is
refused), so it is not an `@infrastructure` slice.

## Dogfood moment
Same day: `curl` a hand-crafted callback against the running instance and watch it
refuse with the same page a wrong password produces.
