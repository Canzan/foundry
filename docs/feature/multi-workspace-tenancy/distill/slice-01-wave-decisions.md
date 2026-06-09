# Multi-Workspace Tenancy — Slice 1 (Walking Skeleton) DISTILL Wave Decisions

> Sentinel (nw-acceptance-designer), DISTILL wave, SLICE 1 ONLY (the walking
> skeleton: two workspaces coexist + request→workspace resolution). Legacy
> per-feature layout; trunk-based (commit to `main`, no branch/PR). Scenarios for
> Slices 2-6 are NOT authored here.

## Reconciliation HARD GATE result

**Reconciliation passed — 0 contradictions.**

Read: `discuss/wave-decisions.md` (ratified OD-2 multi-membership, OD-3 instance
super-admin), `discuss/stories.md` (US-MWT00 infra-folded + US-MWT01),
`slices/slice-01-walking-skeleton-coexist.md`, `design/wave-decisions.md`,
`design/adr-001/002/006`, `design/upstream-changes.md`.

| DISCUSS decision | DESIGN position | Verdict |
|---|---|---|
| OD-1 shared-schema + `workspace_id` | DD-MWT-03 / ADR-003 ratify shared-schema | consistent |
| OD-2 multi-membership (RATIFIED) | DD-MWT-05 / ADR-001/005 session-active-workspace + membership resolution | consistent |
| OD-3 instance super-admin (RATIFIED) | DD-MWT-04 / ADR-004 `instance_admins` (Slice 6, out of slice-1 scope) | consistent |
| OD-4 forward-only no-touch migration | DD-MWT-06 / ADR-006 `0002` drops index + adds table, no data rewrite | consistent |
| DM1 "drop the single-workspace guard" (framed singular) | upstream Finding 1: ALSO drop the app-level 409 at `bootstrap.rs:289` | **REFINEMENT** — upstream-changes.md explicitly "no story changes"; the 409-removal is Slice 6 provisioning, and slice-1 seeding inserts the workspace row directly via sqlx (the 409 guard is not on that path), so the slice-1 RED edge is the DB index only |
| Assumption #6 "no un-scoped reads" | upstream Finding 2: two un-scoped reads (`invites WHERE id`, `teams WHERE id`) | **REFINEMENT** — both folded into Slice 2/6, NOT slice 1 |

Both upstream findings are documented refinements (no ratified OD changed, no
story changed). No DISCUSS↔DESIGN↔DEVOPS opposition. Gate passed.

## Chosen read path + justification

**`GET /api/v1/teams/{team}/projects/{project}/issues`** (the JSON API, machine
bearer), authenticated by the SHIPPED `MachinePrincipal` extractor whose
`token.workspace_id` is the acting workspace (ADR-001 API leg).

Why this path (least new code, most load-bearing):
- The resolution seam for the API leg is `token.workspace_id` — **already
  shipped** (`foundry-api/src/lib.rs:583`, `Principal::Machine{workspace_id}`).
- The per-table `workspace_id` issue scoping is **already shipped**
  (`foundry-store`).
- The route + the `mint_bearer(world, user_id, workspace_id, …)` helper bound to
  a workspace are **already shipped** (Feature-A US-W05a, token-management-api).
- So the ONLY genuinely-new production surface this slice's skeleton requires is
  the `0002_multi_workspace.sql` migration that drops `uniq_one_workspace`. Two
  machine tokens bound to Acme vs Globex hitting the issues endpoint exercise the
  resolution seam end-to-end with mostly-shipped machinery.

Why NOT the web board path: the session leg (`SessionUser` active-workspace
EXTEND + the multi-membership switcher, ADR-005) is genuinely new surface and is
explicitly **Slice 3**. Using it for the skeleton would pull Slice-3 work
forward. The API leg is the smaller, more load-bearing proof.

## Tier classification

**Tier A only.** LAYER 3 (real Postgres via testcontainers + real HTTP via
`spawn_app`/`reqwest`). Per Mandates 9 + 11: example-based, sad path enumerated
explicitly, NO PBT machinery. Per Mandate 10: Tier B (state-machine PBT,
in-memory doubles) is NOT added — the journey runs at layer 3 with real I/O, and
the slice is the thin coexistence proof, not a domain-rich ≥3-chained in-memory
journey. Per Mandate 8: layer-3 uses traditional assertions over port-exposed
observables (listed issue keys, HTTP refusal status, workspace row count) — the
state-delta universe-guard is the layers 1-3 requirement satisfied by traditional
port-observable assertions at this layer per the Layered Test Discipline table.

