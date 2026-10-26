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
//!   a refused connection turns `connected → lost` there, once;
//! - [`Store::slack_app_lost`]: Slack's signed word that the Foundry app left a
//!   Slack workspace, the same `connected → lost` compare-and-set;
//! - [`Store::expire_deliveries`]: the expiry sweep, failing exactly the ids
//!   the plan names, each still pending, past its give-up time and unleased;
//! - [`Store::prune_deliveries`]: the retention sweep (DD-10), deleting exactly
//!   the finished deliveries and unreferenced dispatched outbox rows the plan
//!   names;
//! - [`Store::beat_worker`] and [`Store::integration_backlog`]: the stall
//!   signals (DDR-22);
//! - [`Store::create_route`], [`Store::pause_route`], [`Store::resume_route`],
//!   [`Store::edit_route`] and [`Store::delete_route`]: the route writers,
//!   each opening with `lock_live_project` and writing its audit record in
//!   the same transaction; a pause skips exactly the ids its cancel plan
//!   names, each still pending and holding no live lease (DD-8);
//! - [`Store::upsert_connection`]: a connection and its audit record, one
//!   transaction (ADR-EIP-005 decision 4); after a disconnect its routes
//!   receive from the reconnect on (R-9);
//! - [`Store::claim_email_delivery`] and [`Store::email_send_facts`]: a
//!   member's project emails, claimed under the member's lock by the same
//!   compare-and-set, and what their send-time checks read (ADR-EIP-007);
//! - [`Store::save_email_choices`]: a member's choices in one project,
//!   opening with `lock_live_project`;
//! - [`Store::disconnect_connection`]: a disconnect skips exactly the ids its
//!   cancel plan names across the connection's routes, forgets the credential
//!   and writes its audit record, one transaction (US-EIP-07).
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

/// A stored integration kind, read closed at the store boundary: a kind this
/// Foundry does not know (a newer release's, a hand edit's) is a decode
/// error, never a string handed on for a caller to guess at. The kinds are
/// `foundry_integrations::ChannelKind`'s, which the store may not name
/// (deny.toml rule 4); readers keep handing on their wire names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StoredKind {
    Slack,
    Matrix,
    Email,
    Webhook,
}

impl StoredKind {
    const ALL: [StoredKind; 4] = [Self::Slack, Self::Matrix, Self::Email, Self::Webhook];

    fn wire_name(self) -> &'static str {
        match self {
            Self::Slack => "slack",
            Self::Matrix => "matrix",
            Self::Email => "email",
            Self::Webhook => "webhook",
        }
    }

    /// The kind's wire name, as the row structs carry it.
    fn wire(self) -> String {
        self.wire_name().to_string()
    }
}

impl sqlx::Type<sqlx::Postgres> for StoredKind {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        <&str as sqlx::Type<sqlx::Postgres>>::type_info()
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Postgres> for StoredKind {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let wire = <&str as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        Self::ALL
            .into_iter()
            .find(|kind| kind.wire_name() == wire)
            .ok_or_else(|| format!("unknown integration kind {wire:?}").into())
    }
}

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
    fn columns(self) -> (Option<Uuid>, Option<Uuid>) {
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
    /// The vendor asked Foundry to wait: retry at `next_attempt_at`, and send
    /// nothing to the destination before `not_before` (its "retry after").
    SlowedDown {
        next_attempt_at: OffsetDateTime,
        not_before: Option<OffsetDateTime>,
        vendor_reason: String,
    },
    /// The vendor refused the destination: the delivery waits, unscheduled,
    /// and its route gets `vendor_reason` as its problem.
    RouteRefused {
        vendor_reason: String,
    },
    /// The vendor refused the connection: the delivery waits, unscheduled,
    /// and a connected connection becomes lost for `vendor_reason`.
    ConnectionRefused {
        vendor_reason: String,
    },
}

/// The columns a record writes.
struct RecordColumns<'a> {
    status: &'a str,
    /// The record finishes the row (`finished_at`).
    finishes: bool,
    next_attempt_at: Option<OffsetDateTime>,
    vendor_reason: Option<&'a str>,
    final_reason: Option<&'a str>,
    /// The vendor asked Foundry to slow down.
    slowed_down: bool,
}

impl<'a> RecordColumns<'a> {
    /// A finished row: `status`, for `final_reason` if any.
    fn finished(status: &'a str, final_reason: Option<&'a str>) -> Self {
        Self {
            status,
            finishes: true,
            next_attempt_at: None,
            vendor_reason: None,
            final_reason,
            slowed_down: false,
        }
    }

    /// A row left pending, to be tried at `next_attempt_at` (unscheduled when
    /// `None`).
    fn pending(next_attempt_at: Option<OffsetDateTime>, vendor_reason: Option<&'a str>) -> Self {
        Self {
            status: STATUS_PENDING,
            finishes: false,
            next_attempt_at,
            vendor_reason,
            final_reason: None,
            slowed_down: false,
        }
    }
}

impl DeliveryResult {
    fn columns(&self) -> RecordColumns<'_> {
        match self {
            Self::Delivered => RecordColumns::finished("delivered", None),
            Self::Retry {
                next_attempt_at,
                vendor_reason,
            } => RecordColumns::pending(Some(*next_attempt_at), vendor_reason.as_deref()),
            Self::SlowedDown {
                next_attempt_at,
                vendor_reason,
                ..
            } => RecordColumns {
                slowed_down: true,
                ..RecordColumns::pending(Some(*next_attempt_at), Some(vendor_reason))
            },
            Self::RouteRefused { vendor_reason } | Self::ConnectionRefused { vendor_reason } => {
                RecordColumns::pending(None, Some(vendor_reason))
            }
            Self::Failed {
                final_reason,
                vendor_reason,
            } => RecordColumns {
                vendor_reason: vendor_reason.as_deref(),
                ..RecordColumns::finished("failed", Some(final_reason))
            },
            Self::Skipped { final_reason } => {
                RecordColumns::finished("skipped", Some(final_reason))
            }
        }
    }
}

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

/// What the connection and route gauges read (U-D8): `(kind, state, count)`
/// rows; a route's state is `active` or `paused`, and routes of deleted
/// projects are not counted.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IntegrationCensus {
    pub connections: Vec<(String, String, i64)>,
    pub routes: Vec<(String, String, i64)>,
}

/// A delivery as the delivery counters count it (DDR-22), read in the same
/// statement that recorded or failed it: its kind (`email` for a recipient's),
/// its event, its status after the write, and its lag `now() - occurred_at`
/// on the database's clock, in seconds.
#[derive(Debug, Clone, PartialEq)]
pub struct DeliveryTally {
    pub kind: String,
    pub event_kind: String,
    pub status: String,
    pub lag_seconds: f64,
}

/// What the stall gauges read (DDR-22). Ages are seconds, 0 when none.
#[derive(Debug, Clone, PartialEq)]
pub struct IntegrationBacklog {
    pub undispatched: i64,
    pub undispatched_oldest_seconds: f64,
    /// Pending deliveries per kind (`slack`, `matrix`, `email`); a kind with
    /// none is absent.
    pub pending_by_kind: Vec<(String, i64)>,
    pub pending_oldest_seconds: f64,
}

/// A connection that has just turned `connected → lost`: what its
/// workspace's admins are told, and their addresses (DDR-23).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LostConnection {
    pub workspace_id: Uuid,
    pub workspace_name: String,
    pub kind: String,
    pub reason: Option<String>,
    pub admins: Vec<String>,
}

/// The rest of a `connected → lost` transition, in its transaction: the
/// audit record, and what the admins are told.
async fn connection_lost(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    connection_id: Uuid,
    via: &str,
    now: OffsetDateTime,
) -> Result<LostConnection, StoreError> {
    let (workspace_id,): (Uuid,) = sqlx::query_as(CONNECTION_LOST_AUDIT_SQL)
        .bind(Uuid::now_v7())
        .bind(via)
        .bind(now)
        .bind(connection_id)
        .fetch_one(&mut **tx)
        .await?;
    let (workspace_name, kind, reason): (String, StoredKind, Option<String>) =
        sqlx::query_as(LOST_CONNECTION_SQL)
            .bind(connection_id)
            .fetch_one(&mut **tx)
            .await?;
    let admins: Vec<(String,)> = sqlx::query_as(WORKSPACE_ADMINS_SQL)
        .bind(workspace_id)
        .fetch_all(&mut **tx)
        .await?;
    Ok(LostConnection {
        workspace_id,
        workspace_name,
        kind: kind.wire(),
        reason,
        admins: admins.into_iter().map(|(email,)| email).collect(),
    })
}

