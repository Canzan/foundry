# DISTILL wave-decisions — slice-7-gc-counter-race

**Date**: 2026-05-27
**Wave coverage**: DISTILL only (DISCUSS + DESIGN intentionally skipped — single-feature
test-side hardening; mirrors slice-6-scenario-hardening and us-06-timing-symmetry-redesign)
**Inheritance**:
- `docs/evolution/2026-05-26-comment-tombstone-gc.md` — slice-7 design (ADR-015..017,
  D1-D8) and the GC task contract.
- `docs/evolution/2026-05-27-slice-6-scenario-hardening.md` — the bounded-poll precedent
  and `poll_until_sample` helper. D2 there deferred promoting the helper to a support
  module "when the second caller appears (YAGNI)". This feature IS that second caller.
- `crates/foundry-app/src/main.rs:307-320` — the unchanged GC tick: it increments
  `comments_tombstones_purged_total` by `deleted` **only after** `gc_tombstoned_comments`
  returns `Ok(deleted)` (i.e. after the DELETE has committed).

## Problem in one paragraph

Three fast-loop slice-7 scenarios assert the GC `comments_tombstones_purged_total`
counter via a one-shot scrape after a fixed `running for at least N seconds` wait:
`When … running for at least 2 seconds / And … scrapes / Then … sample has value 3`.
The counter is incremented by a background sweep tick on a cadence (test mode = 2s).
The wait-then-scrape can land *before* the tick fires — or before the seeded
tombstones are even visible to the first tick — so the scrape reads the
register-at-0 value: **`expected 3, got 0`**, intermittently, under `@all`
contention (the subprocess competes for CPU/IO with up to 6 sibling scenarios,
stretching the tick latency past the fixed wait). The counter is correct; the test
samples a temporal value at a single instant. Same class of flake as slice-6's
`db_connections_in_use` gauge — and the same fix.

## Decisions

### D1 — Bounded-poll the counter instead of one-shot scrape

Replace the `running for N seconds` + `scrapes` + `sample has value V` triple with a
single bounded-poll Then that owns its own scrape loop:

```gherkin
Then the "comments_tombstones_purged_total" counter eventually reaches 3 within 15 seconds
```

The step polls `/metrics` every 250ms until the counter sample is `>= 3` or a 15s
deadline elapses (panicking with the full scrape history on timeout). This is the
slice-6 D1 principle applied to a counter: the Gherkin words now express the temporal
nature of the contract a reader can predict the step-impl shape from.

### D2 — Promote `poll_until_sample` to `support::metrics_scrape` (fulfilling slice-6 D2)

slice-6 D2: *"Not promoted to `support/metrics_scrape.rs` yet — single caller, single
scenario. Promote when the second caller appears (YAGNI)."* Slice-7 is that caller.
Moved `poll_until_sample` + `POLL_INTERVAL` (250ms) + `POLL_SCRAPE_TIMEOUT` (750ms)
verbatim from `steps/handler_instrumentation.rs` into `support/metrics_scrape.rs` as
`pub`. The helper is metric-shape-agnostic (it takes a predicate), so a gauge caller
(slice-6) and a counter caller (slice-7) share it unchanged. handler_instrumentation
now imports it; its doc comment generalised from "sampled gauge" to "asynchronously-
updated metric (gauge or scheduled-sweep counter)".

### D3 — `>=` (reaches) semantics, not `==`

The predicate is `s.value >= expected`, expressed in Gherkin as "eventually reaches N".
The counter is monotonic and the seeded tombstone count is exact, so "reaches N" is
robust to an extra tick (no overshoot risk: only N ancient tombstones exist) while
still failing if a tick is missing. `==` would be brittle if a future scenario seeded
in two batches; `>=` is the honest "the work has happened" assertion.

### D4 — Row-state assertions move *after* the counter poll → deterministic

In the gc-lock and gc-failure scenarios, the post-recovery `database holds 0 …older
than 90 days` assertion previously raced the same tick. Because production increments
the counter only after the DELETE commits (`main.rs:319`), ordering the counter poll
*before* the row-state assertion makes the latter deterministic: once the counter
reads `>= 3`, the three rows are provably gone. The `running for N seconds` +
`scrapes` steps that preceded these asserts are removed (the poll does the waiting).

### D5 — Leave the "negative" assertions and the @slow cap scenario unchanged

- **gc-lock first half** (`sample has value 0` while the lock is held) and **gc-failure
  first half** (`database holds 3` after the injected-failure tick) are *negative*
  assertions — they verify that no progress happened. The metric registers at 0 at
  startup (slice-7 sub-deliverable A), so a one-shot scrape reads 0 honestly; bounded-
  poll-until-0 is meaningless. Their preceding `running for 2 seconds` waits are
  load-bearing (they ensure a tick was *attempted* and correctly blocked), so they
  stay. Left untouched (slice-6 D5 "don't touch the non-flaky companion" principle).
- **gc-cap `@slow` scenario** (10000/11000 counter values) is not in the fast loop and
  is not the reported flake. Its DB-state asserts own the cap correctness. Left as-is
  to avoid churn; it passed unchanged in verification.

### D6 — Production code stays unchanged

Zero changes to `crates/foundry-app/`, `crates/foundry-store/`. The GC task, the
counter, the advisory lock, and the register-at-0 are all correct. The fix is in the
test layer only.

## Why the RED gate doesn't apply

Same rationale as slice-6 / us-06: no production code missing, scenarios already green
in isolation (flake only under @all contention), change is assertion-shape not
implementation. Replacement gate: scenario isolation pass + the user's @all sweep.

## Files touched

| Path | Change |
|---|---|
| `crates/foundry-acceptance/src/support/metrics_scrape.rs` | Add `pub poll_until_sample` + the two poll constants (promoted from handler_instrumentation per slice-6 D2) |
| `crates/foundry-acceptance/src/steps/handler_instrumentation.rs` | Remove the local helper + constants; import the promoted one |
| `crates/foundry-acceptance/src/steps/us_10_tombstone_gc.rs` | Add the `…counter eventually reaches N within M seconds` Then |
| `crates/foundry-acceptance/tests/features/comment-tombstone-gc.feature` | Reword the 3 fast-loop counter-increment scenarios to bounded-poll |

Production code: untouched. DESIGN docs: untouched. DEVOPS / CI: untouched.

## Verification protocol

```
cargo test -p foundry-acceptance --test acceptance -- --name "sweep|pending-tombstones"
cargo test -p foundry-acceptance --test acceptance -- --name "Postgres connection pool"
```

### Isolation run (post-change) — PASSED

- GC feature (`--name "sweep|pending-tombstones"`): 6 scenarios / 75 steps passed,
  including the unchanged gc-threshold, the `@slow` gc-cap (10000/11000), and the
  gc-metrics gauge scenarios, plus the 3 reworded bounded-poll scenarios.
- Slice-6 regression (`--name "Postgres connection pool"`): 1 scenario / 9 steps
  passed — confirms relocating `poll_until_sample` to the support module left the
  original caller working.
- `cargo clippy -p foundry-acceptance --tests -- -D warnings`: clean.
- `cargo fmt -p foundry-acceptance -- --check`: clean.

Full @all flake-resistance (N≥5) is validated in the combined sweep with the US-06
fix before the v0.2.0 tag.
