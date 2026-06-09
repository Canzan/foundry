# Multi-Workspace Tenancy — DISCUSS Wave Decisions

> This is the file DESIGN reads FIRST. This feature makes Foundry's tenancy REAL: multiple
> workspaces coexist in one instance with genuine per-tenant data isolation across every
> surface (web htmx tier + JSON `/api/v1`). Foundry is SINGLE-workspace today, enforced by
> `CREATE UNIQUE INDEX uniq_one_workspace ON workspaces ((true))`
> (`crates/foundry-store/migrations/0001_init.sql:15`). The security-critical core is **tenant
> data isolation**: a member/admin/machine-token of workspace A must never read or mutate
> workspace B's data, and the system must be **NON-ENUMERABLE** across tenants (B's existence
> and resources are invisible to A). This feature is the documented blocker for two accepted
> residuals: real cross-workspace test fixtures, and rate-bucket map eviction.

## Feature Summary

Every tenant-scoped table already carries a `workspace_id` FK (`teams`, `projects`, `issues`,
`invites` all `REFERENCES workspaces(id) ON DELETE CASCADE`; users hang off
`workspace_memberships`), and the code already scopes queries by `workspace_id` and already
collapses "not found" and "not yours" into one non-enumerable response (see
`crates/foundry-store/src/attachments.rs` `find_attachment_for_requester(requester_workspace_id)`).
The authz seam exists too: `is_workspace_admin(workspace_id, user_id)` and
`is_team_member(team_id, user_id)` in `crates/foundry-store/src/lib.rs`. **What is missing is
that more than one workspace can exist at all** — `uniq_one_workspace` forbids a second row —
and a **request → workspace resolution** seam so a signed-in user (or a machine-token
principal) acts on exactly their workspace. This feature:

- **Drops the single-workspace guard** and adds a workspace-resolution seam (which workspace
  does this request act on?) — the walking skeleton.
- **Proves the isolation boundary** on each surface end-to-end with REAL two-workspace
  fixtures (A and B coexisting), starting with the web htmx tier, then `/api/v1` + machine
  tokens, then sign-in/sessions.
- **Hardens non-enumerability** across tenants (uniform refusal; no existence leak; adversarial
  coverage).
- **Migrates existing single-workspace installs** forward to "workspace 1" with no data loss.
- **Provisions new tenants** (create a workspace + seed its first admin).
- **Closes the two residuals**: real cross-workspace fixtures + rate-bucket map eviction.

Feature type: **cross-cutting** (tenancy spans schema, request routing/resolution, authz, and
every workspace-scoped query, across the web htmx tier and the JSON API).

## Phase 1 — Discovery & Job Grounding

### No DIVERGE directory (RISK, low-medium impact)
There is no `docs/feature/multi-workspace-tenancy/diverge/` (no validated
`recommendation.md`/`job-analysis.md`). The jobs in `jobs.yaml` are NEW and Luna-derived from
(a) the brief, (b) a fresh 2026-06 reading of the single-workspace schema + authz seam, and
(c) the two residuals this unblocks. Importances/satisfactions are Luna estimates pending
user/field validation. Mitigation: the per-table `workspace_id` scoping + the non-enumerable
lookup pattern + the authz checks are SHIPPED and tested, so the blast radius is bounded to
(i) the tenant model, (ii) the user↔workspace cardinality, and (iii) provisioning authority —
all flagged as Open Product Decisions below.

### What was grounded by reading the actual 2026-06 code (not assumed)
- `crates/foundry-store/migrations/0001_init.sql`: `workspaces (id, name, created_at)`;
  **line 15** `CREATE UNIQUE INDEX uniq_one_workspace ON workspaces ((true))` — "I-W1: at most
  one workspace per instance." Every tenant table FKs to `workspaces(id) ON DELETE CASCADE`:
  `workspace_memberships (workspace_id, user_id, role)`, `teams (workspace_id, …, UNIQUE
  (workspace_id, slug))`, `projects (workspace_id, …, UNIQUE (workspace_id, key_prefix))`,
  `issues (workspace_id, …)`, `invites (workspace_id, …)`. **`users` is NOT FK'd to a
  workspace** — `users (id, email_lower UNIQUE, …)` is global; membership is the
  many-to-many `workspace_memberships`. So the schema ALREADY models a user belonging to
  potentially many workspaces (see OD-2). `signin_attempts` is keyed by `email_lower` only
  (no workspace), and `bootstrap_tokens` / the `tower_sessions` table are instance-global.
