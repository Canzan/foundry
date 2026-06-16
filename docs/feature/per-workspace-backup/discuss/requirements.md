# Requirements: per-workspace-backup (OD-5 / DD-MWT-09)

## Business context

Foundry ships whole-instance backup only (`pg_dump -Fc` + `foundry doctor backup-verify`). That dump
mixes every workspace together. An operator who needs **one tenant's data in isolation** — to archive
a churned customer, hand a departing workspace its own data, migrate a single tenant to another
instance, or take a pre-deletion snapshot — has no tool. This feature adds a **per-workspace EXPORT**:
a portable, isolation-scoped logical dump of exactly one workspace.

OD-5 was deferred from multi-workspace-tenancy and ratified as DD-MWT-09: whole-instance backup is
unchanged for v1; per-workspace export is the deferred slice. Crucially, DD-MWT-09 notes that a
per-workspace *restore* must not clobber a sibling — an isolation-sensitive write path that is the
meaningfully-harder half. **This feature is EXPORT ONLY; restore/import is the deferred follow-up.**

## Scope

- **In**: `foundry doctor list-workspaces`, `foundry doctor export-workspace <id|slug> <out-path>`,
  `foundry doctor verify-export <out-path>`.
- **Out (v1)**: per-workspace restore/import; web/API surface; whole-instance backup changes;
  encryption-at-rest of the archive (operator responsibility, as today).

## System Constraints (cross-cutting)

- **Surface = operator CLI only.** New `foundry doctor` subcommands, OFF the bearer API. Shell access
  ⇒ host trust; consistent with the CLI-first provisioning decision (DD-MWT / ADR-002). Mirrors the
  shipped scaffold (`crates/foundry-app/src/admin_cli.rs`): structured `key: value` stdout, a
  terminating `status: <verb>` line, structured exit codes, thread-isolated tokio runtime, live DB
  via `DATABASE_URL` through the `Store`/`Services` seam.
- **Mechanism = isolation-scoped LOGICAL export.** Walk the tenant tables, select ONLY rows belonging
  to the target workspace (directly via `workspace_id`, or transitively via team/project/issue FK
  chains — the SHIPPED scoping seam), into a portable archive.
- **Tenant surface = the 10 TENANT_TABLES**: workspaces, users, workspace_memberships, teams,
  team_memberships, projects, issues, invites, comments, machine_tokens
  (`feature_mwt_slice_05_migration_guarantee.rs`). DESIGN ratifies the authoritative export list
  (OD-PWB-2).
- **ZERO new crates** expected (reuse `pg_dump`/`pg_restore`-style tooling or the shipped sqlx Store
  seam — DESIGN's call). No bearer surface, no Redis, no Node.
- **Defining property (the crux)**: the export contains ALL of the target workspace's tenant data and
  NONE of any sibling workspace's data.

## Functional requirements

| ID | Requirement | Story |
|----|-------------|-------|
| FR-1 | The operator can list every workspace with its id, slug, and name. | US-PWB-01 |
| FR-2 | The operator can export a workspace by id OR slug to a chosen output path. | US-PWB-01 |
| FR-3 | The export contains only the target workspace's rows across all 10 tenant tables. | US-PWB-02 |
| FR-4 | The export prints a per-table row-count report and a terminating `status: OK`. | US-PWB-01 |
| FR-5 | The operator can verify an archive is complete (all tenant tables present). | US-PWB-02 |
| FR-6 | The operator can verify an archive is isolation-clean (no sibling rows). | US-PWB-02 |
| FR-7 | Unknown workspace is refused (exit 2) with a redirect to `list-workspaces`. | US-PWB-03 |
| FR-8 | Output-path errors fail before any DB read (exit 5); writes are atomic (no half-archive). | US-PWB-03 |
| FR-9 | Exporting the sole/last workspace is valid and removes nothing (read-only). | US-PWB-03 |
| FR-10 | Truncated/incomplete archives are detected by verify (exit 4). | US-PWB-03 |
| FR-11 | The CLI surfaces an at-rest sensitivity note (password hashes, token rows). | US-PWB-03 |

## Non-functional requirements (quality attributes)

| ID | NFR | Measurable criterion |
|----|-----|----------------------|
| NFR-PWB-ISO-01 (CRUX) | **Isolation** — an export contains zero rows belonging to any other workspace. | For an instance with >=2 workspaces, verify-export reports `no rows reference a sibling workspace: YES`; a falsifiability test planting one sibling row makes verify exit non-zero. Measured per export. |
| NFR-PWB-COMP-01 | **Completeness** — all 10 tenant tables are covered; no tenant table silently omitted. | verify-export reports `all 10 tenant tables present: YES`; a test planting a row in each table sees it in both the export count and the completeness check. |
| NFR-PWB-INT-01 | **Integrity / verifiability** — the archive is self-verifiable for completeness + isolation with no out-of-band argument. | `verify-export <path>` round-trips an `export-workspace` archive using only the path (declared-workspace read from the archive header); exit 0 on a clean archive, non-zero on a corrupt/leaky one. |
| NFR-PWB-ATOM-01 | **Atomicity** — a failed/killed export leaves no file at the output path that could be mistaken for complete. | Write to `<out>.partial`, fsync, atomic rename to `<out>`; on failure no `<out>` exists. Test: kill mid-export -> no `<out>`. |
| NFR-PWB-SURF-01 | **Off-bearer surface** — export is reachable only from the operator CLI, never the bearer API. | No `/api/v1` or web route exposes export; boundary guard stays green. |
| NFR-PWB-SEC-01 | **No secret mishandling** — the archive contains password hashes + token rows by necessity (it is a faithful tenant dump); it is an operator-trust artifact and its contents are disclosed. | CLI prints the sensitivity note on success; docs state at-rest protection is the operator's responsibility (same posture as the whole-instance dump). No secret is logged to stdout/stderr beyond the disclosed note. |

## Business rules

- An export NEVER mutates the source instance (read-only); exporting a workspace is not a delete.
- "Belongs to workspace W" = the SHIPPED scope definition (direct `workspace_id` or transitive FK).
  The `users` rule under multi-membership is OPEN (OD-PWB-1).
- The export selection predicate and the verify isolation predicate MUST be the same definition.

## Open decisions

See `dor-checklist.md` and the handoff `## Open decisions` block. The three product-relevant ones:
OD-PWB-1 (users scoping under multi-membership — RECOMMENDED: export membership edges + users who are
members of this workspace), OD-PWB-2 (authoritative tenant-table list — RECOMMENDED: pin + gold-test),
OD-PWB-3 (archive container — RECOMMENDED: scoped `pg_dump`-style custom archive with a self-describing
header). All carry a recommended option for DESIGN.
