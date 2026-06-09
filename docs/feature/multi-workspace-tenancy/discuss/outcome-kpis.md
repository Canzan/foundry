# Outcome KPIs — Multi-Workspace Tenancy

> Each KPI is [Who] [does what] [by how much] [measured by] [baseline]. Outcome (behavior /
> security change) over output (code shipped). Tenancy is security-critical, so several KPIs are
> ZERO-tolerance isolation invariants, verified by adversarial acceptance + a code audit rather
> than by usage telemetry.

## Epic-level outcomes

### KPI-MWT-E1 — Tenants coexist on one instance (mwt-job-1)
- **Who**: instance operators hosting multiple teams.
- **Does what**: run two or more isolated workspaces in ONE process / ONE Postgres.
- **By how much**: from a hard cap of 1 workspace per instance (today) to N coexisting,
  isolated workspaces.
- **Measured by**: an acceptance scenario creating ≥2 workspaces and operating both
  independently in one instance.
- **Baseline**: 1 (the `uniq_one_workspace` cap).

### KPI-MWT-E2 — Zero cross-tenant data exposure (mwt-job-2) — HARD invariant
- **Who**: every workspace member, admin, and machine-token principal.
- **Does what**: read/write ONLY their own workspace's data; never read/mutate/enumerate
  another's.
- **By how much**: 0 cross-tenant reads, 0 cross-tenant writes, 0 existence-leak oracles across
  ALL surfaces, with real A/B fixtures.
- **Measured by**: adversarial acceptance scenarios (US-MWT05) + a query audit asserting every
  tenant-scoped query binds an acting `workspace_id`.
- **Baseline**: enforced today only by `uniq_one_workspace` (one tenant) + synthetic-uuid tests;
  unproven against a real second tenant.

## Story-level outcomes

### KPI-MWT-01 (US-MWT01) — Request resolves to its own workspace
- **Who**: a signed-in user / request on a multi-workspace instance.
- **Does what**: act on exactly the resolved acting workspace's data.
- **By how much**: 100% of requests resolve to exactly one workspace; 0 requests served against
  an unresolved/ambiguous workspace (fail-closed).
- **Measured by**: Slice-1 walking-skeleton acceptance with real A/B data on one read path.
- **Baseline**: N/A (only one workspace can exist today).

### KPI-MWT-02 (US-MWT02) — Web htmx tier isolation
- **Who**: a member/admin of workspace A on the web tier.
- **Does what**: see/manage only A; a reach for B is refused non-enumerably.
- **By how much**: 0 B-rows in any of A's web reads; 0 A-writes affecting B; A-admin authority
  in B = refused 100%.
- **Measured by**: US-MWT02 acceptance with real A/B fixtures (list/read/write + admin actions).
- **Baseline**: 0 real-fixture coverage today.

### KPI-MWT-03 (US-MWT03) — API + machine-token isolation
- **Who**: a `/api/v1` caller / machine-token principal bound to A.
- **Does what**: act only on A; a cross-tenant call to B is refused non-enumerably.
- **By how much**: 0 cross-tenant bearer calls succeed; foreign-id ≡ missing-id response.
- **Measured by**: US-MWT03 acceptance with a real A-bound token against B resources.
- **Baseline**: cross-workspace bearer paths tested with synthetic uuids only.

### KPI-MWT-04 (US-MWT04) — Resolution is unambiguous and fail-closed
- **Who**: every signed-in session (incl. multi-membership users).
- **Does what**: resolve to exactly one acting workspace; refuse if none resolvable.
- **By how much**: 100% of sessions resolve to exactly one workspace; 0 served against an
  unresolved workspace.
- **Measured by**: US-MWT04 acceptance for the no-resolvable + ambiguous-membership paths.
- **Baseline**: N/A (implicit single workspace).

### KPI-MWT-05 (US-MWT05) — Non-enumerability across every surface
- **Who**: any cross-tenant actor (accidental or hostile).
- **Does what**: learn NOTHING about another tenant's existence or resources.
- **By how much**: 0 surfaces expose a 403-vs-404 (or timing/shape) existence oracle.
- **Measured by**: adversarial US-MWT05 scenarios on each surface (foreign-id ≡ missing-id).
- **Baseline**: pattern shipped for attachments only; not generalized/proven.

### KPI-MWT-06 (US-MWT06) — Migration safety
- **Who**: every existing single-workspace install.
- **Does what**: upgrade to multi-workspace with the existing workspace intact as workspace 1.
- **By how much**: 0 rows lost/changed across all tenant tables (before/after checksums equal);
  existing auth suites 100% green post-migration.
- **Measured by**: US-MWT06 migration acceptance on a real pre-feature DB snapshot.
- **Baseline**: no migration path exists today.

### KPI-MWT-07 (US-MWT07) — Tenant provisioning
- **Who**: the instance operator / instance super-admin.
- **Does what**: create a new workspace + seed its first admin, without a redeploy or manual DB
  edit.
- **By how much**: time-to-new-tenant under a few minutes; the new workspace is isolated from
  creation; creating B does not touch A.
- **Measured by**: US-MWT07 acceptance creating B then asserting A unaffected + B reachable.
- **Baseline**: 0 (only the bootstrap-created first workspace exists).

### KPI-MWT-08 (US-MWT08) — Residuals closed
- **Who**: security reviewers + the test suite.
- **Does what**: prove isolation with REAL two-workspace fixtures; keep the per-principal
  rate-bucket map bounded under many tenants.
- **By how much**: synthetic-uuid cross-workspace tests replaced/augmented with real A/B
  fixtures; rate-bucket map size bounded (cap or idle-eviction), not unbounded in principal
  count.
- **Measured by**: US-MWT08 real-fixture acceptance + a `rate_limit` eviction unit/property test
  (mirrors the shipped 100%-mutation tests).
- **Baseline**: synthetic-uuid tests (UI-1); unbounded map under multi-workspace (residual F2).
