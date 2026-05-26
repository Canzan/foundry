# Wave Decisions — comment-tombstone-gc (slice 7)

DESIGN-wave decisions for the slice that closes ADR-007's v0.2 GC
commitment + slice-5 D5's deferred admin-undelete runbook. **STATUS:
FINAL.** All seven open questions in `proposals.md` resolved with the
recommended pick; no overrides. Three ADRs created (ADR-015, ADR-016,
ADR-017) consolidating the seven decisions into coherent decision
clusters.

This document is the slice-7 handoff artifact for the DISTILL wave
alongside `architecture.md`.

## DDD Decisions (D1 – D7) — final

| ID  | Question | Decision | ADR |
|-----|----------|----------|-----|
| D1  | Polling interval / cadence | **A — Daily** (24h); env-tunable via `FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS` | ADR-015 |
| D2  | Batch size + safety cap | **B — Batches of 1000**, hard per-run cap of 10,000 (env-tunable via `FOUNDRY_TOMBSTONE_GC_MAX_PER_RUN`) | ADR-015 |
| D3  | Implementation home | **A — Inline** in `crates/foundry-app/src/main.rs` (promote to a `gc.rs` module when a SECOND cleanup task lands) | ADR-015 |
| D4  | Observability hook | **A — Emit now**: 2 new bounded-cardinality metrics (`comments_tombstones_purged_total` counter, `comments_tombstones_pending` gauge) | ADR-016 |
| D5  | Admin-undelete recipe scope | **C — Both**: `foundry doctor restore-comment` CLI (primary) + psql one-liner in `RELEASING.md` (fallback) | ADR-016 |
| D6  | `comments_visible` SQL VIEW | **A — DON'T ship** the VIEW this slice (separable defense-in-depth concern; v0.3 candidate as its own slice) | ADR-017 |
| D7  | Failure handling pattern | **A — Log warn + continue** to next tick; establishes precedent for future cleanup tasks | ADR-015 |

ADR consolidation rationale: D1+D2+D3+D7 form a coherent "scheduled
cleanup task" pattern (one ADR-015 captures the whole pattern, since
splitting them would scatter the same conceptual decision across four
files); D4+D5 form a coherent operator-visibility pair (metrics for
running state + CLI for one-off restoration); D6 stands alone as a
deferral decision.

### D1 — Daily cadence (CHOSEN: A)

**Rationale**: (a) the 90-day SLA tolerates slack measured in hours,
not days, so daily is comfortably inside the operator promise; (b) the
FIRST cleanup task should pick the simplest viable pattern —
over-engineering it locks future cleanup tasks into the wrong
baseline; (c) matches slice-6's 5s-pool-poll simplicity for scheduled
work.

**Why other options rejected**:
- B (hourly) — over-engineering for a 90-day threshold; 24× more
  ticks for ~0 user-observable benefit.
- C (manual-trigger only) — operators who don't read the docs ship a
  privacy regression; the "automatic by default" path is the safer
  default for an OSS tool with unknown operators.
- D (hybrid daily + CLI) — clean v0.3 evolution if operator feedback
  arrives, but premature for v0.2 release (the inner GC function will
  be reusable from a future CLI without redesign).

**Captured in**: ADR-015 § Decision.

### D2 — Batched 1000 with 10k cap (CHOSEN: B)

**Rationale**: (a) the safety cap is cheap insurance against the
unique failure mode of THIS kind of task — operational misconfig of
`deleted_at` is the textbook "GC hit the wrong threshold" disaster;
(b) batching of 1000 keeps lock-hold time bounded; (c) env-tunable cap
gives the operator a "go bigger during recovery" knob without
redeploying; (d) at expected steady-state load (~50 tombstones per
workspace per quarter), the cap is never reached — pure safety net.

**Why other options rejected**:
- A (delete-all-in-one-transaction) — no safety against misconfigured
  `deleted_at`; the whole table could evaporate in one transaction.
