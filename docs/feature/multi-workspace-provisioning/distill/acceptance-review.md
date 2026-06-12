# DISTILL — Acceptance Self-Review (multi-workspace-provisioning)

> Quinn (nw-acceptance-designer), DISTILL wave. Self-review against the success criteria + the
> critique dimensions + the traceability matrix. Authoring only (no cargo, no commit).

## Success-criteria checklist (from the command)

- [x] Every inherited user story (US-MWT06/07/08) has acceptance scenarios. US-MWT06 → slice-05
      sc 1-6; US-MWT07 → slice-06 sc 1,2,5,6,7,8,9; US-MWT08 → slice-06 sc 3,4 (real
      provisioned-tenant isolation) + the `rate_limit.rs` module test (rate-bucket bound, layered
      correctly at unit/property, NOT acceptance).
- [x] Every ratified decision D1-D7 is exercised or explicitly noted as non-testable-at-this-layer.
      D1 (sc 7/8), D2 (every slice-06 provisioning scenario + sc 9 off-bearer), D3 (sc 5/6/7),
      D4 (slice-05 sc 2/3/4), D6 (slice-05 sc 5 + the `0011`-MISSING RED gate), are exercised.
      D5 (eviction) is NOTED as layer-1/2 unit/property, not acceptance. D7 (no new check-arch rule)
      is NOTED as a build-time guard (`cargo xtask check-arch`), not an acceptance `.feature`.
- [x] Scenarios exercise driving ports — the CLI provisioning port (subprocess), the in-process
      API/web + sign-in/resolution seam (isolation proof), the migration runner + store seam
      (slice 5) — NOT internal components. (CM-A.)
- [x] Walking skeleton authored first in each feature file; remaining scenarios `@pending` for
      one-at-a-time DELIVER.
- [x] No mocks at the acceptance level; `@real-io` real Postgres (the clock is the only fake, and
      only at the rate_limit layer-1/2 test, not in any acceptance scenario).
- [x] House-style header + banner-comment convention matches the slice-03 exemplar (long header
      block: hypothesis + what disproves it; driving adapter with concrete entry points; driven
      adapters @real-io; refusal/non-enumerability decision; explicit RED-state contract — COMPILES
      ⇒ not BROKEN, genuine RED is MISSING_FUNCTIONALITY enumerated; explicit scope in/out; then
      `@feature @slice @real-io @driving_adapter` tags, Feature narrative, shared Background,
      numbered scenarios each with a `# ---` banner; first scenario `@walking_skeleton @wiring_e2e`,
      rest `@pending`).
- [x] Honours both upstream findings — CLI-first (every provisioning scenario drives
      `foundry doctor provision-workspace`; web flow explicitly OUT); the 409 guard present (header
      states the `bootstrap.rs:301` 409 is the deferred-web-flow EXTEND point and is NOT exercised;
      no scenario assumes it is gone; the parent docs are NOT modified).
- [x] No cargo builds / migrations run; authoring only; not committed. `acceptance.rs` NOT edited;
      no step glue authored (DELIVER's job); the crate still COMPILES (feature files are Gherkin
      text and do not affect compilation).

## Critique dimensions (Sentinel's 9, self-applied)

- **Dim 1 (happy-path bias)**: 7 of 15 scenarios are error/sad/evil-user/regression (47%) ≥ 40%.
  PASS. Covers: non-super-admin refused, non-enumerable authz, cross-tenant refusal, off-bearer
  refusal, idempotent re-apply, regression-unchanged.
- **Dim 2 (GWT compliance)**: every scenario has Given (context, mostly in shared Background)/When
  (single action)/Then (observable outcome). The two-`When` evil-user scenarios (slice-06 sc 4/6,
  slice-03-exemplar pattern) pair a real-target probe with a never-existed probe to assert
  indistinguishability — the established repo idiom for non-enumerability, not a multi-behaviour
  smell. PASS.
