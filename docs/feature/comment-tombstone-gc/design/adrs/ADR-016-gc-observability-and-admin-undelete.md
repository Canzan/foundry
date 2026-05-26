# ADR-016: GC Observability + Admin-Undelete Operator Surface

## Status
Accepted — 2026-05-26

## Context

Slice 7 ships a scheduled background cleanup task (ADR-015) and an
operator-facing recovery path for accidentally-deleted comments
(slice-5 D5's deferred runbook). Two related decisions need recording:

- **Q4 — Observability hook**: does the GC task emit metrics this
  slice, or defer to the v0.3 "5 deferred metrics" slice that slice-6
  D0 implied?
- **Q5 — Admin-undelete recipe scope**: psql one-liner in
  `RELEASING.md`, `foundry doctor restore-comment` CLI, or both?

They are bundled into one ADR because they form the slice's
**operator-visibility pair**: D4 makes the GC's running state
visible (operators can answer "is GC running?" and "is the backlog
growing?"); D5 makes the GC's reversibility actionable (operators can
recover a specific deletion within the 90-day window). The two are
the operator's complete situational-awareness story for the GC: see
what it's doing + undo what shouldn't have happened.

Quality attributes driving these decisions: **observability (MEDIUM)**
— silent GC failure for months is a worse outcome than 2 metrics
shipped slightly ahead of catalogue; **recoverability (MEDIUM)** —
within the 90-day window, undelete must remain a tractable operator
action; **operational ergonomics (MEDIUM)** — operators get the same
information density as slice-3's `backup-verify` runbook.

## Decision

### Observability: Emit 2 metrics now (Q4 = A)

Two new bounded-cardinality metrics emitted from the GC task,
registered at startup at value 0 per slice-6 D4 precedent (avoid "no
data" Grafana panel):

| Metric | Type | Labels | Emission point |
|---|---|---|---|
| `comments_tombstones_purged_total` | counter | (none) | After each GC tick completes successfully; incremented by `rows_deleted`. |
| `comments_tombstones_pending` | gauge | (none) | At the end of each GC tick (after lock release); set to `count(*) WHERE deleted_at < now() - interval '90 days'`. |

Cardinality: both unlabelled — bounded at exactly 1 series each.
Honors slice-6 D2 invariant ("cardinality invariant: forbidden labels
list is binding"). Slice-6's cardinality unit test in
`metrics_server.rs` covers them automatically.

Metric names are UNPREFIXED (no `foundry_gc_*` prefix), matching the
slice-6 `http_requests_total` / `db_connections_in_use` precedent.
Slice-6's `foundry_app_startup_total` had the prefix as an exception
(domain-distinct "process lifecycle" namespace).

Operator alerting story: `comments_tombstones_pending` flat over 48h
indicates a stalled GC task. Standard Prometheus rate / increase /
flatness alert; no custom rule shipped this slice (operator-specific).

**No Grafana dashboard panel added this slice.** The metrics are
emitted but no panel consumer yet — same "instrument me" recursive
pattern slice 6 established with `foundry_app_startup_total`. Panel
addition is a follow-up 5-minute job in `observability/grafana-dashboards/`.

**Coupling to slice-6 D0 deferred-metrics list**: slice-6 D0 pinned 5
deferred metrics (including `bootstrap_tokens_unclaimed`). Slice 7
ships 2 of the GC-family metrics ahead of the broader v0.3
instrumentation slice. Catalog goes from 5 deferred → 3 deferred + 2
shipped. The v0.3 instrumentation slice absorbs the rest naturally;
no structural coupling.

### Admin-undelete: CLI + psql, CLI primary (Q5 = C)

Two operator paths ship in this slice:

**Path 1 — `foundry doctor restore-comment <comment_id>` CLI (recommended)**

Follows the slice-3 `foundry doctor backup-verify` shape:

- Dispatch in `crates/foundry-app/src/main.rs` `dispatch_subcommand`
  next to the `"backup-verify"` arm.
- Implementation in `crates/foundry-app/src/admin_cli.rs`
  (`pub fn run_restore_comment(comment_id: &str) -> i32`).
- Reads `DATABASE_URL` (unlike `backup-verify`'s `FOUNDRY_DOCTOR_PROBE_URL`,
  the restore operates on the live production DB).
- Connects to the live pool, calls `Store::undelete_comment(uuid)`,
  prints `status: restored` on success.
- Exit codes (DISTILL confirms): 0 = restored; 2 = invalid UUID
  syntax; 3 = DB connect failure; 4 = comment not found or not
  tombstoned.

**Path 2 — psql one-liner in `RELEASING.md` (fallback)**

For operators with DB access but no foundry binary on their bastion
host:

```sql
UPDATE comments
   SET deleted_at = NULL, deleted_by = NULL
 WHERE id = '<comment_uuid>'
   AND deleted_at IS NOT NULL
RETURNING id, body_markdown;
```

With safety considerations (confirm UUID matches what was deleted;
verify `deleted_at` is within the 90-day window; inform affected
users).

Both paths use the same SQL semantics (idempotent — re-invoking on a
restored row is a zero-return no-op, not an error).

### `Store::undelete_comment(comment_id)` method

New method on the existing `Store` adapter:

```rust
// Returns 0 if comment not tombstoned or doesn't exist; 1 if restored.
async fn undelete_comment(&self, comment_id: Uuid) -> Result<u64, StoreError>;
```

Single UPDATE matching the slice-5 schema-affords-undelete property
(slice-5 ADR-007). Single chokepoint for both the CLI and any future
audit-logging additions.

## Alternatives Considered

### Observability alternatives

#### A: Emit metrics now (chosen)
See Decision.

#### B: Defer to the broader v0.3 instrumentation slice (rejected)
- **Pros**: Zero new entries in `observability-infra.md` mid-cycle;
  keeps catalog churn batched.
- **Cons**: "GC silently failed for 6 months" is a worse outcome than
  "we shipped 2 metrics in v0.2 instead of v0.3". Operators have no
  programmatic way to alert on GC stalls. Re-discovers the failure
  mode slice 6 was built to address.
- **Rejected because**: defeats slice-6's instrumentation purpose.

#### C: Logs only, structured for log-based alerting (rejected)
- **Pros**: No metric-catalog churn; Loki-based alerts are a real
  pattern.
- **Cons**: Log-based absence-alerts are operationally fragile
  (loggers drop lines under pressure). Less robust than a gauge that
  goes flat. Couples alerting to whatever log-aggregator the operator
  runs.
- **Rejected because**: gauge flatness is the more robust signal.

### Admin-undelete alternatives

#### A: psql one-liner only in `RELEASING.md` (rejected)
- **Pros**: Zero new code.
- **Cons**: Operators without direct DB access need a different path.
  Recipe is harder to test than a CLI subcommand.
- **Rejected because**: slice-3 `backup-verify` precedent argues for
  the CLI surface; psql-only leaves operator ergonomics on the table.

#### B: CLI subcommand only (rejected)
- **Pros**: Consistent with slice-3 precedent.
- **Cons**: Denies the rare "binary unavailable on bastion host"
  operator path.
- **Rejected because**: cost of the psql recipe is ~10 lines of doc;
  ergonomics gain is real.

#### C: Both (chosen)
See Decision.

## Consequences

### Positive
- **Operators can answer "is the GC working?"** from day one via the
  Prometheus counter + gauge.
- **Alerting story is robust.** Gauge flatness is more reliable than
  log absence; Standard PromQL alerts work without custom tooling.
- **Cardinality invariant preserved.** Both metrics unlabelled;
  bounded at exactly 1 series each.
- **CLI provides one-command recovery.** Operators run
  `foundry doctor restore-comment <uuid>` and get a clear exit code +
  status line, matching the slice-3 `backup-verify` ergonomics.
- **psql fallback covers the bastion-host case.** Operators without
  the binary can still recover via the documented SQL recipe.
- **Single chokepoint for audit logging.** Future "emit
  `comment.restored` structured log line" additions land in one place
  (`Store::undelete_comment` or `admin_cli::run_restore_comment`).
- **Schema-affords-undelete property** (slice-5 ADR-007) becomes
  operator-actionable. The slice-5 schema design pays off in slice 7.

### Negative
- **Catalog coherence dip until v0.3.** The slice-6 deferred-metric
  list goes from 5 deferred → 3 deferred + 2 shipped. Documented in
  the metric-naming table; v0.3 instrumentation slice reconciles.
- **Doc duplication (CLI + psql).** Both paths demonstrate the same
  UPDATE. Two surfaces to keep in sync if v0.3 adds, e.g., a logging
  hook to undeletes. Mitigation: `Store::undelete_comment` is the
  single chokepoint; the psql recipe is documentation-only and
  doesn't carry behaviour.
- **No Grafana panel this slice.** Operators who set up dashboards
  via the existing `observability/grafana-dashboards/` directory must
  add panels manually until the v0.3 instrumentation slice ships
  defaults. Trivial 5-minute job.
- **CLI requires `DATABASE_URL` in env.** Production operators have
  it set; runbook-time operators may need to `export DATABASE_URL=...`
  first. Same ergonomic gap as slice-3 backup-verify (which uses
  `FOUNDRY_DOCTOR_PROBE_URL` instead; the restore intentionally hits
  the production DB, not a probe URL).

### Neutral
- **Reversibility**: removing the 2 metrics is a code-level reversal
  (no schema/migration impact); removing the CLI is also code-only.
  The psql recipe in RELEASING.md is removable independently.
- **Metric names follow the `comments_*` family.** Future
  comment-related metrics (e.g., `comments_edited_total`) fit
  alongside.
- **Exit codes are per-subcommand contracts.** The `restore-comment`
  exit-code table (0/2/3/4) is not promised stable across other
  `foundry doctor` subcommands; slice-3 `backup-verify` already uses
  2/3/4 with subcommand-specific meanings.

## Verification

- An acceptance scenario seeds a tombstoned comment, runs the GC
  tick (forced via `FOUNDRY_TOMBSTONE_GC_INTERVAL_SECONDS=1`),
  asserts `comments_tombstones_purged_total` counter incremented by
  the expected count via the `/metrics` endpoint scrape.
- An acceptance scenario seeds N pending tombstones, runs the GC,
  asserts `comments_tombstones_pending` gauge reflects N after the
  tick (and decreases to 0 after sufficient ticks have drained).
- An acceptance scenario invokes the CLI via
  `assert_cmd::Command::cargo_bin("foundry").args(["doctor", "restore-comment", "<uuid>"])`,
  asserts exit 0 + stdout contains "status: restored" + the row's
  `deleted_at` returns to NULL in the DB.
- An acceptance scenario invokes the CLI on a non-tombstoned comment,
  asserts exit 4 + stdout contains an error indication; the row's
  `deleted_at` remains NULL (unchanged).
- An acceptance scenario invokes the CLI on a malformed UUID,
  asserts exit 2.
- An acceptance scenario invokes the CLI with an unreachable
  `DATABASE_URL`, asserts exit 3.
- Idempotency probe: invoking the CLI twice on the same tombstoned
  comment returns exit 0 on the first call and exit 4 on the second
  (because the row is no longer tombstoned). This is the
  `restore-comment` substrate-lie probe per principle 12.
- Metric cardinality is asserted by the existing slice-6
  `metrics_server.rs` invariant unit test (no new test needed; the
  unlabelled metrics pass trivially).
