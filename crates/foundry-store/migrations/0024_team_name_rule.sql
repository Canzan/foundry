-- team-lifecycle slice 03 (DESIGN DDD-6, DDD-24; ADR-NAME-DB-001): the
-- database refuses every NEW team name that foundry_core::TeamName refuses,
-- arm for arm, at its cap of 64 characters. Existing rows are never scanned or
-- rewritten: the rule lives in BEFORE triggers, not in a CHECK constraint, so a
-- legacy team name stays until someone renames it.
--
-- Reuses migration 0019's foundry_name_rule_violation(text, integer) unchanged.
-- REPLACES 0019's foundry_enforce_name_rule() with an identical body plus one
-- HINT arm ('teams' -> TeamName), so a refusal names foundry_core::TeamName
-- rather than foundry_core::teams; workspaces and projects keep their HINT.
--
-- Re-runnable body: CREATE OR REPLACE FUNCTION, DROP TRIGGER IF EXISTS then
-- CREATE TRIGGER. Undo (documented in CHANGELOG.md):
--   DROP TRIGGER IF EXISTS teams_name_rule_on_insert ON teams;
--   DROP TRIGGER IF EXISTS teams_name_rule_on_rename ON teams;
-- (the extra HINT arm is inert without a teams trigger and may stay).

-- (1) The guard, with the teams arm. Returns NEW unchanged or raises a
-- check_violation naming the arm. Never a DETAIL: the row's name is not logged.
CREATE OR REPLACE FUNCTION foundry_enforce_name_rule()
RETURNS trigger
LANGUAGE plpgsql
SET search_path FROM CURRENT
AS $enforce$
DECLARE
    arm text;
    violated text;
BEGIN
    arm := foundry_name_rule_violation(NEW.name, TG_ARGV[0]::integer);
    IF arm IS NULL THEN
        RETURN NEW;
    END IF;
    violated := TG_TABLE_NAME || '_name_' || arm;
    RAISE EXCEPTION USING
        ERRCODE = 'check_violation',
        MESSAGE = format('new row for relation "%s" violates check constraint "%s"',
                         TG_TABLE_NAME, violated),
        CONSTRAINT = violated,
        TABLE = TG_TABLE_NAME,
        COLUMN = 'name',
        SCHEMA = TG_TABLE_SCHEMA,
        HINT = format('The name must be a valid foundry_core::%s: trimmed, not empty, '
                      'no control characters, at most %s characters.',
                      CASE TG_TABLE_NAME
                          WHEN 'workspaces' THEN 'WorkspaceName'
                          WHEN 'projects' THEN 'ProjectName'
                          WHEN 'teams' THEN 'TeamName'
                          ELSE TG_TABLE_NAME
                      END,
                      TG_ARGV[0]);
END
$enforce$;

COMMENT ON FUNCTION foundry_enforce_name_rule() IS
    'Name rule trigger: mirrors foundry_core::WorkspaceName / foundry_core::ProjectName / '
    'foundry_core::TeamName; the cap is TG_ARGV[0]. Refuses with SQLSTATE 23514 and '
    'CONSTRAINT <table>_name_<arm>.';

-- (2) The team triggers: every insert, and every UPDATE that changes the name.
-- Other-column writes (the slug) and same-value writes never meet the rule.
DROP TRIGGER IF EXISTS teams_name_rule_on_insert ON teams;
CREATE TRIGGER teams_name_rule_on_insert
    BEFORE INSERT ON teams
    FOR EACH ROW
    EXECUTE FUNCTION foundry_enforce_name_rule('64');

DROP TRIGGER IF EXISTS teams_name_rule_on_rename ON teams;
CREATE TRIGGER teams_name_rule_on_rename
    BEFORE UPDATE OF name ON teams
    FOR EACH ROW
    WHEN (OLD.name IS DISTINCT FROM NEW.name)
    EXECUTE FUNCTION foundry_enforce_name_rule('64');

COMMENT ON TRIGGER teams_name_rule_on_insert ON teams IS
    'Name rule on insert: mirrors foundry_core::TeamName (cap 64).';
COMMENT ON TRIGGER teams_name_rule_on_rename ON teams IS
    'Name rule on rename: mirrors foundry_core::TeamName (cap 64).';

-- (3) Earned Trust at apply time: the verdict function judges the team cap
-- exactly, counted in code points (multi-byte and astral included), and both
-- team triggers are installed and enabled, or the migration is refused.
DO $self_check$
DECLARE
    probe record;
    verdict text;
    enabled bigint;
BEGIN
    FOR probe IN
        SELECT label, name, expected
          FROM (VALUES
              ('64 ascii',        repeat('x', 64),              NULL::text),
              ('65 ascii',        repeat('x', 65),              'max_64_chars'),
              ('64 U+65E5',       repeat(E'日', 64),        NULL),
              ('65 U+65E5',       repeat(E'日', 65),        'max_64_chars'),
              ('64 astral',       repeat(E'\U0001F600', 64),    NULL),
              ('65 astral',       repeat(E'\U0001F600', 65),    'max_64_chars'),
              ('interior U+0001', E'Platform\u0001Ops',         'no_control_chars'),
              ('interior U+202E', E'Platform‮Ops',         'no_control_chars'),
              ('padded',          ' Platform',                  'trimmed'),
              ('space only',      ' ',                          'not_empty')
          ) AS cases (label, name, expected)
    LOOP
        verdict := foundry_name_rule_violation(probe.name, 64);
        IF verdict IS DISTINCT FROM probe.expected THEN
            RAISE EXCEPTION 'migration 0024 self-check: % judged %, expected %',
                probe.label, coalesce(verdict, 'NULL'), coalesce(probe.expected, 'NULL')
                USING ERRCODE = 'feature_not_supported';
        END IF;
    END LOOP;

    SELECT count(*) INTO enabled
      FROM pg_trigger t
     WHERE t.tgrelid = to_regclass('teams')
       AND t.tgname IN ('teams_name_rule_on_insert', 'teams_name_rule_on_rename')
       AND t.tgenabled = 'O'
       AND NOT t.tgisinternal;
    IF enabled <> 2 THEN
        RAISE EXCEPTION 'migration 0024 self-check: % of 2 team name-rule triggers enabled',
            enabled
            USING ERRCODE = 'feature_not_supported';
    END IF;
END
$self_check$;
