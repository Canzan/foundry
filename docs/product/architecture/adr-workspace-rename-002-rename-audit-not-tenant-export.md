# ADR-WORKSPACE-RENAME-002: Workspace rename audit rows are instance records, excluded from the per-workspace export

- Status: Accepted (2026-10-05)
- Feature: `instance-admin-workspace-rename` (closes DISCUSS OQ-3; DESIGN DDD-6)

## Context

`workspace_rename_events` (ADR-WORKSPACE-RENAME-001) is keyed by `workspace_id` but
written from the instance surface by an instance admin. The per-workspace export
(`Store::export_workspace`, per-workspace-backup ADR-003/005) walks exactly the ten
`foundry_store::TENANT_TABLES`. `verify-export` re-applies the same scope predicate
offline. It checks completeness against all ten tables and isolation, including the
check that every `users` row in the archive is a member of the declared workspace.
Tests pin the "ten" invariant: the `export_workspace_gold` plant-a-row-per-table test,
the `verify_export` completeness check, the CLI manifest `tenant_tables`, and the
per-workspace-backup acceptance steps. No per-workspace import or restore exists;
restore is a whole-instance `pg_restore`.

## Decision

Do **not** add `workspace_rename_events` to `TENANT_TABLES`. The per-workspace export,
`verify-export` and their tests are unchanged, and the archive stays at ten tables.
The rows are covered by the whole-instance `pg_dump` backup, which includes every
table without code changes. `admin_cli::KNOWN_FOUNDRY_TABLES`, the row-count report
of the whole-instance backup-verify, is also unchanged, consistent with 0013.

## Alternatives

- **Add an 11th tenant table** (predicate `workspace_id = W`). Rejected. The actor is
  normally an instance admin who is not a workspace member, so `actor_id` would dangle
  inside the archive, which is a referential-closure violation. Fixing that would mean
  widening the membership-bounded `users` predicate, which leaks a non-member's user
  row into a tenant's archive. That is the isolation property per-workspace-backup
  exists to guarantee. It would also touch every "ten" test for a record no tenant can
  see.
- **Export the rows in a separate, unverified section of the archive.** Rejected. It
  creates a second archive shape that verify does not cover. An unverified section is
  a weaker guarantee than leaving the rows out, and nothing restores it anyway.

## Consequences

- Positive: the tenant isolation and completeness contracts, and their tests, are
  untouched.
- Positive: this is consistent with every audit or auxiliary table added after
  per-workspace-backup (`issue_change_events`, `notification_unsubscribes`,
  `project_lanes`), none of which is in `TENANT_TABLES`.
- Negative: a per-workspace archive does not carry the workspace's rename history.
  Its manifest `declared_workspace_name` shows the name at export time (D8), and the
  history survives only in whole-instance backups. This is accepted, because rename
  history is operator accountability, not tenant content.
- Follow-up trigger: if a per-workspace import or restore is ever built, or the
  members of a workspace are given a viewer for its rename history, revisit this
  decision together with the actor-membership problem.
