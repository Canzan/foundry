//! The operator's delivery metrics as closed label sets (DDR-22, U-D8; DEVOPS
//! observability). Every label value is drawn from a closed enum here: a
//! channel kind, an event, an outcome or a state, never a channel, room,
//! project, workspace, address or person. The shells register each series at
//! 0 and set or count exactly what these functions name.

use crate::{ChannelKind, ConnectionState, EventKind};

/// What a recorded delivery came to, as `foundry_integration_deliveries_total`
/// counts it: derived from the delivery's state after the record
/// (ADR-EIP-001). A delivery left pending is a retry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeliveryOutcome {
    Delivered,
    Retried,
    Failed,
    Skipped,
}

/// `(outcome, delivery status it is read from, label)`.
const OUTCOMES: [(DeliveryOutcome, &str, &str); 4] = [
    (DeliveryOutcome::Delivered, "delivered", "delivered"),
    (DeliveryOutcome::Retried, "pending", "retried"),
    (DeliveryOutcome::Failed, "failed", "failed"),
    (DeliveryOutcome::Skipped, "skipped", "skipped"),
];

impl DeliveryOutcome {
    pub const ALL: [DeliveryOutcome; 4] = [
        DeliveryOutcome::Delivered,
        DeliveryOutcome::Retried,
        DeliveryOutcome::Failed,
        DeliveryOutcome::Skipped,
    ];

    /// The outcome a delivery's stored `status` means; `None` for a status
    /// this core does not know.
    pub fn of_status(status: &str) -> Option<DeliveryOutcome> {
        OUTCOMES
            .iter()
            .find(|(_, stored, _)| *stored == status)
            .map(|(outcome, _, _)| *outcome)
    }

    /// The `outcome` label, e.g. `retried`.
    pub fn label(self) -> &'static str {
        OUTCOMES
            .iter()
            .find(|(outcome, _, _)| *outcome == self)
            .map_or("", |(_, _, label)| label)
    }
}

/// The kinds deliveries are counted by (a webhook arrives with deliverable C).
pub const METERED_KINDS: [ChannelKind; 3] =
    [ChannelKind::Slack, ChannelKind::Matrix, ChannelKind::Email];
/// The kinds that have connections and routes.
pub const CONNECTED_KINDS: [ChannelKind; 2] = [ChannelKind::Slack, ChannelKind::Matrix];
/// The `state` labels of `foundry_integration_routes`.
pub const ROUTE_STATES: [&str; 2] = ["active", "paused"];

/// One labelled series: its label values, in the metric's label order.
pub type Series = Vec<&'static str>;

/// Every `{kind, event, outcome}` series of the delivery counter: each
/// metered kind, each event a route of that kind is given, each outcome.
pub fn delivery_series() -> Vec<Series> {
    let mut series = Vec::new();
    for kind in METERED_KINDS {
        for event in EventKind::ALL.into_iter().filter(|e| e.announced_to(kind)) {
            for outcome in DeliveryOutcome::ALL {
                series.push(vec![kind.wire_name(), event.wire_name(), outcome.label()]);
            }
        }
    }
    series
}

/// The counted labels of one recorded delivery, or `None` when its kind,
/// event or status is outside the closed sets (it is not counted).
pub fn delivery_labels(
    kind: &str,
    event: &str,
    status: &str,
) -> Option<(ChannelKind, EventKind, DeliveryOutcome)> {
    let kind = ChannelKind::from_wire_name(kind).filter(|k| METERED_KINDS.contains(k))?;
    let event = EventKind::from_wire_name(event).filter(|e| e.announced_to(kind))?;
    Some((kind, event, DeliveryOutcome::of_status(status)?))
}

/// `foundry_integration_connections{kind, state}` over its closed set, from
/// `(kind, state, count)` rows: a pair without a row reads 0, a row outside
/// the set is ignored.
pub fn connection_census(rows: &[(String, String, i64)]) -> Vec<(Series, f64)> {
    let states = [
        ConnectionState::Connected,
        ConnectionState::Lost,
        ConnectionState::Disconnected,
    ]
    .map(ConnectionState::wire_name);
    census(rows, &states)
}

/// `foundry_integration_routes{kind, state}` over its closed set, from
/// `(kind, state, count)` rows, as [`connection_census`].
pub fn route_census(rows: &[(String, String, i64)]) -> Vec<(Series, f64)> {
    census(rows, &ROUTE_STATES)
}

/// Each connected kind in each of `states`, with the rows' count of it.
fn census(rows: &[(String, String, i64)], states: &[&'static str]) -> Vec<(Series, f64)> {
    CONNECTED_KINDS
        .iter()
        .flat_map(|kind| {
            states.iter().map(move |state| {
                let n: i64 = rows
                    .iter()
                    .filter(|(k, s, _)| k == kind.wire_name() && s == state)
                    .map(|(_, _, n)| *n)
                    .sum();
                (vec![kind.wire_name(), *state], n as f64)
            })
        })
        .collect()
}
