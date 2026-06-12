# Evolution — multi-workspace-provisioning (slices 5-6: provisioning + migration-guarantee + eviction)

**Finalized**: 2026-06-12
**Ship commits**: `299a8c1` (the `0011` migration / instance-admin foundation) → `49f1d0e` (idle+LRU eviction) → `3cf4ec1` (mutation-harden) — the 16 DES-monitored TDD steps, committed directly to `main` (trunk-based, no PRs).
**Wave coverage**: requirements INHERITED from the parent (no DISCUSS re-run); DESIGN ratified here (D1–D7, 5 ADRs); DISTILL authored 15 acceptance scenarios; DELIVER shipped 16 steps slice-by-slice. Legacy per-feature layout (`docs/feature/multi-workspace-provisioning/`).
**Scope**: this finalizes the **deferred slices 5-6** of the shipped `multi-workspace-tenancy` isolation core (see `docs/evolution/2026-06-11-multi-workspace-tenancy.md`), delivered as their own feature. The parent shipped the isolation CORE (slices 1-4); this ships the **provisioning surface**, the **migration-as-guarantee proof**, and **closes the F2 rate-bucket residual**. The entire feature directory is PRESERVED (same policy as the parent).

## Feature summary

The parent milestone made tenancy REAL — multiple workspaces coexist with genuine per-tenant isolation across every surface — but it intentionally deferred two things: the **way new tenants come into existence** (the provisioning surface + the instance super-admin role that authorizes it) and the **formal upgrade-safety proof** for existing single-workspace installs. It also left one accepted residual open: the per-principal revoke rate-bucket map (F2) was bounded only by the active-principal count, which grows with tenants.

This feature closes all three. It is overwhelmingly **EXTEND, not CREATE NEW** (DESIGN reuse verdict: 9 reuse/extend · 2 create-new): provisioning reuses the bootstrap seeding transaction and the already-allow-listed `admin_cli` scaffold; the migration proof reuses the shipped `resolve_active_workspace` seam; the eviction reuses the shipped clock seam. The only genuinely new artifacts are one additive migration (`0011_instance_admins.sql`) and the `instance_admins` table + `is_instance_admin` authz function. **ZERO new crates.** Forward-only migrations.

## What shipped

### Slice 6 — provisioning (D1/D2/D3/D6/D7)

- **The instance super-admin role (D3/D6)**: additive migration `0011_instance_admins.sql` creates `instance_admins(user_id PK)`; `is_instance_admin(user_id)` is an EXISTS-shape authz predicate in store + services, mirroring the shipped `is_workspace_admin` shape. It is **instance-scoped by construction** (takes no workspace argument), so it sits **off the LAYER-1e tenant guard** and cannot trip it — which is exactly why D7 needed no new check-arch rule. Fail-closed: an empty table denies every user; an absent row denies.
- **First super-admin seeding (D1)**: the bootstrap CLAIM now seeds the operator as the first `instance_admins` row in the **same atomic claim transaction** that creates workspace 1 and its admin — no fresh instance can exist with a workspace but no provisioning authority. Upgraded installs gain a super-admin via an idempotent `foundry doctor grant-super-admin` CLI (`INSERT … ON CONFLICT DO NOTHING`).
- **CLI-first provisioning (D2)**: `foundry doctor provision-workspace` is the v1 surface. It is gated by `is_instance_admin` (fail-closed), runs an atomic workspace + first-admin transaction (reusing the bootstrap seeding shape), and returns structured exit codes (0 success / 2 / 3 / 4). The web `/admin/instance/…` flow is **DEFERRED** — this REVISES the parent ADR-004's web-first surface (recorded in `upstream-changes.md`).
- **Provisioned-tenant isolation proven (green-by-inheritance, then probed)**: a freshly-provisioned workspace honors the shipped isolation boundary through the scoped-read seam; cross-tenant + unauthorized refusals are proven **non-enumerable** — byte-identical responses, no 403-vs-404 oracle; and provisioning is proven **unreachable from the `/api/v1` bearer surface** (the `api≠mint` boundary holds).

### Slice 5 — migration-as-guarantee (D4)

