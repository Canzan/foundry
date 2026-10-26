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

mod claims;
mod connections;
mod context;
mod dispatch;
mod emails;
mod history;
mod keys;
mod metrics;
mod recording;
mod routes;
mod sweeps;

pub use claims::{ClaimOutcome, ClaimView, ClaimedDelivery, PendingDelivery, DESTINATION_SPACING};
pub use connections::{
    AuditRow, ConnectionChange, ConnectionRow, ConnectionWriteOutcome, DisconnectOutcome,
    DisconnectPreview, NewConnection, SealedConnection,
};
pub use context::{DeliveryContext, DeliveryEvent, DeliveryIssue, DeliveryProject, EmailSendFacts};
pub use dispatch::{
    DeliveryTarget, DispatchBatch, DispatchEmailChoice, DispatchLane, DispatchOutcome,
    DispatchRoute, NewDelivery, OutboxEvent, DISPATCH_LOCK_ID,
};
pub use emails::{EmailChoiceRow, EmailProject};
pub use history::{DeliveryHistoryRow, FinishedDeliveryRow, ProjectRoute, RouteSettings};
pub use keys::{KeyCheck, ResealOutcome};
pub use metrics::{DeliveryTally, IntegrationBacklog, IntegrationCensus};
pub use recording::{DeliveryResult, LostConnection, RecordOutcome};
pub use routes::{
    CancelCandidate, NewRoute, RouteChange, RouteChangeOutcome, RouteEdit, RouteLane,
    RouteWriteOutcome,
};
pub use sweeps::{Pruned, Pruning, RetainedOutbox, RetainedRow, RetentionView};

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
