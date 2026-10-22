//! The dispatch plan (DDR-7, DDR-8, DDR-27; ADR-EIP-002 decision 2).

use crate::{EventKind, RouteSpec};
use time::OffsetDateTime;
use uuid::Uuid;

/// One committed outbox row, as the dispatcher read it.
#[derive(Debug, Clone, PartialEq)]
pub struct OutboxRow {
    pub id: i64,
    pub event_id: Uuid,
    pub source_txid: u64,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub created_at: OffsetDateTime,
}

/// One delivery the plan asks the store to queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedDelivery {
    pub workspace_id: Uuid,
    pub project_id: Uuid,
    pub route_id: Uuid,
    pub event_kind: EventKind,
    pub event_id: Uuid,
    pub outbox_ids: Vec<i64>,
    pub issue_id: Option<Uuid>,
    pub issue_key: Option<String>,
    pub occurred_at: OffsetDateTime,
}

/// The deliveries one dispatcher tick queues.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchPlan {
    deliveries: Vec<PlannedDelivery>,
}

/// The kinds the dispatcher routes in this release: New issue only (slice 01).
/// Later slices widen it; a kind outside it is never queued.
const DISPATCHED_KINDS: [EventKind; 1] = [EventKind::IssueCreated];

impl DispatchPlan {
    /// One delivery per (row, route) pair the route receives: a dispatched
    /// kind of the route's own project that it chose, written at or after its
    /// `receives_from` (DDR-27), while the route is not paused and its
    /// connection is not disconnected. A row of an unknown type, or without a
    /// readable project, is matched by no route.
    pub fn build(rows: &[OutboxRow], routes: &[RouteSpec]) -> DispatchPlan {
        let deliveries = rows
            .iter()
            .filter_map(RoutableEvent::read)
            .flat_map(|event| event.deliveries(routes))
            .collect();
        DispatchPlan { deliveries }
    }

    pub fn deliveries(&self) -> &[PlannedDelivery] {
        &self.deliveries
    }
}

/// An outbox row the plan can route: a dispatched kind and its project.
pub(crate) struct RoutableEvent<'a> {
    row: &'a OutboxRow,
    pub(crate) kind: EventKind,
    pub(crate) project_id: Uuid,
    pub(crate) occurred_at: OffsetDateTime,
}

impl<'a> RoutableEvent<'a> {
    fn read(row: &'a OutboxRow) -> Option<RoutableEvent<'a>> {
        let kind = EventKind::from_outbox_type(&row.event_type)
            .filter(|kind| DISPATCHED_KINDS.contains(kind))?;
        Some(RoutableEvent {
            row,
            kind,
            project_id: payload_uuid(&row.payload, "project_id")?,
            occurred_at: row.created_at,
        })
    }

    fn deliveries(&self, routes: &[RouteSpec]) -> Vec<PlannedDelivery> {
        routes
            .iter()
            .filter(|route| route.receives(self))
            .map(|route| self.delivery_to(route))
            .collect()
    }

    fn delivery_to(&self, route: &RouteSpec) -> PlannedDelivery {
        PlannedDelivery {
            workspace_id: route.workspace_id,
            project_id: route.project_id,
            route_id: route.id,
            event_kind: self.kind,
            event_id: self.row.event_id,
            outbox_ids: vec![self.row.id],
            issue_id: payload_uuid(&self.row.payload, "issue_id"),
            issue_key: self.row.payload["key"].as_str().map(str::to_owned),
            occurred_at: self.occurred_at,
        }
    }
}

fn payload_uuid(payload: &serde_json::Value, field: &str) -> Option<Uuid> {
    payload[field].as_str()?.parse().ok()
}
