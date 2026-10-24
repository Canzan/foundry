//! The closed v1 event catalogue (DDR-7, ADR-EIP-003).

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
