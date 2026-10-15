-- 0027_project_detach.sql — project-lifecycle slice 04 (DISCUSS OQ-1 (b);
-- DESIGN DDD-19, DDD-28, DDD-29; ADR-PROJECT-LIFECYCLE-004).
--
-- The detach: a deleted project can outlive its team. `projects.team_id`
-- becomes nullable and `projects.detached_from_team_id` (no foreign key, so it
-- outlives the team it names) records the team a detached project belonged to.
-- The CHECK `projects_team_or_detached` keeps the two exclusive: a project
-- either has its team and names none, or it is deleted, has no team and names
-- the team it was detached from. A live project can therefore never lose its
-- team: a missed live row is a loud 23514, never a silent cascade.
-- `projects_team_id_fkey` stays `ON DELETE CASCADE` (the rejected `SET NULL`
-- breaks workspace delete by firing order).
--
-- STANDING RULE (0026): a migration that adds a `projects` column recreates
-- `live_projects` in the same file. `SELECT *` is expanded when the view is
-- created; the new column is appended last to `projects`, so the view's old
-- columns are a prefix of the new list and `CREATE OR REPLACE` appends it. The
-- self-check below refuses the migration while the two column lists differ.
--
-- Purely additive: one NULL column (every existing project names no team it
-- was detached from), one dropped NOT NULL, one CHECK every existing row
-- satisfies (each has its team), and the view recreated with the same rows; no
-- existing row is read or written. Re-runnable body (IF NOT EXISTS, the CHECK
-- dropped before it is added, CREATE OR REPLACE). Undo (documented in
-- CHANGELOG.md), only while no project is detached:
--   ALTER TABLE projects DROP CONSTRAINT IF EXISTS projects_team_or_detached;
--   ALTER TABLE projects ALTER COLUMN team_id SET NOT NULL;
--   DROP VIEW IF EXISTS live_projects;
--   ALTER TABLE projects DROP COLUMN IF EXISTS detached_from_team_id;
--   CREATE VIEW live_projects AS SELECT * FROM projects WHERE deleted_at IS NULL;

ALTER TABLE projects
    ADD COLUMN IF NOT EXISTS detached_from_team_id UUID NULL;

ALTER TABLE projects
    ALTER COLUMN team_id DROP NOT NULL;

ALTER TABLE projects
    DROP CONSTRAINT IF EXISTS projects_team_or_detached;
ALTER TABLE projects
    ADD CONSTRAINT projects_team_or_detached
    CHECK (
        (team_id IS NOT NULL AND detached_from_team_id IS NULL)
        OR (team_id IS NULL AND detached_from_team_id IS NOT NULL AND deleted_at IS NOT NULL)
    );

CREATE OR REPLACE VIEW live_projects AS SELECT * FROM projects WHERE deleted_at IS NULL;

-- Self-check at apply time: the detach column and its CHECK are installed,
-- the team foreign key still cascades, and the view has exactly the columns of
-- `projects`, by name, type and order, or the migration is refused naming the
-- view.
DO $self_check$
DECLARE
    detach_count integer;
    has_check boolean;
    fk_cascades boolean;
    table_columns text;
    view_columns text;
BEGIN
    SELECT count(*) INTO detach_count
      FROM information_schema.columns
     WHERE table_schema = current_schema()
       AND table_name = 'projects'
       AND column_name = 'detached_from_team_id';
    SELECT EXISTS (SELECT 1 FROM pg_constraint
                    WHERE conname = 'projects_team_or_detached'
                      AND contype = 'c'
                      AND conrelid = to_regclass('projects'))
      INTO has_check;
    SELECT EXISTS (SELECT 1 FROM pg_constraint
                    WHERE conname = 'projects_team_id_fkey'
                      AND contype = 'f'
                      AND confdeltype = 'c'
                      AND conrelid = to_regclass('projects'))
      INTO fk_cascades;
    SELECT string_agg(column_name || ' ' || data_type, ', ' ORDER BY ordinal_position)
      INTO table_columns
      FROM information_schema.columns
     WHERE table_schema = current_schema() AND table_name = 'projects';
    SELECT string_agg(column_name || ' ' || data_type, ', ' ORDER BY ordinal_position)
      INTO view_columns
      FROM information_schema.columns
     WHERE table_schema = current_schema() AND table_name = 'live_projects';
    IF detach_count <> 1 OR NOT has_check OR NOT fk_cascades THEN
        RAISE EXCEPTION 'migration 0027 self-check: detach column % of 1, team-or-detached check %, team fk cascades %',
            detach_count, has_check, fk_cascades
            USING ERRCODE = 'feature_not_supported';
    END IF;
    IF view_columns IS DISTINCT FROM table_columns THEN
        RAISE EXCEPTION 'migration 0027 self-check: live_projects columns (%) differ from projects columns (%)',
            coalesce(view_columns, 'none'), table_columns
            USING ERRCODE = 'feature_not_supported';
    END IF;
END
$self_check$;
