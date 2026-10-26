//! The dispatcher: one tick reads whole committed source transactions from
//! the outbox and inserts the plan's deliveries (ADR-EIP-002 decision 2, DDR-4,
//! DDR-17).

use super::StoredKind;
use crate::{Store, StoreError};
use std::time::Duration;
use time::OffsetDateTime;
use uuid::Uuid;

/// Advisory-lock key of the dispatcher (DDR-4), salted per `search_path`.
pub const DISPATCH_LOCK_ID: i64 = 0x_D1_5B_A7_C4_D1_5B_A7_C4_u64 as i64;

/// One committed, undispatched outbox row with an event id.
#[derive(Debug, Clone, PartialEq)]
pub struct OutboxEvent {
    pub id: i64,
    pub event_id: Uuid,
    /// The writing transaction (`pg_current_xact_id()`); one board action.
    pub source_txid: u64,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub created_at: OffsetDateTime,
}

/// A route of a live project that an event of the batch may match.
#[derive(Debug, Clone, PartialEq)]
pub struct DispatchRoute {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub project_id: Uuid,
    pub kind: String,
    pub connection_id: Option<Uuid>,
    /// `integration_connections.state`; `None` for a webhook route.
    pub connection_state: Option<String>,
    pub events: Vec<String>,
    pub content_level: String,
    pub paused: bool,
    pub receives_from: OffsetDateTime,
    /// The route's filter lanes, by lane id order.
    pub lanes: Vec<DispatchLane>,
}

/// A filter lane of a route: its id and its slug now, `None` when the
/// project no longer has it (no FK to `lanes`, on purpose).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchLane {
    pub id: Uuid,
    pub slug: Option<String>,
}

/// What one dispatcher tick read: whole source transactions, in `min(id)`
/// order, and the routes of the projects they name.
#[derive(Debug, Clone, PartialEq)]
pub struct DispatchBatch {
    pub events: Vec<OutboxEvent>,
    pub routes: Vec<DispatchRoute>,
    pub email_choices: Vec<DispatchEmailChoice>,
}

/// Who a delivery goes to: a route's destination, or one member by email
/// (slice 05, `recipient_user_id`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryTarget {
    Route(Uuid),
    Recipient(Uuid),
}

impl DeliveryTarget {
    /// The row's `route_id` and `recipient_user_id`: exactly one is set.
    pub(super) fn columns(self) -> (Option<Uuid>, Option<Uuid>) {
        match self {
            DeliveryTarget::Route(route_id) => (Some(route_id), None),
            DeliveryTarget::Recipient(user_id) => (None, Some(user_id)),
        }
    }
}

/// A member's email choice in a live project an event of the batch names
/// (ADR-EIP-007 decision 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchEmailChoice {
    pub user_id: Uuid,
    pub workspace_id: Uuid,
    pub project_id: Uuid,
    pub event: String,
    pub scope: String,
}

