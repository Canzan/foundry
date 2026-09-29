# RCA: `foundry doctor backup-verify` reports `status: OK` without verifying anything

**Date:** 2026-09-28. **Found by:** the keycloak-sso D3a closing gate (`9abf827`).
**Result:** 854/855; the one failure is `us-03-backup-restore` @us-03-cli, "reports row counts
and exits zero on a healthy backup". It failed 3/3 in isolation.
**Investigator:** nw-troubleshooter. The RCA was not peer-reviewed. The user approved fix
direction (b) plus fail-closed on 2026-09-28.

## Symptom

On a host with no Postgres client, `backup-verify` prints `schema: …`, an **empty**
`row-counts:` block, then `status: OK`, and exits 0. The scenario fails on
`stdout missing row-count line "issues: 4"`.

This reproduces in about a second, with no Docker (verified by the orchestrator):

```sh
FOUNDRY_PG_RESTORE=/usr/bin/true \
FOUNDRY_DOCTOR_PROBE_URL=postgres://nobody:x@127.0.0.1:1/nowhere \
  target/debug/foundry doctor backup-verify <any file>
# … row-counts:  (empty)
# status: OK
# exit 0
```

The verifier reports OK even when its probe database is unreachable.

## Root cause chain

### Branch A: the trigger (missing `psql`)

1. No count lines are printed, because every `count_rows` call returns `Err`.
2. `count_rows` spawns a bare `Command::new("psql")` (`admin_cli.rs:1636`), and the spawn
   fails with ENOENT. `which -a psql pg_restore pg_dump` finds nothing: Homebrew's
   `postgresql@16` is keg-only and not on PATH.
3. `439ee6f` (2026-09-04) moved only `pg_restore` behind the `FOUNDRY_PG_RESTORE` seam and
   the container shim (`pg_backup.rs` `pg_restore_shim`). Both `psql` calls stayed bare:
   the per-invocation `DROP SCHEMA` at `:192` and the count at `:1636`. Both date from
   `b2475f6` (slice 3).
4. Later gates stayed green (e.g. `0beb1d5`, 825/825 on 2026-09-15) because Homebrew's psql
   14 was still on PATH. psql, unlike pg_dump, tolerates a newer server. psql 14 is gone now;
   Homebrew activity on 2026-09-18 is the likely cause (unverified).
5. **Root cause A:** backup-verify depends on two external client binaries, looked up by
   name on PATH. `439ee6f` found its dependencies by what visibly failed, not by listing
   every `Command::new` call. Nothing ran the lane on a PATH with no Postgres client, so
   the claim at `atdd-infrastructure-policy.md:41`, "No host Postgres client is required",
   was never tested.

### Branch B: the defect (the verifier fails open)

1. The command prints `status: OK` with exit 0 although it counted no table.
2. The call site (`admin_cli.rs:178-185`) discards every error with
   `Err(_) => { /* Skip tables not present */ }`. A failed spawn, a failed connection or
   login, a failed parse and a genuinely missing table all look the same.
3. `count_rows` returns `Result<u64, String>`, and its doc comment says an error means
   "table does not exist".
4. No check requires that anything was counted before `status: OK` prints
   (`:198-199` runs unconditionally), and the `DROP` result is discarded with `let _ =`.
5. **Root cause B:** "Tolerate tables that are optional" was implemented as "tolerate every
   failure", in a command whose contract is that cron greps for `status: OK` (RELEASING.md,
   DEVELOPER.md). The same bug reports OK for a dump of the wrong database, which contains
   no Foundry tables at all.

## Contributing factors

- GitHub CI (`.github/workflows/ci.yml:87-92`) still runs
  `apt-get install postgresql-client-16`, so CI hosts always have psql and hide both
  branches. `xtask/src/main.rs:117` says this host-client preflight is retired.
- The `DROP SCHEMA IF EXISTS {schema_name}` at `:195` leaves the schema name unquoted, while
  the count at `:1635` quotes it. Mixed-case names break, and a crafted archive TOC could
  inject SQL (low risk, because the operator supplies the dump).
- The install hint at `:98-101` still says `brew install libpq`.
- Operator impact: any host that has `pg_restore` but no `psql`, or where psql's connection
  differs from pg_restore's, silently gets a false OK. The distroless K8s Job pattern fails
  loudly at step 1 (exit 3).

## Fix (approved: option b plus fail-closed)

- **Count rows and drop the per-invocation schema in the binary with sqlx**, over
  `FOUNDRY_DOCTOR_PROBE_URL`.
  - Use the doctor subcommands' existing `std::thread::spawn` plus current-thread runtime
    pattern (`main` is `#[tokio::main]`, so an in-place `block_on` would panic).
  - Use a single sqlx connection with a timeout, not `Store::connect`, and never migrate.
  - psql leaves both the product and the test path. `pg_restore` stays the only external
    tool.
- **Fail closed.** Only a table that genuinely does not exist
  (`to_regclass(...) IS NULL`) may be skipped.

  | Condition | Exit | Output |
  |---|---|---|
  | Probe unreachable for row counts | **8** | Error on stderr; no `status: OK` |
  | Count query fails on a table that exists | **9** | Error naming the table; no `status: OK` |
  | The restored schema contains none of the known Foundry tables | **10** | "not a Foundry backup?"; no `status: OK` |

- **Quote identifiers** by doubling `"`, including in the DROP. A DROP failure is a warning
  on stderr and is not fatal. The `public` schema is never dropped.
- **CI and docs:**
  - Remove the psql install from `ci.yml`.
  - Document exit codes 8–10 in the `admin_cli.rs` module docs, DEVELOPER.md and
    RELEASING.md.
  - Drop the libpq install hint and correct `atdd-infrastructure-policy.md:41`.
  - Add a CHANGELOG note: runs that used to report a false OK now exit non-zero.

## Regression tests

- **New:** a hermetic `crates/foundry-app/tests/backup_verify_fail_closed.rs`, needing no
  Docker. It sets `FOUNDRY_PG_RESTORE` to a stub that exits 0 and points the probe at an
  unreachable URL. It expects exit 8, no `status: OK` on stdout, and the probe named on
  stderr. It fails today.
- **Existing:** the @us-03-cli "reports row counts" scenario goes green on a host with no
  psql.
- The new branches (absent, counted, connect-fail, zero tables) each need a killing test for
  the ≥80% mutation gate.

## Outcome (2026-09-28)

- **Step 01-01** is green, and DES integrity is complete for 1 of 1 steps.
  - RED: the hermetic tests exited 0 and printed `status: OK` where exits 8, 9 and 10
    were expected.
  - GREEN: `backup_verify_fail_closed` passes 4/4.
- **Refactor (L1–L4):** behaviour unchanged.
- **Adversarial review:** NEEDS_REVISION, with one blocker: `README.md` still told
  contributors to install libpq or `postgresql-client-16`. It was fixed, and the same
  stale prerequisite in `AGENTS.md` was corrected too.
- **Mutation:** **100%** (20 of 20 viable), plus hand-seeded faults H1–H3 (the original
  fail-open, the empty-schema guard and unquoted identifiers), all killed. The full report
  is in `deliver/mutation/mutation-report.md`.
- **Full gate:** `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci` reported **all gates
  green**: 855/855 scenarios and 5974/5974 steps, on a host with no `psql`, `pg_dump` or
  `pg_restore` on PATH.
