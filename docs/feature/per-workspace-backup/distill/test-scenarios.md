# DISTILL Test Scenarios: per-workspace-backup

> Acceptance designer (Quinn / DISTILL wave). Source Gherkin SSOT:
> `crates/foundry-acceptance/tests/features/us-per-workspace-backup.feature`.
> This catalog maps every scenario to its story / NFR / design decision, records the
> per-scenario RED state, and documents the @walking_skeleton / @pending plan.

## Configuration

- **test_type**: core feature
- **framework**: cucumber-rs (`tests/features/*.feature`; harness `acceptance.rs` `harness=false`;
  step glue authored in DELIVER under `crates/foundry-acceptance/src/steps/feature_per_workspace_backup.rs`
  + `world.rs`, registered in `lib.rs`, force-linked in `acceptance.rs`).
- **integration approach**: real services — real Postgres (testcontainers, per-scenario schema) +
  the operator CLI driven as a REAL subprocess (`assert_cmd::Command::cargo_bin("foundry")`) against
  the per-scenario DB. Mirrors the shipped `backup-verify` / `provision-workspace` / `migration-guarantee`
  scenarios. Tag: `@real-io`.
- **layer**: LAYER-3 (real adapter + real subprocess). Example-based; no PBT machinery (Mandates 9+11).
- **lang-mode**: rust. **policy-mode**: inherit (`docs/architecture/atdd-infrastructure-policy.md` present).

## Phase-0 / gate log

- `[lang-mode] rust` — `Cargo.toml` at repo root.
- `[policy-mode] inherit` — `docs/architecture/atdd-infrastructure-policy.md` present (18.8KB); CLI +
  Postgres-via-testcontainers + filesystem-via-tmpdir treatments inherited.
- **State-delta port**: NO `tests/common/state_delta.rs` exists (matches slices 1-6 precedent). Mandate 8
  (universe-bound assertion) is a layers 1-3 requirement WITH a Python pilot port; no Rust port exists, so
  LAYER-3 assertions are traditional assertions over port-exposed observables (CLI exit code + stdout, archive
  file presence/absence, unchanged source rows). No bootstrap performed — consistent with the shipped
  Rust acceptance suite; introducing a Rust state-delta port is out of scope for this feature.
- **Wave-Decision Reconciliation HARD GATE**: PASSED — 0 contradictions. DISCUSS `wave-decisions.md` and the
  `devops/` directory are ABSENT (WARN, not block — graceful degradation). DESIGN explicitly reconciles the
  DISCUSS slug/transitive assumptions via `design/upstream-changes.md` + ADR-001..006 with ratified
  recommended options (auto-accepted). No DISCUSS decision is contradicted by DESIGN; the drifts are
  refinements, not conflicts. DEVOPS absent → default environment matrix not applicable (the only environment
  is the per-scenario testcontainers PG16 + tmpdir filesystem the shipped suite already uses).

## Driving adapter coverage (Mandatory — RCA P1)

The DESIGN specifies a CLI entry point family (`foundry doctor list-workspaces / export-workspace /
verify-export`). Every subcommand is exercised via its real subprocess protocol:

