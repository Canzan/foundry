# Non-Functional Requirements — Multi-Workspace Tenancy

> SECURITY-heavy by nature: this feature lets multiple tenants share one instance, so every NFR
> is about ISOLATION, NON-ENUMERABILITY, MIGRATION SAFETY, or BOUNDED per-tenant resources.
> Every NFR is testable. IDs are referenced from `stories.md` and `wave-decisions.md`.
> Solution-neutral: these fix the constraints + observable properties; DESIGN picks mechanisms.

## Security — Tenant Isolation (HARD)

### NFR-MWT-SEC-01 — Every tenant-scoped read and write is confined to the acting workspace
No read or write returns or affects data outside `${acting_workspace_id}` (the workspace
resolved for the request). Generalizes the per-table `workspace_id` scoping already shipped.
- **Measurable**: with real workspaces A and B coexisting, 0 of A's queries return any B row;
  0 of A's writes affect any B row, on every surface (web htmx tier, JSON `/api/v1`).
- **Verify**: real two-workspace (A/B) acceptance fixtures (US-MWT08) covering list/read/write
  on each surface; a query-audit (DESIGN/DELIVER guard) that every tenant-scoped query binds an
  `${acting_workspace_id}`.

### NFR-MWT-SEC-02 — Cross-tenant refusal is NON-ENUMERABLE (no existence oracle)
A request for a resource outside the acting workspace is refused IDENTICALLY to a request for a
resource that does not exist — no status, body, timing, or error-shape difference reveals that
the foreign resource exists. Generalizes `attachments.rs find_attachment_for_requester`.
- **Measurable**: for any foreign resource id, the response is byte-equivalent (status + body
  shape) to the response for a never-existed id; no `403`-vs-`404` oracle, no field that leaks
  existence.
- **Verify**: adversarial acceptance scenarios (US-MWT05) on every surface asserting
  foreign-id ≡ missing-id; an API-contract check on the refusal envelope.

### NFR-MWT-SEC-03 — Isolation is FAIL-CLOSED
If `${acting_workspace_id}` cannot be resolved (no workspace, or an ambiguous/absent
membership), the request is REFUSED — never defaulted to "the first" or "any" workspace.
- **Measurable**: a request with no resolvable workspace yields a refusal, not data from any
  workspace; 0 requests are served against an un-resolved or guessed workspace.
- **Verify**: acceptance scenarios (US-MWT04) for the no-resolvable-workspace and
  ambiguous-membership paths.

### NFR-MWT-SEC-04 — Per-tenant authority does not cross tenants
`is_workspace_admin(workspace, user)` and `is_team_member(team, user)` are evaluated against
`${acting_workspace_id}`; an admin of A has NO admin authority in B; a member of A is not a
member of B's teams.
- **Measurable**: 100% of A-admin attempts against B are refused; 0 cross-tenant privilege
  leaks.
- **Verify**: adversarial scenarios (US-MWT02/05) — admin-of-A manages B's members/tokens →
  refused non-enumerably.

### NFR-MWT-SEC-05 — Machine-token workspace binding is enforced as the acting workspace
A `/api/v1` request authenticated by a machine token acts ONLY on the token's bound
`${token.workspace_id}`; a token bound to A cannot read or mutate B.
- **Measurable**: a token bound to A used against a B resource is refused non-enumerably; 0
  cross-tenant bearer calls succeed.
- **Verify**: acceptance scenarios (US-MWT03) with a real A-bound token against B resources.

### NFR-MWT-SEC-06 — Resolution happens at a single, auditable seam
The acting workspace is resolved at ONE place per surface (not re-derived ad hoc in each
handler), so the boundary is auditable and a missing scope is structurally hard.
- **Verify**: DESIGN documents the resolution seam; a structural/`check-arch`-style guard
  (DESIGN/DELIVER) that handlers consume the resolved `${acting_workspace_id}` rather than
  trusting client-supplied workspace ids.

## Data Integrity — Migration Safety (HARD)

### NFR-MWT-DATA-01 — Forward-only migration, no data rewrite, no data loss
The migration that enables multi-workspace is forward-only (ADR-003): it DROPS
`uniq_one_workspace` and adds resolution support, and does NOT rewrite, move, or delete any
existing row. The single pre-existing workspace and all its data remain intact as workspace 1.
- **Measurable**: before/after row counts and checksums for `workspaces`, `users`,
  `workspace_memberships`, `teams`, `projects`, `issues`, `invites` are identical; the existing
  workspace's id is unchanged.
