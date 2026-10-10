-- team-membership slice 02 (ADR-TEAM-MEMBERSHIP-001, DESIGN DDD-6, DDD-7): a
-- team that has a lead never loses its last one, for every caller (INV-8). The
-- store serialises lead-removing writes with FOR NO KEY UPDATE on the team row
-- and decides by an exact pre-read; these BEFORE triggers take the same lock and
-- are the backstop for every other writer (psql, scripts, future code).
--
-- Re-runnable body (ADR-001 decision 7): the triggers are dropped first, so the
-- one-time orphan cleanup never meets them; then CREATE OR REPLACE FUNCTION and
-- CREATE TRIGGER. Undo (documented in CHANGELOG.md):
--   DROP TRIGGER IF EXISTS team_memberships_lead_guard_on_delete ON team_memberships;
--   DROP TRIGGER IF EXISTS team_memberships_lead_guard_on_update ON team_memberships;
--   DROP FUNCTION IF EXISTS foundry_team_keeps_a_lead();

DROP TRIGGER IF EXISTS team_memberships_lead_guard_on_delete ON team_memberships;
DROP TRIGGER IF EXISTS team_memberships_lead_guard_on_update ON team_memberships;

-- (1) "Lead" means a lead row (DDD-7, ADR-001 decision 6): delete the team rows
-- of people who are no longer members of the team's workspace (left behind by
-- the pre-OQ-9 workspace removal). Expected 0; the count is reported.
DO $orphans$
DECLARE
    removed bigint;
BEGIN
    DELETE FROM team_memberships tm
     USING teams t
     WHERE t.id = tm.team_id
       AND NOT EXISTS (SELECT 1 FROM workspace_memberships wm
                        WHERE wm.workspace_id = t.workspace_id
                          AND wm.user_id = tm.user_id);
    GET DIAGNOSTICS removed = ROW_COUNT;
    RAISE NOTICE 'migration 0021: deleted % team row(s) of people no longer in the team''s workspace',
        removed;
END
$orphans$;

-- (2) The guard (ADR-001 decision 3). Fires only for a lead row that stops
-- being a lead of its team (the trigger WHEN clauses). Takes the team row lock
-- the store takes; a team row that is gone means the team is being deleted (a
-- cascade from teams or workspaces), and a team that is gone needs no lead.
-- Row-level BEFORE triggers see rows processed earlier in the same command, so
-- one statement demoting both leads is refused on its second row. Messages
-- carry the team id only, never a name or an email, and no DETAIL.
CREATE OR REPLACE FUNCTION foundry_team_keeps_a_lead()
RETURNS trigger
LANGUAGE plpgsql
SET search_path FROM CURRENT
AS $guard$
DECLARE
    other_leads bigint;
BEGIN
    PERFORM 1 FROM teams WHERE id = OLD.team_id FOR NO KEY UPDATE;
    IF FOUND THEN
        SELECT count(*) INTO other_leads
          FROM team_memberships
         WHERE team_id = OLD.team_id
           AND user_id <> OLD.user_id
           AND role = 'lead';
        IF other_leads = 0 THEN
            RAISE EXCEPTION USING
                ERRCODE = 'check_violation',
                MESSAGE = format('new row for relation "%s" violates check constraint "%s"',
                                 TG_TABLE_NAME, 'team_memberships_team_keeps_a_lead'),
                CONSTRAINT = 'team_memberships_team_keeps_a_lead',
                TABLE = TG_TABLE_NAME,
                SCHEMA = TG_TABLE_SCHEMA,
                HINT = format('Team %s must keep a lead: make someone else its lead first.',
                              OLD.team_id);
        END IF;
    END IF;
    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END
$guard$;

COMMENT ON FUNCTION foundry_team_keeps_a_lead() IS
    'Team lead guard (ADR-TEAM-MEMBERSHIP-001): locks the team row FOR NO KEY UPDATE and refuses, '
    'with SQLSTATE 23514 and CONSTRAINT team_memberships_team_keeps_a_lead, a write that would '
    'leave a team that had a lead with none. Exempt when the team row is gone (cascades).';

-- (3) The triggers: a lead row deleted, or a lead row demoted or moved off its
-- team. Promotions, member rows and same-value writes never meet the guard.
CREATE TRIGGER team_memberships_lead_guard_on_delete
    BEFORE DELETE ON team_memberships
    FOR EACH ROW
    WHEN (OLD.role = 'lead')
    EXECUTE FUNCTION foundry_team_keeps_a_lead();

CREATE TRIGGER team_memberships_lead_guard_on_update
    BEFORE UPDATE OF role, team_id ON team_memberships
    FOR EACH ROW
    WHEN (OLD.role = 'lead' AND (NEW.role <> 'lead' OR NEW.team_id <> OLD.team_id))
    EXECUTE FUNCTION foundry_team_keeps_a_lead();

COMMENT ON TRIGGER team_memberships_lead_guard_on_delete ON team_memberships IS
    'Lead guard on delete: a team keeps its last lead (team_memberships_team_keeps_a_lead).';
COMMENT ON TRIGGER team_memberships_lead_guard_on_update ON team_memberships IS
    'Lead guard on demotion or move: a team keeps its last lead (team_memberships_team_keeps_a_lead).';

-- (4) Self-check at apply time: the function and both triggers are in place,
-- and no orphan team row is left, or the migration is refused.
DO $self_check$
DECLARE
    triggers bigint;
    orphans bigint;
BEGIN
    SELECT count(*) INTO triggers
      FROM pg_trigger tg JOIN pg_class c ON c.oid = tg.tgrelid
     WHERE c.relname = 'team_memberships'
       AND c.relnamespace = current_schema()::regnamespace
       AND tg.tgname IN ('team_memberships_lead_guard_on_delete',
                         'team_memberships_lead_guard_on_update');
    SELECT count(*) INTO orphans
      FROM team_memberships tm JOIN teams t ON t.id = tm.team_id
     WHERE NOT EXISTS (SELECT 1 FROM workspace_memberships wm
                        WHERE wm.workspace_id = t.workspace_id
                          AND wm.user_id = tm.user_id);
    IF to_regprocedure('foundry_team_keeps_a_lead()') IS NULL OR triggers <> 2 OR orphans <> 0 THEN
        RAISE EXCEPTION 'migration 0021 self-check: function %, % of 2 triggers, % orphan team row(s)',
            to_regprocedure('foundry_team_keeps_a_lead()') IS NOT NULL, triggers, orphans
            USING ERRCODE = 'feature_not_supported';
    END IF;
END
$self_check$;
