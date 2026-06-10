-- crates/foundry-store/migrations/0010_active_workspace.sql
-- multi-workspace-tenancy — Slice 2 (step 02-05): persisted ACTIVE workspace for
-- the multi-membership switcher (ADR-005, OD-2).
--
-- A contractor who is a member of MORE than one workspace acts on EXACTLY one at a
-- time — their ACTIVE workspace. The web `/workspace/switch` action re-points this
-- so a subsequent request (even a fresh sign-in) scopes to the switched tenant.
-- We persist the choice on `users` (nullable) rather than in the thin session row,
-- so the active workspace survives session rotation exactly like memberships do
-- (design/auth.md: keep session data thin).
--
-- `ON DELETE SET NULL`: if the active workspace is ever removed, the column falls
-- back to NULL and `resolve_active_workspace` reverts to its deterministic
-- lowest-id membership default — never dangling, never a foreign tenant.
--
-- NOTE: membership is NOT enforced by a DB constraint here — that privilege guard
-- lives in `Store::set_active_workspace`, which refuses (fail-closed, no write) to
-- set an active workspace the user is not a member of. `resolve_active_workspace`
-- additionally honours the column ONLY when a matching membership still exists, so
-- a stale value can never scope a user to a tenant they no longer belong to.
--
-- Forward-only (ADR-006); idempotent via `IF NOT EXISTS` so re-runs and fresh
-- schemas both succeed.

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS active_workspace_id UUID
        REFERENCES workspaces(id) ON DELETE SET NULL;
