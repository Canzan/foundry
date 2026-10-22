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
//! - [`Store::create_route`]: a route writer, opening with `lock_live_project`;
//! - [`Store::upsert_connection`]: a connection and its audit record, one
//!   transaction (ADR-EIP-005 decision 4).
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
    /// The injected clock's time of the save (the audit record's date).
    pub created_at: OffsetDateTime,
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

    /// Save a route, its lane filter and its audit record (ADR-EIP-002
    /// decision 3, DDR-21), one transaction. Opens with `lock_live_project`, so
    /// a project delete in flight makes it wait and a deleted project refuses
    /// it ([`RouteWriteOutcome::ProjectDeleted`]). `receives_from` is the
    /// transaction's `now()`; the audit record is dated `created_at`.
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
        sqlx::query(
            "INSERT INTO integration_audit_events
                    (id, workspace_id, project_id, actor_id, via, action, kind, subject, created_at)
             VALUES ($1, $2, $3, $4, $5, 'route_created', $6, $7, $8)",
        )
        .bind(Uuid::now_v7())
        .bind(route.workspace_id)
        .bind(route.project_id)
        .bind(route.created_by)
        .bind(&route.created_via)
        .bind(&route.kind)
        .bind(&route.destination_label)
        .bind(route.created_at)
        .execute(&mut *tx)
        .await?;
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

/// A connection as the connect use-case saves it (ADR-EIP-005 decision 4).
/// `sealed_credential` was sealed for the row `id` (row-bound AAD), so the
/// write refuses a different row.
pub struct NewConnection {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub kind: String,
    pub external_id: String,
    pub app_id: Option<String>,
    pub bot_user_id: Option<String>,
    pub label: String,
    pub detail: Option<String>,
    pub sealed_credential: Vec<u8>,
    pub connected_by: Uuid,
    pub connected_at: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionWriteOutcome {
    /// No connection of the kind existed: one row and its audit row.
    Connected,
    /// The workspace's connection of the kind was replaced in place.
    Reconnected,
    /// The workspace's connection of the kind is another row than the one the
    /// credential was sealed for; nothing was written.
    StaleRow,
}

/// A workspace's connection as its pages show it. Never the credential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionRow {
    pub kind: String,
    pub state: String,
    pub label: String,
    pub detail: Option<String>,
    pub connected_by_name: Option<String>,
    pub connected_at: OffsetDateTime,
}

/// One integration audit record as the activity list shows it (DDR-21).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRow {
    pub created_at: OffsetDateTime,
    pub actor_name: Option<String>,
    pub action: String,
    pub kind: String,
    pub subject: String,
}

/// A route as a project's integrations page lists it, with the facts its
/// status is derived from (DDR-19).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRoute {
    pub id: Uuid,
    pub kind: String,
    pub destination_label: String,
    pub paused: bool,
    /// `integration_connections.state`; `None` for a webhook route.
    pub connection_state: Option<String>,
    pub problem_reason: Option<String>,
    /// The label at save of a filter lane the project no longer has.
    pub missing_lane: Option<String>,
    pub newest_finished: Option<FinishedDeliveryRow>,
    /// The head pending delivery, when it has failed at least once, and the
    /// vendor's latest reason for it.
    pub head_failed: Option<Option<String>>,
    pub last_delivered_at: Option<OffsetDateTime>,
}

/// The newest finished delivery of a route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinishedDeliveryRow {
    pub status: String,
    pub final_reason: Option<String>,
    pub issue_key: Option<String>,
}

/// A project's routes with their status facts, oldest first.
const PROJECT_ROUTES_SQL: &str = "
    SELECT r.id, r.kind, r.destination_label, r.paused_at IS NOT NULL AS paused,
           c.state AS connection_state, r.problem_reason,
           (SELECT l.label_at_save FROM integration_route_lanes l
             WHERE l.route_id = r.id
               AND NOT EXISTS (SELECT 1 FROM lanes x
                                WHERE x.id = l.lane_id AND x.project_id = r.project_id)
             ORDER BY l.label_at_save LIMIT 1) AS missing_lane,
           f.status AS finished_status, f.final_reason, f.issue_key,
           h.id IS NOT NULL AND (h.next_attempt_at IS NOT NULL
                                 OR h.last_vendor_reason IS NOT NULL) AS head_failed,
           h.last_vendor_reason,
           (SELECT max(d.finished_at) FROM integration_deliveries d
             WHERE d.route_id = r.id AND d.status = 'delivered') AS last_delivered_at
      FROM integration_routes r
      LEFT JOIN integration_connections c ON c.id = r.connection_id
      LEFT JOIN LATERAL (
            SELECT d.status, d.final_reason, d.issue_key FROM integration_deliveries d
             WHERE d.route_id = r.id AND d.status <> 'pending'
             ORDER BY d.finished_at DESC, d.id DESC LIMIT 1) f ON TRUE
      LEFT JOIN LATERAL (
            SELECT d.id, d.next_attempt_at, d.last_vendor_reason FROM integration_deliveries d
             WHERE d.route_id = r.id AND d.status = 'pending'
             ORDER BY d.id LIMIT 1) h ON TRUE
     WHERE r.project_id = $1
     ORDER BY r.created_at, r.id";