- `crates/foundry-store/src/lib.rs`: `is_workspace_admin(workspace_id, user_id)` checks
  `workspace_memberships.role='admin'`; `is_team_member(team_id, user_id)` checks
  `team_memberships`. Inserts (`insert_project`, issue create, invite create) already take
  and bind `workspace_id`. Reads already filter by it.
- `crates/foundry-store/src/attachments.rs`: `find_attachment_for_requester(id,
  requester_workspace_id)` — `WHERE id = $1 AND workspace_id = $2` — the established
  NON-ENUMERABLE pattern ("collapsing 'missing' and 'not yours'"). This is the seam the
  isolation boundary generalizes.
- Surfaces a tenant boundary must cover (read from the shipped features):
  the **htmx web tier** (`foundry-app`, session + double-submit CSRF, tower-sessions Postgres
  store, 30-day cookie, argon2id sign-in, non-enumerable sign-in error); the **JSON `/api/v1`**
  (`foundry-api`, the `MachinePrincipal` bearer extractor, the `is_workspace_admin` gate, the
  per-request `jti` denylist, the per-principal revoke-storm rate guardrail); **machine-token
  auth** (tokens carry `workspace_id` + optional `scope_team_id` in their claims —
  `machine_tokens` is workspace-scoped); **sign-in/sessions**; **backup/restore** (the US-03
  restore machinery — granularity is OD-5).
