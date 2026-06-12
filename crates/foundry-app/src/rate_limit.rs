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
//!
//! ## Bucket-map eviction (residual F2 CLOSED — ADR-005, multi-workspace)
//!
//! The per-principal `buckets` `HashMap` (keyed by bound `user_id`) is bounded by
//! a two-tier eviction policy applied opportunistically on each `consume`, keyed
//! off the SAME shipped clock seam (zero new crate, std `HashMap::retain`):
//!
//!   - **PRIMARY — idle eviction (behaviour-preserving).** A bucket idle longer
//!     than the window `W = ceil(C / R)` seconds has fully refilled to `C` and is
//!     indistinguishable from a fresh one, so dropping it and re-creating it at
//!     full `C` on return yields the IDENTICAL state — eviction cannot change any
//!     decision. Under multi-workspace this bounds the map by ACTIVE principals.
//!   - **SECONDARY — LRU size cap `N`.** In the pathological "many distinct
//!     principals all active within `W`" case the idle sweep cannot fire, so a
//!     hard cap `N` ([`DEFAULT_REVOKE_BUCKET_MAX_PRINCIPALS`]) evicts the
//!     least-recently-used buckets down to `N`. Evicting a still-active bucket
//!     only RELAXES its throttle (it resets to full `C` on return) — a bounded,
//!     one-directional trade-off (never an over-throttle of a legitimate
//!     principal), accepted and documented per ADR-005.
//!
//! Pre-multi-workspace this was an accepted residual (keyspace bounded by the
//! O(dozens) admins of a single workspace); with multi-workspace shipped the
//! principal population grows with tenants, so the map is now explicitly bounded.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use uuid::Uuid;

use crate::clock::Clock;

/// Bucket capacity `C` — the size of a legitimate burst (rotation / incident
/// response) before sustained throttling kicks in. DESIGN-tunable.
pub const DEFAULT_REVOKE_BUCKET_CAPACITY: u32 = 20;

/// Refill rate `R` in tokens per second. DESIGN-tunable.
pub const DEFAULT_REVOKE_BUCKET_REFILL_PER_SEC: f64 = 1.0;

