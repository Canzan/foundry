//! The sender's claim choice and the expiry sweep's set (ADR-EIP-002
//! decisions 4 and 9).

use time::OffsetDateTime;

/// One pending delivery of a route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueuedDelivery {
    pub id: i64,
    pub next_attempt_at: Option<OffsetDateTime>,
    pub lease_until: Option<OffsetDateTime>,
    pub give_up_at: OffsetDateTime,
}

impl QueuedDelivery {
    /// A lease is live until its `lease_until`; an expired one is a crashed
    /// sender's, and frees the row.
    fn is_leased_at(&self, now: OffsetDateTime) -> bool {
        self.lease_until.is_some_and(|until| until > now)
    }

    fn is_due_at(&self, now: OffsetDateTime) -> bool {
        self.next_attempt_at.is_none_or(|next| next <= now)
    }

    fn is_past_give_up_at(&self, now: OffsetDateTime) -> bool {
        self.give_up_at <= now
    }

    /// At or past its give-up time with no live lease: the expiry sweep fails
    /// it, and it is never sent again.
    pub(crate) fn is_expired_at(&self, now: OffsetDateTime) -> bool {
        self.is_past_give_up_at(now) && !self.is_leased_at(now)
    }
}

/// The delivery a sender may claim on one route: the head (its lowest id), and
/// only while the head holds no live lease, is due, and its destination's
/// pacing allows a send now. Never a later row, so per-route order holds
/// across replicas.
pub fn claimable_head(
    deliveries: &[QueuedDelivery],
    destination_not_before: Option<OffsetDateTime>,
    now: OffsetDateTime,
) -> Option<i64> {
    if destination_not_before.is_some_and(|paced| paced > now) {
        return None;
    }
    let head = deliveries.iter().min_by_key(|delivery| delivery.id)?;
    (!head.is_leased_at(now) && head.is_due_at(now)).then_some(head.id)
}

/// The deliveries the expiry sweep fails: exactly those at or past their
/// give-up time that hold no live lease.
pub fn expiry_set(now: OffsetDateTime, deliveries: &[QueuedDelivery]) -> Vec<i64> {
    deliveries
        .iter()
        .filter(|delivery| delivery.is_expired_at(now))
        .map(|delivery| delivery.id)
        .collect()
}
