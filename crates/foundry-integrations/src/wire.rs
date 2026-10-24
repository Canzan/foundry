//! The stored names of the closed domain enums, and the words a reason reads
//! as (UI-7). One table per enum.

use crate::{ChannelKind, ConnectionState, ContentLevel, ReasonCode};

const CHANNEL_KINDS: [(ChannelKind, &str); 4] = [
    (ChannelKind::Slack, "slack"),
    (ChannelKind::Matrix, "matrix"),
    (ChannelKind::Email, "email"),
    (ChannelKind::Webhook, "webhook"),
];

const CONNECTION_STATES: [(ConnectionState, &str); 3] = [
    (ConnectionState::Connected, "connected"),
    (ConnectionState::Lost, "lost"),
    (ConnectionState::Disconnected, "disconnected"),
];

const CONTENT_LEVELS: [(ContentLevel, &str); 3] = [
    (ContentLevel::LinkOnly, "link_only"),
    (ContentLevel::Summary, "summary"),
    (ContentLevel::SummaryWithComment, "summary_with_comment"),
];

/// `(code, stored name, words)`. `{destination}` stands for the route's
/// destination label.
const REASONS: [(ReasonCode, &str, &str); 18] = [
    (
        ReasonCode::SlackNoAnswer,
        "slack_no_answer",
        "Slack did not answer",
    ),
    (
        ReasonCode::SlackSlowDown,
        "slack_slow_down",
        "Slack asked Foundry to slow down",
    ),
    (
        ReasonCode::ChannelArchived,
        "channel_archived",
        "{destination} was archived",
    ),
    (
        ReasonCode::NotInChannel,
        "not_in_channel",
        "Foundry is no longer in {destination}. Invite @Foundry back, then Resume.",
    ),
    (
        ReasonCode::ChannelNotFound,
        "channel_not_found",
        "{destination} was not found",
    ),
    (
        ReasonCode::AppRemoved,
        "app_removed",
        "the Foundry app was removed from Slack",
    ),
    (
        ReasonCode::TokenRevoked,
        "token_revoked",
        "Foundry's access was revoked",
    ),
    (
        ReasonCode::BotNotInRoom,
        "bot_not_in_room",
        "Foundry's bot is not in {destination}",
    ),
    (
        ReasonCode::RoomEncrypted,
        "room_encrypted",
        "{destination} is now end-to-end encrypted. Foundry's bot cannot post to encrypted rooms yet.",
    ),
    (
        ReasonCode::HomeserverNoAnswer,
        "homeserver_no_answer",
        "the homeserver did not answer",
    ),
    (
        ReasonCode::LaneMissing,
        "lane_missing",
        "a filter lane no longer exists",
    ),
    (
        ReasonCode::GaveUp24h,
        "gave_up_24h",
        "it could not be delivered within 24 hours",
    ),
    (ReasonCode::IssueGone, "issue_gone", "the issue is gone"),
    (
        ReasonCode::ProjectDeleted,
        "project_deleted",
        "the project was deleted",
    ),
    (ReasonCode::RoutePaused, "route_paused", "the route was paused"),
    (ReasonCode::NoAccess, "no_access", "no access to the project"),
    (ReasonCode::Muted, "muted", "muted"),
    (
        ReasonCode::SecretUnavailable,
        "secret_unavailable",
        "this Foundry cannot read its saved connection",
    ),
];

fn by_name<T: Copy>(table: &[(T, &str)], wire: &str) -> Option<T> {
    table
        .iter()
        .find(|(_, name)| *name == wire)
        .map(|(value, _)| *value)
}

impl ChannelKind {
    pub fn from_wire_name(wire: &str) -> Option<ChannelKind> {
        by_name(&CHANNEL_KINDS, wire)
    }

    /// The reason an attempt that got no answer at all reads as: the
    /// vendor did not answer. A kind without a vendor to answer has none.
    pub fn no_answer(self) -> Option<ReasonCode> {
        match self {
            ChannelKind::Slack => Some(ReasonCode::SlackNoAnswer),
            ChannelKind::Matrix => Some(ReasonCode::HomeserverNoAnswer),
            ChannelKind::Email | ChannelKind::Webhook => None,
        }
    }

    /// "Slack", as K-9 names a kind.
    pub fn display_name(self) -> &'static str {
        match self {
            ChannelKind::Slack => "Slack",
            ChannelKind::Matrix => "Matrix",
            ChannelKind::Email => "Email",
            ChannelKind::Webhook => "Webhook",
        }
    }
}

impl ConnectionState {
    pub fn from_wire_name(wire: &str) -> Option<ConnectionState> {
        by_name(&CONNECTION_STATES, wire)
    }
}

impl ContentLevel {
    pub fn from_wire_name(wire: &str) -> Option<ContentLevel> {
        by_name(&CONTENT_LEVELS, wire)
    }
}

impl ReasonCode {
    pub fn from_wire_name(wire: &str) -> Option<ReasonCode> {
        REASONS
            .iter()
            .find(|(_, name, _)| *name == wire)
            .map(|(code, _, _)| *code)
    }

    pub fn wire_name(self) -> &'static str {
        REASONS
            .iter()
            .find(|(code, _, _)| *code == self)
            .map_or("", |(_, name, _)| name)
    }

    /// The reason in Foundry's words, for the route named by `destination`.
    pub fn words(self, destination: &str) -> String {
        REASONS
            .iter()
            .find(|(code, _, _)| *code == self)
            .map_or("", |(_, _, words)| words)
            .replace("{destination}", destination)
    }
}
