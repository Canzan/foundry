//! When a failed attempt is tried again (ADR-EIP-002 decision 6).

use crate::{ChannelOutcome, ReasonCode};
use std::time::Duration;
use time::OffsetDateTime;

/// Slice 01: a fixed interval, never an attempt at or after `give_up_at`.
/// The backoff, jitter and `retry_after` are 03-01's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    interval: Duration,
}

/// What one attempt's outcome does to its delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Settlement {
    Delivered,
    /// Try again at `at`; `reason` is the vendor's.
    RetryAt {
        at: OffsetDateTime,
        reason: ReasonCode,
    },
    /// The next attempt would not be before `give_up_at`: failed, the 24-hour
    /// give-up, with the vendor's last reason.
    GaveUp {
        vendor_reason: ReasonCode,
    },
}

impl RetryPolicy {
    /// Every 30 seconds (slice 01).
    pub const FIXED_30S: RetryPolicy = RetryPolicy {
        interval: Duration::from_secs(30),
    };

    /// Settle one attempt that ended in `outcome` at `now`.
    pub fn settle(
        &self,
        outcome: ChannelOutcome,
        now: OffsetDateTime,
        give_up_at: OffsetDateTime,
    ) -> Settlement {
        let reason = match outcome {
            ChannelOutcome::Delivered => return Settlement::Delivered,
            ChannelOutcome::Transient { reason, .. }
            | ChannelOutcome::RouteRefused { reason }
            | ChannelOutcome::ConnectionRefused { reason } => reason,
        };
        let at = now + self.interval;
        if at < give_up_at {
            Settlement::RetryAt { at, reason }
        } else {
            Settlement::GaveUp {
                vendor_reason: reason,
            }
        }
    }
}
