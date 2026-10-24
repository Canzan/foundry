//! A route's status (DDR-19): derived from what is stored, never stored
//! itself. [`RouteStatus::derive`] applies the eight rules, first match wins.

use crate::{ChannelKind, ConnectionState, ReasonCode};
use time::OffsetDateTime;

/// How the newest finished delivery of a route ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FinishedDelivery {
    /// Failed at its give-up time (`gave_up_24h`), for this issue key.
    GaveUp { issue_key: String },
    /// Delivered, skipped, or failed for another reason.
    Other,
}

impl FinishedDelivery {
    /// A stored finished delivery: a `failed` one whose final reason is the
    /// 24-hour give-up is [`FinishedDelivery::GaveUp`].
    pub fn read(status: &str, final_reason: Option<&str>, issue_key: Option<&str>) -> Self {
        let gave_up = status == "failed"
            && final_reason.and_then(ReasonCode::from_wire_name) == Some(ReasonCode::GaveUp24h);
        match (gave_up, issue_key) {
            (true, Some(key)) => FinishedDelivery::GaveUp {
                issue_key: key.to_string(),
            },
            _ => FinishedDelivery::Other,
        }
    }
}

/// What a route's page knows about one route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteFacts {
    pub kind: ChannelKind,
    pub paused: bool,
    /// `None` for a webhook route, which has no connection.
    pub connection_state: Option<ConnectionState>,
    /// The route's stored problem (`problem_reason`).
    pub problem: Option<ReasonCode>,
    /// The label at save of a filter lane the project no longer has.
    pub missing_lane: Option<String>,
    pub newest_finished: Option<FinishedDelivery>,
    /// The head pending delivery, when it has failed at least once: the
    /// vendor's latest reason (if one was recorded).
    pub head_failed: Option<Option<ReasonCode>>,
    pub last_delivered_at: Option<OffsetDateTime>,
}

/// Why a route needs attention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Attention {
    Problem(ReasonCode),
    LaneMissing { label: String },
    GaveUp { issue_key: String },
}

/// K-9.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RouteStatus {
    Paused,
    Disconnected(ChannelKind),
    ConnectionLost,
    NeedsAttention(Attention),
    Retrying(Option<ReasonCode>),
    Delivering {
        last_delivery: Option<OffsetDateTime>,
    },
}

impl RouteStatus {
    /// DDR-19's rules in order; the first that matches decides.
    pub fn derive(facts: &RouteFacts) -> RouteStatus {
        if facts.paused {
            return RouteStatus::Paused;
        }
        match facts.connection_state {
            Some(ConnectionState::Disconnected) => return RouteStatus::Disconnected(facts.kind),
            Some(ConnectionState::Lost) => return RouteStatus::ConnectionLost,
            Some(ConnectionState::Connected) | None => {}
        }
        if let Some(reason) = facts.problem {
            return RouteStatus::NeedsAttention(Attention::Problem(reason));
        }
        if let Some(label) = &facts.missing_lane {
            return RouteStatus::NeedsAttention(Attention::LaneMissing {
                label: label.clone(),
            });
        }
        if let Some(FinishedDelivery::GaveUp { issue_key }) = &facts.newest_finished {
            return RouteStatus::NeedsAttention(Attention::GaveUp {
                issue_key: issue_key.clone(),
            });
        }
        if let Some(reason) = facts.head_failed {
            return RouteStatus::Retrying(reason);
        }
        RouteStatus::Delivering {
            last_delivery: facts.last_delivered_at,
        }
    }

    /// The status in K-9's words for the route to `destination`.
    pub fn words(&self, destination: &str) -> String {
        match self {
            RouteStatus::Paused => "Paused".to_string(),
            RouteStatus::Disconnected(kind) => {
                format!("Paused: {} was disconnected", kind.display_name())
            }
            RouteStatus::ConnectionLost => "Paused: connection lost".to_string(),
            RouteStatus::NeedsAttention(attention) => {
                format!("Needs attention: {}", attention.words(destination))
            }
            RouteStatus::Retrying(Some(reason)) => {
                format!("Retrying: {}", reason.words(destination))
            }
            RouteStatus::Retrying(None) => "Retrying".to_string(),
            RouteStatus::Delivering {
                last_delivery: Some(at),
            } => format!(
                "Delivering · last delivery {:02}:{:02}",
                at.hour(),
                at.minute()
            ),
            RouteStatus::Delivering {
                last_delivery: None,
            } => "Delivering".to_string(),
        }
    }
}

impl Attention {
    fn words(&self, destination: &str) -> String {
        match self {
            Attention::Problem(ReasonCode::ChannelArchived) => {
                format!("Foundry can no longer post to {destination} (the channel was archived)")
            }
            Attention::Problem(reason) => reason.words(destination),
            Attention::LaneMissing { label } => {
                format!("the lane {label} no longer exists. Choose lanes again.")
            }
            Attention::GaveUp { issue_key } => {
                format!("{issue_key}'s message could not be delivered within 24 hours")
            }
        }
    }
}
