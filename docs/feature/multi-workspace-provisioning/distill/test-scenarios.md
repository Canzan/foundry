# DISTILL — Test Scenario Catalog (multi-workspace-provisioning)

> Quinn (nw-acceptance-designer), DISTILL wave. The deferred slices 5-6 of the shipped
> `multi-workspace-tenancy` isolation core, now their own feature. Requirements INHERITED
> (US-MWT06/07/08); DESIGN COMPLETE + RATIFIED (D1-D7, ADR-001..005). Legacy per-feature layout.
> Trunk-based; authoring only (no cargo builds, no migrations, no commit).
>
> Test framework: **cucumber-rs**. Feature files: `crates/foundry-acceptance/tests/features/*.feature`.
> Harness: `crates/foundry-acceptance/tests/acceptance.rs` (`harness = false`) — UNCHANGED by this
> wave (step glue is DELIVER's job; feature files are Gherkin text and do not affect compilation).
> Integration approach: **real services** — real Postgres via testcontainers + per-scenario schema;
> in-process axum router (the slices 1-4 `InProcHarness`) for HTTP; real `foundry` subprocess
> (`assert_cmd`) for the operator CLI. NO mocks at the acceptance level (the clock is the only fake,
> per the inherited infrastructure policy).

## Phase logs (Phase 0 + 1.5)

- `[lang-mode] rust` — workspace `Cargo.toml` is the project marker; cucumber-rs / proptest matrix row.
- `[policy-mode] inherit` — `docs/architecture/atdd-infrastructure-policy.md` exists; applied recorded
  decisions and APPENDED four new rows (provision-workspace CLI; `instance_admins` authz; pre-feature
  migration snapshot; rate-bucket eviction clock).
- `[port-mode] inherit` — no `tests/common/state_delta.rs`. Per the slice-03 exemplar's own note and
  the project precedent, NO Rust state-delta port exists (Python is the canonical pilot). LAYER-3
  acceptance uses traditional assertions over port-exposed observables (matching slices 1-4). No
  bootstrap performed — consistent with the shipped convention; Mandate 8's universe-guard is a
  layers 1-3 requirement satisfied at the Rust layer by explicit port-exposed assertions.
- **Reconciliation HARD GATE: PASSED — 0 contradictions.** No DISCUSS/DEVOPS dir for this feature
  (requirements inherited from the parent). The DESIGN wave-decisions (D1-D7) are ratified and
  internally consistent; both upstream findings (CLI-first revising parent ADR-004; the 409 guard
  still present) are recorded and honoured, not contradicted. Parent DISCUSS OD-3 (instance
  super-admin only, no self-serve) aligns with DESIGN D1/D2/D3. Nothing to block on.

## Deliverable feature files

| File | Slice | Story | Scenarios | Tags |
|---|---|---|---|---|
| `us-mwt-slice-05-migration-guarantee.feature` | 5 | US-MWT06 | 6 | `@multi-workspace-provisioning @mwt-slice-05 @real-io @driving_adapter`; 1 `@walking_skeleton @wiring_e2e`, 5 `@pending` |
| `us-mwt-slice-06-provision-and-prove.feature` | 6a + US-MWT08 isolation leg | US-MWT07 / US-MWT08 | 9 | `@multi-workspace-provisioning @mwt-slice-06 @real-io @driving_adapter`; 1 `@walking_skeleton @wiring_e2e`, 8 `@pending` |

**Total acceptance scenarios: 15** (2 `@walking_skeleton`, 13 `@pending`).
**Error / sad / evil-user / regression scenarios: 7** (`#5,#6` slice-05; `#4,#5,#6,#9` slice-06 +
`#6` slice-05 regression) ⇒ **47% error/edge ratio** (target ≥40% met).

> Note on one-`@walking_skeleton`-per-feature: the repo convention (and the slice-03 exemplar) is one
> `@walking_skeleton @wiring_e2e` first scenario PER `.feature` file. Both feature files carry one.
> The OVERALL headline demo (the single thinnest end-to-end cut for stakeholders) is slice-06 sc 1
> (CLI provisioning of a real isolated tenant) — see `walking-skeleton.md`.

## Slice 6b — rate-bucket eviction is NOT an acceptance scenario (layer placement)

US-MWT08's second half (NFR-MWT-PERF-01 / residual F2: bound the per-principal `RevokeRateLimiter`
map under many tenants, behaviour-preserving) is **deliberately NOT authored as a `@real-io`
acceptance feature.** Per ADR-005 + the Layered Test Discipline table:

