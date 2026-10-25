//! The dispatch plan (DDR-7, DDR-8, DDR-27; ADR-EIP-002 decision 2).

use crate::{EventKind, RouteSpec};
use std::collections::HashMap;
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

/// One delivery the plan asks the store to queue. A burst (D-11) lists every
/// row it stands for and names no single issue.
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

/// More events than this of one kind, from one action, to one route post one
/// message (D-11).
const BURST_OVER: usize = 5;

/// The rows of one action (`source_txid`) of one kind that reach one route.
type BurstKey = (u64, Uuid, EventKind);

impl DispatchPlan {
    /// One delivery per (row, route) pair the route receives: an event of the
    /// route's own project that it chose and its kind is given (ADR-EIP-003),
    /// written at or after its `receives_from` (DDR-27), while the route is
    /// not paused and its connection is not disconnected; a move only into
    /// the route's lanes, when it has a lane filter. A row of an unknown
    /// type, an in-lane reposition, or a row without a readable project is
    /// matched by no route. More than five pairs of one source transaction,
    /// kind and route become one burst delivery, in the place of its first
    /// row (DDR-8).
    pub fn build(rows: &[OutboxRow], routes: &[RouteSpec]) -> DispatchPlan {
        let pairs: Vec<(RoutableEvent<'_>, &RouteSpec)> = rows
            .iter()
            .filter_map(RoutableEvent::read)
            .flat_map(|event| {
                routes
                    .iter()
                    .filter(move |route| route.receives(&event))
                    .map(move |route| (event, route))
            })
            .collect();
        let mut bursts: HashMap<BurstKey, Vec<i64>> = HashMap::new();
        for (event, route) in &pairs {
            bursts
                .entry(event.burst_key(route))
                .or_default()
                .push(event.row.id);
        }
        let deliveries = pairs
            .iter()
            .filter_map(|(event, route)| {
                let ids = &bursts[&event.burst_key(route)];
                match ids.len() > BURST_OVER {
                    false => Some(event.delivery_to(route)),
                    true if ids[0] == event.row.id => Some(event.burst_to(route, ids.clone())),
                    true => None,
                }
            })
            .collect();
        DispatchPlan { deliveries }
    }

    pub fn deliveries(&self) -> &[PlannedDelivery] {
        &self.deliveries
    }
}

/// An outbox row the plan can route: a known kind and its project.
#[derive(Clone, Copy)]
pub(crate) struct RoutableEvent<'a> {
    pub(crate) row: &'a OutboxRow,
    pub(crate) kind: EventKind,
    pub(crate) project_id: Uuid,
    pub(crate) occurred_at: OffsetDateTime,
    /// The slug of the lane the issue is in after the event.
    pub(crate) lane: Option<&'a str>,
}

impl<'a> RoutableEvent<'a> {
    pub(crate) fn read(row: &'a OutboxRow) -> Option<RoutableEvent<'a>> {
        let kind = EventKind::from_outbox_type(&row.event_type)?;
        if kind == EventKind::IssueMoved && is_reposition(&row.payload) {
            return None;
        }
        Some(RoutableEvent {
            row,
            kind,
            project_id: payload_uuid(&row.payload, "project_id")?,
            occurred_at: row.created_at,
            lane: row.payload["state"].as_str(),
        })
    }

    fn burst_key(&self, route: &RouteSpec) -> BurstKey {
        (self.row.source_txid, route.id, self.kind)
    }

    fn delivery_to(&self, route: &RouteSpec) -> PlannedDelivery {
        PlannedDelivery {
            issue_id: payload_uuid(&self.row.payload, "issue_id"),
            issue_key: self.row.payload["key"].as_str().map(str::to_owned),
            ..self.burst_to(route, vec![self.row.id])
        }
    }

    fn burst_to(&self, route: &RouteSpec, outbox_ids: Vec<i64>) -> PlannedDelivery {
        PlannedDelivery {
            workspace_id: route.workspace_id,
            project_id: route.project_id,
            route_id: route.id,
            event_kind: self.kind,
            event_id: self.row.event_id,
            outbox_ids,
            issue_id: None,
            issue_key: None,
            occurred_at: self.occurred_at,
        }
    }
}

/// A lane change that did not change the lane: a reorder, never an event
/// (ADR-EIP-003 decision 1). A row without `from_state` (written before it was
/// added) is a move.
fn is_reposition(payload: &serde_json::Value) -> bool {
    payload["from_state"].is_string() && payload["from_state"] == payload["state"]
}

pub(crate) fn payload_uuid(payload: &serde_json::Value, field: &str) -> Option<Uuid> {
    payload[field].as_str()?.parse().ok()
}
