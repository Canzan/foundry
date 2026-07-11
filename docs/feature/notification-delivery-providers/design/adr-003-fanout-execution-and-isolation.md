# ADR-003: Fan-out execution model, failure isolation, and error taxonomy

## Status
Accepted (DESIGN, Propose mode). Resolves **ODD-3** (Risk R2) and **ODD-4**.

## Context
This is the crux of NFR-3. One emitted notification must reach ALL active providers, and a
failing/slow/panicking provider must **never** (a) fail the originating request, (b) stall it, nor (c) block
the other providers. Today the three call sites do this by hand at N=1: `if let Err(e) =
state.email.send(..).await { tracing::warn!(..) }` (`signin.rs:235`, `bootstrap.rs:258`,
`member_invites.rs:189`) — log-and-continue. The dispatcher must **generalize that exact semantics to N
providers**, not invent a new one (DISCUSS D2).

Sub-decisions: sequential vs concurrent; per-provider timeout; await-bounded vs spawn-detach; and how a
provider reports retryable vs permanent failure (ODD-4) and how that maps to the `outcome` metric label
(ADR-004).

A load-bearing brownfield constraint: the shipped acceptance suite asserts delivery **synchronously** right
after a request (`FakeEmailSender::count_to(x) == 1`, `email.rs:64-70`), and NFR-5 requires that coverage to
"pass unchanged with the notifier substituted." The execution model must keep those synchronous assertions
valid.

## Decision
**`Notifier::notify(&self, notification: &Notification)` — concurrent, per-provider timeout, await-bounded,
infallible:**
1. For each active provider, spawn a task in a `tokio::task::JoinSet`, each wrapping
   `tokio::time::timeout(delivery_timeout, provider.deliver(notification))`.
2. `await` the whole `JoinSet`. Because the tasks run **concurrently**, wall-clock time ≈ a single
   `delivery_timeout` (default 5000ms) regardless of N — not the sum.
3. For each task result, classify the outcome and record it (ADR-004 metric + one structured log line):
   - `Ok(Ok(()))` → `outcome="delivered"`.
   - `Ok(Err(DeliveryError::Transient(msg)))` → `outcome="failed"`, `class="transient"`.
   - `Ok(Err(DeliveryError::Permanent(msg)))` → `outcome="failed"`, `class="permanent"`.
   - `Err(elapsed)` (timeout) → `outcome="failed"`, `class="transient"` (a timeout is transient).
   - `JoinError` (the provider **panicked**) → `outcome="failed"`, `class="transient"`; the panic is contained
     in its task and cannot unwind the request.
4. Return `()`. **`notify` is infallible** — it never returns `Err`, so no call site can be made to fail by a
   provider. The isolation the three call sites hand-code moves *inside* `notify`.

Call sites become a bare `state.notifier.notify(&n).await;` (no `if let Err`) — the await preserves the shipped
"attempt delivery within the request, then continue" shape, now bounded and infallible.

**Error taxonomy (ODD-4)** — `DeliveryError { Transient(String), Permanent(String) }`, returned by every
adapter's `deliver()`. The `String` is an operator-safe, **secret-free** message (ADR-006). Mapping guidance
for adapters:
- **Transient**: connection refused/reset, DNS failure, TLS handshake failure, timeout, HTTP 5xx, HTTP 429,
  SMTP 4xx greylisting.
- **Permanent**: HTTP 4xx (non-429), SMTP 5xx permanent reject, a malformed-recipient rejection.

**The metric `outcome` label stays BINARY `{delivered, failed}`** (ADR-004). The Transient/Permanent
distinction is carried in the **log line** (`class=`) and in the `DeliveryError` type — **not** as a third
`outcome` label value — because (a) there is no retry in v1 (NFR-6) so the distinction has no runtime effect
yet, and (b) a third label value would widen cardinality for no v1 consumer. The taxonomy is the forward-compat
seam the future durable-retry layer (ADR-007) will branch on.

## Alternatives Considered
- **Sequential fan-out (await each provider in a loop)** — REJECTED. A slow provider early in the list would
  delay every later provider (total latency = sum of timeouts), and a hung first provider would stall the
  whole set up to its timeout before the others even start. Concurrency isolates slowness (NFR-3, US-03
  scenario 3).
- **Spawn-and-detach (`tokio::spawn` the whole fan-out, return immediately, never await)** — REJECTED for v1,
  and this is the load-bearing rejection. It is the strongest "never wait", but it **breaks the shipped
  synchronous acceptance assertions** (`FakeEmailSender::count_to` right after the request would race the
  detached task) — violating NFR-5 "existing coverage passes unchanged" and adding regression risk (R7). It
  also makes per-notification observability non-deterministic (the metric increments after the response). The
  await-bounded model bounds the stall to one concurrent timeout while keeping delivery attempts complete
  within the request window — the same trade the shipped call sites already make (they await `email.send`
  today), only now bounded. If a future NFR demands zero emit-path latency, detach becomes a clean follow-up
  (the `notify` signature does not change).
- **`futures::join_all` of timeout futures on the caller task (no `JoinSet`/spawn)** — REJECTED in favor of
  `JoinSet`. `join_all` runs the futures concurrently but on the **caller's** task, so a provider that
  **panics** (not just errors) would unwind the caller — failing the request, the exact thing NFR-3 forbids.
  `JoinSet` spawns each on its own task so a panic is contained (`JoinError`) and counted `failed`.
- **A third `outcome="retryable"` metric label** — REJECTED. No retry in v1 (NFR-6); it widens the bounded
  label (R6) for no consumer. The class lives in the log + error type instead (forward-compat for ADR-007).
- **`anyhow::Result` from `deliver()` (as `EmailSender` uses today)** — REJECTED. An opaque `anyhow` error can
  embed a secret (a `lettre`/`reqwest` error may render a URL with credentials) and gives the dispatcher no
  clean class. A closed `DeliveryError` with hand-built messages is secret-safe (ADR-006) and classifiable.

## Consequences
- Positive: best-effort per-provider isolation is **structural**, not conventional — concurrency + per-provider
  timeout + task containment + infallible return. One provider refused/5xx/timeout/panic cannot fail, stall
  (beyond one timeout), or block the others (@property failure-isolation, AC-03.2/03.3/03.7).
- Positive: the shipped `FakeEmailSender`-based acceptance suite passes unchanged (NFR-5) — the await window
  lets the fake record synchronously.
- Positive: the fan-out generalizes the call sites' existing log-and-continue at N=1 → N (DISCUSS D2); N=1 is
  the slice-01 walking skeleton, so the model is exercised from the first slice.
- Negative: a hung provider adds up to `NOTIFICATION_DELIVERY_TIMEOUT_MS` to the emit path (bounded, tunable,
  counted `failed`). Accepted; the alternative (detach) costs testability + determinism.
- Negative: the metric cannot distinguish transient from permanent failures (only the log can) — accepted; the
  distinction has no v1 runtime meaning and would cost cardinality.
- Probe (Earned Trust): with `NOTIFICATION_PROVIDERS=log,smtp` and SMTP pointed at an unreachable/hanging host,
  a `POST /forgot-password` returns its normal response, the log provider still delivers, and the metric shows
  `{provider="smtp",outcome="failed"}` next to `{provider="log",outcome="delivered"}` — revert-reds-it: making
  `notify` propagate a provider `Err` REDs the isolation @property (AC-03.2/03.3, NFR-3).
