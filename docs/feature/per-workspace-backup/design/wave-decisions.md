# Wave Decisions: per-workspace-backup (DESIGN)

> Propose mode. Paradigm ESTABLISHED (Rust, ports-and-adapters, @nw-software-crafter). All
> open decisions carry a recommended option; the orchestrator auto-accepts the recommendation.

## DDD decisions

This feature lives ENTIRELY inside the shipped multi-workspace-tenancy bounded context — it does NOT
introduce a new context. The relevant tactical facts:

- **Aggregate boundary = the Workspace.** "Belongs to workspace W" is the consistency boundary the
  export captures. The 10 tenant tables are the Workspace aggregate's persisted footprint; the
  export is a faithful serialization of that aggregate for one W.
- **The `users` entity is SHARED, not owned.** `users` is a global identity table referenced by
  multiple Workspace aggregates (multi-membership). It is a Shared Kernel entity. The export
  includes the slice of `users` that are MEMBERS of W (membership-bounded), and isolation never
  treats a shared user identity as a sibling leak (OD-PWB-1 / ADR-001). This is the one place the
  aggregate's "owned-by-W" rule yields to "member-of-W".
- **The scope predicate is the ubiquitous-language definition of "belongs to W".** It MUST be a
  single shared definition (Section 5 of architecture.md) used identically by export selection and
  verify isolation — divergence is the crux risk.
- **No new aggregates, no domain events, no ES/CQRS.** The export is a read-model projection
  (a serialization), not a new write model. ES/CQRS is explicitly NOT warranted (simple read-only
  batch capability).

## Reuse-vs-new breakdown

| Concern | Decision | Source / target |
|---------|----------|-----------------|
| `foundry doctor` CLI scaffold (thread-isolated tokio runtime, `key: value` + `status:` stdout, exit codes 0/2/3/4/5, live DB via `DATABASE_URL`) | **REUSE** | `crates/foundry-app/src/admin_cli.rs` (`run_restore_comment` / `run_provision_workspace` pattern) |
| doctor subcommand dispatch | **EXTEND** | `crates/foundry-app/src/main.rs` doctor match arm — add `list-workspaces`, `export-workspace`, `verify-export` |
| List workspaces | **REUSE** | `Store::list_workspaces()` (returns `(id, name)`, shipped) |
| Workspace id resolution | **REUSE + EXTEND** | `Store::list_workspaces` for id/name match; NO `slug` column exists (see upstream-changes.md) |
| Whole-row capture idiom | **REUSE** | `feature_mwt_slice_05_migration_guarantee.rs::snapshot_tenant_tables` (`SELECT to_jsonb(t.*)::text`) |
| Tenant-table set | **REUSE the thinking, PIN a NEW constant** | slice-05 `TENANT_TABLES` (10 tables) — re-declared as this feature's owned constant (OD-PWB-2), NOT the `admin_cli` backup-verify list |
| Per-table scoped reader + read tx | **NEW (in foundry-store)** | `Store::export_workspace(W) -> WorkspaceExport` (10 scoped SELECTs in one REPEATABLE READ tx) |
| Archive container + manifest | **NEW** | tar of `manifest.json` + `tables/*.jsonl` (OD-PWB-3) |
| Atomic write | **NEW (std)** | `<out>.partial` -> fsync -> rename |
| verify-export reader + isolation check | **NEW (in admin_cli)** | reads manifest, re-applies the scope predicate per archived row |
| LAYER-1e exemption | **REUSE (no change)** | `admin_cli` already allow-listed in `check_arch.rs::is_tenant_scoping_allowlisted` |
| Gold-test table-set guard | **NEW (acceptance)** | mirrors `check_arch.rs` plant-a-violation discipline |
| New crates | **ZERO** | confirmed |
| Migration | **NONE** | read-only |

## Reading checklist (for acceptance-designer + software-crafter)

Read in this order before implementing:

1. `docs/feature/per-workspace-backup/design/architecture.md` — the scope-predicate table (Section 5),
   the manifest schema (Section 6), the pipeline component diagram (Section 4).
2. `docs/feature/per-workspace-backup/discuss/{user-stories.md, acceptance-criteria.md}` — the 16 ACs.
3. `crates/foundry-app/src/admin_cli.rs` — the scaffold to mirror (`run_provision_workspace` is the
   closest precedent: live DB, thread-isolated runtime, structured stdout, exit codes).
4. `crates/foundry-app/src/main.rs` (doctor dispatch, lines ~678-777) — the match arm to extend.
5. `crates/foundry-store/src/lib.rs` — `list_workspaces`, `resolve_active_workspace`, the sqlx idioms;
   the home of the new `export_workspace` fn.
