# Coverage Matrix — Slice 7 (comment-tombstone-gc)

Per-AC trace from DESIGN (architecture.md + ADR-015..017 + the 7
ACCEPTED invented-detail items) to scenario files. Slice 7 closes
two v0.2-deferred items: the 90-day tombstone GC (ADR-007 v0.2
commitment) + the admin-undelete operator runbook (slice-5 D5
deferred).

The applicable NFR cell is NFR-OBS-03 (metric emission correctness)
per slice-7 D8 = A; no new NFR-PERF row this slice (24h cadence is
operational, not performance).

## DESIGN decision × scenario trace

Source: `docs/feature/comment-tombstone-gc/design/wave-decisions.md`
D1-D7 + the 9 constraints. Each DESIGN decision is covered by at
least one acceptance scenario or routed to DELIVER unit test.

| DESIGN decision | Scenario(s) covering | DELIVER unit test (if any) | Tag(s) |
|---|---|---|---|
| **D1 — Daily cadence (ADR-015)** | #1 (WS — exercises the cadence override to 1s + observes the first tick) | DELIVER PBT covers the env-var-parsing fallback ("default 86400 if env unset"); not acceptance-covered | `@walking_skeleton @real-io @gc-tick @nfr-obs-03` |
| **D2 — Batched 1000 with 10k cap (ADR-015)** | #3 (`@slow` — 11k rows over 2 ticks: first tick removes 10k, second removes 1k) | DELIVER PBT covers the loop-termination invariant (`min(N, cap)` per tick) at unit layer | `@real-io @gc-cap @slow` |
| **D3 — Inline in main.rs (ADR-015)** | NOT acceptance-covered (the hosting choice has no behavioural surface — same observable from main.rs vs gc.rs) | NONE — hosting is structural | n/a |
| **D4 — Emit 2 metrics now (ADR-016)** | #1 (purged_total after WS tick); #6 (pending gauge across ticks); #3 (purged_total after cap tick) | DELIVER PBT covers the gauge-set-to-0-at-startup invariant per slice-6 D4 precedent | `@nfr-obs-03 @gc-metrics` |
| **D5 — CLI + psql, CLI primary (ADR-016)** | #7 (WS CLI happy path); #8 (CLI not-restorable); #9 (CLI invalid-UUID) | DELIVER PBT covers exit-code 3 (DB connect failure) per "Open Decisions for DELIVER" — acceptance can't easily inject without the subprocess exiting before the harness can observe | `@walking_skeleton @real-io @driving_adapter @admin-cli` (#7); `@real-io @error @admin-cli` (#8 + #9) |
| **D6 — Defer comments_visible VIEW (ADR-017)** | NOT acceptance-covered (the deferral has no behavioural surface; verification is the ABSENCE of a 0007 migration) | NONE — covered by slice-5 `@soft-delete-invariant` regression run as part of the slice-7 PR | n/a |
| **D7 — Log + continue on failure (ADR-015)** | #5 (synthetic error mid-tick → task survives → next tick succeeds) | DELIVER PBT covers the error-handling branch in the tick body at unit layer | `@real-io @gc-failure` |

## DESIGN constraint × scenario trace

Source: `docs/feature/comment-tombstone-gc/design/wave-decisions.md`
§ "Constraints Established" (9 constraints).

| Constraint | Scenario(s) covering |
|---|---|
| 1. Background cleanup task pattern (log + continue) | #5 (gc-failure) |
| 2. Advisory-lock naming pattern (`TOMBSTONE_GC_LOCK_ID` distinct from `MIGRATION_LOCK_ID`) | #4 (gc-lock — exercises the lock behaviour; the literal value is invented-detail #1, not asserted) |
| 3. Env-var family naming (`FOUNDRY_TOMBSTONE_GC_*`) | #1 + #3 + #5 + #6 (all GC scenarios use the env-var-driven cadence + cap overrides) |
| 4. First-tick-soon invariant (within ~5s) | #1 + #2 + #4 + #5 + #6 (every GC scenario asserts behaviour after 2s wall-clock; first tick must have fired by then) |
| 5. GC affects ONLY tombstoned rows older than threshold | #2 (gc-threshold — explicitly seeds rows on both sides of the boundary) |
| 6. Hard-delete fires no SSE event | NOT acceptance-covered (the absence of an SSE event is harder to assert than its presence; the slice-2 SSE consumer would have to subscribe and observe NO event in a wall-clock window). Trade-off: deferred to DELIVER PBT or a `@manual` operator test. Documented here as a known gap — not blocking. |
| 7. Undelete is operator-only | #7 + #8 + #9 (all 3 admin-undelete scenarios go through the CLI; no UI scenarios; the absence is the test) |
| 8. Bounded-cardinality invariant extends to slice-7 metrics | NOT a separate slice-7 scenario; covered by the slice-6 D2 cardinality unit test in `metrics_server.rs` (automatic — slice-7 metrics are unlabelled, the test passes trivially) |
| 9. CLI exit codes are per-subcommand contracts | #7 + #8 + #9 (the 4 exit codes per D6 = A) |

