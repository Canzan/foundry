# ADR-013: SSE Subscriber Gauge via RAII Guard in `foundry-realtime`

## Status
Accepted — 2026-05-25

## Context

The Grafana "Foundry Overview" dashboard has Panel 4 ("Active SSE
subscribers") which queries `sum(sse_subscribers_total)`. The metric is a
gauge representing the count of currently-connected SSE viewers per
project (or in aggregate, since the dashboard `sum()` collapses any
label).

The SSE handler in `crates/foundry-app/src/events.rs` is the natural
point to observe a subscriber's existence — a subscriber exists for
exactly the lifetime of an `sse_stream` Future (from subscription via
`state.realtime_tx.subscribe()` through stream termination by client
disconnect, server shutdown, or panic unwind). The subscriber registry
(the `broadcast::Sender` in `AppState.realtime_tx`) lives in
`foundry-realtime`.

The implementation question is WHERE the gauge increment and decrement
happen, and how the decrement is guaranteed to fire under all termination
paths — client disconnect (network drop, browser tab close), server
shutdown (graceful or panic), and any other Future drop.

Rust's `Drop` trait fires on stack unwinding (including panic unwinding,
unless the panic strategy is `abort`) and on normal scope exit. This
makes RAII the canonical idiom for "this resource exists exactly while
this binding is alive". The pattern is well-established in Rust
(`MutexGuard`, `File`, `tokio::sync::OwnedSemaphorePermit`).

Quality attributes driving this decision: **correctness (HIGH)** — a
leaked subscriber (incremented but never decremented) silently corrupts
the gauge until process restart; **maintainability (HIGH)** — one-line
handler change is the upper limit on cost the slice should pay;
**cohesion (MEDIUM)** — the subscriber concept lives in
`foundry-realtime`, so the type encoding subscriber-lifetime should too.

## Decision

**A RAII guard type `SubscriberGauge` in `foundry-realtime`.**

Definition (in `crates/foundry-realtime/src/lib.rs`):

```rust
pub struct SubscriberGauge {
    project_id: Uuid,
}

impl SubscriberGauge {
    pub fn new(project_id: Uuid) -> Self {
        metrics::gauge!("sse_subscribers_total", "project_id" => project_id.to_string())
            .increment(1.0);
        Self { project_id }
    }
}

impl Drop for SubscriberGauge {
    fn drop(&mut self) {
        metrics::gauge!("sse_subscribers_total", "project_id" => self.project_id.to_string())
            .decrement(1.0);
    }
}
```

Construction in the SSE handler (in `crates/foundry-app/src/events.rs`)
is a single line near the existing `state.realtime_tx.subscribe()` call:

```rust
let _gauge = foundry_realtime::SubscriberGauge::new(project_id);
```

The `_gauge` binding holds the guard for the lifetime of the SSE stream
Future. When the Future is dropped (client disconnect, server shutdown,
panic unwind), `SubscriberGauge::drop` runs and decrements the gauge.

The `project_id` label is kept (per Q4 sub-decision and ADR-011's
exception note). The set of `project_id` values is bounded by the count
of projects with active SSE subscriptions — small at MVP, low hundreds
in the long tail; cardinality-safe.

## Alternatives Considered

### A: RAII guard in `foundry-realtime` (chosen)
See Decision.

### B: Explicit `metrics::gauge!.increment` / `.decrement` in the SSE handler
Handler calls `metrics::gauge!("sse_subscribers_total", ...).increment(1.0)`
after subscribing, and `.decrement(1.0)` in a cleanup arm of the streaming
select.

- **Pros**: All instrumentation visible in one file. No new types.
- **Cons**: Easy to miss the decrement when the stream is cancelled by
  client disconnect mid-poll (the streaming select's "client gone" arm is
  not always obvious in hand-rolled SSE code). The slice-2 SSE handler
  uses a hand-rolled streaming pattern (see the `events.rs` rationale
  comment) — the drop point is non-obvious. Forgetting the decrement
  causes a permanently-incremented gauge that requires process restart
  to reset. Foot-gun.
