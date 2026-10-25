//! The closed v1 event catalogue (DDR-7, ADR-EIP-003).

use crate::ChannelKind;

/// A routable board event. Closed: the six shipped outbox types, each with
/// one wire name. A reorder is not an event, and an unknown outbox type maps
/// to none (it is skipped, never guessed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum EventKind {
    IssueCreated,
    IssueMoved,
    IssueDeleted,
    CommentAdded,
    CommentEdited,
    CommentDeleted,
}

/// `(kind, outbox event_type, wire name)` — the one mapping table.
const CATALOGUE: [(EventKind, &str, &str); 6] = [
    (EventKind::IssueCreated, "IssueCreated", "issue.created"),
    (EventKind::IssueMoved, "IssueUpdated", "issue.moved"),
    (EventKind::IssueDeleted, "IssueDeleted", "issue.deleted"),
    (EventKind::CommentAdded, "CommentAdded", "comment.added"),
    (EventKind::CommentEdited, "CommentEdited", "comment.edited"),
    (
        EventKind::CommentDeleted,
        "CommentDeleted",
        "comment.deleted",
    ),
];

impl EventKind {
    /// Every event, in catalogue order.
    pub const ALL: [EventKind; 6] = [
        EventKind::IssueCreated,
        EventKind::IssueMoved,
        EventKind::IssueDeleted,
        EventKind::CommentAdded,
        EventKind::CommentEdited,
        EventKind::CommentDeleted,
    ];

    /// The kind an outbox row's `event_type` announces (ADR-EIP-003 decision 2).
    pub fn from_outbox_type(event_type: &str) -> Option<EventKind> {
        CATALOGUE
            .iter()
            .find(|(_, outbox, _)| *outbox == event_type)
            .map(|(kind, _, _)| *kind)
    }

    /// The kind a stored wire name (`integration_routes.events`,
    /// `integration_deliveries.event_kind`) names.
    pub fn from_wire_name(wire: &str) -> Option<EventKind> {
        CATALOGUE
            .iter()
            .find(|(_, _, name)| *name == wire)
            .map(|(kind, _, _)| *kind)
    }

    /// The stored wire name, e.g. `issue.created`.
    pub fn wire_name(self) -> &'static str {
        CATALOGUE
            .iter()
            .find(|(kind, _, _)| *kind == self)
            .map_or("", |(_, _, name)| name)
    }

    /// Whether a route of `kind` is ever given this event (ADR-EIP-003
    /// decision 1): chat and email take New issue, Issue moved, Issue deleted
    /// and New comment; a webhook (deliverable C) takes all six. A comment's
    /// edit or delete therefore never posts to chat.
    pub fn announced_to(self, kind: ChannelKind) -> bool {
        kind == ChannelKind::Webhook
            || !matches!(self, EventKind::CommentEdited | EventKind::CommentDeleted)
    }

    /// Whether a message about this event can still be sent once its issue is
    /// gone: only a delete's, which carries its own title and actor.
    pub fn outlives_its_issue(self) -> bool {
        self == EventKind::IssueDeleted
    }

    /// Whether a delivery standing for `issue_count` events of this kind is
    /// still sent once its issue is gone: a delete, or a burst (D-11), which
    /// names no one issue.
    pub fn sendable_without_its_issue(self, issue_count: usize) -> bool {
        self.outlives_its_issue() || issue_count > 1
    }

    /// The event as a person reads it, e.g. "New issue" (D-2).
    pub fn display_name(self) -> &'static str {
        match self {
            EventKind::IssueCreated => "New issue",
            EventKind::IssueMoved => "Issue moved",
            EventKind::IssueDeleted => "Issue deleted",
            EventKind::CommentAdded => "New comment",
            EventKind::CommentEdited => "Comment edited",
            EventKind::CommentDeleted => "Comment deleted",
        }
    }
}
