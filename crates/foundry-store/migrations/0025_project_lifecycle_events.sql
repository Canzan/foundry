-- 0025_project_lifecycle_events.sql — project-lifecycle slice 01 (DISCUSS OQ-3;
-- DESIGN DDD-13, DDD-14, DDD-28, DDD-29; ADR-PROJECT-LIFECYCLE-003 decision 1).
--
-- One append-only record per effective project rename (`renamed`: actor · old →
-- new · when), per project delete (`deleted`: actor · name · slug · key · when)
-- and per restore (`restored`: the operator, so no actor · name · when), written
-- in the SAME transaction as the change it records. All three kinds are in the
-- CHECK from the start, so slices 02 and 03 need no change to this table.
--
-- `project_id` and `team_id` have NO foreign key: a record must outlive the
-- project and the team it names (a psql cascade, a team delete), and the ids
-- still correlate the records about one project. The slug and key are carried
-- as text. The record lives and dies with its workspace (CASCADE); the actor FK
-- has no ON DELETE action, as 0018 and 0023. `actor_id` is NULL only for
-- `restored` (the operator through `foundry doctor`).
-- `project_lifecycle_events_kind_shape` keeps each kind coherent in the schema
-- as well as in code: a rename has an actor and names a different new name
-- (byte-wise, so a case-only change is a real rename), a delete has an actor
-- and names none, a restore has no actor and names none.
--
-- Not a tenant table: kept out of TENANT_TABLES and the per-workspace export
-- (ADR-WORKSPACE-RENAME-002); present in every whole-instance pg_dump.
--
-- Purely additive: a new, empty table and its two indexes; no existing row is
-- read or written. Re-runnable body (IF NOT EXISTS, and the CHECK dropped
-- before it is added). Undo (documented in CHANGELOG.md):
--   DROP TABLE IF EXISTS project_lifecycle_events;

CREATE TABLE IF NOT EXISTS project_lifecycle_events (
    id            UUID PRIMARY KEY,
    workspace_id  UUID NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    project_id    UUID NOT NULL,
    team_id       UUID NOT NULL,
    project_slug  TEXT NOT NULL,
    key_prefix    TEXT NOT NULL,
    kind          TEXT NOT NULL CHECK (kind IN ('renamed', 'deleted', 'restored')),
    actor_id      UUID NULL REFERENCES users(id),
    name          TEXT NOT NULL,
    new_name      TEXT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE project_lifecycle_events
    DROP CONSTRAINT IF EXISTS project_lifecycle_events_kind_shape;
ALTER TABLE project_lifecycle_events
    ADD CONSTRAINT project_lifecycle_events_kind_shape CHECK (
        (kind = 'renamed' AND actor_id IS NOT NULL AND new_name IS NOT NULL AND new_name <> name)
        OR (kind = 'deleted' AND actor_id IS NOT NULL AND new_name IS NULL)
        OR (kind = 'restored' AND actor_id IS NULL AND new_name IS NULL)
    );

-- "The project records of workspace W, in order."
CREATE INDEX IF NOT EXISTS idx_project_lifecycle_events_workspace_created
    ON project_lifecycle_events (workspace_id, created_at);

-- "The records about project P, in order."
CREATE INDEX IF NOT EXISTS idx_project_lifecycle_events_project_created
    ON project_lifecycle_events (project_id, created_at);

-- Self-check at apply time: the table carries every column the store writes,
-- the shape CHECK is installed, and no foreign key points at `projects` or
-- `teams`, or the migration is refused.
DO $self_check$
DECLARE
    column_count integer;
    has_shape boolean;
    links_projects_or_teams boolean;
BEGIN
    SELECT count(*) INTO column_count
      FROM information_schema.columns
     WHERE table_schema = current_schema()
       AND table_name = 'project_lifecycle_events'
       AND column_name IN ('id', 'workspace_id', 'project_id', 'team_id', 'project_slug',
                           'key_prefix', 'kind', 'actor_id', 'name', 'new_name',
                           'created_at');
    SELECT EXISTS (SELECT 1 FROM pg_constraint
                    WHERE conname = 'project_lifecycle_events_kind_shape'
                      AND contype = 'c'
                      AND conrelid = to_regclass('project_lifecycle_events'))
      INTO has_shape;
    SELECT EXISTS (SELECT 1 FROM pg_constraint
                    WHERE contype = 'f'
                      AND conrelid = to_regclass('project_lifecycle_events')
                      AND confrelid IN (to_regclass('projects'), to_regclass('teams')))
      INTO links_projects_or_teams;
    IF column_count <> 11 OR NOT has_shape OR links_projects_or_teams THEN
        RAISE EXCEPTION 'migration 0025 self-check: columns % of 11, shape check %, link to projects or teams %',
            column_count, has_shape, links_projects_or_teams
            USING ERRCODE = 'feature_not_supported';
    END IF;
END
$self_check$;
