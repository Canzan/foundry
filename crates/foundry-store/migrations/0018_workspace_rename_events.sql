-- 0018_workspace_rename_events.sql — instance-admin-workspace-rename (D5, D10;
-- DESIGN DDD-5, ADR-WORKSPACE-RENAME-001).
--
-- An append-only record of every effective workspace rename:
-- `actor · old name → new name · when`. Written in the SAME transaction as the
-- name UPDATE (`Store::rename_workspace_with_audit`), with `old_name` read under
-- the row lock, so a rename and its record commit or roll back together.
--
-- FK semantics copy 0013: the record lives and dies with its workspace (CASCADE);
-- the actor FK has no ON DELETE action, so a user on record cannot be deleted.
-- `CHECK (old_name <> new_name)` keeps "a no-op records nothing" (D4) in the
-- schema as well as in code; the compare is byte-wise, so a case-only change is
-- a real rename. `id` is an app-minted uuid v7; `created_at` is the database
-- clock.
--
-- Purely additive: no backfill, and no CHECK on `workspaces.name` (D10) — legacy
-- names longer than the new limit stay as they are. Forward-only.

CREATE TABLE workspace_rename_events (
    id            UUID PRIMARY KEY,
    workspace_id  UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    actor_id      UUID NOT NULL REFERENCES users(id),
    old_name      TEXT NOT NULL,
    new_name      TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (old_name <> new_name)
);

-- "The renames of workspace W, in order."
CREATE INDEX idx_workspace_rename_events_workspace_created
    ON workspace_rename_events (workspace_id, created_at);
