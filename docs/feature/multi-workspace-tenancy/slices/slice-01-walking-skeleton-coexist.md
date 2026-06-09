# Slice 1 (Walking Skeleton) — Two workspaces coexist + request→workspace resolution

## Outcome
Drop `uniq_one_workspace` and add a request→workspace resolution seam so two real workspaces
coexist in one instance and a single read path returns ONLY the acting workspace's data.

## Learning hypothesis
**We believe** two real workspaces can coexist in one Foundry instance and a request can resolve
to EXACTLY its workspace end-to-end on one read path (over the per-table `workspace_id` scoping
that already ships) — **and we will know we are right when** a member of Acme lists issues and
sees only Acme's, a member of Globex sees only Globex's, with real coexisting data, and a request
that resolves to no workspace is refused, not defaulted.

## Riskiest assumption being validated
That dropping the single-workspace guard + adding resolution keeps two workspaces' data APART on
even one path — i.e. the shipped `workspace_id` scoping is genuinely load-bearing once a second
tenant exists. This is the one load-bearing abstraction every later slice depends on, so it ships
FIRST.

## Stories
- **US-MWT00** (`@infrastructure`, folded) — forward-only migration dropping `uniq_one_workspace`
  (`0001_init.sql:15`) + the resolution seam (mechanism is DESIGN, DM1).
- **US-MWT01** — two workspaces coexist; a member sees only their own workspace's data on one
  read path, proven with real A/B fixtures.

## IN scope
- The forward-only migration dropping the guard.
- The resolution seam yielding exactly one `${acting_workspace_id}`, fail-closed.
- ONE proven read path (web board or `GET /api/v1/issues`) returning only the acting workspace's
  rows, with real Acme/Globex fixtures.

## OUT scope
- Generalizing to every web/API surface (Slices 2-3).
- Non-enumerable cross-tenant refusal hardening (Slice 4).
- Migration of an existing install as a user-visible guarantee (Slice 5).
- Provisioning a new workspace via a product action (Slice 6).

## Reuses (shipped — do not rebuild)
- The per-table `workspace_id` scoping in `foundry-store` (every tenant query already takes it).
- The session/bearer auth that establishes the acting user.

## Done when
- A second `workspaces` row can be created (guard gone).
- A request resolves to exactly one workspace and is fail-closed if none resolves.
- A member of Acme sees only Acme's issues; a member of Globex sees only Globex's — real data.
- The migration rewrites no existing data.

## Learning hypothesis verdict shape
Confirms: the shipped scoping holds with two real tenants on one path → safe to propagate.
Disproves: if two tenants' data bleeds on even one path → reframe the isolation seam before any
surface work.

## Open questions touching this slice
- **OD-1** tenant model (default shared-schema-with-`workspace_id`) — DESIGN ratifies; this slice
  assumes it.
- **OD-2** user↔workspace cardinality — shapes the resolution mechanism; flag for user before
  DESIGN.

## Effort estimate
~1 day (the migration is a one-line index drop; the resolution seam + one proven path is the work).