- An existing single-workspace install upgrades via the forward-only `0009`/`0010`/`0011` path with **NO backfill and NO row rewrite**. The shipped `resolve_active_workspace` already maps (NULL `active_workspace_id` + sole membership) ⇒ workspace 1 deterministically, so a backfill would rewrite rows OD-4 promises to leave untouched for zero gain.
- This is **proved**, not assumed: a real-snapshot before/after **row-for-row equality harness** across all tenant tables (against a real DB), the workspace id unchanged, carried sessions + machine tokens still resolving, and existing sign-in / board behaviour unchanged. The **no-backfill observable** — `active_workspace_id` stays NULL after the upgrade — is verified against a real DB.

### Slice 6b — rate-bucket eviction (D5)

- Std-only **idle + LRU eviction** on `RevokeRateLimiter`: idle eviction with window `W = ceil(C/R)s` (behaviour-preserving by the module's own documented rule — an idle-evicted bucket re-creates at full capacity), plus an LRU size-cap fallback that is **one-directional** (it can only ever relax an active principal's throttle, never tighten it). Keyed off the **shipped clock seam** — deterministic under `MockClock`. This **closes the F2 unbounded-map residual** carried from `token-management-api` / `machine-token-admin-ux`. **ZERO new crate** (`std::collections::HashMap::retain`).

## Decisions realized (D1–D7)

| # | Decision | Status |
|---|---|---|
| **D1** | Bootstrap claim seeds the operator as first super-admin (same atomic tx); upgraded installs use idempotent `foundry doctor grant-super-admin`. | **IMPLEMENTED** |
| **D2** | Provisioning is **CLI-first** (`foundry doctor provision-workspace`); web `/admin/instance/…` flow DEFERRED. REVISES parent ADR-004's web-first surface. | **IMPLEMENTED** (web flow deferred) |
| **D3** | NEW `instance_admins(user_id PK)` table + `is_instance_admin` EXISTS-shape authz in store/services. | **IMPLEMENTED** |
| **D4** | Migration-as-guarantee = real-snapshot before/after-equality PROOF; **NO backfill**. | **IMPLEMENTED** |
| **D5** | Std-only idle (`W=ceil(C/R)s`) + LRU size-cap eviction on the revoke rate-bucket map, off the shipped clock. | **IMPLEMENTED** |
| **D6** | The only genuinely-new schema is ONE additive migration `0011_instance_admins.sql`. | **IMPLEMENTED** |
| **D7** | No new check-arch rule for v1 (`admin_cli`+`bootstrap` already allow-listed; `is_instance_admin` is non-tenant-scoped). | **IMPLEMENTED** |

