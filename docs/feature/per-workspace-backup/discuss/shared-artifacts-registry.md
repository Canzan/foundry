# Shared Artifacts Registry — per-workspace-backup

> Every value that flows across the list -> export -> verify journey, its single source of truth, and
> the integration risk if a consumer diverges. The two HIGH-risk artifacts are the crux of the
> feature: they make completeness and isolation *provable* rather than asserted.

## Registry

```yaml
shared_artifacts:
  workspace_identity:
    source_of_truth: "workspaces table (id, slug, name)"
    consumers:
      - "foundry doctor list-workspaces (displays id + slug + name)"
      - "foundry doctor export-workspace <id|slug> (accepts id OR slug as the selector)"
      - "the export archive header (records the resolved workspace_id as declared-workspace)"
      - "foundry doctor verify-export (reads declared-workspace back, uses it as the isolation predicate)"
    owner: "multi-workspace-tenancy schema (shipped)"
    integration_risk: "HIGH — if list-workspaces, the export selector, the archive header, and verify use different identity rules (e.g. slug vs id, case sensitivity), the operator could verify a DIFFERENT workspace than they exported, defeating the whole trust chain."
    validation: "One resolution function maps an id-or-slug argument to a single workspace_id; that id is what the header records and verify reads. Acceptance: export then verify the same selector resolve to the same workspace."

  tenant_tables_set:
    source_of_truth: "crates/foundry-acceptance/src/steps/feature_mwt_slice_05_migration_guarantee.rs::TENANT_TABLES (workspaces, users, workspace_memberships, teams, team_memberships, projects, issues, invites, comments, machine_tokens) — DESIGN ratifies the authoritative export list (see Open Decision OD-PWB-2)."
    consumers:
      - "export-workspace (walks each table, selects this workspace's rows, prints per-table row-counts)"
      - "verify-export (asserts all N tables are present in the archive — the completeness check)"
      - "whole-instance backup-verify (admin_cli.rs counts an OVERLAPPING but not identical list — see risk)"
    owner: "per-workspace-backup (this feature) — but MUST stay in sync with the tenant surface as new tenant tables are added"
    integration_risk: "HIGH — (1) if export walks a different set than verify checks, a silently-omitted table passes verification (incomplete archive looks complete). (2) The shipped backup-verify table list in admin_cli.rs differs (it includes issue_attachments, session, outbox; omits invites, machine_tokens). The per-workspace export list must be deliberately chosen and DOCUMENTED, not copied from either source by accident."
    validation: "export and verify reference ONE constant. A test plants a row in each tenant table and asserts both the export count and the verify completeness check see all of them."

  workspace_scope_predicate:
    source_of_truth: "the SHIPPED scoping seam — each tenant table reaches workspace_id either directly (a workspace_id column) or transitively via team_id -> project_id -> issue_id FK chains (per the TENANT_TABLES doc-comment)."
    consumers:
      - "export-workspace (the WHERE clause selecting this workspace's rows)"
      - "verify-export (the predicate confirming each archived row resolves to the declared workspace AND none resolves to a sibling)"
    owner: "multi-workspace-tenancy scoping seam (shipped)"
    integration_risk: "HIGH (the crux) — if export SELECTs rows by one predicate and verify checks isolation by a different one, a leaked row could pass verification. Selection and isolation must be the SAME definition of 'belongs to this workspace'. The users-table scoping rule is an OPEN DECISION (OD-PWB-1) — multi-membership users belong to multiple workspaces."
    validation: "export and verify share one scope definition. Falsifiability test: plant a sibling-owned row into an archive and assert verify's isolation check REDS (mirrors the slice-05 falsifiability discipline)."

  exit_code_contract:
    source_of_truth: "this feature's CLI contract, mirroring crates/foundry-app/src/admin_cli.rs conventions (0 ok / 2 invalid-arg / 3 DB-infra / 4 not-restorable-or-incomplete / 5 output-error)."
    consumers:
      - "export-workspace and verify-export (return codes)"
      - "operator cron / scripts (grep status: line, branch on exit code)"
      - "acceptance scenarios (assert exact codes)"
    owner: "per-workspace-backup"
    integration_risk: "MEDIUM — operators wire these into cron exactly as they do backup-verify; inconsistent codes across the foundry doctor family would surprise existing scripts. Stay consistent with the shipped scaffold's exit-code discipline."
    validation: "Each documented failure path asserts its exact exit code in an acceptance scenario."

  archive_format:
    source_of_truth: "this feature (DESIGN pins the concrete container — see OD-PWB-3)."
    consumers:
      - "export-workspace (writes it)"
      - "verify-export (reads it, including the declared-workspace header)"
    owner: "per-workspace-backup"
    integration_risk: "MEDIUM — export and verify must agree byte-for-byte on the container shape and where the declared-workspace id lives. A self-describing header is required so verify can read the declared workspace without an out-of-band argument."
    validation: "verify-export round-trips an export-workspace archive with no extra arguments beyond the path."
```

## Consistency check (validation questions answered)

- **Does every value shown in the TUI mockups have a documented source?** Yes — workspace id/slug
  (workspaces table), the 10 row-count table names (TENANT_TABLES), declared-workspace (archive
  header), exit codes (CLI contract).
- **If a new tenant table is added later, would the export stay complete?** Only if the
  `tenant_tables_set` constant is the single source both export and verify read AND it is updated
  with the schema. Flagged HIGH; DESIGN must decide whether to derive the list or pin+test it
  (OD-PWB-2).
- **Do any two steps display the same data from different sources?** The risk is workspace identity
  and the scope predicate — both addressed by mandating one resolution function and one scope
  definition shared by export and verify.
- **Hardcoded values that should reference a shared artifact?** The table list must NOT be
  copy-pasted from `admin_cli.rs` (whose list differs) — it is its own documented constant.

## Integration checkpoints (carried into DISTILL)

1. export selector -> archive header -> verify declared-workspace: one identity end to end.
2. export table walk == verify completeness set: one tenant-table constant.
3. export selection predicate == verify isolation predicate: one scope definition (the crux).
