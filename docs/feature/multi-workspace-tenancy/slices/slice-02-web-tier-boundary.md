# Slice 2 — Tenant-scoped authz + non-enumerable refusal on the WEB htmx tier

## Outcome
On the web htmx tier, a member/admin of workspace A sees and manages only A; a reach for B's
resource by id is refused identically to a non-existent one — proven with real A/B fixtures.

## Learning hypothesis
**We believe** the shipped `workspace_id` scoping + `is_workspace_admin` + the attachments-style
non-enumerable lookup, driven by REAL Acme/Globex fixtures, refuse a member of A who reaches for
B's resource identically to a non-existent one across the web tier — **and we will know we are
right when** every web read/write path returns/affects only A, an A-admin cannot manage B, and a
crafted id to a B resource is indistinguishable from a non-existent id.

## Riskiest assumption being validated
That EVERY web read/write path is already scoped (no un-scoped query leaks B to A) and that
admin authority does not cross tenants. The web tier has the most read/write paths, so a scoping
gap is most likely to surface here.

## Stories
- **US-MWT02** — tenant-scoped reads/writes + per-tenant authz + non-enumerable refusal on the
  web htmx tier, with real A/B fixtures.

## IN scope
- Feed every web tenant-scoped read/write the resolved `${acting_workspace_id}`.
- Gate web admin actions with `is_workspace_admin(${acting_workspace_id}, …)`.
- Non-enumerable refusal for a foreign-workspace resource id on the web (generalize
  `find_attachment_for_requester`).
- Real Acme/Globex fixtures.

## OUT scope
- The `/api/v1` + machine-token + sign-in surfaces (Slice 3).
- Uniform non-enumerability across ALL surfaces + the full adversarial matrix (Slice 4).

## Reuses (shipped — do not rebuild)
- `is_workspace_admin`, `is_team_member`, the per-table `workspace_id` scoping, the
  `attachments.rs` non-enumerable lookup pattern.

## Done when
- A member of A sees/edits only A's web resources.
- A foreign-workspace resource id on the web is refused like a non-existent one.
- An A-admin cannot manage B's members/teams.
- All proven with real A/B fixtures.

## Learning hypothesis verdict shape
Confirms: the web tier's scoping + authz hold against a real second tenant → propagate to the API.
Disproves: if any web path leaks B to A → fix the scoping seam before widening surfaces.

## Open questions touching this slice
- Per-surface refusal status/shape (404 vs 403) — DESIGN; must be uniform (NFR-MWT-SEC-02).

## Dependencies
- Slice 1 (US-MWT00/01: resolution + coexistence).

## Effort estimate
~1 day (mostly proving + adversarial coverage over shipped scoped queries).
