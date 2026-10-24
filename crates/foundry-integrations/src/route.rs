//! A route as the dispatch plan reads it.

use crate::dispatch::RoutableEvent;
use crate::{ChannelKind, ConnectionState, ContentLevel, EventKind};
use time::OffsetDateTime;
use uuid::Uuid;

/// One live project's chosen events to one destination.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteSpec {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub project_id: Uuid,
    pub kind: ChannelKind,
    /// `None` for a webhook route, which has no connection.
    pub connection_state: Option<ConnectionState>,
    pub events: Vec<EventKind>,
    pub content_level: ContentLevel,
    pub paused: bool,
    /// Database time of the create or resume (DDR-27).
    pub receives_from: OffsetDateTime,
    /// The lanes Issue moved is limited to; empty: any lane.
    pub lane_ids: Vec<FilterLane>,
}

/// A lane a route limits Issue moved to, by its id (saved with the route, so
/// it survives a rename) and its slug as the project has it now; `None` once
/// the lane is gone (DDR-19 rule 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterLane {
    pub id: Uuid,
    pub slug: Option<String>,
}

impl RouteSpec {
    /// Whether this route receives `event` (DDR-8, DDR-27).
    pub(crate) fn receives(&self, event: &RoutableEvent<'_>) -> bool {
        self.project_id == event.project_id
            && self.events.contains(&event.kind)
            && event.kind.announced_to(self.kind)
            && self.is_open()
            && self.receives_from <= event.occurred_at
            && (event.kind != EventKind::IssueMoved || self.takes_move_into(event.lane))
    }

    /// A move into `lane` passes the lane filter: no filter, or every chosen
    /// lane still exists and `lane` is one of them. A route with a gone lane
    /// posts no move until it is edited (US-EIP-03 example 5).
    fn takes_move_into(&self, lane: Option<&str>) -> bool {
        if self.lane_ids.is_empty() {
            return true;
        }
        let slugs: Option<Vec<&str>> = self
            .lane_ids
            .iter()
            .map(|chosen| chosen.slug.as_deref())
            .collect();
        matches!((slugs, lane), (Some(slugs), Some(lane)) if slugs.contains(&lane))
    }

    /// Not paused, and its connection (if it has one) not disconnected.
    fn is_open(&self) -> bool {
        !self.paused && self.connection_state != Some(ConnectionState::Disconnected)
    }
}