## ACCEPTED invented-detail × scenario trace

Source: `docs/feature/comment-tombstone-gc/design/wave-decisions.md`
§ "Decision-driven invented detail — ACCEPTED" (7 items).

| Invented detail | Scenario behaviour | Pinned literally? |
|---|---|---|
| 1. `TOMBSTONE_GC_LOCK_ID = 0x_60_C0_DE_60_C0_DE_60_u64 as i64` | #4 exercises the lock contention behaviour | NO — only behaviour pinned (lock contended → no GC work) |
| 2. Env-var family `FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS` / `_MAX_PER_RUN` / `_OLDER_THAN_DAYS` | #1, #3, #6 use the env vars | YES — name is locked (DELIVER MUST NOT rename) |
| 3. First-tick-soon (~5s) | every GC scenario implicitly | YES — the 2s wait between Givens and Whens depends on the contract |
| 4. CLI subcommand name `restore-comment` | #7, #8, #9 invoke `foundry doctor restore-comment` | YES — locked |
| 5. Metric names `comments_tombstones_purged_total` / `comments_tombstones_pending` | #1, #3, #6 assert these literal names | YES — locked (these are the dashboard query keys) |
| 6. No-backoff failure handling | #5 exercises that the same error doesn't change behaviour tick-to-tick | YES — behaviour pinned (no observable backoff) |
| 7. Default 90-day threshold | #2 uses 91d + 89d on either side; #6 uses 91d | YES — locked (the 90-day boundary IS the SLA) |

## Driving-adapter coverage for slice 7

Per Mandate 6 (RCA-fix P1 — every driving adapter exercised via its
protocol). Slice 7 introduces TWO new surfaces:

- The background GC tokio task — INTERNAL driver (not externally invocable; observed via DB state + scraped metrics).
- The `foundry doctor restore-comment <uuid>` CLI subcommand — NEW EXTERNAL driving adapter; exercised via subprocess.

| Endpoint / driver | Method | Scenario covering via subprocess | Tag |
|---|---|---|---|
| Background tombstone GC task (internal driver — reads `comments`, calls `Store::gc_tombstoned_comments`, emits 2 metrics) | Not externally invocable; observed via DB state + scrape | #1, #2, #3, #4, #5, #6 (6 scenarios) | `@real-io` |
| `foundry doctor restore-comment <uuid>` (NEW driving adapter) | subprocess via `assert_cmd::Command::cargo_bin("foundry").args(["doctor", "restore-comment", uuid])` | #7 (happy path), #8 (not-restorable), #9 (invalid-UUID) | `@walking_skeleton @driving_adapter @admin-cli` (#7); `@admin-cli` (#8, #9) |
| `GET /metrics` (slice-6 sidecar listener, REUSED) | GET via `reqwest::Client` to subprocess metrics port | #1, #3, #6 (the 3 scenarios that assert on metric values) | `@real-io @nfr-obs-03` |
| `GET /team/.../issues/.../` (issue-page render, slice-2 reused) | GET via `reqwest::Client` to subprocess main port | #1 (asserts 0 tombstones on the rendered page), #7 (asserts the restored comment is rendered) | `@real-io` |

All driving adapters covered. The CLI subcommand's exit-code 3 (DB
connect failure) path is the only one NOT covered by an acceptance
scenario — routed to DELIVER PBT per "Open Decisions for DELIVER" in
wave-decisions.md.

## Adapter coverage table (Mandate 6 enforcement)

