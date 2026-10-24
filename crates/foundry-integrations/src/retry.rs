//! When a failed attempt is tried again (ADR-EIP-002 decision 6, DDR-5).
//!
//! The backoff starts at 10 s and doubles to 15 min, spread by a jitter of at
//! most ±20% that is fixed per delivery (so one delivery's waits never shrink
//! as its attempts grow, while many deliveries failing together spread out).
//! A wait runs from when the failed attempt began, so a hung attempt's own
//! time counts towards it; a vendor's "retry after" runs from its answer and
//! wins when it is longer. No attempt is ever scheduled at or after the
//! delivery's give-up time.

use crate::{ChannelOutcome, ReasonCode};
use std::time::Duration;
use time::OffsetDateTime;

/// DDR-5's backoff.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    first: Duration,
    cap: Duration,
}

/// One delivery's share of the jitter, in thousandths of its backoff: within
/// ±[`Jitter::BOUND`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Jitter(i16);

impl Jitter {
    /// ±20%.
    pub const BOUND: i16 = 200;
    pub const NONE: Jitter = Jitter(0);

    /// The jitter of the delivery `delivery_id`: the same every time it is
    /// asked, spread evenly across deliveries.
    pub fn of(delivery_id: i64) -> Jitter {
        let mixed = (delivery_id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 32;
        let span = 2 * Self::BOUND as u64 + 1;
        Jitter((mixed % span) as i16 - Self::BOUND)
    }

    /// A jitter of `thousandths`, clamped to the bound.
    pub fn thousandths(thousandths: i16) -> Jitter {
        Jitter(thousandths.clamp(-Self::BOUND, Self::BOUND))
    }

    fn scale(self, base: Duration) -> Duration {
        base * (1000 + self.0 as i32) as u32 / 1000
    }
}

/// One finished attempt: how many attempts the delivery has had (this one
/// included), when this one began, and its jitter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attempt {
    pub number: u32,
    pub started_at: OffsetDateTime,
    pub jitter: Jitter,
}

/// What one attempt's outcome does to its delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Settlement {
    Delivered,
    /// Try again at `at`; `reason` is the vendor's. `slowed_down` when the
    /// vendor asked Foundry to wait, and `not_before` how long the
    /// destination itself must wait (its "retry after").
    RetryAt {
        at: OffsetDateTime,
        reason: ReasonCode,
        slowed_down: bool,
        not_before: Option<OffsetDateTime>,
    },
    /// The vendor refused the destination: the route stops, its deliveries
    /// wait (under the give-up time) until the route is resumed.
    RouteStopped {
        reason: ReasonCode,
    },
    /// The vendor refused the connection: every route of it pauses, its
    /// deliveries wait (under the give-up time) for a reconnect.
    ConnectionLost {
        reason: ReasonCode,
    },
    /// The next attempt would not be before `give_up_at`: failed, the 24-hour
    /// give-up, with the vendor's last reason.
    GaveUp {
        vendor_reason: ReasonCode,
    },
}

impl RetryPolicy {
    /// 10 s doubling to 15 min (DDR-5).
    pub const BACKOFF: RetryPolicy = RetryPolicy {
        first: Duration::from_secs(10),
        cap: Duration::from_secs(15 * 60),
    };

    /// The wait after the `attempt`-th failed attempt (1-based), before
    /// jitter: `first` doubled once per earlier attempt, at most `cap`.
    fn backoff(&self, attempt: u32) -> Duration {
        let doublings = attempt.saturating_sub(1).min(16);
        (self.first * 2u32.pow(doublings)).min(self.cap)
    }

    /// The wait after the `attempt`-th failed attempt with `jitter`.
    pub fn delay(&self, attempt: u32, jitter: Jitter) -> Duration {
        jitter.scale(self.backoff(attempt))
    }

    /// When the delivery is tried again after `attempt` failed with an answer
    /// at `now`: the backoff from the attempt's start, or the vendor's
    /// `retry_after` from its answer when that is later. `None` when that is
    /// not before `give_up_at`.
    pub fn next(
        &self,
        attempt: Attempt,
        retry_after: Option<Duration>,
        now: OffsetDateTime,
        give_up_at: OffsetDateTime,
    ) -> Option<OffsetDateTime> {
        let backoff = attempt.started_at + self.delay(attempt.number, attempt.jitter);
        let at = match retry_after {
            Some(wait) => backoff.max(now + wait),
            None => backoff,
        };
        (at < give_up_at).then_some(at)
    }

    /// Settle one attempt that ended in `outcome` at `now`.
    pub fn settle(
        &self,
        outcome: ChannelOutcome,
        attempt: Attempt,
        now: OffsetDateTime,
        give_up_at: OffsetDateTime,
    ) -> Settlement {
        let (reason, retry_after) = match outcome {
            ChannelOutcome::Delivered => return Settlement::Delivered,
            ChannelOutcome::RouteRefused { reason } => return Settlement::RouteStopped { reason },
            ChannelOutcome::ConnectionRefused { reason } => {
                return Settlement::ConnectionLost { reason }
            }
            ChannelOutcome::Transient {
                reason,
                retry_after,
            } => (reason, retry_after),
        };
        match self.next(attempt, retry_after, now, give_up_at) {
            Some(at) => Settlement::RetryAt {
                at,
                reason,
                slowed_down: reason == ReasonCode::SlackSlowDown,
                not_before: retry_after.map(|wait| now + wait),
            },
            None => Settlement::GaveUp {
                vendor_reason: reason,
            },
        }
    }
}