/// One `integration_deliveries` row the dispatch plan asks for. The store adds
/// `give_up_at = max(occurred_at, dispatched_at) + window` (DDR-5).
#[derive(Debug, Clone, PartialEq)]
pub struct NewDelivery {
    pub workspace_id: Uuid,
    pub project_id: Uuid,
    pub target: DeliveryTarget,
    pub event_kind: String,
    pub event_id: Uuid,
    pub outbox_ids: Vec<i64>,
    pub issue_id: Option<Uuid>,
    pub issue_key: Option<String>,
    pub occurred_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchOutcome {
    /// Another replica holds the dispatcher lock; nothing was read.
    Busy,
    /// `events` outbox rows were marked dispatched and `deliveries` rows written.
    Dispatched { events: usize, deliveries: usize },
}

/// Whole source transactions, oldest first, never split: up to `$1`
/// `source_txid` values in `min(id)` order, then every undispatched row of
/// them. Rows at or below the dispatch start, or without an event id, are
/// never read (DDR-17).
const DISPATCH_READ_SQL: &str = "
    WITH start AS (SELECT outbox_id FROM integration_dispatch_start),
    eligible AS (
        SELECT o.* FROM outbox o, start
         WHERE o.dispatched_at IS NULL AND o.event_id IS NOT NULL
           AND o.source_txid IS NOT NULL AND o.id > start.outbox_id
    ),
    txs AS (
        SELECT source_txid FROM eligible GROUP BY source_txid ORDER BY min(id) LIMIT $1
    )
    SELECT id, event_id, source_txid::text, event_type, payload, created_at
      FROM eligible WHERE source_txid IN (SELECT source_txid FROM txs)
     ORDER BY id";

const DISPATCH_ROUTES_SQL: &str = "
    SELECT r.id, r.workspace_id, r.project_id, r.kind, r.connection_id, c.state,
           r.events, r.content_level, r.paused_at IS NOT NULL, r.receives_from,
           COALESCE(array_agg(l.lane_id ORDER BY l.lane_id)
                    FILTER (WHERE l.lane_id IS NOT NULL), '{}'),
           COALESCE(array_agg(x.slug ORDER BY l.lane_id)
                    FILTER (WHERE l.lane_id IS NOT NULL), '{}')
      FROM integration_routes r
      JOIN live_projects p ON p.id = r.project_id
      LEFT JOIN integration_connections c ON c.id = r.connection_id
      LEFT JOIN integration_route_lanes l ON l.route_id = r.id
      LEFT JOIN lanes x ON x.id = l.lane_id AND x.project_id = r.project_id
     WHERE r.project_id IN (
               SELECT (payload->>'project_id')::uuid FROM outbox WHERE id = ANY($1))
     GROUP BY r.id, c.state
     ORDER BY r.id";

/// The email choices of the batch's live projects, by member.
const DISPATCH_EMAIL_CHOICES_SQL: &str = "
    SELECT ch.user_id, p.workspace_id, ch.project_id, ch.event, ch.scope
      FROM integration_email_choices ch
      JOIN live_projects p ON p.id = ch.project_id
     WHERE ch.project_id IN (
               SELECT (payload->>'project_id')::uuid FROM outbox WHERE id = ANY($1))
     ORDER BY ch.user_id, ch.project_id, ch.event";

const INSERT_DELIVERY_SQL: &str = "
    INSERT INTO integration_deliveries
           (workspace_id, project_id, route_id, recipient_user_id, event_kind, event_id,
            outbox_ids, issue_id, issue_key, occurred_at, give_up_at)
    VALUES ($1, $2, $3, $12, $4, $5, $6, $7, $8, $9, $11 + make_interval(secs => $10))";

type RouteRow = (
    Uuid,
    Uuid,
    Uuid,
    StoredKind,
    Option<Uuid>,
    Option<String>,
    Vec<String>,
    String,
    bool,
    OffsetDateTime,
    Vec<Uuid>,
    Vec<Option<String>>,
);

type EventRow = (i64, Uuid, String, String, serde_json::Value, OffsetDateTime);

impl Store {
    /// One dispatcher tick (ADR-EIP-002 decision 2). Returns
    /// [`DispatchOutcome::Busy`] when another replica is the active dispatcher.
    /// Reads up to `max_transactions` whole source transactions, hands them and
    /// their projects' routes to `plan`, inserts the plan's deliveries with
    /// `give_up_at = dispatched_at + give_up_after`, and marks every read row
    /// dispatched at `dispatched_at`, all in one transaction; a failure changes
    /// nothing. `dispatched_at` is the injected clock's now: a row is read only
    /// after it committed, so it is never before the row's `occurred_at` and
    /// is DDR-5's `max(occurred_at, dispatched_at)` on that one clock.
    pub async fn dispatch_tick<P>(
        &self,
        max_transactions: i64,
        dispatched_at: OffsetDateTime,
        give_up_after: Duration,
        plan: P,
    ) -> Result<DispatchOutcome, StoreError>
    where
        P: FnOnce(&DispatchBatch) -> Vec<NewDelivery>,
    {
        let mut tx = self.pool.begin().await?;
        let lock_id = scoped_lock_id(&mut tx, DISPATCH_LOCK_ID).await?;
        let (locked,): (bool,) = sqlx::query_as("SELECT pg_try_advisory_xact_lock($1)")
            .bind(lock_id)
            .fetch_one(&mut *tx)
            .await?;
        if !locked {
            tx.rollback().await?;
            return Ok(DispatchOutcome::Busy);
        }
        let batch = read_dispatch_batch(&mut tx, max_transactions).await?;
        let ids: Vec<i64> = batch.events.iter().map(|e| e.id).collect();
        let deliveries = plan(&batch);
        for delivery in &deliveries {
            insert_delivery(&mut tx, delivery, give_up_after, dispatched_at).await?;
        }
        sqlx::query("UPDATE outbox SET dispatched_at = $2 WHERE id = ANY($1)")
            .bind(&ids)
            .bind(dispatched_at)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(DispatchOutcome::Dispatched {
            events: ids.len(),
            deliveries: deliveries.len(),
        })
    }
}

/// The dispatcher's (or a sweep's) advisory-lock id for the transaction's
/// `search_path`: the canonical `base` for the production schema, a salted
/// FNV-1a of the path otherwise, so per-schema test runs never contend (U-D2).
async fn scoped_lock_id(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    base: i64,
) -> Result<i64, sqlx::Error> {
    let (path,): (String,) = sqlx::query_as("SHOW search_path")
        .fetch_one(&mut **tx)
        .await?;
    Ok(scope_lock_id(path.trim(), base))
}

fn scope_lock_id(path: &str, base: i64) -> i64 {
    if matches!(path, "" | "\"$user\", public" | "public" | "\"$user\"") {
        return base;
    }
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in path.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    (hash as i64) ^ base
}

/// What one dispatcher tick reads, in its transaction: up to
/// `max_transactions` whole source transactions, and the routes and email
/// choices of the projects they name.
async fn read_dispatch_batch(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    max_transactions: i64,
) -> Result<DispatchBatch, StoreError> {
    let rows: Vec<EventRow> = sqlx::query_as(DISPATCH_READ_SQL)
        .bind(max_transactions)
        .fetch_all(&mut **tx)
        .await?;
    let events = rows
        .into_iter()
        .map(outbox_event)
        .collect::<Result<Vec<_>, _>>()?;
    if events.is_empty() {
        // An idle tick: no project to read routes or choices for.
        return Ok(DispatchBatch {
            events,
            routes: Vec::new(),
            email_choices: Vec::new(),
        });
    }
    let ids: Vec<i64> = events.iter().map(|e| e.id).collect();
    let routes: Vec<RouteRow> = sqlx::query_as(DISPATCH_ROUTES_SQL)
        .bind(&ids)
        .fetch_all(&mut **tx)
        .await?;
    let email_choices: Vec<(Uuid, Uuid, Uuid, String, String)> =
        sqlx::query_as(DISPATCH_EMAIL_CHOICES_SQL)
            .bind(&ids)
            .fetch_all(&mut **tx)
            .await?;
    Ok(DispatchBatch {
        events,
        routes: routes.into_iter().map(dispatch_route).collect(),
        email_choices: email_choices
            .into_iter()
            .map(
                |(user_id, workspace_id, project_id, event, scope)| DispatchEmailChoice {
                    user_id,
                    workspace_id,
                    project_id,
                    event,
                    scope,
                },
            )
            .collect(),
    })
}

/// One planned delivery, queued with `give_up_at = dispatched_at +
/// give_up_after`.
async fn insert_delivery(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    delivery: &NewDelivery,
    give_up_after: Duration,
    dispatched_at: OffsetDateTime,
) -> Result<(), StoreError> {
    let (route_id, recipient_user_id) = delivery.target.columns();
    sqlx::query(INSERT_DELIVERY_SQL)
        .bind(delivery.workspace_id)
        .bind(delivery.project_id)
        .bind(route_id)
        .bind(&delivery.event_kind)
        .bind(delivery.event_id)
        .bind(&delivery.outbox_ids)
        .bind(delivery.issue_id)
        .bind(&delivery.issue_key)
        .bind(delivery.occurred_at)
        .bind(give_up_after.as_secs_f64())
        .bind(dispatched_at)
        .bind(recipient_user_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

fn outbox_event(row: EventRow) -> Result<OutboxEvent, StoreError> {
    let (id, event_id, txid, event_type, payload, created_at) = row;
    let source_txid = txid
        .parse()
        .map_err(|_| StoreError::Sqlx(sqlx::Error::Decode(format!("xid8 {txid}").into())))?;
    Ok(OutboxEvent {
        id,
        event_id,
        source_txid,
        event_type,
        payload,
        created_at,
    })
}

fn dispatch_route(row: RouteRow) -> DispatchRoute {
    let (
        id,
        workspace_id,
        project_id,
        kind,
        connection_id,
        connection_state,
        events,
        content_level,
        paused,
        receives_from,
        lane_ids,
        lane_slugs,
    ) = row;
    DispatchRoute {
        id,
        workspace_id,
        project_id,
        kind: kind.wire(),
        connection_id,
        connection_state,
        events,
        content_level,
        paused,
        receives_from,
        lanes: lane_ids
            .into_iter()
            .zip(lane_slugs)
            .map(|(id, slug)| DispatchLane { id, slug })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_production_schema_keeps_the_canonical_lock_id() {
        for path in ["", "\"$user\", public", "public", "\"$user\""] {
            assert_eq!(scope_lock_id(path, DISPATCH_LOCK_ID), DISPATCH_LOCK_ID);
        }
    }

    #[test]
    fn each_schema_gets_its_own_stable_lock_id() {
        let a = scope_lock_id("test_a", DISPATCH_LOCK_ID);
        let b = scope_lock_id("test_b", DISPATCH_LOCK_ID);
        assert_ne!(a, DISPATCH_LOCK_ID);
        assert_ne!(a, b);
        assert_eq!(a, scope_lock_id("test_a", DISPATCH_LOCK_ID));
        assert_ne!(a, scope_lock_id("test_a", 0));
    }

    /// Two releases' dispatchers must contend during a rolling upgrade, so the
    /// derivation is a cross-release contract: pinned, never re-derived.
    #[test]
    fn the_lock_id_is_stable_across_releases() {
        assert_eq!(
            scope_lock_id("eip", DISPATCH_LOCK_ID),
            1_406_450_040_211_198_821
        );
    }
}
