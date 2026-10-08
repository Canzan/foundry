# ADR-NAME-DB-001: The database enforces the pure workspace- and project-name arms with one shared verdict function and name-write triggers, not CHECK constraints

- Status: Accepted (2026-10-08)
- Feature: `name-db-checks` (DISCUSS D1-D12, OQ-1 resolved as (a′); DESIGN DDD-1 to DDD-14)
- Amends (at DELIVER): ADR-WORKSPACE-NAME-001 ("Neutral: the store remains able to write any name"), ADR-PROJECT-NAME-001 item 8 and its consequence "every production door, not every row". Both become "every door and every new name write".

## Context

`workspaces.name` and `projects.name` are `TEXT NOT NULL` with no rule. Every app door has
parsed names through `WorkspaceName::try_new` (v0.10.0) and `ProjectName::try_new` (v0.11.0)
since those releases, but a hand-typed `psql` statement, a script or a hand-made restore can
still store a name the app would refuse. Constraints on the fix:

- **Parity (D3).** The database must accept `s` if and only if `try_new(s)` is `Ok` and its
  value is byte-equal to `s`. If the database is stricter, the result is a 500 at an app door.
  If it is looser, the rule has a hole. Rust trims with `str::trim` (Unicode White_Space, 25
  code points). Postgres `btrim(s)` trims U+0020 only, and `\s` and `[[:space:]]` differ from
  White_Space as well.
- **Legacy rows keep every capability (D4, D6).** `insert_issue_attempt` UPDATEs
  `projects.next_issue_number` on every new issue. A row-level `CHECK … NOT VALID` is
  re-checked on every UPDATE of the row, so a legacy project name would turn every new issue
  into a 23514.
- **Restore keeps working (D9).** `pg_restore --clean --if-exists` of a `pg_dump -Fc` archive
  that holds legacy rows must succeed.
- **The refusal names its arm (D5)**, so the operator at the psql prompt can see which rule
  was broken.
- **Fixtures seed legacy rows on purpose (D8)**, through one test-only seam.
- Both production databases are UTF8. The legacy count is 0 on dev and on prod (OQ-3).

## Decision

1. **One pure verdict function.** `foundry_name_rule_violation(name text, max_chars integer)
   RETURNS text` is `IMMUTABLE`. It returns NULL when the name passes. Otherwise it returns the
   arm suffix, evaluated in Rust's order on the trimmed value: `not_empty`, then
   `no_control_chars`, then `max_<N>_chars`. If all three pass but the trimmed value differs
   from the input, it returns `trimmed`. The function is the functional core. Bulk parity
   tests call it directly. A NULL input returns NULL, so the `NOT NULL` constraint keeps
   ownership of NULLs.
   - **Trim** is `btrim(name, <set>)`, where the set lists the 25 White_Space code points
     explicitly: U+0009-000D, U+0020, U+0085, U+00A0, U+1680, U+2000-200A, U+2028, U+2029,
     U+202F, U+205F and U+3000. It does not use `\s`, POSIX classes or regex trimming.
   - **Refused set** is one bracket expression of explicit code-point escapes:
     U+0001-001F, U+007F-009F, U+2028-202E and U+2066-2069. The range U+2028-202E is
     contiguous: it covers U+2028, U+2029 and U+202A-202E, the same set as `name_chars`.
     U+0000 cannot be stored in `text`, so it is out of scope.
   - **Length** is `char_length` of the trimmed value. In a UTF8 database this equals
     `chars().count()`.