/// Hard ceiling `N` on the number of live per-principal buckets (ADR-005
/// secondary policy). Bounds the map even in the pathological "many distinct
/// principals all active within the idle window `W`" case where idle eviction
/// cannot fire. When the map exceeds `N` after the idle sweep, the
/// least-recently-used buckets are evicted down to `N`. Evicting a still-active
/// bucket only ever RELAXES that principal's throttle (it resets to full `C` on
/// return) — one-directional, never an over-throttle. DESIGN-tunable.
pub const DEFAULT_REVOKE_BUCKET_MAX_PRINCIPALS: usize = 10_000;

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
    max_principals: usize,
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
    /// Build a limiter with an explicit capacity `C` and refill rate `R`, using
    /// the default LRU size cap `N` ([`DEFAULT_REVOKE_BUCKET_MAX_PRINCIPALS`]).
    pub fn new(capacity: u32, refill_per_sec: f64) -> Self {
        Self::with_max_principals(
            capacity,
            refill_per_sec,
            DEFAULT_REVOKE_BUCKET_MAX_PRINCIPALS,
        )
    }

    /// Build a limiter with an explicit capacity `C`, refill rate `R`, and LRU
    /// size cap `N` (the hard bound on live buckets — ADR-005 secondary policy).
    pub fn with_max_principals(capacity: u32, refill_per_sec: f64, max_principals: usize) -> Self {
        Self {
            capacity,
            refill_per_sec,
            max_principals,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// The behaviour-preserving idle window `W = ceil(C / R)` seconds — the
    /// refill-to-full horizon. A bucket idle longer than `W` has refilled to the
    /// `C` clamp and is indistinguishable from a fresh one, so evicting it (and
    /// re-creating it at full `C` on return) is behaviour-preserving (ADR-005).
    pub fn idle_window_secs(&self) -> u64 {
        // ceil(C / R) over f64, guarding R <= 0 (treat as "never idle-evict").
        if self.refill_per_sec <= 0.0 {
            return u64::MAX;
        }
        (self.capacity as f64 / self.refill_per_sec).ceil() as u64
    }

    /// The current number of live per-principal buckets — the port-exposed
    /// observable of the eviction policy (used to assert the map stays bounded).
    pub fn bucket_count(&self) -> usize {
        self.buckets.lock().expect("revoke bucket mutex").len()
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

        // ADR-005 PRIMARY policy — idle eviction (behaviour-preserving): drop any
        // bucket idle longer than W = ceil(C/R) seconds. Such a bucket has
        // refilled to the full C clamp, so re-creating it at full C on return
        // yields the identical state — eviction cannot change any decision.
        let window_nanos = (self.idle_window_secs() as i128).saturating_mul(1_000_000_000);
        buckets.retain(|key, b| {
            key == &principal_user_id || (now_nanos - b.last_refill_unix_nanos) <= window_nanos
        });

        // ADR-005 SECONDARY policy — LRU size cap: if the map still exceeds N
        // after the idle sweep (many distinct principals all active within W),
        // evict the least-recently-used (oldest last_refill) down to N. Never
        // evict the principal being charged now. Evicting an active bucket only
        // RELAXES (resets to full C on return) — one-directional, never an
        // over-throttle.
        if buckets.len() >= self.max_principals && !buckets.contains_key(&principal_user_id) {
            let overflow = buckets.len() + 1 - self.max_principals;
            let mut by_recency: Vec<(Uuid, i128)> = buckets
                .iter()
                .map(|(key, b)| (*key, b.last_refill_unix_nanos))
                .collect();
            by_recency.sort_by_key(|(_, last_refill)| *last_refill);
            for (victim, _) in by_recency.into_iter().take(overflow) {
                buckets.remove(&victim);
            }
        }

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
    use crate::clock::MockClock;
    use foundry_api::RevokeRateGuard;
    use proptest::prelude::*;

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

    /// Refill arithmetic must be `elapsed_secs * R` (multiply), not divide.
    /// The original invariant test used R=1.0 with whole-second elapsed, where
    /// `e * 1.0 == e / 1.0`, so the `*`→`/` mutant on line 159 survived. Here R≠1
    /// AND elapsed≠1s so the two operators diverge: at R=2/sec over 2s the bucket
    /// must refill EXACTLY 4 tokens (multiply); the divide mutant would refill
    /// `2/2.0 = 1` token, allowing only 1 of the 4 expected post-refill consumes.
    #[test]
    fn refill_is_elapsed_times_rate_not_divided() {
        let capacity: u32 = 10;
        let refill_per_sec: f64 = 2.0; // R != 1 so * and / diverge
        let limiter = RevokeRateLimiter::new(capacity, refill_per_sec);
        let principal = Uuid::now_v7();
        let now = t0();

        // Drain the fresh bucket completely (C consumes), then confirm empty.
        for _ in 0..capacity {
            assert_eq!(limiter.consume(principal, now), RateDecision::Allowed);
        }
        assert_eq!(limiter.consume(principal, now), RateDecision::Throttled);

        // Advance 2s at R=2/sec → 2 * 2.0 = 4 tokens refilled (multiply).
        // Divide mutant: 2 / 2.0 = 1 token → only the 1st of these would pass.
        let later = now + time::Duration::seconds(2);
        for i in 0..4 {
            assert_eq!(
                limiter.consume(principal, later),
                RateDecision::Allowed,
                "refilled consume #{i}: 2s * R=2/sec must yield exactly 4 tokens (multiply, not divide)"
            );
        }
        // The 5th must throttle: exactly 4 refilled, no more.
        assert_eq!(
            limiter.consume(principal, later),
            RateDecision::Throttled,
            "exactly elapsed*R = 4 tokens refilled — the 5th must be Throttled"
        );
    }

    /// `RateDecision::is_allowed()` and `outcome_label()` must report the exact
    /// per-variant predicate/string. Kills the `is_allowed -> true/false` and
    /// `outcome_label -> ""/"xyzzy"` mutants by asserting both variants directly.
    #[test]
    fn decision_predicates_and_labels_are_per_variant() {
        assert!(
            RateDecision::Allowed.is_allowed(),
            "Allowed must report is_allowed() == true"
        );
        assert!(
            !RateDecision::Throttled.is_allowed(),
            "Throttled must report is_allowed() == false"
        );
        assert_eq!(
            RateDecision::Allowed.outcome_label(),
            "ok",
            "Allowed outcome label must be the metric value \"ok\""
        );
        assert_eq!(
            RateDecision::Throttled.outcome_label(),
            "throttled",
            "Throttled outcome label must be the metric value \"throttled\""
        );
    }

    /// ADR-005 idle eviction (AC1): after advancing the clock past the idle
    /// window `W = ceil(C/R)` seconds, buckets idle beyond `W` are dropped on the
    /// next `consume`, bounding the map under many one-shot idle principals.
    ///
    /// Drive `K` distinct principals once each at `t0` (the map grows to `K`),
    /// then a single distinct "sweeper" principal at `t0 + W + 1s`. The idle `K`
    /// must be evicted, leaving only the active sweeper — observable via the
    /// port-exposed `bucket_count()`. Kills any mutant that skips the sweep.
    #[test]
    fn idle_eviction_bounds_the_map_after_the_idle_window() {
        let capacity: u32 = 5;
        let refill_per_sec: f64 = 1.0;
        let limiter = RevokeRateLimiter::new(capacity, refill_per_sec);
        let now = t0();

        // W = ceil(C/R) = ceil(5/1) = 5s.
        let window = limiter.idle_window_secs();
        assert_eq!(window, 5, "W must be ceil(C/R) = ceil(5/1) = 5s");

        // K idle one-shot principals at t0 → map grows to K.
        let idle_principals: Vec<Uuid> = (0..40).map(|_| Uuid::now_v7()).collect();
        for p in &idle_principals {
            limiter.consume(*p, now);
        }
        assert_eq!(
            limiter.bucket_count(),
            idle_principals.len(),
            "before any sweep the map holds one bucket per distinct principal"
        );

        // A single active sweeper just past the window: idle K are evicted.
        let sweeper = Uuid::now_v7();
        let past_window = now + time::Duration::seconds(window as i64 + 1);
        limiter.consume(sweeper, past_window);

        assert_eq!(
            limiter.bucket_count(),
            1,
            "all K principals idle beyond W must be evicted, leaving only the active sweeper"
        );
    }

    /// The idle window is exactly `W = ceil(C / R)` seconds — the refill-to-full
    /// horizon. The eviction test above uses `R = 1.0`, where `C / R == C * R ==
    /// C % R-ceiled` collapse onto the same value, so the arithmetic-operator
    /// mutants on the `C / R` expression survive there. This pins `W` with
    /// `R != 1` and a NON-integer ratio so divide diverges from every other
    /// operator, and exercises the `R <= 0` guard:
    ///
    /// - C=10, R=4 → ceil(10/4) = ceil(2.5) = 3 (the `*` mutant → ceil(40)=40;
    ///   the `%` mutant → ceil(2)=2)
    /// - C=20, R=1 → ceil(20) = 20 (the shipped default)
    /// - C=20, R=0 → u64::MAX (never idle-evict; the R<=0 guard branch)
    #[test]
    fn idle_window_is_ceil_capacity_over_refill_rate() {
        // Non-integer ratio: divide (2.5→3) differs from multiply (40) and
        // modulo (2). Kills the `/ -> *` and `/ -> %` mutants on `C / R`.
        assert_eq!(
            RevokeRateLimiter::new(10, 4.0).idle_window_secs(),
            3,
            "W = ceil(C/R) = ceil(10/4) = ceil(2.5) = 3s"
        );
        // The shipped default (C=20, R=1) refills to full in 20s.
        assert_eq!(
            RevokeRateLimiter::new(
                DEFAULT_REVOKE_BUCKET_CAPACITY,
                DEFAULT_REVOKE_BUCKET_REFILL_PER_SEC,
            )
            .idle_window_secs(),
            20,
            "W for the shipped defaults C=20, R=1/sec must be ceil(20/1) = 20s"
        );
        // R <= 0 is the "never idle-evict" guard: the window is unbounded so the
        // idle sweep can never fire (a non-refilling bucket is never stale).
        assert_eq!(
            RevokeRateLimiter::new(20, 0.0).idle_window_secs(),
            u64::MAX,
            "a non-positive refill rate must yield an unbounded window (never idle-evict)"
        );
    }

    // ADR-005 behaviour-preservation (AC2): an ACTIVE principal's throttle
    // decisions are byte-identical with eviction enabled and disabled. Property:
    // for an arbitrary interleaving of (other-principal traffic, time advances),
    // the active principal's decision sequence is identical to a reference
    // limiter that the noise/time never perturbs in a decision-relevant way.
    //
    // We compare against a model: the active principal's own bucket math depends
    // ONLY on its own last access + elapsed time, never on other principals or on
    // eviction (idle eviction only drops buckets that have refilled to full C, so
    // re-creation is identical). The model replays the active principal alone.
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(200))]
        #[test]
        fn idle_eviction_is_behaviour_preserving_for_an_active_principal(
            // Each step: (advance_secs in 0..=3, n_noise_principals in 0..=8).
            steps in prop::collection::vec((0u64..=3, 0usize..=8), 1..40),
        ) {
            let capacity: u32 = 4;
            let refill_per_sec: f64 = 1.0;
            let with_eviction = RevokeRateLimiter::new(capacity, refill_per_sec);
            let model = RevokeRateLimiter::new(capacity, refill_per_sec);
            let active = Uuid::now_v7();
            let mut now = t0();

            for (advance, n_noise) in steps {
                now += time::Duration::seconds(advance as i64);
                // Noise: distinct one-shot principals hit the real limiter only.
                for _ in 0..n_noise {
                    with_eviction.consume(Uuid::now_v7(), now);
                }
                // The active principal's decision must match the model that has
                // ONLY ever seen the active principal (no noise, no eviction).
                let real = with_eviction.consume(active, now);
                let reference = model.consume(active, now);
                prop_assert_eq!(
                    real, reference,
                    "active principal decision must be byte-identical with vs without eviction/noise"
                );
            }
        }
    }

    // ADR-005 LRU size-cap fallback (AC3): under pathological load (many active
    // principals within W so idle eviction cannot fire), the hard size cap `N`
    // bounds the map by size, AND eviction is one-directional — it may only ever
    // under-throttle (a returning principal resets to full C), never over-throttle
    // an active principal.
    //
    // Property: drive >> N distinct principals all within the window (no idle
    // sweep possible); the map never exceeds N, and a principal that survived
    // (or returns) is NEVER throttled below what the bucket math alone allows —
    // i.e. the cap can only relax, so the FIRST consume of any principal (fresh
    // or cap-reset) is always Allowed (full C >= 1).
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(100))]
        #[test]
        fn lru_size_cap_bounds_the_map_and_only_relaxes(
            n_principals in 1usize..2_000,
        ) {
            let capacity: u32 = 3;
            let refill_per_sec: f64 = 1.0;
            let cap: usize = 256;
            let limiter = RevokeRateLimiter::with_max_principals(capacity, refill_per_sec, cap);
            let now = t0(); // all within W → idle sweep never fires

            for _ in 0..n_principals {
                let p = Uuid::now_v7();
                // First touch of a fresh (or previously-evicted) principal: a full
                // bucket → Allowed. The cap may evict an LRU victim, never throttle.
                let decision = limiter.consume(p, now);
                prop_assert_eq!(
                    decision, RateDecision::Allowed,
                    "a principal's first consume must be Allowed — the size cap only relaxes, never over-throttles"
                );
                // Hard bound: the map never exceeds the configured cap.
                prop_assert!(
                    limiter.bucket_count() <= cap,
                    "map size {} must stay bounded by the LRU cap N={}",
                    limiter.bucket_count(),
                    cap
                );
            }
        }
    }

    /// ADR-005 LRU size-cap eviction is MINIMAL: when the cap `N` is exceeded by
    /// distinct one-shot principals at a single instant (idle sweep cannot fire),
    /// each new principal evicts EXACTLY ONE least-recently-used victim, so the
    /// map settles at EXACTLY `N` — it must not collapse below `N`.
    ///
    /// This pins the LOWER bound the `<= cap` property cannot see. The overflow
    /// count is `len() + 1 - N`; the `+ -> -` mutant turns it into the `usize`
    /// expression `len() - 1 - N`, which underflows at the cap boundary
    /// (`len() == N`) and (in release) wraps to a huge `take`, evicting the whole
    /// map down to ~1 each time the cap is hit. Asserting the steady-state size is
    /// exactly `N` after driving `>> N` distinct principals kills that mutant: an
    /// over-eviction collapses `bucket_count()` far below `N`.
    #[test]
    fn lru_eviction_is_minimal_map_settles_exactly_at_the_cap() {
        let capacity: u32 = 3;
        let refill_per_sec: f64 = 1.0;
        let cap: usize = 8;
        let limiter = RevokeRateLimiter::with_max_principals(capacity, refill_per_sec, cap);
        let now = t0(); // all within W → idle sweep never fires

        // Drive far more than N distinct principals at the SAME instant. Each one
        // past the cap must evict exactly one LRU victim and insert itself, so the
        // map size is monotone up to N and then PINNED at N — never less.
        for i in 0..(cap * 5) {
            limiter.consume(Uuid::now_v7(), now);
            let size = limiter.bucket_count();
            if i + 1 < cap {
                // Filling phase: exactly one bucket per distinct principal so far.
                assert_eq!(
                    size,
                    i + 1,
                    "before the cap, the map holds one bucket per distinct principal"
                );
            } else {
                // Steady state: minimal eviction holds the map at EXACTLY the cap.
                // (Over-eviction — the `+ -> -` underflow mutant — collapses this
                // far below N.)
                assert_eq!(
                    size, cap,
                    "once the cap N={cap} is reached, each new principal evicts EXACTLY one \
                     LRU victim — the map must settle at exactly N, not collapse below it"
                );
            }
        }
    }

    /// `ClockedRevokeGuard::check_revoke` is the foundry-api driven port the
    /// DELETE route calls. Drive it in-crate over a `RevokeRateLimiter` + the
    /// shipped `MockClock`: within budget it returns true, the over-budget revoke
    /// returns false, and after advancing the clock it returns true again. Kills
    /// the `check_revoke -> true/false` mutants (a constant-true mutant never
    /// throttles; a constant-false mutant never allows).
    #[test]
    fn check_revoke_allows_within_budget_throttles_when_drained_then_refills() {
        let capacity: u32 = 3;
        let refill_per_sec: f64 = 1.0;
        let limiter = Arc::new(RevokeRateLimiter::new(capacity, refill_per_sec));
        let clock = MockClock::at(t0());
        let guard = ClockedRevokeGuard::new(Arc::clone(&limiter), clock.clone());
        let principal = Uuid::now_v7();

        // Within budget: exactly C allowed (true).
        for i in 0..capacity {
            assert!(
                guard.check_revoke(principal),
                "check_revoke #{i} within capacity C={capacity} must return true"
            );
        }
        // Drained: the next revoke is refused (false).
        assert!(
            !guard.check_revoke(principal),
            "check_revoke beyond capacity C={capacity} must return false"
        );

        // Advance the clock 1s at R=1/sec → 1 token refills → true once more.
        clock.advance(std::time::Duration::from_secs(1));
        assert!(
            guard.check_revoke(principal),
            "after a 1s refill at R=1/sec, check_revoke must return true again"
        );
        assert!(
            !guard.check_revoke(principal),
            "only 1 token refilled — the following check_revoke must return false"
        );
    }
}
