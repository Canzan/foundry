# Design proposals — comment-tombstone-gc (slice 7)

**Mode**: propose
**Owner (this wave)**: solution-architect (Morgan)
**Status**: AWAITING USER DECISION on Q1–Q7.
**Predecessor design**: `docs/feature/foundry-backend-mvp/design/` (slice 1) +
`docs/feature/foundry-realtime-collab/distill/` (slice 2) +
`docs/feature/foundry-operator-grade/distill/` (slice 3) +
`docs/feature/foundry-contributor-onboarding/distill/` (slice 4) +
`docs/feature/comment-edit-delete/design/` (slice 5) +
`docs/feature/handler-instrumentation/design/` (slice 6).
**Layout convention**: legacy per-wave (no `docs/product/`, no
`feature-delta.md`) — see slice-4 `wave-decisions.md` line 204, slice-5
ditto, slice-6 ditto.

---

## 0. What this slice is

A small brownfield slice closing two v0.2-deferred items from slice 5,
bundled because both are operator-facing concerns rooted in the
soft-delete tombstone schema:

1. **90-day GC of tombstoned comments** — per ADR-007 ("Hybrid: soft
   now, GC at 90 days"). Background task hard-deletes rows where
   `deleted_at < now() - interval '90 days'`. Storage stays bounded;
   GDPR-friendly retention; backups (slice 3) shed deleted content
   after the audit window.

2. **Admin-undelete operator runbook** — per slice-5 wave-decisions.md
   D5 ("Recommendation: v0.2"). One-line `psql` recipe (+ considerations)
   for the operator who needs to recover an over-eagerly-deleted
   comment.

The two were explicitly bundled in slice 5's D5 rationale: "the natural
v0.2 follow-up bundles the runbook with the GC task (also deferred to
v0.2 per same DESIGN wave-decisions.md) — both are operator concerns
and ship together cleanly."

---

## 1. Inherited findings — the prior-art that shapes this slice

Before presenting options, several inherited facts shape the design
space. Documenting them up front so the user can challenge them.

### 1a. There is NO existing scheduled cleanup task in production code

The slice-1 `architecture.md` line 259 says:

> Single binary, no separate worker. Cron-style cleanup (expired
> bootstrap tokens, expired sessions, expired reset tokens) runs as a
> background tokio task in every replica, guarded by a Postgres
> advisory lock so only one replica actually runs the cleanup at a
> time.

But grep across `crates/foundry-app/src/main.rs` + the store finds NO
such cleanup task implemented. Expired bootstrap tokens, sessions,
reset tokens, and invites are gated by `expires_at > now()` checks at
READ time. Rows accumulate forever.

The closest precedent in production code is the slice-6
**pool-polling task** in `main.rs` (lines 160–183 — `tokio::spawn` with
`tokio::time::interval` reading pool stats every 5s) and the slice-2
**pg_listener task** (line 107 — long-lived task with reconnect-on-error).
Neither uses an advisory lock; both run on every replica.

**Consequence**: this slice is the first cleanup task in the project.
The pattern it picks becomes the precedent for future GC of expired
bootstrap tokens / sessions / reset tokens / invites. That makes some
of the open questions below higher-stakes than the slice scope alone
suggests; flagged.

### 1b. Advisory-lock pattern is established (`MIGRATION_LOCK_ID`)

`crates/foundry-store/src/lib.rs` line 21 + lines 96-105 show the
exact shape:

```rust
const MIGRATION_LOCK_ID: i64 = 0x_F0_0D_BA_BE_F0_0D_BA_BE_u64 as i64;
// ...
sqlx::query("SELECT pg_advisory_lock($1)").bind(MIGRATION_LOCK_ID)...
// do work
sqlx::query("SELECT pg_advisory_unlock($1)").bind(MIGRATION_LOCK_ID)...
```

Plus the slice-4 scoped variant (`scoped_migration_lock_id`, lines
1229–1255) for per-schema test isolation. The new `TOMBSTONE_GC_LOCK_ID`
follows the same shape; the scoped variant pattern transfers verbatim
if the acceptance suite needs per-schema isolation.

### 1c. Soft-delete schema is a strict subset of GC needs

`crates/foundry-store/migrations/0006_comments_edit_delete.sql` (slice
5) added `deleted_at TIMESTAMPTZ NULL` + `deleted_by UUID NULL`. ADR-007
§ Decision explicitly committed: "The slice-5 schema is a strict
subset of the schema that GC needs; no further migration is required
when v0.2 ships GC." Confirmed by re-reading the migration: nothing to
add.

**Consequence**: NO new migration in slice 7. The Reuse Analysis
forbids one.

### 1d. `foundry doctor` CLI surface is established

Slice 3 introduced `foundry doctor backup-verify <file>` via
`crates/foundry-app/src/admin_cli.rs` + dispatch in `main.rs` lines
243–290. Pattern: subcommand parsing in `main.rs` BEFORE `.env` /
tracing / DB connect (so it runs on operator hosts without Foundry env
vars set), delegating to a function in `admin_cli.rs` that returns an
exit code.

This is the precedent option for Q5 (admin-undelete recipe scope). If
chosen, `foundry doctor restore-comment <comment_id>` follows the same
shape: parse → connect to `DATABASE_URL` → run an UPDATE → exit 0/non-0.
But `restore-comment` requires `DATABASE_URL` which `backup-verify`
intentionally does NOT (it uses `FOUNDRY_DOCTOR_PROBE_URL`); this is a
DIFFERENT operator ergonomics (the host running the restore must have
the production DB URL, which most operators set in env anyway).

### 1e. Slice-6 metric patterns are established

Slice-6 ADR-013 (RAII guard) + ADR-012 (poll-based gauge) +
observability-infra.md `bootstrap_tokens_unclaimed` deferral list
(line 161, deferred per D0). The slice-6 D0 deferred list pinned five
metric families; if Q4 below picks "emit metrics now", that list grows
to include `comments_tombstones_purged_total` (counter) +
`comments_tombstones_pending` (gauge). Bounded cardinality — neither
takes labels — so the cardinality invariant (slice-6 constraint 1)
isn't perturbed.

### 1f. Constraint contradictions found

**None blocking.** Two minor notes:

1. ADR-007's "Hybrid GC task" alternative C says "background task in
   `foundry-app` (Postgres advisory lock for single-replica execution,
   per the existing cleanup-task pattern from slice 1)." The pattern
   does not exist in production code (finding 1a above) — only as
   architecture.md prose. Slice 7 therefore ESTABLISHES the pattern
   rather than inheriting it. This is honest with the ADR's intent
   (advisory-lock cleanup) but adjusts the framing (we're picking the
   pattern now, not inheriting).

2. The task brief says "ADR numbering: slice 5 used ADR-006..009;
   slice 6 used ADR-010..014. Continue with ADR-015+." Verified via
   `Glob` — slice-6 ADRs are ADR-010..014; next is ADR-015. Locked.

---

## 2. Reuse Analysis — HARD GATE

Slice-7 footprint should be heavily EXTEND. Every CREATE NEW is
challenged.

| Action | Target | Why | LOC delta |
|---|---|---|---|
| EXTEND | `crates/foundry-store/src/lib.rs` § cleanup | Add `Store::gc_tombstoned_comments(older_than: Duration) -> Result<u64, StoreError>` returning rows deleted. Uses the existing advisory-lock pattern (new `TOMBSTONE_GC_LOCK_ID` constant). Batches per Q2. | +~60 |
| EXTEND | `crates/foundry-app/src/main.rs` | Spawn a new background task next to the slice-6 pool-poll task. `tokio::time::interval` at the cadence picked by Q1, calling `store.gc_tombstoned_comments(Duration::days(90))` and (per Q4) emitting metrics. | +~30 to +~60 (Q4-dependent) |
| EXTEND | `RELEASING.md` § Operator runbook | Add "Recovering an accidentally-deleted comment" subsection: psql one-liner + safety considerations (timestamp window, double-check before running, audit trail). | +~30 |
| EXTEND | `crates/foundry-app/src/admin_cli.rs` (CONDITIONAL on Q5 = B or C) | Add `pub fn run_restore_comment(comment_id: Uuid) -> i32`. Connects to `DATABASE_URL`, runs `UPDATE comments SET deleted_at=NULL, deleted_by=NULL WHERE id=$1 AND deleted_at IS NOT NULL`, reports affected rows. | +~70 (only if Q5 ≠ A) |
| EXTEND | `crates/foundry-app/src/main.rs` § `dispatch_subcommand` (CONDITIONAL on Q5 = B or C) | Add `"restore-comment"` arm to the existing `"doctor"` dispatch. | +~25 (only if Q5 ≠ A) |
| EXTEND | `docs/feature/foundry-backend-mvp/design/system/observability-infra.md` (CONDITIONAL on Q4 = A) | Add two rows to the metric-naming table: `comments_tombstones_purged_total` + `comments_tombstones_pending`. | +~3 (only if Q4 = A) |
| EXTEND | `crates/foundry-store/migrations/` | NO new migration. ADR-007 § Decision and migration `0006`'s header comment both already commit "the slice-5 schema is a strict subset of GC needs". | 0 |
| CREATE NEW | `crates/foundry-app/src/gc.rs` (only if Q3 = B) | Hosts the scheduling loop separately from `main.rs`. Trade-off documented in Q3 below. | +~60 (only if Q3 = B) |
| CREATE NEW | anything else | None. All work fits in existing files. Slice-1 ADR-001 "no new crates" + slice-6 D5 "no new files unless cohesion requires" hold. | — |

**Total estimated delta** (with Q1 = daily, Q2 = batched 1000 with 10k
cap, Q3 = A inline in main.rs, Q4 = A emit-now, Q5 = A psql-only):
~120 LOC of Rust + ~30 lines of docs. Smaller than slice 6
(~190 LOC). Heavier picks (Q3 = B new module + Q5 = B CLI subcommand)
push toward ~280 LOC, still smaller than slice 5.

---

## 3. Quality attribute drivers

| Attribute | Priority | Why |
|---|---|---|
| Operational simplicity | HIGH | This is the first scheduled cleanup task; the pattern it picks becomes precedent. Drives Q1, Q3, Q7. |
| Storage boundedness | HIGH | Whole purpose of the slice. Drives Q1 (cadence), Q2 (batch). |
| Privacy / GDPR posture | HIGH | "90 days after delete, the content is gone" is the operator-facing promise. Drives Q1 (cadence ≤ 1 day). |
| Auditability (read-time) | MEDIUM | Soft-delete window stays the operator's audit ledger. GC doesn't touch the 90-day window. |
| Recoverability | MEDIUM | Within the 90-day window, undelete must remain a tractable operator action. Drives Q5. |
| Observability | MEDIUM | If GC silently fails for weeks, that's a worse outcome than "GC ran but produced few deletions". Drives Q4, Q7. |

---

## Q1 — Polling interval / cadence

**Question**: how often does the GC task run?

| Option | Description | Pros | Cons |
|---|---|---|---|
| **A. Daily** (every 24 hours) | `tokio::time::interval(Duration::from_secs(86400))`. First tick fires shortly after process start (~5s after boot per `MissedTickBehavior::Skip` semantics); subsequent ticks at 24h. Misses no operator-facing SLA — the GC threshold is 90 days, daily is well inside that. | Simplest cadence to reason about. One GC pass per day per replica (only one wins the advisory lock). Low load — at expected volume, daily produces small batches. Matches the "every cleanup runs once per day at low-load hours" operational tradition. | Within a 24h window, a recently-deleted-then-90-days-old comment may remain ~1 day past the threshold. Operationally fine for a 90-day retention claim (89.0 → 90.999 days isn't a privacy regression that matters). |
| **B. Hourly** (every 60 min) | Same shape, `Duration::from_secs(3600)`. | Tighter retention boundary (~1h slip vs ~24h). | Operational over-engineering for a 90-day threshold. 24× more ticks for ~0 user-observable benefit. If a future cleanup task (expired sessions, expired bootstrap tokens) wants sub-day cadence, it gets a separate task — different concerns shouldn't share a tick. |
| **C. Manual-trigger only** (`foundry doctor gc-comments`) | No background task. Operator opts in by running the subcommand on their schedule (cron job, systemd timer, K8s CronJob). | Maximum operator control. Zero background-task complexity in `main.rs`. Predictable load — operator picks the window. Matches the K8s "schedule lives in the cluster, not the workload" pattern. | Operators who don't read the docs ship a privacy regression (forgotten cron → unbounded storage). The "automatic by default" path is the safer default for an OSS tool whose operators are unknown. Requires the Q5 CLI subcommand to exist; doesn't compose with Q5 = A. |
| **D. Hybrid** (daily background + manual-trigger CLI) | Background task at 24h cadence AND a `foundry doctor gc-comments [--older-than 90d]` subcommand that runs the same code synchronously on-demand. | Best of A + C. Operators who want fine control get the CLI; operators who do nothing still get bounded retention. | Two surfaces (background loop + CLI). Roughly 2x slice cost vs A alone. Composes naturally with Q5 = B/C since the doctor surface is already opened. |

**Recommendation: A (daily)**. Rationale: (a) the 90-day SLA tolerates
slack measured in hours, not days, so daily is comfortably inside the
operator promise; (b) the FIRST cleanup task should pick the simplest
viable pattern — over-engineering it locks future cleanup tasks into
the wrong baseline; (c) hybrid (D) is a clean v0.3 evolution if
operator feedback says "I want to GC NOW after a moderation incident"
— add the CLI later, the inner function is the same. C is rejected on
"OSS-tool-with-unknown-operators" grounds.

**Earned-Trust note**: the background task is a driven adapter on the
clock (it depends on `tokio::time::interval` ticking). Slice 5 already
established `FakeClock` injection for `tokio::time::sleep`; the GC task
uses `tokio::time::interval` which is harder to mock cleanly. For the
acceptance scenario, the cadence is overridden to milliseconds via
`FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS=1` (and an analogous
`OLDER_THAN_DAYS` override for the threshold) so tests don't wait 24h
or 90 days. Same precedent as slice-6's `METRICS_POOL_POLL_SECONDS`.

---

## Q2 — Batch size and safety cap

**Question**: when the GC runs, how many rows does it delete per
invocation, and is there a hard ceiling?

| Option | Description | Pros | Cons |
|---|---|---|---|
| **A. Delete-all-in-one-transaction** | `DELETE FROM comments WHERE deleted_at < now() - interval '90 days'`. One statement, one transaction. | Simplest SQL. Atomic — either all eligible rows go or none do. Matches the "advisory lock serialises us, so we have free reign" intuition. | Long-running DELETE on a busy `comments` table holds row-level locks for the duration. At expected slice-1 scale (1000-comment instance, 5% deletion = 50 rows) this is sub-millisecond, fine. At a scale not yet expected (100k comments, 5% deletion = 5000 rows; or recovery from a misconfigured `deleted_at` value), it's a multi-second table-wide hot spot. No safety against a "deleted_at populated incorrectly" misconfig — the whole table evaporates. |
| **B. Batched at 1000 with hard per-run cap of 10,000** | Loop: `DELETE FROM comments WHERE id IN (SELECT id FROM comments WHERE deleted_at < ... LIMIT 1000)`. Break after 10 batches per invocation. Next tick picks up remaining rows. | Bounded lock-time per batch (a 1000-row DELETE is sub-100ms). Hard cap protects against runaway misconfig — if `deleted_at` got accidentally backdated on 10M rows, GC removes 10k then stops, gives operator time to notice in the next scrape. Drains naturally — at daily cadence, even a million-row backlog clears in 100 days. | Two statements per batch (subquery for IDs + DELETE). Per-tick work bounded; backlog clears slowly. The "drains in 100 days" is fine for normal operation but unhelpful during recovery; operator can override via Q1 = D's manual trigger or by tuning the cap env var. |
| **C. Batched at 1000, no cap** | Loop until the eligible set is empty. | Single tick clears any backlog. | Removes the recovery safety net. A misconfigured `deleted_at` (e.g., operator accidentally backdates) reduces the table to zero in one tick. The cap is cheap insurance. |
| **D. Single DELETE LIMIT N (Postgres-extension), no loop, no cap** | `DELETE FROM comments WHERE id IN (SELECT id FROM comments WHERE deleted_at < ... LIMIT 10000)`. One statement, one transaction, ≤10k rows. | Simplest of the bounded options. Postgres supports `LIMIT` in subqueries cleanly. | Doesn't drain — backlogs persist tick-to-tick until daily ticks chip away (10k/day = 10 days per 100k backlog). At expected steady-state load this is fine; recovery scenarios slower than B. |

**Recommendation: B (batched 1000 with 10k hard cap, env-tunable)**.
Rationale: (a) the safety cap is cheap insurance against the unique
failure mode of THIS kind of task — operational misconfig of
`deleted_at` is the textbook "GC hit the wrong threshold" disaster;
(b) batching of 1000 keeps lock-hold time bounded for current and
future scale; (c) env-tunable cap (default 10,000, override via
`FOUNDRY_TOMBSTONE_GC_MAX_PER_RUN`) gives the operator a "go bigger
during recovery" knob without redeploying; (d) at expected steady-state
load (~50 tombstones per workspace per quarter), the cap is never
reached — pure safety net.

**Earned-Trust note**: the GC's substrate lie to probe is "the WHERE
clause does what it says". The acceptance scenario inserts 3 rows with
`deleted_at = now() - 91 days` and 3 rows with `deleted_at = now() - 89
days`, runs the GC, asserts the older 3 vanished and the newer 3
remain. This is the substrate-lie probe per principle 12 applied to
the GC's date-arithmetic. The 10k cap also gets a scenario: insert
11,000 ancient tombstones, run one GC tick, assert 10,000 went and
1,000 remained; tick again, assert the remaining 1,000 went.

---

## Q3 — Implementation home

**Question**: where do the GC task's spawn logic and the scheduling
loop live?

| Option | Description | Pros | Cons |
|---|---|---|---|
| **A. Inline in `crates/foundry-app/src/main.rs`** | Add a `tokio::spawn` block next to the slice-6 pool-poll task. The closure calls `store.gc_tombstoned_comments(Duration::days(90))` on each tick. | Direct precedent — the pool-poll task lives there already. Slice-6 D5 (hybrid hosting) sets the precedent of "background tasks live in main.rs". Smallest delta. Easiest to grep ("what does main spawn?" finds everything in one file). | `main.rs` grows from 314 lines to ~370 lines. Becomes the "and another thing" parking lot for future cleanup tasks. |
| **B. New module `crates/foundry-app/src/gc.rs`** | Module exports `pub fn spawn_tombstone_gc_task(store: Arc<Store>, interval: Duration)`. `main.rs` calls it with one line; future GC tasks add functions to the same module. | Keeps `main.rs` thin (it's already 314 lines). Cohesive home for "background cleanup tasks" — future expired-sessions / expired-tokens GCs land here too. Easier to unit-test in isolation. | New file, new public surface in `foundry-app`. The slice-6 D5 "hybrid: don't create new files unless cohesion requires" rule applies — this slice ALONE doesn't justify a module; the FUTURE pattern does. |
| **C. Push into `foundry-store` with task driver** | `Store::spawn_tombstone_gc(interval)` returns a `JoinHandle`. `main.rs` calls it with one line; the store crate owns both the SQL and the scheduling. | Keeps the cleanup logic next to the SQL it executes. | Mixes concerns: `foundry-store` becomes a runtime orchestrator, not just an adapter. Spawning `tokio::time::interval` from inside the store crate makes the store's lifecycle non-trivial (test code that calls `Store::from_pool` would auto-start a GC ticker; unwanted). Rejected on separation. |

**Recommendation: A (inline in main.rs) for this slice; promote to B
when a second cleanup task lands.** Rationale: (a) the slice-6 D5
precedent ("hybrid: no new file unless cohesion requires") applies —
ONE background cleanup task doesn't yet justify a `gc.rs`; (b) the
slice-1 ADR-001 precedent ("smallest viable shape") applies; (c)
promotion to B is mechanical when the next cleanup task arrives —
extract both tasks into `gc.rs`, ship in the same v0.3 slice that adds
the second one. C is rejected on concern-mixing.

**Earned-Trust note**: the task spawns from `main.rs` AFTER the
existing `Store::probe()` call (line 109 area, which itself was
extended in slice 5 to verify migration 0006 columns exist). If the
slice-5 probe failed, the process already exited; the GC task only
spawns on a confirmed-healthy substrate. No new probe required —
`gc_tombstoned_comments` is a method on the already-probed adapter.

---

## Q4 — Observability hook

**Question**: does the GC task emit metrics this slice, or defer to
the v0.3 "5 deferred metrics" slice that slice-6 D0 implied?

| Option | Description | Pros | Cons |
|---|---|---|---|
| **A. Emit now** | Two new bounded-cardinality metrics: `comments_tombstones_purged_total` (counter, no labels — incremented by rows-deleted per tick) and `comments_tombstones_pending` (gauge, no labels — `SELECT count(*) FROM comments WHERE deleted_at < now() - interval '90 days'`, polled inside the same task). Cardinality stays within the slice-6 D2 invariant. | Operators can answer "is the GC actually running?" and "is the backlog growing?" from day one. Aligns with slice-6 patterns (poll-based gauge per ADR-012; counter-on-event per ADR-010). Adds the two metrics to the v0.2 release coherently with the GC. Negligible runtime cost (one extra SELECT per tick, plus a counter increment). | Adds 2 entries to the metric-naming table in `observability-infra.md` mid-v0.2-cycle. Couples to the deferred-metrics catalog the slice-6 D0 list pinned (5 deferred → 5 deferred + 2 shipped = inconsistent until v0.3 reconciles). Requires updating the Grafana dashboard JSON if anyone wants a panel; slice doesn't include that. |
| **B. Defer to the broader 5-metric instrumentation slice** | GC task logs `tracing::info!(deleted_count, "tombstone GC completed")` on each tick; no Prometheus metrics. | Keeps the metric catalog churn batched in one v0.3 slice. Zero new entries in `observability-infra.md` this slice. | "GC silently failed for 6 months" is a worse outcome than "we shipped 2 metrics in v0.2 instead of v0.3". Operators have no programmatic way to alert on GC stalls. Re-discovers the failure mode slice 6 was built to address. |
| **C. Logs only, structured for log-based alerting** | `tracing::info!` with a stable name like `tombstone_gc.completed`. Operators using Loki / CloudWatch / Datadog can alert on the absence of this log line over a 48h window. | No metric-catalog churn. Loki-based alerts are a real pattern. Honors slice-6 D0's "no consumer = no metric". | Log-based absence-alerts are operationally fragile (loggers drop lines under pressure). Less robust than a gauge that goes flat. Couples the alerting story to whatever log-aggregator the operator runs. |

**Recommendation: A (emit now)**. Rationale: (a) the GC IS the
consumer that slice-6 D0 said "no consumer = no metric" was waiting
for — the GC's correctness is what these metrics observe; (b) the
slice-6 patterns (poll-based gauge for state, counter for events)
transfer verbatim; (c) bounded cardinality preserves the slice-6 D2
invariant; (d) the "Grafana dashboard panel" concern in the cons
column is real but minor — the metric exists, panel addition is a
post-merge five-minute job in `observability/grafana-dashboards/`; (e)
deferring to the v0.3 5-metric slice creates a 6-month window where
GC stalls are invisible, which is a higher-impact regression than the
catalog-coherence trade-off the cons mention.

**Note on coupling**: if user picks A, the v0.3 deferred-metrics slice
absorbs the 2 new entries into the broader catalog naturally — the
catalog file just grows two rows. No structural coupling.

**Earned-Trust note**: metrics emission is a driven adapter on the
already-probed `metrics_exporter_prometheus` recorder (slice-6 ADR-014
gives us the startup probe). The GC's gauge poll executes a `SELECT
count(*)` — if that query fails (Postgres connection drop), the gauge
is NOT updated (Prometheus sees the stale value, which then ages out
as a `_pending` flat-line — operators alert on flatness). The counter
`metrics::counter!(...).increment(n)` is infallible (it writes to
process-local atomic state).

---

## Q5 — Admin-undelete recipe scope

**Question**: what shape does the operator runbook take?

Slice-5 D5 (line 84): "ADR-007 already documents the schema affords
undelete ('undelete is a single UPDATE'); a literate operator can
derive the recipe. The natural v0.2 follow-up bundles the runbook with
the GC task — both are operator concerns and ship together cleanly."
The pick was deferred, not specified.

| Option | Description | Pros | Cons |
|---|---|---|---|
| **A. psql one-liner in `RELEASING.md`** | New subsection "Recovering an accidentally-deleted comment" with: (1) the SQL recipe `UPDATE comments SET deleted_at = NULL, deleted_by = NULL WHERE id = '<uuid>' AND deleted_at IS NOT NULL RETURNING id, body_markdown;` (2) safety considerations — confirm the UUID matches what was deleted; check `deleted_at` is within 90 days (else GC already ran); ensure the operator has the workspace context to inform affected users. | Zero new code. Operators with `psql` access (which they need anyway for backup-verify, etc.) need no extra tooling. Lives next to the existing `foundry doctor backup-verify` runbook section so operators find it. Matches the slice-5 D5 recommendation language ("operator runbook addition"). | Operators without direct DB access (rare but possible — managed-Postgres customers using IAM-bound passthrough) need a different path. The recipe is harder to test than a CLI subcommand (its behaviour is a function of which DB you pointed psql at). |
| **B. `foundry doctor restore-comment <comment_id>` CLI subcommand only** | Per the slice-3 `foundry doctor backup-verify` precedent. Connects to `DATABASE_URL`, runs the UPDATE, prints the affected row's body so the operator can confirm what was restored, exits 0. Documented in `RELEASING.md` alongside `backup-verify`. | Consistent with the slice-3 precedent of opening a `foundry doctor` surface for operator concerns. Testable as a subprocess. Operator runs ONE command, no psql syntax to copy-paste. No connection-string fiddling beyond `DATABASE_URL`. | New surface to maintain. Adds the operator's mental model — "is this a SQL operation or a CLI operation?" — but `backup-verify` already opened that door. Requires `DATABASE_URL` in the operator's env (which production operators have set; runbook-time operators may not — they'd need to `export` it first). |
| **C. Both** | Ship A (psql recipe in RELEASING.md) AND B (CLI subcommand). The runbook starts with the CLI as the primary path and shows the SQL as a "manual override" alternative. | Maximum operator ergonomics — pick the path that fits your environment. Trivial extra cost over B alone (the SQL is already in the CLI's source). | Doc duplication (recipe and CLI both demonstrate the same UPDATE). Two surfaces to keep in sync if v0.3 adds, e.g., a logging hook to undeletes. |

**Recommendation: C (both, with CLI as primary)**. Rationale: (a) the
slice-3 `backup-verify` precedent argues strongly for the CLI surface
— operator-facing concerns get CLI subcommands; (b) the psql one-liner
is cheap insurance for the "operator has DB access but no foundry
binary on their bastion host" scenario; (c) the doc-duplication
concern is minor — the SQL recipe in RELEASING.md serves as
"what is this CLI doing?" documentation and aids the operator's
audit/review process when re-running the SQL by hand for a known-tricky
case; (d) shipping the CLI now means future audit-logging additions
(e.g., emitting a structured `comment.restored` log line) have a
single chokepoint to add the call.

A pragmatic alternative if reviewer or user wants tighter scope: pick
A only this slice, defer B to v0.3. Costs: operator ergonomics for
this v0.2 release; saves ~95 LOC.

**Earned-Trust note**: the CLI subcommand exercises the production
`DATABASE_URL` connection path — same connection adapter that the
running app uses. No separate "undelete adapter" gets introduced. The
UPDATE statement is the same `Store::undelete_comment` method (new,
~10 LOC) that the CLI dispatches into. Acceptance scenario tests the
CLI end-to-end against a real testcontainers Postgres with a
tombstoned row, asserts the row's `deleted_at` returns to NULL.

---

## Q6 — `comments_visible` SQL VIEW (defense-in-depth)

**Question**: does this slice introduce a `comments_visible` VIEW
(mentioned in ADR-007 alternatives) to make the soft-delete invariant
schema-level rather than behavioural?

ADR-007 § "Cons" of option B (lines 88-91): "UI logic must consistently
apply the `WHERE deleted_at IS NULL` filter (one missed `WHERE` and
deleted comments leak — mitigated by acceptance-suite enforcement;
v0.2 may introduce a `comments_visible` SQL VIEW to make it
schema-level)."

| Option | Description | Pros | Cons |
|---|---|---|---|
| **A. Don't ship the VIEW this slice** | Soft-delete invariant remains behavioural — every read path manually filters `WHERE deleted_at IS NULL`. Slice-5 acceptance scenario 9 (`@soft-delete-invariant`) enforces it for the issue-page list query. New read paths in future slices add their own scenarios. | Zero new surface. Status quo holds. Schema migration discipline (ADR-003 forward-only) doesn't get exercised for a "could be wrong" change. | The "one missed WHERE" risk persists. As US-12+ adds more read paths (e.g., comment-search, comment-export), the convention has more places to be forgotten. |
| **B. Ship `CREATE VIEW comments_visible AS SELECT * FROM comments WHERE deleted_at IS NULL` in a new migration `0007_comments_visible_view.sql`. Existing list queries continue to use `comments` directly with the WHERE filter (no migration of read paths in this slice).** | Schema-level enforcement available immediately. Future read paths can SELECT from `comments_visible` and forget the WHERE clause safely. Slice-7 scope stays small — no rewrite of existing queries. | One-time migration. View maintenance overhead (Postgres views are typically zero-cost wrappers, but they exist). Mixes concerns: this slice is about GC + undelete, not read-side defensive engineering. Net win is delayed until SOMETHING actually reads from the view; no immediate value. |
| **C. Ship the VIEW AND migrate all existing read paths to use it** | Full defense-in-depth. The `comments` table is for writes; `comments_visible` is for reads. List queries in `Store::list_comments_for_issue` etc. switch to `FROM comments_visible`. | Maximum benefit of the VIEW pattern. The "missed WHERE" risk is eliminated structurally. | Significant scope creep — this slice grows to touch every existing read path. The acceptance suite needs full re-execution of comment-rendering paths. Pushes slice 7 from "small bundled deferral closure" to "schema refactor". |

**Recommendation: A (don't ship the VIEW this slice)**. Rationale: (a)
the VIEW is a defense-in-depth concern, separable from the GC + undelete
deferrals this slice closes; (b) shipping the VIEW without consuming
it (option B) creates schema surface area with no immediate value —
"I added a view nobody reads" is dead weight; (c) shipping the VIEW
AND migrating reads (option C) is a separate, larger slice that should
get its own DESIGN pass and acceptance coverage; (d) the slice-5
behavioural invariant + acceptance scenario 9 has been holding fine,
and slice-6 added zero new comment read paths, so the "one missed
WHERE" risk hasn't manifested. Treat the VIEW as a SEPARATE v0.3
candidate ("comment-read-defensive-engineering") if telemetry or
incident history warrants.

**Note**: if the user picks B or C, the slice grows materially. Slice
scope and ADR count both expand.

---

## Q7 — Failure handling pattern (establishes precedent for future cleanup tasks)

**Question**: if the GC's DELETE fails mid-batch (pool drop, advisory-
lock contention with migration, transient Postgres error), what does
the task do?

The slice-1 architecture.md commits to background cleanup tasks but
doesn't standardize failure handling. Slice 6's pool-poll task is
infallible (it reads atomic state, never errors). Slice 7 establishes
the precedent for fallible cleanup tasks.

| Option | Description | Pros | Cons |
|---|---|---|---|
| **A. Log + continue to next tick** | On error, `tracing::warn!(error = ?e, "tombstone GC failed; will retry next interval")` and keep the task alive. The next tick fires at the normal cadence and tries again. | Self-healing for transient errors (pool drop, lock contention). No state to manage. Matches the slice-2 PgListener's "log + reconnect with backoff" precedent. Operators alert on the log line via Loki / Datadog if their stack supports it; metrics-based alerting handled by Q4's `comments_tombstones_pending` gauge going flat. | Persistent errors (e.g., the table dropped, the schema went away) loop forever with the same error every tick. Spammy logs. No escalation. |
| **B. Log + exponential backoff** | On error, double the next tick interval (capped at e.g. 24h max), reset to base interval on success. | Reduces log spam for persistent errors. Matches slice-2 PgListener exactly. | More state in the task. Backoff at daily cadence is awkward — "log every 24h, 48h, 96h, 192h" doesn't actually help operators; at daily cadence the alerting story is "the gauge is flat" (Q4), not "logs are quiet". |
| **C. Abort the GC task on first persistent error; report via metric** | After 3 consecutive failures, log fatal and abort the task. Emit `comments_gc_task_dead = 1` gauge for alerting. Operator restarts the pod to recover. | Loudest signal — operator can't miss a dead GC task. | "Restart the pod to recover from a transient blip" is a heavy hammer. K8s liveness probes would catch a dead task but the rest of Foundry is still serving requests fine; killing the pod to restart one cleanup loop is collateral damage. |
| **D. Crash the process on first persistent error** | Treat GC failure as a "the substrate lies" condition; crash like the slice-6 startup probe does. Container restarts; the restart loop surfaces the misconfig. | Maximum operator-facing signal. Matches slice-6 ADR-014's "refuse to serve traffic if metrics are broken" posture. | Way too aggressive for a daily cleanup task. Conflates "the GC is having a bad day" with "the substrate is fundamentally broken." Rejected. |

**Recommendation: A (log + continue to next tick)**. Rationale: (a)
matches the slice-2 PgListener "log + auto-recover" tradition (which
DOES include backoff, but that's because PgListener ticks at every
notification, not at a daily cadence — slice-7 GC's natural daily
cadence already IS the backoff); (b) the Q4 metric story handles
alerting structurally — flat `comments_tombstones_pending` gauge
over time triggers an operator alert without needing log-based
tooling; (c) the failure modes for a 90-day-window GC are inherently
non-urgent — a missed tick costs at most "deletion happens at day 91
instead of day 90"; (d) the slice-2 precedent works for this slice
modulo the backoff (which doesn't help at daily cadence).

**Documented precedent for future cleanup tasks**: slice 7 establishes
"log warn + continue next tick + operator alerts via Prometheus
gauge flatness". When slice-?? adds expired-sessions GC, expired-
bootstrap-tokens GC, etc., they inherit this pattern. ADR-016
captures it.

**Earned-Trust note**: the GC task is a driven adapter on the Store +
metrics emission. The failure path empirically demonstrates the
contract "errors don't kill the task". The acceptance scenario
forces a failure (e.g., kill the testcontainers Postgres mid-task)
and asserts the task survives — this is the "probe the lie that the
task always recovers" scenario per principle 12.

---

## 4. Proposed ADRs to write once decisions land

Continuing slice-6's ADR-010..014. Slice 7 proposes:

| ADR | Title | Captures | Decision required from |
|---|---|---|---|
| ADR-015 | Tombstone GC scheduling cadence | Q1 outcome | User |
| ADR-016 | Background cleanup task failure handling pattern | Q7 outcome | User |
| ADR-017 | (CONDITIONAL on Q4 = A) Tombstone GC observability — emit-now | Q4 outcome | User |
| ADR-018 | (CONDITIONAL on Q5 = B or C) Admin-undelete CLI surface | Q5 outcome | User |

Q2 (batch + cap), Q3 (implementation home), Q6 (VIEW) settled inline
in `architecture.md` — they're implementation/scope choices that don't
constrain v0.3 evolution. Promote to ADRs if user disagrees.

---

## 5. External integration check (principle 10)

This slice introduces NO new external integrations. GC is fully
internal — same Postgres, same advisory-lock pattern, same metrics
sidecar. No contract test annotation needed for the platform-architect
handoff. The existing SMTP annotation from slice 1 remains.

---

## 6. Architecture enforcement (principle 11)

Existing enforcement holds:

- `cargo xtask check-arch` (slice 1 ADR-001) — no changes to crate boundaries.
- `cargo deny check` — zero new dependencies expected.
- `cargo sqlx prepare --check` — new query (single DELETE statement, single SELECT count(*)) added to the offline cache.

No new tooling needed. The "GC interval is non-zero" and "GC batch cap
is non-zero" invariants are unit-test enforced in
`crates/foundry-store/src/lib.rs` (the gc method panics or returns Err
for zero-or-negative inputs).

---

## 7. Earned Trust (principle 12) — adapter probes

This slice adds NO new adapters and NO new ports. The GC task is a
new METHOD on the existing `Store` adapter, which already has a
`probe()` (extended in slice 5 to verify migration 0006 columns).

No new probe is REQUIRED — but the acceptance scenarios MUST exercise
the substrate lies relevant to THIS task:

1. **Date arithmetic lie**: insert rows with `deleted_at` straddling
   the 90-day threshold; assert only the older ones go.
2. **Batch cap lie**: insert 11,000 ancient tombstones; assert one
   tick removes 10,000 and the next tick removes the last 1,000.
3. **Advisory-lock lie**: ensure two replicas of the GC task running
   simultaneously do NOT double-delete (one wins the lock, the other
   sees zero work).
4. **Transient failure lie** (Q7 = A precedent test): kill the DB
   mid-task; assert the task survives and the next tick succeeds.
5. **Undelete idempotency lie** (Q5 = B/C only): running the
   `restore-comment` CLI on an already-restored comment is a no-op
   that reports zero rows affected, not an error.

All five live in `crates/foundry-acceptance/tests/features/`. No
hand-rolled probe code; the acceptance scenarios ARE the probe per the
slice's "Earned Trust applied to behavior" rule.

---

## 8. Quality-gate self-check before user decisions

- [x] Requirements traced to components — ADR-007 v0.2 commitment →
      `Store::gc_tombstoned_comments` + main.rs task; slice-5 D5 →
      RELEASING.md + (conditional) admin_cli.rs
- [x] Component boundaries respected — no new crates; all changes land
      in existing files (per ADR-001) unless Q3 = B chosen
- [x] Technology choices justified — zero new deps
- [x] Quality attributes addressed — operational simplicity (Q1, Q3),
      storage boundedness (Q1, Q2), privacy (Q1), observability (Q4),
      recoverability (Q5)
- [x] Dependency-inversion compliance — main.rs → store → PG, no
      reverse dependencies
- [x] C4 diagrams — L1/L2 inherited from slice 1; L3 sequence diagram
      provided in architecture.md for the GC task lifecycle
- [x] Integration patterns specified — uses existing advisory-lock
      pattern from `MIGRATION_LOCK_ID`
- [x] OSS preference validated — no new deps
- [x] AC behavioural, not implementation-coupled — all options framed
      around WHAT the system does
- [x] External integrations — none new (principle 10)
- [x] Architectural enforcement tooling — existing cargo-xtask + cargo
      sqlx prepare + cargo deny suffice (principle 11)
- [ ] Peer review — DEFERRED until user decisions on Q1–Q7 land and
      `architecture.md` is finalized

---

## 9. Decision-driven invented detail (FLAGGED for user override)

The following specifics were chosen to make the design concrete and
are under the user's authority to override:

1. **`TOMBSTONE_GC_LOCK_ID` literal value**: proposed
   `0x_60_C_0_DE_60_C_0_DE_u64 as i64` (chosen for visual
   distinctness from `MIGRATION_LOCK_ID = 0x_F0_0D_BA_BE_F0_0D_BA_BE_`).
   Any non-conflicting i64 works; the literal value has no
   user-observable consequence beyond the runbook readability when
   inspecting `pg_locks`.

2. **Env-var names**: proposed `FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS`
   (default 86400 = 1 day) and `FOUNDRY_TOMBSTONE_GC_MAX_PER_RUN`
   (default 10000) and `FOUNDRY_TOMBSTONE_GC_OLDER_THAN_DAYS`
   (default 90). Naming follows the slice-6
   `METRICS_POOL_POLL_SECONDS` precedent (no `FOUNDRY_` prefix on
   metrics vars). The `FOUNDRY_` prefix on GC vars distinguishes
   them from the metrics family; alternative is to drop the prefix
   for consistency.

3. **First-tick offset**: proposed first tick fires ~5 seconds after
   process start (per `tokio::time::interval` default semantics),
   then at 24h intervals. Alternative: skip first tick (wait full
   24h before first GC) — defensible if the operator wants
   "deterministic deletion window from process start". Default
   behaviour matches the slice-6 pool-poll task.

4. **Q5 = B/C subcommand name**: proposed `foundry doctor
   restore-comment <comment_id>`. Alternative: `undelete-comment`.
   "restore" matches the `backup-verify` family of recovery verbs;
   "undelete" matches the conceptual operation more precisely. Both
   work; pick one.

5. **Q4 = A metric names**: proposed `comments_tombstones_purged_total`
   (counter) + `comments_tombstones_pending` (gauge). Alternative
   namings: `foundry_gc_comments_deleted_total` /
   `foundry_gc_comments_pending` (matches the slice-6 `foundry_app_*`
   prefix on the startup counter, but diverges from `http_requests_*`
   which has no prefix). Inconsistent prefixes are already a pattern
   in slice 6's metric catalog — both naming styles defensible.

6. **GC SQL query shape (Q2 = B baseline)**:

   ```sql
   DELETE FROM comments
   WHERE id IN (
       SELECT id FROM comments
       WHERE deleted_at < now() - ($1 || ' days')::interval
       LIMIT $2
   )
   ```

   `$1` = `older_than_days` (default 90), `$2` = batch size (default
   1000). Single statement per batch; loop in Rust until rows-affected
   < batch size OR cumulative-deleted >= per-run cap. The cap variable
   defaults to 10,000 per Q2.

7. **Q7 = A backoff specifics**: NO backoff. Daily cadence already
   IS the backoff; consecutive errors log identically without delay
   change. Alerting story is "Q4's `comments_tombstones_pending`
   gauge stays flat for 48h+" — that's the operator-facing signal.

---

## 10. Constraint contradictions found

**None blocking.** Two findings worth surfacing:

1. **ADR-007 claimed an existing cleanup-task pattern that doesn't
   exist in code** (finding 1a above). The architecture.md prose
   commits to background cleanup; production code never landed it.
   Slice 7 ESTABLISHES the pattern rather than inherits it; this is
   honest with the ADR's intent but flagged for transparency.

2. **Slice-6 D0 deferred-list pinned 5 metrics, including
   `bootstrap_tokens_unclaimed`**, but slice 7's Q4 = A adds 2 more
   GC metrics. If accepted, the "5 deferred metrics" catalog grows
   to 5 deferred + 2 shipped (which is fine; the deferred-list was
   never closed, it was a snapshot of v0.2 state). Documented in Q4.

---

## 11. Under-specified inherited docs

None. ADR-007 § "Hybrid GC task" gives full specification of intent.
Slice-5 D5 deferral is unambiguous. Slice-6 patterns (poll-based gauge,
advisory-lock idiom) are well-documented. The only blank spot is
finding 1a (no existing cleanup-task pattern in code) which this slice
fills.

---

## Next-step instruction for the orchestrator

Collect user picks on Q1–Q7. For each picked option, dispatch back
with `execute --finalize` and the selected options. The finalize pass
will:

1. Write `architecture.md` (slice-specific design summary referencing
   slices 1–6 by inheritance).
2. Write `wave-decisions.md` (DDD-numbered decision list + Reuse
   Analysis + DISTILL open questions).
3. Write `adrs/ADR-015..N.md` (one per decision needing record; final
   count depends on Q4 and Q5 picks).
4. Invoke `solution-architect-reviewer` for peer-review approval.
5. Produce DISTILL handoff package.