## Scenario list + tags

File: `crates/foundry-acceptance/tests/features/us-mwt-slice-01-coexist.feature`

| # | Scenario | Story | Tags | Class |
|---|----------|-------|------|-------|
| 1 | A member of one workspace lists only their own workspace's issues | US-MWT01 | `@walking_skeleton @wiring_e2e @us-mwt01` | happy (core hypothesis) |
| 2 | Each workspace's members see a disjoint set of data | US-MWT01 | `@us-mwt01 @pending` | happy |
| 3 | A second workspace can be created where none could before | US-MWT00 | `@us-mwt00 @coexistence @pending` | coexistence (guard gone) |
| 4 | A brand-new workspace starts empty, not populated from a neighbour | US-MWT01 | `@us-mwt01 @pending` | edge (empty state) |
| 5 | A request that resolves to no workspace is refused, not defaulted | US-MWT00 | `@us-mwt00 @error @pending` | error (fail-closed) |
| 6 | Dropping the guard leaves the existing workspace's data unchanged | US-MWT00 | `@us-mwt00 @migration @no-rewrite @pending` | no-rewrite |

Feature-level tags: `@multi-workspace-tenancy @mwt-slice-01 @real-io @driving_adapter`.

- **Error/edge ratio**: 3 of 6 (scenarios 4, 5, 6 are edge/error/safety) = 50%
  (exceeds the 40% bar).
- **Story coverage**: US-MWT00 (scenarios 3, 5, 6) + US-MWT01 (scenarios 1, 2,
  4) — both slice-1 stories covered.
- **Walking skeleton**: exactly ONE (`@walking_skeleton`, scenario 1) — the
  core hypothesis, demo-able: "a member of Acme sees only Acme's issues."

## Adapter coverage table (Mandate 6)

| Driven adapter | `@real-io` scenario | Covered by |
|---|---|---|
| Postgres `workspaces` (the coexistence row) | YES | scenarios 1-6 (real two-workspace insert; scenario 3 asserts row count ≥ 2) |
| Postgres `issues` per-table `workspace_id` scoping | YES | scenarios 1, 2, 4 (real issues, workspace-scoped list) |
| `machine_tokens` registry + EdDSA verify (the `token.workspace_id` resolution seam) | YES | scenarios 1, 2, 4, 5 (real bearer minted bound to a workspace) |
| The `0002` forward-only migration (drop `uniq_one_workspace`) | YES | scenarios 3, 6 (second workspace insert; no-rewrite before/after) |
| JSON API driving adapter (`GET /api/v1/.../issues` over real HTTP) | YES | scenarios 1, 2, 4, 5 (driving-adapter coverage per RCA-fix P1) |

Zero `NO — MISSING` rows. All driven adapters in slice-1 scope are exercised
with real I/O. Mechanism per the Project Infrastructure Policy
(`docs/architecture/atdd-infrastructure-policy.md`) — all ports already recorded
(HTTP API via `spawn_app`/`reqwest`; PgPool via testcontainers + per-scenario
schema; EdDSA fixed test keypair). No policy rows added this run.

## NEW steps/fixtures vs reused

**NEW (this slice's two-workspace fixture — the heart of the walking-skeleton work):**
- `workspace "<name>" exists with admin "<email>"` — ADDITIVE workspace seed (does
  NOT reset the harness). The first fixture in the repo that holds >1 workspace.
  The SECOND invocation's `INSERT INTO workspaces` is the RED edge (fails on
  `uniq_one_workspace` until `0002`).
- `"<ws>" has a member "<email>" in team "<team>" with project "<project>" prefix "<prefix>"`
  — workspace-scoped member+team+project seed (name lookups alone are ambiguous
  across tenants, so the route is recorded workspace-scoped).
- `the "<ws>" project "<project>" has issues <KEY> and <KEY>` — workspace-scoped
  issue seed (resolves the project WITHIN the workspace).
- `a machine credential is bound to "<email>" in workspace "<ws>"` — mints a REAL
  EdDSA bearer bound to `(user, workspace)` (the resolution seam).
- `a credential whose holder belongs to no workspace` — the fail-closed edge.
- When: `the Acme-bound credential lists …`, `the Globex-bound credential lists …`,
  `that credential lists …`, `the workspace "<name>" is created alongside it`,
  `the single-workspace guard is dropped …`.
- Then: disjoint-set / empty / refused / both-exist / identity-unchanged /
  issues-unchanged assertions.