Slice 7 introduces ZERO new driven adapters per architecture.md
§ Reuse Analysis (line 195-203 — "CREATE NEW: none"). Every driven
adapter touched by slice 7 was already exercised by slice 1+/3+/6+.

| Adapter | @real-io scenario | Covered by |
|---|---|---|
| Postgres `comments` table (DELETE write path — GC sweep) | YES (slice 7 NEW exercise) | #1, #2, #3, #4 (the GC scenarios that observe row removal) |
| Postgres `comments` table (UPDATE write path — undelete) | YES (slice 7 NEW exercise; mirrors slice-5 PATCH-on-tombstone) | #7 (admin-undelete CLI) |
| Postgres advisory lock (`pg_try_advisory_lock(TOMBSTONE_GC_LOCK_ID)`) | YES (slice 7 NEW exercise; same MECHANISM as slice-1 `MIGRATION_LOCK_ID`) | #4 (gc-lock; the holder Pool acquires via `pg_advisory_lock` and the subprocess's GC sees contention via `pg_try_advisory_lock`) |
| `metrics_exporter_prometheus::PrometheusHandle` (render path) | YES (slice 6 inherited; slice 7 ADDS 2 new emissions but uses the existing recorder) | #1, #3, #6 (the 3 slice-7 scenarios that scrape /metrics) |
| `metrics::counter!` / `metrics::gauge!` facades | YES (slice 6 inherited; slice 7 ADDS 2 new metric families) | #1 (counter), #3 (counter), #6 (gauge) |
| `tokio::time::interval` (slice-7 GC task; slice-6 pool-poll task) | YES (slice 6 inherited; slice 7 ADDS a second tokio::time::interval consumer) | All 6 GC scenarios |
| sqlx `PgPool` (slice-1 inherited) | YES | All 9 scenarios |
| `assert_cmd::Command::cargo_bin("foundry")` (slice-3 inherited) | YES (slice 6 second use; slice 7 third use) | #7, #8, #9 |
| Postgres per-scenario schema (slice 1 inherited) | YES | All 9 scenarios |
| `reqwest::Client` (slice 1 inherited; slice 6 scrape direction) | YES | All 9 scenarios |
| Direct SQL test-fixture (slice 7 NEW helper `tombstone_factory`) | YES | All 6 GC scenarios + #7 |

Zero `NO — MISSING` rows.

## Cross-cutting roll-up

