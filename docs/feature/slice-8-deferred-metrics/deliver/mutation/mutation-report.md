# Mutation Report — slice-8-deferred-metrics

**Date**: 2026-05-29
**Tool**: cargo-mutants 25.3.1 (Rust)
**Scope**: feature-scoped — production lines changed by the ship commit `73aee8f`
(`--in-diff`), across `foundry-app`, `foundry-realtime`, `foundry-store`.
**Test command**: the 11 slice-8 acceptance scenarios only
(`FOUNDRY_ACCEPTANCE_TAGS=tag:slice8 cargo test -p foundry-acceptance -p foundry-app`).
**Build profile**: debug, serial (`-j 1`), per-mutant test timeout floor 300s.

## Result (after gap fixes)

| Metric | Value |
|---|---|
| Mutants tested | 13 |
| Caught | 13 |
| Missed (survived) | 0 |
| Timeouts / unviable | 0 / 0 |
| **Kill rate** | **100%** |
| Gate verdict | **PASS** (≥ 80%) |

The first valid run scored 76.9% (10/13) with 3 surviving mutants. All three
were genuine test gaps; the test additions below closed them and a re-run
confirmed **13/13 caught**. slice-8 was already shipped + finalized (`73aee8f`);
these are test-quality hardening commits on top.

## Gap fixes applied

| Former survivor | Fix |
|---|---|
| `migration_id_label -> String::new()` | Scenario #5 now asserts the histogram samples **include the `migration_id` value `0001_init`** (the real `{version:04}_{desc}` stem), not just the label key. |
| `migration_id_label -> "xyzzy".into()` | Same assertion — a constant value `!= "0001_init"` fails it. |
| `record_probe_result -> Ok(())` | New scenario "A failing startup store probe refuses to start…" boots against a dedicated container whose migration-0006 `comments` columns are dropped, so the `store` probe fails **through `record_probe_result`**. Asserts non-zero exit + the `"startup store probe failed"` cause. Under the mutant the failure is swallowed and the process keeps serving (no exit) → caught. |

Test changes (production code untouched):
- `crates/foundry-acceptance/tests/features/slice-8-deferred-metrics.feature`
  — +1 assertion on #5, +1 store-probe-failure scenario.
- `crates/foundry-acceptance/src/steps/slice_8_deferred_metrics.rs`
  — `then_samples_include_migration_id_value`, the store-probe Given/When,
  and `spawn_subprocess_expecting_store_probe_failure`.
- `crates/foundry-acceptance/src/world.rs` — `slice8_store_probe_db` field.

Note uncovered while writing the store-probe scenario: `Store::probe()`'s
migration-0006 column check (`crates/foundry-store/src/lib.rs:148`) queries
`information_schema.columns WHERE table_name = 'comments'` **without a
`table_schema` filter**, so it counts columns across *all* schemas, not just the
active search_path (contrary to its code comment). Harmless in production
(single schema) but worth tightening; the test works around it with a dedicated
single-schema container.

## Original surviving mutants (3) — pre-fix, all genuine test gaps

### 1 & 2. `migration_id_label` value is never asserted (2 mutants)

- `crates/foundry-store/src/lib.rs:1512` — `migration_id_label -> String::new()` **survived**
- `crates/foundry-store/src/lib.rs:1512` — `migration_id_label -> "xyzzy".into()` **survived**

The two `@migration-histogram` scenarios and the `@cardinality` scenario assert
that `migration_apply_duration_seconds` samples *carry only the label keys
`migration_id`* — i.e. they check the label **key** is present and bounded. No
scenario asserts the label **value** equals the real migration identifier. So
emitting `migration_id=""` or `migration_id="xyzzy"` for every migration passes
all assertions.

**Fix**: in "Each migration that actually applies records one timing observation
labelled with its migration id" (feature line 209), assert the observation
carries a `migration_id` value matching an actual applied migration (e.g. the
known production filename stem, or simply `migration_id != ""` / matches
`^\d+_`). That pins `migration_id_label`'s output.

### 3. `probe_failures_total` increment is never scraped (1 mutant)

- `crates/foundry-app/src/main.rs:509` — `record_probe_result -> Ok(())` **survived**

