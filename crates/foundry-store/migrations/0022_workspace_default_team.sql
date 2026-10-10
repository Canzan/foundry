-- team-membership slice 05 (ADR-TEAM-MEMBERSHIP-002, DESIGN DDD-12): a
-- workspace's default team for new members is one of its own teams or none
-- (INV-7), for every caller, and deleting that team clears only the default
-- (OQ-6). Additive: the column is NULL for every existing workspace, so the
-- default is unset on upgrade (OQ-4) and nobody is backfilled (OQ-8).
--
-- `ON DELETE SET NULL (default_team_id)` (the column-list form) needs
-- Postgres 15 or later; production and the harness pin 16. An older server
-- refuses this migration with a syntax error rather than install a weaker key.
--
-- Re-runnable body: the foreign key is dropped before the unique key it
-- references, then both are added again. Undo (documented in CHANGELOG.md):
--   ALTER TABLE workspaces DROP CONSTRAINT IF EXISTS workspaces_default_team_same_workspace;
--   ALTER TABLE workspaces DROP COLUMN IF EXISTS default_team_id;
--   ALTER TABLE teams DROP CONSTRAINT IF EXISTS teams_workspace_id_id_key;

ALTER TABLE workspaces DROP CONSTRAINT IF EXISTS workspaces_default_team_same_workspace;
ALTER TABLE teams DROP CONSTRAINT IF EXISTS teams_workspace_id_id_key;

-- (1) The composite target. Redundant as uniqueness (`id` is the key), but a
-- composite foreign key needs a unique constraint on exactly its columns.
ALTER TABLE teams
    ADD CONSTRAINT teams_workspace_id_id_key UNIQUE (workspace_id, id);

-- (2) The setting: one nullable cell per workspace.
ALTER TABLE workspaces ADD COLUMN IF NOT EXISTS default_team_id UUID NULL;

-- (3) Same workspace, self-clearing. Under MATCH SIMPLE a NULL default is never
-- checked; a non-NULL one must name a team whose workspace_id is this row's id.
-- Deleting that team nulls default_team_id only (never the row's own id).
ALTER TABLE workspaces
    ADD CONSTRAINT workspaces_default_team_same_workspace
    FOREIGN KEY (id, default_team_id)
    REFERENCES teams (workspace_id, id)
    ON DELETE SET NULL (default_team_id);

COMMENT ON COLUMN workspaces.default_team_id IS
    'The team new members land on as member (ADR-TEAM-MEMBERSHIP-002); NULL = none. '
    'Kept in this workspace and cleared on team delete by workspaces_default_team_same_workspace.';

-- (4) Self-check at apply time: the column, the unique key and the foreign key
-- (exactly (id, default_team_id) -> teams (workspace_id, id), with its
-- column-list SET NULL) are in place, or the migration is refused.
DO $self_check$
DECLARE
    has_column boolean;
    has_unique boolean;
    fk_ok boolean;
BEGIN
    SELECT EXISTS (SELECT 1 FROM information_schema.columns
                    WHERE table_schema = current_schema()
                      AND table_name = 'workspaces'
                      AND column_name = 'default_team_id'
                      AND is_nullable = 'YES')
      INTO has_column;
    SELECT EXISTS (SELECT 1 FROM pg_constraint
                    WHERE conname = 'teams_workspace_id_id_key'
                      AND contype = 'u'
                      AND conrelid = to_regclass('teams'))
      INTO has_unique;
    SELECT EXISTS (
        SELECT 1 FROM pg_constraint k
         WHERE k.conname = 'workspaces_default_team_same_workspace'
           AND k.contype = 'f'
           AND k.conrelid = to_regclass('workspaces')
           AND k.confrelid = to_regclass('teams')
           AND k.conkey = ARRAY[(SELECT attnum FROM pg_attribute
                                  WHERE attrelid = to_regclass('workspaces') AND attname = 'id'),
                                (SELECT attnum FROM pg_attribute
                                  WHERE attrelid = to_regclass('workspaces')
                                    AND attname = 'default_team_id')]::int2[]
           AND k.confkey = ARRAY[(SELECT attnum FROM pg_attribute
                                   WHERE attrelid = to_regclass('teams')
                                     AND attname = 'workspace_id'),
                                 (SELECT attnum FROM pg_attribute
                                   WHERE attrelid = to_regclass('teams') AND attname = 'id')]::int2[]
           AND k.confdeltype = 'n'
           AND k.confdelsetcols = ARRAY[(SELECT attnum FROM pg_attribute
                                          WHERE attrelid = to_regclass('workspaces')
                                            AND attname = 'default_team_id')]::int2[])
      INTO fk_ok;
    IF NOT (has_column AND has_unique AND fk_ok) THEN
        RAISE EXCEPTION 'migration 0022 self-check: column %, unique key %, foreign key %',
            has_column, has_unique, fk_ok
            USING ERRCODE = 'feature_not_supported';
    END IF;
END
$self_check$;