fn project_route(row: &sqlx::postgres::PgRow) -> Result<ProjectRoute, sqlx::Error> {
    use sqlx::Row;
    let finished_status: Option<String> = row.try_get("finished_status")?;
    let head_failed: bool = row.try_get("head_failed")?;
    let head_reason: Option<String> = row.try_get("last_vendor_reason")?;
    Ok(ProjectRoute {
        id: row.try_get("id")?,
        kind: row.try_get("kind")?,
        destination_label: row.try_get("destination_label")?,
        paused: row.try_get("paused")?,
        connection_state: row.try_get("connection_state")?,
        problem_reason: row.try_get("problem_reason")?,
        missing_lane: row.try_get("missing_lane")?,
        newest_finished: match finished_status {
            Some(status) => Some(FinishedDeliveryRow {
                status,
                final_reason: row.try_get("final_reason")?,
                issue_key: row.try_get("issue_key")?,
            }),
            None => None,
        },
        head_failed: head_failed.then_some(head_reason),
        last_delivered_at: row.try_get("last_delivered_at")?,
    })
}

/// A workspace's connection of one kind with its sealed credential, for the
/// adapters that open it (route form, Send test, route save).
#[derive(Clone, PartialEq, Eq)]
pub struct SealedConnection {
    pub id: Uuid,
    pub state: String,
    /// `None` only for a disconnected connection.
    pub sealed_credential: Option<Vec<u8>>,
}

impl std::fmt::Debug for SealedConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SealedConnection")
            .field("id", &self.id)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

