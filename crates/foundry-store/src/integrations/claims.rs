//! Claiming a delivery: a sender's compare-and-set on the row its choice saw,
//! a destination's pacing, a member's email turn, and the senders' candidates
//! (DDR-5, NFR-EIP-3, ADR-EIP-007).

use super::dispatch::DeliveryTarget;
use crate::{Store, StoreError};
use std::time::Duration;
use time::OffsetDateTime;
use uuid::Uuid;

/// NFR-EIP-3: at most one attempt per destination per second.
pub const DESTINATION_SPACING: Duration = Duration::from_secs(1);

/// A pending delivery as the claim choice sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct PendingDelivery {
    pub id: i64,
    pub attempts: i32,
    pub next_attempt_at: Option<OffsetDateTime>,
    pub claim_id: Option<Uuid>,
    pub lease_until: Option<OffsetDateTime>,
    pub give_up_at: OffsetDateTime,
}

/// One locked route, its destination's pacing and its pending deliveries in
/// id order (the head first).
#[derive(Debug, Clone, PartialEq)]
pub struct ClaimView {
    pub route_id: Uuid,
    pub connection_id: Option<Uuid>,
    pub destination_ref: String,
    pub destination_not_before: Option<OffsetDateTime>,
    pub deliveries: Vec<PendingDelivery>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClaimedDelivery {
    pub id: i64,
    pub claim_id: Uuid,
    pub attempts: i32,
    pub lease_until: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaimOutcome {
    /// Another sender holds the route (or it no longer exists).
    RouteBusy,
    /// The choice named no delivery.
    NothingClaimable,
    /// The chosen row changed after it was read; nothing was written.
    Missed,
    Claimed(ClaimedDelivery),
}

/// The claim's compare-and-set: the row the choice saw, unchanged, still the
/// route's (`$5`) or the member's (`$10`), and before its give-up time (`$4`
/// is now): a row at or past `give_up_at` belongs to the expiry sweep, never
/// to a sender (DDR-5).
const CLAIM_SQL: &str = "
    UPDATE integration_deliveries
       SET claim_id = $2, attempts = attempts + 1, lease_until = $3,
           first_attempt_at = COALESCE(first_attempt_at, $4)
     WHERE id = $1 AND route_id IS NOT DISTINCT FROM $5
       AND recipient_user_id IS NOT DISTINCT FROM $10
       AND status = 'pending' AND give_up_at > $4 AND attempts = $6
       AND claim_id IS NOT DISTINCT FROM $7
       AND lease_until IS NOT DISTINCT FROM $8
       AND next_attempt_at IS NOT DISTINCT FROM $9";

/// One member's email claim turn: a transaction lock on the member, scoped
/// by `search_path` like the dispatcher's (U-D2); `false` while another
/// sender holds it.
const RECIPIENT_LOCK_SQL: &str = "
    SELECT pg_try_advisory_xact_lock(hashtextextended(
               current_setting('search_path') || '|email|' || $1::text, 0))";

/// The senders' email candidates: members with a pending email, the oldest
/// pending first.
const SENDABLE_RECIPIENTS_SQL: &str = "
    SELECT recipient_user_id FROM integration_deliveries
     WHERE status = 'pending' AND recipient_user_id IS NOT NULL
     GROUP BY recipient_user_id ORDER BY min(id) LIMIT $1";

/// One destination's claim turn (NFR-EIP-3): a transaction lock on the
/// destination, scoped by `search_path` like the dispatcher's (U-D2).
const DESTINATION_LOCK_SQL: &str = "
    SELECT pg_advisory_xact_lock(hashtextextended(
               current_setting('search_path') || '|' || $1::text || '|' || $2, 0))";

const PACE_SQL: &str = "
    INSERT INTO integration_destination_pacing (connection_id, destination_ref, not_before)
    VALUES ($1, $2, $3)
    ON CONFLICT (connection_id, destination_ref) DO UPDATE SET not_before = EXCLUDED.not_before";

/// The senders' candidates: routes with a pending delivery that are neither
/// stopped by a refusal nor on a lost connection, the oldest pending first.
const SENDABLE_ROUTES_SQL: &str = "
    SELECT d.route_id FROM integration_deliveries d
      JOIN integration_routes r ON r.id = d.route_id
      LEFT JOIN integration_connections c ON c.id = r.connection_id
     WHERE d.status = 'pending' AND r.problem_reason IS NULL
       AND c.state IS DISTINCT FROM 'lost'
     GROUP BY d.route_id ORDER BY min(d.id) LIMIT $1";

pub(super) type PendingRow = (
    i64,
    i32,
    Option<OffsetDateTime>,
    Option<Uuid>,
    Option<OffsetDateTime>,
    OffsetDateTime,
);

impl Store {
    /// One sender claim on `route_id` (ADR-EIP-002 decision 4). Locks the route
    /// (`SKIP LOCKED`: [`ClaimOutcome::RouteBusy`] when another sender holds it,
    /// or when a refusal stopped it or its connection is lost)
    /// and its destination pacing row, reads the route's pending deliveries,
    /// and asks `choose` for the id to claim. The claim is a compare-and-set on
    /// that row as it was read: a fresh `claim_id`, `attempts + 1`,
    /// `lease_until = now + lease`, the first attempt time, and the
    /// destination's `not_before = now + 1 s`. `now` is the injected clock's.
    pub async fn claim_delivery<C>(
        &self,
        route_id: Uuid,
        now: OffsetDateTime,
        lease: Duration,
        choose: C,
    ) -> Result<ClaimOutcome, StoreError>
    where
        C: FnOnce(&ClaimView) -> Option<i64>,
    {
        let mut tx = self.pool.begin().await?;
        let route: Option<(Option<Uuid>, String)> = sqlx::query_as(
            "SELECT r.connection_id, r.destination_ref FROM integration_routes r
              WHERE r.id = $1 AND r.problem_reason IS NULL
                AND NOT EXISTS (SELECT 1 FROM integration_connections c
                                 WHERE c.id = r.connection_id AND c.state = 'lost')
                FOR UPDATE OF r SKIP LOCKED",
        )
        .bind(route_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((connection_id, destination_ref)) = route else {
            tx.rollback().await?;
            return Ok(ClaimOutcome::RouteBusy);
        };
        // Senders of every route to this destination, on every replica, take
        // their turn here, so the pacing row is read as the last claim left
        // it, even before the destination's first claim creates it.
        if connection_id.is_some() {
            sqlx::query(DESTINATION_LOCK_SQL)
                .bind(connection_id)
                .bind(&destination_ref)
                .execute(&mut *tx)
                .await?;
        }
        let destination_not_before: Option<OffsetDateTime> = sqlx::query_scalar(
            "SELECT not_before FROM integration_destination_pacing
              WHERE connection_id = $1 AND destination_ref = $2 FOR UPDATE",
        )
        .bind(connection_id)
        .bind(&destination_ref)
        .fetch_optional(&mut *tx)
        .await?;
        let pending: Vec<PendingRow> = sqlx::query_as(
            "SELECT id, attempts, next_attempt_at, claim_id, lease_until, give_up_at
               FROM integration_deliveries
              WHERE route_id = $1 AND status = 'pending' ORDER BY id",
        )
        .bind(route_id)
        .fetch_all(&mut *tx)
        .await?;
        let view = ClaimView {
            route_id,
            connection_id,
            destination_ref,
            destination_not_before,
            deliveries: pending.into_iter().map(pending_delivery).collect(),
        };
        let target = DeliveryTarget::Route(route_id);
        let claim = match claim_chosen(&mut tx, &view, target, now, lease, choose).await? {
            ClaimOutcome::Claimed(claim) => claim,
            other => {
                tx.rollback().await?;
                return Ok(other);
            }
        };
        if let Some(connection_id) = view.connection_id {
            sqlx::query(PACE_SQL)
                .bind(connection_id)
                .bind(&view.destination_ref)
                .bind(now + DESTINATION_SPACING)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(ClaimOutcome::Claimed(claim))
    }

    /// One sender claim on the member `recipient_user_id`'s project emails
    /// (ADR-EIP-007 decision 2): under the member's own lock
    /// ([`ClaimOutcome::RouteBusy`] while another sender holds it), reads
    /// their pending emails and claims the one `choose` names, by the same
    /// compare-and-set as a route's claim. An email has no destination to
    /// pace: the view names none, and its `route_id` is the member.
    pub async fn claim_email_delivery<C>(
        &self,
        recipient_user_id: Uuid,
        now: OffsetDateTime,
        lease: Duration,
        choose: C,
    ) -> Result<ClaimOutcome, StoreError>
    where
        C: FnOnce(&ClaimView) -> Option<i64>,
    {
        let mut tx = self.pool.begin().await?;
        let (locked,): (bool,) = sqlx::query_as(RECIPIENT_LOCK_SQL)
            .bind(recipient_user_id)
            .fetch_one(&mut *tx)
            .await?;
        if !locked {
            tx.rollback().await?;
            return Ok(ClaimOutcome::RouteBusy);
        }
        let pending: Vec<PendingRow> = sqlx::query_as(
            "SELECT id, attempts, next_attempt_at, claim_id, lease_until, give_up_at
               FROM integration_deliveries
              WHERE recipient_user_id = $1 AND status = 'pending' ORDER BY id",
        )
        .bind(recipient_user_id)
        .fetch_all(&mut *tx)
        .await?;
        let view = ClaimView {
            route_id: recipient_user_id,
            connection_id: None,
            destination_ref: String::new(),
            destination_not_before: None,
            deliveries: pending.into_iter().map(pending_delivery).collect(),
        };
        let target = DeliveryTarget::Recipient(recipient_user_id);
        let outcome = claim_chosen(&mut tx, &view, target, now, lease, choose).await?;
        if matches!(outcome, ClaimOutcome::Claimed(_)) {
            tx.commit().await?;
        } else {
            tx.rollback().await?;
        }
        Ok(outcome)
    }

    /// The members with a pending project email, the oldest pending first
    /// (the senders' email candidates; each is claimed under its own lock).
    pub async fn recipients_with_pending_deliveries(
        &self,
        limit: i64,
    ) -> Result<Vec<Uuid>, StoreError> {
        Ok(sqlx::query_scalar(SENDABLE_RECIPIENTS_SQL)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?)
    }

    /// The routes with a pending delivery that a sender may claim from (not
    /// stopped by a refusal, not on a lost connection), the oldest pending
    /// first (the senders' candidates; each is claimed under its own lock).
    pub async fn routes_with_pending_deliveries(
        &self,
        limit: i64,
    ) -> Result<Vec<Uuid>, StoreError> {
        Ok(sqlx::query_scalar(SENDABLE_ROUTES_SQL)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?)
    }
}

/// The claim `choose` names over `view`, by compare-and-set on the row as it
/// was read; `Missed` when that row is not in the view or changed since.
async fn claim_chosen<C>(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    view: &ClaimView,
    target: DeliveryTarget,
    now: OffsetDateTime,
    lease: Duration,
    choose: C,
) -> Result<ClaimOutcome, StoreError>
where
    C: FnOnce(&ClaimView) -> Option<i64>,
{
    let Some(chosen) = choose(view) else {
        return Ok(ClaimOutcome::NothingClaimable);
    };
    let Some(seen) = view.deliveries.iter().find(|d| d.id == chosen) else {
        return Ok(ClaimOutcome::Missed);
    };
    let claim = ClaimedDelivery {
        id: chosen,
        claim_id: Uuid::now_v7(),
        attempts: seen.attempts + 1,
        lease_until: now + lease,
    };
    let (route_id, recipient_user_id) = target.columns();
    let applied = sqlx::query(CLAIM_SQL)
        .bind(chosen)
        .bind(claim.claim_id)
        .bind(claim.lease_until)
        .bind(now)
        .bind(route_id)
        .bind(seen.attempts)
        .bind(seen.claim_id)
        .bind(seen.lease_until)
        .bind(seen.next_attempt_at)
        .bind(recipient_user_id)
        .execute(&mut **tx)
        .await?
        .rows_affected();
    Ok(match applied {
        0 => ClaimOutcome::Missed,
        _ => ClaimOutcome::Claimed(claim),
    })
}

pub(super) fn pending_delivery(row: PendingRow) -> PendingDelivery {
    let (id, attempts, next_attempt_at, claim_id, lease_until, give_up_at) = row;
    PendingDelivery {
        id,
        attempts,
        next_attempt_at,
        claim_id,
        lease_until,
        give_up_at,
    }
}