- C (batched, no cap) — removes the recovery safety net.
- D (single DELETE LIMIT N) — doesn't drain backlogs as elegantly as
  the batch-loop pattern.

**Captured in**: ADR-015 § Batching strategy.

### D3 — Inline in main.rs (CHOSEN: A)

**Rationale**: (a) the slice-6 D5 precedent ("hybrid: no new file
unless cohesion requires") applies — ONE background cleanup task
doesn't yet justify a `gc.rs`; (b) the slice-1 ADR-001 precedent
("smallest viable shape") applies; (c) promotion to a `gc.rs` module
is mechanical when the next cleanup task arrives — extract both tasks
into `gc.rs`, ship in the same v0.3 slice that adds the second one;
(d) the slice-6 pool-poll task is the existing precedent for
`tokio::spawn` + `tokio::time::interval` in main.rs.

**Why other options rejected**:
- B (new module `gc.rs`) — premature; promote when 2nd cleanup task
  lands.
- C (push into `foundry-store`) — concern-mixing; store crate becomes
  a runtime orchestrator, not just an adapter.

**Captured in**: ADR-015 § Hosting decision.

### D4 — Emit metrics now (CHOSEN: A)

**Rationale**: (a) the GC IS the consumer that slice-6 D0 said "no
consumer = no metric" was waiting for — the GC's correctness is what
these metrics observe; (b) the slice-6 patterns (poll-based gauge for
state, counter for events) transfer verbatim; (c) bounded cardinality
preserves the slice-6 D2 invariant; (d) deferring to a v0.3
5-metric slice creates a 6-month window where GC stalls are
invisible, which is a higher-impact regression than the catalog
coherence trade-off.

**Why other options rejected**:
- B (defer to broader instrumentation slice) — "GC silently failed
  for 6 months" is a worse outcome than "we shipped 2 metrics in v0.2
  instead of v0.3".
- C (logs only, structured for log-based alerting) — log-based
  absence-alerts are operationally fragile; less robust than a gauge
  that goes flat.

**Catalog impact**: slice-6 D0 deferred-list pinned 5 metrics
including `bootstrap_tokens_unclaimed`. Slice 7 ships 2 GC metrics
ahead of that slice; deferred-list goes from 5 deferred → 3 deferred
+ 2 shipped. No structural coupling; the v0.3 instrumentation slice
absorbs the rest naturally.

**Captured in**: ADR-016 § Observability decision.

### D5 — Both CLI + psql, CLI primary (CHOSEN: C)

**Rationale**: (a) the slice-3 `backup-verify` precedent argues
strongly for the CLI surface — operator-facing concerns get CLI
subcommands; (b) the psql one-liner is cheap insurance for the
"operator has DB access but no foundry binary on their bastion host"
scenario; (c) the doc-duplication concern is minor — the SQL recipe
serves as "what is this CLI doing?" documentation; (d) shipping the
CLI now means future audit-logging additions have a single chokepoint.

**Why other options rejected**:
- A (psql one-liner only) — leaves operator ergonomics on the table;
  the slice-3 precedent already opened the doctor CLI surface.
- B (CLI only) — denies the rare "binary unavailable" operator path;
  cost of adding the psql recipe is ~10 lines of doc.

**Captured in**: ADR-016 § Admin-undelete surface.

### D6 — Defer `comments_visible` VIEW (CHOSEN: A)

**Rationale**: (a) the VIEW is a defense-in-depth concern, separable
from the GC + undelete deferrals this slice closes; (b) shipping the
VIEW without consuming it creates schema surface area with no
immediate value — "I added a view nobody reads" is dead weight; (c)
shipping the VIEW AND migrating reads is a separate, larger slice
that should get its own DESIGN pass and acceptance coverage; (d) the
slice-5 behavioural invariant + acceptance scenario 9 has been
holding fine.

**Why other options rejected**:
- B (ship VIEW only, no read migration) — schema bloat with no value.
- C (ship VIEW + migrate all reads) — scope creep beyond this slice;
  belongs in a dedicated v0.3 slice "comment-read-defensive-engineering".

**Captured in**: ADR-017. Explicitly promotes the VIEW from "v0.2
candidate" (per slice-5 wave-decisions.md) to "v0.3 candidate as its
own slice that retrofits all read paths."

### D7 — Log + continue on failure (CHOSEN: A)

**Rationale**: (a) matches the slice-2 PgListener "log + auto-recover"
tradition (which DOES include backoff, but that's because PgListener
ticks at every notification, not at a daily cadence — slice-7 GC's
natural daily cadence already IS the backoff); (b) the D4 metric story
handles alerting structurally — flat `comments_tombstones_pending`
gauge over time triggers an operator alert without needing log-based
tooling; (c) the failure modes for a 90-day-window GC are inherently
non-urgent — a missed tick costs at most "deletion happens at day 91
instead of day 90".

**Why other options rejected**:
- B (log + exponential backoff) — backoff at daily cadence is
  awkward; "log every 24h, 48h, 96h" doesn't help operators.
- C (abort task on 3 consecutive failures) — "restart the pod to
  recover from a transient blip" is a heavy hammer.
- D (crash the process on first persistent error) — conflates "the
  GC is having a bad day" with "the substrate is fundamentally
  broken"; rejected at proposal time.

**Documented precedent for future cleanup tasks**: slice 7 establishes
"log warn + continue next tick + operator alerts via Prometheus gauge
flatness". When future cleanup tasks land (expired sessions GC,
expired bootstrap tokens GC), they inherit this pattern.

