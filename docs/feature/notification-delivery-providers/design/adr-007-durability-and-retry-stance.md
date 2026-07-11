# ADR-007: Delivery durability + retry stance (best-effort at-most-once for v1)

## Status
Accepted (DESIGN, Propose mode). Resolves **ODD-7** (Risk R5).

## Context
v1 must decide how durable delivery is: retry, de-duplicate, persist and re-drain, or best-effort at-most-once?
Today's shipped behavior is best-effort at-most-once by default (`NoopEmailSender` drops; the three call sites
log-and-continue on failure). The repo has an `outbox` seam (`main.rs:29`, the `OUTBOX_PENDING_JOBS` gauge)
that *could* back durable delivery later. NFR-6 states the stance up front so the limitation is a conscious
product decision, not a silent gap; the DISCUSS recommends ratifying best-effort and deciding whether to leave
a seam.

## Decision
**Ratify best-effort, at-most-once per provider for v1. Leave exactly ONE minimal, non-committal seam — the
`DeliveryError::Transient | Permanent` classification (ADR-003/004) — and build nothing more.** Concretely:
- A provider that fails once is **not** re-invoked for the same notification within the same request; the
  failure is counted (`outcome="failed"`, ADR-004), logged (`class=transient|permanent`, ADR-006), and the
  request proceeds (ADR-003). No retry, no backoff, no dedup key, no idempotency token in v1.
- The **only** forward-compat hook is the error taxonomy: a future durable-retry layer will re-attempt
  `Transient` failures and drop `Permanent` ones — the classification is already produced by every adapter and
  recorded in the log, so the retry layer needs no new provider surface.
- The repo `outbox` (`main.rs:29`) is named as the deferred backing for durable delivery, but is **NOT touched,
  NOT wired, NOT extended** in this feature. No `notify()` signature accommodates it (a future durable path
  would introduce a persisting decorator around the `Notifier`, not change the port).

## Alternatives Considered
- **Build outbox-backed durable retry now (persist each notification, re-drain with backoff, dedup)** —
  REJECTED for v1. It is a separate, larger reliability effort with its own bounded context: a persisted
  delivery-attempt log, an idempotency/dedup key, a re-drainer task, dead-lettering, and an at-least-once vs
  exactly-once semantics decision. Bundling it blows the v1 boundary (slices 01–03) and duplicates the existing
  `outbox` concern. v1's job is to make delivery **possible and observable** through real channels; guaranteed
  delivery is the successor reliability effort.
- **In-request retry with exponential backoff (retry `Transient` N times before giving up)** — REJECTED for
  v1. It would extend the emit-path latency (the request awaits the bounded fan-out, ADR-003) by the retry
  window, and at-most-once + no-dedup means a partially-delivered retry could double-send on a provider that
  actually succeeded-then-reported-timeout. Retry belongs in the durable layer (out of band, deduped), not the
  in-request fan-out.
- **No seam at all (opaque `anyhow` errors)** — REJECTED. It would force the future retry layer to
  string-sniff failure classes (and risk re-attempting a permanent 4xx forever). The `Transient|Permanent`
  taxonomy is a near-zero-cost seam that ADR-003/ADR-004 already need for the log/metric — so it is free to
  leave in place for ADR-007's future.

## Consequences
- Positive: v1 stays thin and honest — it preserves today's best-effort semantics exactly (NFR-5/NFR-6), ships
  no half-built durability, and leaves a single cheap, already-needed seam (the error class) for the successor
  effort.
- Positive: the durable-retry decision is deferred to a feature that can weigh it properly (idempotency keys,
  the `outbox`, dead-lettering) rather than rushed into the delivery-abstraction slice.
- Negative: a notification is lost on a transient provider outage with no automatic recovery (R5) — a conscious,
  stated v1 limitation (NFR-6), mitigated by fan-out (other active providers still deliver) + observability
  (the failure is counted + logged for the operator).
- Probe (Earned Trust): a provider that fails once is not re-invoked for the same notification within the same
  request; the failure is counted + logged with its `class`, and the request proceeds (AC-05.5, @property
  failure-isolation).