- World fields: `mwt_workspace_ids`, `mwt_project_route`, `mwt_bearer_by_email`,
  `mwt_no_workspace_bearer`, `mwt_issues_before_by_workspace`,
  `mwt_workspace_id_before`, `mwt_last_status`, `mwt_last_body`,
  `mwt_acme_answer`, `mwt_globex_answer`.

**REUSED (verbatim machinery, not re-implemented):**
- `InProcHarness::spawn` + per-scenario schema + the shared testcontainers PG16
  container (`support::harness`).
- `harness.base_url()` + `reqwest` client pattern (mirrors `feature_a_programmatic`).
- `Store::insert_machine_token(jti, user_id, workspace_id, …)` +
  `foundry_auth::test_keys::signer()` + `MachineTokenClaims` (mirrors
  `feature_token_management_api::mint_bearer`).
- `foundry_auth::hash_password`, the `users`/`workspace_memberships`/`teams`/
  `projects`/`issues` insert idioms (mirror us_06/us_07/us_08 — but additive +
  workspace-scoped).

Deliberately NOT reused: the single-workspace `a workspace "…" exists with admin
"…"` (us_06) — it RESETS the harness to one workspace, incompatible with seeding
two. New unique text seeds additively.

## Scaffold inventory (Mandate 7 / RED-ready)

- `.feature`: `crates/foundry-acceptance/tests/features/us-mwt-slice-01-coexist.feature`
  (6 scenarios; #1 active `@walking_skeleton`, #2-6 `@pending` per one-at-a-time).
- Steps: `crates/foundry-acceptance/src/steps/feature_mwt_slice_01_coexist.rs`
  (registered in `src/lib.rs` + force-linked via `use … as _feature_mwt_s01` in
  `tests/acceptance.rs`).
- World fields added to `src/world.rs`.
- **No production-source scaffold stub needed**: the production surface this
  slice requires (the `0002` migration dropping `uniq_one_workspace`) is a
  migration file DELIVER authors; there is no Rust module to stub. The crate
  COMPILES clean (`cargo test -p foundry-acceptance --no-run` → Finished), so the
  test is RED-not-BROKEN by construction.

## Test placement + precedent

`crates/foundry-acceptance/tests/features/*.feature` + `src/steps/*.rs` — matches
EVERY prior feature (us-w05a, us-tma, us-mt0x, us-0x). Feature filename
`us-mwt-slice-01-coexist.feature` mirrors the slice-named precedent.

## Pre-requisites DELIVER must build (to flip RED → GREEN)

1. **`0002_multi_workspace.sql`** (ADR-006): `DROP INDEX uniq_one_workspace;` so a
   second `workspaces` row can exist. (`CREATE TABLE instance_admins` is also in
   `0002` per ADR-006 but is Slice-6-consumed; harmless here.)
2. **Resolution seam (API leg)**: confirm `token.workspace_id` →
   `ActingWorkspace` newtype (ADR-001/002) feeds the issue-list query; refuse
   fail-closed (401/403/404) when the holder resolves to no workspace.
3. The shipped `GET /api/v1/.../issues` handler scoped by the acting workspace
   (already ships scoped; this slice proves it under two tenants).

NOTE (upstream-changes Finding 1): the application-level 409 in `create_workspace`
(`bootstrap.rs:289`) is removed as part of Slice-6 provisioning, NOT slice 1.
Slice-1 seeding inserts the workspace row directly via sqlx and never hits that
guard, so dropping the DB index alone makes the slice-1 seeding GREEN.

## Fail-for-the-right-reason expectation

The crate compiles clean (no import/collection error → not BROKEN). At runtime,
against the real testcontainers PG16:
- The `@walking_skeleton` scenario's Background seeds workspace "Acme" then
  workspace "Globex"; the SECOND `INSERT INTO workspaces` FAILS on
  `uniq_one_workspace` (0001_init.sql:15). This is the genuine RED:
  **MISSING_FUNCTIONALITY** ("a second workspace cannot exist yet"), not a fixture
  bug. Once DELIVER ships `0002` dropping the index, the second insert succeeds
  and the isolation assertions (Acme sees only ACME-*, no GLOBEX-* leak) become
  the behaviour under test.
- The fail-closed scenario (5) expects a 401/403/404 refusal once resolution is
  wired; until then the second-workspace seed already reds it for the same reason.

This is the RED-phase entry signal DELIVER reads at PREPARE (ADR-025 D2).
