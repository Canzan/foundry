//! The expiry and retention sweeps (DDR-5, DD-10).

use super::claims::{pending_delivery, PendingDelivery, PendingRow};
use super::metrics::{delivery_tally, DeliveryTally, TallyRow, TALLY_COLUMNS};
use crate::{Store, StoreError};
use time::OffsetDateTime;

/// What the retention sweep reads (DD-10): finished deliveries old enough to
/// go, each with how many deliveries of its route (or recipient) are newer,
/// and dispatched outbox rows old enough to go, each with whether a pending
/// delivery still reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetentionView {
    pub deliveries: Vec<RetainedRow>,
    pub outbox: Vec<RetainedOutbox>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetainedRow {
    pub id: i64,
    pub finished_at: Option<OffsetDateTime>,
    pub newer_on_target: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetainedOutbox {
    pub id: i64,
    pub dispatched_at: Option<OffsetDateTime>,
    pub referenced_by_pending: bool,
}

/// The ids the retention sweep deletes, and the heartbeat cut-off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pruning {
    pub deliveries: Vec<i64>,
    pub outbox: Vec<i64>,
    pub heartbeats_before: OffsetDateTime,
}

/// How many rows one retention sweep deleted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Pruned {
    pub deliveries: u64,
    pub outbox: u64,
    pub heartbeats: u64,
}

/// The expiry sweep's compare-and-set on the plan's ids.
const EXPIRE_SQL: &str = "
    UPDATE integration_deliveries
       SET status = 'failed', final_reason = $3, finished_at = $2,
           lease_until = NULL, next_attempt_at = NULL
     WHERE id = ANY($1) AND status = 'pending' AND give_up_at <= $2
       AND (lease_until IS NULL OR lease_until <= $2)
    RETURNING";

/// The finished deliveries the retention sweep may delete, each with how
/// many deliveries of its target are newer.
const RETAINED_DELIVERIES_SQL: &str = "
    SELECT id, finished_at, newer FROM (
        SELECT id, status, finished_at,
               row_number() OVER (PARTITION BY COALESCE(route_id, recipient_user_id)
                                  ORDER BY id DESC) - 1 AS newer
          FROM integration_deliveries) ranked
     WHERE status <> 'pending' AND finished_at <= $1
     ORDER BY id";

/// The dispatched outbox rows the retention sweep may delete.
const RETAINED_OUTBOX_SQL: &str = "
    SELECT o.id, o.dispatched_at,
           EXISTS (SELECT 1 FROM integration_deliveries d
                    WHERE d.status = 'pending' AND o.id = ANY(d.outbox_ids))
      FROM outbox o
     WHERE o.dispatched_at <= $1
     ORDER BY o.id";

/// The retention sweep's compare-and-set on the plan's delivery ids: still
/// finished, and finished by `$2`.
const PRUNE_DELIVERIES_SQL: &str = "
    DELETE FROM integration_deliveries
     WHERE id = ANY($1) AND status <> 'pending' AND finished_at <= $2";

/// ... and on its outbox ids: dispatched by `$2`, and read by no pending
/// delivery.
const PRUNE_OUTBOX_SQL: &str = "
    DELETE FROM outbox o
     WHERE o.id = ANY($1) AND o.dispatched_at <= $2
       AND NOT EXISTS (SELECT 1 FROM integration_deliveries d
                        WHERE d.status = 'pending' AND o.id = ANY(d.outbox_ids))";

impl Store {
    /// The retention sweep (DD-10): reads the finished deliveries and the
    /// dispatched outbox rows at or before `read_before`, asks `plan` which to
    /// delete, and deletes exactly those ids, each only while it still
    /// qualifies (a delivery finished, an outbox row read by no pending
    /// delivery), with the heartbeats not renewed since the plan's cut-off,
    /// in one transaction.
    pub async fn prune_deliveries<P>(
        &self,
        read_before: OffsetDateTime,
        plan: P,
    ) -> Result<Pruned, StoreError>
    where
        P: FnOnce(&RetentionView) -> Pruning,
    {
        let mut tx = self.pool.begin().await?;
        let deliveries: Vec<(i64, Option<OffsetDateTime>, i64)> =
            sqlx::query_as(RETAINED_DELIVERIES_SQL)
                .bind(read_before)
                .fetch_all(&mut *tx)
                .await?;
        let outbox: Vec<(i64, Option<OffsetDateTime>, bool)> = sqlx::query_as(RETAINED_OUTBOX_SQL)
            .bind(read_before)
            .fetch_all(&mut *tx)
            .await?;
        let view = RetentionView {
            deliveries: deliveries
                .into_iter()
                .map(|(id, finished_at, newer_on_target)| RetainedRow {
                    id,
                    finished_at,
                    newer_on_target,
                })
                .collect(),
            outbox: outbox
                .into_iter()
                .map(
                    |(id, dispatched_at, referenced_by_pending)| RetainedOutbox {
                        id,
                        dispatched_at,
                        referenced_by_pending,
                    },
                )
                .collect(),
        };
        let pruning = plan(&view);
        let mut pruned = Pruned::default();
        if !pruning.deliveries.is_empty() {
            pruned.deliveries = sqlx::query(PRUNE_DELIVERIES_SQL)
                .bind(&pruning.deliveries)
                .bind(read_before)
                .execute(&mut *tx)
                .await?
                .rows_affected();
        }
        if !pruning.outbox.is_empty() {
            pruned.outbox = sqlx::query(PRUNE_OUTBOX_SQL)
                .bind(&pruning.outbox)
                .bind(read_before)
                .execute(&mut *tx)
                .await?
                .rows_affected();
        }
        pruned.heartbeats =
            sqlx::query("DELETE FROM integration_worker_heartbeats WHERE beat_at < $1")
                .bind(pruning.heartbeats_before)
                .execute(&mut *tx)
                .await?
                .rows_affected();
        tx.commit().await?;
        Ok(pruned)
    }

    /// The expiry sweep (ADR-EIP-002 decision 9, DDR-5): reads the pending
    /// deliveries at or past their give-up time at `now`, asks `plan` which to
    /// fail, and fails exactly those ids with `final_reason`, each only while
    /// it is still pending, past its give-up time and holds no live lease (a
    /// late record of a lease it lost writes nothing). Returns the tally of
    /// each delivery it failed. The vendor's last reason is kept.
    pub async fn expire_deliveries<P>(
        &self,
        now: OffsetDateTime,
        final_reason: &str,
        plan: P,
    ) -> Result<Vec<DeliveryTally>, StoreError>
    where
        P: FnOnce(&[PendingDelivery]) -> Vec<i64>,
    {
        let due: Vec<PendingRow> = sqlx::query_as(
            "SELECT id, attempts, next_attempt_at, claim_id, lease_until, give_up_at
               FROM integration_deliveries
              WHERE status = 'pending' AND give_up_at <= $1 ORDER BY id",
        )
        .bind(now)
        .fetch_all(&self.pool)
        .await?;
        let due: Vec<PendingDelivery> = due.into_iter().map(pending_delivery).collect();
        let ids = plan(&due);
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let failed: Vec<TallyRow> = sqlx::query_as(&format!("{EXPIRE_SQL}{TALLY_COLUMNS}"))
            .bind(&ids)
            .bind(now)
            .bind(final_reason)
            .fetch_all(&self.pool)
            .await?;
        Ok(failed.into_iter().map(delivery_tally).collect())
    }
}
