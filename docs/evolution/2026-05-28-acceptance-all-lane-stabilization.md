# Evolution — acceptance-all-lane-stabilization

**Finalized**: 2026-05-28
**Ship commits**: [fc3aa94](../../) (connection ceiling + load-gen `@serial`) + [fe3ba9a](../../) (xtask isolate + foundry-app standalone-test fix)
**Wave coverage**: DISTILL only (test-infrastructure hardening; follow-on to gc-transient-state-hardening D8)

## Feature summary

Makes the full `cargo xtask ci` gate green end-to-end — including the release-mode
`@all` acceptance stress lane — for the first time in the project. Resolves two
distinct contention root causes the gate had been masking, and fixes a pre-existing
latent build bug surfaced along the way.

## Business context

`RELEASING.md` gates releases on `cargo xtask ci`, whose acceptance step runs
`FOUNDRY_ACCEPTANCE_TAGS=all`. That gate had been red since before slice 7 (prior
sessions ran targeted tests instead). v0.2.0 was gated on the DEFAULT lane (D8) with
the `@all` instability deferred; this feature closes that deferral so the documented
gate and the release gate agree.

## Root causes + fixes

The `@all`/`xtask ci` instability had **three** independent root causes (the first
already addressed in gc-transient-state-hardening; the latter two here):

1. **Transient/non-monotonic assertions** sampled at one instant — fixed earlier
   (bounded-poll / ordered-subsequence).
2. **Connection-ceiling contention** (`fc3aa94`). One shared `Postgres::default()`
   testcontainer (default `max_connections=100`, ~97 usable) vs. 6 concurrent
   scenarios each with a 10-conn pool, several opening multiple pools. The slice-6
   `db_connections_in_use` scenario's `/readyz` load generator couldn't acquire under
   the ceiling, so the gauge never rose. **Fix:** start the ephemeral container with
   `-c max_connections=300`.
3. **Test-runtime starvation of a load generator** (`fc3aa94`). The same slice-6
   scenario spawns 32 `tokio::spawn` `/readyz` hammer tasks; under `@all` they share
   the test runtime with 5 sibling scenarios and are starved, so they never saturate
   the subprocess pool. **Fix:** tag the scenario `@serial` (de-contended, the hammer
   tasks get the CPU). Raising `max_connections` alone only moved the flake 1/3→1/5,
   confirming it was scheduling, not just the ceiling.
4. **`--workspace` cross-binary OOM + a latent build bug** (`fe3ba9a`). The full
   `cargo xtask ci` then failed at `cargo test --workspace --release` — which ran the
   heavy acceptance suite (300-conn container + subprocesses) **concurrently with
   foundry-app's own container tests**, OOM-killing subprocesses (17 failures one run,
   1 the next). The acceptance run there is redundant (the `@all` step is a superset).
   **Fix:** exclude foundry-acceptance from the `--workspace` step. That required
   foundry-app to **self-enable `test-support`** via a dev-dependency self-reference,
   which also fixed a pre-existing latent bug — `cargo test -p foundry-app` did not
   compile standalone (its tests use `test-support`-gated `AppState` fields; the
   feature was only ever enabled transitively via foundry-acceptance under
   `--workspace` unification).

## Files touched

| Path | Change | Commit |
|---|---|---|
| `crates/foundry-acceptance/src/support/harness.rs` | testcontainer `-c max_connections=300` | `fc3aa94` |
| `crates/foundry-acceptance/tests/features/handler-instrumentation.feature` | slice-6 db_connections scenario tagged `@serial` | `fc3aa94` |
| `xtask/src/main.rs` | `cargo test --workspace --exclude foundry-acceptance --release` | `fe3ba9a` |
| `crates/foundry-app/Cargo.toml` | dev-dependency self-reference enabling `test-support` | `fe3ba9a` |
| `Cargo.lock` | self-dep entry | `fe3ba9a` |

Production code: untouched.

## Verification

- **Full `cargo xtask ci` GREEN end-to-end**: fmt + clippy + build --release +
  `--workspace` (excl. acceptance) unit tests + cargo-deny + dedicated `@all`
  acceptance step at **111/111 (983 steps)**.
- **5 consecutive release-mode `@all` sweeps green** (111/111 each) — commit `fc3aa94`.
- `cargo test -p foundry-app` now compiles + passes standalone (latent bug fixed).
- Default lane remains 107/107 (the v0.2.0 gate, unchanged).

## Lessons learned

1. **Contention has multiple axes.** Within-binary scenario concurrency (`@all`) and
   cross-binary test concurrency (`cargo test --workspace`) are *different* pressures.
   A suite can be green under one and flake under the other. The fix for each differs
   (`@serial` / ceiling for within-binary; isolation for cross-binary).
2. **A scenario that PRODUCES a runtime condition needs de-contention, not just a
   robust assertion.** The slice-6 db_connections scenario must generate sustained
   load; under starvation it can't, and no assertion shape saves it. `@serial` is the
   right tool when the work itself (not just the observation) needs CPU.
3. **Raising a resource ceiling can shift the bottleneck.** `max_connections=300`
   fixed `@all` but pushed memory pressure into the concurrent `--workspace` run.
   Always re-check the next-most-contended consumer after relieving one.
4. **Feature unification hides missing dev-dependencies.** foundry-app's tests
   "worked" only because foundry-acceptance enabled `test-support` for them across the
   workspace. The moment the suite was isolated, the latent bug surfaced. Crates
   should self-enable features their own tests require; relying on a sibling's
   unification is a trap that breaks under `-p` or `--exclude`.
5. **Run the real gate, in full, early.** Every root cause here was masked by *not*
   running `cargo xtask ci` end-to-end. Targeted tests are faster but a green
   targeted run says nothing about the gate the release process actually invokes.

## Issues encountered

- **`@serial` on register-at-0 was a dead end** (handled in gc-transient-state-
  hardening; reverted there).
- **First `--exclude` attempt broke compilation** — exposed the foundry-app latent
  bug, then fixed by the dev-dependency self-reference.
- **`PoolTimedOut`** seen in an earlier 3-`@serial`-at-100 experiment was a transient
  artifact; not reproduced after the ceiling raise.

## Open items

1. **5 deferred metrics** (`outbox_pending_jobs`, `bootstrap_tokens_unclaimed`,
   `migration_apply_duration_seconds`, `realtime_listen_disconnects_total`,
   `probe_failures_total`) — still need dashboard consumers + emission. Genuine
   product work, unrelated to this stabilization.
2. **v0.2.0 push** — the release commit + tag remain prepared and held; the gate is
   now fully green, so `cargo xtask ci` (RELEASING.md step 1) passes.

## Workflow note

Per project convention, the 4-reviewer parallel gate is deferred to PR time.
