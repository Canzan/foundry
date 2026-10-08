# Slice 03: Restoring a backup that holds legacy names still works, and the upgrade can be rolled back

Story: US-NDC-03 | Estimate: 0.5 day | job_id: `job-name-rule-below-the-app`

## Goal

A whole-instance backup that holds names that break the rule (taken before or
after the upgrade) restores with `pg_restore --clean --if-exists`, and
`foundry doctor backup-verify` accepts it. A rollback to v0.11.0 boots and serves
against the migrated database. CHANGELOG migration notes tell the operator what
changed, how to count legacy names, and how to undo the change.

## IN

- A restore scenario on the us-03 harness: a post-migration dump holding "Canzan Labs Platform Engineering Group" and "Homelab\tOps" restores, and both rows render. The constraints are back and are still not validated (D9).
- A pre-migration dump (schema at 0018) restores, and the next boot applies the new migration(s) (D9, D10).
- `backup-verify` on the same dump exits 0.
- A rollback check: the v0.11.0 binary boots against the migrated database and creates a workspace and a project with valid names (D10). Reuse the us-04 rolling-upgrade harness if it fits (DESIGN).
- CHANGELOG `[Unreleased]` migration notes: what is added, that no row is scanned or rewritten, the lock taken, rolling-deploy safety, the rollback floor (v0.11.0), the read-only legacy-count query, VALIDATE as a later step, and DROP CONSTRAINT as the undo.

## OUT

- VALIDATE. A `foundry doctor` legacy report. A per-workspace import (none exists).

## Learning Hypothesis

- **Disproves if it fails**: that `pg_dump` puts not-yet-validated name rules after the data, so COPY loads legacy rows first. If the restore fails on a legacy row, every operator with a legacy name has lost the ability to restore. The rule's shape must then change before release.
- **Confirms if it succeeds**: that the feature adds no restore-time or rollback-time hazard, so it can ship without a pre-upgrade data repair.

## Acceptance Criteria

- [ ] A post-migration dump holding legacy names restores with exit 0. The legacy names are byte-identical after the restore. A `psql` write of a bad name is still refused afterwards.
- [ ] A pre-migration dump restores. After the next boot, the name rule is present and the legacy rows are untouched.
- [ ] `foundry doctor backup-verify` exits 0 on the legacy-holding dump.
- [ ] v0.11.0 boots against the migrated database and serves `/readyz` 200.
- [ ] CHANGELOG migration notes cover every item in IN.

## Dependencies

- Slices 01 and 02 (the rules being restored). It can run after slice 01 alone if OQ-1 drops slice 02.
- The us-03 harness needs Docker (`FOUNDRY_XTASK_INCLUDE_DOCKER=1`, repo memory).