2. **One trigger function, two triggers per table.** `foundry_enforce_name_rule()` is a
   plpgsql function (precedent: 0003's outbox trigger) with `SET search_path FROM CURRENT`,
   so a caller's session `search_path`, including pg_restore's empty one, cannot unbind it. It
   reads the cap from `TG_ARGV[0]`, calls the verdict function, and on a verdict raises
   `ERRCODE 'check_violation'` (23514) with these fields:
   - `CONSTRAINT = <table>_name_<arm>`, which gives `workspaces_name_not_empty`,
     `workspaces_name_trimmed`, `workspaces_name_no_control_chars` and
     `workspaces_name_max_24_chars`, and the same four for `projects` with
     `projects_name_max_256_chars`.
   - `MESSAGE = new row for relation "<table>" violates check constraint "<arm name>"`. This
     is the native CHECK wording, so psql prints exactly what DISCUSS's journey shows.
   - `TABLE`, `COLUMN = name` and `SCHEMA`.
   - A one-line `HINT` stating the table's rule ("mirrors foundry_core::WorkspaceName" or
     "ProjectName").
   - There is no `DETAIL` row dump.

   Otherwise the function returns NEW unchanged. Each table gets two triggers:
   - `<table>_name_rule_on_insert`: `BEFORE INSERT … FOR EACH ROW`.
   - `<table>_name_rule_on_rename`: `BEFORE UPDATE OF name … FOR EACH ROW WHEN
     (OLD.name IS DISTINCT FROM NEW.name)`.

   Postgres forbids `OLD` in the WHEN clause of an INSERT trigger, which is why each table
   needs two. An UPDATE that does not list `name`, such as `next_issue_number`, never fires the
   rule. Neither does a same-value write.
3. **Two migrations.** `0019_workspace_name_rule.sql` holds the encoding guard, both
   functions, the self-check and the `workspaces` triggers with argument `'24'`.
   `0020_project_name_rule.sql` holds the `projects` triggers with argument `'256'`, plus the
   self-check for 256. Both bodies are **re-runnable**: `CREATE OR REPLACE FUNCTION`, then
   `DROP TRIGGER IF EXISTS` followed by `CREATE TRIGGER`. A pre-0019 dump restored with
   `--clean` over a newer schema leaves the functions in place (they are not in that archive),
   and the next boot must re-apply cleanly over them. Each object gets a `COMMENT` that says it
   mirrors `foundry_core::WorkspaceName` or `ProjectName`.
4. **Earned Trust at apply time.** 0019 runs two checks in the migration's own transaction,
   and either one fails the migration, so a new replica never becomes ready while old replicas
   keep serving:
   - **Encoding guard.** It refuses a database whose `server_encoding` is not UTF8.
   - **Self-check.** A `DO` block asserts the verdict function's answer for the substrate's
     known traps: NBSP and U+0085 are trimmed, U+180E and U+FEFF are allowed, an edge U+2028
     is trimmed while an interior one is refused, U+009F is refused, an interior NBSP or
     U+202F is allowed, and an astral character counts once at 24 and 25.

   A Postgres whose regex, `btrim` or `char_length` semantics differ from the app's refuses
   the migration rather than installing a rule that is stricter or looser than the app.
5. **`Store::probe` is unchanged.** The binary reads and writes nothing that the rule adds,
   so a missing trigger causes no app malfunction. The documented undo is to drop the triggers,
   and a probe that refused readiness without them would turn that undo into an outage.
6. **Legacy seam (fixtures only).** One function in `foundry-store`, behind
   `#[cfg(feature = "test-support")]`, writes a row "that predates the rule". It takes the
   table from a **closed enum** of the two name-rule tables, plus the caller's write. In one
   transaction it runs `ALTER TABLE <t> DISABLE TRIGGER <t>_name_rule_on_insert` (and
   `_on_rename`) by name, then the write, then re-enables the triggers, then commits.
   - It never uses `DISABLE TRIGGER ALL`, which would also disable FK enforcement and needs
     superuser.
   - Other sessions never observe the rule absent: the ALTER is transactional, and its
     SHARE ROW EXCLUSIVE lock serialises them. Acceptance scenarios have per-scenario
     schemas, so there is no cross-scenario contention.
   - A store-level migration test keeps the staged-migration model instead (F8, and the new
     0019/0020 apply tests): stage to 0018, insert, then migrate.
   - A `cargo xtask check-arch` rule, `name-rule-legacy-seam`, enforces two things. (a) The
     seam has no call under `crates/{foundry-app,foundry-services,foundry-api}/src`.
     (b) `DISABLE TRIGGER` and `session_replication_role` appear in no `.rs` under `crates/`
     except the seam's file.
