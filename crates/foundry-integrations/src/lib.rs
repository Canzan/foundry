//! foundry-integrations — the pure core of external-integration-plugins
//! (DESIGN DDR-1, DDR-2, ADR-EIP-001, ADR-EIP-002).
//!
//! Every decision the integrations worker makes is a pure function here; the
//! shells (worker loops, handlers, store transactions) only apply its plans:
//!
//! - [`DispatchPlan::build`] turns committed outbox rows and live routes into
//!   the deliveries to queue (DDR-7, DDR-8, DDR-27);
//! - [`claimable_head`] chooses the one delivery a sender may claim on a
//!   route, and [`expiry_set`] the deliveries the expiry sweep fails
//!   (ADR-EIP-002 decisions 4 and 9).
//!
//! The driven ports a channel kind implements are [`DeliveryChannel`]
//! (write-only), [`DestinationCatalog`] (read-only), [`DestinationEnrolment`]
//! and, for project emails, [`EmailTransport`]. This crate does no I/O and
//! depends on no other Foundry crate but `foundry-core` (check-arch
//! `integrations-pure-core`).

#![forbid(unsafe_code)]

mod dispatch;
mod event;
mod kinds;
mod ports;
mod queue;
mod route;

pub use dispatch::{DispatchPlan, OutboxRow, PlannedDelivery};
pub use event::EventKind;
pub use kinds::{ChannelKind, ChannelOutcome, ConnectionState, ContentLevel, ReasonCode};
pub use ports::{
    DeliveryChannel, DeliveryKey, Destination, DestinationCatalog, DestinationCheck,
    DestinationEnrolment, DestinationRef, EmailTransport, EmailTransportError, EncodedMessage,
    OutboundEmail,
};
pub use queue::{claimable_head, expiry_set, QueuedDelivery};
pub use route::RouteSpec;
