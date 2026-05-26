# Wave Decisions — comment-tombstone-gc (Slice 7)

DISTILL-wave decisions that gate DELIVER. Authored 2026-05-26 in
**propose mode** for the 8 DISTILL open questions surfaced in
`docs/feature/comment-tombstone-gc/design/wave-decisions.md` lines
314-395. Each recommendation is flagged below; user picks override
without re-running the DISTILL plan.

Slice 7 inherits slice-1/2/3/4/5/6 patterns verbatim per the project's
Architecture of Reference at
`docs/architecture/atdd-infrastructure-policy.md` (`--policy=inherit`,
default). Project pattern: legacy per-wave layout
(`docs/feature/<slice>/distill/`), no `docs/product/`, no
`feature-delta.md`, no SSOT migration gate — same as slices 1-6.

## Strategy: C (all real adapters) — inherited

Slice 7 inherits Strategy C from slices 1-6. **No new policy rows
needed** — DESIGN § Reuse Analysis (architecture.md table at
wave-decisions.md lines 195-203) records ZERO new ports. Every adapter
the slice-7 scenarios exercise is already in the policy file:

- HTTP via subprocess + `reqwest::Client` against `spawn_app` — inherited (`Driving` row 1).
- `assert_cmd::Command::cargo_bin("foundry")` for the doctor CLI — inherited (`Driving` row 6, the slice-3 `backup-verify` row). The slice-7 `restore-comment` subcommand uses the IDENTICAL mechanism (same `assert_cmd` invocation shape, same `foundry doctor <action> <arg>` parse, same exit-code-as-contract pattern). **NO new policy row required** — verified by structural identity with the slice-3 entry.
- Postgres per-scenario schema rotation — inherited (`Driven internal` row 1).
- Postgres advisory lock (`pg_try_advisory_lock`) — inherited shape from `MIGRATION_LOCK_ID` (`Driven internal` row 4); the slice-7 `TOMBSTONE_GC_LOCK_ID` is a new literal but the MECHANISM is identical.
- Metrics scrape via `support/metrics_scrape.rs` — inherited from slice 6; the 2 new metric families (`comments_tombstones_purged_total` + `comments_tombstones_pending`) are emitted via the same `metrics::counter!` / `metrics::gauge!` facades the parser already handles generically.
- Direct SQL test-fixture inserts for the time-warp pattern (`deleted_at = now() - interval 'N days'`) — uses the inherited slice-1 per-scenario `PgPool`; no new adapter.

**Confirmation**: `docs/architecture/atdd-infrastructure-policy.md`
is UNCHANGED in this DISTILL pass.

## Tier composition: Tier A only — Mandate 10 condition not met

9 automated scenarios, **none chained** (each spawns a fresh foundry
subprocess + per-scenario PG schema; preconditions re-established per
scenario rather than chained from the prior scenario's state). Input
space is config-shaped (sweep cadence, per-run cap, deletion age,
UUID arguments) — not domain-rich. Mandate 10's ≥3-chained +
domain-rich threshold is not crossed. **Tier B is NOT emitted.**

The 6 GC scenarios + 3 CLI scenarios share the same step vocabulary
within a tier but are otherwise independent narratives — none reuses
the When of a prior scenario as its Given. The chained-narrative
Pillar 2 applies inside each scenario (Background + Givens compose
into the When), not across scenarios.

## PBT input mode: example-only — Mandate 9 layer constraint

All 9 scenarios run at layer 3+ (real subprocess + real Postgres +
real testcontainers). Per Mandate 9, layer 3+ tests are example-only.
No proptest. Sad paths (gc-failure, admin-cli not-restorable,
admin-cli invalid-UUID) are named examples per Mandate 11.

DELIVER's PBT phase (per ADR-025 D2 + slice-5/6 precedent) covers the
property-shaped invariants at unit layer:

- `Store::gc_tombstoned_comments` SQL predicate: `forall rows where
  deleted_at < now() - threshold, deletion matches; forall rows where
  deleted_at >= now() - threshold or deleted_at IS NULL, row remains`.
- Batch-loop termination: `forall N tombstones, GC removes
  min(N, cap)` per tick.
- `Store::undelete_comment` UPDATE return: `affected_rows ∈ {0, 1}`;
  idempotent on a NULL-deleted_at row.
- CLI UUID parser: `forall malformed inputs, exit code = 2`.

## ADR-style decision table (D1-D8 propose-mode picks)

### D1 — Polling-interval test approach (Q1 from DESIGN open questions)

| Option | Status | Rationale |
|---|---|---|
| **A. Env-var override (`FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS=1`) + wait ~1.5-2s for first tick** | **CHOSEN (RECOMMENDED)** | Matches slice-6 precedent (`METRICS_POOL_POLL_SECONDS=5` is overridable via env in the subprocess spawn). The first-tick-soon invariant (DESIGN constraint 4) means `MissedTickBehavior::Skip` fires within ~5s of interval construction; with cadence=1s the first tick fires within ~1s; the scenarios wait 2s for safety margin. Wall-clock cost per scenario: ~2-3s. No production-code change required to expose the env var (DESIGN architecture.md § Background task additions already commits to env-var-driven configuration). |
| B. Extend `FakeClock` to intercept `tokio::time::interval` | DEFERRED | More engineering (the slice-5 `FakeClock` intercepts `sleep`, not `interval`); single-scenario benefit doesn't earn its keep. Revisit if a second test surface (slice-?? expired-sessions GC, expired-bootstrap-tokens GC) wants the same pattern — then the extension amortises across multiple scenarios. |

**Slice-7 outcome of D1 = A**: 6 GC scenarios use `FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS=1`
in the subprocess env; the "operator's foundry instance has been
running for at least N seconds" step (reused from slice-6's
phrase inventory) acts as the cadence-wait gate.

### D2 — Quarterly `@manual` admin-undelete drill (Q2 from DESIGN open questions)

