-- name-db-checks slice 02 (ADR-NAME-DB-001, DESIGN DDD-4..8, DDD-16): the
-- database refuses every NEW project name that foundry_core::ProjectName
-- refuses, arm for arm, at its cap of 256 characters. Existing rows are never
-- scanned or rewritten (D4): the rule lives in BEFORE triggers.
--
-- Reuses migration 0019's foundry_name_rule_violation(text, integer) and
-- foundry_enforce_name_rule(); nothing is redefined here. Re-runnable body
-- (DDD-7): DROP TRIGGER IF EXISTS then CREATE TRIGGER.

-- (1) The project triggers (DDD-5): every insert, and every UPDATE that changes
-- the name. Other-column writes (the issue counter, next_issue_number) and
-- same-value writes never meet the rule.
DROP TRIGGER IF EXISTS projects_name_rule_on_insert ON projects;
CREATE TRIGGER projects_name_rule_on_insert
    BEFORE INSERT ON projects
    FOR EACH ROW
    EXECUTE FUNCTION foundry_enforce_name_rule('256');

DROP TRIGGER IF EXISTS projects_name_rule_on_rename ON projects;
CREATE TRIGGER projects_name_rule_on_rename
    BEFORE UPDATE OF name ON projects
    FOR EACH ROW
    WHEN (OLD.name IS DISTINCT FROM NEW.name)
    EXECUTE FUNCTION foundry_enforce_name_rule('256');

COMMENT ON TRIGGER projects_name_rule_on_insert ON projects IS
    'Name rule on insert: mirrors foundry_core::ProjectName (cap 256).';
COMMENT ON TRIGGER projects_name_rule_on_rename ON projects IS
    'Name rule on rename: mirrors foundry_core::ProjectName (cap 256).';

-- (2) Earned Trust at apply time (DDD-8): the verdict function must judge the
-- project cap exactly, counted in code points (multi-byte and astral included),
-- or the migration is refused.
DO $self_check$
DECLARE
    probe record;
    verdict text;
BEGIN
    FOR probe IN
        SELECT label, name, expected
          FROM (VALUES
              ('256 ascii',   repeat('x', 256),             NULL::text),
              ('257 ascii',   repeat('x', 257),             'max_256_chars'),
              ('256 U+00E9',  repeat(E'é', 256),       NULL),
              ('257 U+00E9',  repeat(E'é', 257),       'max_256_chars'),
              ('256 U+65E5',  repeat(E'日', 256),       NULL),
              ('257 U+65E5',  repeat(E'日', 257),       'max_256_chars'),
              ('256 astral',  repeat(E'\U0001F600', 256),   NULL),
              ('257 astral',  repeat(E'\U0001F600', 257),   'max_256_chars'),
              ('256 padded',  ' ' || repeat('x', 256) || ' ', 'trimmed'),
              ('empty',       '',                           'not_empty')
          ) AS cases (label, name, expected)
    LOOP
        verdict := foundry_name_rule_violation(probe.name, 256);
        IF verdict IS DISTINCT FROM probe.expected THEN
            RAISE EXCEPTION 'migration 0020 self-check: % judged %, expected %',
                probe.label, coalesce(verdict, 'NULL'), coalesce(probe.expected, 'NULL')
                USING ERRCODE = 'feature_not_supported';
        END IF;
    END LOOP;
END
$self_check$;