7. **Restore.** `pg_dump -Fc` writes functions in pre-data, table data in data, and triggers
   in post-data. A full restore therefore COPYs legacy rows before any trigger exists and does
   not scan rows when it creates the triggers. Slice 03 proves four things:
   - A post-0020 dump restores.
   - A pre-0019 dump restores over a post-0020 schema, and the next boot re-applies
     0019/0020 cleanly.
   - `backup-verify` accepts the dump.
   - A rollback simulation passes (decision 8).

   A **data-only** restore (`pg_restore -a`) into a live schema does fire the triggers through
   COPY. It is not the documented path. The CHANGELOG says so, and names `--disable-triggers`
   (superuser).
8. **Rollback floor v0.11.0, forward-only.** The boot migrator (`run_migrator_timed`) skips
   applied versions that it does not embed, and v0.11.0's probe checks only columns up to
   0018. Slice 03 proves this at store level: a boot-path migrator over a staged set up to
   0018, run against a 0020 schema, is a no-op success. The undo is documented SQL
   (`DROP TRIGGER … ; DROP FUNCTION …`). Because the bodies are re-runnable, re-arming is a
   matter of deleting the 19 and 20 rows in `_sqlx_migrations` and rebooting.

## Alternatives considered

- **`CHECK (…) NOT VALID` per arm.** This was the native named constraint. Rejected (OQ-1):
  Postgres re-checks a CHECK on every UPDATE of the row, so a legacy project would refuse
  every new issue (`lib.rs:1746`), and the first future UPDATE of another `workspaces` column
  would re-arm the same trap.
- **A `DOMAIN` type with the rule.** It is not re-checked on an untouched column. Rejected:
  `ALTER COLUMN … TYPE` coerces, and so checks, every existing row, and COPY during restore
  checks every value. Both refuse legacy rows.
- **One trigger function per table, with the caps hard-coded.** Rejected: that is two copies
  of the trim and refused sets to keep in parity. `TG_ARGV` carries the only difference.
- **One trigger per table, with an `IF TG_OP = 'UPDATE' AND NEW.name IS NOT DISTINCT FROM
  OLD.name` guard inside the function.** Viable, and it creates fewer objects. Rejected: the
  "only when the name changes" rule would then be hidden in a function body instead of being
  visible in `\d <table>`.
- **A GUC bypass for fixtures (`SET LOCAL foundry.allow_legacy_name = on`).** Rejected: any
  role can set a custom placeholder GUC, so it would be a one-line production bypass at the
  very psql prompt the feature guards. It would also put a test flag inside production SQL.
- **A staged migration per legacy acceptance scenario.** It is the most faithful option.
  Rejected for acceptance: the harness migrates each scenario's schema at boot, before any
  Given step runs. Staging would mean re-plumbing the scenario lifecycle for six fixture sites.
  It is kept for the store-level migration tests.
- **`session_replication_role = replica` for fixtures.** Rejected (D8 d): it needs superuser,
  and it also skips FK enforcement triggers. It now *would* skip the name triggers, which is
  why check-arch bans the string.
- **Refuse readiness in `Store::probe` without the triggers.** Rejected (decision 5).

## Consequences

- Positive: every new name write through any client meets the app's exact rule, and the
  error names the arm. Legacy rows keep every capability, including new issues and restore.
- Positive: parity is testable in bulk (10k+ names per table, one round trip) through the
  pure function, and arm for arm, not only accept versus refuse.
- Positive: a Postgres with different semantics refuses the migration rather than installing
  a wrong rule.
- Negative: the rule is a second statement of the Rust rule, in SQL. The parity tests are the
  drift detector. A change to `name_chars`, `str::trim`'s set or a cap needs a new migration
  that replaces the function.
- Negative: `\d workspaces` lists triggers, not check constraints. `pg_constraint` holds
  nothing for the rule.
- Neutral: the table owner can still `DISABLE` or `DROP` the triggers, and a superuser can
  still use replica mode. The rule guards against slips, not against a hostile DBA.
