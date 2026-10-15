-- 0026_project_soft_delete.sql — project-lifecycle slice 02 (DISCUSS D-A;
-- DESIGN DDD-2, DDD-3, DDD-28, DDD-29; ADR-PROJECT-LIFECYCLE-001,
-- ADR-PROJECT-LIFECYCLE-002 decision 1).
--
-- The deleted marker: `projects.deleted_at` and `projects.deleted_by` (the
-- deleting user; no ON DELETE action, as `comments.deleted_by` in 0006). The
-- CHECK `projects_deleted_marker_shape` keeps it whole: both parts or neither.
-- A deleted project keeps every row under it; only the marker hides it.
--
-- `live_projects` is the view every project reader goes through: the projects
-- whose marker is empty, with exactly the table's columns in the table's order
-- (`SELECT *` is expanded when the view is created, so a column added to
-- `projects` later does NOT reach the view). STANDING RULE: any later migration
-- that adds a `projects` column recreates `live_projects` in the same file, and
-- the self-check below refuses the migration while the two column lists differ.
-- The view is simple, so it is auto-updatable and takes row locks through to
-- `projects` (the write guard's `FOR KEY SHARE`, DDD-7).
--
-- Additive: two NULL columns (every existing project stays live), one CHECK
-- every existing row satisfies, and one view. ADD CONSTRAINT ... CHECK
-- validates every existing projects row: it scans the whole table and holds an
-- ACCESS EXCLUSIVE lock on projects for that scan (rows are read, never
-- modified). Re-runnable body (IF NOT EXISTS, the CHECK dropped before it is
-- added, CREATE OR REPLACE). Undo (documented in CHANGELOG.md), only while no
-- project is deleted:
--   DROP VIEW IF EXISTS live_projects;
--   ALTER TABLE projects DROP CONSTRAINT IF EXISTS projects_deleted_marker_shape,
--       DROP COLUMN IF EXISTS deleted_by, DROP COLUMN IF EXISTS deleted_at;

ALTER TABLE projects
    ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ NULL,
    ADD COLUMN IF NOT EXISTS deleted_by UUID NULL REFERENCES users(id);

ALTER TABLE projects
    DROP CONSTRAINT IF EXISTS projects_deleted_marker_shape;
ALTER TABLE projects
    ADD CONSTRAINT projects_deleted_marker_shape
    CHECK ((deleted_at IS NULL) = (deleted_by IS NULL));

CREATE OR REPLACE VIEW live_projects AS SELECT * FROM projects WHERE deleted_at IS NULL;

-- Self-check at apply time: the marker and its CHECK are installed, and the
-- view has exactly the columns of `projects`, by name, type and order, or the
-- migration is refused naming the view.
DO $self_check$
DECLARE
    marker_count integer;
    has_shape boolean;
    table_columns text;
    view_columns text;
BEGIN
    SELECT count(*) INTO marker_count
      FROM information_schema.columns
     WHERE table_schema = current_schema()
       AND table_name = 'projects'
       AND column_name IN ('deleted_at', 'deleted_by');
    SELECT EXISTS (SELECT 1 FROM pg_constraint
                    WHERE conname = 'projects_deleted_marker_shape'
                      AND contype = 'c'
                      AND conrelid = to_regclass('projects'))
      INTO has_shape;
    SELECT string_agg(column_name || ' ' || data_type, ', ' ORDER BY ordinal_position)
      INTO table_columns
      FROM information_schema.columns
     WHERE table_schema = current_schema() AND table_name = 'projects';
    SELECT string_agg(column_name || ' ' || data_type, ', ' ORDER BY ordinal_position)
      INTO view_columns
      FROM information_schema.columns
     WHERE table_schema = current_schema() AND table_name = 'live_projects';
    IF marker_count <> 2 OR NOT has_shape THEN
        RAISE EXCEPTION 'migration 0026 self-check: deleted marker columns % of 2, shape check %',
            marker_count, has_shape
            USING ERRCODE = 'feature_not_supported';
    END IF;
    IF view_columns IS DISTINCT FROM table_columns THEN
        RAISE EXCEPTION 'migration 0026 self-check: live_projects columns (%) differ from projects columns (%)',
            coalesce(view_columns, 'none'), table_columns
            USING ERRCODE = 'feature_not_supported';
    END IF;
END
$self_check$;
