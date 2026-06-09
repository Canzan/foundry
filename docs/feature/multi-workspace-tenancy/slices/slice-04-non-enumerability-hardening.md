# Slice 4 — Cross-tenant non-enumerability hardening

## Outcome
Across EVERY surface, a request for another tenant's resource is observationally identical to a
request for a non-existent one — no status, body, timing, or shape oracle reveals that B's
resource exists.

## Learning hypothesis
**We believe** the cross-tenant refusal can be made UNIFORM across web reads/writes, admin
actions, `/api/v1` reads, and token revoke (generalizing the shipped `attachments.rs` idiom) so
no surface leaks existence — **and we will know we are right when** an adversarial matrix on every
surface shows foreign-id ≡ missing-id with no 403-vs-404 (or timing/shape) oracle.

## Riskiest assumption being validated
That NO surface has an existence oracle — that every "not yours" collapses to "doesn't exist"
identically. A single leaky surface (e.g. a 403 where a 404 is expected) defeats non-enumerability.

## Stories
- **US-MWT05** — uniform non-enumerable refusal across every surface; adversarial coverage; no
  existence oracle.

## IN scope
- Make foreign-resource and never-existed responses observationally identical on every surface.
- Adversarial acceptance matrix: web reads/writes, admin actions, `/api/v1` reads, token revoke.
- Real Acme/Globex fixtures.

## OUT scope
- Migration / provisioning / residual closure (Slices 5-6).
- The underlying scoping (Slices 2-3 already enforce it; this slice unifies the REFUSAL).

## Reuses (shipped — do not rebuild)
- The `attachments.rs find_attachment_for_requester` non-enumerable idiom as the canonical
  pattern to generalize.

## Done when
- On every surface, foreign-id ≡ missing-id (status + body shape; no timing/shape oracle).
- No 403-vs-404 cross-tenant existence oracle anywhere.
- The adversarial matrix passes against real A/B fixtures.

## Learning hypothesis verdict shape
Confirms: the boundary is non-enumerable everywhere → the security core is provably complete.
Disproves: if any surface leaks existence → close that oracle before the feature is acceptable.

## Open questions touching this slice
- Per-surface refusal status/shape (DESIGN) — DISCUSS fixes only that it is UNIFORM.

## Dependencies
- Slices 2-3 (the surfaces whose refusals it unifies).

## Effort estimate
~1 day (hardening + adversarial coverage; the scoping already exists from Slices 2-3).