#[derive(Debug, Clone, PartialEq)]
pub enum RecordOutcome {
    Recorded(DeliveryTally),
    /// Recorded, and this record's refusal turned the route's connection
    /// from connected to lost: its admins are to be told, once.
    ConnectionLost {
        tally: DeliveryTally,
        lost: LostConnection,
    },
    /// The claim is no longer the row's (taken over, or already finished).
    LeaseLost,
}

impl RecordOutcome {
    /// What the record counts, unless it wrote nothing.
    pub fn tally(&self) -> Option<&DeliveryTally> {
        match self {
            Self::Recorded(tally) | Self::ConnectionLost { tally, .. } => Some(tally),
            Self::LeaseLost => None,
        }
    }
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

/// Who changes a route, and when (the injected clock's time: the audit
/// record's date, and the "now" a lease is judged against).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteChange {
    pub project_id: Uuid,
    pub route_id: Uuid,
    pub actor_id: Uuid,
    pub via: String,
    pub at: OffsetDateTime,
}

/// What a route's edit sets (its destination stays).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteEdit {
    pub events: Vec<String>,
    pub content_level: String,
    pub lanes: Vec<RouteLane>,
}

/// One delivery of a route as a pause's cancel plan sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CancelCandidate {
    pub id: i64,
    pub route_id: Uuid,
    pub finished: bool,
    pub lease_until: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteChangeOutcome {
    /// The route changed and its audit record was written; a pause also
    /// skipped `cancelled` deliveries.
    Changed { cancelled: u64 },
    /// The route already was so (paused, live, gone): nothing was written.
    Unchanged,
    /// The project is deleted (or being deleted); nothing was written.
    ProjectDeleted,
}

/// Who changes a workspace's connection of `kind`, and when (the injected
/// clock's time: the audit record's date, and the "now" a lease is judged
/// against).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionChange {
    pub workspace_id: Uuid,
    pub kind: String,
    pub actor_id: Uuid,
    pub via: String,
    pub at: OffsetDateTime,
}

/// What a disconnect stops (US-EIP-07): the connection's label and its
/// routes that post now (not paused, in a live project), and their projects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisconnectPreview {
    pub label: String,
    pub routes: i64,
    pub projects: i64,
}

/// What a disconnect did.
#[derive(Clone, PartialEq, Eq)]
pub enum DisconnectOutcome {
    /// The connection is disconnected and holds no credential; the credential
    /// it held, still sealed for the row `connection_id`, is handed back once
    /// so the vendor can be asked to withdraw it.
    Disconnected {
        connection_id: Uuid,
        sealed_credential: Vec<u8>,
        cancelled: u64,
    },
    /// No connection of the kind, or it already was disconnected.
    Unchanged,
}

impl std::fmt::Debug for DisconnectOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disconnected {
                connection_id,
                cancelled,
                ..
            } => f
                .debug_struct("Disconnected")
                .field("connection_id", connection_id)
                .field("cancelled", cancelled)
                .finish_non_exhaustive(),
            Self::Unchanged => f.write_str("Unchanged"),
        }
    }
}

/// A disconnect's compare-and-set on the plan's ids: still a pending
/// delivery of one of the connection's routes, holding no live lease at `$3`.
const DISCONNECT_CANCEL_SQL: &str = "
    UPDATE integration_deliveries d
       SET status = 'skipped', final_reason = 'route_paused', finished_at = $3,
           next_attempt_at = NULL, lease_until = NULL
      FROM integration_routes r
     WHERE d.id = ANY($1) AND r.id = d.route_id AND r.connection_id = $2
       AND d.status = 'pending' AND (d.lease_until IS NULL OR d.lease_until <= $3)";

/// The routes a disconnect stops: not paused, in a live project.
const DISCONNECT_PREVIEW_SQL: &str = "
    SELECT c.label, count(r.id), count(DISTINCT r.project_id)
      FROM integration_connections c
      LEFT JOIN integration_routes r ON r.connection_id = c.id AND r.paused_at IS NULL
            AND EXISTS (SELECT 1 FROM live_projects p WHERE p.id = r.project_id)
     WHERE c.workspace_id = $1 AND c.kind = $2 AND c.state <> 'disconnected'
     GROUP BY c.id, c.label";

/// A pause's compare-and-set on the plan's ids: still the route's, pending,
/// and holding no live lease at `$3`.
const CANCEL_SQL: &str = "
    UPDATE integration_deliveries
       SET status = 'skipped', final_reason = $4, finished_at = $3,
           next_attempt_at = NULL, lease_until = NULL
     WHERE id = ANY($1) AND route_id = $2 AND status = 'pending'
       AND (lease_until IS NULL OR lease_until <= $3)";

/// A route's audit record (DDR-21), its subject the route's destination.
const ROUTE_AUDIT_SQL: &str = "
    INSERT INTO integration_audit_events
           (id, workspace_id, project_id, actor_id, via, action, kind, subject, created_at)
    SELECT $1, r.workspace_id, r.project_id, $2, $3, $4, r.kind, r.destination_label, $5
      FROM integration_routes r WHERE r.id = $6";

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

/// What a write of a delivery row returns for the delivery counters.
const TALLY_COLUMNS: &str = "
       COALESCE((SELECT r.kind FROM integration_routes r
                  WHERE r.id = integration_deliveries.route_id), 'email'),
       event_kind, status, EXTRACT(EPOCH FROM now() - occurred_at)::float8";

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

/// The expiry sweep's compare-and-set on the plan's ids.
const EXPIRE_SQL: &str = "
    UPDATE integration_deliveries
       SET status = 'failed', final_reason = $3, finished_at = $2,
           lease_until = NULL, next_attempt_at = NULL
     WHERE id = ANY($1) AND status = 'pending' AND give_up_at <= $2
       AND (lease_until IS NULL OR lease_until <= $2)
    RETURNING";

/// A recorded row: its route and project, then its tally.
type RecordedRow = (Option<Uuid>, Uuid, String, String, String, f64);

/// [`TALLY_COLUMNS`] as Postgres answers them.
type TallyRow = (String, String, String, f64);

/// The fenced record: only the current claim of a pending row writes.
const RECORD_SQL: &str = "
    UPDATE integration_deliveries
       SET status = $3, lease_until = NULL, next_attempt_at = $4,
           last_vendor_reason = COALESCE($5, last_vendor_reason), final_reason = $6,
           finished_at = CASE WHEN $7 THEN $8 END,
           slowed_down = slowed_down OR $9
     WHERE id = $1 AND claim_id = $2 AND status = 'pending'
    RETURNING route_id, project_id,";

/// A "retry after": the route's destination waits at least until $2.
const SLOW_DOWN_SQL: &str = "
    INSERT INTO integration_destination_pacing (connection_id, destination_ref, not_before)
    SELECT r.connection_id, r.destination_ref, $2 FROM integration_routes r
     WHERE r.id = $1 AND r.connection_id IS NOT NULL
    ON CONFLICT (connection_id, destination_ref)
    DO UPDATE SET not_before = GREATEST(integration_destination_pacing.not_before,
                                        EXCLUDED.not_before)";

/// A refused destination: the route needs attention (kept if already set).
const ROUTE_PROBLEM_SQL: &str = "
    UPDATE integration_routes SET problem_reason = $2, problem_at = $3
     WHERE id = $1 AND problem_reason IS NULL";

/// A refused connection: `connected → lost`, once (compare-and-set).
const CONNECTION_LOST_SQL: &str = "
    UPDATE integration_connections c SET state = 'lost', lost_reason = $2, lost_at = $3
      FROM integration_routes r
     WHERE r.id = $1 AND c.id = r.connection_id AND c.state = 'connected'
    RETURNING c.id";

/// Slack's word that the Foundry app left a Slack workspace: that
/// workspace's connection of this app, `connected → lost`, once.
const SLACK_APP_LOST_SQL: &str = "
    UPDATE integration_connections SET state = 'lost', lost_reason = $3, lost_at = $4
     WHERE kind = 'slack' AND external_id = $1 AND app_id = $2 AND state = 'connected'
    RETURNING id";

