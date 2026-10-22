//! external-integration-plugins — the delivery queue's persistence
//! (ADR-EIP-002; DESIGN [REF] Driven Ports and Contract Shapes, [REF]
//! Persistence Model; DDR-3, DDR-4, DDR-5, DDR-17, DDR-27).
//!
//! The store applies plans; it never makes one. Each operation is one short
//! transaction that reads the facts a pure planner needs, hands them to a
//! caller-supplied closure (the planners live in `foundry-integrations`), and
//! applies the returned rows or id by compare-and-set:
//!
//! - [`Store::dispatch_tick`]: the single active dispatcher (a transaction-scoped
//!   advisory lock, scoped by `search_path`, U-D2) reads whole committed source
//!   transactions from the outbox, inserts the plan's deliveries and stamps
//!   `dispatched_at` on exactly the rows it read, in one transaction;
//! - [`Store::claim_delivery`]: a sender locks one route (`SKIP LOCKED`) and its
//!   destination pacing row, and claims the delivery the choice names only if
//!   that row is unchanged (fresh `claim_id`, `attempts + 1`, lease, pacing);
//! - [`Store::record_delivery`]: the fenced record, `WHERE claim_id = <claim>`;
//! - [`Store::create_route`]: a route writer, opening with `lock_live_project`.
//!
//! All project reads go through `live_projects`; `outbox.notified_at` is never
//! touched.

use crate::project_lifecycle::lock_live_project;
use crate::{Store, StoreError};
use std::time::Duration;
use time::OffsetDateTime;
use uuid::Uuid;

/// Advisory-lock key of the dispatcher (DDR-4), salted per `search_path`.
pub const DISPATCH_LOCK_ID: i64 = 0x_D1_5B_A7_C4_D1_5B_A7_C4_u64 as i64;

/// NFR-EIP-3: at most one attempt per destination per second.
pub const DESTINATION_SPACING: Duration = Duration::from_secs(1);

const STATUS_PENDING: &str = "pending";

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
    pub lane_ids: Vec<Uuid>,
}

/// What one dispatcher tick read: whole source transactions, in `min(id)`
/// order, and the routes of the projects they name.
#[derive(Debug, Clone, PartialEq)]
pub struct DispatchBatch {
    pub events: Vec<OutboxEvent>,
    pub routes: Vec<DispatchRoute>,
}

/// Who a delivery goes to. Slice 05 adds `Recipient` (one member by email;
/// the table's `recipient_user_id` already exists).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryTarget {
    Route(Uuid),
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

/// What a sender learned from one attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryResult {
    Delivered,
    Retry {
        next_attempt_at: OffsetDateTime,
        vendor_reason: Option<String>,
    },
    Failed {
        final_reason: String,
        vendor_reason: Option<String>,
    },
    Skipped {
        final_reason: String,
    },
}

/// The columns a record writes: status, whether it finishes the row, the next
/// attempt, the vendor's reason and the final reason.
type RecordColumns<'a> = (
    &'a str,
    bool,
    Option<OffsetDateTime>,
    Option<&'a str>,
    Option<&'a str>,
);

