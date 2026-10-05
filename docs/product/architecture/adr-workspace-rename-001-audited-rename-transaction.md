# ADR-WORKSPACE-RENAME-001: A workspace rename and its audit row commit in one store transaction, in a dedicated append-only table

- Status: Accepted (2026-10-05)
- Feature: `instance-admin-workspace-rename` (D4, D5, D10; DESIGN DDD-1 to DDD-5)

## Context

Instance admins rename a workspace's display name from `/admin/instance/workspaces`.
Every effective rename must leave exactly one append-only record (actor, workspace,
old name, new name, time), and the rename and its record must succeed or fail
together (D5). A same-name submission is a quiet success that writes nothing and
records nothing (D4). Length is enforced in the rename path only, with no CHECK on
`workspaces.name` (D10). No generic audit table exists. The nearest precedent is
`issue_change_events` (0013): append-only, a uuid v7 id, `created_at DEFAULT now()`,
and written inside the mutation's own transaction by `Store::record_issue_change`.
Project rename (ADR-PROJECT-RENAME-002) put validation in a `foundry-services`
use-case over pool-level store calls and accepted a check-then-write race, because
its worst case was cosmetic. Here the race would corrupt the audit row's `old_name`,
so it is not acceptable.

## Decision

1. **Placement** mirrors ADR-PROJECT-RENAME-002. `foundry_services::workspaces::rename_workspace`
   re-checks `is_instance_admin`, reads the current name, and runs a pure ordered
   classifier: trim, then no-op, then empty, then more than 24 Unicode scalars.
   On a write it delegates to the store. The handler owns the copy and the uniform 404.
2. **Atomic write.** `Store::rename_workspace_with_audit(workspace_id, actor_id, new_name)`
   opens one transaction, runs `SELECT name … FOR UPDATE`, and returns `NotFound`
   when the workspace is absent. If the locked name equals `new_name` it returns
   `Unchanged` and writes nothing. Otherwise it updates `workspaces.name`, inserts
   one `workspace_rename_events` row with `old_name` taken from the **locked** read,
   commits, and returns `Renamed{old_name}`. The use-case's pre-read exists only so
   D4's no-op takes precedence over the 422 gates. It never decides a write.
3. **Schema (migration 0018).** The new table is `workspace_rename_events`:
   - `id UUID PK` (app-minted v7)
   - `workspace_id … REFERENCES workspaces(id) ON DELETE CASCADE`
   - `actor_id … REFERENCES users(id)` (no ON DELETE action)
   - `old_name TEXT NOT NULL`, `new_name TEXT NOT NULL`
   - `created_at TIMESTAMPTZ NOT NULL DEFAULT now()`
   - `CHECK (old_name <> new_name)`
   - index `(workspace_id, created_at)`

   The migration is additive only. No code path updates or deletes a row.
4. `Store::probe` refuses a schema without the table, so `/readyz` fails instead of the rename returning a 500.

## Alternatives

- **A generic `instance_admin_audit(kind, payload jsonb)` table.** Rejected. One event
  kind does not justify an untyped schema, and project-rename auditing is explicitly
  out of scope. A generic table can be introduced later with these rows migrated into
  it if a second kind arrives.
- **A single-statement CTE** (lock, update and insert in one `WITH` query). Correct and
  atomic, but opaque to review and to the mutation gate, and it breaks from the shipped
  multi-statement transaction idiom (`reposition_issue_with_outbox`). Rejected for
  clarity.
- **A DB trigger on `workspaces` that writes the audit row.** Rejected. The trigger
  cannot know the actor without a per-transaction GUC, and the behaviour would be
  hidden from Rust code and tests.
- **A transaction handle driven from `foundry-services`.** Rejected. It leaks
  `sqlx::Transaction` across the store port.
- **Check-then-write without a lock** (the project precedent). Rejected. Two
  concurrent renames could both record the same `old_name`.

## Consequences

- Positive: KPI-2 ("one entry per effective rename, none for no-ops") holds by
  construction, enforced twice: by code and by the `old <> new` CHECK.
- Positive: there is a natural fault-injection seam for the atomicity proof. A
  non-existent `actor_id` fails the INSERT after the UPDATE, and the transaction rolls
  the name back (a store-level test).
- Negative: the actor FK has no ON DELETE action, so deleting a user who has renamed
  a workspace is refused. No user-delete path exists today. A future user-deletion
  feature must decide between keeping the audit and anonymizing it.
- Negative: deleting a workspace cascades its rename history. No workspace deletion
  exists, and this matches 0013. It must be revisited if workspace deletion or
  archival lands.
