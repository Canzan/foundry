//! The driven ports a channel kind implements (ADR-EIP-001 decision 1,
//! ADR-EIP-007). None names a credential type: a per-connection adapter is
//! built in foundry-app, which opens the sealed credential and keeps it inside.

use crate::{ChannelKind, ChannelOutcome, ReasonCode};
use async_trait::async_trait;

/// A destination's vendor reference: a Slack channel id, a Matrix room id
/// (a webhook URL in deliverable C).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DestinationRef(pub String);

/// One destination the catalogue lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Destination {
    pub reference: DestinationRef,
    pub label: String,
    /// Shared with another organisation (Slack Connect): a route to it needs
    /// the lead's confirmation (K-6).
    pub shared_with_another_organisation: bool,
}

/// What checking one destination found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DestinationCheck {
    pub can_post: bool,
    /// The bot is a member of the destination (Slack `is_member`).
    pub member: bool,
    pub shared_with_another_organisation: bool,
    pub encrypted: bool,
}

impl DestinationCheck {
    /// Whether this check shows the channel problem `problem` gone, so Resume
    /// may clear it (D-8: a route that needs attention is not sent until
    /// Resume re-checks its destination). Removing the bot from a channel is
    /// only undone by inviting it back, even where it could post unasked.
    pub fn clears(&self, problem: ReasonCode) -> bool {
        match problem {
            ReasonCode::NotInChannel | ReasonCode::BotNotInRoom => self.can_post && self.member,
            _ => self.can_post,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(can_post: bool, member: bool) -> DestinationCheck {
        DestinationCheck {
            can_post,
            member,
            shared_with_another_organisation: false,
            encrypted: false,
        }
    }

    #[test]
    fn removed_bot_needs_to_be_invited_back() {
        for problem in [ReasonCode::NotInChannel, ReasonCode::BotNotInRoom] {
            assert!(check(true, true).clears(problem));
            assert!(!check(true, false).clears(problem));
            assert!(!check(false, true).clears(problem));
        }
    }

    #[test]
    fn other_problems_clear_once_the_bot_can_post() {
        for problem in [ReasonCode::ChannelArchived, ReasonCode::ChannelNotFound] {
            assert!(check(true, false).clears(problem));
            assert!(!check(false, true).clears(problem));
        }
    }
}

/// A kind's encoded message, as its encoder produced it.
#[derive(Debug, Clone, PartialEq)]
pub struct EncodedMessage(pub serde_json::Value);

/// The idempotency key of one delivery (Matrix uses it as the `txnId`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeliveryKey(pub i64);

/// Write-only: sends one encoded message. `probe` has no default body, so a
/// kind that does not implement it does not compile.
#[async_trait]
pub trait DeliveryChannel: Send + Sync {
    fn kind(&self) -> ChannelKind;

    async fn deliver(
        &self,
        destination: &DestinationRef,
        message: &EncodedMessage,
        key: DeliveryKey,
    ) -> ChannelOutcome;

    async fn probe(&self) -> ChannelOutcome;
}

/// Strictly read-only: it has no method that changes anything at the vendor.
#[async_trait]
pub trait DestinationCatalog: Send + Sync {
    async fn verify_connection(&self) -> Result<(), ReasonCode>;

    async fn list_destinations(&self) -> Result<Vec<Destination>, ReasonCode>;

    async fn check_destination(
        &self,
        destination: &DestinationRef,
    ) -> Result<DestinationCheck, ReasonCode>;
}

/// The bot's membership of one destination (Matrix join and leave; Slack's
/// are no-ops). Only the route save and delete use-cases call it.
#[async_trait]
pub trait DestinationEnrolment: Send + Sync {
    async fn enrol(&self, destination: &DestinationRef) -> Result<(), ReasonCode>;

    async fn withdraw(&self, destination: &DestinationRef) -> Result<(), ReasonCode>;
}

/// One project email (ADR-EIP-007 decision 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutboundEmail {
    pub recipient: String,
    pub subject: String,
    pub text_body: String,
    /// The `List-Unsubscribe` URL and `List-Unsubscribe-Post` value.
    pub unsubscribe: Option<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmailTransportError {
    Transient,
    Permanent,
}

/// Write-only: sends one email through a configured provider.
#[async_trait]
pub trait EmailTransport: Send + Sync {
    async fn send(&self, email: &OutboundEmail) -> Result<(), EmailTransportError>;
}