impl DeliveryResult {
    fn columns(&self) -> RecordColumns<'_> {
        match self {
            Self::Delivered => ("delivered", true, None, None, None),
            Self::Retry {
                next_attempt_at,
                vendor_reason,
            } => (
                STATUS_PENDING,
                false,
                Some(*next_attempt_at),
                vendor_reason.as_deref(),
                None,
            ),
            Self::Failed {
                final_reason,
                vendor_reason,
            } => (
                "failed",
                true,
                None,
                vendor_reason.as_deref(),
                Some(final_reason),
            ),
            Self::Skipped { final_reason } => ("skipped", true, None, None, Some(final_reason)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordOutcome {
    Recorded,
    /// The claim is no longer the row's (taken over, or already finished).
    LeaseLost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteLane {
    pub lane_id: Uuid,
    pub label_at_save: String,
}

/// A route as the create use-case saves it; `receives_from` is the database's
/// `now()` of the write (DDR-27).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRoute {
    pub workspace_id: Uuid,
    pub project_id: Uuid,
    pub connection_id: Option<Uuid>,
    pub kind: String,
    pub destination_ref: String,
    pub destination_label: String,
    pub events: Vec<String>,
    pub content_level: String,
    pub shared_confirmed: bool,
    pub lanes: Vec<RouteLane>,
    pub created_by: Uuid,
    pub created_via: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteWriteOutcome {
    Created(Uuid),
    /// The project is deleted (or being deleted); nothing was written.
    ProjectDeleted,
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
                    FILTER (WHERE l.lane_id IS NOT NULL), '{}')
      FROM integration_routes r
      JOIN live_projects p ON p.id = r.project_id
      LEFT JOIN integration_connections c ON c.id = r.connection_id
      LEFT JOIN integration_route_lanes l ON l.route_id = r.id
     WHERE r.project_id::text IN (
               SELECT payload->>'project_id' FROM outbox WHERE id = ANY($1))
     GROUP BY r.id, c.state
     ORDER BY r.id";

const INSERT_DELIVERY_SQL: &str = "
    INSERT INTO integration_deliveries
           (workspace_id, project_id, route_id, event_kind, event_id,
            outbox_ids, issue_id, issue_key, occurred_at, give_up_at)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9,
            GREATEST($9, now()) + make_interval(secs => $10))";

/// The claim's compare-and-set: the row the choice saw, unchanged.
const CLAIM_SQL: &str = "
    UPDATE integration_deliveries
       SET claim_id = $2, attempts = attempts + 1, lease_until = $3,
           first_attempt_at = COALESCE(first_attempt_at, $4)
     WHERE id = $1 AND route_id = $5 AND status = 'pending' AND attempts = $6
       AND claim_id IS NOT DISTINCT FROM $7
       AND lease_until IS NOT DISTINCT FROM $8
       AND next_attempt_at IS NOT DISTINCT FROM $9";

const PACE_SQL: &str = "
    INSERT INTO integration_destination_pacing (connection_id, destination_ref, not_before)
    VALUES ($1, $2, $3)
    ON CONFLICT (connection_id, destination_ref) DO UPDATE SET not_before = EXCLUDED.not_before";

/// The fenced record: only the current claim of a pending row writes.
const RECORD_SQL: &str = "
    UPDATE integration_deliveries
       SET status = $3, lease_until = NULL, next_attempt_at = $4,
           last_vendor_reason = COALESCE($5, last_vendor_reason), final_reason = $6,
           finished_at = CASE WHEN $7 THEN $8 END
     WHERE id = $1 AND claim_id = $2 AND status = 'pending'";

type RouteRow = (
    Uuid,
    Uuid,
    Uuid,
    String,
    Option<Uuid>,
    Option<String>,
    Vec<String>,
    String,
    bool,
    OffsetDateTime,
    Vec<Uuid>,
);

type EventRow = (i64, Uuid, String, String, serde_json::Value, OffsetDateTime);

type PendingRow = (
    i64,
    i32,
    Option<OffsetDateTime>,
    Option<Uuid>,
    Option<OffsetDateTime>,
    OffsetDateTime,
);

impl Store {
    /// One dispatcher tick (ADR-EIP-002 decision 2). Returns
    /// [`DispatchOutcome::Busy`] when another replica is the active dispatcher.
    /// Reads up to `max_transactions` whole source transactions, hands them and
    /// their projects' routes to `plan`, inserts the plan's deliveries with
    /// `give_up_at = max(occurred_at, now()) + give_up_after`, and marks every
    /// read row dispatched, all in one transaction; a failure changes nothing.
    pub async fn dispatch_tick<P>(
        &self,
        max_transactions: i64,
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
        let rows: Vec<EventRow> = sqlx::query_as(DISPATCH_READ_SQL)
            .bind(max_transactions)
            .fetch_all(&mut *tx)
            .await?;
        let events = rows
            .into_iter()
            .map(outbox_event)
            .collect::<Result<Vec<_>, _>>()?;
        let ids: Vec<i64> = events.iter().map(|e| e.id).collect();
        let routes: Vec<RouteRow> = sqlx::query_as(DISPATCH_ROUTES_SQL)
            .bind(&ids)
            .fetch_all(&mut *tx)
            .await?;
        let batch = DispatchBatch {
            events,
            routes: routes.into_iter().map(dispatch_route).collect(),
        };
        let deliveries = plan(&batch);
        for delivery in &deliveries {
            let DeliveryTarget::Route(route_id) = delivery.target;
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
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("UPDATE outbox SET dispatched_at = now() WHERE id = ANY($1)")
            .bind(&ids)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(DispatchOutcome::Dispatched {
            events: ids.len(),
            deliveries: deliveries.len(),
        })
    }

    /// One sender claim on `route_id` (ADR-EIP-002 decision 4). Locks the route
    /// (`SKIP LOCKED`: [`ClaimOutcome::RouteBusy`] when another sender holds it)
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
            "SELECT connection_id, destination_ref FROM integration_routes
              WHERE id = $1 FOR UPDATE SKIP LOCKED",
        )
        .bind(route_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((connection_id, destination_ref)) = route else {
            tx.rollback().await?;
            return Ok(ClaimOutcome::RouteBusy);
        };
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
        let Some(chosen) = choose(&view) else {
            tx.rollback().await?;
            return Ok(ClaimOutcome::NothingClaimable);
        };
        let Some(seen) = view.deliveries.iter().find(|d| d.id == chosen) else {
            tx.rollback().await?;
            return Ok(ClaimOutcome::Missed);
        };
        let claim = ClaimedDelivery {
            id: chosen,
            claim_id: Uuid::now_v7(),
            attempts: seen.attempts + 1,
            lease_until: now + lease,
        };
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
            .execute(&mut *tx)
            .await?
            .rows_affected();
        if applied == 0 {
            tx.rollback().await?;
            return Ok(ClaimOutcome::Missed);
        }
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

    /// The fenced record of one attempt (ADR-EIP-002 decision 4): writes only
    /// while `claim_id` is still the pending row's claim, releasing the lease.
    /// A recorder whose lease was taken over gets [`RecordOutcome::LeaseLost`]
    /// and has changed nothing.
    pub async fn record_delivery(
        &self,
        delivery_id: i64,
        claim_id: Uuid,
        now: OffsetDateTime,
        result: DeliveryResult,
    ) -> Result<RecordOutcome, StoreError> {
        let (status, finishes, next_attempt_at, vendor_reason, final_reason) = result.columns();
        let written = sqlx::query(RECORD_SQL)
            .bind(delivery_id)
            .bind(claim_id)
            .bind(status)
            .bind(next_attempt_at)
            .bind(vendor_reason)
            .bind(final_reason)
            .bind(finishes)
            .bind(now)
            .execute(&self.pool)
            .await?
            .rows_affected();
        Ok(if written == 0 {
            RecordOutcome::LeaseLost
        } else {
            RecordOutcome::Recorded
        })
    }

    /// Save a route and its lane filter (ADR-EIP-002 decision 3). Opens with
    /// `lock_live_project`, so a project delete in flight makes it wait and a
    /// deleted project refuses it ([`RouteWriteOutcome::ProjectDeleted`]).
    /// `receives_from` is the transaction's `now()`.
    pub async fn create_route(&self, route: &NewRoute) -> Result<RouteWriteOutcome, StoreError> {
        let mut tx = self.pool.begin().await?;
        if !lock_live_project(&mut tx, route.project_id).await? {
            tx.rollback().await?;
            return Ok(RouteWriteOutcome::ProjectDeleted);
        }
        let id = Uuid::now_v7();
        sqlx::query(
            "INSERT INTO integration_routes
                    (id, workspace_id, project_id, connection_id, kind, destination_ref,
                     destination_label, events, content_level, shared_confirmed,
                     receives_from, created_by, created_via)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, now(), $11, $12)",
        )
        .bind(id)
        .bind(route.workspace_id)
        .bind(route.project_id)
        .bind(route.connection_id)
        .bind(&route.kind)
        .bind(&route.destination_ref)
        .bind(&route.destination_label)
        .bind(&route.events)
        .bind(&route.content_level)
        .bind(route.shared_confirmed)
        .bind(route.created_by)
        .bind(&route.created_via)
        .execute(&mut *tx)
        .await?;
        for lane in &route.lanes {
            sqlx::query(
                "INSERT INTO integration_route_lanes (route_id, lane_id, label_at_save)
                 VALUES ($1, $2, $3)",
            )
            .bind(id)
            .bind(lane.lane_id)
            .bind(&lane.label_at_save)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(RouteWriteOutcome::Created(id))
    }

    /// U-D7: every INVALID index in the active schema (a failed
    /// `CREATE INDEX CONCURRENTLY`, e.g. 0029's), as one boot warning each,
    /// naming the remedy (runbook R-7). `Store::migrate` logs them.
    pub async fn invalid_index_warnings(&self) -> Result<Vec<String>, StoreError> {
        let names: Vec<String> = sqlx::query_scalar(
            "SELECT c.relname::text FROM pg_index i
               JOIN pg_class c ON c.oid = i.indexrelid
               JOIN pg_namespace n ON n.oid = c.relnamespace
              WHERE n.nspname = current_schema() AND NOT i.indisvalid
              ORDER BY 1",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(names
            .into_iter()
            .map(|name| {
                format!(
                    "store.index.invalid index={name}: a CREATE INDEX CONCURRENTLY failed and \
                     left it INVALID, and the migration that creates it cannot apply over it; \
                     run DROP INDEX CONCURRENTLY {name}; then restart (runbook R-7)"
                )
            })
            .collect())
    }
}

/// The probe's view of 0028-0030 in the active schema: the outbox dispatch
/// columns, the dispatch start table, the 0029 index, and the eight 0030 tables.
pub(crate) const PROBE_SQL: &str = "
    SELECT (SELECT count(*) FROM information_schema.columns
             WHERE table_schema = current_schema() AND table_name = 'outbox'
               AND column_name IN ('event_id', 'source_txid', 'dispatched_at'))
         + (SELECT count(*) FROM information_schema.tables
             WHERE table_schema = current_schema()
               AND table_name = 'integration_dispatch_start'),
           EXISTS (SELECT 1 FROM pg_class c JOIN pg_namespace n ON n.oid = c.relnamespace
                    WHERE n.nspname = current_schema() AND c.relkind = 'i'
                      AND c.relname = 'idx_outbox_undispatched'),
           (SELECT count(*) FROM information_schema.tables
             WHERE table_schema = current_schema()
               AND table_name IN ('integration_worker_heartbeats', 'integration_connections',
                                  'integration_routes', 'integration_destination_pacing',
                                  'integration_route_lanes', 'integration_deliveries',
                                  'integration_audit_events', 'integration_key_checks'))";

/// The 0028 objects the probe counts (three columns and the start table).
pub(crate) const PROBE_0028_OBJECTS: i64 = 4;
/// The 0030 tables the probe counts.
pub(crate) const PROBE_0030_TABLES: i64 = 8;

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
    ) = row;
    DispatchRoute {
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
    }
}

fn pending_delivery(row: PendingRow) -> PendingDelivery {
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
