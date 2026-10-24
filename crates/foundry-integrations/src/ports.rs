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
    pub shared_with_another_organisation: bool,
    pub encrypted: bool,
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
