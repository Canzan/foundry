# Slice 6 — Provision a new tenant + close the two residuals

## Outcome
An operator creates a second workspace with a first admin, and the isolation boundary is PROVEN
with real two-workspace fixtures while the per-principal rate-bucket map stays bounded under many
tenants.

## Learning hypothesis
**We believe** an operator can create a second workspace + first admin (reusing the bootstrap
idiom) without touching existing tenants, and the boundary can be proven with REAL A/B fixtures
while the rate-bucket map evicts idle principals to stay bounded — **and we will know we are
right when** provisioning Globex leaves Acme untouched, the cross-tenant scenarios run on real
fixtures (no synthetic uuids), and the bucket map size is bounded by active principals.

## Riskiest assumption being validated
That provisioning a tenant is isolated from creation (creating B never touches A) and that the
residual closures (real fixtures + bucket eviction) do not weaken the proven boundary or the
shipped throttle correctness.

## Stories
- **US-MWT07** — operator creates a new workspace + seeds its first admin (authority per OD-3).
- **US-MWT08** — real two-workspace fixtures replace synthetic uuids; rate-bucket map eviction
  (closes residuals UI-1 + F2).

## IN scope
- A create-workspace path (operator/super-admin authority, OD-3) seeding a first admin; isolated
  from creation.
- Replace/augment synthetic-uuid cross-workspace fixtures with real Acme/Globex fixtures backing
  US-MWT02/03/05.
- Add an eviction policy (LRU/idle) to the per-principal rate-bucket map; preserve throttle
  correctness for active principals.

## OUT scope
- Self-serve workspace signup (OD-3 default = operator/super-admin only).
- Per-workspace backup/restore (OD-5).
- Workspace deletion/archival (out of feature scope).

## Reuses (shipped — do not rebuild)
- The bootstrap-token / invite idiom for seeding the first admin (single-workspace bootstrap is
  the precedent).
- `crates/foundry-app/src/rate_limit.rs` (per-principal token bucket keyed by `user_id`,
  100%-mutation-hardened) — add eviction, keep throttle semantics.

## Done when
- An authorized operator creates a new isolated workspace + first admin; creating it does not
  touch existing workspaces; a non-authorized actor is refused.
- The cross-tenant isolation scenarios use REAL two-workspace fixtures (UI-1 closed).
- The rate-bucket map evicts idle/stale principals so its size is bounded by active principals
  (F2 closed), with throttle correctness preserved + unit/property coverage.

## Learning hypothesis verdict shape
Confirms: tenants are operator-provisionable + the boundary is provable + resources bounded →
the feature is operationally complete.
Disproves: if provisioning leaks across tenants or the bucket map grows unbounded → fix before
release.

## Open questions touching this slice
- **OD-3** provisioning authority + instance super-admin role — flag for user ratification before
  DESIGN (introduces a new role concept).

## Dependencies
- Slice 1 (multiple workspaces possible) + Slices 2/3/5 (the scenarios the real fixtures back).

## Effort estimate
~1-1.5 days (provisioning reuses the bootstrap idiom; eviction is a bounded refactor of a
hardened module; real fixtures retroactively strengthen earlier slices).