**Captured in**: ADR-015 § Failure handling (consolidated with the
scheduling decisions since it's part of the same "scheduled cleanup
task" pattern).

## Reuse Analysis — HARD GATE artifact

This table is the slice's hard gate. Every CREATE NEW is challenged;
every EXTEND is justified by reuse over reimplementation per
principle 5. Verbatim from `proposals.md` § 2, finalized for the
chosen picks (A/B/A/A/C/A/A).

| Action | Target | Why | LOC delta |
|---|---|---|---|
| EXTEND | `crates/foundry-store/src/lib.rs` § cleanup (new section) | Add `Store::gc_tombstoned_comments(older_than, batch, cap)` using new `TOMBSTONE_GC_LOCK_ID` advisory-lock constant (follows existing `MIGRATION_LOCK_ID` pattern lines 21 + 96-105). Also `Store::count_pending_tombstones(older_than)` for D4 gauge feeding. Also `Store::undelete_comment(uuid)` for D5 CLI dispatch. | +~95 |
| EXTEND | `crates/foundry-app/src/main.rs` | Spawn the GC background task next to the slice-6 pool-poll task (lines 160-183). `tokio::time::interval` at `FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS` default 86400. On each tick: call GC, log result, emit D4 metrics. Per D7 (ADR-015 § failure-handling) log + continue on error. | +~50 |
| EXTEND | `crates/foundry-app/src/main.rs` § `dispatch_subcommand` | Add `"restore-comment"` arm to the existing `"doctor"` dispatch. Calls into `admin_cli::run_restore_comment` (slice-3 backup-verify shape). | +~25 |
| EXTEND | `crates/foundry-app/src/admin_cli.rs` | Add `pub fn run_restore_comment(comment_id: &str) -> i32`. Parse UUID, connect to `DATABASE_URL`, call `Store::undelete_comment`, print result, return exit code. | +~70 |
| EXTEND | `RELEASING.md` | Add "Recovering an accidentally-deleted comment" subsection after the `foundry doctor backup-verify` section. Documents the CLI as primary path + psql one-liner as fallback. | +~50 lines docs |
| EXTEND | `docs/feature/foundry-backend-mvp/design/system/observability-infra.md` | Add 2 rows to the metric-naming table for `comments_tombstones_purged_total` + `comments_tombstones_pending`. Note coupling to slice-6 D0 deferred list (5 → 3 deferred + 2 shipped). | +~3 lines |
| EXTEND | `crates/foundry-store/migrations/` | NO new migration. ADR-007 § Decision and migration `0006`'s header both committed: slice-5 schema is a strict subset of GC needs. | 0 |
| CREATE NEW | none | All work fits in existing files. ADR-001 "no new crates" + slice-6 D5 "no new files unless cohesion requires" both hold. The GC task is single — a `gc.rs` module is justified only when a SECOND cleanup task arrives. | — |

**Total estimated delta**: ~240 LOC of Rust + ~50 lines of docs + 3
lines of cross-reference. Smaller than slice 5 (~340 LOC); comparable
to slice 6 (~190 LOC). Bundling the runbook + CLI into the same slice
as the GC task keeps shared infrastructure (`Store::undelete_comment`,
the `dispatch_subcommand` extension) on a single review surface.

## Architecture Summary

- **Pattern**: Layered with strict inward dependency, dependency-inversion
  at the crate boundary (inherited from slice-1 ADR-001).
- **Paradigm**: OOP-flavored Rust with plain async fns. New background
  task follows the slice-6 pool-poll task shape verbatim (tokio::spawn
  + tokio::time::interval with `MissedTickBehavior::Skip`).
- **Key components touched**:
  - `foundry-store` — gains 3 methods on the existing `Store` adapter
    + 1 new advisory-lock constant.
  - `foundry-app::main` — gains 1 background task spawn + 1
    subcommand-dispatch arm.
  - `foundry-app::admin_cli` — gains 1 function for the CLI subcommand.
  - `foundry-core` — unchanged (I/O-free invariant preserved).
  - `foundry-realtime` — unchanged.
  - `foundry-auth` — unchanged.
- **Communication**: GC task reads via advisory-lock-guarded SQL; emits
  to the existing `metrics_exporter_prometheus` recorder. CLI
  subcommand reads `DATABASE_URL` and dispatches into the store. No
  new wire protocols. No new external integrations. No HTTP-route
  changes.

## Technology Stack

**Zero new dependencies.** Slice 7 is a pure extension of existing
adapters:

- Rust 2021 / sqlx / tokio — unchanged.
- `metrics` (MIT/Apache-2.0) — already declared at workspace level per
  slice-6 commit; consumed by `foundry-app` already; this slice adds
  the GC task's emissions in the same crate.
- `tokio::time::interval` — core tokio capability; same primitive the
  slice-6 pool-poll task uses.
- `uuid` (slice-1 baseline) — for CLI argument parsing.

`cargo deny check` expected to pass without changes. AGPLv3-clean
dependency graph preserved.

## Constraints Established

These constraints are established by slice-7 decisions and become
invariants downstream waves and future slices must honor:

1. **Background cleanup task pattern (ADR-015)**: every fallible
   background cleanup task in `foundry-app` MUST log + continue on
   error (no exit, no backoff, no escalation). Alerting story is
   carried by Prometheus gauge flatness, not log-based absence
   alerts. Future cleanup tasks (expired sessions GC, expired
   bootstrap tokens GC, expired reset tokens GC, expired invites GC)
   follow this pattern.

2. **Advisory-lock naming pattern**: every long-running cleanup task
   gets its own canonical `i64` literal lock id constant next to
   `MIGRATION_LOCK_ID` + `TOMBSTONE_GC_LOCK_ID`. Operators observing
   `pg_locks` MUST be able to distinguish locks by literal value.
   Future cleanup tasks ADD constants; never share. The literal
   `TOMBSTONE_GC_LOCK_ID = 0x_60_C0_DE_60_C0_DE_60_u64 as i64` is
   reserved for this slice; future cleanup tasks pick new literals.

3. **Env-var naming pattern**: cleanup-task scheduling env vars
   carry the `FOUNDRY_` prefix and the `_INTERVAL_SECONDS` /
   `_MAX_PER_RUN` / `_OLDER_THAN_DAYS` suffix family. Examples:
   `FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS`,
   `FOUNDRY_TOMBSTONE_GC_MAX_PER_RUN`,
   `FOUNDRY_TOMBSTONE_GC_OLDER_THAN_DAYS`. Future cleanup tasks
   follow this family.

4. **First-tick-soon invariant**: cleanup tasks emit a first-tick
   shortly after process boot (matches slice-6 pool-poll task
   behaviour — `MissedTickBehavior::Skip` semantics fire the first
   tick within ~5s of interval construction). This guards against
   operators staring at Grafana for 24h before seeing the first GC
   tick.

5. **GC affects ONLY tombstoned rows older than threshold**: the SQL
   filter `WHERE deleted_at < now() - interval '90 days'` is binding.
   Future GCs MUST NOT delete rows where `deleted_at IS NULL` or
   `deleted_at` is recent. Enforced by acceptance scenarios 1 + 2
   (date-arithmetic probe + batch-cap probe per principle 12 § Earned
   Trust).

6. **Hard-delete fires no SSE event**: by the time a comment's
   tombstone is 90+ days old, no viewer cares. The GC's DELETE does
   NOT carry an outbox INSERT; no `CommentPurged` event_type is
   added. (Distinguished from slice-5's soft-delete which DOES fire
   `CommentDeleted` so viewers see the immediate tombstone effect.)

7. **Undelete is operator-only**: the schema affords undelete
   (slice-5 ADR-007); slice 7 ships the CLI + runbook surface. No
   UI-affordance to undelete is shipped or planned — the moderation
   reversal UX is intentionally a high-friction operator action.

8. **Bounded-cardinality invariant extends to slice-7 metrics**:
   `comments_tombstones_purged_total` (counter) and
   `comments_tombstones_pending` (gauge) carry NO labels. Slice-6 D2
   cardinality invariant unit test in `metrics_server.rs` covers them
   automatically.

9. **CLI exit codes are per-subcommand contracts**: `restore-comment`
   defines {0, 2, 3, 4} per its own table; codes are NOT promised
   stable across `foundry doctor` subcommands (slice-3 `backup-verify`
   already uses 2/3/4 with subcommand-specific meanings). Per-
   subcommand documentation is the contract surface.

## Open Questions for DISTILL

These are intentionally small and bounded; DISTILL resolves them with
the acceptance-designer:

1. **Polling-interval test approach: fakable clock vs subprocess
   override?** The GC task uses `tokio::time::interval` which is
   harder to mock cleanly than `tokio::time::sleep` (slice-5 already
   uses a `FakeClock` for sleep). Two viable approaches: (a) the
   acceptance suite overrides `FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS=1`
   and waits ~1.5s for the first tick (cheap, slightly slow); (b)
   extend `FakeClock` to also intercept `interval` (more engineering,
   faster tests). Recommendation: option (a) for slice-7; revisit
   FakeClock extension if a second test needs the same pattern.
   DISTILL confirms.

2. **`@manual` admin-undelete drill — do we document a manual operator
   drill or rely entirely on the acceptance scenario?** The acceptance
   scenario covers the happy path (CLI exit 0 + DB state change).
   Operator confidence sometimes wants a periodic "run the runbook
   end-to-end on staging" drill (e.g., quarterly). Recommendation:
   one-paragraph addition to `RELEASING.md` § "Recovering an
   accidentally-deleted comment" suggesting a quarterly drill against
   a staging instance; not a hard requirement; no acceptance scenario.
   DISTILL decides.

3. **Suite-time impact of GC scenarios — keep below budget?** The 5
   acceptance scenarios for GC + undelete (date arithmetic, batch
   cap, advisory-lock, transient failure, undelete idempotency) each
   spin up the testcontainers Postgres + run a tick. The cap-scenario
   inserts 11,000 rows — that alone may take ~2-5 seconds. Total
   addition: ~10-30s to the suite. Slice-6's suite-time budget (per
   wave-decisions.md) is ~120s for the integration tier. Suite
   addition is within budget but bears noting. Recommendation:
   DISTILL profiles the cap scenario specifically; if >5s wall-time,
   gate behind `@slow` cucumber tag and run on CI but not on
   pre-commit. DISTILL decides.

