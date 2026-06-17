# Evolution — per-workspace-backup (isolation-scoped logical export via the operator CLI)

**Finalized**: 2026-06-16
**DELIVER commits**: `3ad6595` (01-01) → `80a66ba` (03-03) — the 9 DES-monitored TDD steps committed directly to `main` (trunk-based, no PRs), plus phase-log chore `60c9dfe`. Each step ran all 5 DES phases; deliver-integrity verification is green ("All 9 steps have complete DES traces", exit 0).
**Wave coverage**: DISCUSS/DESIGN ratified ahead of DELIVER (isolation-scoped logical export via an operator CLI, **0 migrations**, ADR-001 membership-bounded isolation, the exit-code taxonomy, and the NFR set — INT-01 path-only verify, ATOM-01 atomic write, ISO-01 falsifiability, SEC-01 at-rest sensitivity); DISTILL authored the 17 acceptance scenarios (export + the isolation crux + the failure modes); DELIVER shipped 9 steps across 3 phases. Per-feature layout `docs/feature/per-workspace-backup/`.
**Scope**: a `foundry doctor` operator surface that exports ONE workspace's ten tenant tables to a verifiable archive, and verifies any archive's completeness and isolation **from the path alone** — with no schema change. The crux is provable per-tenant isolation: an export of one workspace contains no sibling-workspace data, and verification *bites* on contamination.

## Milestone — a tenant can be backed up and proven clean

Two new operator subcommands, both LAYER-3 real-subprocess + real-Postgres:

- `foundry doctor export-workspace <id|name> <path>` — snapshots one workspace's ten tenant tables (single `REPEATABLE READ` transaction) into a tar archive with a manifest header; read-only against the DB; discloses that the archive holds sensitive at-rest material (password hashes, machine tokens).
- `foundry doctor verify-export <path>` — reads `declared_workspace_id` from the manifest (path-only, no out-of-band arg — NFR-PWB-INT-01), checks COMPLETENESS (all 10 tables present; per-table JSONL line count == manifest `row_count`), then re-applies the SAME scope predicate offline to every archived row for ISOLATION.
- `foundry doctor list-workspaces` — identity/selector resolution (id-or-name; DRIFT-1: no slug column assumed).

## The crux — isolation, and its falsifiability

- **Scope predicate (architecture §5)** re-applied per archived row: direct `workspace_id` tables, plus the transitive chains `team_memberships → teams.workspace_id` and `comments → issues.workspace_id` (the DRIFT-2 denormalized-`workspace_id` corruption cross-check).
- **Membership-bounded user inclusion (ADR-001)**: a user who belongs to two workspaces is legitimately present in each workspace's archive and is NOT flagged as a leak — inclusion is bounded by membership, not by a sibling-row heuristic.
- **Falsifiability (NFR-PWB-ISO-01)**: planting one Acme row into a Globex archive makes `verify-export` fail loudly (exit 6, naming the row's resolved foreign workspace). A `@property` Scenario Outline pins both directions (export+verify globex and acme each resolve zero foreign rows, exit 0). The isolation guarantee is therefore demonstrated to be refutable, not merely asserted.

## Durability & failure modes (the exit-code taxonomy)

| Code | Condition |
|------|-----------|
| 0 | success (+ sensitivity disclosure; sole-workspace note when only one exists) |
| 2 | unknown workspace selector → message redirects to `list-workspaces`, no archive written |
| 3 | database unreachable (mirrors `admin_cli.rs` DB/infra failure code) |
| 4 | `verify-export` completeness tripwire: manifest `row_count` > actual JSONL lines (truncated/incomplete → re-run guidance) |
| 5 | output-path not writable — pre-flight path stage runs BEFORE any tenant data is read |
| 6 | `verify-export` isolation failure (a row resolves to a workspace other than the declared one) |

**Atomicity (NFR-PWB-ATOM-01)**: writes go to `<out>.partial` → fsync → rename, so a mid-write/disk-full failure never leaves a complete-looking archive at the final path; a later `verify-export` on the final path finds nothing to accept.

## How it was built (DELIVER) — the 9-step TDD arc

- **Phase 01 — walking skeleton + identity + disclosure** (`3ad6595`, `6645779`, `0a24e7e`): export writes a verifiable archive reporting all ten tenant tables; operator identifies and exports by id/name; export is read-only and discloses sensitive at-rest contents (sole-workspace note).
- **Phase 02 — verify: completeness, isolation, falsifiability** (`4a55f94`, `9b8ff46`, `fb258dd`): `verify-export` confirms completeness + isolation from the path alone; transitive FK-chain isolation + multi-membership inclusion; the planted-sibling-row falsifiability crux.
- **Phase 03 — failure modes** (`7f39184`, `9212f9d`, `80a66ba`): unknown-workspace + unreachable-DB refusal with guidance; output-path + disk-full atomicity (no half-written archive); `verify-export` detects a truncated archive.

Six of nine steps recorded `RED_UNIT` as `SKIPPED/NOT_APPLICABLE` — each verified legitimate in adversarial review: the pure detection logic (completeness tripwire, scope-predicate re-application, transitive resolution) carries real unit tests in `crates/foundry-store/src/verify_export.rs` driven out in phase 02; the skipped steps add only thin CLI-adapter glue or acceptance wiring over that already-tested core (a new unit test would be Test Duplication or would mock the filesystem inside the adapter).

## Quality at ship

- **Acceptance**: full `@all` lane green — 367/367 scenarios, 2880/2880 steps (incl. the `@docker-compose` and `@needs-pgclient` groups).
- **Adversarial review (Phase 4)**: APPROVED ("ship it") — no testing theater across the 7-pattern scan; isolation logic sound and falsifiable; all 6 `RED_UNIT` skips ruled legitimate; no dead code.
- **Mutation (Phase 5, per-feature ≥80% gate)**: `verify_export.rs` scoped run = 27 caught / 5 missed / 2 unviable = **84.4%** kill rate (a lower bound — the run was `--lib`-scoped and excluded the foundry-store integration tests that kill more). PASSES the gate.
- **Integrity (Phase 6)**: all 9 steps have complete DES traces (exit 0).
- **0 migrations**: logical export only; no schema change, no new crate.

## Deferred / follow-ups

- **Harden the 5 mutation survivors** in `verify_export.rs` (the `"workspaces"` and `"users"` match arms in `check_isolation`, `VerifyReport::is_ok` `&&`→`||` / `→true`, and the `DIRECT_WORKSPACE_ID_TABLES.contains` match guard): add targeted unit tests, or confirm they fall to the foundry-store integration tests under a non-`--lib` mutation pass. Gate already met; this is depth, not a blocker.
- **DRIFT-3 — the 11th table**: `issue_attachments` is deliberately omitted from the v1 ten-table set; revisit for a v2 that includes attachment payload export.
- **Carried from prior features**: close the bootstrap claim-flow enumeration oracle; Prometheus `foundry_token_mutations_total` exporter; key-rotation UX; nightly scoped mutation pass on the web adapter.

## Pointers

- Production: `crates/foundry-app/src/admin_cli.rs` (CLI handlers + exit-code mapping + atomic write), `crates/foundry-store/src/verify_export.rs` (pure completeness + isolation verifier), `crates/foundry-store/src/lib.rs` (`export_workspace` query surface).
- Tests: `crates/foundry-acceptance/tests/features/us-per-workspace-backup.feature`, `crates/foundry-acceptance/src/steps/feature_per_workspace_backup.rs`, `crates/foundry-store/tests/export_workspace_gold.rs`.
- Wave artifacts: `docs/feature/per-workspace-backup/` (discuss, design, distill, deliver).
