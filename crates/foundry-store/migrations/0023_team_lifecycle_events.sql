-- 0023_team_lifecycle_events.sql — team-lifecycle slice 02 (DISCUSS OQ-6,
-- INV-TL-9; DESIGN DDD-11, DDD-12; ADR-TEAM-LIFECYCLE-003).
--
-- One append-only record per team rename (`renamed`: actor · old → new · when)
-- and per team delete (`deleted`: actor · name · slug · when), written in the
-- SAME transaction as the write it records (`Store::delete_team_if_empty`,
-- later `Store::rename_team_with_audit`).
--
-- `team_id` has NO foreign key to `teams`: a delete record must outlive the row
-- it names, and the id still correlates the records about one team. The name
-- and slug are carried as text. The record lives and dies with its workspace
-- (CASCADE); the actor FK has no ON DELETE action, as 0018.
-- `team_lifecycle_events_kind_shape` keeps each kind coherent in the schema as
-- well as in code: a rename names a different new name (byte-wise, so a
-- case-only change is a real rename), a delete names none.
--
-- Not a tenant table: kept out of TENANT_TABLES and the per-workspace export
-- (ADR-WORKSPACE-RENAME-002); present in every whole-instance pg_dump.
--
-- Purely additive: a new, empty table and its index; no existing row is read
-- or written. Re-runnable body (IF NOT EXISTS, and the CHECK dropped before it
-- is added). Undo (documented in CHANGELOG.md):
--   DROP TABLE IF EXISTS team_lifecycle_events;

CREATE TABLE IF NOT EXISTS team_lifecycle_events (
    id            UUID PRIMARY KEY,
    workspace_id  UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    team_id       UUID NOT NULL,
    team_slug     TEXT NOT NULL,
    kind          TEXT NOT NULL CHECK (kind IN ('renamed', 'deleted')),
    actor_id      UUID NOT NULL REFERENCES users(id),
    name          TEXT NOT NULL,
    new_name      TEXT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE team_lifecycle_events
    DROP CONSTRAINT IF EXISTS team_lifecycle_events_kind_shape;
ALTER TABLE team_lifecycle_events
    ADD CONSTRAINT team_lifecycle_events_kind_shape CHECK (
        (kind = 'renamed' AND new_name IS NOT NULL AND new_name <> name)
        OR (kind = 'deleted' AND new_name IS NULL)
    );

-- "The team records of workspace W, in order."
CREATE INDEX IF NOT EXISTS idx_team_lifecycle_events_workspace_created
    ON team_lifecycle_events (workspace_id, created_at);

-- Self-check at apply time: the table carries every column the store writes,
-- the shape CHECK is installed, and no foreign key points at `teams`, or the
-- migration is refused.
DO $self_check$
DECLARE
    column_count integer;
    has_shape boolean;
    links_teams boolean;
BEGIN
    SELECT count(*) INTO column_count
      FROM information_schema.columns
     WHERE table_schema = current_schema()
       AND table_name = 'team_lifecycle_events'
       AND column_name IN ('id', 'workspace_id', 'team_id', 'team_slug', 'kind',
                           'actor_id', 'name', 'new_name', 'created_at');
    SELECT EXISTS (SELECT 1 FROM pg_constraint
                    WHERE conname = 'team_lifecycle_events_kind_shape'
                      AND contype = 'c'
                      AND conrelid = to_regclass('team_lifecycle_events'))
      INTO has_shape;
    SELECT EXISTS (SELECT 1 FROM pg_constraint
                    WHERE contype = 'f'
                      AND conrelid = to_regclass('team_lifecycle_events')
                      AND confrelid = to_regclass('teams'))
      INTO links_teams;
    IF column_count <> 9 OR NOT has_shape OR links_teams THEN
        RAISE EXCEPTION 'migration 0023 self-check: columns % of 9, shape check %, link to teams %',
            column_count, has_shape, links_teams
            USING ERRCODE = 'feature_not_supported';
    END IF;
END
$self_check$;