`record_probe_result` is the call site that increments `probe_failures_total`.
The failure scenario ("A startup probe failure increments the probe-failure
counter…", feature line 274) deliberately asserts only the **exit code** and the
**`health.startup.refused` log line** — by design, because "a dying process
cannot serve a final scrape reliably" (scenario comment). The healthy scenario
asserts the counter *settles to 0*. Neither path observes the counter actually
*increment*, so making the recorder a no-op survives.

**Fix options** (note the design tension — the refuse-to-start path genuinely
can't be scraped post-mortem):
- Add a probe-failure mode that does **not** refuse to start (a soft/degraded
  probe), then scrape `probe_failures_total{probe_name="…"} >= 1`. Requires a
  product decision on whether any probe is non-fatal.
- Or accept this as a documented residual: the counter increment is covered by
  the `metrics_server.rs` unit test's register-at-0 path, and the call site is
  one line; the behavioral risk is low. If accepted, record it as a known
  surviving mutant rather than adding a contrived seam.

## Caught mutants (10)

All value- and control-flow-bearing slice-8 logic is killed by the scenarios:

- `Store::count_pending_outbox -> Ok(0)` and `-> Ok(1)` — killed by the
  outbox-gauge "≥ 3 after 3 comments" assertion.
- `Store::count_unclaimed_bootstrap_tokens -> Ok(0)` and `-> Ok(1)` — killed by
  the "settles to 1 / drops to 0" gauge assertions.
- `run_pg_listener` body → `()` — killed (listener gutted ⇒ realtime path fails).
- `run_migrator_timed -> Ok(Default::default())` and the `!=`→`==` guard flip —
  killed by the histogram observation-count assertions.
- `run_migrations -> Ok(())` and `Store::migrate -> Ok(())` — killed (no schema
  ⇒ the harness's in-process setup and the booted subprocess both fail).
- `main -> Ok(())` — killed (the subprocess does nothing ⇒ scenarios fail).

## Methodology note (important for re-running in this repo)

slice-8's acceptance scenarios are `@real-io`: they **spawn the `foundry` binary
as a subprocess** (`assert_cmd::cargo_bin("foundry")`). cargo-mutants must
rebuild that binary with the mutation, or the subprocess silently runs
un-mutated code and nearly every mutant falsely "survives".

- A first pass using only `--test-package foundry-acceptance` scored **15%**
  (2/13). cargo-mutants rebuilt the test binary + libraries but **not** the
  `foundry` bin (a separate compile target in `foundry-app`), so the spawned
  subprocess executed the baseline binary. Only the 2 mutants the harness *also*
  exercises **in-process** (`run_migrations`, `run_migrator_timed` via
  `support/harness.rs` schema setup) were caught.
- Adding **`--test-package foundry-app`** forces the `foundry` bin to recompile
  per mutant (verified: `Compiling foundry-store → foundry-app → foundry-acceptance`
  with the bin re-linked). Kill rate then rose to the true **76.9%**.

**Rule for future feature-scoped mutation runs here**: any feature whose
acceptance scenarios spawn the binary must include `--test-package foundry-app`
(or `--test-workspace`) so the mutated binary is the one under test.

A temporary `FOUNDRY_ACCEPTANCE_TAGS=tag:<tag>` passthrough was added to
`crates/foundry-acceptance/tests/acceptance.rs` to scope the per-mutant suite to
just the feature's 11 scenarios; it is reverted after the run (not part of CI
lanes).

## Reproduce

```bash
git show 73aee8f -- crates/foundry-app/src/main.rs crates/foundry-app/src/metrics_server.rs \
  crates/foundry-realtime/src/lib.rs crates/foundry-store/src/lib.rs > /tmp/slice8-prod.diff
# (re-add the tag: passthrough arm to acceptance.rs first)
FOUNDRY_ACCEPTANCE_TAGS=tag:slice8 cargo mutants \
  --in-diff /tmp/slice8-prod.diff \
  --test-package foundry-acceptance --test-package foundry-app \
  -j 1 --minimum-test-timeout 300 \
  --output docs/feature/slice-8-deferred-metrics/deliver/mutation/
```