6. `crates/foundry-store/migrations/0001_init.sql` + `0004`/`0007`/`0008` — the authoritative columns
   the scope predicate depends on (note: `workspaces` has NO `slug`; `comments` HAS a direct
   `workspace_id`; only `team_memberships` is transitive-only).
7. `crates/foundry-acceptance/src/steps/feature_mwt_slice_05_migration_guarantee.rs` — `TENANT_TABLES`
   + `snapshot_tenant_tables` (the idiom to reuse) + the falsifiability discipline.
8. `xtask/src/check_arch.rs` — `is_tenant_scoping_allowlisted` (admin_cli exemption) + the gold-test
   plant-a-violation model for OD-PWB-2.
9. `docs/feature/foundry-backend-mvp/design/system/backup-restore.md` — the shipped backup conventions.

## Open decisions (resolved — recommended option each)

### OD-PWB-1 — the `users` scoping predicate under multi-membership
**Recommended (ADR-001): membership-bounded include, NOT a sibling violation.** Export the
`workspace_memberships` edges for W and the `users` rows that are members of W (predicate #10).
A multi-membership user appearing in a W export is legitimate (the user IS a member of W); isolation
applies to workspace-OWNED resources (tables 1-9), never to shared user identities. Verify confirms
each archived user is a member of W; it does NOT fail because the user also belongs elsewhere.

### OD-PWB-2 — the authoritative tenant-table set
**Recommended (ADR-005): pin ONE shared constant + gold-test it.** A single `TENANT_TABLES`
constant (the 10 tables: workspaces, users, workspace_memberships, teams, team_memberships, projects,
issues, invites, comments, machine_tokens) read by BOTH export and verify, written into the manifest
and re-checked by verify. Gold test plants a row per table and asserts both export count and verify
completeness see all 10. DELIBERATELY distinct from `admin_cli.rs::run_backup_verify`'s list.

### OD-PWB-3 — the archive container + manifest header
**Recommended (ADR-002): a single tar file of `manifest.json` + `tables/<table>.jsonl`** (whole-row
`to_jsonb(t.*)` JSONL, the slice-05 idiom). NOT `pg_dump` (cannot column-scope). The manifest header
records `declared_workspace_id` (+ name, table set, row counts, format_version) so `verify-export
<path>` round-trips from the path alone. Tar renames atomically (the ACs assert "a file at <path>").

### OD — export consistency
**Recommended (ADR-003): one `Store::export_workspace(W)` fn running all 10 scoped SELECTs inside a
single `REPEATABLE READ` read tx.** Predicate lives in ONE place; archive is a consistent cut;
referential closure holds.

### OD — verify-export design
**Recommended (ADR-004): verify reads the manifest header, checks completeness (10 tables present +
JSONL line count == header count -> exit 4 on mismatch), then re-applies the SAME scope predicate
per archived row for isolation (sibling row -> non-zero, names the foreign row).** Path-only input.

### OD — LAYER-1e / check-arch allow-list
**Recommended (ADR-006): NO new allow-list line.** The export code lives in `admin_cli.rs`, already
allow-listed by `is_tenant_scoping_allowlisted`. The reader scopes by a CLI-supplied
(operator-trusted) workspace id off the bearer surface. Keep export code in `admin_cli.rs` /
`foundry-store`; do NOT route it through a foundry-app handler that LAYER-1e scans.

### OD — migration
**Recommended: NONE.** Read-only feature; no schema change. Confirmed.

## Residual open decisions (carry into DISTILL/DELIVER with recommended options)

1. **Selector grammar without a workspace `slug` (drift — see upstream-changes.md).** The user
   stories use `globex`/`acme` as slugs, but `workspaces` has no `slug` column.
   **Recommended: accept `<id>` OR an exact `<name>` match** (case-insensitive, ambiguity -> exit 2
   listing matches). Software-crafter confirms whether DISTILL wants name-matching or id-only; the
   ACs that hard-code slug strings need acceptance-designer to treat them as the selector token
   (id or name), not as a literal `slug` column lookup.
2. **tar crate choice.** `tar` (MIT/Apache) is the obvious OSS pick; software-crafter selects the
   exact crate during GREEN. Design pins the container SHAPE, not the crate.
3. **`exported_at` non-determinism in gold tests.** The manifest's timestamp makes byte-equality
   tests flaky. **Recommended: gold tests assert on row counts + table set + isolation, not on
   manifest byte-equality** (mirroring slice-05, which orders by row text and projects out additive
   columns).
