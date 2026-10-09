-- name-db-checks slice 01 (ADR-NAME-DB-001, DESIGN DDD-1..8): the database
-- refuses every NEW workspace name that foundry_core::WorkspaceName refuses,
-- arm for arm. Existing rows are never scanned or rewritten (D4): the rule
-- lives in BEFORE triggers, not in a CHECK constraint.
--
-- Re-runnable body (DDD-7): CREATE OR REPLACE FUNCTION, DROP TRIGGER IF EXISTS
-- then CREATE TRIGGER. A pre-0019 dump restored with --clean leaves the
-- functions behind; the next boot re-applies over them.
--
-- Every invisible character is written as an escape inside a dollar-quoted
-- body, so the encoding guard below runs before any of them is lexed.

-- (1) Encoding guard (DDD-8, OQ-D1): the code-point sets below mean nothing
-- outside UTF8. Refused inside this migration's own transaction.
DO $encoding_guard$
BEGIN
    IF current_setting('server_encoding') <> 'UTF8' THEN
        RAISE EXCEPTION 'migration 0019 (workspace name rule) needs a UTF8 database; server_encoding is %',
            current_setting('server_encoding')
            USING ERRCODE = 'feature_not_supported',
                  HINT = 'Create the database WITH ENCODING ''UTF8''.';
    END IF;
END
$encoding_guard$;

-- (2) The verdict (DDD-1/2/3): NULL when the name passes, else the arm suffix.
-- Rust's order: trim (the 25 Unicode White_Space points, as str::trim), then on
-- the trimmed value: empty, refused character (name_chars::is_refused_name_char),
-- length in code points; all pass but trimmed <> input gives 'trimmed'.
-- Explicit code-point sets only: no \s, no [[:space:]], no regex flags.
CREATE OR REPLACE FUNCTION foundry_name_rule_violation(name text, max_chars integer)
RETURNS text
LANGUAGE sql
IMMUTABLE STRICT PARALLEL SAFE
AS $verdict$
    SELECT CASE
               WHEN rule.trimmed = '' THEN 'not_empty'
               WHEN rule.trimmed ~ E'[\u0001-\u001F\u007F-\u009F -‮⁦-⁩]'
                   THEN 'no_control_chars'
               WHEN char_length(rule.trimmed) > max_chars THEN 'max_' || max_chars || '_chars'
               WHEN rule.trimmed <> name THEN 'trimmed'
           END
      FROM (SELECT btrim(name, white_space.points) AS trimmed
              FROM (SELECT E'\u0009\u000A\u000B\u000C\u000D \u0085                 　'
                           AS points) AS white_space) AS rule
$verdict$;

COMMENT ON FUNCTION foundry_name_rule_violation(text, integer) IS
    'Name rule verdict: mirrors foundry_core::WorkspaceName (and foundry_core::ProjectName at its cap). '
    'NULL when the name passes, else the arm: not_empty, no_control_chars, max_<cap>_chars, trimmed.';

-- (3) The guard (DDD-4): returns NEW unchanged or raises a check_violation
-- naming the arm. Never a DETAIL: the row's name is not logged.
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
                          ELSE TG_TABLE_NAME
                      END,
                      TG_ARGV[0]);
END
$enforce$;

COMMENT ON FUNCTION foundry_enforce_name_rule() IS
    'Name rule trigger: mirrors foundry_core::WorkspaceName / foundry_core::ProjectName; '
    'the cap is TG_ARGV[0]. Refuses with SQLSTATE 23514 and CONSTRAINT <table>_name_<arm>.';

-- (4) The workspace triggers (DDD-5): every insert, and every UPDATE that
-- changes the name. Other-column and same-value writes never meet the rule.
DROP TRIGGER IF EXISTS workspaces_name_rule_on_insert ON workspaces;
CREATE TRIGGER workspaces_name_rule_on_insert
    BEFORE INSERT ON workspaces
    FOR EACH ROW
    EXECUTE FUNCTION foundry_enforce_name_rule('24');

DROP TRIGGER IF EXISTS workspaces_name_rule_on_rename ON workspaces;
CREATE TRIGGER workspaces_name_rule_on_rename
    BEFORE UPDATE OF name ON workspaces
    FOR EACH ROW
    WHEN (OLD.name IS DISTINCT FROM NEW.name)
    EXECUTE FUNCTION foundry_enforce_name_rule('24');

COMMENT ON TRIGGER workspaces_name_rule_on_insert ON workspaces IS
    'Name rule on insert: mirrors foundry_core::WorkspaceName (cap 24).';
COMMENT ON TRIGGER workspaces_name_rule_on_rename ON workspaces IS
    'Name rule on rename: mirrors foundry_core::WorkspaceName (cap 24).';

-- (5) Earned Trust at apply time (DDD-8): a substrate whose btrim, regex or
-- char_length semantics differ refuses the migration instead of installing a
-- stricter or looser rule.
DO $self_check$
DECLARE
    probe record;
    verdict text;
BEGIN
    FOR probe IN
        SELECT label, name, expected
          FROM (VALUES
              ('edge U+00A0',        E' Globex',            'trimmed'::text),
              ('edge U+0085',        E'Globex\u0085',            'trimmed'),
              ('edge U+180E',        E'᠎Globex',            NULL),
              ('interior U+180E',    E'Glo᠎bex',            NULL),
              ('edge U+FEFF',        E'﻿Globex',            NULL),
              ('edge U+2028',        E' Globex',            'trimmed'),
              ('interior U+2028',    E'Glo bex',            'no_control_chars'),
              ('interior U+009F',    E'Glo\u009Fbex',            'no_control_chars'),
              ('interior U+00A0',    E'Glo bex',            NULL),
              ('interior U+202F',    E'Glo bex',            NULL),
              ('24 astral',          repeat(E'\U0001F600', 24),  NULL),
              ('25 astral',          repeat(E'\U0001F600', 25),  'max_24_chars'),
              ('empty',              '',                         'not_empty'),
              ('all White_Space',    E' \t　 \u0085',   'not_empty')
          ) AS cases (label, name, expected)
    LOOP
        verdict := foundry_name_rule_violation(probe.name, 24);
        IF verdict IS DISTINCT FROM probe.expected THEN
            RAISE EXCEPTION 'migration 0019 self-check: % judged %, expected %',
                probe.label, coalesce(verdict, 'NULL'), coalesce(probe.expected, 'NULL')
                USING ERRCODE = 'feature_not_supported';
        END IF;
    END LOOP;
END
$self_check$;