| Option | Status | Rationale |
|---|---|---|
| **A. Add a one-paragraph drill recommendation to `RELEASING.md` § "Recovering an accidentally-deleted comment" (no acceptance scenario)** | **CHOSEN (RECOMMENDED)** | Operator confidence benefits from "did the runbook actually work the last time we tried it?" without forcing automated machinery for a once-per-quarter check. The paragraph is ~5 lines, ships with the runbook DELIVER writes anyway, and adds zero acceptance/test surface. |
| B. Rely solely on the @admin-cli acceptance scenarios | DEFERRED | The 3 CLI scenarios cover the happy path + 2 error paths, which IS the contract; the drill is an OPERATIONAL prophylactic, not a contract. Skipping the drill paragraph is defensible if the team wants tighter docs scope. |
| C. Add an `@manual` cucumber scenario documenting the drill | DEFERRED | Same anti-pattern as slice-6 #10 (`@manual` cucumber for criterion microbench) but with even less value — there's nothing for cucumber-rs to assert at all. The runbook paragraph IS the right surface. |

**Slice-7 outcome of D2 = A**: DELIVER adds the drill paragraph to the
runbook section it writes per architecture.md § "RELEASING.md runbook
addition". DISTILL writes ZERO new acceptance scenarios for it.

### D3 — Suite-time impact + `@slow` tag for cap scenario (Q3 from DESIGN open questions)

| Option | Status | Rationale |
|---|---|---|
| **A. Tag the cap scenario `@slow` and gate behind that tag in CI (run on full CI but exclude from the fast loop)** | **CHOSEN (RECOMMENDED)** | The 11k-row insert + 2 GC ticks is the ONE expensive scenario in the slice (~5-10s wall-clock; bigger than the slice-6 6s connection-hold). Pre-emptively gating it follows the slice-6 D7 precedent ("accept-and-re-baseline" was chosen for slice 6, but only because slice-6 had no single scenario big enough to gate cheaply — slice 7 does). Cost to introduce: one new tag in `tests/acceptance.rs` default-exclude list. |
| B. Defer the tag decision until DELIVER measures the actual wall-clock | DEFERRED | The wall-clock estimate (5-10s) is large enough that the gating decision is unlikely to change at DELIVER time. Pre-emptive tagging is the lower-risk default. |
| C. Refactor the scenario to use a smaller cap | DEFERRED | The whole point of the scenario is to exercise the per-run cap at its production-default value (10000). Reducing the cap weakens the contract. |

**Slice-7 outcome of D3 = A**: scenario 3 (cap) carries `@slow`. The
`@slow` tag is NEW for the project; `tests/acceptance.rs` default
closure-level filter will exclude it (one-line addition to DELIVER
checklist). To run the cap scenario locally:
`FOUNDRY_ACCEPTANCE_TAGS=all` or
`FOUNDRY_ACCEPTANCE_TAGS=@slow cargo test …`. CI runs it via
`FOUNDRY_ACCEPTANCE_TAGS=all` in a dedicated job.

### D4 — Time-warp fixture mechanism (Q4 from DESIGN open questions)

| Option | Status | Rationale |
|---|---|---|
| **A. Direct SQL insert into `comments` with `deleted_at = now() - interval 'N days'`, bypassing the production soft-delete handler** | **CHOSEN (RECOMMENDED)** | The production handler always sets `deleted_at = now()`, useless for testing the 90-day threshold. Direct SQL into the per-scenario schema (slice-1 pattern) is the only honest fixture. NOT fixture theater per principle (the GC IS the production code under test; the fixture is the test-only seed of the GC's input universe). Helper module `support/tombstone_factory.rs` (~50 LOC) keeps the helper out of the step file. |
| B. Add a production debug-only handler `POST /debug/age-tombstone` | REJECTED | Pollutes production code with test-only surfaces (anti-pattern slice-3 explicitly rejected — see slice-3 `wave-decisions.md` re: `mark_db_unreachable` cfg-gating). |
| C. Mock the clock the production handler reads from | REJECTED | The production handler reads `now()` at the SQL level (`UPDATE comments SET deleted_at = now() ...`); mocking would require intercepting Postgres's clock, which is impossible without a wholly different test architecture. |

**Slice-7 outcome of D4 = A**: NEW test-infrastructure module
`crates/foundry-acceptance/src/support/tombstone_factory.rs` (~50
LOC) exposes:

```rust
pub async fn insert_tombstoned_comment(
    pool: &PgPool,
    issue_id: Uuid,
    author_id: Uuid,
    body: &str,
    deletion_age_days: i64,
) -> Uuid;

pub async fn bulk_insert_tombstoned_comments(
    pool: &PgPool,
    issue_id: Uuid,
    author_id: Uuid,
    count: u64,
    deletion_age_days: i64,
) -> Vec<Uuid>;
```

The bulk variant exists for the cap scenario (11k rows). DELIVER fills
in both bodies; DISTILL ships them as RED scaffolds + the module
registration. Slice-precedent for support helpers: slice-2's
`sse_client.rs`, slice-3's `pg_backup.rs`, slice-6's
`metrics_scrape.rs`. The slice-7 helper is smaller than any of these.

### D5 — Failure-injection mechanism for D7 test (Q5 from DESIGN open questions)

| Option | Status | Rationale |
|---|---|---|
| **A. Extend the slice-3 `AppState::mark_db_unreachable` health-injection flag (cfg-gated `cfg(any(test, feature = "test-hooks"))`) to also cause `Store::gc_tombstoned_comments` to return a synthetic `StoreError::Sqlx(...)` once + auto-clear** | **CHOSEN (RECOMMENDED)** | Reuses the slice-3 seam that already opened the test-only-hook door at the right architectural layer (`AppState` flag, NOT `Store` flag — the GC task reads the flag during its tick and synthesizes the error). Cleanest mechanism per the option's framing. DELIVER scope: ~10 LOC in `AppState` + ~3 LOC at the GC tick site. |
| B. Inject a wrapping Store with synthetic-error behaviour via a constructor arg | REJECTED | Heavier; touches the Store API surface for one test. The cfg-gated flag is the existing convention. |
| C. Kill the testcontainers Postgres container mid-task | REJECTED | Slice-3 explicitly rejected this (see `docs/architecture/atdd-infrastructure-policy.md` row 8: "we cannot reliably kill the testcontainers Postgres mid-scenario without affecting sibling scenarios"). Per slice-3 precedent. |

**Slice-7 outcome of D5 = A**: DELIVER extends `AppState` with a
test-only `mark_next_gc_tick_fail()` method (parallel to
`mark_db_unreachable`); the acceptance step "the next tombstone sweep
tick will fail with a synthetic database error" calls it via the
existing in-process harness mechanism (BEFORE the subprocess spawn —
the subprocess inherits the flag via env var injected by the helper).
Actually — clarify: the subprocess is a SEPARATE process; the
in-process `AppState` flag is not shared. DELIVER ALTERNATIVE that
respects the subprocess boundary: inject via env var
`FOUNDRY_TEST_HOOK_GC_FAIL_NEXT=1`, parsed at GC-tick time inside the
subprocess. Same `cfg(any(test, feature = "test-hooks"))` gating;
slightly different mechanism than the in-process flag. DELIVER picks
the exact wiring; DISTILL flags this for the DELIVER pre-flight
checklist.

**DISTILL FLAG**: the env-var-flag variant of D5 = A is the only
mechanism that works honestly across the subprocess boundary. Surfaced
for DELIVER awareness; DELIVER may converge to a fully different
mechanism if a cleaner option appears (e.g. an admin HTTP endpoint
gated behind `feature = "test-hooks"`).

### D6 — CLI exit codes for `restore-comment` (Q6 from DESIGN open questions)

| Option | Status | Rationale |
|---|---|---|
| **A. Consolidate "comment exists but is not tombstoned" and "comment does not exist" into the SAME exit code 4** | **CHOSEN (RECOMMENDED)** | Both are operationally identical ("the UPDATE matched zero rows"); distinguishing them would require a separate SELECT round-trip BEFORE the UPDATE, which doubles the DB round-trip cost AND introduces a race window where the comment gets soft-deleted between the SELECT and the UPDATE. Operators see "not restorable" + the SQL fallback recipe in `RELEASING.md` — the psql one-liner returns 0 rows in BOTH cases, so the operator's mental model is unified. |
| B. Differentiate with a separate exit 5 for "not tombstoned" | DEFERRED | Doubles the round-trip cost; adds operator mental load for no actionable difference (both cases require investigation of "where did this UUID come from?"); rejected on operator-ergonomics grounds. |

**Slice-7 outcome of D6 = A**: CLI exit codes = {0 = restored;
2 = invalid UUID; 3 = DB connect failure; 4 = not restorable (comment
not found OR not currently tombstoned)}. The doctor stdout message
distinguishes between "comment <uuid> not in database" and "comment
<uuid> is not currently tombstoned" diagnostically (so the operator
can see WHICH one happened in the log), but the EXIT CODE is the same.

