//! The sender's claim choice, the expiry sweep's set (ADR-EIP-002
//! decisions 4 and 9) and the retention sweep's plan (DD-10).

use time::{Duration, OffsetDateTime};

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

/// How long a finished delivery beyond its route's newest
/// [`RETAINED_PER_ROUTE`] is kept, and a dispatched outbox row (DD-10).
pub const RETENTION: Duration = Duration::days(30);
/// Every route keeps at least its newest this many deliveries (NFR-EIP-7).
pub const RETAINED_PER_ROUTE: u32 = 100;
/// A worker heartbeat not renewed for this long is a replica long gone.
pub const HEARTBEAT_RETENTION: Duration = Duration::days(7);

/// One delivery as the retention sweep sees it: when it finished (`None`
/// while it is pending) and how many deliveries of its route (or recipient)
/// are newer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetainedDelivery {
    pub id: i64,
    pub finished_at: Option<OffsetDateTime>,
    pub newer_on_target: u32,
}

/// One outbox row as the retention sweep sees it: when it was dispatched
/// (`None` if never) and whether a pending delivery still reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetainedOutboxRow {
    pub id: i64,
    pub dispatched_at: Option<OffsetDateTime>,
    pub referenced_by_pending: bool,
}

/// What the retention sweep deletes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrunePlan {
    pub deliveries: Vec<i64>,
    pub outbox: Vec<i64>,
    /// Heartbeats last renewed before this are deleted.
    pub heartbeats_before: OffsetDateTime,
}

/// The retention sweep's plan at `now` (DD-10): the finished deliveries older
/// than [`RETENTION`] that are not among their route's newest
/// [`RETAINED_PER_ROUTE`], and the dispatched outbox rows older than
/// [`RETENTION`] no pending delivery reads. A pending delivery, and an outbox
/// row never dispatched, are always kept.
pub fn prune_set(
    now: OffsetDateTime,
    deliveries: &[RetainedDelivery],
    outbox: &[RetainedOutboxRow],
) -> PrunePlan {
    let cutoff = now - RETENTION;
    PrunePlan {
        deliveries: deliveries
            .iter()
            .filter(|d| d.finished_at.is_some_and(|at| at <= cutoff))
            .filter(|d| d.newer_on_target >= RETAINED_PER_ROUTE)
            .map(|d| d.id)
            .collect(),
        outbox: outbox
            .iter()
            .filter(|row| row.dispatched_at.is_some_and(|at| at <= cutoff))
            .filter(|row| !row.referenced_by_pending)
            .map(|row| row.id)
            .collect(),
        heartbeats_before: now - HEARTBEAT_RETENTION,
    }
}
