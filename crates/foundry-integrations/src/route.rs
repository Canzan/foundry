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
    pub lane_ids: Vec<Uuid>,
}

impl RouteSpec {
    /// Whether this route receives `event` (DDR-8, DDR-27).
    pub(crate) fn receives(&self, event: &RoutableEvent<'_>) -> bool {
        self.project_id == event.project_id
            && self.events.contains(&event.kind)
            && event.kind.announced_to(self.kind)
            && self.is_open()
            && self.receives_from <= event.occurred_at
    }

    /// Not paused, and its connection (if it has one) not disconnected.
    fn is_open(&self) -> bool {
        !self.paused && self.connection_state != Some(ConnectionState::Disconnected)
    }
}