### D7 — Walking-skeleton coverage for the GC task (Q7 from DESIGN open questions)

| Option | Status | Rationale |
|---|---|---|
| **A. ONE `@walking_skeleton` scenario for the full GC tick path (spawn → tick → DELETE → counter increment) + ONE `@walking_skeleton` scenario for the admin-undelete CLI path** | **CHOSEN (RECOMMENDED)** | Mirrors slice-6 DD-11 precedent exactly. Two structurally distinct end-to-end loops earn distinct WS scenarios: (i) "GC ran and removed ancient tombstones" — observable via the metric + the database state; (ii) "operator restored an accidentally-deleted comment" — observable via the CLI exit code + the issue page. Both are operator-facing demos. Demoting either loses one of the two slice-7 narratives. |
| B. Single WS for the GC tick, demote the CLI scenario to non-WS | DEFERRED | The admin-undelete CLI is a fully-formed operator affordance with its own walking-skeleton litmus test ("operator runs one command, comment comes back"); calling it merely a focused scenario understates its end-to-end nature. |
| C. No WS at all (GC is internal infrastructure, CLI is an operator tool) | REJECTED | Loses the demo-able stakeholder outcome for both narratives; violates Mandate 5 ("walking skeletons that demo user value"). |

**Slice-7 outcome of D7 = A**: scenarios 1 + 7 carry
`@walking_skeleton`. Both are tagged `@real-io`; scenario 7 also
carries `@driving_adapter` because the CLI subcommand is a NEW driving
adapter (per Mandate 6 / RCA-fix P1).

### D8 — `@nfr-*` tag set for slice-7 scenarios (Q8 from DESIGN open questions)