- **Rejected because**: correctness regression risk is real; RAII is the
  canonical idiom for exactly this problem; the one-line handler change
  in option A is no more code than the explicit calls.

### C: Tower middleware specific to the `/events` route
A second middleware layer that only applies to the SSE endpoint. Increment
on request entry, decrement on response future drop.

- **Pros**: Composes with the existing layer pattern (ADR-010).
- **Cons**: The SSE handler returns an `axum::response::Sse<Stream>`. The
  request future "completes" when the handler returns the Sse response;
  the underlying Stream is then polled by axum/hyper. The middleware
  sees the request as complete at handler-return time, NOT at stream-end
  time. Decrement fires at the wrong moment (gauge drops to zero
  immediately after subscription, even though the stream is still
  delivering events). **Rejected on correctness**, not on style.

## Consequences

### Positive
- Drop fires on every termination path uniformly — client disconnect,
  server graceful shutdown, panic unwind. No "cleanup arm" code path can
  be forgotten.
- One-line handler change. The slice-1 "handlers stay thin" property
  (ADR-001) preserved.
- The cohesion boundary is right: `SubscriberGauge` lives where the
  subscriber concept lives (`foundry-realtime`). The app crate's job is
  to construct one per SSE handler invocation.
- Cross-replica aggregation is Prometheus's job — per-replica gauges
  emit independently; the dashboard's `sum()` query aggregates. The
  per-replica view is also useful for debugging "which replica has all
  the subscribers?" (uneven load-balancer distribution).
- The `project_id` label enables "which project has the most viewers
  right now?" diagnostic queries — useful input for the slice-2 fanout
  sizing decisions.

### Negative
- Adds a `metrics = { workspace = true }` line to
  `crates/foundry-realtime/Cargo.toml`. The crate now has a metrics
  dependency where it previously had none. Acceptable cost (the
  workspace already declares the dep; this is the second crate to
  consume it, after `foundry-app`).
- If Rust's panic strategy is set to `abort` (not Foundry's default,
  but configurable per-deployment), `Drop` does NOT fire on panic and
  the gauge would leak on a panicking handler. The current Foundry
  panic strategy is `unwind` (cargo default); document an invariant
  that future profile changes to `abort` would require revisiting this
  decision.
- A long-running test that subscribes + drops + subscribes + drops
  repeatedly may briefly observe the gauge transiently at +1 or +2
  between increments and decrements (race window of a few microseconds).
  Acceptable for an operational gauge; not acceptable if used for
  correctness assertions in tests (use a higher-level "subscriber list
  size" query instead in tests that need exact consistency).

### Neutral
- `SubscriberGauge::new` takes only `project_id: Uuid`. If the gauge is
  ever wanted with additional labels (e.g., `replica_id`, `client_type`),
  the constructor signature evolves; current scope keeps it minimal.
- No "fire-and-forget" subscribe/unsubscribe counter is emitted
  (would require `sse_subscriptions_total{event="subscribed"}` +
  `event="dropped"`). Not in the dashboard; deferred.

## Verification

- Extension of the existing US-09 SSE acceptance scenario: subscribe N
  clients to a project, scrape `/metrics`, assert
  `sse_subscribers_total{project_id="..."} == N`; then drop all clients,
  wait for stream termination, scrape `/metrics`, assert the gauge
  returns to zero. This is the principle-12 probe ("substrate lie that
  Drop actually ran").
- A panic-injection unit test: construct a `SubscriberGauge`, panic the
  scope, observe the gauge returns to its pre-construction value via
  unwind-triggered Drop. Confirms the panic-unwind invariant.
- An acceptance scenario asserts the `project_id` label appears on the
  emitted gauge (confirms the cardinality exception documented in
  ADR-011 is honored — and only on this specific gauge).
- A code-review checklist item: any future call to `state.realtime_tx.subscribe()`
  outside the events handler MUST be accompanied by a `SubscriberGauge::new`
  binding (or document why the gauge does not apply). The handler is the
  only current call site; this is a forward-discipline note.
