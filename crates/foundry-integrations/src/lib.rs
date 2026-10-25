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
//!   (ADR-EIP-002 decisions 4 and 9), [`cancel_set`] those a pause cancels
//!   (DD-8), and [`prune_set`] what the retention
//!   sweep deletes (DD-10);
//! - [`compose`] decides a message's lines from the event as it happened
//!   and [`encode_slack`] and [`encode_matrix`] place them (D-3, D-11, DDR-12,
//!   DDR-13); [`classify_slack`] and [`classify_matrix`] read the vendor's answer;
//! - [`RetryPolicy::settle`] schedules a retry with backoff and the vendor's
//!   "retry after", never past the give-up time, or stops a route or a
//!   connection on a refusal;
//! - [`RouteStatus::derive`] gives a route's K-9 status (DDR-19), and
//!   [`DeliveryRecord::line`] a delivery's history line;
//! - [`judge`] decides whether Foundry may connect to an address (DDR-14),
//!   with the operator's [`OutboundAllow`] and [`OutboundTarget::parse`];
//! - [`delivery_series`], [`delivery_labels`], [`connection_census`] and
//!   [`route_census`] name the operator's metric series over closed label
//!   sets (DDR-22, U-D8).
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
mod meter;
mod outbound;
mod ports;
mod queue;
mod retry;
mod route;
mod status;
mod wire;

pub use classify::{classify_matrix, classify_slack, MatrixAnswer, SlackAnswer};
pub use dispatch::{DispatchPlan, OutboxRow, PlannedDelivery};
pub use event::EventKind;
pub use history::DeliveryRecord;
pub use kinds::{ChannelKind, ChannelOutcome, ConnectionState, ContentLevel, ReasonCode};
pub use message::{
    compose, encode_matrix, encode_slack, escape_slack, issue_url, test_message, EventView,
    MessageLines, EXCERPT_CHARACTERS, OPEN_IN_FOUNDRY,
};
pub use meter::{
    connection_census, delivery_labels, delivery_series, route_census, DeliveryOutcome, Series,
    CONNECTED_KINDS, METERED_KINDS, ROUTE_STATES,
};
pub use outbound::{
    judge, judge_host, MalformedAllowEntry, OutboundAllow, OutboundTarget, Refusal, TargetError,
    TargetHost, Verdict, OUTBOUND_ALLOW_SETTING,
};
pub use ports::{
    DeliveryChannel, DeliveryKey, Destination, DestinationCatalog, DestinationCheck,
    DestinationEnrolment, DestinationRef, EmailTransport, EmailTransportError, EncodedMessage,
    OutboundEmail,
};
pub use queue::{
    cancel_set, claimable_head, expiry_set, prune_set, PrunePlan, QueuedDelivery, RetainedDelivery,
    RetainedOutboxRow, TargetedDelivery, HEARTBEAT_RETENTION, RETAINED_PER_ROUTE, RETENTION,
};
pub use retry::{Attempt, Jitter, RetryPolicy, Settlement};
pub use route::{FilterLane, RouteSpec};
pub use status::{Attention, ConnectionLostNotice, FinishedDelivery, RouteFacts, RouteStatus};