4. **Acceptance scenario for the time-warp**: the GC tests need to
   insert rows with `deleted_at = now() - 91 days`. Recommendation:
   the test inserts the literal value directly via SQL setup (not
   via the existing soft-delete handler) — the handler always sets
   `deleted_at = now()`, which is unhelpful for testing the
   threshold. The scenario fixture is "insert tombstoned row with
   `deleted_at = now() - 91 days`" via the test schema rotation
   pattern (slice-1 precedent). DISTILL confirms.

5. **Scope of the failure-survives-task scenario (D7 precedent
   test)**: how does the acceptance suite kill the DB mid-tick? The
   slice-3 US-02 health-injection pattern (`AppState::mark_db_unreachable`
   gated behind `cfg(test, feature = "test-hooks")`) is the
   precedent. Recommendation: extend that flag to also cause the
   `Store::gc_tombstoned_comments` call to fail with a synthetic
   `StoreError::Sqlx(...)`; the test then asserts the task is still
   alive on the next tick. DISTILL confirms — or proposes a cleaner
   mechanism.

6. **`foundry doctor restore-comment` CLI exit codes**: the proposal
   suggests {0 = restored, 2 = invalid UUID, 3 = DB connect failure,
   4 = comment not found / not tombstoned}. Does DISTILL want a
   specific exit code for "comment exists but is NOT currently
   tombstoned" (operator typo on the UUID matching a live comment)?
   Recommendation: same exit 4 — both cases are "the UPDATE matched
   zero rows", indistinguishable from the operator's perspective.
   DISTILL confirms.

