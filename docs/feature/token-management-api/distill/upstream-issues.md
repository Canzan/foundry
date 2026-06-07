# Token-Management API — DISTILL Upstream Issues

> One open item. Not a contradiction (the Reconciliation HARD GATE PASSED with 0
> contradictions). This is a DESIGN decision still marked OPEN that blocks
> authoring ONE deterministic acceptance scenario as live RED.

## ISSUE-1 (the one open item): the rate-guardrail MECHANISM is undecided — burst scenario scaffolded `@pending`

**Where**: `design/rate-guardrail.md` + `design/wave-decisions.md` OD-TMA-1,
OD-TMA-1b, OD-TMA-5 — all listed under "Open decisions awaiting user
ratification."

**What is open**:
- OD-TMA-1: the guardrail mechanism (RECOMMENDED: in-process per-principal token
  bucket; alternatives: DB-backed throttle, timestamp check).
- OD-TMA-1b: the key (RECOMMENDED: bound `user_id` vs `jti`).
- OD-TMA-5: the 429 representation (RECOMMENDED: adapter-local response vs a new
  `ServiceError::TooManyRequests`).
- Implied: the bucket capacity `C` / refill `R` (DESIGN-tunable, "NOT
  load-bearing") AND the **test-only clock-advance affordance** the bucket must
  expose so a burst scenario can drive refill via the SHIPPED `state.clock` /
  `MockClock` without wall-clock sleeps.

**Why it blocks a live scenario**: US-TMA05's burst scenario (#18,
`A burst of revocations beyond the guardrail is throttled`) asserts that the
within-guardrail revokes succeed (204), the excess are throttled (429
`rate_limited`), and the per-principal mutation rate is observable as a metric.
Until the bucket mechanism + 429 representation + the clock-advance affordance are
ratified, the scenario cannot be wired to a NON-flaky, deterministic assertion —
and authoring it as a wall-clock timing test would violate the DISTILL
anti-flake guidance.

**What DISTILL did instead** (per the contract: "if the guardrail mechanism is
still an open decision, author this as a SCAFFOLD/`@pending` scenario with a clear
marker rather than a flaky timing test"):
- Scenario #18 is authored **deterministic-by-design** — its step body drives a
  clock-advanced burst and records per-request statuses into
  `world.tma_burst_statuses`; it NEVER calls `sleep` / waits on wall-clock.
- It is tagged `@pending` so it is excluded from the default AND the `@all`
  acceptance lanes (it will not flake CI).
- Its `Then` steps carry explicit SCAFFOLD comments naming OD-TMA-1 and what
  DELIVER must wire (drive `C+K` revokes against distinct seeded tokens, advance
  the `MockClock` between sub-bursts to prove refill, assert the first `C` are
  204 and the excess 429, and assert the
  `foundry_token_mutations_total{principal,outcome}` counter reflects the burst).

**Recommendation**: resolve OD-TMA-1 / OD-TMA-1b / OD-TMA-5 at the post-roadmap
ratification checkpoint (the RECOMMENDED options — token bucket keyed by
`user_id`, adapter-local 429 — are self-consistent and single-binary-native; no
new crate, no migration, deterministically testable via the SHIPPED clock). Once
ratified, DELIVER:
1. Wires the bucket into `AppState` (derived into foundry-api via `FromRef`, like
   `Services` / `MachineTokenVerifier`).
2. Exposes the test-only clock-advance affordance (gated behind
   `cfg(any(test, feature = "test-hooks"))`, the pattern the multi-replica
   harness already uses for `AppState::mark_db_unreachable`).
3. Removes `@pending` from scenario #18 and fills in the `tma_burst_statuses`
   classification + the metric assertion.

**Metric sink sub-note (DEVOPS)**: the per-principal mutation metric
(`foundry_token_mutations_total{principal,outcome}`, DD-TMA-05) needs a confirmed
sink. The DEVOPS directory for this feature is EMPTY. Emitting the metric is in
DELIVER scope; wiring the Prometheus exporter is platform/DEVOPS. Confirm the sink
with the platform-architect before DELIVER reaches the metric assertion in #18.

## Everything else: directly implementable (no other issues)

DESIGN's own audit ("Upstream changes: None") was re-verified independently in
the Reconciliation HARD GATE: the SHIPPED `list_tokens` / `revoke_token`
use-cases, the `MachinePrincipal` extractor, `status_for`/`ErrorBody`, the `jti`
denylist, and `TokenView` are exactly as DISCUSS + DESIGN describe (confirmed by
reading the live source). The 16 active scenarios need only the two route
wirings + the `TokenJson` shape + the no-mint structural-absence + check-arch
rule — all fully specified.