- **Dim 3 (business-language purity)**: scanned all titles + steps. No HTTP verbs, status codes,
  table names, SQL, `is_instance_admin`, `provision_workspace`, or `instance_admins` in any scenario
  title or Gherkin step (those appear ONLY in the header comment + this review, never in executable
  steps). Domain terms only: super-admin, provision, workspace, sign in, isolated, refused, invite
  link, upgrade. PASS.
- **Dim 4 (coverage completeness)**: every inherited story mapped (matrix in test-scenarios.md);
  every ratified decision exercised or layer-noted. PASS.
- **Dim 5 (walking-skeleton user-centricity)**: both WS titles are user goals ("A super-admin
  provisions a new isolated workspace with a first admin"; "Upgrading a single-workspace install
  keeps it working as workspace 1"), not layer-connectivity ("end-to-end through all layers"). Then
  steps are user observations (new admin signs in and acts; users work exactly as before), not
  internal side effects (no "row inserted in `instance_admins`"). PASS.
- **Dim 6 (priority)**: the two slices are the explicitly-deferred residual of a shipped milestone;
  the headline WS (CLI provisioning) is the largest user-value gap (mwt-job-4). PASS.
- **Dim 7 (observable-behaviour assertions)**: every Then asserts a port-exposed observable — CLI
  reports id + invite link; new admin signs in and acts; responses refused identically; tenant rows
  present/unchanged; workspace identity unchanged; refused as not authorized; no new workspace
  created. None assert internal state / private fields / method-call counts. (The "active-workspace
  choice remains unwritten" in slice-05 sc 4 is the port-observable resolution behaviour — an
  upgraded user resolves to ws1 with no value written, observed via the resolution seam, NOT a raw
  column read — DELIVER's step glue reads it through `resolve_active_workspace`, not a private
  field.) PASS.
- **Dim 8 (traceability)**: Check A — every story ID (US-MWT06/07/08) has ≥1 scenario tagged with
  it. PASS. Check B — no per-feature `devops/environments.yaml` (inherited); the upgrade scenarios
  carry an explicit pre-feature-install Given (the only environment variant that matters for this
  feature: clean fresh-claim vs upgraded install — both covered, slice-06 sc 7 fresh-claim, sc 8
  upgraded). PASS.
- **Dim 9 (walking-skeleton boundary proof)**: 9a strategy declared (inherited Infrastructure
  Policy + walking-skeleton.md). 9b both WS are `@real-io @wiring_e2e`, real adapters, no
  `@in-memory`. 9c every NEW driven adapter has real-I/O coverage (the `provision-workspace` CLI →
  real subprocess + real PG; the migration runner → real PG; `instance_admins`/`is_instance_admin`
  → real PG rows exercised by sc 5/6/7). 9d deletion test: if the real `provision_workspace` tx were
  deleted, slice-06 sc 1 would fail (no workspace created, new admin cannot sign in) — it tests
  wiring, not fixtures. 9e no `@in-memory` on any WS. PASS.

## Traceability matrix (story → scenario; decision → scenario)

### Story → scenario

| Story | Scenarios |
|---|---|
| US-MWT06 | slice-05 sc 1, 2, 3, 4, 5, 6 |
| US-MWT07 | slice-06 sc 1, 2, 5, 6, 7, 8, 9 |
| US-MWT08 | slice-06 sc 3, 4 (real provisioned-tenant isolation); rate-bucket bound = `rate_limit.rs` unit/property (layer 1-2, DELIVER) |

### Decision → assertion (every ratified decision has ≥1 assertion or a layer note)

| Decision | Assertion |
|---|---|
| D1 | slice-06 sc 7 (claim ⇒ first super-admin can provision), sc 8 (grant idempotent on upgrade) |
| D2 | every slice-06 provisioning scenario drives the CLI; sc 9 asserts provisioning off the bearer surface; web OUT |
| D3 | slice-06 sc 5 (non-super-admin refused fail-closed), sc 6 (refusal non-enumerable), sc 7 (super-admin allowed) |
| D4 | slice-05 sc 2 (row equality), sc 4 (active workspace UNWRITTEN — no-backfill made observable), sc 3 (carried session/token resolves) |
| D5 | LAYER NOTE — `rate_limit.rs` unit/property (MockClock idle-window + bounded-map + behaviour-preserving); not acceptance (mutates in-memory map, not port-observable DB state) |
| D6 | slice-05 sc 5 (idempotent re-apply of the additive `0011`), slice-06 sc 7/8 (table seeded); the `0011`-MISSING RED gate threads every scenario |
| D7 | LAYER NOTE — build-time `cargo xtask check-arch` guard (admin_cli + bootstrap already allow-listed; `is_instance_admin` non-tenant-scoped); not an acceptance `.feature` |

## Open decision points / ambiguities for the user (confirm before DELIVER)

1. **Rate-bucket eviction layer placement (D5).** I deliberately did NOT author a `@real-io`
   acceptance feature for the eviction. The two claims (bounded map; behaviour-preserving for active
   principals) are correctly layer-1/2 unit/property tests at `crates/foundry-app/src/rate_limit.rs`
   (MockClock-driven), per ADR-005 + the Layered Test Discipline table — an HTTP round-trip cannot
   observe `HashMap` size. The command suggested "if cohesion warrants, a separate eviction feature";
   my judgement is it does NOT warrant an acceptance feature (it would mis-layer). **Confirm** you
   accept the eviction as DELIVER-owned module tests, not an acceptance `.feature`.

2. **Two walking skeletons (one per slice/feature file), not one.** The feature spans two distinct
   driving surfaces (migration runner; operator CLI) with two distinct riskiest assumptions, so each
   `.feature` carries its own `@walking_skeleton @wiring_e2e` first scenario per the repo convention.
   The OVERALL headline demo is slice-06 sc 1. **Confirm** two WS is acceptable (vs forcing a single
   one, which would drop either the migration-safety or the provisioning proof).

3. **Slice-5 "pre-feature snapshot" mechanism.** The proof stages the pre-`0009` migration history
   (`0001`..`0008`) in a `TestMigrationsDir` tempdir, seeds representative tenant data via the real
   `Store`, then applies the canonical `0009/0010/0011`. An alternative is restoring a stored real
   `pg_dump` of a genuine pre-feature instance (ADR-004 option c mentions "or restore a real
   pre-feature dump"). I chose the staged-migration-history approach because it reuses the SHIPPED
   `support/test_migration.rs` precedent and is deterministic/CI-portable; a stored dump adds a
   binary fixture to maintain. **Confirm** the staged-history approach (vs a committed dump fixture)
   for the DELIVER harness.

4. **CLI exit-code contract for the unauthorized-provisioning refusal.** Slice-06 sc 5/6 assert "not
   authorized" via a distinct non-zero exit code (mirroring `run_restore_comment`'s structured
   exit-code discipline: 0 ok / 2 bad-arg / 3 DB-fail / 4 not-found). DELIVER owns the exact code,
   but the non-enumerability contract (sc 6: an existing-name attempt and a never-existed-name
   attempt are refused IDENTICALLY) constrains it: the "not authorized" outcome MUST be observably
   independent of whether the target exists. **Confirm** the structured-exit-code refusal (vs a free
   text stderr) is the intended non-enumerable authz surface for the CLI.

## Handoff readiness

- Feature files: 2, both house-style, both `@real-io`, 15 scenarios, 2 `@walking_skeleton`, 13
  `@pending`. Crate COMPILES (no step glue, no `acceptance.rs` edit).
- Infrastructure policy: 4 rows appended (`inherit` mode).
- RED-state contract declared per scenario (test-scenarios.md). DELIVER runs the
  fail-for-right-reason gate at RED-phase entry, then unskips one `@pending` per RED→GREEN→COMMIT
  cycle, authoring the step glue + force-linking the new step modules in `acceptance.rs`.
- Blocking on the 4 confirmations above is NOT required (they are judgement calls I made with
  documented rationale); surfaced so the user can redirect before DELIVER if any differs from intent.