7. **Walking-skeleton for the GC**: should one scenario be
   `@walking_skeleton` (real spawn_app + real Postgres + tick
   override to 1 second + assert GC fires within 5s)? Slice-6's
   pool-poll task gets one `@walking_skeleton`; the GC task is
   parallel infrastructure. Recommendation: yes, one walking skeleton
   that exercises the full path (spawn → tick → SELECT → DELETE →
   counter increment → gauge update). DISTILL decides.

8. **`@nfr-*` tag set for slice-7 scenarios**: GC and undelete are
   operationally-flavoured concerns. Does the GC's "deletes within X
   seconds of threshold" scenario ride `@nfr-perf-*` (no existing
   slot) or a new `@nfr-ops-01`? Does the metrics-emission scenario
   ride `@nfr-obs-03` (slice-6 catalogue)? Recommendation: reuse
   `@nfr-obs-03` for metric correctness; introduce no new NFR tag for
   the GC's timing (the 24h cadence is operational, not performance).
   DISTILL confirms.

## Decision-driven invented detail — ACCEPTED

The following 7 specifics were flagged in `proposals.md` § 9 as
under user authority to override. User did NOT override during pick
selection; they are recorded here as ACCEPTED defaults binding on
DISTILL + RED.

1. **`TOMBSTONE_GC_LOCK_ID` literal**: `0x_60_C0_DE_60_C0_DE_60_u64 as i64`
   (visual mnemonic "GOCODE..."). ACCEPTED. Any non-conflicting i64
   would work; the literal has no user-observable consequence beyond
   `pg_locks` readability. The literal is now reserved for tombstone
   GC across the project's lifetime.