- The eviction is pure-arithmetic over the SHIPPED clock seam (`MockClock`), local to one module
  (`crates/foundry-app/src/rate_limit.rs`). It mutates an in-memory `HashMap`, not user-observable
  Postgres state through a driving port.
- The two claims to prove — (i) the map stays bounded under many idle+active principals, and
  (ii) an ACTIVE principal's throttle is byte-identical with/without eviction — are **layer-1/2
  unit/property tests** (PBT full is permitted here, Mandate 9), driven by advancing the `MockClock`
  past the idle window `W = ceil(C/R)s`. Authoring them as slow `@real-io` acceptance scenarios would
  mis-layer them (an HTTP round-trip cannot observe `HashMap` size; the assertion universe is the map
  itself).
- **DELIVER owns these as `rate_limit` module tests**, mirroring the existing 100%-mutation harness.
  This catalog records the contract; the tests live beside the code under test. CM-G (Tier B
  state-machine PBT) is NOT triggered for this feature (no ≥3-chained-scenario rich-input journey at
  the acceptance layer); the eviction PBT is module-level, not acceptance Tier B.

## Scenario catalog — slice 05 (US-MWT06, migration guarantee)

| # | Scenario | Tags | Drives | RED-state (genuine RED reason) |
|---|---|---|---|---|
| 1 | Upgrading a single-workspace install keeps it working as workspace 1 | `@walking_skeleton @wiring_e2e @us-mwt06` | migration runner → sign-in + `resolve_active_workspace` | `0011` + snapshot harness MISSING → upgrade/proof unbuilt |
| 2 | No tenant data is lost or changed by the upgrade | `@pending @us-mwt06` | snapshot harness over real PG | before/after-equality harness MISSING (NEW test infra) |
| 3 | Existing sessions and machine tokens still resolve after the upgrade | `@pending @us-mwt06` | `resolve_active_workspace` + verify path | snapshot harness MISSING; resolution green-by-inheritance once built |
| 4 | An upgraded user resolves to workspace 1 without their active workspace being written | `@pending @us-mwt06` | `resolve_active_workspace` (NULL-active path) | snapshot harness MISSING; D4 no-backfill made observable |
| 5 | Re-running the upgrade does not duplicate or alter anything | `@pending @us-mwt06 @error` | migration runner (idempotent re-apply) | `0011` MISSING; `IF EXISTS`/`IF NOT EXISTS` idempotence unproven until built |
| 6 | Existing sign-in and workspace behaviour is unchanged after the upgrade | `@pending @us-mwt06 @regression` | sign-in + scoped reads | snapshot harness MISSING; NFR-MWT-REL-02 regression proof |

## Scenario catalog — slice 06 (US-MWT07 + US-MWT08 isolation leg)

| # | Scenario | Tags | Drives | RED-state (genuine RED reason) |
|---|---|---|---|---|
| 1 | A super-admin provisions a new isolated workspace with a first admin | `@walking_skeleton @wiring_e2e @us-mwt07` | `provision-workspace` CLI subprocess → sign-in | CLI subcommand + `provision_workspace` tx + `0011` MISSING |
| 2 | Provisioning a new workspace leaves existing workspaces untouched | `@pending @us-mwt07` | CLI + Acme snapshot | provisioning MISSING; NFR-MWT-REL-01 untouched-A proof |
| 3 | The provisioned workspace is a real coexisting tenant that sees only its own data | `@pending @us-mwt07 @us-mwt08` | CLI + scoped reads | provisioning MISSING; isolation green-by-inheritance once provisionable |
| 4 | A member of the existing workspace cannot reach the provisioned one non-enumerably | `@pending @us-mwt07 @us-mwt08 @error` | scoped read + uniform-404 | provisioning MISSING; SHIPPED uniform-404 extended to new tenant |
| 5 | A non-super-admin cannot provision a workspace | `@pending @us-mwt07 @error` | CLI + `is_instance_admin` (fail-closed) | `is_instance_admin` authz + CLI MISSING |
| 6 | An unauthorized provisioning attempt does not reveal whether the target exists | `@pending @us-mwt07 @error` | CLI authz refusal (non-enumerable) | authz refusal envelope MISSING; existence-oracle-free refusal unproven |
| 7 | The bootstrap-claiming operator is the first super-admin and can provision | `@pending @us-mwt07` | bootstrap claim seed + CLI | first-super-admin seed (D1) + `instance_admins` MISSING |
| 8 | An upgraded install grants its first super-admin and can then provision | `@pending @us-mwt07` | `grant-super-admin` CLI (idempotent) | `grant-super-admin` subcommand + `ON CONFLICT DO NOTHING` MISSING |
| 9 | Provisioning is unreachable from the bearer API surface | `@pending @us-mwt07 @error @verify-path-unchanged` | `/api/v1` (api≠mint) | provisioning-off-bearer invariant unproven until provisioning exists |

