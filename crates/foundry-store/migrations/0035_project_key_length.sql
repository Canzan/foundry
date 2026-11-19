-- project-key-length S1 (ADR-PROJECT-KEY-001, laws L3 and L6): the stored key
-- rule widens to 2 to 10 capital letters A to Z, so the database admits exactly
-- what `foundry_core::ProjectKey::try_new` accepts — for every writer,
-- including a database client that never passes the application.
--
-- Migration 0001 states the earlier `^[A-Z]{2,6}$` CHECK inline and is shipped:
-- it is never edited, so the rule changes here. The old CHECK was declared
-- without a name, so it is found by its DEFINITION (a CHECK on `projects`
-- mentioning `key_prefix`), never by a generated name that differs between
-- databases restored from different tools.
--
-- Every catalog read below is pinned to `'projects'::regclass` — this schema's
-- table alone. A scan that filtered on `pg_class`/`pg_namespace` instead let
-- `pg_get_constraintdef` take an AccessShareLock on the `projects` of OTHER
-- schemas, which deadlocked (SQLSTATE 40P01) against the AccessExclusiveLock
-- their own concurrent run of this migration held: the per-scenario schemas of
-- the acceptance suite migrate side by side.
--
-- L6: the widening is monotone, so no existing row is scanned, rewritten or
-- re-keyed; every key written under the 2 to 6 rule satisfies the new one.
-- ALTER TABLE ... ADD CONSTRAINT does validate the existing rows, which is
-- exactly the check that no stored key falls outside the widened rule.

-- (1) Drop every earlier key_prefix shape CHECK, found by its definition.
DO $drop_earlier$
DECLARE
    earlier text;
BEGIN
    FOR earlier IN
        SELECT con.conname
          FROM pg_constraint con
         WHERE con.conrelid = 'projects'::regclass
           AND con.contype = 'c'
           AND con.conname <> 'projects_key_prefix_shape'
           AND pg_get_constraintdef(con.oid) LIKE '%key_prefix%'
    LOOP
        EXECUTE format('ALTER TABLE projects DROP CONSTRAINT %I', earlier);
    END LOOP;
END
$drop_earlier$;

-- (2) The widened rule, named so a later migration can find it by name as well
-- as by definition.
ALTER TABLE projects
    ADD CONSTRAINT projects_key_prefix_shape
    CHECK (key_prefix ~ '^[A-Z]{2,10}$');

COMMENT ON CONSTRAINT projects_key_prefix_shape ON projects IS
    'Project key shape: mirrors foundry_core::ProjectKey (2 to 10 capital letters A to Z).';

-- (3) Earned Trust at apply time: exactly one key_prefix shape CHECK survives
-- the drop above, and it is the widened named one. A database that still
-- carries an older, stricter CHECK — the state that turns an accepted create
-- into a server error — refuses this migration instead of serving that state.
--
-- Which keys the surviving CHECK admits is NOT restated here: a third
-- transcription of the rule could only agree with itself. The keys the stored
-- rule admits are compared arm for arm against the rule the application
-- enforces by the executed parity probe,
-- `crates/foundry-store/tests/project_key_db_rule.rs`.
DO $self_check$
DECLARE
    shape_checks text[];
BEGIN
    SELECT coalesce(array_agg(con.conname ORDER BY con.conname), ARRAY[]::text[])
      INTO shape_checks
      FROM pg_constraint con
     WHERE con.conrelid = 'projects'::regclass
       AND con.contype = 'c'
       AND pg_get_constraintdef(con.oid) LIKE '%key_prefix%';
    IF shape_checks <> ARRAY['projects_key_prefix_shape'] THEN
        RAISE EXCEPTION
            'migration 0035 self-check: the key_prefix CHECK constraints on projects are %, expected exactly {projects_key_prefix_shape}; an older shape CHECK was not dropped, so the database would refuse keys the application accepts',
            shape_checks
            USING ERRCODE = 'feature_not_supported';
    END IF;
END
$self_check$;