2. **Env-var family naming**: `FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS`,
   `FOUNDRY_TOMBSTONE_GC_MAX_PER_RUN`,
   `FOUNDRY_TOMBSTONE_GC_OLDER_THAN_DAYS`. ACCEPTED. The `FOUNDRY_`
   prefix follows the slice-1 `FOUNDRY_PORT` / `FOUNDRY_HOST` family,
   distinguishing operator configuration from internal `METRICS_*` /
   `SESSION_*` vars. Future cleanup tasks follow this family
   (Constraint 3).

3. **First-tick-soon (run first tick within ~30s of startup rather
   than waiting the full 24h)**: ACCEPTED. `tokio::time::interval`
   with `MissedTickBehavior::Skip` fires the first tick within ~5s
   (well inside the "soon" window). Constraint 4 makes this an
   invariant for future cleanup tasks.

4. **CLI subcommand name `restore-comment`** (over `undelete-comment`):
   ACCEPTED. Matches the slice-3 `backup-verify` recovery-verb family
   (`restore`, `verify`, `recover`).

5. **Metric name prefix `foundry_gc_*` (unprefixed shape chosen)**:
   ACCEPTED. Metric names are `comments_tombstones_purged_total`
   (counter) and `comments_tombstones_pending` (gauge) — UNPREFIXED.
   Matches the slice-6 `http_requests_total` / `db_connections_in_use`
   precedent (slice-6 added `foundry_app_startup_total` with the
   prefix as an exception; the unprefixed shape is the project
   default for new metrics).

