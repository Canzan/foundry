# DESIGN Decisions — multi-workspace-provisioning

> Morgan (nw-solution-architect), DESIGN wave, application/component scope, **Propose** mode.
> The deferred slices 5-6 of the shipped `multi-workspace-tenancy` isolation core, now their own
> feature. Requirements INHERITED (no DISCUSS re-run); seed = the slice-05/06 briefs + the parent
> `discuss/{wave-decisions,nfrs}.md` + the deferred parent ADR-004/006 + the evolution doc.
> Legacy per-feature layout (`docs/feature/multi-workspace-provisioning/design/`). Trunk-based.

## Reading checklist
- ✓ `slices/slice-05-existing-install-migration.md` (US-MWT06)
- ✓ `slices/slice-06-provision-and-prove.md` (US-MWT07/08)
- ✓ `discuss/wave-decisions.md` (OD-1..5; OD-3 instance super-admin RATIFIED)
- ✓ `discuss/nfrs.md` (NFR-MWT-SEC/DATA/REL/PERF/TEST)
- ✓ `design/adr-004-instance-super-admin-role.md` (deferred parent ADR — FIRMED here)
- ✓ `design/adr-006-forward-only-migration.md` (migration discipline — guarantee FIRMED here)
- ✓ `design/architecture.md` (parent — the resolution seam, ActingWorkspace, LAYER-1e)
- ✓ `docs/evolution/2026-06-11-multi-workspace-tenancy.md` (what shipped; what deferred)
- ✓ `crates/foundry-app/src/bootstrap.rs` (the 409 guard + create_initial_workspace seeding)
- ✓ `crates/foundry-app/src/rate_limit.rs` (RevokeRateLimiter + F2 residual note + clock seam)
- ✓ `crates/foundry-app/src/admin_cli.rs` (the operator-CLI home + live-DB sqlx precedent)
- ✓ `crates/foundry-store/src/lib.rs` (resolve/set_active_workspace, is_workspace_admin, seeding tx)
- ✓ `crates/foundry-store/migrations/{0001,0009,0010}.sql` (schema, guard-drop, active-ws column)
- ✓ `xtask/src/check_arch.rs` (LAYER-1e rule + the bootstrap/admin_cli allow-list)

## Key Decisions (DDD-numbered)