/// The audit record of a connection that was lost (DDR-21), by Foundry.
const CONNECTION_LOST_AUDIT_SQL: &str = "
    INSERT INTO integration_audit_events
           (id, workspace_id, project_id, actor_id, via, action, kind, subject, created_at)
    SELECT $1, c.workspace_id, NULL, NULL, $2, 'connection_lost', c.kind, c.label, $3
      FROM integration_connections c WHERE c.id = $4
    RETURNING workspace_id";

/// What a lost connection's admins are told: the workspace's name, the
/// connection's kind and why it was lost.
const LOST_CONNECTION_SQL: &str = "
    SELECT w.name, c.kind, c.lost_reason
      FROM integration_connections c JOIN workspaces w ON w.id = c.workspace_id
     WHERE c.id = $1";

/// A workspace's admins' addresses.
const WORKSPACE_ADMINS_SQL: &str = "
    SELECT u.email_display FROM workspace_memberships m JOIN users u ON u.id = m.user_id
     WHERE m.workspace_id = $1 AND m.role = 'admin'
     ORDER BY u.email_lower";

/// The senders' candidates: routes with a pending delivery that are neither
/// stopped by a refusal nor on a lost connection, the oldest pending first.
const SENDABLE_ROUTES_SQL: &str = "
    SELECT d.route_id FROM integration_deliveries d
      JOIN integration_routes r ON r.id = d.route_id
      LEFT JOIN integration_connections c ON c.id = r.connection_id
     WHERE d.status = 'pending' AND r.problem_reason IS NULL
       AND c.state IS DISTINCT FROM 'lost'
     GROUP BY d.route_id ORDER BY min(d.id) LIMIT $1";

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

