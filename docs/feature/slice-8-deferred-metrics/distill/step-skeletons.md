# Step skeletons — slice-8-deferred-metrics (DISTILL)

The step phrases the 11 scenarios need, grouped by reused-vs-new.
SIGNATURES ONLY — DELIVER implements the bodies (RED-scaffolded with
the canonical panic per Mandate 7 / slice-7 precedent:
`Not yet implemented -- RED scaffold (DISTILL); DELIVER finishes this`).

The step phrases registered in the new step module
`crates/foundry-acceptance/src/steps/slice_8_deferred_metrics.rs` ARE
the contract between DISTILL and DELIVER. They MUST NOT change during
GREEN — DELIVER replaces the panic body verbatim with the real
implementation.

## PHRASE-COLLISION CHECK (do this BEFORE adding any step)

Several phrases this slice uses are ALREADY registered by slice-6
(`handler_instrumentation.rs`) / slice-1 Background. DELIVER MUST reuse
them, not re-register (cucumber-rs ambiguous-step error otherwise):

| Phrase | Already provided by | Action |
|---|---|---|
| `a workspace "{}" exists with admin "{}"` | slice-1 Background | REUSE |
| `a member "{}" belongs to the team "{}"` | slice-1 Background | REUSE |
| `a project "{}" with key prefix "{}" exists in the "{}" team` | slice-1 Background | REUSE |
| `the "{}" project already has issue {}-{}` (or AUTH-N shape) | slice-1 Background | REUSE |
| `the operator's foundry instance is running` | slice-6 `handler_instrumentation.rs` | REUSE |
| `the operator scrapes the metrics endpoint` | slice-6 | REUSE |
| `the operator scrapes the metrics endpoint immediately` | slice-6 | REUSE |
| `the scrape returns HTTP 200` | slice-6 | REUSE |
| `the scrape body contains the line "{}"` | slice-6 | REUSE |
| `the scrape body's "{}" sample settles to {} within {} seconds` | slice-6 (register-at-0 hardening) | REUSE |
| `the scrape body's "{}" sample is eventually greater than 0 within {} seconds` | slice-6 | REUSE (template for the new `at least N` variant below) |
| `the foundry subprocess is alive` | slice-7 | REUSE |
| `Mei posts a comment on "{}" with body "{}"` | slice-6 | REUSE (the multi-comment phrase below is new) |

DELIVER VERIFIES each REUSE phrase still resolves before filling in the
new ones — if slice-6/7 bodies were refactored, the reuse may need a
re-point, NOT a duplicate registration.

## NEW Given steps

```rust
// Gauge cadence override (wraps the slice-6 subprocess spawn + sets
// METRICS_POOL_POLL_SECONDS=1 so the 5s pool poll ticks ~1s in test).
#[given(expr = "the operator's foundry instance is running with the gauge poll cadence set to {int} second")]
async fn given_foundry_running_with_gauge_cadence(world: &mut World, seconds: u64);

// Bootstrap-token seeding (direct-SQL fixture, mirrors slice-7
// tombstone_factory direct-SQL approach; production handler untouched).
#[given(expr = "an unclaimed admin bootstrap token that has not yet expired exists")]
async fn given_unclaimed_unexpired_bootstrap_token(world: &mut World);

#[given(expr = "a used admin bootstrap token exists")]
async fn given_used_bootstrap_token(world: &mut World);

#[given(expr = "an expired admin bootstrap token exists")]
async fn given_expired_bootstrap_token(world: &mut World);

// Migration-timing setup — reuses support/test_migration.rs::stage.
#[given(expr = "the operator's foundry instance is staged with one extra migration on top of the production set")]
async fn given_foundry_staged_with_extra_migration(world: &mut World);

#[given(expr = "the operator's foundry instance has already applied its full migration set")]
async fn given_foundry_already_migrated(world: &mut World);

#[given(expr = "the migration-timing observation count has been recorded")]
async fn given_migration_observation_count_recorded(world: &mut World);

// Listen-disconnect setup — dedicated per-scenario Postgres (DD-5; no
// new production seam). Boots the foundry subprocess against a database
// the scenario owns and can restart.
#[given(expr = "the operator's foundry instance is running against a dedicated database it can lose")]
async fn given_foundry_running_against_dedicated_db(world: &mut World);

// Probe-failure setup — pre-bind METRICS_PORT (slice-6 ADR-014 precedent).
#[given(expr = "the metrics port is already bound by another process before boot")]
async fn given_metrics_port_prebound(world: &mut World);
```

## NEW When steps

```rust
// Multi-comment write — enqueues N outbox rows via the COMMIT-time
// NOTIFY trigger. Reuses the slice-6 single-comment POST path in a loop.
#[when(expr = "Mei posts {int} comments on \"{}\"")]
async fn when_mei_posts_n_comments(world: &mut World, n: u32, issue: String);

// Claim admin with the seeded unclaimed token (drives the real claim
// path so the gauge transitions 1 -> 0).
#[when(expr = "the operator claims admin with the unclaimed bootstrap token")]
async fn when_operator_claims_admin(world: &mut World);

// Migration boot.
#[when(expr = "the operator's foundry instance boots and applies its migrations")]
async fn when_foundry_boots_and_migrates(world: &mut World);

#[when(expr = "a second foundry instance boots against the already-migrated schema")]
async fn when_second_foundry_boots_already_migrated(world: &mut World);

// Force a real LISTEN drop by restarting the dedicated database.
#[when(expr = "the realtime LISTEN connection is dropped by restarting that database")]
async fn when_listen_connection_dropped_by_db_restart(world: &mut World);

// Probe-failure boot (expected to refuse to start).
#[when(expr = "the operator's foundry instance attempts to start")]
async fn when_foundry_attempts_to_start(world: &mut World);
```