| # | Decision | Rationale | ADR |
|---|---|---|---|
| **D1** | First super-admin is seeded by the bootstrap CLAIM (operator becomes ws1 admin AND first super-admin); upgraded installs use `foundry doctor grant-super-admin` (idempotent). | One "claim the instance" entry; no fresh instance with a workspace but no provisioning authority; clean grant path for upgrades. FIRMS parent ADR-004. | adr-001 |
| **D2** | Provisioning surface is **CLI-first for v1** (`foundry doctor provision-workspace …`); the web `/admin/instance/…` flow is DEFERRED. | Smallest attack surface (shell access already implies host trust), off the bearer surface, reuses the allow-listed `run_restore_comment` scaffold, ships faster. **REVISES parent ADR-004's web-first surface** (see upstream-changes.md). | adr-002 |
| **D3** | NEW `instance_admins(user_id PK)` table + `is_instance_admin(user_id)` authz in services/store (mirrors `is_workspace_admin`'s `EXISTS` shape). | Explicit, auditable, minimal; instance-scoped by construction so it cannot trip/confuse the LAYER-1e tenant guard. FIRMS parent ADR-004. | adr-003 |
| **D4** | Slice-5 guarantee = a REAL-snapshot before/after-equality PROOF; **NO backfill migration**. | `resolve_active_workspace` already maps (NULL `active_workspace_id` + sole membership) ⇒ workspace 1 deterministically. A backfill would rewrite rows OD-4 promises to leave untouched, for zero gain. FIRMS parent ADR-006's Earned-Trust clause. | adr-004 |
| **D5** | Rate-bucket eviction = std-only **idle eviction** (`W = ceil(C/R)s`, behaviour-preserving) + an LRU size-cap fallback, keyed off the SHIPPED clock. | Bounds the map by active principals (F2) while preserving throttle correctness; the module's own note proves idle eviction is behaviour-preserving. ZERO new crate. | adr-005 |
| **D6** | The genuinely-new schema is ONE additive migration `0011_instance_admins.sql` (+ the empty role table); everything else EXTENDS shipped code. | Latest migration is `0010`; forward-only (ADR-003 discipline), edits no prior migration, no row rewrite. | adr-003/004 |
| **D7** | No new check-arch rule for v1; provisioning lands in the ALREADY allow-listed `admin_cli` + `bootstrap`. A future web flow in a NEW file WOULD owe a LAYER-1e allow-list line. | `is_tenant_scoping_allowlisted` (`check_arch.rs:394`) already exempts both; `is_instance_admin` is non-tenant-scoped so it cannot trip the detector. | adr-002/003 |

## Architecture Summary
- **Pattern**: modular monolith + ports-and-adapters (inherited, in force). Provisioning is a
  `foundry-services` use-case driven by an `admin_cli` driving port; authz + the new role live in
  services/store; the eviction is an adapter-layer (composition-root) policy.
- **Paradigm**: Rust (multi-paradigm; the codebase is composition-over-inheritance, functional-core
  / imperative-shell — unchanged).
- **Key components**: `instance_admins` table + `is_instance_admin` (NEW); `create_workspace`
  use-case + `provision_workspace` tx (NEW, reuse the seeding shape); `provision-workspace` CLI
  subcommand (EXTEND `admin_cli`); first-super-admin seed (EXTEND `bootstrap`); idle/LRU eviction
  (EXTEND `rate_limit`); slice-5 real-snapshot proof (NEW, test/DISTILL).

## Reuse Analysis (verdict: 9 REUSE/EXTEND · 2 CREATE NEW — overwhelmingly inheritance)

| # | Existing component | File | Overlap | Decision | Justification |
|---|---|---|---|---|---|
| 1 | `create_workspace` 409 guard (still present) | `foundry-app/src/bootstrap.rs:301` | Provisioning call-site | **EXTEND** | The literal provisioning handler; v1 gates via CLI, web 409 replaced when the deferred flow lands. |
| 2 | `create_initial_workspace` seeding tx | `foundry-store/src/lib.rs:307` | Atomic ws+admin+team+project insert | **REUSE/EXTEND** | `provision_workspace` mirrors this exact tx; no new seeding mechanism. |
| 3 | bootstrap/invite seeding idiom | `bootstrap.rs:234`, `invites` (`0001:93`) | Seed first admin | **REUSE** | New tenant's first admin seeded by the same idiom as ws1's. |
| 4 | `is_workspace_admin` | `lib.rs:1128` | Role-authz `EXISTS` shape | **REUSE (shape)** | `is_instance_admin` mirrors it, instance-scoped. |
| 5 | `admin_cli` + `run_restore_comment` live-DB sqlx scaffold | `admin_cli.rs:235` | Operator subcommand | **EXTEND** | `provision-workspace` reuses the whole scaffold; ALREADY allow-listed. |
| 6 | LAYER-1e allow-list | `check_arch.rs:387` | Tenant-guard exemption | **REUSE** | `bootstrap`+`admin_cli` already exempt — no new entry for v1. |
| 7 | `resolve_active_workspace` | `lib.rs:419` | Post-migration resolution | **REUSE** | NULL active + sole membership ⇒ ws1 ⇒ **no backfill**. |
| 8 | `RevokeRateLimiter` map | `rate_limit.rs:106` | Bounded in-memory resource | **EXTEND** | Add idle/LRU eviction; keep `user_id` key + semantics + 100%-mutation tests. |
| 9 | shipped clock seam | `clock.rs` (via `ClockedRevokeGuard`) | Deterministic time | **REUSE** | Eviction reads the same clock; no new time source/crate. |
| 10 | `instance_admins` / `is_instance_admin` | — (does not exist) | Instance authority | **CREATE NEW** | No instance-level role today; OD-3 ratified a new one. |
| 11 | `0011_instance_admins.sql` | — (new file) | Schema change | **CREATE NEW** | A migration is by definition a new forward-only file; adds one table; no rewrite. |

## Technology Stack
- **Rust** (inherited): axum, askama, sqlx, std `Mutex`/`HashMap`, the shipped clock seam, `metrics`
  facade. **ZERO new crates** — eviction is `std::collections::HashMap::retain` over the shipped
  clock; `lru`/`dashmap`/`moka` are explicitly REJECTED (ADR-005, parent zero-new-crate discipline).
- **PostgreSQL** (one instance, inherited): one additive migration `0011_instance_admins.sql`.
- **Enforcement**: `cargo xtask check-arch` (inherited; no new rule for v1).
- **OSS-first / license**: all inherited deps; no proprietary; no new dependency to license.

## Constraints Established / honored
- ONE binary · ONE Postgres · NO Redis · NO Node · NO CDN · **ZERO new crates**.
- Provisioning is super-admin-gated, fail-closed, and OFF the `/api/v1` bearer surface (`api≠mint`).
- The migration is forward-only, rewrites no row; the existing workspace stays workspace 1.
- The browser auth/CSRF/session contract + the machine-token verify path are byte-for-byte unchanged.
- The `foundry-acceptance` suite green-before stays green-after.
- Provisioning needs **no new LAYER-1e allow-list entry for v1** (`admin_cli`+`bootstrap` already
  exempt); a future web-flow file WOULD owe one.

## Earned-Trust (probe-don't-assume) commitments for DISTILL/DELIVER
- **Slice 5**: a REAL pre-feature DB snapshot is upgraded and before/after row-equality +
  workspace-id-unchanged + carried-session/token-still-resolves are asserted (ADR-004) — the
  migration contract is PROVED, not assumed.
- **Slice 6a**: `create_workspace` is exercised by a non-super-admin (refused, fail-closed) AND a
  super-admin (creates B; A asserted untouched — NFR-MWT-REL-01).
- **Slice 6b**: a property/unit test proves the map stays bounded under many idle+active principals
  AND that an ACTIVE principal's throttle is byte-identical with/without eviction (ADR-005).

## Open decisions awaiting user ratification (BEFORE DISTILL)

The 5 design decisions all sit at recommended options; the THREE that most change the design (rank
ordered) and need explicit user confirmation:

1. **[D2 / adr-002] Provisioning surface = CLI-first, web DEFERRED — REVISES parent ADR-004.**
   *Most design-changing.* The parent ADR-004 chose a web flow as the v1 surface; this DESIGN
   revises to a CLI subcommand on operational-safety grounds. If the user wants the web flow in v1,
   the component set, the test surface, and a NEW LAYER-1e allow-list entry all change. **Confirm
   CLI-first.** (See upstream-changes.md.)
2. **[D1 / adr-001] First super-admin == the bootstrap-claiming operator (ws1 admin).** Settles
   whether the super-admin is the same human as ws1's admin (recommended) or a separate
   instance-level identity. Shapes the bootstrap claim transaction + the upgraded-install grant
   path. **Confirm operator-is-first-super-admin + the `grant-super-admin` upgrade path.**
3. **[D5 / adr-005] Eviction policy = idle (`W=ceil(C/R)s`) + LRU size-cap fallback, std-only.**
   The user may wish to tune the idle window / size-cap constants, or accept the one-directional
   (never-over-throttle) relaxation the size-cap fallback allows under pathological load. **Confirm
   the policy + that the size-cap relaxation is acceptable.**

D3 (`instance_admins` table) and D4 (no backfill, real-snapshot proof) sit at their inherited /
grounded defaults and need confirmation only if the user disagrees — D4 in particular is a grounded
finding (NULL-resolves-fine), not a judgement call.

## Upstream Changes
See `upstream-changes.md` — two findings: (a) the parent ADR-004's web-first surface is revised to
CLI-first (D2); (b) the evolution doc's claim that `0009` "removes the application 409 guard" is
inaccurate — the `bootstrap.rs:301` 409 guard is STILL PRESENT (the DB index was dropped, the app
handler was not). Neither blocks DESIGN; both are recorded so DELIVER does not trip on them.
</content>

## Open Decisions — RATIFIED 2026-06-11

User ratified the three flagged decisions (all at the DESIGN-recommended option):
- **D2 = CLI-first provisioning (RATIFIED).** `foundry doctor provision-workspace --name … --admin-email …` is the v1 surface; the web `/admin/instance` flow is DEFERRED to a later increment. This REVISES the parent ADR-004's web-first sketch. No new LAYER-1e allow-list entry needed (the `admin_cli` path is already allow-listed).
- **D1 = Bootstrap-claiming operator = first super-admin (RATIFIED).** Whoever claims the bootstrap token (creating workspace 1 + its admin) also becomes the first `instance_admins` row; existing installs get an idempotent `foundry doctor grant-super-admin --email …`. No separate instance identity.
- **D5 = Idle-window + LRU size-cap eviction (RATIFIED).** Evict buckets idle > W=ceil(C/R)s (behaviour-preserving) with an LRU size-cap fallback under pathological load (one-directional: only ever under-throttles, never over-throttles an active principal). std-only off the shipped clock; zero new crate.

D3 (instance_admins schema + is_instance_admin) and D4 (migration guarantee = real-snapshot before/after-equality, NO backfill) stand at their recommended options. Ready for DISTILL.
