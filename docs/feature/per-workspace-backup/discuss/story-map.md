# Story Map: per-workspace-backup

## User: Devansh — the self-hosting operator

## Goal: lift exactly one workspace's data out of a multi-tenant instance into a portable, verifiable, isolation-clean archive (to archive / migrate / hand off / pre-deletion-snapshot just that tenant)

## Scope boundary

EXPORT ONLY. Per-workspace RESTORE/import is OUT of v1 (DD-MWT-09 — a per-workspace restore must
not clobber a sibling; isolation-sensitive write path, deferred follow-up). The walking skeleton IS
the export happy-path.

## Backbone

| A. Identify target            | B. Export the workspace               | C. Verify the archive                 |
|-------------------------------|---------------------------------------|---------------------------------------|
| A.1 List workspaces (id+slug) | B.1 Scoped logical export (happy path)| C.1 Completeness check (all tables)   |
| A.2 Resolve id-or-slug arg    | B.2 Isolation: only this tenant's rows| C.2 Isolation check (no sibling rows) |
|                               | B.3 Per-table row-count report        | C.3 Readable/truncation check         |
|                               | B.4 Unknown-workspace refusal (exit 2)| C.4 Sensitivity note surfaced         |
|                               | B.5 Output-path / partial-write safety|                                       |
|                               | B.6 Sole/last-workspace is valid      |                                       |

---

### Walking Skeleton (thinnest end-to-end slice)

The minimum that connects all three activities and delivers a verifiable, isolation-clean archive:

- **A.1** List workspaces so the operator can name the target (id or slug).
- **B.1 + B.2 + B.3** Run a scoped logical export that writes an archive containing ONLY the target
  workspace's rows across the 10 tenant tables, printing per-table counts and ending `status: OK`.
- **C.1 + C.2** Verify the archive: all tenant tables present (completeness) AND every row belongs to
  the declared workspace with no sibling rows (isolation — the crux).

This skeleton is exactly the task's "export happy-path = the thin slice", with isolation baked in
from the first slice because isolation is the security-critical reason the feature exists (a leaky
export is worse than no export).

### Release 1 (= Walking Skeleton): "Operator can produce a trustworthy single-tenant archive"

- Tasks: A.1, A.2, B.1, B.2, B.3, C.1, C.2.
- Target outcome (KPI-1, KPI-2): operator extracts one tenant's data end-to-end and machine-confirms
  it is complete and isolation-clean, with zero sibling leakage.
- Rationale: validates the riskiest assumption (a scoped logical export can be both COMPLETE across
  the transitive-FK tenant surface AND provably isolation-clean) on the very first slice. Without
  this, nothing else matters.

### Release 2: "Operator is never surprised or burned by a failed/edge export"

- Tasks: B.4 (unknown workspace -> exit 2 + guidance), B.5 (output-path errors + atomic partial-write
  safety), B.6 (sole/last workspace valid, read-only), C.3 (truncation/readability), C.4 (at-rest
  sensitivity note).
- Target outcome (KPI-3): every failure path exits with a documented code and an actionable message;
  no partial archive can masquerade as complete; the operator is warned about sensitive at-rest
  contents.
- Rationale: hardens the happy path against the operator's real-world failure modes (typo'd
  workspace, full disk, single-tenant install). These are the "sad paths" that turn a demo into a
  dependable tool. They build on Release 1 (they decorate the same export/verify commands) so they
  cannot precede it.

## Priority Rationale

1. **Walking skeleton first** because the riskiest assumption is technical-and-security: that a
   *transitive-FK* scoped export can be simultaneously complete (no tenant table silently omitted)
   and isolation-clean (no sibling row leaks). Proving this end-to-end on slice 1 de-risks everything.
2. **Isolation is IN the skeleton, not deferred** — a leaky export is a security incident, not a
   missing nicety. The crux invariant (B.2 + C.2) ships with the first usable archive or the archive
   is not usable.
3. **Failure-path hardening is Release 2** — high value but strictly dependent on the happy-path
   commands existing first. Ordered by operator-burn risk: silent-incomplete (partial write) and
   wrong-target (unknown workspace) rank highest within R2 because they corrupt the trust the feature
   sells.
4. **Per-workspace restore is explicitly Won't-Have (v1)** — deferred by DD-MWT-09; tracked, not
   dropped.

> Story IDs (US-PWB-NN) are assigned in `requirements.md` / `user-stories.md`. See
> `prioritization.md` for the scored backlog.
