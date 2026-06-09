# Slice 5 — Migrate an existing single-workspace install to workspace 1

## Outcome
A real pre-feature single-workspace install upgrades forward-only with ZERO data loss and ZERO
change to how its users sign in and work; the existing workspace becomes workspace 1.

## Learning hypothesis
**We believe** a real pre-feature single-workspace DB can upgrade forward-only (dropping
`uniq_one_workspace` + adding resolution) with zero data loss and zero change to existing
sessions/tokens/sign-in — **and we will know we are right when** a real pre-feature DB snapshot
upgrades, every tenant row is present and unchanged, the workspace id is unchanged, and the
existing auth suites stay green with live sessions/tokens still resolving to workspace 1.

## Riskiest assumption being validated
That the migration touches NO existing data — that dropping a unique index + adding resolution
does not rewrite, move, or cross-wire any row, and that existing sessions/tokens keep resolving.
This is the single highest-stakes data-safety step; every existing install passes through it.

## Stories
- **US-MWT06** — existing workspace becomes workspace 1; forward-only; no data loss;
  sessions/tokens keep working; run against a REAL pre-feature DB snapshot.

## IN scope
- Apply the forward-only migration to a real pre-feature DB.
- Assert row-level before/after equality across all tenant tables; workspace id unchanged.
- Assert existing sessions + machine tokens still resolve to workspace 1.
- Assert existing auth + workspace acceptance suites stay green post-migration.

## OUT scope
- Provisioning a NEW workspace (Slice 6).
- Per-workspace backup/restore (OD-5, out of feature scope).

## Reuses (shipped — do not rebuild)
- The forward-only migration discipline (ADR-003) used by the shipped features (no-rewrite,
  no-loss); the existing workspace's FKs already point at it.

## Done when
- The migration is forward-only and edits no prior migration.
- Before/after row equality across all tenant tables; workspace id unchanged.
- Existing users sign in as before; existing sessions/tokens resolve to workspace 1.
- The migration acceptance runs against a real pre-feature DB snapshot.
- Existing auth suites green post-migration.

## Learning hypothesis verdict shape
Confirms: the upgrade is seamless and lossless → safe to ship to existing installs.
Disproves: if the migration touches data or breaks sessions/tokens → redesign the migration
before release.

## Open questions touching this slice
- **OD-4** existing-install migration — confirmed-by-default (forward-only, no data touch); user
  confirms acceptable.

## Dependencies
- Slice 1 (US-MWT00 migration) + Slice 3 (US-MWT04 session resolution defaulting the single
  workspace).

## Effort estimate
~1 day (the migration is small; the rigor is in the real-DB before/after + auth-suite proof).