| Subcommand | Exercised by (scenario #) | Verifies exit code + stdout + arg handling |
|------------|---------------------------|--------------------------------------------|
| `list-workspaces` | 2 | exit 0, lists id+name, `status: OK` |
| `export-workspace <selector> <path>` | 1 (WS), 3, 4, 5, 8, 10, 11, 12, 13, 15, 16, 17 | exit 0/2/3/5, per-table counts, sensitivity note, atomic write |
| `verify-export <path>` | 6, 7, 8, 9, 10, 13, 14 | exit 0/4/non-zero, completeness + isolation report lines |

Zero uncovered entry points.

## Driven adapter coverage (Mandate 6)

Every driven adapter has at least one `@real-io` scenario exercising real I/O:

| Adapter | @real-io scenario | Covered by |
|---------|-------------------|------------|
| `Store::export_workspace` (scoped reader, REPEATABLE READ tx, real Postgres) | YES | WS #1 + all export scenarios (real testcontainers PG16, two seeded workspaces) |
| tar archive writer on real filesystem (atomic `.partial` → rename) | YES | #1, #13 (atomicity), #12 (path pre-flight) |
| verify-export archive reader + offline isolation predicate | YES | #6, #7, #8, #9, #14 |
| id-or-name selector resolution (`Store::list_workspaces`) | YES | #2 (list), #3 (by id), #11 (unknown → exit 2) |

No costly external adapters (no LLM/paid API); no `@requires_external`. No mocks at the acceptance level
(DB unreachable in #15 is simulated by a real bad `DATABASE_URL`, not a mock).

## Scenario catalog

| # | Scenario | Tags | Story / AC | NFR / decision | RED reason (MISSING_FUNCTIONALITY) |
|---|----------|------|-----------|----------------|------------------------------------|
| 1 | Export one workspace to a verifiable archive reporting all ten tables | `@walking_skeleton @wiring_e2e @us-pwb01` | US-PWB-01 / AC-01.2 | ADR-002/003 | `export-workspace` subcommand + `Store::export_workspace` absent |
| 2 | See every workspace's identity before exporting | `@pending @us-pwb01` | US-PWB-01 / AC-01.1 | DRIFT-1 (id+name) | `list-workspaces` subcommand absent |
| 3 | Export a workspace selected by its id | `@pending @us-pwb01` | US-PWB-01 / AC-01.3 | DRIFT-1 selector fn | selector resolution + subcommand absent |
| 4 | Exporting removes nothing from the instance | `@pending @us-pwb01` | US-PWB-01 (read-only) | read-only constraint | export subcommand absent |
| 5 | Archive contains every target row and no sibling row | `@pending @us-pwb02` | US-PWB-02 / AC-02.1 | NFR-PWB-ISO-01, §5 predicate | scoped reader absent |
| 6 | Confirm complete + isolation-clean | `@pending @us-pwb02` | US-PWB-02 / AC-02.2 | ADR-004, NFR-PWB-INT-01 | `verify-export` absent |
| 7 | Transitively-scoped rows isolation-checked via FK chain | `@pending @us-pwb02` | US-PWB-02 / AC-02.3 | DRIFT-2, §5 cross-check | verify FK-chain resolver absent |
| 8 | Multi-membership user included, not flagged as leak | `@pending @us-pwb02` | US-PWB-02 / OD-PWB-1 | ADR-001 membership-bounded | users predicate #10 + verify absent |
| 9 | Verification fails loudly on a planted sibling row | `@pending @us-pwb02 @error` | US-PWB-02 / AC-02.4 | NFR-PWB-ISO-01 falsifiability | verify isolation check absent |
| 10 | Any single-workspace export is sibling-free (example-pinned) | `@pending @us-pwb02 @property` | US-PWB-02 / AC-02.5 | the invariant | export + verify absent |
| 11 | Unknown workspace refused with guidance | `@pending @us-pwb03 @error` | US-PWB-03 / AC-03.1 | exit 2 + redirect | selector + exit-2 path absent |
| 12 | Path unwritable fails before any DB read | `@pending @us-pwb03 @error` | US-PWB-03 / AC-03.2 | exit 5 pre-flight | pre-flight path stage absent |
| 13 | Disk-full leaves no complete-looking archive | `@pending @us-pwb03 @error` | US-PWB-03 / AC-03.3 | NFR-PWB-ATOM-01 | atomic `.partial`→rename absent |
| 14 | Verification detects an incomplete archive | `@pending @us-pwb03 @error` | US-PWB-03 / AC-03.5 | exit 4 count tripwire | verify completeness check absent |
| 15 | DB unreachable reports a clear error | `@pending @us-pwb03 @error` | US-PWB-01 / AC-01.4 | exit 3 | connect-error mapping absent |
| 16 | Sole-workspace export is valid and read-only | `@pending @us-pwb03` | US-PWB-03 / AC-03.4 | only-workspace note | subcommand + note absent |
| 17 | Operator warned about sensitive at-rest contents | `@pending @us-pwb03` | US-PWB-03 / AC-03.6 | NFR-PWB-SEC-01 | sensitivity note absent |

**17 scenarios** (scenario 10 is a `Scenario Outline` with 2 examples → 18 executable cases). Error/edge
ratio: scenarios 9, 11, 12, 13, 14, 15 tagged `@error` = 6/17 = **35%**; counting the read-only/boundary
safety scenarios 4 + 16 as edge-coverage = 8/17 = **47%** (≥40% target met).

## Exit-code contract coverage (mirrors admin_cli.rs)

| Code | Meaning | Scenario(s) |
|------|---------|-------------|
| 0 | success (`status: OK`) | 1, 2, 3, 6, 8, 10, 17 |
| 2 | unknown/invalid workspace (+ redirect) | 11 |
| 3 | DB / infra unreachable | 15 |
| 4 | archive truncated / incomplete (verify) | 14 |
| 5 | output-path error (before any DB read) | 12 |

Every exit code in the contract is asserted by at least one scenario.

## Completeness gold-test discipline (OD-PWB-2 / ADR-005) — DELIVER build/unit guard, NOT a subprocess scenario

The acceptance side asserts the archive COVERS all 10 `TENANT_TABLES` (scenario 1 "row count for all 10
tenant tables" + scenario 6 "all 10 tenant tables are present"). The ADR-005 **plant-a-row-PER-table gold
test** — plant one row in EACH of the 10 tables for the target workspace, export, assert the export count
AND verify completeness both see all 10, and removing a table from the constant reds the test — is a
DELIVER build/unit guard at the store/admin_cli seam (mirroring `xtask/src/check_arch.rs`'s
plant-a-violation discipline). It is faster and more precise as a unit-level guard than a subprocess
scenario, and it is the forcing function that keeps the `TENANT_TABLES` constant honest as the schema
evolves. **Action for DELIVER**: author the gold test alongside the `TENANT_TABLES` constant; it is NOT a
`.feature` scenario.

## Tenant-table set (the owned constant — OD-PWB-2)

The 10 `TENANT_TABLES`, with their scope predicate (§5 of architecture.md), all asserted present by the
completeness scenarios:

| # | Table | Scope predicate (export WHERE == verify isolation) |
|---|-------|----------------------------------------------------|
| 1 | workspaces | `id = W` |
| 2 | workspace_memberships | `workspace_id = W` |
| 3 | teams | `workspace_id = W` |
| 4 | team_memberships | `team_id IN (SELECT id FROM teams WHERE workspace_id = W)` (transitive) |
| 5 | projects | `workspace_id = W` |
| 6 | issues | `workspace_id = W` |
| 7 | invites | `workspace_id = W` |
| 8 | comments | `workspace_id = W` (verify ALSO cross-checks `issue_id → issues.workspace_id` — DRIFT-2) |
| 9 | machine_tokens | `workspace_id = W` |
| 10 | users | `id IN (SELECT user_id FROM workspace_memberships WHERE workspace_id = W)` (membership-bounded — ADR-001) |

DELIBERATELY distinct from `admin_cli.rs::run_backup_verify`'s list (DRIFT-3): that list includes
`issue_attachments`/`session`/`outbox` and omits `invites`/`machine_tokens`. Do NOT copy it.

## Layered test discipline — what stays at layer 1-2 (DELIVER)

Per the Paradigm Mandate, generative exploration belongs at the cheap layer. DELIVER authors these as
layer-1/2 property tests at the `foundry-store` / `admin_cli` seam (NOT subprocess scenarios):

- **Isolation invariant (generative)**: for any seeded multi-workspace fixture, `export_workspace(W)`
  selects exactly the rows the predicate admits and verify re-resolves every row to W — the layer-1-2
  amplification of scenario 10's `@property` example.
- **Predicate symmetry (selection == isolation)**: the export `WHERE` and the verify isolation check are
  the SAME definition (the crux). A property test asserting they agree on arbitrary row sets.
- **Gold table-set guard**: the plant-a-row-per-table completeness test (above).
- **Selector resolution**: id-or-name match, case-insensitivity, ambiguous-name → exit 2.
- **Manifest round-trip**: write manifest → read manifest yields the same declared workspace + table set
  + counts (the `to_jsonb` whole-row idiom round-trips).

## Open decisions / ambiguities carried into DELIVER (recommended options — orchestrator auto-accepts)

1. **Selector grammar (DRIFT-1, residual #1).** `workspaces` has no `slug`. **Recommended (baked into the
   feature file): accept `<id>` OR exact case-insensitive `<name>`; ambiguous name → exit 2 listing
   matches.** The feature file's `globex`/`acme` tokens are SELECTOR tokens, not slug lookups. The header
   reads "workspace name", not "slug". DELIVER confirms id-or-name (not id-only).
2. **tar crate choice (residual #2).** Design pins the SHAPE (tar of manifest.json + tables/*.jsonl), not
   the crate. **Recommended: the `tar` crate (MIT/Apache).** Software-crafter selects during GREEN.
3. **`exported_at` non-determinism (residual #3).** The manifest timestamp makes byte-equality flaky.
   **Recommended: gold/unit tests assert on row counts + table set + isolation, NOT manifest byte-equality**
   (mirroring slice-05). The acceptance scenarios already assert on counts + presence + isolation, never on
   manifest bytes — no flakiness exposure at the acceptance layer.
4. **`issue_attachments` (DRIFT-3, LOW).** Exists in schema + the backup-verify list but NOT in the
   slice-05 10-table `TENANT_TABLES`. **Recommended: follow the ratified 10-table set for v1**; flag an 11th
   table as a follow-up if operators report missing attachments. The gold test makes any future addition
   explicit. No scenario asserts attachments are present.
5. **Step glue + world fixtures (DELIVER infra).** DISTILL authors Gherkin only (crate compiles, no new
   `.rs`). DELIVER builds `feature_per_workspace_backup.rs` (steps), `world.rs` additions (two-workspace
   seeding helper reusing the slice-05 `snapshot_tenant_tables` / slice-06 two-workspace fixtures), registers
   in `lib.rs`, and force-links in `acceptance.rs` — in the same RED→GREEN→COMMIT cycle that unskips the WS
   scenario.

## Pre-DELIVER fail-for-the-right-reason note

No cargo run performed in DISTILL (per orchestrator instruction: "Do not run cargo"). The RED contract is
established structurally: the feature file adds no undefined-symbol reference to any `.rs` and `acceptance.rs`
is untouched, so the crate COMPILES (NOT BROKEN). At DELIVER RED-phase entry, each unskipped scenario will
fail as MISSING_FUNCTIONALITY (unknown subcommand / absent `Store::export_workspace`), not as a
collection/import error. DELIVER classifies RED genuineness per the unskip cycle.
