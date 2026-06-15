# ADR-004 — Password-strength policy: min-12 length-first, reusable home

## Status
**IMPLEMENTED / SHIPPED** (finalized 2026-06-14). DESIGN wave. Resolves NFR-4 placement; OD-2 RATIFIED at min-12 (length-first, NIST). `foundry_auth::check_password_policy` shipped + 100% (4/4) scoped mutation coverage; see `docs/evolution/2026-06-14-invite-accept-flow.md`.

## Context
NFR-4 introduces a minimum password-strength policy at set-password. Grounding confirms **foundry
enforces NO minimum today**: `bootstrap.rs:149-157` hashes whatever is submitted; `signin.rs` has no
length check. So there is no existing strength policy to reuse — only the argon2id **hashing**
(`foundry_auth::hash_password`, `lib.rs:319`) is shipped and reused verbatim. This is a net-new,
app-wide-relevant policy. The DISCUSS artifacts ask DESIGN to place it so a future app-wide rollout
(sign-up, reset, bootstrap claim) can reuse the SAME check.

## Options considered
- **(a) A small pure `check_password_policy(pwd) -> Result<(), PolicyError>` in `foundry-auth`, beside
  `hash_password` (RECOMMENDED).** The accept handler is its first caller; bootstrap/signin can import
  it later WITHOUT moving it. Policy lives next to the hashing primitive — the obvious shared home.
- **(b) Inline the length check in the accept handler (`invites_accept.rs`).** REJECTED — buries an
  app-wide policy in one adapter; a future rollout would have to extract it (and risk divergence).
- **(c) A new `foundry-policy` crate.** REJECTED — over-engineered for one length check; violates the
  ZERO-new-crate constraint; `foundry-auth` already owns credential concerns.

## Decision
**(a)** — add `foundry_auth::check_password_policy(pwd: &SecretString) -> Result<(), PolicyError>`,
**min 12 characters, length-first, no composition rule** (aligned with NIST 800-63B: favor length over
arbitrary complexity). It is pure (no I/O), unit-testable, and the single source of the policy. The
accept POST calls it BEFORE opening the consume TX; a violation re-renders the form inline with a clear
message and the invite UNTOUCHED (FR-5, US-03, AC-03.1/03.3/03.4). `hash_password` is reused verbatim,
called only after the policy passes.

**OD-2 (threshold)**: proposed **12**. Net-new (no shipped baseline to match), so it is flagged for
explicit user ratification before DELIVER. The placement decision (a) stands regardless of the number;
changing the threshold is a one-constant edit in the shared fn.

## Consequences
- **Positive**: a single reusable policy home; a future app-wide rollout imports one fn; pure +
  unit-testable; the boundary (count of characters) is observable and matches the AC at exactly 12.
- **Negative**: introduces app-wide-relevant behavior in a feature that only needs it at one call site
  (acceptable — the point is reusability; the cost is one tiny fn).
- **Security**: length-first resists modern guessing better than short-but-complex; argon2id hashing
  (OWASP params) is unchanged; a rejected password never consumes the invite.

## Relationship
Reuses `hash_password` (`foundry-auth/lib.rs:319`) verbatim; co-locates the new policy beside it.
Adopters (bootstrap claim, sign-up, reset) are OUT of this feature's scope but can import it unchanged.
