-- crates/foundry-store/migrations/0011_instance_admins.sql
-- multi-workspace-provisioning — Slice 5/6 (step 01-01): the instance-level
-- super-admin role table (ADR-003, D3/D6).
--
-- OD-3 ratified a NEW instance-level super-admin ABOVE workspace-admin. The only
-- prior roles are per-workspace (`workspace_memberships.role`, 0001_init.sql:29);
-- there is no instance-level role. The provisioning use-case (ADR-002) needs a
-- single, queryable, auditable authz gate that stays OFF the tenant-scoped
-- boundary: a super-admin is NOT a workspace member.
--
-- Representation (ADR-003 option a): an explicit `instance_admins(user_id)` table
-- mirroring the relational membership model already in use — `is_instance_admin`
-- is then `EXISTS (SELECT 1 FROM instance_admins WHERE user_id=$1)`, fail-closed
-- (absent row ⇒ refused), mirroring the shipped `is_workspace_admin` idiom.
--
-- FK conventions mirror `workspace_memberships`/`users` (0001_init.sql): a UUID
-- key, `REFERENCES users(id) ON DELETE CASCADE` (a deleted user drops their
-- super-admin grant), and `created_at TIMESTAMPTZ NOT NULL DEFAULT now()` for
-- audit. `user_id` is the PRIMARY KEY (a user is a super-admin at most once).
--
-- Forward-only (ADR-006/ADR-004); purely ADDITIVE — it creates one EMPTY table
-- and rewrites no prior row. Idempotent via `CREATE TABLE IF NOT EXISTS` so
-- re-running the migration set (the slice-05 re-upgrade guarantee) and fresh
-- schemas both succeed. Empty until a super-admin is seeded (bootstrap claim /
-- grant-super-admin, later slice-06 steps).

CREATE TABLE IF NOT EXISTS instance_admins (
    user_id     UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
