# ADR-TEAM-LIFECYCLE-003: Team renames and deletes are recorded in one workspace-scoped, append-only table that outlives the team and stays out of the per-workspace export; a rename is a locked, audited, name-only write

- Status: Accepted (2026-10-11)
- Feature: `team-lifecycle` (DISCUSS D4, D15, OQ-6, INV-TL-5, INV-TL-9; DESIGN DDD-11, DDD-12, DDD-14)
- Follows: ADR-WORKSPACE-RENAME-001 (audited rename transaction), ADR-WORKSPACE-RENAME-002 (audit rows are not tenant export)

## Context

OQ-6 (user): a rename records actor · old → new · when, and a delete records actor · name · slug ·
when. Each record is written in the same transaction as its write, and it must **outlive the team**
(INV-TL-9). There is no UI for the records: they are read with psql (or a whole-instance backup).
Create records nothing.

The precedent `workspace_rename_events` (0018) cascades with its workspace, and its old name comes
from a `FOR UPDATE` read inside the rename transaction. That is the reason ADR-WORKSPACE-RENAME-001
rejected an unlocked check-then-write: two concurrent renames would both record the same
`old_name`. ADR-WORKSPACE-RENAME-002 kept that table out of `TENANT_TABLES` (fixed at ten, pinned by
`export_workspace_gold`, `verify_export`, the manifest and the acceptance steps). Its reason was that
the actor's `users` row may not be inside the archive's membership-bounded closure.

## Decision

1. **One table, migration `0023_team_lifecycle_events.sql` (slice 02, additive):**
   - `id UUID PRIMARY KEY` (app-minted v7)
   - `workspace_id UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE`
   - `team_id UUID NOT NULL` (**no FK**: the record outlives the row it names, and the id still
     correlates records about one team)
   - `team_slug TEXT NOT NULL`
   - `kind TEXT NOT NULL CHECK (kind IN ('renamed', 'deleted'))`
   - `actor_id UUID NOT NULL REFERENCES users(id)` (no ON DELETE action, as 0018)
   - `name TEXT NOT NULL` (the name before the event)
   - `new_name TEXT NULL`
   - `created_at TIMESTAMPTZ NOT NULL DEFAULT now()`
   - `CONSTRAINT team_lifecycle_events_kind_shape CHECK ((kind = 'renamed' AND new_name IS NOT NULL
     AND new_name <> name) OR (kind = 'deleted' AND new_name IS NULL))`
   - index `(workspace_id, created_at)`

   No code path updates or deletes a row. The record carries the name and slug as text, so it
   needs nothing from `teams`.
2. **Rename: `Store::rename_team_with_audit(workspace_id, team_id, actor_id, new_name) ->
   TeamRenameWrite { Renamed { old_name } | Unchanged | NotFound }`.** One transaction:
   1. `SELECT name, slug FROM teams WHERE id = $team AND workspace_id = $ws FOR NO KEY UPDATE`.
      No row: `NotFound`.
   2. A byte-equal name returns `Unchanged` and writes nothing (D15; a case-only change is a
      rename).
   3. `UPDATE teams SET name = $new WHERE id = $team`. Name only: the slug is never written (D4).
   4. `INSERT … kind = 'renamed', name = <locked old name>, new_name`.
   5. Commit.

   `FOR NO KEY UPDATE` serialises concurrent renames, so each record's `name` is the value under the
   lock. It does not block member adds or project creates (FK `KEY SHARE`). It conflicts with the
   delete's `FOR UPDATE`, so a rename and a delete are totally ordered. If the rename goes first,
   the delete's record carries the new name. If the delete goes first, the rename gets `NotFound`
   and the uniform 404.
3. **Delete** writes `kind = 'deleted', name, team_slug` from its locked read, in its transaction
   (ADR-TEAM-LIFECYCLE-002 statement 4).
4. **Uniqueness stays check-then-write (INV-TL-5).** The use-case reads the siblings, without self,
   before calling the store. A concurrent create or rename can still leave two case-insensitively
   equal names. That residual is accepted as in ADR-PROJECT-RENAME-002, because identity is the slug
   and no rename touches it. The record's old name is exact regardless.
5. **Not in `TENANT_TABLES`.** The per-workspace export, `verify-export` and their "ten" tests are
   unchanged. `admin_cli::KNOWN_FOUNDRY_TABLES` is unchanged (consistent with 0013 and 0018). The
   rows are in every whole-instance `pg_dump`.
6. **Probe.** From slice 02, `Store::probe` refuses readiness without the table's columns (the
   0018 precedent), because delete and rename write it inside their transaction.

## Alternatives considered

- **`team_id … REFERENCES teams(id) ON DELETE SET NULL`.** The record survives, but loses its
  correlation id at the moment it matters, and the cascade would update an append-only record.
  **`ON DELETE CASCADE`** destroys the delete record with its team, which breaks INV-TL-9.
- **Two tables (`team_rename_events`, `team_delete_events`), the 0018 shape per kind.** Each is
  equally typed, but it doubles the migration, probe and test surface for two kinds that share
  every column except `new_name`. The `kind` CHECK keeps the one table typed. This is not the
  untyped `jsonb` table that ADR-WORKSPACE-RENAME-001 rejected.
- **Add the table to `TENANT_TABLES`.** Rejected, as in ADR-WORKSPACE-RENAME-002. The actor may since
  have left the workspace, so `actor_id` would dangle inside the archive's membership-bounded `users`
  closure. Widening that closure leaks a non-member into a tenant archive. No per-workspace restore
  exists to consume the rows.
- **Record from a DB trigger on `teams`.** Rejected, as in ADR-WORKSPACE-RENAME-001: the trigger
  cannot know the actor without a GUC, and it would hide the write from the Rust code and tests.
- **Rename without a lock (the ADR-PROJECT-RENAME-002 posture).** Rejected: it would corrupt the
  record's old name under concurrency.

## Consequences

- Positive: every effective rename and every delete leaves exactly one record, atomically, enforced
  twice (by code and by the shape CHECK). Records outlive teams. Tenant isolation and export
  contracts are untouched.
- Negative: a per-workspace archive does not carry team history (accepted, as for workspace
  renames). A workspace delete, which does not exist, would cascade it.
- Negative: the actor FK has no ON DELETE action, so a future user hard-delete meets it, alongside
  0018 and 0021 (team-membership OQ-D9). That feature decides for all three.
- Earned Trust (store tests on `postgres:16`):
  - A rename whose record INSERT fails (an unknown actor) rolls the name back.
  - Two concurrent renames record two distinct, chained old names.
  - A same-name submission writes no record.
  - A delete record survives the team row.
  - The shape CHECK refuses `renamed` with an equal name and `deleted` with a `new_name`.
