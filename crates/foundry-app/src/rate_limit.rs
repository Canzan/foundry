//! Per-principal revoke-storm guardrail (US-TMA05, NFR-TMA-SEC-07 / Q-RATE-LIMIT).
//!
//! An in-process, per-principal **token bucket** that bounds the rate of
//! management MUTATIONS (DELETE/revoke) from a single accountable identity, so a
//! leaked management bearer cannot drive a workspace-confined revoke storm
//! (`docs/feature/token-management-api/design/rate-guardrail.md`, Option 3 —
//! ratified OD-TMA-1 / OD-TMA-1b / OD-TMA-5).
//!
//! Design facts this module pins:
//!   - **Key** = the bound `user_id` of `Principal::Machine` (OD-TMA-1b) — the
//!     accountable identity, so sibling tokens of the same admin share one
//!     budget and a rotating jti cannot dodge the cap.
//!   - **Bucket** = capacity `C` tokens, refill `R` tokens/second. A revoke
//!     consumes 1; an empty bucket throttles. Defaults `C=20`, `R=1/sec`
//!     (named constants, DESIGN-tunable, NOT load-bearing).
//!   - **Clock** = the SHIPPED [`crate::clock::Clock`] seam, so refill is
//!     deterministic under the acceptance harness's `MockClock` (advance time,
//!     never sleep).
//!   - **Metric** = `foundry_token_mutations_total{principal,outcome}` emitted
//!     per decision via the `metrics` facade foundry-app already depends on, so
//!     the per-principal mutation rate is observable as a guardrail signal.
//!     foundry-api gains NO new crate dependency — it reads this handle through
//!     the existing `AppState` `FromRef` seam and never names the metrics facade.
//!
//! This is an ADAPTER transport-rate policy, NOT a domain rule: it lives in the
//! composition root / adapter layer, never in `foundry-services`, and the 429 it
//! drives rides adapter-local (it leaves the cross-adapter `ServiceError`
//! contract unchanged).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use uuid::Uuid;

use crate::clock::Clock;

/// Bucket capacity `C` — the size of a legitimate burst (rotation / incident
/// response) before sustained throttling kicks in. DESIGN-tunable.
pub const DEFAULT_REVOKE_BUCKET_CAPACITY: u32 = 20;

/// Refill rate `R` in tokens per second. DESIGN-tunable.
pub const DEFAULT_REVOKE_BUCKET_REFILL_PER_SEC: f64 = 1.0;

/// The metric name for the per-principal management-mutation counter
/// (rate-guardrail.md §Metric). `principal` + `outcome` (`ok`|`throttled`)
/// labels make the per-principal rate observable.
pub const TOKEN_MUTATIONS_METRIC: &str = "foundry_token_mutations_total";

/// The outcome of a guardrail check for one revoke attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateDecision {
    /// Within budget — the revoke proceeds (a token was consumed).
    Allowed,
    /// Bucket empty — the revoke is refused (adapter maps to 429 `rate_limited`).
    Throttled,
}

impl RateDecision {
    /// `true` when the revoke is allowed to proceed.
    pub fn is_allowed(self) -> bool {
        matches!(self, RateDecision::Allowed)
    }

    fn outcome_label(self) -> &'static str {
        match self {
            RateDecision::Allowed => "ok",
            RateDecision::Throttled => "throttled",
        }
    }
}

/// Per-principal mutable bucket state. `tokens` is fractional so a sub-second
/// refill accrues correctly; it is clamped to `[0, capacity]` on every refill.
#[derive(Debug, Clone, Copy)]
struct BucketState {
    tokens: f64,
    last_refill_unix_nanos: i128,
}

/// The in-process per-principal revoke guardrail. Held in `AppState`, derived
/// into the foundry-api adapter via `FromRef` exactly like `Services` /
/// `MachineTokenVerifier`.
#[derive(Debug)]
pub struct RevokeRateLimiter {
    capacity: u32,
    refill_per_sec: f64,
    buckets: Mutex<HashMap<Uuid, BucketState>>,
}

impl Default for RevokeRateLimiter {
    fn default() -> Self {
        Self::new(
            DEFAULT_REVOKE_BUCKET_CAPACITY,
            DEFAULT_REVOKE_BUCKET_REFILL_PER_SEC,
        )
    }
}

