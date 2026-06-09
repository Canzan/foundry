# Multi-Workspace Tenancy — DESIGN Wave Decisions

> Morgan (nw-solution-architect), DESIGN wave, application/component scope, **Propose** mode:
> options + recommendation per decision; the user ratifies at the post-roadmap checkpoint. Builds
> DIRECTLY on the RATIFIED DISCUSS decisions (OD-1 shared-schema; OD-2 multi-membership; OD-3
> instance super-admin only; OD-4 forward-only no-touch migration; OD-5 whole-instance backup v1).
> One DISCUSS assumption is REFINED (not contradicted) — see "Upstream changes" + `upstream-changes.md`.

## Architecture summary

A **predominantly-EXTEND, cross-cutting feature** over the modular monolith + ports-and-adapters
already in force. The schema is already multi-tenant-shaped (every tenant table FKs to
`workspaces(id)`, every read binds `workspace_id`), the non-enumerable lookup idiom is shipped
(`attachments.rs`), authz lives in `foundry-services`, and — critically — the **request→workspace
resolution seam already exists**: at sign-in `SessionUser{ user_id, workspace_id }` is written to
the session and read per-request; on the API, `Principal::Machine{ workspace_id }` carries the
token's claim. This feature removes two guards (the DB `uniq_one_workspace` index AND a hard-coded
409 in `create_workspace`), generalizes the resolution seam from "the sole workspace" to "the
membership-resolved active workspace", makes scoping structurally hard to forget (a new check-arch
rule + an `ActingWorkspace` newtype), and proves the boundary with real A/B fixtures. **Zero new
crates.**

- **Pattern**: modular monolith + ports-and-adapters (in force; one binary, one Postgres).
- **Paradigm**: Rust — neither pure OOP nor FP; the project's existing idioms (typed handles,
  explicit threading, newtypes) are followed.
- **Key components**: the resolution seam (`SessionUser` EXTEND + `token.workspace_id` reuse), the
  `ActingWorkspace` newtype, the generalized non-enumerable lookup, `instance_admins` +
  `is_instance_admin` + the provisioning UI, the `0002` forward-only migration, the rate-bucket
  eviction, the NEW check-arch tenant-scoping guard.

## DDD-numbered decisions