## NEW Then steps

```rust
// Gauge lower-bound bounded-poll (new variant of the slice-6
// "eventually greater than 0" — wraps poll_until_sample with a >= N
// predicate held to the deadline).
#[then(expr = "the scrape body's \"{}\" sample is eventually at least {int} within {int} seconds")]
async fn then_sample_eventually_at_least(world: &mut World, metric: String, n: i64, secs: u64);

// Counter monotonic bounded-poll (same helper, >= predicate). Phrase is
// identical to the gauge lower-bound; ONE registration serves both
// (counter + gauge are both "value >= N" against poll_until_sample).

// Histogram observation-count bounded-poll (polls the "{name}_count"
// series via histogram_observation_count until >= N).
#[then(expr = "the scrape body eventually contains a \"{}\" observation count of at least {int} within {int} seconds")]
async fn then_histogram_observation_count_at_least(world: &mut World, metric: String, n: u64, secs: u64);

// Histogram no-op semantic — count did not grow vs the recorded baseline.
#[then(expr = "the scrape body's \"{}\" observation count has not grown")]
async fn then_histogram_observation_count_unchanged(world: &mut World, metric: String);

// Label-key bound (reuses ScrapeSnapshot::label_keys_for). The empty
// string ("") asserts NO label keys — used for the 3 unlabelled metrics.
#[then(expr = "the scrape body's \"{}\" samples carry only the label keys \"{}\"")]
async fn then_samples_carry_only_label_keys(world: &mut World, metric: String, csv_keys: String);

// Bounded probe-name value set (the closed {store, metrics} set).
#[then(expr = "the scrape body's \"{}\" samples carry only the probe names \"{}\"")]
async fn then_samples_carry_only_probe_names(world: &mut World, metric: String, csv_names: String);

// Refuse-to-start observables (process boundary, not the scrape — a
// dying process can't reliably serve a final scrape; DISTILL Q4 = log).
#[then(expr = "the foundry subprocess exits non-zero")]
async fn then_foundry_exits_nonzero(world: &mut World);

#[then(expr = "the foundry startup log mentions \"{}\"")]
async fn then_startup_log_mentions(world: &mut World, fragment: String);

#[then(expr = "the foundry startup log mentions probe failure for probe \"{}\"")]
async fn then_startup_log_mentions_probe_failure(world: &mut World, probe_name: String);
```

## Disambiguation notes (cucumber-rs regex hazards)

1. **`carry only the label keys ""`** — the empty-CSV case asserts the
   metric carries NO labels. The step parses the CSV; an empty string
   splits to an empty set. The same phrase with a non-empty CSV asserts
   the exact key set. ONE registration, branch on empty.
2. **`is eventually at least {int} within {int} seconds`** serves BOTH
   gauges and counters (both are `value >= N`). Do NOT register a
   separate counter phrase — the predicate is identical.
3. **`settles to {int} within {int} seconds`** is the slice-6
   register-at-0 phrase — REUSE it (do NOT re-register). It holds the
   `== N` predicate to the deadline (settle, not just touch).
4. **The two `boots ...` Whens** (`boots and applies its migrations`
   vs `second foundry instance boots against the already-migrated
   schema`) are distinct literal phrases — no regex variable collision.

## Helper-reuse confirmation (no new support code)

| Assertion | Existing helper (support/metrics_scrape.rs) |
|---|---|
| gauge `settles to N` / `eventually at least N` | `poll_until_sample(addr, name, predicate, timeout)` |
| counter `eventually at least N` | `poll_until_sample` (>= predicate) |
| histogram `_count` eventually >= N | `poll_until_sample` over `"{name}_count"` / `histogram_observation_count` |
| label-key set | `ScrapeSnapshot::label_keys_for(name)` |
| label-value set (probe names) | `samples_for(name)` + read `labels["probe_name"]` |
| line present + HTTP 200 | `scrape_metrics` / `scrape_metrics_raw` + `contains_metric_line` |

NO new helper, NO parser change, NO new NFR tag (reuse `@nfr-obs-03`).
The only new test code is the step module + the RED-scaffolded
`Store::count_pending_outbox` / `count_unclaimed_bootstrap_tokens`
(production stubs, DELIVER fills) + the World-struct fields for the new
fixtures (dedicated-DB handle, staged-migrations handle, recorded
observation count, prebound-port guard, subprocess exit capture).

## RED-scaffold inventory (Mandate 7)

Files the step bodies import that need RED stubs so the suite COMPILES
(RED, not BROKEN):

- `crates/foundry-store/src/lib.rs` — `Store::count_pending_outbox(&self) -> Result<u64, StoreError>` + `Store::count_unclaimed_bootstrap_tokens(&self, now: OffsetDateTime) -> Result<u64, StoreError>`: production scaffolds returning `Err`/`unimplemented!` per the slice-7 scaffold convention. (The Rust scaffold marker is a `panic!`/`unimplemented!` body, classified RED by the cucumber step failure, NOT an ImportError.)
- `crates/foundry-acceptance/src/steps/slice_8_deferred_metrics.rs` — all NEW step bodies panic with the canonical scaffold marker.
- World-struct field additions (in the acceptance World) for the new fixtures — added as `Option<...>` defaults so the struct compiles.

The pre-DELIVER fail-for-the-right-reason gate (red-classification.md)
runs after scaffolds land: every scenario must panic at its first
slice-8-specific step (MISSING_FUNCTIONALITY), not at an import/fixture
error.
