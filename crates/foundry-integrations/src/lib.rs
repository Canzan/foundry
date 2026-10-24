//! foundry-integrations — the pure core of external-integration-plugins
//! (DESIGN DDR-1, DDR-2, ADR-EIP-001, ADR-EIP-002).
//!
//! Every decision the integrations worker makes is a pure function here; the
//! shells (worker loops, handlers, store transactions) only apply its plans:
//!
//! - [`DispatchPlan::build`] turns committed outbox rows and live routes into
//!   the deliveries to queue (DDR-7, DDR-8, DDR-27): the events a route's
//!   kind is given, never a reorder, more than five of one kind from one
//!   action as one burst (D-11);
//! - [`claimable_head`] chooses the one delivery a sender may claim on a
//!   route, [`expiry_set`] the deliveries the expiry sweep fails
//!   (ADR-EIP-002 decisions 4 and 9), and [`prune_set`] what the retention
//!   sweep deletes (DD-10);
//! - [`compose`] decides a message's lines from the event as it happened
//!   and [`encode_slack`] places them (D-3, D-11, DDR-12); [`classify_slack`] reads Slack's answer;
//! - [`RetryPolicy::settle`] schedules a retry with backoff and the vendor's
//!   "retry after", never past the give-up time, or stops a route or a
//!   connection on a refusal;
//! - [`RouteStatus::derive`] gives a route's K-9 status (DDR-19), and
//!   [`DeliveryRecord::line`] a delivery's history line.
//!
//! The driven ports a channel kind implements are [`DeliveryChannel`]
//! (write-only), [`DestinationCatalog`] (read-only), [`DestinationEnrolment`]
//! and, for project emails, [`EmailTransport`]. This crate does no I/O and
//! depends on no other Foundry crate but `foundry-core` (check-arch
//! `integrations-pure-core`).

#![forbid(unsafe_code)]

mod classify;
mod dispatch;
mod event;
mod history;
mod kinds;
mod message;
mod ports;
mod queue;
mod retry;
mod route;
mod status;
mod wire;

pub use classify::{classify_slack, SlackAnswer};
pub use dispatch::{DispatchPlan, OutboxRow, PlannedDelivery};
pub use event::EventKind;
pub use history::DeliveryRecord;
pub use kinds::{ChannelKind, ChannelOutcome, ConnectionState, ContentLevel, ReasonCode};
pub use message::{
    compose, encode_slack, escape_slack, issue_url, test_message, EventView, MessageLines,
    EXCERPT_CHARACTERS, OPEN_IN_FOUNDRY,
};
pub use ports::{
    DeliveryChannel, DeliveryKey, Destination, DestinationCatalog, DestinationCheck,
    DestinationEnrolment, DestinationRef, EmailTransport, EmailTransportError, EncodedMessage,
    OutboundEmail,
};
pub use queue::{
    claimable_head, expiry_set, prune_set, PrunePlan, QueuedDelivery, RetainedDelivery,
    RetainedOutboxRow, HEARTBEAT_RETENTION, RETAINED_PER_ROUTE, RETENTION,
};
pub use retry::{Attempt, Jitter, RetryPolicy, Settlement};
pub use route::{FilterLane, RouteSpec};
pub use status::{Attention, FinishedDelivery, RouteFacts, RouteStatus};
