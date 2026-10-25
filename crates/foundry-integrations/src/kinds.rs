//! The closed domain enums of the delivery contract (ADR-EIP-001 decision 1).

use std::time::Duration;

/// The kinds of destination a route can deliver to. `Webhook` is defined now
/// and implemented by deliverable C; a webhook route has no connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelKind {
    Slack,
    Matrix,
    Email,
    Webhook,
}

/// How much of an event a message carries (D-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentLevel {
    LinkOnly,
    Summary,
    SummaryWithComment,
}

/// A connection's state (`integration_connections.state`). A `Lost`
/// connection keeps receiving deliveries, which drain on reconnect; a
/// `Disconnected` one receives none (ADR-EIP-002 decision 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConnectionState {
    Connected,
    Lost,
    Disconnected,
}

/// Why a delivery was refused, retried, skipped or failed, in Foundry's words
/// (UI-7). Closed: a vendor's raw text never leaves its adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReasonCode {
    SlackNoAnswer,
    SlackSlowDown,
    ChannelArchived,
    NotInChannel,
    ChannelNotFound,
    AppRemoved,
    TokenRevoked,
    BotNotInRoom,
    RoomEncrypted,
    HomeserverNoAnswer,
    LaneMissing,
    GaveUp24h,
    IssueGone,
    ProjectDeleted,
    RoutePaused,
    NoAccess,
    Muted,
    /// The member no longer has the email choice that queued it.
    ChoiceGone,
    /// "Issues I created or commented on": the member is neither.
    NotInvolved,
    /// The email relay did not answer, or this Foundry has none right now.
    EmailNoAnswer,
    /// The email relay refused the message.
    EmailRefused,
    /// A credential this replica cannot open (ADR-EIP-004): a local fault,
    /// never a lost connection.
    SecretUnavailable,
}

/// What one delivery attempt came to, as a per-kind classifier reads the
/// vendor's answer. Every unknown answer is `Transient`, bounded by the
/// delivery's give-up time, so an unrecognised fault never parks a route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChannelOutcome {
    Delivered,
    Transient {
        reason: ReasonCode,
        retry_after: Option<Duration>,
    },
    RouteRefused {
        reason: ReasonCode,
    },
    ConnectionRefused {
        reason: ReasonCode,
    },
}