| Option | Status | Rationale |
|---|---|---|
| **A. Reuse `@nfr-obs-03` (slice-6 catalogue) for the 2 metric-emission scenarios (#1 and #6). NO new `@nfr-*` tag for GC timing — the 24h cadence is operational, not performance.** | **CHOSEN (RECOMMENDED)** | Matches D1 = A from slice 5 + slice 6 (NFR-tag reuse is the default; new tags need explicit operational justification). The 2 new metrics ARE the testable surface of NFR-OBS-03 (slice-1 `nfrs.md` line 60 enumerates `db_connections_in_use` + the deferred families that slice-6 + slice-7 ship; same NFR cell). The GC's timing contract is "the threshold is 90 days, with up to 24h slack" — that is NOT a performance NFR; it is an operational retention promise enforced by the SQL `WHERE` clause, asserted by scenario #2 (gc-threshold). |
| B. Add `@nfr-ops-01` for "GC operates at promised cadence" | DEFERRED | New NFR matrix row for a contract that the scenario already names structurally (the @gc-threshold + @gc-cap scenarios ARE the cadence + retention proofs); rejected per slice-5/6 D1 default. |
| C. Add `@nfr-perf-06` for the cap scenario's wall-clock | REJECTED | The cap is a SAFETY contract (operational misconfig protection), not a performance contract. The @slow tag (per D3 = A) communicates the wall-clock concern; no NFR. |

**Slice-7 outcome of D8 = A**: 3 scenarios carry `@nfr-obs-03` (the
WS GC tick #1; the gc-metrics scenario #6). The remaining 6 scenarios
carry no `@nfr-*` tag. No back-propagation to slice-1 `nfrs.md`
required (the catalogue entry for NFR-OBS-03 already covers the new
metric families implicitly via the deferred-list pattern slice-6 D0
established).

## Structural decisions (no user pick — locked by inheritance + brief)

| ID | Question | Pick | Captured in |
|---|---|---|---|
| DD-1 | Strategy (per port-class default) | C — all real adapters per policy file | `docs/architecture/atdd-infrastructure-policy.md` (inherited; no new rows) |
| DD-2 | Test invocation pattern | Subprocess via `assert_cmd::Command::cargo_bin("foundry")` (slice-3 + slice-6 precedent). The GC task only exists inside the foundry binary, so observing its effects requires spawning the real binary. | `crates/foundry-acceptance/src/steps/us_10_tombstone_gc.rs` (RED scaffold) |
| DD-3 | New step file name + lineage | NEW file `us_10_tombstone_gc.rs` — extends the US-10 (comments) lineage; sits next to slice-5 `us_10_comment_edit_delete.rs`. The slice-7 work IS the next chapter of US-10's lifecycle (post-soft-delete sweep + undo). | `crates/foundry-acceptance/src/steps/us_10_tombstone_gc.rs` + `lib.rs` registration |
| DD-4 | Scaffold-RED mechanism | Step bodies `panic!("Not yet implemented -- RED scaffold (DISTILL); DELIVER finishes this")`; production code NOT touched per task brief | step file body + `red-classification.md` |
| DD-5 | Force-link discipline | `tests/acceptance.rs` adds `use foundry_acceptance::steps::us_10_tombstone_gc as _us_10_gc;` next to the existing `_us_10_edit` import | `crates/foundry-acceptance/tests/acceptance.rs` |
| DD-6 | World additions | Six `Option` / `HashMap`-default fields appended under a new `// ---- US-10 tombstone GC (slice 7) ----` block; all defaulted so existing scenarios unaffected | `crates/foundry-acceptance/src/world.rs` (bottom) |
| DD-7 | New test-infrastructure file | `crates/foundry-acceptance/src/support/tombstone_factory.rs` (~50 LOC) per D4 = A. Wraps direct SQL insertion of tombstoned comments at controlled `deleted_at` ages. Mirrors slice-2/3/6 helper conventions. | `crates/foundry-acceptance/src/support/tombstone_factory.rs` + `support/mod.rs` reg |
| DD-8 | New step file vs extending an existing one | NEW file (per DD-3). Slice-5's `us_10_comment_edit_delete.rs` is left intact; no slice-6/3 phrases collide. | confirmed by phrase inventory in `step-skeletons.md` |
| DD-9 | New dep needed for any slice-7 work | NONE — `assert_cmd` already in workspace deps (slice-3 inheritance); `tokio` + `sqlx` + `reqwest` + `uuid` all inherited; `support::metrics_scrape` reused from slice 6. | confirmed in `crates/foundry-acceptance/Cargo.toml` |
| DD-10 | Scope reconciliation (DISCUSS vs DESIGN vs DEVOPS) | Zero contradictions — see § "Reconciliation HARD GATE" below | this file |
| DD-11 | Reviewer dispatch deferred to PR time | Per slice-4 wave-decisions.md line 209 + slice-5 DD-7 + slice-6 DD-10 — no in-DISTILL reviewer parallel-dispatch | this file § "Final Wave Review Gate" |
| DD-12 | `@slow` tag for the cap scenario (per D3 = A) | NEW project-wide tag; `tests/acceptance.rs` default-exclude list grows by one entry in DELIVER | DELIVER pre-flight checklist entry |

## Reconciliation HARD GATE

Per nw-distill § "Wave-Decision Reconciliation HARD GATE". Files read:

- `docs/feature/foundry-backend-mvp/discuss/stories.md` § US-10 + § US-03 (the latter for the operator-runbook + CLI surface precedent). Slice 7 inherits US-10 from the v0.1 baseline; ADR-007's v0.2 commitment (slice 5 wave-decisions.md D5 = B) IS the inherited DISCUSS-side commitment.
- `docs/feature/foundry-backend-mvp/discuss/nfrs.md` — NFR-OBS-03 enumerates metric families; the 2 slice-7 metrics fall under that cell per D8 = A.
- `docs/feature/comment-tombstone-gc/design/wave-decisions.md` — D1-D7 + 9 constraints + 8 DISTILL open questions + 7 ACCEPTED invented-detail items.
- `docs/feature/comment-tombstone-gc/design/architecture.md` — slice-specific design + L3 sequence diagram.
- `docs/feature/comment-tombstone-gc/design/adrs/ADR-015..017.md` — all three locked decisions.
- `docs/feature/comment-edit-delete/distill/wave-decisions.md` — slice-5 D5 deferral that slice 7 closes.
- `docs/feature/comment-edit-delete/distill/coverage-matrix.md` § @soft-delete-invariant — slice-5 contract slice 7 honors.
- `docs/feature/handler-instrumentation/distill/wave-decisions.md` D0 — slice-6 5-deferred-metrics list that slice 7 takes 2 from.
- No `docs/feature/comment-tombstone-gc/devops/` directory — slice 7 has no infra changes (per architecture.md § External Integration check = NONE). WARN + proceed per nw-distill § Graceful Degradation matrix.

**Specifically checked for contradictions**:

1. **ADR-007 (slice-5) committed to v0.2 GC with no new migration** vs **slice-7 architecture.md commits to no new migration** — IDENTICAL. Verified via `ls /Users/jeffbailey/Projects/foss/leading/foundry/crates/foundry-store/migrations/` — directory shows `0001..0006`, no `0007`. Confirmed.

2. **Slice-5 wave-decisions.md D5 = B deferred admin-undelete to v0.2** vs **slice-7 ships CLI + psql per D5 = C** — IDENTICAL intent (the slice-5 deferral pointed to the slice-7 work); not a contradiction.

3. **Slice-6 D0 deferred-metrics list (5 deferred, including `bootstrap_tokens_unclaimed`)** vs **slice-7 ships 2 GC metrics that grow the slice-6 list** — NOT A CONTRADICTION. The slice-6 D0 list was a snapshot of v0.2 state, not a contract. The catalog goes from 5 deferred → 3 deferred + 2 shipped; slice-6 D4 explicitly framed the deferred-list as forward-looking. The v0.3 instrumentation slice absorbs the remaining 3 naturally.

4. **ADR-017 supersedes slice-5's "v0.2-candidate VIEW" status** — EXPLICIT SUPERSESSION (ADR-017 § Decision names the supersession). Not a contradiction; a clean handoff to a future v0.3 slice (`comment-read-defensive-engineering`).

5. **NFR-OBS-03's catalogue claim that `db_connections_in_use` has no labels** vs **slice-7's 2 new metrics with no labels** — CONSISTENT (both are bounded at 1 series per metric, honoring slice-6 D2 cardinality invariant). Not a contradiction.

**Reconciliation result: PASSED — 0 contradictions** across DISCUSS / DESIGN.

## Scenarios per file table

| File | Scenarios | Of which @walking_skeleton | Of which @error | Of which @slow |
|---|---|---|---|---|
| `features/comment-tombstone-gc.feature` (slice 7, NEW) | 9 | 2 (#1 GC tick + #7 admin-undelete CLI) | 2 (#8 not-restorable + #9 invalid-UUID) | 1 (#3 cap — per D3 = A) |

Total acceptance surface after slice 7: pre-existing ~65 (slice 1+2+3+4+5+6) + 9 = ~74 scenarios across the project.

Scenario count of 9 fits inside the 7-9 prompt ceiling. **No merging proposed** — each scenario covers a distinct substrate-lie probe (per architecture.md § Earned Trust):

- #1 — walking skeleton (the full GC loop)
- #2 — date arithmetic lie (90-day boundary)
- #3 — batch cap lie (per-run cap @ 10k)
- #4 — advisory-lock lie (two-replica race)
- #5 — transient failure lie (D7 = A precedent)
- #6 — metrics-emission contract (per D4 = A)
- #7 — walking skeleton (the admin-undelete CLI)
- #8 — CLI not-restorable error path
- #9 — CLI invalid-UUID error path

Error-path ratio for slice 7: 2 of 9 = **22%** — below the 40% nw-distill target. **Justification** (same posture as slices 5 and 6):

- The GC error surface is intrinsically thin (the GC has ONE input — the SQL query — and ONE output — rows deleted; the "error" cases are exhaustively: transient DB error covered by #5, advisory-lock contention covered by #4, misconfigured threshold partially covered by #3 via the cap safety net).
- The CLI error surface is GENERATIVELY enumerated by exit code: 0 / 2 / 3 / 4 — scenarios cover 0 (WS #7), 2 (#9), 4 (#8). Exit 3 (DB connect failure) is NOT covered by an acceptance scenario because the failure mode is "the env var DATABASE_URL is unreachable", which the subprocess harness would surface as a spawn failure, not an in-test assertion. DELIVER's PBT phase covers exit 3 via a unit test on the connection-establishment failure path.
- Adding bogus error scenarios to hit 40% would lower signal quality (slice-5 + slice-6 used this same justification).

## Tag conventions added

Inherited from slice 1/2/3/4/5/6 (unchanged):
`@walking_skeleton`, `@real-io`, `@driving_adapter`, `@error`,
`@nfr-obs-03`, `@us-NN`, `@manual`, `@docker-compose`,
`@slice1`..`@slice6`.

Added in slice 7 (deltas only):

- `@slice7` — every scenario in the new feature file.
- `@comment-tombstone-gc` — feature-level (mirrors slice-2's `@realtime`, slice-5's `@comment-edit-delete`, slice-6's `@handler-instrumentation`).
- `@gc-tick` — sub-area: the WS scenario.
- `@gc-threshold` — sub-area: date-arithmetic probe.
- `@gc-cap` — sub-area: batch-cap probe.
- `@gc-lock` — sub-area: advisory-lock probe.
- `@gc-failure` — sub-area: D7 = A failure-survives probe.
- `@gc-metrics` — sub-area: the pending-gauge contract.
- `@admin-cli` — sub-area: the 3 `foundry doctor restore-comment` scenarios.
- `@slow` — **NEW PROJECT-WIDE TAG** per D3 = A — gates the 11k-row cap scenario behind explicit selection. Excluded by default in `tests/acceptance.rs` (DELIVER adds the entry to the default-filter closure; one-line edit alongside the existing `@manual` / `@manual-trigger` / `@docker-compose` exclusions).

`@nfr-obs-03` reused per D8 = A (2 scenarios in slice 7: #1 + #6).
No new `@nfr-*` tag in slice 7.

## CI invocation

Matching slice-2/3/4/5/6 style:

```bash
# Full default suite (slices 1+2+3+4+5+6+7, excluding @manual + @manual-trigger + @docker-compose + @slow)
cargo test -p foundry-acceptance --test acceptance

# Slice-7 only (DELIVER iteration; INCLUDES @slow)
FOUNDRY_ACCEPTANCE_TAGS=@slice7 cargo test -p foundry-acceptance --test acceptance

# Slice 7 default (DELIVER fast-loop; EXCLUDES @slow)
FOUNDRY_ACCEPTANCE_TAGS="@slice7 and not @slow" cargo test -p foundry-acceptance --test acceptance

# Narrow band by sub-area
FOUNDRY_ACCEPTANCE_TAGS=@gc-threshold cargo test -p foundry-acceptance --test acceptance
FOUNDRY_ACCEPTANCE_TAGS=@gc-lock      cargo test -p foundry-acceptance --test acceptance
FOUNDRY_ACCEPTANCE_TAGS=@admin-cli    cargo test -p foundry-acceptance --test acceptance

# CI full-fat (includes @slow + @docker-compose + @manual-trigger; excludes @manual only)
FOUNDRY_ACCEPTANCE_TAGS=all cargo test -p foundry-acceptance --test acceptance
```

Concurrency cap stays at `--max-concurrent-scenarios 6` (inherited
from slice 3). Slice-7 scenarios spawn one foundry subprocess each
(slice-6 pattern; ~50-80MB resident per subprocess × 6 concurrent =
~300-500MB peak — within dev-laptop budgets).

## Suite-time budget

| Scenario | Cost (estimated) | Notes |
|---|---|---|
| 1 walking skeleton: GC tick removes 3 ancient tombstones + counter increment | ~4.0 s | subprocess (~2s) + 3-row insert (~50ms) + 2s wait for tick + scrape + assertions |
| 2 gc-threshold: 6 rows straddling the 90-day boundary | ~4.5 s | subprocess + 6-row insert + 2s wait + database row-count assertion |
| 3 gc-cap: 11k rows, two ticks (`@slow`) | ~10-15 s | subprocess (~2s) + 11k-row bulk insert (~3-5s) + 2 GC ticks (2 × ~1-2s) + 2 scrapes + assertions. Largest single scenario in slice 7. |
| 4 gc-lock: two-replica race, lock holding | ~5.0 s | subprocess + advisory-lock acquisition by a sibling pool + 3-row insert + 2s wait + state check + lock release + 2s wait + state check |
| 5 gc-failure: synthetic error mid-tick | ~5.0 s | subprocess + 3-row insert + flag-set + 2s wait + alive-check + flag-clear + 2s wait + state check |
| 6 gc-metrics: pending gauge across multiple ticks | ~6.0 s | subprocess (cap=2) + 5-row insert + 2s + scrape + 2s + scrape + 2s + scrape |
| 7 walking skeleton: admin-undelete CLI happy path | ~4.0 s | subprocess (~2s) + 1-row tombstone insert + CLI subprocess (~0.5s) + database state check |
| 8 admin-cli not-restorable | ~3.0 s | subprocess + CLI subprocess + exit-code check |
| 9 admin-cli invalid-UUID | ~3.0 s | subprocess + CLI subprocess + exit-code check |
| **Slice-7 default subtotal (excluding `@slow` #3)** | **~34.5 s** | within the 40s slice-budget addition |
| **Slice-7 full subtotal (including `@slow` #3)** | **~44.5-49.5 s** | invoked with `FOUNDRY_ACCEPTANCE_TAGS=all` or `@slice7` explicit selection |
| Slice 1+2+3+4+5+6 baseline (fast-loop projection from slice-6 wave-decisions.md) | ~70 s | per slice-6 § Fast-loop budget drift |
| **Slice 1+2+3+4+5+6+7 default fast-loop projection** | **~105 s** | exceeds slice-6 re-baselined ~70s by ~35s |

### Fast-loop budget drift — ACKNOWLEDGED (mirrors slice-6 pattern)

The fast-loop iteration pattern (strip `@docker-compose` + `@manual` +
the slice-3 caddy scenario; now ALSO strip `@slow`) projects to ~70s
baseline + ~34.5s slice-7 default = **~105s total fast-loop**. This
exceeds the slice-6 re-baselined ~70s by ~35s.

| Mitigation option | Status | Cost / consequence |
|---|---|---|
| (a) Shard CI matrix into two parallel jobs (one per `@slice` band) | **STILL AVAILABLE** per slice-6 wave-decisions.md DEVOPS plan | CI YAML edit (~20 LOC); cuts wall-clock to ~50s per shard; doubles CI minutes |
| (b) Accept-and-re-baseline the top-line at ~105s (or ~120s with headroom) | **RECOMMENDED for v0.2 RC** | Zero churn; document new baseline in slice-1 wave-decisions.md back-propagation when v0.2 ships |
| (c) Tag scenarios 4 + 5 + 6 as `@slow` (each spends ~2-5s on cadence waits) | DEFERRED to slice-8+ if fast-loop hits ~120s | Halves slice-7's default fast-loop contribution; loses default visibility into the lock/failure/metrics contracts |

**Recommendation for slice 7**: option (b) — accept-and-re-baseline to
~105s. Revisit option (a) sharding if the fast loop hits ~120s in
slice 8+. Option (c) is the bail-out if PR review disputes the value
of the gc-lock / gc-failure / gc-metrics scenarios — not the default.

The slice-7 contribution is dominated by subprocess spawn overhead
(~2s × 9 scenarios = ~18s — same per-scenario cost as slice-6) plus
the cadence waits (~2s × 6 GC scenarios = ~12s). Per-scenario cost
math is consistent with slice-6 measurements.

## Open Decisions for DELIVER

| Decision | DISTILL status | DELIVER inheritance |
|---|---|---|
| `TOMBSTONE_GC_LOCK_ID` literal exact value (proposed `0x_60_C0_DE_60_C0_DE_60_u64 as i64`) | DESIGN flagged as ACCEPTED invented-detail #1; DISTILL scenario tests do NOT assert the literal value, only the BEHAVIOUR (lock acquired → work done; lock contended → no work) | DELIVER picks; if literal differs, scenarios stay green |
| The `mark_db_unreachable`-style env-var flag name for D5 = A failure injection | DISTILL flagged as DESIGN-side wiring detail; the scenario step phrase ("the next tombstone sweep tick will fail with a synthetic database error") is contract-stable; DELIVER picks the env-var name + cfg-gating | DELIVER picks; scenarios stay green so long as the step body wires through |
| Exact error message string for exit-code 4 ("not restorable" vs "comment not found" vs "comment not tombstoned") | DISTILL asserts `stderr mentions "not restorable"` (substring match); DELIVER picks the literal copy. Suggested wording in `RELEASING.md` is "status: not restorable" but the exact string is not pinned | DELIVER picks; substring-match keeps scenarios green |
| Exact error message string for exit-code 2 ("invalid UUID") | DISTILL asserts `stderr mentions "invalid UUID"` (substring match); DELIVER picks the literal copy | DELIVER picks; substring-match keeps scenarios green |
| Exit code 3 (DB connect failure) acceptance coverage | DISTILL DOES NOT cover via cucumber (failure mode is subprocess-level; cucumber can't easily inject a bad DATABASE_URL without the subprocess exiting before the harness can observe). DELIVER PBT covers via unit test on the connection-establishment failure path. | DELIVER adds the PBT unit test per ADR-016 § Verification line 6 ("invokes the CLI with an unreachable `DATABASE_URL`, asserts exit 3") in DELIVER's unit-test phase instead of as cucumber |
| Whether `Store::probe()` needs extension for the new GC method existence | DESIGN architecture.md § Earned Trust says NO — "the existing `Store::probe()` validates Postgres reachability + migration version + LISTEN/NOTIFY round-trip + slice-5 migration-0006 columns; the 3 new methods ride the already-probed adapter" | DELIVER inherits; no probe extension required |
| `bulk_insert_tombstoned_comments` implementation strategy (single COPY vs batched INSERT) | DISTILL ships the function SIGNATURE in `support/tombstone_factory.rs`; the implementation is DELIVER's. 11k rows is small enough that batched INSERT (~10ms) is fine; COPY is overkill | DELIVER picks; scenarios stay green either way |

## DELIVER Pre-flight Checklist

Categorized by the 5 sub-deliverables DESIGN architecture.md surfaces.

### Sub-deliverable A — Tombstone GC background task in `crates/foundry-app/src/main.rs`

- [ ] `tokio::spawn` block added next to the slice-6 pool-poll task (main.rs lines 160-183)
- [ ] `tokio::time::interval(Duration::from_secs(gc_interval_seconds))` with `MissedTickBehavior::Skip`
- [ ] `gc_interval_seconds` parsed from `FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS`, default `DEFAULT_TOMBSTONE_GC_INTERVAL_SECONDS = 86400` per architecture.md
- [ ] Per-tick body: call `store.gc_tombstoned_comments(...)`, log result, emit metrics; on error log + continue (D7 = A per ADR-015)
- [ ] `FOUNDRY_TEST_HOOK_GC_FAIL_NEXT` env-var flag honored per D5 = A (DISTILL flag: cfg-gated `feature = "test-hooks"`; once-and-clear semantics)
- [ ] **Acceptance criterion**: scenarios 1, 2, 5 GREEN

### Sub-deliverable B — `Store::gc_tombstoned_comments` + `Store::count_pending_tombstones` in `crates/foundry-store/src/lib.rs`

- [ ] `pub const TOMBSTONE_GC_LOCK_ID: i64 = 0x_60_C0_DE_60_C0_DE_60_u64 as i64;` (per ACCEPTED invented-detail #1)
- [ ] `Store::gc_tombstoned_comments(older_than: Duration, batch: usize, cap: usize) -> Result<u64, StoreError>` — uses `pg_try_advisory_lock` (non-blocking); loops DELETE-with-LIMIT-subquery until rows_affected < batch OR cumulative >= cap; releases lock in ALL paths (incl. error)
- [ ] `Store::count_pending_tombstones(older_than: Duration) -> Result<u64, StoreError>` — single `SELECT count(*)`
- [ ] Slice-1 `sqlx prepare` cache updated (3 new queries: gc DELETE subquery, lock acquire, count SELECT)
- [ ] **Acceptance criterion**: scenarios 1, 2, 3, 4 GREEN

### Sub-deliverable C — Metric emission in main.rs GC tick

- [ ] `metrics::counter!("comments_tombstones_purged_total").increment(deleted_count)` after each successful tick
- [ ] `metrics::gauge!("comments_tombstones_pending").set(pending_count as f64)` after each tick (after lock release; computed via `count_pending_tombstones`)
- [ ] Both metrics registered at value 0 at startup (slice-6 D4 precedent — avoid "no data" Grafana panel) BEFORE the GC interval task spawns
- [ ] Both metrics UNLABELLED (bounded cardinality, slice-6 D2 invariant — slice-6 unit test in `metrics_server.rs` covers them automatically)
- [ ] Cross-reference added to `docs/feature/foundry-backend-mvp/design/system/observability-infra.md` metric-naming table (2 new rows + slice-6 D0 deferred-list note)
- [ ] **Acceptance criterion**: scenarios 1, 6 GREEN (plus the gauge contract inside #3)

### Sub-deliverable D — `Store::undelete_comment` + `restore-comment` CLI subcommand

- [ ] `Store::undelete_comment(comment_id: Uuid) -> Result<u64, StoreError>` — single `UPDATE comments SET deleted_at=NULL, deleted_by=NULL WHERE id=$1 AND deleted_at IS NOT NULL`; returns rows affected (0 or 1); idempotent on a NULL-deleted_at row
- [ ] `crates/foundry-app/src/admin_cli.rs::run_restore_comment(comment_id: &str) -> i32` — parse UUID, connect to `DATABASE_URL`, call `Store::undelete_comment`, print `status: restored` or `status: not restorable`; exit codes per D6 = A
- [ ] Dispatch arm in `crates/foundry-app/src/main.rs::dispatch_subcommand` next to the `"backup-verify"` arm
- [ ] CLI reads `DATABASE_URL` (NOT `FOUNDRY_DOCTOR_PROBE_URL`; per architecture.md — restore operates on the live production DB)
- [ ] DELIVER PBT unit test for exit-code 3 (DB connect failure) per "Open Decisions for DELIVER" — covers the failure path acceptance can't easily inject
- [ ] **Acceptance criterion**: scenarios 7, 8, 9 GREEN

### Sub-deliverable E — RELEASING.md operator runbook addition

- [ ] New subsection "Recovering an accidentally-deleted comment" after the `foundry doctor backup-verify` section
- [ ] Path 1 (CLI) + Path 2 (psql) per architecture.md § "RELEASING.md runbook addition" lines 241-290
- [ ] One-paragraph quarterly drill recommendation per D2 = A
- [ ] **Acceptance criterion**: no automated test; reviewer reads the runbook in the PR

### Sub-deliverable F — Acceptance harness wiring

- [ ] `crates/foundry-acceptance/src/support/tombstone_factory.rs` filled in per DD-7 + DD-7 signatures
- [ ] `crates/foundry-acceptance/src/support/mod.rs` registers the new module
- [ ] `crates/foundry-acceptance/src/steps/us_10_tombstone_gc.rs` step bodies filled in (replacing the RED scaffolds)
- [ ] `crates/foundry-acceptance/src/world.rs` slice-7 fields used + reset between scenarios
- [ ] `tests/acceptance.rs` default-filter closure excludes `@slow` (one-line edit) per D3 = A
- [ ] **Acceptance criterion**: all 9 slice-7 scenarios GREEN end-to-end

### Cross-cutting regression

- [ ] All 9 automated slice-7 scenarios GREEN (default: 8; with @slow: 9)
- [ ] No regression in the existing ~65 scenarios across slice 1+2+3+4+5+6
- [ ] `cargo check -p foundry-acceptance --tests` passes
- [ ] `cargo deny check` passes (zero new deps per architecture.md § "Technology Stack")
- [ ] `cargo xtask check-arch` passes — no crate-boundary changes; `foundry-app` already depends on `metrics` (slice 6)
- [ ] Step-phrase contract: the new phrases (per `step-skeletons.md`) MUST NOT be renamed in GREEN. Awkward phrasings should be surfaced as DELIVER → DISTILL retro items.

## Final Wave Review Gate

Per slice-4 wave-decisions.md line 209 / slice-5 DD-7 / slice-6 DD-10
— the project pattern defers the 4-reviewer wave-gate to PR time
(legacy per-wave file layout, all slices 1-6 reviewer-approved under
this convention). No in-DISTILL parallel reviewer dispatch. The PR
will carry the DESIGN ADRs (ADR-015..017) + this DISTILL artifact set
+ DELIVER work for reviewers to inspect simultaneously.

## Decision-driven invented detail (slice 7 DISTILL deltas only)

DESIGN's "Decision-driven invented detail — ACCEPTED" list (the 7
items at design/wave-decisions.md lines 397-456) is INHERITED
UNCHANGED. DISTILL adds these phrasing flags + propose-mode picks:

1. **Two `@walking_skeleton` scenarios in one feature file (#1 + #7)** — per D7 = A. Slice-6 DD-11 set the precedent for this when two structurally distinct end-to-end loops both deserve WS status. **PROPOSED**. Alternative: demote #7 to `@admin-cli` only; not taken because the CLI is the operator-facing affordance the slice-3 backup-verify precedent legitimized as walking-skeleton-worthy.

2. **`@slow` is a NEW project-wide tag** per D3 = A. Excluded by default in `tests/acceptance.rs` (DELIVER one-line edit). First project use; precedent for future slow-but-valuable scenarios. **PROPOSED**.

3. **`support/tombstone_factory.rs` is NEW test-infrastructure** per D4 = A. ~50 LOC helper analogous to slice-3's `pg_backup.rs` (direct SQL fixture insertion). NOT taking a dependency on any new crate — vanilla `sqlx::query!` calls. **PROPOSED**.

4. **NEW step-body file `us_10_tombstone_gc.rs` continues the US-10 lineage** per DD-3 — slice-2 shipped POST + GET + sanitize (`us_10_comments.rs`); slice-5 shipped PATCH + DELETE + admin-delete + 410-Gone (`us_10_comment_edit_delete.rs`); slice-7 ships GC + admin-undelete (`us_10_tombstone_gc.rs`). The US-10 surface across the project is now 3 step files (consistent with slice-2/5 precedent of additive splits within a US-NN lineage). **PROPOSED**.

5. **D5 = A wiring detail — env-var flag instead of in-process flag** (the DISTILL FLAG inside D5 above). The subprocess boundary forces this; DESIGN's recommendation assumed in-process. **DISTILL-introduced clarification**, not an override; DELIVER picks the exact wiring shape.

6. **D6 = A diagnostic-message split** — the EXIT CODE collapses "not found" + "not tombstoned" into 4, but the STDOUT/STDERR distinguishes them diagnostically. **PROPOSED**. Operators see the literal reason in the log; the exit code IS the contract; the diagnostic is observability. Aligns with the slice-3 backup-verify pattern (exit code is the contract; stdout shape is the observability layer).

7. **Suite-time fast-loop budget drift to ~105s** — ACKNOWLEDGED, not a re-litigation. Slice 7 pushes the fast loop ~35s over slice-6's re-baselined ~70s. Recommendation per "Suite-time budget" table: accept-and-re-baseline (option b) for v0.2 RC; revisit (a) CI sharding if it hits ~120s in slice 8+. The `@slow` gating on #3 already pulls 10-15s off the fast-loop subtotal.

All seven DESIGN-side ACCEPTED invented-detail items (TOMBSTONE_GC_LOCK_ID literal; env-var family naming; first-tick-soon invariant; `restore-comment` CLI subcommand name; metric name prefix; no-backoff failure handling; 90-day default threshold) are unchanged by DISTILL — see "Open Decisions for DELIVER" above for the DELIVER-time touchpoints.

## Pointer to proposals.md

Per slice-5 / slice-6 precedent: no separate `proposals.md` for slice 7
this round. The 8 DISTILL open questions are answered inline in D1-D8
above with the SAME rationale density a `proposals.md` would carry.
DESIGN's `proposals.md` (the historical reasoning file) is the
authoritative source for WHY each option was framed; this file is the
authoritative source for WHICH option DISTILL picked.

If user wants a separate `proposals.md` for the DISTILL-side options
(slice-5 + slice-6 did not produce one either), one can be extracted
from D1-D8 above; otherwise this file IS the propose-mode artifact.