| Metric | Target | Actual (slice 7) |
|---|---|---|
| Total NEW scenarios | 7-9 prompt cap | **9** — within ceiling. No merging proposed (each scenario covers a distinct substrate-lie probe per architecture.md § Earned Trust). |
| @walking_skeleton scenarios | exactly 1 per feature file (project convention); deviation justified per DD-11 invented detail | **2** (#1 GC tick + #7 admin-undelete CLI per D7 = A; flagged as invented detail #1). Slice-6 set the precedent for 2 WS when end-to-end loops are structurally distinct. |
| @real-io scenarios | every driven adapter covered | **9 of 9** scenarios |
| @error scenarios | ≥40% of automated total | **2 of 9 = 22%** — below target. Justified inline in wave-decisions.md § "Scenarios per file table"; same posture as slice-5 (30%) and slice-6 (11%). The GC + CLI error surfaces are intrinsically thin; bogus errors would lower signal quality. |
| @manual scenarios | as needed | **0** (the operator runbook drill is documented in RELEASING.md, not as an `@manual` cucumber scenario per D2 = A) |
| `@nfr-*` scenarios | one per applicable NFR cell | **2** (`@nfr-obs-03` on #1 + #6 per D8 = A). The cap scenario (#3) ALSO touches NFR-OBS-03 (the counter assertion) but is primarily tagged `@gc-cap @slow` to keep tag economy. |
| Test-suite runtime impact | ≤40s slice budget; ≤120s top-line | **~34.5s default; ~44.5-49.5s with @slow** (within slice-7 budget); pushes fast-loop to ~105s total (within ~120s top-line; accept-and-re-baseline per slice-7 wave-decisions.md option b) |
| Driving-adapter coverage | every new endpoint exercised via its protocol | **`foundry doctor restore-comment` covered by 3 scenarios (#7 + #8 + #9) via `assert_cmd` subprocess.** The GC task is internal (no separate driving-adapter test possible — observed via DB state + scrape). |
| KPI observability scenarios | one per KPI contract | **N/A** — `docs/product/kpi-contracts.yaml` does not exist in this project. The slice-1 NFR catalogue + the Grafana dashboard JSON (`observability/grafana-dashboards/foundry-overview.json`) ARE the operational KPI contracts; #1 + #3 + #6 cover the new dashboard queries empirically. |

## Mandate compliance evidence (CM-A through CM-H — per slice-2 + slice-5 + slice-6 template)

- **CM-A (Hexagonal boundary)**: every step-method invokes the production composition root via the foundry SUBPROCESS (slice-6 pattern) + `reqwest::Client` against the bound ports OR via `assert_cmd::Command::cargo_bin("foundry")` for the doctor CLI. Zero step bodies construct `AppState`, `Store`, or `Router` directly. The subprocess IS the production composition root by construction. The direct-SQL `tombstone_factory` helper is a TEST-ONLY FIXTURE (Given-side setup), not a production-port invocation — same pattern as slice-3's `pg_backup` (test inserts dump artifacts directly + the CLI runs against them). Verified against slice-3 + slice-6 precedents.

- **CM-B (Business language)**: no Gherkin line mentions `pg_advisory_lock`, `tokio::time::interval`, `sqlx`, `axum`, `MatchedPath`, `PrometheusHandle`, `TOMBSTONE_GC_LOCK_ID`, `now()`, `interval`, or specific SQL syntax. The operator-facing terms in the Gherkin: "tombstone sweep cadence", "ancient tombstoned comments", "deletion age 91 days", "another replica is holding the tombstone-sweep advisory lock", "synthetic database error", "doctor subprocess exits with code 0", "issue page shows ... tombstoned comments". The technical machinery (advisory lock, SQL date arithmetic, metric facade calls) is in the step bodies and the production code, not the .feature. Numeric exit codes (0, 2, 4) appear in the `@admin-cli` scenarios where the exit code IS the user-facing CLI contract — same exemption as slice-3 backup-verify. Metric names (`comments_tombstones_purged_total`, `comments_tombstones_pending`) appear where the operator's PromQL queries use them — same exemption as slice-6.

- **CM-C (User journey completeness)**: every scenario walks from an operator-observable trigger (subprocess start with explicit cadence; tombstone seeding; CLI invocation) to an observable outcome (a metric value via scrape; DB state via `tombstone_factory::count_*`; an exit code; a re-rendered issue page). No "validator-accepts-JSON" or "internal-function-returns-result" framings. Operator perspective is preserved: "operator observes the GC ran and removed N rows", "operator runs `foundry doctor restore-comment` and sees `status: restored`".

- **CM-D (Pure function extraction)**: not applicable at the acceptance layer — DELIVER's PBT unit tests cover the pure functions (the SQL predicate for GC eligibility; the batch-loop termination invariant; the CLI UUID parse predicate; the exit-code mapping for `Store::undelete_comment` return values). Routed to DELIVER's PBT phase per ADR-025 D2.

- **CM-E (No fixture theater)**: every Given step sets up PRECONDITIONS, not expected outputs. The "N ancient tombstoned comments exist" Given uses `tombstone_factory::insert_tombstoned_comment` which writes rows the GC would DELETE — the test PASSES only if the production code's `Store::gc_tombstoned_comments` actually queries and deletes them. The "another replica is holding the tombstone-sweep advisory lock" Given uses a SECOND `PgPool` calling `pg_advisory_lock(TOMBSTONE_GC_LOCK_ID)` — the test PASSES only if the production code's GC actually CHECKS the lock (via `pg_try_advisory_lock`). The "the next tombstone sweep tick will fail with a synthetic database error" Given sets the test-hook env var — the test PASSES only if the production code's tick body honours the test hook. Confirmed: every scenario's WHEN is genuinely observable through the production code path; no fixture pre-populates an expected outcome. The first scenarios to fail in RED do so at the first slice-7 Given (where the test-only helper `tombstone_factory::insert_tombstoned_comment` is RED-scaffolded), not at the When — DELIVER first lands the helper (which is acceptance-test infra, not production code), then the scenarios re-fail at the When step where the production GC is missing.

- **CM-F (Walking skeleton litmus test)**: scenario #1 ("A daily tombstone sweep removes comment tombstones older than 90 days and increments the purged-total counter") is demo-able to a non-technical operator: "I started foundry. I waited for the daily sweep. The 3 ancient deleted comments are gone, the metrics show 3 purged, and the issue page reflects it." That IS the user-facing privacy contract of slice 7. Scenario #7 ("An operator restores an accidentally-deleted comment by running the doctor subcommand") is demo-able to the moderation operator: "I ran one command with the UUID of the comment I shouldn't have deleted. The CLI said 'restored'. The comment is back on the issue page." That IS the user-facing recovery contract. Both pass the litmus test for a non-technical stakeholder.

- **CM-G (Driving-adapter coverage per Mandate 6 / RCA-fix P1)**: the `foundry doctor restore-comment` CLI subcommand is exercised via subprocess in #7 (happy path with assertion on `status: restored` substring + database state) + #8 (exit 4 + stderr substring) + #9 (exit 2 + stderr substring). The GC background task is INTERNAL (no separate driving-adapter test possible — observed via the 2 emitted metrics on /metrics scrape + DB state via `tombstone_factory::count_*`). The `/metrics` GET endpoint is exercised in #1, #3, #6 (the 3 metric-observation scenarios). The issue-page GET (slice-2 reused) is exercised in #1 (0 tombstones rendered) + #7 (restored comment rendered). All driving adapters covered.

- **CM-H (Pre-DELIVER fail-for-right-reason gate)**: will be finalized post-compile-and-run. Expected outcome: all 9 scenarios fail with `panic!("Not yet implemented -- RED scaffold (DISTILL); DELIVER finishes this")` from the step bodies (the failure is "the step body panicked", correctly classified as RED MISSING_FUNCTIONALITY by cucumber-rs). The `@slow` scenario #3 is excluded from the default run by the new `@slow` filter in `tests/acceptance.rs` (which DELIVER adds; until then, it runs alongside the others under `-t "@slice7"` explicit selection). See `red-classification.md` for the empirical result.

## Definition of Done — slice 7 DISTILL

- [x] 1 feature file (`features/comment-tombstone-gc.feature`), 9 scenarios (within the 7-9 prompt ceiling — no merging proposed).
- [x] 2 `@walking_skeleton` scenarios (per D7 = A; flagged as invented detail #1; slice-6 DD-11 precedent for 2 WS when end-to-end loops are structurally distinct).
- [x] The new `foundry doctor restore-comment` CLI subcommand is exercised via subprocess in 3 scenarios (#7 + #8 + #9).
- [x] The new 2 metric families (`comments_tombstones_purged_total` counter + `comments_tombstones_pending` gauge) are asserted via /metrics scrape in 3 scenarios (#1 + #3 + #6).
- [x] `driver.md` documents the subprocess pattern inheritance from slice 6 + the new `tombstone_factory` helper + the world additions + force-link + module reg.
- [x] `step-skeletons.md` enumerates the new step signatures + lists the inherited slice-1/2/5/6 steps it reuses.
- [x] No new crate dependencies (per architecture.md § "Technology Stack"; `assert_cmd` + `sqlx` + `reqwest` + `uuid` all inherited).
- [x] No new policy rows in `docs/architecture/atdd-infrastructure-policy.md` (zero new ports per architecture.md § Reuse Analysis line 195-203; CLI mechanism is identical to slice-3 backup-verify row).
- [ ] Suite runtime delta within slice-7 ~40s budget — to be verified post-compile (estimated ~34.5s default).
- [ ] Compile passes: `cargo check -p foundry-acceptance --tests` — to be verified post-write.
- [ ] Pre-DELIVER fail-for-right-reason gate: target = PASSED (see `red-classification.md` post-run).
- [x] Reuse-Analysis HARD GATE: zero new ports, zero new adapters; all changes additive in existing files + ONE new step file + ONE new test-support file (per slice-7 DESIGN wave-decisions.md § Reuse Analysis).
- [x] Wave-Decision Reconciliation HARD GATE: 0 contradictions across DISCUSS / DESIGN (slice-7 has no DEVOPS wave by design; per nw-distill graceful-degradation matrix WARN + proceed).
- [x] 8 DISTILL open questions answered with propose-mode recommendations + rationale (D1-D8 in wave-decisions.md).
- [ ] PR-time 4-reviewer wave-gate (deferred per slice-4 + slice-5 + slice-6 convention).
