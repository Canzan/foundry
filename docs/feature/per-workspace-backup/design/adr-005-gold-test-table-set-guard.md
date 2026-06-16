# ADR-005: Gold-test guard for the tenant-table set (OD-PWB-2)

## Status
Accepted

## Context
The completeness guarantee (NFR-PWB-COMP-01) requires that export walks, and verify checks, the SAME
set of tenant tables — and that the set stays correct as the schema evolves. The HIGH-risk failure
(shared-artifacts-registry `tenant_tables_set`): if export walks a different set than verify checks,
a SILENTLY OMITTED table passes verification (an incomplete archive looks complete). Separately, the
shipped `admin_cli.rs::run_backup_verify` already hard-codes a DIFFERENT table list (it includes
`issue_attachments`, `session`, `outbox`; omits `invites`, `machine_tokens`) — copying from it by
accident would corrupt the tenant surface. The shipped `check_arch.rs` establishes the discipline for
guarding an architectural invariant: plant a violation, assert the guard bites.

## Decision
- ONE shared constant `TENANT_TABLES` = the 10 tables (workspaces, users, workspace_memberships,
  teams, team_memberships, projects, issues, invites, comments, machine_tokens), owned by THIS
  feature, read by BOTH export and verify, written into the manifest and re-checked by verify.
- A **gold test** (acceptance suite) plants exactly one row in EACH of the 10 tenant tables for a
  target workspace, runs export, and asserts: (a) the export's per-table count reports all 10 tables
  with the planted row, and (b) verify's completeness check sees all 10. Removing a table from the
  constant reds the gold test (a planted row goes uncounted / a table goes missing from the manifest).
- The constant is DELIBERATELY distinct from `run_backup_verify`'s list and is documented as such
  (upstream-changes.md).

## Alternatives Considered
1. **Derive the table set dynamically from `information_schema` (query the DB for tables with a
   `workspace_id` column).** Rejected: `team_memberships` and `users` have NO `workspace_id`, so a
   column-presence derivation would WRONGLY omit them; and a derivation could silently include a
   future NON-tenant table. The set is a domain decision (which tables ARE tenant data), not a
   schema-shape fact — pin it explicitly.
2. **Reuse `admin_cli.rs`'s table list.** Rejected: it is a different set for a different purpose
   (whole-instance restore-verify); reusing it would silently omit `invites`/`machine_tokens` and
   include non-tenant `outbox`/`session`.
3. **A plain unit assertion of the constant's length (== 10).** Rejected: a length check does not
   catch a wrong-but-same-length set, and does not prove export+verify actually READ the constant.
   The plant-a-row-per-table gold test proves the constant is load-bearing end to end.

## Consequences
- Positive: a future tenant table added to the schema but not to the constant is caught by the gold
  test the moment a row is planted in it — no silent omission.
- Positive: mirrors the shipped `check_arch.rs` falsifiability discipline (Principle 11/12).
- Negative: the gold test must be UPDATED (plant a row in the new table) when a tenant table is added
  — but that is the point: the update is the forcing function that keeps the constant honest.