## Mapping to ratified decisions D1-D7 (every decision exercised or noted)

| Decision | Exercised by | How |
|---|---|---|
| **D1** first super-admin = bootstrap-claiming operator; upgraded installs grant | slice-06 sc 7 (bootstrap claim ⇒ first super-admin), sc 8 (grant-super-admin idempotent on upgraded install) | the claim seeds `instance_admins`; grant is `ON CONFLICT DO NOTHING` |
| **D2** CLI-first provisioning, web DEFERRED | EVERY slice-06 provisioning scenario drives `foundry doctor provision-workspace`; sc 9 asserts provisioning is off the bearer surface; the web flow is explicitly OUT (feature header) | honours upstream finding 1 |
| **D3** `instance_admins` table + `is_instance_admin` authz | slice-06 sc 5 (non-super-admin refused), sc 6 (non-enumerable authz), sc 7 (super-admin can provision) | the `EXISTS`-shaped authz gate, fail-closed |
| **D4** migration guarantee = real-snapshot before/after-equality, NO backfill | slice-05 sc 2 (row equality), sc 4 (active workspace stays UNWRITTEN — the no-backfill finding made observable), sc 3 (carried session/token resolves) | proof, not row rewrite |
| **D5** idle + LRU size-cap eviction, std-only off the shipped clock | **NOT an acceptance scenario** — layer-1/2 unit/property test at `rate_limit.rs` (see § above). Noted as non-testable at the acceptance layer (it mutates in-memory map state, not port-observable DB state). | DELIVER owns the MockClock-driven bounded-map + behaviour-preserving PBT |
| **D6** one additive migration `0011_instance_admins.sql` | slice-05 sc 5 (idempotent re-apply), slice-06 sc 7/8 (table seeded); the `0011`-MISSING RED gate threads every scenario | forward-only, additive, empty until seeded |
| **D7** no new check-arch rule for v1 (admin_cli + bootstrap already allow-listed) | implicit invariant — provisioning lands in `admin_cli` (allow-listed `check_arch.rs:394`); `is_instance_admin` is non-tenant-scoped so cannot trip LAYER-1e. **Not an acceptance scenario** — it is a build-time guard (`cargo xtask check-arch`), asserted by the existing boundary-guard lane, not by a `.feature` | noted as build-time, not acceptance-layer |

## Mapping to inherited stories + NFRs

