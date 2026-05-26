# RED classification — slice 6 (handler-instrumentation)

Per nw-distill § "Pre-DELIVER fail-for-the-right-reason gate" (Rust
adaptation). After scaffolds + .feature landed, the slice-6 scenarios
were executed against the RED scaffold step bodies; classification
below.

Command used:
```bash
cargo check -p foundry-acceptance --tests          # gate 1: compile
/path/to/target/debug/deps/acceptance-* -t "@slice6"  # gate 2: run only @slice6 scenarios
```

Gate 1 result: `Finished dev profile [unoptimized + debuginfo] target(s) in 1m 14s` — compile passes.
Gate 2 result: `1 feature, 10 scenarios (10 failed), 70 steps (60 passed, 10 failed)`.

Each failure is the canonical RED scaffold panic emitted from the
slice-6 step bodies in
`crates/foundry-acceptance/src/steps/handler_instrumentation.rs`.
Captured output verbatim: `Not yet implemented -- RED scaffold (DISTILL); DELIVER finishes this`.

Note on the 10th scenario: the cucumber-rs `-t "@slice6"` argv filter
overrides the runner's closure-level `@manual` exclusion, so the
@manual perf-budget scenario #10 ALSO executes in this gated run.
Under the DEFAULT `cargo test` invocation (no `-t` arg), the runner's
closure-level filter at `tests/acceptance.rs` line 96-97 excludes
`@manual` and only the 9 automated scenarios run.

## Per-scenario classification

All 10 scenarios fail at their first slice-6-specific Given (the
"Given the operator's foundry instance is running" step at
`handler_instrumentation.rs:122`), because that's the shortest path
from the inherited slice-1/2 Background to the slice-6-specific
behaviour. The slice-6 RED scaffold pattern matches slice-5 exactly
(every scenario fails at the first new Given; the 60 inherited
Background steps pass GREEN).

Each entry below records: scenario title → classification (category)
→ step that fired the panic.

1. `Operator scrapes the metrics endpoint after a single comment POST and sees the request reflected in the counter and the histogram` → **RED (MISSING_FUNCTIONALITY)** → `Given the operator's foundry instance is running` (step body panics with scaffold message; the `FoundrySubprocess::spawn` is the missing implementation DELIVER fills)
2. `The request counter breakdown distinguishes route templates, methods, and statuses` → **RED (MISSING_FUNCTIONALITY)** → same Given step
3. `A request to a parameterized route emits the route template as the path label, never the concrete URI, and carries no forbidden high-cardinality labels` → **RED (MISSING_FUNCTIONALITY)** → same
4. `After the operator issues N HTTP requests, the counter sum across all label combinations equals N exactly` → **RED (MISSING_FUNCTIONALITY)** → same
5. `The Postgres connection pool gauge reflects the in-use connection count within one polling interval` → **RED (MISSING_FUNCTIONALITY)** → same
6. `Immediately after process start, the connection-pool gauge is scrapable at value 0 so Grafana sees the metric line without a delay` → **RED (MISSING_FUNCTIONALITY)** → same
7. `When a viewer opens an SSE subscription the subscriber gauge increments and returns to zero after the viewer closes cleanly` → **RED (MISSING_FUNCTIONALITY)** → same
8. `When a viewer's SSE stream is abruptly dropped mid-poll the subscriber gauge still decrements via the RAII guard's Drop` → **RED (MISSING_FUNCTIONALITY)** → same
9. `The process refuses to serve traffic until the self-scrape probe confirms the metrics endpoint is reachable and the startup counter line is present` → **RED (MISSING_FUNCTIONALITY)** → same
10. `The request-tracking middleware adds no more than ten microseconds P95 of overhead per request` (@manual) → **RED (MISSING_FUNCTIONALITY)** → same (the @manual scenario also panics; this is correct — DELIVER ships the criterion microbench separately per sub-deliverable F, the cucumber scenario is the documentation anchor)

## Failure-mode categories

- **MISSING_FUNCTIONALITY** (correct RED): 10 of 10 — slice-6
  production code (middleware factory, pool stats + poll task,
  SubscriberGauge, startup probe, events.rs guard wire-up) is not
  yet implemented; the step body panics with the scaffold marker.
  DELIVER's responsibility.
- **IMPORT_ERROR / FIXTURE_BROKEN / SETUP_FAILURE** (wrong RED):
  0 of 10. The test infrastructure (per-scenario PG schema via
  slice-1 `fresh_schema_pool_with_url`, Postgres testcontainer,
  slice-1/2 Background step modules, World struct with the new
  slice-6 fields) is all sound; only the slice-6 step bodies panic.
- **WRONG_ASSERTION / OBSERVABLE_NOT_AT_PORT** (wrong shape): 0 of 10.
  The assertions are at the right port — the `/metrics` GET endpoint
  is the operator's observable surface; the Then steps assert on the
  scraped body, which is the production contract Prometheus consumes.

All 60 background steps pass GREEN (slice-1 + slice-2 inherited
workspace + team-member + project + issue + sign-in seeding). No
infrastructure or fixture failure was observed; the only failures
are the deliberate scaffold panics.

Pre-DELIVER gate: **PASSED** — proceed to DELIVER under ADR-025 D2
(DELIVER RED phase = unskip these scaffolds, write PBT unit tests
for the cardinality static check + the SubscriberGauge panic-unwind
+ the probe failure-injection per ADR-014 § Verification, then
implement the 6 sub-deliverables per `step-skeletons.md`).

## DELIVER read-back instructions

When DELIVER picks up:

1. The 10 slice-6 scenarios are all live (no `@skip` / `@ignore`
   tag). Each panics on its first slice-6-specific Given — that's
   the correct entry point for the GREEN phase. The 10th scenario
   (`@manual`) is excluded from the DEFAULT cucumber run by the
   runner's closure filter; it executes only under explicit `-t`
   tag selection or `FOUNDRY_ACCEPTANCE_TAGS=all`.
2. Cucumber-rs treats `panic!` from a step body as a step failure
   with the panic message as the captured output (verified above).
   DELIVER does NOT need to change the step bodies' panic-to-
   implementation pattern — replace the body verbatim with the real
   implementation.
3. The step phrases (regex strings) registered in
   `crates/foundry-acceptance/src/steps/handler_instrumentation.rs`
   ARE the contract between DISTILL and DELIVER. They MUST NOT
   change during GREEN. If a phrase reads awkwardly during
   implementation, surface it as a DELIVER → DISTILL retro item,
   not a unilateral rename.
4. The `FoundrySubprocess::spawn` is the SINGLE most-leveraged
   implementation: 9 scenarios depend on it. DELIVER's first move
   is most efficiently to land `FoundrySubprocess::spawn` + the
   `scrape_metrics` helper + the simplest Then assertion (e.g.
   `then_scrape_returns_200`); that unblocks scenario #1 (the WS).
   The remaining scenarios then unblock incrementally as sub-
   deliverables A-E land.
5. The `support/metrics_scrape.rs` parser is also a RED scaffold
   (the `scrape_metrics` + `scrape_metrics_raw` + `parse_exposition`
   functions all panic). DELIVER fills in the Prometheus text-
   exposition parser per the function signatures (the SHAPE is the
   contract; the implementation is DELIVER's).
6. The scenario count is 10 (above the 7-9 prompt cap per the
   brief's "8-10" target). Per `proposals.md` § "Scope confirmation",
   merging is NOT recommended (the 5 metric families + cardinality
   + 2 SSE round-trips + startup-probe + perf-budget = 10 distinct
   contracts; merging would lose granularity).
