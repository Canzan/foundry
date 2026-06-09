# Story Map: multi-workspace-tenancy

## User: Sasha Okonkwo (instance operator) + Marco/Priya (tenant members/admins) + Dana (security reviewer)
## Goal: host several independent teams in one Foundry instance with genuine, provable per-tenant isolation

## Backbone (user activities, chronological left-to-right)

| Enable many tenants | Resolve & scope a request | Enforce the boundary per surface | Harden non-enumerability | Migrate existing install | Provision & prove |
|---------------------|---------------------------|----------------------------------|--------------------------|--------------------------|-------------------|
| Drop `uniq_one_workspace` (US-MWT00) | Request → workspace resolution (US-MWT00/01) | Web htmx tier scoped + authz (US-MWT02) | Uniform refusal everywhere (US-MWT05) | Existing ws → workspace 1 (US-MWT06) | Create new tenant + first admin (US-MWT07) |
| Two workspaces coexist (US-MWT01) | Act only on acting workspace (US-MWT01) | JSON `/api/v1` + machine principals scoped (US-MWT03) | No existence oracle (US-MWT05) | No data loss / sessions intact (US-MWT06) | Real A/B fixtures + rate-bucket eviction (US-MWT08) |
| | | Sign-in/session resolution (US-MWT04) | Adversarial coverage (US-MWT05) | | |

---

### Walking Skeleton (thinnest end-to-end slice — Slice 1)
The minimum that connects ALL activities for ONE path: **drop `uniq_one_workspace`** +
**resolve a request to its workspace** + **two real workspaces coexist and a single read path
returns only the acting workspace's data**. This is the abstraction every later slice depends
on, so it ships FIRST (carpaccio taste-test: ship the shared abstraction first).
- US-MWT00 (`@infrastructure`, folded) — drop the guard + the resolution seam.
- US-MWT01 — two workspaces coexist; a request resolves to ITS workspace; ONE read path proven
  isolated end-to-end with real A/B data.

### Release / Slice 2: Boundary proven on the WEB htmx tier — Outcome: a member of A cannot reach B on the web
- US-MWT02 — tenant-scoped reads/writes + per-tenant authz + non-enumerable refusal on the web
  htmx tier, driven by REAL A/B fixtures. Targets mwt-job-2 (KPI: 0 cross-tenant web leaks).

### Release / Slice 3: Boundary propagated to API + auth surfaces — Outcome: the same boundary holds for tokens and sessions
- US-MWT03 — `/api/v1` + machine-token principals act only on the token's bound workspace;
  cross-tenant bearer call refused. Targets mwt-job-2.
- US-MWT04 — sign-in/session resolution yields exactly one workspace, fail-closed; multi-
  membership selection is explicit (OD-2). Targets mwt-job-2.

### Release / Slice 4: Non-enumerability hardened — Outcome: B's existence is invisible from A on every surface
- US-MWT05 — uniform non-enumerable refusal across every surface; no existence oracle;
  adversarial coverage. Targets mwt-job-2 (KPI: 0 existence-leak oracles).

### Release / Slice 5: Existing install migrated — Outcome: every existing single-workspace install upgrades safely
- US-MWT06 — existing workspace becomes workspace 1; forward-only; no data loss; sessions/tokens
  keep working. Targets mwt-job-3 (KPI: 0 data loss; existing auth suites green).

### Release / Slice 6: Provision + prove — Outcome: an operator onboards a tenant and the boundary is provable; residuals closed
- US-MWT07 — operator creates a new workspace + seeds its first admin. Targets mwt-job-4.
- US-MWT08 — real two-workspace fixtures replace synthetic uuids; rate-bucket map eviction.
  Targets mwt-job-5 (closes the two accepted residuals).

---

## Priority Rationale

Priority follows (1) walking-skeleton-first, (2) riskiest-assumption-first, (3) outcome impact /
dependency chain — per the user-story-mapping skill's tie-breaking (Walking Skeleton > Riskiest
Assumption > Highest Value).

| Priority | Slice | Why this order |
|----------|-------|----------------|
| **P1** | Slice 1 (skeleton) | The riskiest, most load-bearing assumption: that two workspaces can coexist and a request resolves to exactly one. Everything downstream depends on the resolution seam, so it ships first and de-risks the whole feature. If it fails, the feature is reframed before any surface work. |
| **P2** | Slice 2 (web boundary) | Highest-leverage proof of the security core (mwt-job-2) on the surface with the most read/write paths. Proves the isolation contract concretely with real A/B fixtures on ONE surface before propagating — cheaper to find a scoping gap here than across all surfaces at once. |
| **P3** | Slice 3 (API + auth) | Propagates the now-proven boundary to the machine-token + session surfaces. Depends on Slice 1 (resolution) and reuses Slice 2's contract; the bearer path is the second-highest-risk surface (a token bound to A is a credential that must not reach B). |
| **P4** | Slice 4 (non-enumerability) | Hardening across every surface; depends on Slices 2-3 existing so the refusal can be made uniform. High security value, lower novelty (generalizes the shipped attachments pattern). |
| **P5** | Slice 5 (migration) | High-stakes but well-bounded (forward-only, drop the guard). Sequenced after the boundary is proven so the upgrade lands into a system whose isolation is already trustworthy; can also be validated independently against a real pre-feature DB. |
| **P6** | Slice 6 (provision + prove + residuals) | The operator-facing onboarding + the residual closure. Lowest risk (provisioning reuses the bootstrap idiom; residuals are bounded refactors), highest dependency count — naturally last; it also delivers the real A/B fixtures that retroactively strengthen Slices 2-5's evidence. |

> **Note**: US-MWT0x IDs are assigned here (Phase 4 / requirements) and mirrored in `jobs.yaml`
> `story_job_map`. Each story traces to a job and an outcome KPI (`outcome-kpis.md`). No orphan
> stories: US-MWT00 is `@infrastructure` and folds into Slice 1 (never ships standalone), so
> Slice 1's value story is US-MWT01.