| Story / NFR | Scenarios |
|---|---|
| **US-MWT06** (upgrade safety) | slice-05 sc 1-6 |
| **US-MWT07** (operator provisions + first admin) | slice-06 sc 1, 2, 5, 6, 7, 8, 9 |
| **US-MWT08** (real two-workspace fixtures + bound rate map) | slice-06 sc 3, 4 (real A/B provisioned-tenant isolation); rate-bucket bound = `rate_limit.rs` module test (§ above) |
| NFR-MWT-SEC-01 (scoped read/write) | slice-06 sc 3 |
| NFR-MWT-SEC-02 (non-enumerable refusal) | slice-06 sc 4 (cross-tenant), sc 6 (non-enumerable authz) |
| NFR-MWT-SEC-03 (fail-closed) | slice-06 sc 5 |
| NFR-MWT-SEC-04 (authority does not cross tenants) | slice-06 sc 5 |
| NFR-MWT-DATA-01 (forward-only, no rewrite, no loss) | slice-05 sc 2, 4 |
| NFR-MWT-DATA-02 (sessions/tokens/sign-in keep working) | slice-05 sc 1, 3, 6 |
| NFR-MWT-DATA-03 (no query depends on `uniq_one_workspace`) | SHIPPED-verified in the parent (slice 1 audit); not re-authored here |
| NFR-MWT-REL-01 (provisioning yields a fully isolated tenant) | slice-06 sc 1, 2, 3 |
| NFR-MWT-REL-02 (existing green stays green) | slice-05 sc 6 (regression); the full `@all` suite is the standing guard |
| NFR-MWT-PERF-01 (bounded per-tenant in-memory resource) | `rate_limit.rs` module test (§ above) — NOT acceptance |
| NFR-MWT-PERF-02 (resolution adds no material cost) | inherited timing budget; no new acceptance assertion (resolution unchanged) |
| NFR-MWT-TEST-01 (real two-workspace fixtures) | slice-06 sc 3, 4 (REAL provisioned Globex, not synthetic uuids) |

## Mandate compliance (CM-A..H)

- **CM-A (hexagonal boundary)**: every scenario enters through a driving port — the operator CLI
  subprocess (`provision-workspace` / `grant-super-admin`), the migration runner (slice 5), the
  in-process axum router + sign-in/resolution seam (isolation + resolution proofs). No internal
  component (`provision_workspace` tx, `is_instance_admin`) is invoked directly from a scenario.
- **CM-B (business language)**: Gherkin uses domain terms (super-admin, provision, workspace, sign
  in, isolated, refused, invite link). Zero technical jargon (no HTTP verbs, status codes, table
  names, SQL) in scenario titles or steps — those live in DELIVER's step glue.
- **CM-C (user-journey completeness)**: each scenario is a complete user journey with observable
  value (operator provisions ⇒ new admin signs in ⇒ acts isolated; operator upgrades ⇒ users work
  unchanged). Walking skeletons are user-goal framed, not layer-connectivity framed.
- **CM-D (pure-function extraction)**: the only business arithmetic (token-bucket refill + eviction)
  is already a pure function (`RevokeRateLimiter::consume`); it is tested directly at layer 1-2, not
  through fixture-parametrized acceptance. No fixture parametrization at the acceptance layer.
- **CM-E (Mandate 8 universe-bound assertion)**: layer-3 Rust acceptance has no `state_delta.rs`
  port (Python pilot only). Universe-guard is satisfied by explicit port-exposed assertions
  (CLI exit code + stdout, workspace-scoped DB row presence, sign-in success, resolution result,
  unchanged-snapshot equality) — matching the shipped slices 1-4 convention.
- **CM-F (Mandate 9 PBT mode)**: zero PBT machinery in any `.feature` (all LAYER-3, example-only).
  The only PBT in the feature is the `rate_limit` eviction (layer 1-2), correctly placed at the unit
  layer in DELIVER, not in a `.feature`.
- **CM-G (Mandate 10 Tier B)**: NOT triggered. No acceptance journey is ≥3 chained scenarios with a
  domain-rich input space; the provisioning journey is example-covered (Tier A only).
- **CM-H (Mandate 11 sad paths example-based)**: every sad/evil-user/unauthorized path
  (slice-05 sc 5; slice-06 sc 4, 5, 6, 9) is a named example-based scenario; no PBT generation at
  layer 3+.

## Pre-DELIVER fail-for-the-right-reason note

DISTILL does NOT run `cargo` in this authoring-only session. The RED-state contract is declared per
scenario (tables above): the crate COMPILES (feature files are Gherkin text; no new undefined-symbol
references added to any `.rs`; `acceptance.rs` is NOT edited), so RED is never BROKEN. The genuine
RED for every scenario is MISSING_FUNCTIONALITY (the `0011` migration, the `instance_admins` table,
`is_instance_admin`, the `provision_workspace` tx, the three CLI subcommands, and the slice-5
real-snapshot harness do not exist yet). DELIVER runs the fail-for-right-reason gate at RED-phase
entry (ADR-025 D2) before unskipping each scenario, and must add the step glue + force-link the new
step modules in `acceptance.rs` (DELIVER's job, not DISTILL's).