| # | Decision | Rationale | ADR |
|---|---|---|---|
| **DD-MWT-01** | Resolution = **hybrid**: session-carried active workspace (web, EXTEND the shipped `SessionUser` seam) + `token.workspace_id` (API, reuse-as-is). Never trust a client-supplied/path-parsed workspace id; fail-closed if none/ambiguous. | The two seams already exist; URL/host scoping would rewrite every shipped route/template and put a client-supplied id on the trust path (NFR-MWT-SEC-06). | ADR-001 |
| **DD-MWT-02** | Enforcement = an **`ActingWorkspace(Uuid)` newtype** handlers consume instead of a parsed `Uuid`, PLUS a **NEW `check-arch` LAYER-1e tenant-scoping AST rule**. A scoped-store wrapper was rejected (too large a refactor for an additive feature). | Makes "forgot to scope / trusted the client" a build-time failure, not a runtime leak — answers the top risk + NFR-MWT-SEC-06 at minimal cost. | ADR-002 |
| **DD-MWT-03** | Non-enumerability = **generalize the shipped `find_*_in_workspace → None` idiom** as the single refusal pattern; per surface ONE response (web 404 page / API shipped JSON 404 envelope); cross-tenant resource access never 403s. Timing-equivalent by construction (same query). | One shipped+tested idiom reused everywhere; no per-resource decision to get wrong; the timing equivalence is structural, not a fragile hack (NFR-MWT-SEC-02). | ADR-003 |
| **DD-MWT-04** | Instance super-admin = a **NEW `instance_admins(user_id)` table** + `is_instance_admin` (in services); provisioning via an **`/admin/instance/workspaces` web flow** (session+CSRF) calling a NEW `create_workspace` use-case that seeds the first admin via the invite idiom; bootstrap seeds workspace 1 + the first super-admin. | Cleanest minimal role (OD-3); keeps provisioning OFF the bearer surface; reuses the admin-UI + invite idioms. A `users` boolean / "workspace-1-admin" convention conflates concerns. | ADR-004 |
| **DD-MWT-05** | Sign-in selection = **single-membership auto-resolves (no prompt); multi-membership picks explicitly + a switcher re-stamps the session's active workspace; 0 memberships ⇒ refuse, never default.** Per-request membership re-check (session stays thin). Sign-in security contract unchanged. | OD-2 multi-membership; the common case stays zero-friction (and identical to every upgraded install); multi-membership is always an intentional choice (no silent default = no ambiguity hazard). | ADR-005 |
| **DD-MWT-06** | Migration = a NEW forward-only **`0002_multi_workspace.sql`**: `DROP INDEX uniq_one_workspace` + `CREATE TABLE instance_admins`; existing workspace stays workspace 1, id unchanged, zero data rewrite; idempotent on re-apply. Pre-drop un-scoped-query audit performed (see upstream). | OD-4 forward-only no-touch; dropping an index touches no row; the existing FKs already point at workspace 1; probed against a REAL pre-feature DB (US-MWT06). | ADR-006 |
| **DD-MWT-07** | **NEW `check-arch` LAYER-1e tenant-scoping rule** (EXTEND `xtask/src/check_arch.rs`) — an adapter-side tenant-scoped store call must be fed a resolved acting workspace, not a request-parsed id; one detector + one acceptance gold test (mirrors the shipped no-mint rule, Principle 11/12). | Turns the hard isolation NFR into a build-time invariant; the AST-walk infra + gold-test discipline already exist, so the cost is one detector + one test. | ADR-002 |
| **DD-MWT-08** | Rate-bucket map = **EXTEND `rate_limit.rs` with idle/LRU eviction** keyed by `user_id`; bound by ACTIVE principals; throttle semantics + 100%-mutation discipline preserved. | Residual F2; a new module would discard a hardened one — extension is mandatory (NFR-MWT-PERF-01). | (in architecture.md §4 #13) |
| **DD-MWT-09** | **OD-5 ratified as-is**: whole-instance backup/restore unchanged for v1; per-workspace export deferred. No design change to the US-03 restore machinery. | A per-workspace restore must not clobber a sibling — a meaningfully harder, isolation-sensitive feature; out of v1 scope. | (DISCUSS DM7) |
| **DD-MWT-10** | **Legacy per-feature layout** under `docs/feature/multi-workspace-tenancy/design/`; NOT the SSOT/feature-delta model; trunk-based (commit to `main`, no branch/PR). | Per the brief; 20+ prior features use the per-feature layout; SSOT migration is intentionally not triggered. | (DISCUSS DM10) |

## Reuse Analysis (verdict counts)

**REUSE/EXTEND = 14** (8 reuse-as-is: per-table `workspace_id` scoping, `is_workspace_admin`,
`is_team_member`, `MachinePrincipal`/`Principal::Machine{workspace_id}`, the invite-seeding idiom,
tower-sessions+CSRF, argon2id sign-in, the workspace-scoped token use-cases; 6 EXTEND: the
`attachments.rs` non-enumerable idiom [generalize], the `SessionUser` web resolution seam,
`first_workspace`→membership resolution, the `create_workspace` handler, the rate-bucket map,
`check-arch`). **CREATE NEW = 2** (the `instance_admins` role/`is_instance_admin` — no instance-level
role exists today, OD-3; the `0002` migration file — a migration is by definition a new file).
Full table in `architecture.md §4`. **The feature is overwhelmingly inheritance** — the schema, the
scoping, the non-enumerable idiom, the authz, AND both resolution seams already ship.

## Technology choices

**ZERO new crates.** Reuses `axum` (routing/extractors/the shipped `FromRequestParts` seam),
`sqlx` (the `0002` migration + the new membership/instance-admin queries), `askama` (the switcher +
provisioning templates), `serde`, `uuid`, `time`, `tower-sessions`, `argon2`, `jsonwebtoken` —
all already in the workspace. The rate-bucket eviction uses `std` (the existing
`Mutex<HashMap>` + the SHIPPED `state.clock` for deterministic idle-timeout tests) — no `lru`/`dashmap`
crate is added (prefer std to add nothing; a bounded idle-eviction over the shipped map is
sufficient). The new check-arch rule is a function in the existing `xtask` crate. All Rust/OSS, no
proprietary, no license question.

## Architecture enforcement (annotation for software-crafter)

Style: Modular monolith + ports-and-adapters. Language: Rust. Tool: `cargo xtask check-arch` (EXTEND).
Rules to enforce: the 5 shipped rules (api≠HTML, api≠ad-hoc-authz, api≠mint, JWT EdDSA pin,
dependency direction) stay green; ADD the tenant-scoping rule (DD-MWT-07/ADR-002) — an adapter-side
tenant-scoped store call must be fed a resolved `ActingWorkspace`, not a request-parsed id. The
acceptance gold test plants an un-scoped tenant query and asserts the guard exits non-zero.

## Constraints (inherited, all honored)

ONE binary · ONE Postgres · NO Redis · NO Node runtime · NO CDN · **ZERO new crate** · foundry-api
stays HTML-free + off `foundry_store::Store` (boundary guard LAYER 1+2 green) · the browser
auth/CSRF/session contract + the machine-token verify path (EdDSA, `jti` denylist, `iss`/`aud`) are
byte-for-byte unchanged (this feature scopes WHICH workspace, not how a credential is verified) ·
the `foundry-acceptance` suite green-before stays green-after (single-workspace = one-membership
special case of multi-workspace).

## Component decomposition ↔ the 6 DISCUSS slices (alignment confirmation)

| Slice | DISCUSS hypothesis | DESIGN components that deliver it |
|---|---|---|
| **1 — walking skeleton** | two workspaces coexist + request resolves to its workspace on one path | `0002` migration (drop guard, ADR-006) · `ActingWorkspace` newtype + `SessionUser` EXTEND (ADR-001/002) · the shipped scoped read path proven with A/B fixtures |
| **2 — web tier boundary** | shipped scoping + `is_workspace_admin` + non-enumerable lookup refuse A→B on the web | reuse #1/#3/#4, generalize the non-enumerable idiom (ADR-003) · the check-arch tenant-scoping rule lands here (ADR-002) |
| **3 — API + auth boundary** | `token.workspace_id` is authoritative; session resolves to exactly one workspace | reuse `MachinePrincipal` (#5) · ADR-005 single/multi-membership resolution + fail-closed |
| **4 — non-enumerability hardening** | foreign-id ≡ missing-id on EVERY surface, no oracle | ADR-003 uniform refusal + the adversarial matrix (behavioral counterpart to the ADR-002 guard) |
| **5 — existing-install migration** | real pre-feature DB upgrades forward-only, no loss, sessions/tokens still resolve | ADR-006 migration + ADR-005 single-membership auto-resolution; probed on a real snapshot |
| **6 — provision + prove** | operator creates an isolated tenant; boundary proven on real fixtures; bucket bounded | ADR-004 instance-admin + provisioning UI · #12 real A/B fixtures · #13 rate-bucket eviction (DD-MWT-08) |

The decomposition maps cleanly onto all six slices; roadmap (DELIVER) can proceed slice-ordered.

## ============================================================
## Open decisions for DISTILL / DELIVER (awaiting user ratification)
## ============================================================

| # | Open decision | Options | RECOMMENDED | One-line why |
|---|---|---|---|---|
| **OD-MWT-D1** | Resolution mechanism (DM1) | (a) session+token hybrid; (b) URL `/w/{slug}`; (c) host/subdomain | **(a) hybrid** | The seams already exist; (b)/(c) rewrite shipped routes/templates or need per-tenant DNS/TLS and put a client-supplied id on the trust path. |
| **OD-MWT-D2** | Enforcement seam | (a) explicit threading only; (b) scoped-store wrapper; (c) `ActingWorkspace` newtype + check-arch rule | **(c)** | Makes forgetting-to-scope a build-time failure at minimal cost; (b) is too large a refactor; (a) is convention not enforcement. |
| **OD-MWT-D3** | NEW check-arch tenant-scoping rule | (a) no static guard; (b) add LAYER-1e tenant-scoping rule | **(b) add it** | High value given the HARD isolation NFR; the AST infra + gold-test pattern already exist, so it is one detector + one test (mirrors no-mint). |
| **OD-MWT-D4** | Instance-admin representation | (a) `instance_admins` table; (b) `users.is_instance_admin` flag; (c) workspace-1-admin convention | **(a) table** | Explicit, auditable, future-proof; (b)/(c) conflate instance + workspace concerns. |
| **OD-MWT-D5** | Provisioning surface | (a) `/admin/instance/...` web flow; (b) CLI; (c) `/api/v1` endpoint | **(a) web flow** | Reuses the admin UI + invite seeding, session+CSRF; keeps creation OFF the bearer surface; (b) noted as a follow-up convenience; (c) rejected (privileged path on bearer surface). |
| **OD-MWT-D6** | Cross-tenant refusal status (per-surface) | confirm web=404-page, API=shipped JSON 404 envelope | **web 404 page / API JSON 404** | DISCUSS fixes only that it is UNIFORM + non-enumerable; this pins the concrete shapes per surface (ADR-003). |
| **OD-MWT-D7** | Multi-tab / bookmarkable per-workspace URLs | (a) out of v1 (one active workspace per session); (b) in v1 (path scoping) | **(a) out of v1** | Consistent with the session-carried mechanism; revisit with path-scoping only if demanded — avoids a route/template rewrite now. |
| **OD-MWT-D8** | Rate-bucket eviction policy | (a) idle-timeout; (b) LRU cap; (c) both | **(a) idle-timeout (std, shipped clock)** | Bounds by active principals, deterministically testable via the shipped clock, adds no crate; an LRU cap can layer on later if needed. |
| **OD-MWT-D9** | Tighten the two un-scoped reads (DESIGN finding) | (a) tighten now (add `workspace_id` scope); (b) defer | **(a) tighten now** | They are un-scoped tenant reads under multiple tenants; cheap to scope; the new guard would flag them anyway (see upstream-changes.md). |

## Upstream changes (DISCUSS assumptions challenged)

**One DISCUSS assumption is REFINED (not contradicted) — `upstream-changes.md` is written.** Assumption
#6 ("no query depends on `uniq_one_workspace`; audit for un-scoped `FROM teams|projects|issues|invites`")
holds — nothing depends on the guard — BUT the audit surfaced (1) a **hard-coded application-level
409 guard** in `create_workspace` (`bootstrap.rs:289`) that must ALSO be removed (the DISCUSS framing
implied only the DB index forbids a second workspace), and (2) **two un-scoped single-row tenant
reads** (`invites WHERE id` and `teams WHERE id`) that should be workspace-scoped before/with the
boundary work. Both refine the clean-migration claim rather than break it; details + verbatim quotes
in `upstream-changes.md`.

## Handoff to DISTILL / DELIVER

- **Acceptance-designer (DISTILL)**: the AC in `stories.md` are observable and implementation-neutral.
  New contract assertions to formalize: the uniform non-enumerable refusal matrix (web/API/revoke/admin,
  foreign-id ≡ missing-id, no timing/shape oracle — ADR-003/US-MWT05); the fail-closed resolution
  scenarios (0/1/≥2 memberships — ADR-005/US-MWT04); the migration before/after row-equality on a real
  snapshot (ADR-006/US-MWT06); the non-super-admin-cannot-provision authz path (ADR-004/US-MWT07); the
  rate-bucket eviction bound + active-principal throttle-preservation (DD-MWT-08/US-MWT08); and the NEW
  check-arch tenant-scoping rule's gold test (ADR-002).
- **Platform-architect (DEVOPS)**: **NO external integrations / no consumer-driven contract tests owed.**
  The NEW `check-arch` tenant-scoping rule joins the existing boundary-guard CI lane. No new
  observability sink beyond the existing rate metric. Migration `0002` runs in the standard migrator;
  the real-pre-feature-DB migration acceptance needs a snapshot fixture in CI.