Upstream-changes findings honored: D2's CLI-first revises the parent ADR-004's web-first surface, and the `bootstrap.rs` 409 guard remains present (the parent evolution doc's "removes the 409 guard" claim was inaccurate — the DB index was dropped, the app handler was not).

## How it was built (DELIVER) — the 16-step TDD arc

**16 DES-monitored TDD steps across 5 phases**, each driven by `@real-io` cucumber scenarios over the real surfaces (real CLI invocation, real testcontainers PG16, real EdDSA bearer), every step running all 5 DES phases (integrity exit 0).

| Phase / slice | Steps | What it proved |
|---|---|---|
| **01 — instance-admin foundation** | 01-01..02 | additive `0011` creates `instance_admins` (idempotent, no prior-table rewrite); `is_instance_admin` EXISTS-shape predicate, fail-closed, instance-scoped |
| **02 — first super-admin bootstrap (D1)** | 02-01..02 | bootstrap claim seeds the operator as first super-admin in the same tx; `grant-super-admin` CLI idempotent for upgraded installs |
| **03 — CLI provisioning + isolation proof (D2)** | 03-01..06 | `provision-workspace` happy path creates a real isolated tenant; existing workspaces untouched; provisioned tenant honors the isolation boundary; cross-tenant reach non-enumerable; authz refusal fail-closed + non-enumerable; provisioning unreachable from the bearer API |
| **04 — migration-as-guarantee (slice 5 / D4)** | 04-01..05 | existing install upgrades and resolves to workspace 1; real-snapshot before/after row equality; carried sessions + machine tokens still resolve; no-backfill observable (`active_workspace_id` stays NULL); existing sign-in + board unchanged |
| **05 — rate-bucket eviction (D5)** | 05-01 | idle + LRU size-cap eviction bounds the `RevokeRateLimiter` map while preserving active-principal throttle correctness |

A revert-reds-it litmus was applied to the green-by-inheritance isolation scenarios to prove they were real, not fixture theater (reverting the production change had to re-RED the security assertion).

## Quality at ship

`cargo xtask ci` — **ALL GATES GREEN**:

- **fmt**: `cargo fmt --all --check` clean.
- **clippy**: `cargo clippy --all-targets --release -- -D warnings` clean.
- **`@all` acceptance**: **275 scenarios / 2278 steps** green (the parent suite plus this feature's 15 new scenarios; green-before stays green-after).
- **Adversarial review**: **APPROVED** — Testing Theater: none found.
- **Scoped mutation testing**: **100% (28/28 viable mutants killed)** on `rate_limit` + the new store/services functions — including a **security-critical authz-gate-inversion mutant** on the provisioning gate.
- **Zero new crates.** Forward-only migrations.

## Deferred / follow-ups

**Deferred design surface:**
- **Web provisioning flow** `/admin/instance/…` (D2 deferred to a follow-up). If/when it lands in a new file it owes a LAYER-1e allow-list entry.
- **Real invite-accept / password-set flow.** No `/invites/accept` route exists; the provisioned first-admin "sign in" is proven via the shipped `resolve_active_workspace` membership seam (the same approximation the parent slices used for US-MWT04). A real invite-accept flow is a follow-up.

**Review findings (non-blocking, from the APPROVED adversarial review — code is correct):**
- **D1**: add a test variant covering the `--as never-registered` vs `--as non-admin` refusal-identity (secondary oracle).
- **D2**: the `invites` row in slice-06 scenario-2's snapshot is a vacuous `[]==[]` — seed an invite or drop `invites` from the table list.
- **D3**: refactor `run_provision_workspace` (157-line method; extract shared isolated-runtime / store-connect helpers — the refactor pass was de-scoped this run).
- **D4**: add `instance_admins` to `foundry doctor backup-verify`'s table list.

**Known test-infra flakiness (pre-existing, not feature-specific):**
- Concurrent testcontainers tests can hit a sqlx `unknown message type '\0'` wire-protocol race (seen in 02-01 and when running services tests in parallel); passes serially.

**Still-open from the parent:**
- Prometheus exporter for `foundry_token_mutations_total`.
- Per-workspace backup/restore (OD-5) — whole-instance backup unchanged.
- Key-rotation UX.

## Pointers

- Spec (preserved): `docs/feature/multi-workspace-provisioning/{design,distill,deliver}/` — notably `design/wave-decisions.md` (D1–D7 ratification), the 5 ADRs (`adr-001..005`), `design/upstream-changes.md` (the CLI-first revision + the 409-guard finding), and the 15 DISTILL scenarios.
- DES roadmap + execution log (the audit trail, preserved): `docs/feature/multi-workspace-provisioning/deliver/roadmap.json` (5 phases / 16 steps) + `execution-log.json` (DES-verify-integrity clean) + `deliver/mutation/mutation-report.md`.
- Core production files:
  - Migration: `crates/foundry-store/migrations/0011_instance_admins.sql`.
  - Authz + provisioning: `crates/foundry-store/src/lib.rs` (`is_instance_admin`, the provision transaction), `crates/foundry-app/src/bootstrap.rs` (first-super-admin seed), `crates/foundry-app/src/admin_cli.rs` (`provision-workspace` + `grant-super-admin` subcommands).
  - Eviction: `crates/foundry-app/src/rate_limit.rs` (idle + LRU eviction on `RevokeRateLimiter`).
  - Acceptance: the slice-05/06 `.feature` files + step defs under `crates/foundry-acceptance/`.
- Predecessor: `docs/evolution/2026-06-11-multi-workspace-tenancy.md` (the isolation core this completes).
