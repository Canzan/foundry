# Rate Guardrail — per-principal mutation throttle (DESIGN)

> NFR-TMA-SEC-07 / Q-RATE-LIMIT. Bound a revoke storm from a single (possibly leaked) bearer.
> ONE binary, NO Redis. Recommend the simplest mechanism that actually protects.

## What it must protect against

The ratified v1 worst case is a **revoke storm**: a leaked management bearer revokes every token in
its workspace (a loud, reversible, workspace-confined DoS). There is no mint loop to bound (mint is
off the bearer surface). So the guardrail's job is narrow: **cap the rate of management MUTATIONS
(DELETE/revoke) per principal**, and **emit a metric** so the storm is observable. LIST is read-only
and not self-amplifying; the guardrail applies to the DELETE route only.

## Options considered

### Option 1 — DB-backed throttle (reuse the sign-in pattern) — REJECTED for this use

The SHIPPED brute-force defense (`signin.rs`) counts failed attempts in Postgres
(`count_recent_failed_signin_attempts`) and adds a 5s delay past a threshold. It is built for a
DIFFERENT threat (password guessing on a public, unauthenticated endpoint, keyed by email) and a
different response (slow down, don't reject). Reusing it for revoke would add a DB round-trip + a new
table/columns to the hot mutation path, and a *delay* is the wrong response to an authenticated
storm (it holds connections open). Rejected: heavier, wrong response shape, and it would add schema
(NFR-TMA-DATA-01 wants zero migrations).

### Option 2 — `last_used`/timestamp check — REJECTED

Gating on the token's `last_used_at` does not bound a *burst* (many revokes in one second all see the
same stale timestamp) and conflates auth-activity with mutation-rate. Insufficient.

### Option 3 — in-process per-principal token bucket — **RECOMMENDED**

A small in-memory token bucket keyed by the calling principal, held in `AppState`, checked on the
DELETE route before the use-case runs. No DB, no Redis, no migration, O(1), single-binary-native.

## Recommended design

**Mechanism:** a per-principal token bucket in process memory.

- **Key:** the bound `user_id` of `Principal::Machine` (the accountable identity — a leaked token and
  any sibling tokens of the same admin share the budget, which is the right blast-radius unit; the
  `jti` alone would let an attacker dodge the cap by rotating which token it calls with — though
  under v1 it cannot mint new ones, keying on `user_id` is strictly safer and matches the audit
  identity). **Confirm at ratification: key by bound `user_id` (recommended) vs `jti`.**
- **Bucket:** capacity `C` tokens, refill `R` tokens/second. A DELETE consumes 1 token; an empty
  bucket → **429 `rate_limited`**. Suggested starting values (DESIGN-tunable, NOT load-bearing):
  `C = 20`, `R = 1/sec` — i.e. a 20-revoke burst then ~1/sec sustained. Generous for legitimate
  rotation/incident response (a few revokes), throttling a runaway loop. Numbers are config-overridable.
- **Storage:** a `Mutex<HashMap<Uuid, BucketState>>` (or `dashmap::DashMap` if already a dep — check;
  prefer std `Mutex<HashMap>` to avoid a new crate). Entries are lazily created and can be swept on a
  coarse timer or simply bounded; given workspace-confined keys the map stays small. Lives in
  `AppState`, derived into the foundry-api adapter via `FromRef` exactly like `Services` /
  `MachineTokenVerifier` (so foundry-api gains no new crate dependency — it reads the shared state
  through the existing seam).
- **Clock:** use the SHIPPED `state.clock` abstraction (`foundry-app/src/clock.rs`) so the bucket is
  deterministically testable (the mock clock the sign-in tests already use) — a burst scenario can
  advance time and assert refill without real sleeps.
- **Placement:** the check runs in the DELETE handler (or a thin tower layer scoped to the mutation
  route) AFTER authentication (so we have a principal to key on) and BEFORE `Services::revoke_token`.
  It must NOT live in foundry-services (the guardrail is an adapter concern — a transport-rate
  policy, not a domain rule) and it must NOT do ad-hoc authz (boundary guard).

### The 429 — where it rides

`ServiceError` has no `TooManyRequests` variant today. Two clean options:

- **(preferred)** the guardrail returns its OWN `ApiError`-compatible 429 response directly in the
  adapter (a small `rate_limited` `ErrorBody`), since rate-limiting is a transport concern that never
  reaches the domain. This keeps `ServiceError` (the cross-adapter contract) unchanged — the web UI
  has no rate concept, so adding a variant there would be noise.
- **(alternative)** add `ServiceError::TooManyRequests` → `status_for` maps it to 429 `rate_limited`.
  Only worth it if a future surface also needs it. **Recommend the adapter-local 429** for v1 (no
  cross-cutting change), revisit if a second consumer appears.

The body is the SHIPPED envelope shape `{"error":{"code":"rate_limited","message":"…"}}` so
US-TMA04's "every refusal is a stable code" contract still holds.

### Metric (the guardrail's observability half)

Emit a per-principal management-mutation counter and a throttle counter:

- `foundry_token_mutations_total{principal, outcome}` — incremented per DELETE (`outcome` =
  `ok|throttled`), so the per-principal mutation rate is observable (NFR-TMA-SEC-07 "guardrail
  metric"). Use the project's existing metrics facade if one is wired; otherwise a `tracing` counter
  field is the minimum. (Confirm the metrics sink with platform-architect — emitting the metric is
  in scope; wiring a Prometheus exporter is platform/DEVOPS.)

## Why this is the simplest effective option

It is pure process memory (no Redis, no DB, no migration), O(1) on the hot path, deterministically
testable via the SHIPPED clock, keyed by the accountable identity, and scoped to the one
self-DoS-capable verb. It bounds the only v1 abuse vector (revoke storm) without touching the domain
core or the cross-adapter `ServiceError` contract.

## Verify

- **US-TMA05 burst scenario** — a DELETE burst beyond `C` returns 429 `rate_limited`; under the cap
  succeeds; the metric reflects the per-principal rate. Uses the mock clock to drive refill
  deterministically.
- The guardrail does NOT regress LIST (read path, unguarded) or the existing `/api/v1` routes.
</content>