- The two residuals (from `docs/evolution/2026-06-07-machine-token-admin-ux.md` and
  `…06-08-token-management-api.md`): cross-workspace evil-user paths are tested with SYNTHETIC
  uuids because only one workspace can exist (UI-1, `distill/upstream-issues.md`); and the
  per-principal revoke-storm rate-bucket `HashMap` in `crates/foundry-app/src/rate_limit.rs`
  is bounded today ONLY by the single-workspace admin count (residual F2, "LRU / idle-eviction
  is the tracked mitigation for multi-workspace").

## Phase 2 — Scope Assessment (Elephant Carpaccio Gate)

### Scope Assessment: OVERSIZED — split required (user-approved thin-slice spine below)

Oversize signals checked (any 2+ = oversized) — **4 of 5 trip**:

| Signal | Verdict |
|--------|---------|
| >10 user stories | BORDERLINE — 9 stories (1 `@infrastructure`). Does not trip alone. |
| >3 bounded contexts / modules | **TRIPS** — touches `foundry-store` (schema + resolution), `foundry-app` (web htmx tier + sessions + rate-bucket), `foundry-api` (JSON surface + machine principals), `foundry-auth` (token workspace scoping), plus backup/restore. |
| Walking skeleton needs >5 integration points | **TRIPS** — resolution seam must thread through session → web handler → store query → authz; and the same for the bearer path. |
| Estimated effort >2 weeks | **TRIPS** — a security-critical, cross-cutting change with adversarial coverage on every surface; whole-feature estimate ~3-4 weeks. |
| Multiple independent user outcomes that could ship separately | **TRIPS** — "two tenants coexist", "boundary proven per surface", "non-enumerability hardened", "existing install migrated", "new tenant provisioned" are independently shippable, independently valuable outcomes. |

**Verdict: OVERSIZED.** This is exactly the case the Elephant Carpaccio gate exists to catch.
The feature is NOT split into separate `docs/feature/` directories (it is one coherent
isolation boundary), but it IS decomposed into **6 thin end-to-end slices**, each ≤1 day, each
with a named learning hypothesis and real-data acceptance. The riskiest, most load-bearing
thing — that two workspaces can coexist and a request resolves to the right one — is the
walking skeleton (Slice 1) and ships FIRST so every later slice builds on a proven seam.

### Proposed thin-slice spine (ordered; one-line learning hypothesis each)

| Slice | Name | Stories | Learning hypothesis (disproved if it fails) |
|-------|------|---------|----------------------------------------------|
| **1** | Walking skeleton: two workspaces coexist + request→workspace resolution | US-MWT00 (`@infra`, folded) + US-MWT01 | Two real workspaces can coexist in one instance and a request resolves to ITS workspace end-to-end on ONE surface — disproved if dropping `uniq_one_workspace` + adding resolution cannot keep two workspaces' data apart on even one read path. |
| **2** | Tenant-scoped authz + non-enumerable refusal on the WEB htmx tier | US-MWT02 | The existing `workspace_id` scoping + `is_workspace_admin` + the attachments-style non-enumerable lookup, driven by REAL A/B fixtures, refuse a member of A who reaches for B's resource identically to a non-existent one — disproved if any web read/write path leaks B to A. |
| **3** | Propagate isolation to the JSON `/api/v1` + machine-token + sign-in/session surfaces | US-MWT03 + US-MWT04 | The SAME boundary holds for a machine-token bearer principal and for a signed-in API caller, including how a request picks its workspace — disproved if a token scoped to A can touch B, or a session cannot be resolved to exactly one workspace. |
| **4** | Cross-tenant non-enumerability hardening | US-MWT05 | Across EVERY surface, a cross-tenant request reveals nothing about B's existence (uniform refusal, no 404-vs-403 oracle, no id-enumeration) — disproved if any surface's refusal differs in a way that confirms a foreign resource exists. |
| **5** | Migrate an existing single-workspace install to workspace 1 | US-MWT06 | A real pre-feature single-workspace DB upgrades forward-only with zero data loss and zero change to how its users sign in and work — disproved if the migration touches existing data or breaks existing sessions/tokens. |
| **6** | Provision a new tenant + close the two residuals | US-MWT07 + US-MWT08 | An operator can create a second workspace with a first admin, and the boundary can be PROVEN with real two-workspace fixtures while the rate-bucket map stays bounded — disproved if provisioning leaks across tenants or the bucket map grows unbounded under many tenants. |

**Slice taste-tests** (applied in `story-map.md` Phase 2.5; all pass or are documented):
Slice 1 ships the resolution abstraction FIRST (every later slice depends on it — correct per
the taste test "ship the abstraction first as its own slice"). No slice uses only synthetic
data — Slices 2-6 all require real two-workspace fixtures (the whole point of the feature; this
is also residual mwt-job-5). No two slices are identical-except-scale. Every slice disproves a
specific pre-commitment (see hypotheses).

## Key Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| **DM1** | **Drop `uniq_one_workspace` and introduce a request→workspace RESOLUTION seam.** This wave captures it as a REQUIREMENT + the walking-skeleton risk; the resolution mechanism (where the acting `workspace_id` comes from — session claim, URL segment, host/subdomain, token claim) is DESIGN. | Today exactly one workspace can exist and "the workspace" is implicit. Multi-tenant needs (a) the guard gone and (b) an explicit, single, auditable answer to "which workspace does this request act on?" so every existing `workspace_id`-scoped query is fed the right tenant. **The resolution mechanism is the central DESIGN decision** (see OD-1/OD-2). |
| **DM2** | **Isolation is enforced fail-closed and NON-ENUMERABLY at the workspace-scoping seam that already exists**, generalizing the `attachments.rs` `requester_workspace_id` pattern: a request for a resource outside the acting workspace is refused IDENTICALLY to a request for a non-existent one. | The non-enumerable pattern is already shipped and tested for attachments; the feature makes it the universal contract on every surface. DISCUSS fixes the OBSERVABLE property (uniform refusal, no existence leak); DESIGN picks the exact status/shape per surface. |
| **DM3** | **Tenant model defaults to shared-schema with `workspace_id` discriminator** (the model the schema ALREADY uses). Schema-per-tenant and db-per-tenant are NOTED as alternatives but NOT recommended. DESIGN ratifies. | Every tenant table already FKs to `workspaces(id)` and every query already scopes by `workspace_id`; the cheapest, lowest-risk path is to keep that model and remove only the single-workspace guard. (OD-1.) |
| **DM4** | **A user/email MAY belong to multiple workspaces** (the schema already supports this: `users` is global, membership is the many-to-many `workspace_memberships`). This implies a workspace-selection step somewhere in the signed-in experience. | `users.email_lower` is globally UNIQUE and there is NO `users.workspace_id`; membership is purely relational. Forcing one-user-one-workspace would CONTRADICT the shipped schema. DISCUSS adopts multi-membership as the default; the exact selection UX (at sign-in? a switcher? URL-driven?) is DESIGN. **This is OD-2 — flagged for explicit user ratification because it shapes sign-in + resolution.** |
| **DM5** | **Migration to multi-workspace is forward-only (ADR-003) and leaves the existing single workspace as "workspace 1" untouched** — only `uniq_one_workspace` is dropped and resolution added; no existing row's data is rewritten. | The existing workspace's `workspace_id` FKs already point at it; dropping a unique index does not touch data. The migration's only job is to remove the guard and let resolution default the one existing workspace. Zero data loss; existing sessions/tokens keep working. (mwt-job-3.) |
| **DM6** | **Provisioning authority defaults to the instance operator / a NEW instance-level super-admin role above workspace-admin.** Self-serve signup is NOTED but NOT the default. DESIGN + user ratify. | Creating a tenant on a shared instance is a privileged, instance-level act, distinct from administering a single workspace. There is no instance-level role today (only per-workspace `admin`/`member`). **OD-3 — flagged: this introduces a new role concept; user must confirm.** |
| **DM7** | **Backup/restore granularity defaults to whole-instance (unchanged from the US-03 restore machinery), with per-workspace export NOTED as a follow-up.** DESIGN ratifies; flagged because it interacts with the restore machinery and with tenant isolation (a per-workspace restore must not clobber a sibling). | The shipped restore operates on the whole instance. Per-workspace backup/restore is a meaningfully harder, isolation-sensitive feature. DISCUSS recommends keeping whole-instance for v1 and defers per-tenant export. **OD-5.** |
| **DM8** | **The two accepted residuals are IN scope** and mapped to mwt-job-5 / Slice 6: real two-workspace fixtures replace the synthetic-uuid cross-workspace tests, and the per-principal rate-bucket `HashMap` gains an eviction policy (LRU/idle) so it stays bounded under many tenants. | These residuals were explicitly accepted "under the single-workspace model" and named multi-workspace as the trigger to close them (residual F2 + UI-1). This feature is that trigger. |
| **DM9** | **Solution-neutral.** The resolution mechanism, the tenant model ratification, the workspace-selection UX, the instance-super-admin role shape, the per-surface refusal status, and the backup/restore granularity are DESIGN. | DISCUSS fixes the constraints (isolation fail-closed + non-enumerable on every surface, forward-only no-loss migration, bounded per-tenant resources) and the observable outcomes; DESIGN picks the mechanisms. |
| **DM10** | **Output uses the LEGACY per-feature layout** (separate files under `discuss/`), NOT the SSOT/feature-delta model; story IDs use the `US-MWT0x` namespace; slice briefs under `slices/`. | Per the brief: `docs/product/` does not exist and we are intentionally NOT migrating (20+ prior features use the per-feature layout). Mirrors `machine-token-admin-ux/discuss/` + `token-management-api/discuss/`. Trunk-based: commit directly to `main`, no branch/PR. |

## Open Product Decisions (propose a default; user/DESIGN ratify)

| # | Decision | Why it matters | Proposed default (pending ratification) |
|---|----------|----------------|-----------------------------------------|
| **OD-1** | **Tenant model**: shared-schema-with-`workspace_id` vs schema-per-tenant vs db-per-tenant. | Determines isolation mechanism, migration shape, and operational model. | **Shared-schema with `workspace_id` discriminator** — matches the current schema exactly; lowest risk; every query already scopes by it. DESIGN ratifies. (DM3.) |
| **OD-2** | **User↔workspace cardinality**: can one user/email belong to multiple workspaces, or is a user scoped to exactly one? | Big UX + auth implication — drives whether sign-in needs a workspace-selection step and how a request resolves its workspace. | **Multi-membership** (one user MAY belong to many workspaces) — the schema already models this (`users` global + `workspace_memberships` M:N). Implies a workspace-selection/switcher UX (DESIGN owns the exact form). **User must ratify** — it shapes sign-in + resolution. (DM4.) |
| **OD-3** | **Workspace creation / provisioning authority**: who creates workspaces (instance super-admin? self-serve signup?) and is there an instance-level role above workspace-admin? | Determines the provisioning surface, a possible new role concept, and the attack surface for tenant creation. | **Instance-operator / NEW instance-level super-admin only** for v1 (no self-serve signup). Introduces a role above workspace-`admin`. **User must ratify** — new role concept. (DM6.) |
| **OD-4** | **Existing-install migration**: confirm the current single workspace becomes "workspace 1" with a seamless, forward-only backfill (no data rewrite, existing sessions/tokens keep working). | The single highest-stakes data-safety decision; every existing install upgrades through it. | **Forward-only drop of `uniq_one_workspace` + resolution defaults the one existing workspace; no existing data is touched.** Confirmed-by-default; user confirms acceptable. (DM5, mwt-job-3.) |
| **OD-5** | **Backup/restore granularity**: per-workspace vs whole-instance (interacts with the US-03 restore machinery + isolation). | A per-workspace restore must not clobber a sibling tenant; whole-instance restore is simpler but coarser. | **Whole-instance for v1** (unchanged from shipped restore); per-workspace export deferred as a follow-up. DESIGN ratifies. (DM7.) |

## Requirements Summary

- **9 user stories**, 1 explicitly `@infrastructure` (US-MWT00 — drop the guard + the
  resolution seam; the substrate the user-visible stories stand on — folded into the walking
  skeleton, never shipped standalone).
- **6 thin slices** (spine above), walking skeleton = Slice 1 (two workspaces coexist + request
  resolution end-to-end on ONE surface).
- Primary jobs: host several teams in one instance (mwt-job-1); be certain my data is invisible
  to other tenants, non-enumerably (mwt-job-2). Supporting: forward-only migration of existing
  installs (mwt-job-3); provision a new tenant (mwt-job-4); prove the boundary with real
  fixtures + bound per-tenant resources (mwt-job-5).
- NFRs (SECURITY-heavy): tenant isolation (HARD), cross-tenant non-enumerability, forward-only
  migration safety (no data loss for the existing workspace), bounded per-tenant in-memory
  resources, plus carried invariants (one binary, one Postgres, browser auth/CSRF/sessions, the
  shipped verify path unchanged). See `nfrs.md`.

## Constraints Established

- ONE binary, ONE Postgres, no Redis, no Node runtime service, no CDN (carried).
- Isolation is FAIL-CLOSED and NON-ENUMERABLE on EVERY surface (web htmx tier, JSON `/api/v1`,
  machine-token auth, sign-in/sessions): a request for another tenant's resource is refused
  identically to a request for a non-existent one.
- Every tenant-scoped read/write is scoped by the acting `workspace_id`; the acting workspace
  is resolved at a single, auditable seam.
- Migration is FORWARD-ONLY (ADR-003); the existing single workspace becomes workspace 1 with
  no data rewrite; existing sessions/tokens/sign-in keep working.
- Reuse (don't rebuild) the shipped substrate: the per-table `workspace_id` scoping, the
  `attachments.rs` non-enumerable lookup pattern, `is_workspace_admin`, `is_team_member`, the
  machine-token verify path + `jti` denylist.
- Per-tenant in-memory resources (the revoke-storm rate-bucket map) MUST be bounded under many
  tenants (eviction policy).
- Solution-neutral: tenant model ratification, resolution mechanism, workspace-selection UX,
  instance-super-admin role, per-surface refusal shape, and backup/restore granularity are
  DESIGN.

## Risks Surfaced (for DESIGN's risk register)

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|-----------|
| A surface forgets to scope by `workspace_id`, leaking tenant A's data to B | Medium | **Critical** | The boundary is enforced at the resolution seam + the per-table scoping that already exists; US-MWT05 + the NFRs require an enumerated, adversarially-tested list of every surface; real A/B fixtures (US-MWT08) catch what synthetic uuids missed. |
| A 404-vs-403 (or timing/error-shape) difference reveals that B's resource exists | Medium | High | NFR-MWT-SEC-02 mandates a uniform non-enumerable refusal generalizing the shipped `attachments.rs` pattern; US-MWT05 asserts no existence oracle on any surface. |
| The forward-only migration touches or cross-wires existing single-workspace data | Low | **Critical** | DM5/OD-4: the migration only DROPS `uniq_one_workspace` + adds resolution; existing FKs already point at the one workspace; US-MWT06 asserts zero data loss + existing sessions/tokens still work on a real pre-feature DB. |
| Workspace resolution is ambiguous (a request maps to zero or >1 workspace) | Medium | High | DM1: resolution must yield EXACTLY one workspace, fail-closed if none; multi-membership (OD-2) means selection must be explicit; US-MWT04 covers the resolution contract. |
| Machine token scoped to A is accepted against B | Low | **Critical** | Tokens already carry `workspace_id`; US-MWT03 asserts a token's workspace claim is enforced as the acting workspace and a cross-tenant call is refused non-enumerably. |
| Per-principal rate-bucket map grows unbounded across many tenants (residual F2) | Medium | Medium | DM8 / US-MWT08: add an eviction policy (LRU/idle); keyed by principal `user_id` as today. |
| OD-2 (multi-membership) or OD-3 (instance super-admin) ratified differently than the default | Medium | Medium | Both flagged for explicit user ratification BEFORE DESIGN; stories written so the resolution seam + provisioning surface can absorb either answer. |
| No DIVERGE validation of the NEW jobs | Medium | Low | Substrate is shipped/tested; the only real unknowns (tenant model, cardinality, provisioning) are flagged as OD-1/2/3; confirm job ranking before DESIGN. |

## Open Product Decisions — STATUS: RATIFIED 2026-06-09

User ratified at the end of DISCUSS:
- **OD-2 = Multi-membership (RATIFIED).** A user/email MAY belong to multiple workspaces. DESIGN
  owns the workspace-selection/switch UX + the request→workspace resolution mechanism (session
  carries the active workspace), but the cardinality is settled: multi-membership.
- **OD-3 = Instance super-admin only (RATIFIED).** A NEW instance-level super-admin role (above
  workspace-admin) provisions tenants. No self-serve signup for v1. Bootstrap creates workspace 1
  + the super-admin. DESIGN owns the role's exact shape + the provisioning surface.

OD-1 (shared-schema + `workspace_id`), OD-4 (forward-only migration, no data touch), and OD-5
(whole-instance backup for v1) stand at their recommended defaults — DESIGN ratifies the mechanisms.

## Assumptions about the current single-workspace code (flag for verification)

1. **`users` is intentionally global (not workspace-scoped).** Read from `0001_init.sql`:
   `users.email_lower` is globally UNIQUE and there is no `users.workspace_id`; membership is
   the M:N `workspace_memberships`. Assumed this is deliberate and that multi-membership is
   therefore schema-supported (drives OD-2/DM4). Verify no other code path assumes one-user-one-
   workspace.
2. **`signin_attempts` is keyed by `email_lower` only (no workspace).** Assumed the brute-force
   throttle is intentionally instance-global per email, not per-workspace; multi-workspace does
   not change this unless OD-2 says otherwise.
3. **`bootstrap_tokens` + the `tower_sessions` table are instance-global.** Assumed the existing
   bootstrap flow creates the FIRST workspace; provisioning a SECOND (mwt-job-4/US-MWT07) is the
   new path. Verify the bootstrap flow's single-workspace assumptions.
4. **The non-enumerable `attachments.rs` pattern is the canonical isolation idiom.** Assumed it
   generalizes to every tenant-scoped resource; DESIGN should confirm each surface adopts it.
5. **`machine_tokens` rows carry `workspace_id` (+ optional `scope_team_id`)** and the bearer
   path already binds a workspace. Assumed a token's `workspace_id` is the authoritative acting
   workspace for `/api/v1` (drives US-MWT03). Verify against the `MachinePrincipal` extractor.
6. **No code currently depends on `uniq_one_workspace` for correctness** (e.g. a query that
   assumes "the one workspace"). Assumed all reads already filter by `workspace_id`; a grep for
   un-scoped `FROM teams|projects|issues` is a DESIGN/DELIVER guard to confirm before dropping
   the index.