- **Verify**: a migration acceptance test (US-MWT06) on a real pre-feature DB snapshot asserting
  row-level equality before/after; migration review confirms no prior migration is edited.

### NFR-MWT-DATA-02 — Existing sessions, tokens, and sign-in keep working across the upgrade
After the upgrade, the existing workspace's users sign in exactly as before; their live sessions
and machine tokens continue to resolve to workspace 1.
- **Measurable**: existing browser-auth + machine-token acceptance scenarios stay green
  post-migration; a pre-upgrade session/token resolves to workspace 1 after upgrade.
- **Verify**: the `foundry-acceptance` auth suites run against the post-migration schema and
  stay green; US-MWT06 asserts a carried-over session/token still works.

### NFR-MWT-DATA-03 — The `uniq_one_workspace` guard is not depended upon by any query
No query relies on "there is exactly one workspace" for correctness; every tenant-scoped query
filters by an explicit `workspace_id`.
- **Verify**: a code-audit (DESIGN/DELIVER) for un-scoped `FROM teams|projects|issues|invites`
  reads before the guard is dropped (assumption #6 in `wave-decisions.md`).

## Reliability / Correctness

### NFR-MWT-REL-01 — Provisioning a new workspace yields a fully isolated tenant
A newly provisioned workspace (US-MWT07) is immediately reachable, isolated from all others
from creation, and seeded with its first admin; creating B never touches A.
- **Verify**: acceptance scenario creating B then asserting A is unaffected and B is
  isolated + reachable.

### NFR-MWT-REL-02 — Existing green acceptance stays green (no regression)
The full `foundry-acceptance` suite that is green before this feature stays green after — the
single-workspace behaviors are a special case of multi-workspace (one workspace).
- **Verify**: `@all` acceptance suite green before and after.

## Performance / Resource Bounding

### NFR-MWT-PERF-01 — Per-tenant in-memory resources are bounded under many tenants
The per-principal revoke-storm rate-bucket map (`crates/foundry-app/src/rate_limit.rs`, residual
F2) MUST NOT grow unbounded as tenant/admin count grows; it evicts idle/stale principals
(LRU/idle policy).
- **Measurable**: under a workload spanning many workspaces/principals, the bucket map size is
  bounded by a cap (or idle-eviction window), not by total historical principals; the rate
  guardrail's behavior is unchanged for active principals.
- **Verify**: a unit/property test (mirrors the shipped 100%-mutation `rate_limit` tests)
  asserting eviction bounds the map while preserving throttle correctness for active principals
  (US-MWT08).

### NFR-MWT-PERF-02 — Workspace resolution adds no material per-request cost
Resolving `${acting_workspace_id}` is cheap (a session/claim read or an indexed membership
lookup), adding no material latency vs the single-workspace path.
- **Measurable**: request latency p95 unchanged within the existing web-tier budget
  (consistent with NFR-PERF-04 / the shipped ≤200 ms server-side targets).
- **Verify**: timing assertions consistent with the existing web-tier benchmarks.

## Testability — Real Fixtures (unblocks the residuals)

### NFR-MWT-TEST-01 — Cross-tenant isolation is proven with REAL two-workspace fixtures
The cross-workspace evil-user paths are exercised with two genuinely coexisting workspaces (A
and B), NOT synthetic uuids — closing the accepted residual (UI-1 / docs/evolution 2026-06-07 +
2026-06-08).
- **Measurable**: the isolation acceptance scenarios construct real A and B (real members, real
  tokens, real issues/projects) and assert the boundary across them.
- **Verify**: US-MWT08 acceptance fixtures; the synthetic-uuid cross-workspace tests are
  replaced or augmented with real-fixture equivalents.

## Invariants (carried, must not regress)

- ONE binary, ONE Postgres, no Redis, no Node runtime service, no CDN.
- The SHIPPED machine-token verify path + per-request `jti` denylist + `iss`/`aud`/EdDSA pinning
  are unchanged; this feature scopes WHICH workspace a principal acts on, not how a token is
  verified.
- The browser auth/CSRF/session contract (double-submit `foundry_csrf`, tower-sessions Postgres
  store, 30-day cookie, argon2id, brute-force delay, non-enumerable sign-in error) is preserved;
  multi-workspace adds workspace RESOLUTION on top, it does not weaken auth.
- The per-table `workspace_id` scoping, the `attachments.rs` non-enumerable lookup, and the
  `is_workspace_admin`/`is_team_member` checks are REUSED, not reimplemented.
- The `foundry-acceptance` suite green before this feature stays green after.