6. **No-backoff (slice-2 PgListener-style retry-on-error not needed
   for cleanup work)**: ACCEPTED. The 24h natural cadence IS the
   backoff. Persistent errors log identically on each tick; gauge
   flatness is the operator-facing signal.

7. **Default 90-day threshold**: ACCEPTED. ADR-007 already committed
   90 days as the GDPR-friendly default; operators with stricter
   policies tune via `FOUNDRY_TOMBSTONE_GC_OLDER_THAN_DAYS` (down to
   1 day for testing).

Additional flagged detail not in the original 7 but settled by
default during finalization:

- **CLI exit codes** {0, 2, 3, 4} as proposed in
  `architecture.md` § Admin-undelete CLI surface. Per Constraint 9,
  these are subcommand-scoped, not promised across `foundry doctor`
  subcommands. DISTILL may refine within this scope.
- **Default batch size** 1000. ACCEPTED. Tunable via env if needed
  but no `FOUNDRY_TOMBSTONE_GC_BATCH_SIZE` ships in v0.2 (one tunable
  knob = `MAX_PER_RUN` covers the "go bigger during recovery" need).

## Constraint contradictions found

Already documented in `proposals.md` § 10. Briefly:

1. **ADR-007 cited an existing cleanup-task pattern that doesn't
   exist in code**. The architecture.md prose (slice 1, line 259)
   committed to background cleanup with advisory locks; production
   code never landed it. Slice 7 ESTABLISHES the pattern (per
   ADR-015 § Establishes-pattern). Honest with ADR-007's intent;
   flagged for transparency.

2. **Slice-6 D0 deferred-list pinned 5 metrics; D4 adds 2 more**.
   Catalog grows from 5 deferred to 3 deferred + 2 shipped. Not
   blocking; the deferred list was a snapshot, not a contract.

Neither contradiction blocks the slice. Both are documented for
transparency. The slice-7 evolution doc (written post-merge) will
record how the actual implementation honored or diverged from the
pattern.

## Handoff to DISTILL

Acceptance-designer (DISTILL wave) inherits:

1. `architecture.md` — slice-specific design summary; L3 sequence
   diagram for the GC tick + admin-undelete flow.
2. `wave-decisions.md` (this file) — D1–D7 with rationale + 9
   constraints + 8 open questions + 7 invented-detail ACCEPTED items.
3. `adrs/ADR-015-tombstone-gc-scheduling.md` — scheduling pattern
   (cadence + batching + hosting + failure handling).
4. `adrs/ADR-016-gc-observability-and-admin-undelete.md` — observability
   metrics + admin-undelete CLI surface.
5. `adrs/ADR-017-comments-visible-view-deferred.md` — VIEW deferral.

DISTILL's first task is to author `acceptance.feature` files for the 5
substrate-lie scenarios listed in `architecture.md` § Earned Trust,
plus the walking-skeleton scenario (open question 7) if confirmed.
The CLI exit-code contract gets one scenario per code.
