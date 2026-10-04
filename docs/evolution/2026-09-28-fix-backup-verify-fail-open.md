# Evolution: fix-backup-verify-fail-open (backup-verify reported OK without verifying anything)

**Finalized:** 2026-09-28.
**Commit:** `6b8460e`, one DES-monitored step (01-01). Shipped in **v0.6.0** (CHANGELOG "Fixed").
**Origin:** the keycloak-sso D3a closing gate (`9abf827`) went 854/855; the one failure was
`us-03-backup-restore` @us-03-cli "reports row counts and exits zero on a healthy backup" (3/3 in
isolation). Run through `/nw-bugfix`; RCA by nw-troubleshooter, not peer-reviewed. The user approved
fix direction (b) plus fail-closed on 2026-09-28. RCA: `docs/feature/fix-backup-verify-fail-open/rca.md`.

## Defect: real, operator-facing

`foundry doctor backup-verify` printed an empty `row-counts:` block, then `status: OK`, exit 0, when
it could not count a single row. Cron is documented to grep for `status: OK` (RELEASING.md,
DEVELOPER.md), so a host without a working `psql` got a silent false OK. The same bug reported OK
for a dump of the wrong database.

## Root cause

- **Trigger:** `count_rows` and the per-invocation `DROP SCHEMA` spawned a bare `psql` from PATH.
  `439ee6f` moved only `pg_restore` behind the `FOUNDRY_PG_RESTORE` seam; Homebrew's psql 14 had
  been masking the gap and was gone by the D3a gate.
- **Defect:** the call site swallowed every `count_rows` error as "table not present", and nothing
  required a table to be counted before `status: OK`. GitHub CI installed `postgresql-client-16`,
  which hid both branches.

## Fix (`crates/foundry-app/src/admin_cli.rs`)

- Row counts and the schema `DROP` run in-binary over sqlx (one connection, 10s connect timeout,
  no migrations). `pg_restore` is the only external tool.
- Only a genuinely absent table (`to_regclass(...) IS NULL`) is skipped. New exits: **8** probe
  unreachable, **9** count failed on a present table, **10** no known Foundry table ("not a Foundry
  backup?"). None print `status: OK`. Documented in RELEASING.md's exit-code table and DEVELOPER.md.
- Identifiers are quoted (the `DROP` was not); `public` is never dropped; the probe is named
  without its password.
- `ci.yml` no longer installs `postgresql-client-16`; README, AGENTS, DEVELOPER, RELEASING and
  `atdd-infrastructure-policy.md` drop the host-client prerequisite.

## Regression tests

`crates/foundry-app/tests/backup_verify_fail_closed.rs`, 4 tests: unreachable probe exits 8
(hermetic, no Docker), failing count exits 9, no Foundry table exits 10, healthy schema counts
present tables, skips absent ones and drops the schema. RED before the fix (exit 0, `status: OK`);
4/4 green after. The existing @us-03-cli scenario goes green on a host with no `psql`.

## Gates

- Mutation (cargo-mutants, `--in-diff`): **100%**, 20/20 viable (17 by the package tests, 3 by the
  `admin-cli` acceptance tag); hand-seeded H1–H3 (fail-open, empty-schema guard, unquoted
  identifiers) all killed. Report: `deliver/mutation/mutation-report.md`.
- Adversarial review: NEEDS_REVISION, one blocker (README still told contributors to install
  libpq / `postgresql-client-16`); fixed, plus the same stale line in AGENTS.md.
- Full `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci`: all gates green, 855/855 scenarios,
  5974/5974 steps, on a host with no `psql`, `pg_dump` or `pg_restore` on PATH.
- Execution log: PREPARE, RED_ACCEPTANCE, RED_UNIT, GREEN executed; COMMIT recorded as
  `APPROVED_SKIP` (the orchestrator committed after the full gate).

## Follow-ups

- Not probed by mutation: the `PROBE_CONNECT_TIMEOUT` value and the panic-join fallback in
  `count_rows_in_probe` (recorded in the mutation report).
- The RCA's "psql 14 vanished because of Homebrew activity on 2026-09-18" is unverified.
- No other follow-up is recorded for this fix.