/// What a sender needs to attempt one claimed delivery, read at send time
/// (ADR-EIP-002 decision 8). The event's issue facts are `None` when the issue
/// is gone; the project's when it was deleted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryContext {
    pub delivery_id: i64,
    pub event_kind: String,
    pub give_up_at: OffsetDateTime,
    pub route_kind: String,
    pub content_level: String,
    pub route_paused: bool,
    pub connection_id: Option<Uuid>,
    pub connection_state: Option<String>,
    pub destination_ref: String,
    pub project: Option<DeliveryProject>,
    pub issue: Option<DeliveryIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryProject {
    pub name: String,
    pub key_prefix: String,
    pub slug: String,
    pub team_slug: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryIssue {
    pub number: i32,
    pub title: String,
    pub lane_label: String,
    pub author_name: Option<String>,
}

const DELIVERY_CONTEXT_SQL: &str = "
    SELECT d.event_kind, d.give_up_at, r.kind, r.content_level,
           r.paused_at IS NOT NULL, r.connection_id, c.state, r.destination_ref,
           d.project_id, p.name, p.key_prefix, p.slug, t.slug,
           i.number, i.title, i.state, u.display_name
      FROM integration_deliveries d
      JOIN integration_routes r ON r.id = d.route_id
      LEFT JOIN integration_connections c ON c.id = r.connection_id
      LEFT JOIN live_projects p ON p.id = d.project_id
      LEFT JOIN teams t ON t.id = p.team_id
      LEFT JOIN issues i ON i.id = d.issue_id AND i.project_id = d.project_id
      LEFT JOIN users u ON u.id = i.author_id
     WHERE d.id = $1";

/// `workspace_connections`' row as Postgres answers it.
type ConnectionTuple = (
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    OffsetDateTime,
);

impl Store {
    /// The id of the workspace's connection of `kind`, if one exists.
    pub async fn connection_id(
        &self,
        workspace_id: Uuid,
        kind: &str,
    ) -> Result<Option<Uuid>, StoreError> {
        Ok(sqlx::query_scalar(
            "SELECT id FROM integration_connections WHERE workspace_id = $1 AND kind = $2",
        )
        .bind(workspace_id)
        .bind(kind)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Save a connection and its audit record in one transaction: insert the
    /// workspace's first connection of the kind, or replace the existing row
    /// in place when it is the row the credential was sealed for.
    pub async fn upsert_connection(
        &self,
        connection: &NewConnection,
    ) -> Result<ConnectionWriteOutcome, StoreError> {
        let mut tx = self.pool.begin().await?;
        let existing: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM integration_connections
              WHERE workspace_id = $1 AND kind = $2 FOR UPDATE",
        )
        .bind(connection.workspace_id)
        .bind(&connection.kind)
        .fetch_optional(&mut *tx)
        .await?;
        let outcome = match existing {
            Some(id) if id != connection.id => {
                tx.rollback().await?;
                return Ok(ConnectionWriteOutcome::StaleRow);
            }
            Some(_) => ConnectionWriteOutcome::Reconnected,
            None => ConnectionWriteOutcome::Connected,
        };
        sqlx::query(
            "INSERT INTO integration_connections
                    (id, workspace_id, kind, state, external_id, app_id, bot_user_id, label,
                     detail, sealed_credential, credential_saved_at, connected_by, connected_at)
             VALUES ($1, $2, $3, 'connected', $4, $5, $6, $7, $8, $9, $10, $11, $10)
             ON CONFLICT (workspace_id, kind) DO UPDATE
                SET state = 'connected', external_id = EXCLUDED.external_id,
                    app_id = EXCLUDED.app_id, bot_user_id = EXCLUDED.bot_user_id,
                    label = EXCLUDED.label, detail = EXCLUDED.detail,
                    sealed_credential = EXCLUDED.sealed_credential,
                    credential_saved_at = EXCLUDED.credential_saved_at,
                    lost_reason = NULL, lost_at = NULL,
                    connected_by = EXCLUDED.connected_by, connected_at = EXCLUDED.connected_at",
        )
        .bind(connection.id)
        .bind(connection.workspace_id)
        .bind(&connection.kind)
        .bind(&connection.external_id)
        .bind(&connection.app_id)
        .bind(&connection.bot_user_id)
        .bind(&connection.label)
        .bind(&connection.detail)
        .bind(&connection.sealed_credential)
        .bind(connection.connected_at)
        .bind(connection.connected_by)
        .execute(&mut *tx)
        .await?;
        let action = match outcome {
            ConnectionWriteOutcome::Reconnected => "connection_reconnected",
            _ => "connection_connected",
        };
        sqlx::query(
            "INSERT INTO integration_audit_events
                    (id, workspace_id, project_id, actor_id, via, action, kind, subject, created_at)
             VALUES ($1, $2, NULL, $3, 'web', $4, $5, $6, $7)",
        )
        .bind(Uuid::now_v7())
        .bind(connection.workspace_id)
        .bind(connection.connected_by)
        .bind(action)
        .bind(&connection.kind)
        .bind(&connection.label)
        .bind(connection.connected_at)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(outcome)
    }

    /// The workspace's connections, oldest kind first. Never the credential.
    pub async fn workspace_connections(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<ConnectionRow>, StoreError> {
        let rows: Vec<ConnectionTuple> = sqlx::query_as(
            "SELECT c.kind, c.state, c.label, c.detail, u.display_name, c.connected_at
                   FROM integration_connections c
                   LEFT JOIN users u ON u.id = c.connected_by
                  WHERE c.workspace_id = $1
                  ORDER BY c.kind",
        )
        .bind(workspace_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(kind, state, label, detail, connected_by_name, connected_at)| ConnectionRow {
                    kind,
                    state,
                    label,
                    detail,
                    connected_by_name,
                    connected_at,
                },
            )
            .collect())
    }

    /// The workspace's latest integration audit records, newest first.
    pub async fn integration_activity(
        &self,
        workspace_id: Uuid,
        limit: i64,
    ) -> Result<Vec<AuditRow>, StoreError> {
        let rows: Vec<(OffsetDateTime, Option<String>, String, String, String)> = sqlx::query_as(
            "SELECT a.created_at, u.display_name, a.action, a.kind, a.subject
               FROM integration_audit_events a
               LEFT JOIN users u ON u.id = a.actor_id
              WHERE a.workspace_id = $1
              ORDER BY a.created_at DESC, a.id DESC
              LIMIT $2",
        )
        .bind(workspace_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(created_at, actor_name, action, kind, subject)| AuditRow {
                created_at,
                actor_name,
                action,
                kind,
                subject,
            })
            .collect())
    }

    /// A project's routes with their status facts, oldest first.
    pub async fn project_routes(&self, project_id: Uuid) -> Result<Vec<ProjectRoute>, StoreError> {
        let rows = sqlx::query(PROJECT_ROUTES_SQL)
            .bind(project_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .iter()
            .map(project_route)
            .collect::<Result<Vec<_>, _>>()?)
    }

    /// The workspace's connection of `kind` with its sealed credential.
    pub async fn sealed_connection(
        &self,
        workspace_id: Uuid,
        kind: &str,
    ) -> Result<Option<SealedConnection>, StoreError> {
        let row: Option<(Uuid, String, Option<Vec<u8>>)> = sqlx::query_as(
            "SELECT id, state, sealed_credential FROM integration_connections
              WHERE workspace_id = $1 AND kind = $2",
        )
        .bind(workspace_id)
        .bind(kind)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(id, state, sealed_credential)| SealedConnection {
            id,
            state,
            sealed_credential,
        }))
    }

    /// The sealed credential of the connection `connection_id` (none once
    /// disconnected).
    pub async fn sealed_credential(
        &self,
        connection_id: Uuid,
    ) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(sqlx::query_scalar::<_, Option<Vec<u8>>>(
            "SELECT sealed_credential FROM integration_connections WHERE id = $1",
        )
        .bind(connection_id)
        .fetch_optional(&self.pool)
        .await?
        .flatten())
    }

    /// The routes with a pending delivery, the oldest pending first (the
    /// senders' candidates; each is claimed under its own lock).
    pub async fn routes_with_pending_deliveries(
        &self,
        limit: i64,
    ) -> Result<Vec<Uuid>, StoreError> {
        Ok(sqlx::query_scalar(
            "SELECT route_id FROM integration_deliveries
              WHERE status = 'pending' AND route_id IS NOT NULL
              GROUP BY route_id ORDER BY min(id) LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?)
    }

    /// What a sender needs to attempt `delivery_id`, read now: the route, its
    /// connection, the project (through `live_projects`) and the issue.
    pub async fn delivery_context(
        &self,
        delivery_id: i64,
    ) -> Result<Option<DeliveryContext>, StoreError> {
        use sqlx::Row;
        let Some(row) = sqlx::query(DELIVERY_CONTEXT_SQL)
            .bind(delivery_id)
            .fetch_optional(&self.pool)
            .await?
        else {
            return Ok(None);
        };
        let project_id: Uuid = row.try_get(8)?;
        let project_name: Option<String> = row.try_get(9)?;
        let key_prefix: Option<String> = row.try_get(10)?;
        let project_slug: Option<String> = row.try_get(11)?;
        let team_slug: Option<String> = row.try_get(12)?;
        let number: Option<i32> = row.try_get(13)?;
        let title: Option<String> = row.try_get(14)?;
        let state: Option<String> = row.try_get(15)?;
        let author_name: Option<String> = row.try_get(16)?;
        let project = match (project_name, key_prefix, project_slug) {
            (Some(name), Some(key_prefix), Some(slug)) => Some(DeliveryProject {
                name,
                key_prefix,
                slug,
                team_slug,
            }),
            _ => None,
        };
        let issue = match (number, title, state) {
            (Some(number), Some(title), Some(state)) => {
                let lane_label: Option<String> = sqlx::query_scalar(
                    "SELECT label FROM lanes WHERE project_id = $1 AND slug = $2",
                )
                .bind(project_id)
                .bind(&state)
                .fetch_optional(&self.pool)
                .await?;
                Some(DeliveryIssue {
                    number,
                    title,
                    lane_label: lane_label.unwrap_or(state),
                    author_name,
                })
            }
            _ => None,
        };
        Ok(Some(DeliveryContext {
            delivery_id,
            event_kind: row.try_get(0)?,
            give_up_at: row.try_get(1)?,
            route_kind: row.try_get(2)?,
            content_level: row.try_get(3)?,
            route_paused: row.try_get(4)?,
            connection_id: row.try_get(5)?,
            connection_state: row.try_get(6)?,
            destination_ref: row.try_get(7)?,
            project,
            issue,
        }))
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