impl RevokeRateLimiter {
    /// Build a limiter with an explicit capacity `C` and refill rate `R`.
    pub fn new(capacity: u32, refill_per_sec: f64) -> Self {
        Self {
            capacity,
            refill_per_sec,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// Charge one revoke against `principal_user_id`'s bucket as of `now`,
    /// emitting the `foundry_token_mutations_total{principal,outcome}` counter.
    ///
    /// Returns [`RateDecision::Allowed`] when a token was available (and
    /// consumed) or [`RateDecision::Throttled`] when the bucket was empty.
    pub fn check(&self, principal_user_id: Uuid, now: time::OffsetDateTime) -> RateDecision {
        let decision = self.consume(principal_user_id, now);
        metrics::counter!(
            TOKEN_MUTATIONS_METRIC,
            "principal" => principal_user_id.to_string(),
            "outcome" => decision.outcome_label(),
        )
        .increment(1);
        decision
    }

    /// The pure bucket arithmetic (no metric side-effect), exposed for the
    /// invariant unit test: refill by elapsed time at `R`, clamp to `C`, consume
    /// one token if available.
    pub fn consume(&self, principal_user_id: Uuid, now: time::OffsetDateTime) -> RateDecision {
        let now_nanos = now.unix_timestamp_nanos();
        let capacity = self.capacity as f64;
        let mut buckets = self.buckets.lock().expect("revoke bucket mutex");
        let bucket = buckets.entry(principal_user_id).or_insert(BucketState {
            tokens: capacity,
            last_refill_unix_nanos: now_nanos,
        });

        // Refill by elapsed wall-time at R tokens/sec, clamped to capacity C, so
        // tokens_available is monotone-bounded by C and never accrues beyond it.
        let elapsed_secs = (now_nanos - bucket.last_refill_unix_nanos).max(0) as f64 / 1e9;
        bucket.tokens = (bucket.tokens + elapsed_secs * self.refill_per_sec).min(capacity);
        bucket.last_refill_unix_nanos = now_nanos;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            return RateDecision::Allowed;
        }
        RateDecision::Throttled
    }
}

/// Bind a [`RevokeRateLimiter`] to the SHIPPED clock seam so it satisfies the
/// foundry-api [`foundry_api::RevokeRateGuard`] driven port. Built per-request
/// by `AppState`'s `FromRef`; both fields are `Arc`s so the bucket STATE is
/// shared across requests (behind the limiter's own `Mutex`) while the guard
/// itself is cheap to clone. Reading the clock here (rather than in the route)
/// keeps the route ignorant of the time source and the burst test deterministic
/// — advancing the harness `MockClock` is what drives refill, never a sleep.
#[derive(Debug, Clone)]
pub struct ClockedRevokeGuard {
    limiter: Arc<RevokeRateLimiter>,
    clock: Arc<dyn Clock>,
}

impl ClockedRevokeGuard {
    pub fn new(limiter: Arc<RevokeRateLimiter>, clock: Arc<dyn Clock>) -> Self {
        Self { limiter, clock }
    }
}

impl foundry_api::RevokeRateGuard for ClockedRevokeGuard {
    fn check_revoke(&self, principal_user_id: Uuid) -> bool {
        self.limiter
            .check(principal_user_id, self.clock.now())
            .is_allowed()
    }
}

#[cfg(test)]
mod tests {
    //! Port-to-port unit test for the bucket arithmetic. `RevokeRateLimiter`'s
    //! public `consume` IS the driving port — its signature is the contract the
    //! DELETE route calls. The clock value is passed in (the route forwards
    //! `state.clock.now()`), so the test drives refill deterministically by
    //! advancing the timestamp it passes — NO wall-clock, NO mocks inside the
    //! hexagon.
    //!
    //! Behaviour budget: 1 distinct behaviour ("the token bucket bounds the
    //! per-principal revoke rate: capacity-bounded burst, then refill at R over
    //! the clock seam") × 2 = 2. Authored: 1 parametrized invariant test
    //! covering drain, throttle, refill, and the C-clamp.

    use super::*;

    fn t0() -> time::OffsetDateTime {
        time::OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("anchor time")
    }

    /// The core invariant: a fresh principal may consume exactly `C` tokens in a
    /// burst (all Allowed), the next is Throttled, and after advancing the clock
    /// by `n` seconds the bucket has refilled `min(R*n, C)` tokens — never more
    /// than `C` (monotone-bounded by capacity). Drains again to empty afterwards.
    #[test]
    fn bucket_bounds_per_principal_rate_and_refills_over_the_clock() {
        let capacity: u32 = 5;
        let refill_per_sec: f64 = 1.0;
        let limiter = RevokeRateLimiter::new(capacity, refill_per_sec);
        let principal = Uuid::now_v7();
        let now = t0();

        // Burst of exactly C at the same instant: all Allowed (drains the bucket).
        for i in 0..capacity {
            assert_eq!(
                limiter.consume(principal, now),
                RateDecision::Allowed,
                "consume #{i} within capacity C={capacity} must be Allowed"
            );
        }

        // The (C+1)-th at the same instant: bucket empty → Throttled.
        assert_eq!(
            limiter.consume(principal, now),
            RateDecision::Throttled,
            "the revoke beyond capacity C={capacity} must be Throttled"
        );

        // Advance 3s at R=1/sec → 3 tokens refill. Exactly 3 succeed, 4th throttles.
        let later = now + time::Duration::seconds(3);
        for i in 0..3 {
            assert_eq!(
                limiter.consume(principal, later),
                RateDecision::Allowed,
                "refilled consume #{i} (3s elapsed at R={refill_per_sec}) must be Allowed"
            );
        }
        assert_eq!(
            limiter.consume(principal, later),
            RateDecision::Throttled,
            "only R*3=3 tokens refilled — the 4th must be Throttled"
        );

        // Refill is CLAMPED to C: idle a long time, then a burst still tops out at
        // C (monotone-bounded by capacity, never accumulates beyond it).
        let much_later = later + time::Duration::seconds(10_000);
        for i in 0..capacity {
            assert_eq!(
                limiter.consume(principal, much_later),
                RateDecision::Allowed,
                "post-idle consume #{i} must be Allowed up to the C={capacity} clamp"
            );
        }
        assert_eq!(
            limiter.consume(principal, much_later),
            RateDecision::Throttled,
            "refill must clamp at C={capacity}: tokens_available never exceeds capacity"
        );
    }
}