/// `route_settings`' row: kind, destination ref and label, events, content
/// level, lane slugs.
type SettingsRow = (
    StoredKind,
    String,
    String,
    Vec<String>,
    String,
    Vec<String>,
    Option<String>,
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

    /// The fenced record of one attempt (ADR-EIP-002 decision 4): writes only
    /// while `claim_id` is still the pending row's claim, releasing the lease,
    /// and in the same transaction applies what the answer means beyond the
    /// row: a "retry after" on the destination's pacing, a refused
    /// destination as its route's problem, a refused connection as its
    /// `connected → lost` transition.
    /// A recorder whose lease was taken over gets [`RecordOutcome::LeaseLost`]
    /// and has changed nothing.
    pub async fn record_delivery(
        &self,
        delivery_id: i64,
        claim_id: Uuid,
        now: OffsetDateTime,
        result: DeliveryResult,
    ) -> Result<RecordOutcome, StoreError> {
        let columns = result.columns();
        let mut tx = self.pool.begin().await?;
        let written: Option<RecordedRow> = sqlx::query_as(&format!("{RECORD_SQL}{TALLY_COLUMNS}"))
            .bind(delivery_id)
            .bind(claim_id)
            .bind(columns.status)
            .bind(columns.next_attempt_at)
            .bind(columns.vendor_reason)
            .bind(columns.final_reason)
            .bind(columns.finishes)
            .bind(now)
            .bind(columns.slowed_down)
            .fetch_optional(&mut *tx)
            .await?;
        let Some((route_id, project_id, kind, event_kind, status, lag_seconds)) = written else {
            tx.rollback().await?;
            return Ok(RecordOutcome::LeaseLost);
        };
        let tally = delivery_tally((kind, event_kind, status, lag_seconds));
        match (&result, route_id) {
            (
                DeliveryResult::SlowedDown {
                    not_before: Some(not_before),
                    ..
                },
                Some(route_id),
            ) => {
                sqlx::query(SLOW_DOWN_SQL)
                    .bind(route_id)
                    .bind(*not_before)
                    .execute(&mut *tx)
                    .await?;
            }
            // A route of a project deleted meanwhile is left as it is.
            (DeliveryResult::RouteRefused { vendor_reason }, Some(route_id)) => {
                if lock_live_project(&mut tx, project_id).await? {
                    sqlx::query(ROUTE_PROBLEM_SQL)
                        .bind(route_id)
                        .bind(vendor_reason)
                        .bind(now)
                        .execute(&mut *tx)
                        .await?;
                }
            }
            (DeliveryResult::ConnectionRefused { vendor_reason }, Some(route_id)) => {
                let lost: Option<(Uuid,)> = sqlx::query_as(CONNECTION_LOST_SQL)
                    .bind(route_id)
                    .bind(vendor_reason)
                    .bind(now)
                    .fetch_optional(&mut *tx)
                    .await?;
                if let Some((connection_id,)) = lost {
                    let lost = connection_lost(&mut tx, connection_id, "system", now).await?;
                    tx.commit().await?;
                    return Ok(RecordOutcome::ConnectionLost { tally, lost });
                }
            }
            _ => {}
        }
        tx.commit().await?;
        Ok(RecordOutcome::Recorded(tally))
    }

    /// Slack's signed word that the Foundry app `app_id` left the Slack
    /// workspace `team_id` (ADR-EIP-005 decision 6): that connection,
    /// `connected → lost` for `reason`, with its audit record, in one
    /// transaction. Only the call that turns it gets the
    /// [`LostConnection`]; a connection already lost, another app's or
    /// another Slack workspace's changes nothing.
    pub async fn slack_app_lost(
        &self,
        team_id: &str,
        app_id: &str,
        reason: &str,
        now: OffsetDateTime,
    ) -> Result<Option<LostConnection>, StoreError> {
        let mut tx = self.pool.begin().await?;
        let lost: Option<(Uuid,)> = sqlx::query_as(SLACK_APP_LOST_SQL)
            .bind(team_id)
            .bind(app_id)
            .bind(reason)
            .bind(now)
            .fetch_optional(&mut *tx)
            .await?;
        let Some((connection_id,)) = lost else {
            tx.rollback().await?;
            return Ok(None);
        };
        let lost = connection_lost(&mut tx, connection_id, "slack", now).await?;
        tx.commit().await?;
        Ok(Some(lost))
    }

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

    /// A worker component's heartbeat (DDR-22): `role` on `replica` was alive
    /// at `at`.
    pub async fn beat_worker(
        &self,
        role: &str,
        replica: &str,
        at: OffsetDateTime,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO integration_worker_heartbeats (role, replica, beat_at)
             VALUES ($1, $2, $3)
             ON CONFLICT (role, replica) DO UPDATE SET beat_at = EXCLUDED.beat_at",
        )
        .bind(role)
        .bind(replica)
        .bind(at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// The connection and route gauges' facts (U-D8): connections by kind and
    /// state, and live projects' routes by kind and whether they are paused.
    pub async fn integration_census(&self) -> Result<IntegrationCensus, StoreError> {
        let connections = sqlx::query_as(
            "SELECT kind, state, count(*) FROM integration_connections GROUP BY 1, 2",
        )
        .fetch_all(&self.pool)
        .await?;
        let routes = sqlx::query_as(
            "SELECT r.kind, CASE WHEN r.paused_at IS NULL THEN 'active' ELSE 'paused' END,
                    count(*)
               FROM integration_routes r
               JOIN live_projects p ON p.id = r.project_id
              GROUP BY 1, 2",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(IntegrationCensus {
            connections,
            routes,
        })
    }

    /// The stall gauges' facts (DDR-22), on the database's clock: the outbox
    /// rows the dispatcher has yet to read and the age of the oldest, and the
    /// pending deliveries by kind and the age of the oldest.
    pub async fn integration_backlog(&self) -> Result<IntegrationBacklog, StoreError> {
        let (undispatched, undispatched_oldest_seconds): (i64, f64) = sqlx::query_as(
            "SELECT count(*), COALESCE(EXTRACT(EPOCH FROM now() - min(o.created_at)), 0)::float8
               FROM outbox o, integration_dispatch_start s
              WHERE o.dispatched_at IS NULL AND o.event_id IS NOT NULL
                AND o.id > s.outbox_id",
        )
        .fetch_one(&self.pool)
        .await?;
        let pending: Vec<(String, i64, f64)> = sqlx::query_as(
            "SELECT COALESCE(r.kind, 'email'), count(*),
                    EXTRACT(EPOCH FROM now() - min(d.occurred_at))::float8
               FROM integration_deliveries d
               LEFT JOIN integration_routes r ON r.id = d.route_id
              WHERE d.status = 'pending'
              GROUP BY 1",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(IntegrationBacklog {
            undispatched,
            undispatched_oldest_seconds,
            pending_oldest_seconds: pending.iter().map(|p| p.2).fold(0.0, f64::max),
            pending_by_kind: pending.into_iter().map(|(k, n, _)| (k, n)).collect(),
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
        insert_route_lanes(&mut tx, id, &route.lanes).await?;
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

    /// Pause a live route (DD-8), one transaction opening with
    /// `lock_live_project`: stamps it paused, reads its pending deliveries,
    /// skips exactly the ids `plan` names that are still pending and hold no
    /// live lease at `change.at` (a leased one is left to its sender), and
    /// writes its `route_paused` record. A paused or missing route changes
    /// nothing.
    pub async fn pause_route<P>(
        &self,
        change: &RouteChange,
        plan: P,
    ) -> Result<RouteChangeOutcome, StoreError>
    where
        P: FnOnce(&[CancelCandidate]) -> Vec<i64>,
    {
        let mut tx = self.pool.begin().await?;
        if !lock_live_project(&mut tx, change.project_id).await? {
            tx.rollback().await?;
            return Ok(RouteChangeOutcome::ProjectDeleted);
        }
        let paused = sqlx::query(
            "UPDATE integration_routes SET paused_at = $3, paused_by = $4, updated_at = now()
              WHERE id = $1 AND project_id = $2 AND paused_at IS NULL",
        )
        .bind(change.route_id)
        .bind(change.project_id)
        .bind(change.at)
        .bind(change.actor_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if paused == 0 {
            tx.rollback().await?;
            return Ok(RouteChangeOutcome::Unchanged);
        }
        let rows: Vec<(i64, Option<OffsetDateTime>)> = sqlx::query_as(
            "SELECT id, lease_until FROM integration_deliveries
              WHERE route_id = $1 AND status = 'pending' ORDER BY id",
        )
        .bind(change.route_id)
        .fetch_all(&mut *tx)
        .await?;
        let candidates: Vec<CancelCandidate> = rows
            .into_iter()
            .map(|(id, lease_until)| CancelCandidate {
                id,
                route_id: change.route_id,
                finished: false,
                lease_until,
            })
            .collect();
        let ids = plan(&candidates);
        let cancelled = sqlx::query(CANCEL_SQL)
            .bind(&ids)
            .bind(change.route_id)
            .bind(change.at)
            .bind("route_paused")
            .execute(&mut *tx)
            .await?
            .rows_affected();
        route_audit(&mut tx, change, "route_paused").await?;
        tx.commit().await?;
        Ok(RouteChangeOutcome::Changed { cancelled })
    }

    /// Resume a paused route, or one a refusal stopped, and clear its
    /// problem; one transaction opening with `lock_live_project`, with its
    /// `route_resumed` record. A paused route receives the events from this
    /// transaction's `now()` on, never what it skipped (DDR-27); a route only
    /// a refusal stopped keeps its waiting deliveries, which are sent now
    /// (D-8). A live route changes nothing.
    pub async fn resume_route(
        &self,
        change: &RouteChange,
    ) -> Result<RouteChangeOutcome, StoreError> {
        let mut tx = self.pool.begin().await?;
        if !lock_live_project(&mut tx, change.project_id).await? {
            tx.rollback().await?;
            return Ok(RouteChangeOutcome::ProjectDeleted);
        }
        let resumed = sqlx::query(
            "UPDATE integration_routes
                SET paused_at = NULL, paused_by = NULL, problem_reason = NULL,
                    problem_at = NULL, updated_at = now(),
                    receives_from = CASE WHEN paused_at IS NULL THEN receives_from
                                         ELSE now() END
              WHERE id = $1 AND project_id = $2
                AND (paused_at IS NOT NULL OR problem_reason IS NOT NULL)",
        )
        .bind(change.route_id)
        .bind(change.project_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if resumed == 0 {
            tx.rollback().await?;
            return Ok(RouteChangeOutcome::Unchanged);
        }
        route_audit(&mut tx, change, "route_resumed").await?;
        tx.commit().await?;
        Ok(RouteChangeOutcome::Changed { cancelled: 0 })
    }

    /// Edit a route's events, content level and lane filter, one transaction
    /// opening with `lock_live_project`, with its `route_edited` record. The
    /// next event the dispatcher reads is judged by the edited route.
    pub async fn edit_route(
        &self,
        change: &RouteChange,
        edit: &RouteEdit,
    ) -> Result<RouteChangeOutcome, StoreError> {
        let mut tx = self.pool.begin().await?;
        if !lock_live_project(&mut tx, change.project_id).await? {
            tx.rollback().await?;
            return Ok(RouteChangeOutcome::ProjectDeleted);
        }
        let edited = sqlx::query(
            "UPDATE integration_routes SET events = $3, content_level = $4, updated_at = now()
              WHERE id = $1 AND project_id = $2",
        )
        .bind(change.route_id)
        .bind(change.project_id)
        .bind(&edit.events)
        .bind(&edit.content_level)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if edited == 0 {
            tx.rollback().await?;
            return Ok(RouteChangeOutcome::Unchanged);
        }
        sqlx::query("DELETE FROM integration_route_lanes WHERE route_id = $1")
            .bind(change.route_id)
            .execute(&mut *tx)
            .await?;
        insert_route_lanes(&mut tx, change.route_id, &edit.lanes).await?;
        route_audit(&mut tx, change, "route_edited").await?;
        tx.commit().await?;
        Ok(RouteChangeOutcome::Changed { cancelled: 0 })
    }

    /// Delete a route with its deliveries (messages already sent stay where
    /// they are), one transaction opening with `lock_live_project`, with its
    /// `route_deleted` record. A route already gone changes nothing.
    pub async fn delete_route(
        &self,
        change: &RouteChange,
    ) -> Result<RouteChangeOutcome, StoreError> {
        let mut tx = self.pool.begin().await?;
        if !lock_live_project(&mut tx, change.project_id).await? {
            tx.rollback().await?;
            return Ok(RouteChangeOutcome::ProjectDeleted);
        }
        let exists: Option<Uuid> = sqlx::query_scalar(
            "SELECT id FROM integration_routes WHERE id = $1 AND project_id = $2 FOR UPDATE",
        )
        .bind(change.route_id)
        .bind(change.project_id)
        .fetch_optional(&mut *tx)
        .await?;
        if exists.is_none() {
            tx.rollback().await?;
            return Ok(RouteChangeOutcome::Unchanged);
        }
        route_audit(&mut tx, change, "route_deleted").await?;
        sqlx::query("DELETE FROM integration_routes WHERE id = $1")
            .bind(change.route_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(RouteChangeOutcome::Changed { cancelled: 0 })
    }

    /// Whether a route of the workspace's connection of `kind`, in a live
    /// project (paused or not), sends to `destination_ref`: a deleted Matrix
    /// route's bot leaves only a room no route uses. Reads only.
    pub async fn destination_routed(
        &self,
        workspace_id: Uuid,
        kind: &str,
        destination_ref: &str,
    ) -> Result<bool, StoreError> {
        Ok(sqlx::query_scalar(
            "SELECT EXISTS (
                 SELECT 1 FROM integration_routes r
                   JOIN integration_connections c ON c.id = r.connection_id
                   JOIN live_projects p ON p.id = r.project_id
                  WHERE c.workspace_id = $1 AND c.kind = $2 AND r.destination_ref = $3)",
        )
        .bind(workspace_id)
        .bind(kind)
        .bind(destination_ref)
        .fetch_one(&self.pool)
        .await?)
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
                                  'integration_audit_events', 'integration_key_checks')),
           EXISTS (SELECT 1 FROM information_schema.tables
                    WHERE table_schema = current_schema()
                      AND table_name = 'integration_email_choices')";

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

fn delivery_tally(row: TallyRow) -> DeliveryTally {
    let (kind, event_kind, status, lag_seconds) = row;
    DeliveryTally {
        kind,
        event_kind,
        status,
        lag_seconds,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionWriteOutcome {
    /// No connection of the kind existed: one row and its audit row.
    Connected,
    /// The workspace's connection of the kind was replaced in place.
    Reconnected,
    /// The workspace's connection of the kind is another row than the one the
    /// credential was sealed for; nothing was written.
    StaleRow,
    /// The workspace's connection of the kind is to another vendor workspace
    /// (`external_id`) than the one just approved, labelled
    /// `connected_label`; nothing was written (US-EIP-06: reconnecting to a
    /// different Slack workspace changes nothing).
    OtherWorkspace { connected_label: String },
}

/// A workspace's connection as its pages show it. Never the credential.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionRow {
    pub kind: String,
    pub state: String,
    /// Why a lost connection was lost (`lost_reason`).
    pub lost_reason: Option<String>,
    pub workspace_name: String,
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
    /// The connection's bot account (`integration_connections.bot_user_id`).
    pub bot_user_id: Option<String>,
    pub problem_reason: Option<String>,
    /// The label at save of a filter lane the project no longer has.
    pub missing_lane: Option<String>,
    pub newest_finished: Option<FinishedDeliveryRow>,
    /// The head pending delivery, when it has failed at least once, and the
    /// vendor's latest reason for it.
    pub head_failed: Option<Option<String>>,
    pub last_delivered_at: Option<OffsetDateTime>,
}

/// A route as its edit form shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteSettings {
    pub id: Uuid,
    /// `slack`, `matrix`, …
    pub kind: String,
    pub destination_ref: String,
    pub destination_label: String,
    pub events: Vec<String>,
    pub content_level: String,
    /// Slugs of the filter lanes the project still has.
    pub lanes: Vec<String>,
    /// Why a vendor refusal stopped the route, if one did.
    pub problem_reason: Option<String>,
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
           c.state AS connection_state, c.bot_user_id, r.problem_reason,
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

/// One delivery of a route, as its history lists it (US-EIP-05).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryHistoryRow {
    pub occurred_at: OffsetDateTime,
    pub event_kind: String,
    pub issue_key: Option<String>,
    pub status: String,
    pub attempts: i32,
    pub first_attempt_at: Option<OffsetDateTime>,
    pub finished_at: Option<OffsetDateTime>,
    pub give_up_at: OffsetDateTime,
    pub lease_until: Option<OffsetDateTime>,
    pub next_attempt_at: Option<OffsetDateTime>,
    pub last_vendor_reason: Option<String>,
    pub final_reason: Option<String>,
    pub slowed_down: bool,
}

/// A route's deliveries, newest first.
const ROUTE_HISTORY_SQL: &str = "
    SELECT occurred_at, event_kind, issue_key, status, attempts, first_attempt_at,
           finished_at, give_up_at, lease_until, next_attempt_at, last_vendor_reason,
           final_reason, slowed_down
      FROM integration_deliveries
     WHERE route_id = $1
     ORDER BY id DESC
     LIMIT $2";

fn delivery_history_row(row: &sqlx::postgres::PgRow) -> Result<DeliveryHistoryRow, sqlx::Error> {
    use sqlx::Row;
    Ok(DeliveryHistoryRow {
        occurred_at: row.try_get("occurred_at")?,
        event_kind: row.try_get("event_kind")?,
        issue_key: row.try_get("issue_key")?,
        status: row.try_get("status")?,
        attempts: row.try_get("attempts")?,
        first_attempt_at: row.try_get("first_attempt_at")?,
        finished_at: row.try_get("finished_at")?,
        give_up_at: row.try_get("give_up_at")?,
        lease_until: row.try_get("lease_until")?,
        next_attempt_at: row.try_get("next_attempt_at")?,
        last_vendor_reason: row.try_get("last_vendor_reason")?,
        final_reason: row.try_get("final_reason")?,
        slowed_down: row.try_get("slowed_down")?,
    })
}

fn project_route(row: &sqlx::postgres::PgRow) -> Result<ProjectRoute, sqlx::Error> {
    use sqlx::Row;
    let finished_status: Option<String> = row.try_get("finished_status")?;
    let head_failed: bool = row.try_get("head_failed")?;
    let head_reason: Option<String> = row.try_get("last_vendor_reason")?;
    Ok(ProjectRoute {
        id: row.try_get("id")?,
        kind: row.try_get::<StoredKind, _>("kind")?.wire(),
        destination_label: row.try_get("destination_label")?,
        paused: row.try_get("paused")?,
        connection_state: row.try_get("connection_state")?,
        bot_user_id: row.try_get("bot_user_id")?,
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
    /// The vendor's id for what is connected (a Slack team id, a Matrix
    /// homeserver's address).
    pub external_id: String,
    /// The bot's vendor id (a Slack bot user, a Matrix user id).
    pub bot_user_id: Option<String>,
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
    /// The route is paused, its connection disconnected, or the event
    /// happened before the route was last resumed (DDR-27): it is not sent.
    pub route_paused: bool,
    pub connection_id: Option<Uuid>,
    pub connection_state: Option<String>,
    pub destination_ref: String,
    pub project: Option<DeliveryProject>,
    pub issue: Option<DeliveryIssue>,
    /// What the event's outbox row recorded as it happened.
    pub event: DeliveryEvent,
}

/// An event as its (first) outbox row recorded it (ADR-EIP-003): lanes are
/// the lanes it named, by their current label (a lane gone reads as its
/// slug); the actor is who acted. Every field is `None` when the row did not
/// record it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryEvent {
    /// How many outbox rows the delivery stands for: more than one is a
    /// burst (D-11).
    pub issue_count: usize,
    pub issue_number: Option<i32>,
    /// A deleted issue's title.
    pub title: Option<String>,
    pub lane_label: Option<String>,
    pub from_lane_label: Option<String>,
    pub actor_name: Option<String>,
    /// A new comment's text as it reads now (D-3: sent only at Summary with
    /// comment text); `None` for any other event or a comment gone since.
    pub comment_text: Option<String>,
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

/// What the send-time checks of one project email read now (ADR-EIP-007
/// decision 3): the member's address, their choice for the email's project
/// and event, whether they can open the project (a member of its team),
/// whether they muted its workspace, and whether they filed the issue or
/// have a comment on it that is not deleted.
const EMAIL_FACTS_SQL: &str = "
    SELECT u.email_display, ch.scope, u.id, d.project_id, d.event_kind,
           EXISTS (SELECT 1 FROM live_projects p
                     JOIN team_memberships tm ON tm.team_id = p.team_id
                    WHERE p.id = d.project_id AND tm.user_id = u.id),
           EXISTS (SELECT 1 FROM notification_unsubscribes n
                    WHERE n.email_lower = u.email_lower AND n.workspace_id = d.workspace_id),
           EXISTS (SELECT 1 FROM issues i
                    WHERE i.id = d.issue_id AND i.project_id = d.project_id
                      AND i.author_id = u.id)
           OR EXISTS (SELECT 1 FROM comments cm
                       WHERE cm.issue_id = d.issue_id AND cm.author_id = u.id
                         AND cm.deleted_at IS NULL)
      FROM integration_deliveries d
      JOIN users u ON u.id = d.recipient_user_id
      LEFT JOIN integration_email_choices ch
             ON ch.user_id = u.id AND ch.project_id = d.project_id AND ch.event = d.event_kind
     WHERE d.id = $1";

/// One project email's send-time facts, read now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailSendFacts {
    pub recipient: String,
    /// The choice the email is sent under: its member, project and event
    /// (its stop link names exactly this choice).
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub event: String,
    /// The member's choice's scope (`all`, `mine`); `None` once it is gone.
    pub scope: Option<String>,
    pub can_open: bool,
    pub muted: bool,
    pub involved: bool,
}

/// The delivery, its route, the project and issue now, and the event as its
/// first outbox row recorded it. A project deleted after the event reads as
/// gone even once restored (D-8: what waited at the delete is never sent); a
/// disconnected connection reads as a paused route (R-9) (lanes by slug; [`LANE_LABELS_SQL`] reads
/// their labels now). A move row written before `from_state` was
/// added (a rolling deploy) finds its from-lane in the status change the same
/// transaction wrote (ADR-EIP-003 decision 4).
const DELIVERY_CONTEXT_SQL: &str = "
    SELECT d.event_kind, d.give_up_at, COALESCE(r.kind, 'email'),
           COALESCE(r.content_level, 'summary_with_comment'),
           r.paused_at IS NOT NULL OR c.state IS NOT DISTINCT FROM 'disconnected'
               OR COALESCE(o.created_at < r.receives_from, FALSE),
           r.connection_id, c.state, COALESCE(r.destination_ref, ''),
           p.name, p.key_prefix, p.slug, t.slug,
           i.number, i.title, i.state, u.display_name,
           cardinality(d.outbox_ids), (o.payload->>'number')::int, o.payload->>'title',
           o.payload->>'state', ef.slug, a.display_name, d.project_id, cm.body_markdown
      FROM integration_deliveries d
      LEFT JOIN integration_routes r ON r.id = d.route_id
      LEFT JOIN integration_connections c ON c.id = r.connection_id
      LEFT JOIN outbox o ON o.id = d.outbox_ids[1]
      LEFT JOIN live_projects p ON p.id = d.project_id
            AND NOT EXISTS (SELECT 1 FROM project_lifecycle_events e
                             WHERE e.project_id = d.project_id AND e.kind = 'deleted'
                               AND e.created_at >= o.created_at)
      LEFT JOIN teams t ON t.id = p.team_id
      LEFT JOIN issues i ON i.id = d.issue_id AND i.project_id = d.project_id
      LEFT JOIN users u ON u.id = i.author_id
      LEFT JOIN LATERAL (
          SELECT COALESCE(o.payload->>'from_state', (
                     SELECT e.old_value FROM issue_change_events e
                      WHERE o.event_type = 'IssueUpdated' AND e.issue_id = d.issue_id
                        AND e.field = 'status' AND e.new_value = o.payload->>'state'
                        AND e.created_at <= o.created_at
                      ORDER BY e.created_at DESC LIMIT 1)) AS slug
      ) ef ON true
      LEFT JOIN users a ON a.id = (o.payload->>'author_id')::uuid
      LEFT JOIN comments cm ON cm.id = (o.payload->>'comment_id')::uuid
                            AND cm.issue_id = d.issue_id
     WHERE d.id = $1";

/// A project's newest issue as a message shows it now: its lane by label (a
/// lane gone reads as its slug) and its author by display name.
const LATEST_ISSUE_SQL: &str = "
    SELECT i.number, i.title, COALESCE(l.label, i.state), u.display_name
      FROM issues i
      LEFT JOIN lanes l ON l.project_id = i.project_id AND l.slug = i.state
      LEFT JOIN users u ON u.id = i.author_id
     WHERE i.project_id = $1
     ORDER BY i.number DESC
     LIMIT 1";

/// The current labels of a project's lanes, by slug.
const LANE_LABELS_SQL: &str =
    "SELECT slug, label FROM lanes WHERE project_id = $1 AND slug = ANY($2)";

/// `workspace_connections`' row as Postgres answers it.
type SealedConnectionTuple = (Uuid, String, String, Option<String>, Option<Vec<u8>>);
type ConnectionTuple = (
    StoredKind,
    String,
    Option<String>,
    String,
    String,
    Option<String>,
    Option<String>,
    OffsetDateTime,
);

impl Store {
    /// The project's newest issue, for the route form's preview (DDR-25):
    /// one read, nothing written.
    pub async fn latest_issue(
        &self,
        project_id: Uuid,
    ) -> Result<Option<DeliveryIssue>, StoreError> {
        let row: Option<(i32, String, String, Option<String>)> = sqlx::query_as(LATEST_ISSUE_SQL)
            .bind(project_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(
            row.map(|(number, title, lane_label, author_name)| DeliveryIssue {
                number,
                title,
                lane_label,
                author_name,
            }),
        )
    }

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
        let existing: Option<(Uuid, String, String, String)> = sqlx::query_as(
            "SELECT id, external_id, label, state FROM integration_connections
              WHERE workspace_id = $1 AND kind = $2 FOR UPDATE",
        )
        .bind(connection.workspace_id)
        .bind(&connection.kind)
        .fetch_optional(&mut *tx)
        .await?;
        let was_disconnected =
            matches!(&existing, Some((_, _, _, state)) if state == "disconnected");
        let refused = match &existing {
            Some((id, ..)) if *id != connection.id => Some(ConnectionWriteOutcome::StaleRow),
            Some((_, external_id, label, _))
                if *external_id != connection.external_id && !was_disconnected =>
            {
                Some(ConnectionWriteOutcome::OtherWorkspace {
                    connected_label: label.clone(),
                })
            }
            _ => None,
        };
        if let Some(refused) = refused {
            tx.rollback().await?;
            return Ok(refused);
        }
        let outcome = if existing.is_some() {
            ConnectionWriteOutcome::Reconnected
        } else {
            ConnectionWriteOutcome::Connected
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
        if was_disconnected {
            routes_receive_from_now(&mut tx, connection.id).await?;
        }
        tx.commit().await?;
        Ok(outcome)
    }

    /// What disconnecting the workspace's connection of `kind` stops; `None`
    /// when it has none, or it already is disconnected. Reads only.
    pub async fn disconnect_preview(
        &self,
        workspace_id: Uuid,
        kind: &str,
    ) -> Result<Option<DisconnectPreview>, StoreError> {
        let row: Option<(String, i64, i64)> = sqlx::query_as(DISCONNECT_PREVIEW_SQL)
            .bind(workspace_id)
            .bind(kind)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|(label, routes, projects)| DisconnectPreview {
            label,
            routes,
            projects,
        }))
    }

    /// Disconnect the workspace's connection of `change.kind` (US-EIP-07,
    /// R-9), one transaction: the connection `FOR UPDATE`; the pending
    /// deliveries of its routes, of which exactly the ids `plan` names that
    /// still are pending and hold no live lease at `change.at` are skipped;
    /// the connection `disconnected`, its credential forgotten; its
    /// `connection_disconnected` record. Its routes read "Paused: … was
    /// disconnected" and nothing is dispatched to them until a reconnect,
    /// which receives from then on. No connection, or one already
    /// disconnected, changes nothing.
    pub async fn disconnect_connection<P>(
        &self,
        change: &ConnectionChange,
        plan: P,
    ) -> Result<DisconnectOutcome, StoreError>
    where
        P: FnOnce(&[CancelCandidate]) -> Vec<i64>,
    {
        let mut tx = self.pool.begin().await?;
        let held: Option<(Uuid, Option<Vec<u8>>)> = sqlx::query_as(
            "SELECT id, sealed_credential FROM integration_connections
              WHERE workspace_id = $1 AND kind = $2 AND state <> 'disconnected' FOR UPDATE",
        )
        .bind(change.workspace_id)
        .bind(&change.kind)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((connection_id, Some(sealed_credential))) = held else {
            tx.rollback().await?;
            return Ok(DisconnectOutcome::Unchanged);
        };
        let rows: Vec<(i64, Uuid, Option<OffsetDateTime>)> = sqlx::query_as(
            "SELECT d.id, d.route_id, d.lease_until
               FROM integration_deliveries d JOIN integration_routes r ON r.id = d.route_id
              WHERE r.connection_id = $1 AND d.status = 'pending' ORDER BY d.id",
        )
        .bind(connection_id)
        .fetch_all(&mut *tx)
        .await?;
        let candidates: Vec<CancelCandidate> = rows
            .into_iter()
            .map(|(id, route_id, lease_until)| CancelCandidate {
                id,
                route_id,
                finished: false,
                lease_until,
            })
            .collect();
        let ids = plan(&candidates);
        let cancelled = sqlx::query(DISCONNECT_CANCEL_SQL)
            .bind(&ids)
            .bind(connection_id)
            .bind(change.at)
            .execute(&mut *tx)
            .await?
            .rows_affected();
        sqlx::query(
            "UPDATE integration_connections
                SET state = 'disconnected', sealed_credential = NULL, credential_saved_at = NULL,
                    lost_reason = NULL, lost_at = NULL
              WHERE id = $1",
        )
        .bind(connection_id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO integration_audit_events
                    (id, workspace_id, project_id, actor_id, via, action, kind, subject, created_at)
             SELECT $1, c.workspace_id, NULL, $2, $3, 'connection_disconnected', c.kind, c.label, $4
               FROM integration_connections c WHERE c.id = $5",
        )
        .bind(Uuid::now_v7())
        .bind(change.actor_id)
        .bind(&change.via)
        .bind(change.at)
        .bind(connection_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(DisconnectOutcome::Disconnected {
            connection_id,
            sealed_credential,
            cancelled,
        })
    }

    /// The workspace's connections, oldest kind first. Never the credential.
    pub async fn workspace_connections(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<ConnectionRow>, StoreError> {
        let rows: Vec<ConnectionTuple> = sqlx::query_as(
            "SELECT c.kind, c.state, c.lost_reason, w.name, c.label, c.detail, u.display_name,
                        c.connected_at
                   FROM integration_connections c
                   JOIN workspaces w ON w.id = c.workspace_id
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
                |(
                    kind,
                    state,
                    lost_reason,
                    workspace_name,
                    label,
                    detail,
                    connected_by_name,
                    connected_at,
                )| ConnectionRow {
                    kind: kind.wire(),
                    state,
                    lost_reason,
                    workspace_name,
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

    /// What the edit form shows of the project's route `route_id`: its
    /// destination, events, content level and the slugs of its filter lanes
    /// the project still has. `None` for a route of another project.
    pub async fn route_settings(
        &self,
        project_id: Uuid,
        route_id: Uuid,
    ) -> Result<Option<RouteSettings>, StoreError> {
        let row: Option<SettingsRow> = sqlx::query_as(
            "SELECT r.kind, r.destination_ref, r.destination_label, r.events, r.content_level,
                    COALESCE(array_agg(x.slug ORDER BY x.position)
                             FILTER (WHERE x.slug IS NOT NULL), '{}'),
                    r.problem_reason
               FROM integration_routes r
               LEFT JOIN integration_route_lanes l ON l.route_id = r.id
               LEFT JOIN lanes x ON x.id = l.lane_id AND x.project_id = r.project_id
              WHERE r.id = $1 AND r.project_id = $2
              GROUP BY r.id",
        )
        .bind(route_id)
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(
            |(
                kind,
                destination_ref,
                destination_label,
                events,
                content_level,
                lanes,
                problem_reason,
            )| {
                RouteSettings {
                    id: route_id,
                    kind: kind.wire(),
                    destination_ref,
                    destination_label,
                    events,
                    content_level,
                    lanes,
                    problem_reason,
                }
            },
        ))
    }

    /// The route's newest `limit` deliveries, newest first (its history).
    pub async fn route_history(
        &self,
        route_id: Uuid,
        limit: i64,
    ) -> Result<Vec<DeliveryHistoryRow>, StoreError> {
        let rows = sqlx::query(ROUTE_HISTORY_SQL)
            .bind(route_id)
            .bind(limit)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .iter()
            .map(delivery_history_row)
            .collect::<Result<Vec<_>, _>>()?)
    }

    /// The workspace's connection of `kind` with its sealed credential.
    pub async fn sealed_connection(
        &self,
        workspace_id: Uuid,
        kind: &str,
    ) -> Result<Option<SealedConnection>, StoreError> {
        let row: Option<SealedConnectionTuple> = sqlx::query_as(
            "SELECT id, state, external_id, bot_user_id, sealed_credential
               FROM integration_connections
              WHERE workspace_id = $1 AND kind = $2",
        )
        .bind(workspace_id)
        .bind(kind)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(
            |(id, state, external_id, bot_user_id, sealed_credential)| SealedConnection {
                id,
                state,
                external_id,
                bot_user_id,
                sealed_credential,
            },
        ))
    }

    /// The vendor id of the connection `connection_id` (a Matrix
    /// connection's homeserver address); `None` when there is no such row.
    pub async fn connection_external_id(
        &self,
        connection_id: Uuid,
    ) -> Result<Option<String>, StoreError> {
        Ok(
            sqlx::query_scalar("SELECT external_id FROM integration_connections WHERE id = $1")
                .bind(connection_id)
                .fetch_optional(&self.pool)
                .await?,
        )
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

    /// What the send-time checks of the project email `delivery_id` read
    /// now; `None` when it is no member's email.
    pub async fn email_send_facts(
        &self,
        delivery_id: i64,
    ) -> Result<Option<EmailSendFacts>, StoreError> {
        type Row = (String, Option<String>, Uuid, Uuid, String, bool, bool, bool);
        let row: Option<Row> = sqlx::query_as(EMAIL_FACTS_SQL)
            .bind(delivery_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(
            |(recipient, scope, user_id, project_id, event, can_open, muted, involved)| {
                EmailSendFacts {
                    recipient,
                    user_id,
                    project_id,
                    event,
                    scope,
                    can_open,
                    muted,
                    involved,
                }
            },
        ))
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

    /// What a sender needs to attempt `delivery_id`, read now: the route, its
    /// connection, the project (through `live_projects`), the issue and the
    /// event as its outbox row recorded it, in one query.
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
        let project_name: Option<String> = row.try_get(8)?;
        let key_prefix: Option<String> = row.try_get(9)?;
        let project_slug: Option<String> = row.try_get(10)?;
        let team_slug: Option<String> = row.try_get(11)?;
        let number: Option<i32> = row.try_get(12)?;
        let title: Option<String> = row.try_get(13)?;
        let issue_lane: Option<String> = row.try_get(14)?;
        let author_name: Option<String> = row.try_get(15)?;
        let project = match (project_name, key_prefix, project_slug) {
            (Some(name), Some(key_prefix), Some(slug)) => Some(DeliveryProject {
                name,
                key_prefix,
                slug,
                team_slug,
            }),
            _ => None,
        };
        let event_lane: Option<String> = row.try_get(19)?;
        let from_lane: Option<String> = row.try_get(20)?;
        let project_id: Uuid = row.try_get(22)?;
        let slugs: Vec<&String> = [&issue_lane, &event_lane, &from_lane]
            .into_iter()
            .flatten()
            .collect();
        let labels: Vec<(String, String)> = sqlx::query_as(LANE_LABELS_SQL)
            .bind(project_id)
            .bind(&slugs)
            .fetch_all(&self.pool)
            .await?;
        // A lane gone since reads as its slug.
        let label_of = |slug: Option<String>| {
            slug.map(|slug| {
                labels
                    .iter()
                    .find(|(known, _)| *known == slug)
                    .map_or(slug.clone(), |(_, label)| label.clone())
            })
        };
        let issue = match (number, title, label_of(issue_lane)) {
            (Some(number), Some(title), Some(lane_label)) => Some(DeliveryIssue {
                number,
                title,
                lane_label,
                author_name,
            }),
            _ => None,
        };
        let issue_count: i32 = row.try_get(16)?;
        let event = DeliveryEvent {
            issue_count: usize::try_from(issue_count).unwrap_or(1),
            issue_number: row.try_get(17)?,
            title: row.try_get(18)?,
            lane_label: label_of(event_lane),
            from_lane_label: label_of(from_lane),
            actor_name: row.try_get(21)?,
            comment_text: row.try_get(23)?,
        };
        Ok(Some(DeliveryContext {
            delivery_id,
            event_kind: row.try_get(0)?,
            give_up_at: row.try_get(1)?,
            route_kind: row.try_get::<StoredKind, _>(2)?.wire(),
            content_level: row.try_get(3)?,
            route_paused: row.try_get(4)?,
            connection_id: row.try_get(5)?,
            connection_state: row.try_get(6)?,
            destination_ref: row.try_get(7)?,
            project,
            issue,
            event,
        }))
    }
}

/// A connection reconnected after a disconnect: each of its routes in a live
/// project receives the events from this transaction's `now()` on, so
/// nothing that happened while it was disconnected is sent (R-9). A deleted
/// project's routes are left as they are (`lock_live_project`, one project
/// at a time).
async fn routes_receive_from_now(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    connection_id: Uuid,
) -> Result<(), StoreError> {
    let projects: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT project_id FROM integration_routes
          WHERE connection_id = $1 ORDER BY project_id",
    )
    .bind(connection_id)
    .fetch_all(&mut **tx)
    .await?;
    for project_id in projects {
        if lock_live_project(tx, project_id).await? {
            sqlx::query(
                "UPDATE integration_routes SET receives_from = now(), updated_at = now()
                  WHERE connection_id = $1 AND project_id = $2",
            )
            .bind(connection_id)
            .bind(project_id)
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}

/// A route's filter lanes, in the write's transaction.
async fn insert_route_lanes(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    route_id: Uuid,
    lanes: &[RouteLane],
) -> Result<(), StoreError> {
    for lane in lanes {
        sqlx::query(
            "INSERT INTO integration_route_lanes (route_id, lane_id, label_at_save)
             VALUES ($1, $2, $3)",
        )
        .bind(route_id)
        .bind(lane.lane_id)
        .bind(&lane.label_at_save)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

/// A route change's audit record, in the change's transaction.
async fn route_audit(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    change: &RouteChange,
    action: &str,
) -> Result<(), StoreError> {
    sqlx::query(ROUTE_AUDIT_SQL)
        .bind(Uuid::now_v7())
        .bind(change.actor_id)
        .bind(&change.via)
        .bind(action)
        .bind(change.at)
        .bind(change.route_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

/// One key's check value (`integration_key_checks`, DDR-9): a fixed canary
/// sealed under the key `key_id` names, so the boot probe can prove a
/// configured key is the one that wrote it. No `Debug`: it carries sealed
/// bytes.
#[derive(Clone, PartialEq, Eq)]
pub struct KeyCheck {
    pub key_id: String,
    pub sealed_canary: Vec<u8>,
}

/// What `doctor rotate-secrets` did (C-11), or why it changed nothing.
#[derive(Debug, PartialEq, Eq)]
pub enum ResealOutcome<E> {
    /// Every stored credential was sealed again and the canary written.
    Resealed { credentials: usize },
    /// The caller could not re-seal a credential: the transaction was rolled
    /// back, so nothing changed.
    Refused(E),
}

impl Store {
    /// Every stored sealed credential (the boot probe reads only their key
    /// ids; ADR-EIP-004 decision 4).
    pub async fn sealed_credentials(&self) -> Result<Vec<Vec<u8>>, StoreError> {
        Ok(sqlx::query_scalar(
            "SELECT sealed_credential FROM integration_connections
              WHERE sealed_credential IS NOT NULL ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?)
    }

    /// Every key's check value, in key id order.
    pub async fn key_checks(&self) -> Result<Vec<KeyCheck>, StoreError> {
        let rows: Vec<(String, Vec<u8>)> = sqlx::query_as(
            "SELECT key_id, sealed_canary FROM integration_key_checks ORDER BY key_id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|(key_id, sealed_canary)| KeyCheck {
                key_id,
                sealed_canary,
            })
            .collect())
    }

    /// Record a key's check value unless that key already has one (every
    /// replica of a rollout may race to write it; the first stays).
    pub async fn record_key_check(&self, check: &KeyCheck) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO integration_key_checks (key_id, sealed_canary) VALUES ($1, $2)
             ON CONFLICT (key_id) DO NOTHING",
        )
        .bind(&check.key_id)
        .bind(&check.sealed_canary)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// `doctor rotate-secrets` (C-11, ADR-EIP-004 decision 5): in one
    /// transaction, replace every stored sealed credential with what `reseal`
    /// makes of it (given the row id the value is bound to) and write
    /// `canary` as its key's check value. A refusal from `reseal` rolls the
    /// whole transaction back. The store never sees a plaintext.
    pub async fn reseal_credentials<E>(
        &self,
        mut reseal: impl FnMut(Uuid, &[u8]) -> Result<Vec<u8>, E>,
        canary: &KeyCheck,
    ) -> Result<ResealOutcome<E>, StoreError> {
        let mut tx = self.pool.begin().await?;
        let rows: Vec<(Uuid, Vec<u8>)> = sqlx::query_as(
            "SELECT id, sealed_credential FROM integration_connections
              WHERE sealed_credential IS NOT NULL ORDER BY id FOR UPDATE",
        )
        .fetch_all(&mut *tx)
        .await?;
        for (id, sealed) in &rows {
            let resealed = match reseal(*id, sealed) {
                Ok(resealed) => resealed,
                Err(why) => {
                    tx.rollback().await?;
                    return Ok(ResealOutcome::Refused(why));
                }
            };
            sqlx::query("UPDATE integration_connections SET sealed_credential = $2 WHERE id = $1")
                .bind(id)
                .bind(resealed)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query(
            "INSERT INTO integration_key_checks (key_id, sealed_canary) VALUES ($1, $2)
             ON CONFLICT (key_id) DO UPDATE SET sealed_canary = EXCLUDED.sealed_canary",
        )
        .bind(&canary.key_id)
        .bind(&canary.sealed_canary)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(ResealOutcome::Resealed {
            credentials: rows.len(),
        })
    }
}

/// A project a member can open (a member of its team), as the Project emails
/// section offers it (US-EIP-10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailProject {
    pub project_id: Uuid,
    pub project_name: String,
    pub workspace_id: Uuid,
    pub workspace_name: String,
}

/// One of a member's email choices: an event of a project, for the issues
/// its scope (`all`, `mine`) covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailChoiceRow {
    pub project_id: Uuid,
    pub event: String,
    pub scope: String,
}

/// The live projects a member can open, by workspace and project name.
const EMAIL_PROJECTS_SQL: &str = "
    SELECT p.id, p.name, w.id, w.name
      FROM live_projects p
      JOIN team_memberships tm ON tm.team_id = p.team_id AND tm.user_id = $1
      JOIN workspaces w ON w.id = p.workspace_id
     ORDER BY w.name, w.id, p.name, p.id";

/// A live project's name.
const EMAIL_STOP_PROJECT_SQL: &str = "SELECT name FROM live_projects WHERE id = $1";

/// A member's choices in live projects.
const EMAIL_CHOICES_SQL: &str = "
    SELECT ch.project_id, ch.event, ch.scope
      FROM integration_email_choices ch
      JOIN live_projects p ON p.id = ch.project_id
     WHERE ch.user_id = $1
     ORDER BY ch.project_id, ch.event";

impl Store {
    /// The live projects `user_id` can open: the projects of their teams.
    pub async fn email_projects(&self, user_id: Uuid) -> Result<Vec<EmailProject>, StoreError> {
        let rows: Vec<(Uuid, String, Uuid, String)> = sqlx::query_as(EMAIL_PROJECTS_SQL)
            .bind(user_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(
                |(project_id, project_name, workspace_id, workspace_name)| EmailProject {
                    project_id,
                    project_name,
                    workspace_id,
                    workspace_name,
                },
            )
            .collect())
    }

    /// `user_id`'s email choices in live projects.
    pub async fn email_choices(&self, user_id: Uuid) -> Result<Vec<EmailChoiceRow>, StoreError> {
        let rows: Vec<(Uuid, String, String)> = sqlx::query_as(EMAIL_CHOICES_SQL)
            .bind(user_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|(project_id, event, scope)| EmailChoiceRow {
                project_id,
                event,
                scope,
            })
            .collect())
    }

    /// The name of the live project `project_id` (a stop link's confirm
    /// page); `None` once it is deleted.
    pub async fn email_stop_project(&self, project_id: Uuid) -> Result<Option<String>, StoreError> {
        Ok(sqlx::query_scalar(EMAIL_STOP_PROJECT_SQL)
            .bind(project_id)
            .fetch_optional(&self.pool)
            .await?)
    }

    /// Stop `user_id`'s `event` emails from `project_id` (US-EIP-11): delete
    /// that one choice, in one transaction that opens with
    /// `lock_live_project` (rule 4). Idempotent. The project's name, or
    /// `None`, with nothing written, when the project is not live.
    pub async fn stop_email_choice(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        event: &str,
    ) -> Result<Option<String>, StoreError> {
        let mut tx = self.pool.begin().await?;
        if !lock_live_project(&mut tx, project_id).await? {
            tx.rollback().await?;
            return Ok(None);
        }
        sqlx::query(
            "DELETE FROM integration_email_choices
              WHERE user_id = $1 AND project_id = $2 AND event = $3",
        )
        .bind(user_id)
        .bind(project_id)
        .bind(event)
        .execute(&mut *tx)
        .await?;
        let name: String = sqlx::query_scalar(EMAIL_STOP_PROJECT_SQL)
            .bind(project_id)
            .fetch_one(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(Some(name))
    }

    /// Replace `user_id`'s choices in `project_id` with `choices`
    /// (`(event, scope)` pairs), in one transaction that opens with
    /// `lock_live_project` (rule 4). `false`, with nothing written, when the
    /// project is not live.
    pub async fn save_email_choices(
        &self,
        user_id: Uuid,
        project_id: Uuid,
        choices: &[(String, String)],
    ) -> Result<bool, StoreError> {
        let mut tx = self.pool.begin().await?;
        if !lock_live_project(&mut tx, project_id).await? {
            tx.rollback().await?;
            return Ok(false);
        }
        sqlx::query("DELETE FROM integration_email_choices WHERE user_id = $1 AND project_id = $2")
            .bind(user_id)
            .bind(project_id)
            .execute(&mut *tx)
            .await?;
        for (event, scope) in choices {
            sqlx::query(
                "INSERT INTO integration_email_choices (user_id, project_id, event, scope)
                 VALUES ($1, $2, $3, $4)
                 ON CONFLICT (user_id, project_id, event) DO UPDATE SET scope = EXCLUDED.scope",
            )
            .bind(user_id)
            .bind(project_id)
            .bind(event)
            .bind(scope)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(true)
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
