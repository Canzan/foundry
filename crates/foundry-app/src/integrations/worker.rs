//! The delivery worker (ADR-EIP-002 decisions 2, 4, 6 and 8): every replica
//! runs one, spawned beside the other background loops in `main.rs` (and by
//! the acceptance harness's `spawn_app`, through the same constructor).
//!
//! Each tick, while the installed integrations run a worker:
//!
//! - the [`Dispatcher`] turns whole committed source transactions into
//!   deliveries through the pure [`DispatchPlan::build`] (one active
//!   dispatcher, the store's advisory lock);
//! - the [`Sender`] first fails every delivery past its give-up time through
//!   the pure [`expiry_set`] (so nothing is attempted at or after it), then
//!   claims each route's head delivery through the pure [`claimable_head`]
//!   under a fenced lease, sends it outside any transaction, and records the
//!   outcome fenced by its `claim_id`, settled by [`RetryPolicy::settle`].
//!
//! The dispatcher beats its heartbeat row each tick (DDR-22).
//!
//! The components hold only the narrow capabilities of
//! [`super::store_capabilities`], never the store. Every time is the injected
//! [`Clock`]'s.

use super::store_capabilities::{
    ConnectionCredentials, DeliveryClaims, DeliverySweeps, DispatchLog, EventViewReads,
};
use super::{Integrations, IntegrationsHandle};
use crate::Clock;
use foundry_integrations::{
    claimable_head, compose, encode_slack, expiry_set, ChannelKind, ConnectionState, ContentLevel,
    DeliveryChannel, DeliveryKey, DestinationRef, DispatchPlan, EventKind, EventView, OutboxRow,
    PlannedDelivery, QueuedDelivery, ReasonCode, RetryPolicy, RouteSpec, Settlement,
};
use foundry_store::integrations::{
    ClaimOutcome, ClaimView, ClaimedDelivery, DeliveryContext, DeliveryResult, DeliveryTarget,
    DispatchBatch, DispatchRoute, IntegrationBacklog, NewDelivery, OutboxEvent, PendingDelivery,
};
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;

/// Source transactions one dispatcher tick reads at most.
const DISPATCH_BATCH: i64 = 100;
/// Routes one sender tick tries at most.
const SEND_BATCH: i64 = 50;
/// How often an idle worker looks again whether it should run.
const IDLE_POLL: Duration = Duration::from_millis(250);
/// The dispatcher's role in `integration_worker_heartbeats`.
const DISPATCHER_ROLE: &str = "dispatcher";

/// The stall gauges (DDR-22; DEVOPS observability), on the database's clock.
pub const OUTBOX_UNDISPATCHED: &str = "foundry_outbox_undispatched";
pub const OUTBOX_UNDISPATCHED_OLDEST_SECONDS: &str = "foundry_outbox_undispatched_oldest_seconds";
pub const DELIVERIES_PENDING: &str = "foundry_integration_deliveries_pending";
pub const DELIVERIES_PENDING_OLDEST_SECONDS: &str =
    "foundry_integration_deliveries_pending_oldest_seconds";
/// The closed `kind` set of [`DELIVERIES_PENDING`].
const PENDING_KINDS: [&str; 3] = ["slack", "matrix", "email"];
/// Past this, an undispatched outbox row means the dispatcher has stalled.
const DISPATCH_STALL_SECONDS: f64 = 300.0;

/// Register every stall gauge series at 0, before the first poll.
pub fn register_stall_gauges() {
    report_backlog(&IntegrationBacklog {
        undispatched: 0,
        undispatched_oldest_seconds: 0.0,
        pending_by_kind: Vec::new(),
        pending_oldest_seconds: 0.0,
    });
}

/// Set the stall gauges from one backlog read, and warn
/// `integrations.dispatch.stalled` while the oldest undispatched row is older
/// than five minutes.
pub fn report_backlog(backlog: &IntegrationBacklog) {
    metrics::gauge!(OUTBOX_UNDISPATCHED).set(backlog.undispatched as f64);
    metrics::gauge!(OUTBOX_UNDISPATCHED_OLDEST_SECONDS).set(backlog.undispatched_oldest_seconds);
    for kind in PENDING_KINDS {
        let pending = backlog
            .pending_by_kind
            .iter()
            .find(|(k, _)| k == kind)
            .map_or(0, |(_, n)| *n);
        metrics::gauge!(DELIVERIES_PENDING, "kind" => kind).set(pending as f64);
    }
    metrics::gauge!(DELIVERIES_PENDING_OLDEST_SECONDS).set(backlog.pending_oldest_seconds);
    if backlog.undispatched_oldest_seconds > DISPATCH_STALL_SECONDS {
        tracing::warn!(
            undispatched = backlog.undispatched,
            oldest_seconds = backlog.undispatched_oldest_seconds,
            "integrations.dispatch.stalled"
        );
    }
}

/// The dispatcher, the sender and what they read their settings from.
pub struct IntegrationsWorker {
    dispatcher: Dispatcher,
    sender: Sender,
    integrations: IntegrationsHandle,
    clock: Arc<dyn Clock>,
}

impl IntegrationsWorker {
    pub fn new<C>(
        capabilities: Arc<C>,
        integrations: IntegrationsHandle,
        clock: Arc<dyn Clock>,
    ) -> Self
    where
        C: DispatchLog
            + DeliveryClaims
            + DeliverySweeps
            + EventViewReads
            + ConnectionCredentials
            + 'static,
    {
        Self {
            dispatcher: Dispatcher {
                log: capabilities.clone(),
                replica: uuid::Uuid::now_v7().to_string(),
            },
            sender: Sender {
                #[cfg(feature = "test-support")]
                claimant: claimant_seam::Claimant::of(&integrations),
                claims: capabilities.clone(),
                sweeps: capabilities.clone(),
                events: capabilities.clone(),
                credentials: capabilities,
            },
            integrations,
            clock,
        }
    }

    /// Run until the runtime stops (the shipped loops' shutdown semantics).
    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(self.run())
    }

    async fn run(self) {
        loop {
            let integrations = self.integrations.current();
            if !integrations.runs_worker() {
                tokio::time::sleep(IDLE_POLL).await;
                continue;
            }
            self.tick(&integrations).await;
            tokio::time::sleep(integrations.settings().worker.poll).await;
        }
    }

    /// One dispatch and one round of sends.
    pub async fn tick(&self, integrations: &Integrations) {
        let worker = integrations.settings().worker;
        if let Err(err) = self
            .dispatcher
            .tick(self.clock.now(), worker.give_up_after)
            .await
        {
            tracing::warn!(error = %err, "integrations.dispatch.failed");
        }
        if let Err(err) = self
            .sender
            .tick(integrations, self.clock.as_ref(), worker.lease)
            .await
        {
            tracing::warn!(error = %err, "integrations.send.failed");
        }
    }
}

/// Outbox rows to deliveries (ADR-EIP-002 decision 2).
struct Dispatcher {
    log: Arc<dyn DispatchLog>,
    /// This worker's name in the heartbeat table.
    replica: String,
}

impl Dispatcher {
    async fn tick(
        &self,
        now: OffsetDateTime,
        give_up_after: Duration,
    ) -> Result<(), foundry_store::StoreError> {
        self.log.beat(DISPATCHER_ROLE, &self.replica, now).await?;
        self.log
            .dispatch(DISPATCH_BATCH, now, give_up_after, &plan_deliveries)
            .await
            .map(|_| ())
    }
}

/// The store's batch through the pure plan, back to the store's rows.
fn plan_deliveries(batch: &DispatchBatch) -> Vec<NewDelivery> {
    let rows: Vec<OutboxRow> = batch.events.iter().map(outbox_row).collect();
    let routes: Vec<RouteSpec> = batch.routes.iter().filter_map(route_spec).collect();
    DispatchPlan::build(&rows, &routes)
        .deliveries()
        .iter()
        .map(new_delivery)
        .collect()
}

fn outbox_row(event: &OutboxEvent) -> OutboxRow {
    OutboxRow {
        id: event.id,
        event_id: event.event_id,
        source_txid: event.source_txid,
        event_type: event.event_type.clone(),
        payload: event.payload.clone(),
        created_at: event.created_at,
    }
}

/// A stored route the core can read; a kind or level it does not know is
/// matched by no event.
fn route_spec(route: &DispatchRoute) -> Option<RouteSpec> {
    Some(RouteSpec {
        id: route.id,
        workspace_id: route.workspace_id,
        project_id: route.project_id,
        kind: ChannelKind::from_wire_name(&route.kind)?,
        connection_state: route
            .connection_state
            .as_deref()
            .and_then(ConnectionState::from_wire_name),
        events: route
            .events
            .iter()
            .filter_map(|wire| EventKind::from_wire_name(wire))
            .collect(),
        content_level: ContentLevel::from_wire_name(&route.content_level)?,
        paused: route.paused,
        receives_from: route.receives_from,
        lane_ids: route.lane_ids.clone(),
    })
}

fn new_delivery(planned: &PlannedDelivery) -> NewDelivery {
    NewDelivery {
        workspace_id: planned.workspace_id,
        project_id: planned.project_id,
        target: DeliveryTarget::Route(planned.route_id),
        event_kind: planned.event_kind.wire_name().to_string(),
        event_id: planned.event_id,
        outbox_ids: planned.outbox_ids.clone(),
        issue_id: planned.issue_id,
        issue_key: planned.issue_key.clone(),
        occurred_at: planned.occurred_at,
    }
}

/// Claim, send, record (ADR-EIP-002 decision 4).
struct Sender {
    /// Names this worker as the claimant of every claim it takes (DIST-22).
    #[cfg(feature = "test-support")]
    claimant: claimant_seam::Claimant,
    claims: Arc<dyn DeliveryClaims>,
    sweeps: Arc<dyn DeliverySweeps>,
    events: Arc<dyn EventViewReads>,
    credentials: Arc<dyn ConnectionCredentials>,
}

impl Sender {
    async fn tick(
        &self,
        integrations: &Integrations,
        clock: &dyn Clock,
        lease: Duration,
    ) -> Result<(), foundry_store::StoreError> {
        let expire = |due: &[PendingDelivery]| expiry_of(due, clock.now());
        self.sweeps
            .expire(clock.now(), ReasonCode::GaveUp24h.wire_name(), &expire)
            .await?;
        for route_id in self.claims.routes_with_pending(SEND_BATCH).await? {
            let now = clock.now();
            let choose = |view: &ClaimView| head_of(view, now);
            if let ClaimOutcome::Claimed(claim) =
                self.claims.claim(route_id, now, lease, &choose).await?
            {
                #[cfg(feature = "test-support")]
                self.claimant.took(claim.claim_id);
                self.attempt(integrations, clock, claim).await?;
            }
        }
        Ok(())
    }

    async fn attempt(
        &self,
        integrations: &Integrations,
        clock: &dyn Clock,
        claim: ClaimedDelivery,
    ) -> Result<(), foundry_store::StoreError> {
        let result = match self.events.delivery_context(claim.id).await? {
            Some(context) => self.send(integrations, clock, &context, claim.id).await?,
            None => skipped(ReasonCode::IssueGone),
        };
        self.claims
            .record(claim.id, claim.claim_id, clock.now(), result)
            .await
            .map(|_| ())
    }

    /// Send-time checks, then one post (outside any transaction).
    async fn send(
        &self,
        integrations: &Integrations,
        clock: &dyn Clock,
        context: &DeliveryContext,
        delivery_id: i64,
    ) -> Result<DeliveryResult, foundry_store::StoreError> {
        if context.route_paused {
            return Ok(skipped(ReasonCode::RoutePaused));
        }
        let Some(view) = event_view(integrations, context) else {
            return Ok(skipped(gone_reason(context)));
        };
        let level =
            ContentLevel::from_wire_name(&context.content_level).unwrap_or(ContentLevel::Summary);
        let bot = match context.connection_id {
            Some(connection_id) => {
                let sealed = self.credentials.sealed_credential(connection_id).await?;
                sealed.and_then(|sealed| integrations.slack_bot(connection_id, sealed))
            }
            None => None,
        };
        let outcome = match bot {
            Some(bot) => {
                bot.deliver(
                    &DestinationRef(context.destination_ref.clone()),
                    &encode_slack(&compose(&view, level)),
                    DeliveryKey(delivery_id),
                )
                .await
            }
            None => foundry_integrations::ChannelOutcome::Transient {
                reason: ReasonCode::SecretUnavailable,
                retry_after: None,
            },
        };
        let settled = RetryPolicy::FIXED_30S.settle(outcome, clock.now(), context.give_up_at);
        Ok(delivery_result(settled))
    }
}

/// Test-support only (DIST-22; DEVOPS C-13): which replica's worker took a
/// delivery's claim, so the acceptance harness can stop exactly that replica.
/// Every naming of this module carries the test-support gate, and its marker
/// must never reach a shipped binary (check-arch `integrations-test-seam`; the
/// Dockerfile refuses an image whose `foundry` carries the marker).
#[cfg(feature = "test-support")]
pub mod claimant_seam {
    use super::IntegrationsHandle;
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    use uuid::Uuid;

    /// The seam's unique marker: present in a binary only if the seam is.
    pub const CLAIMANT_SEAM_MARKER: &str = "foundry-test-seam:integrations-claimant-v1";

    /// Every claim taken, by its `claim_id`, to the worker that took it. A
    /// `claim_id` is fresh per claim, so it names its taker for good.
    fn takers() -> &'static Mutex<HashMap<Uuid, usize>> {
        static TAKERS: OnceLock<Mutex<HashMap<Uuid, usize>>> = OnceLock::new();
        TAKERS.get_or_init(Mutex::default)
    }

    /// One worker, known by the integrations handle it reads (one per replica).
    pub(super) struct Claimant(usize);

    impl Claimant {
        pub(super) fn of(handle: &IntegrationsHandle) -> Self {
            Self(identity(handle))
        }

        pub(super) fn took(&self, claim_id: Uuid) {
            tracing::trace!(seam = CLAIMANT_SEAM_MARKER, %claim_id, "integrations.claim.taken");
            let mut takers = takers().lock().unwrap_or_else(|p| p.into_inner());
            takers.insert(claim_id, self.0);
        }
    }

    /// Whether the worker reading `handle` took the claim `claim_id`.
    pub fn took_claim(handle: &IntegrationsHandle, claim_id: Uuid) -> bool {
        let takers = takers().lock().unwrap_or_else(|p| p.into_inner());
        takers.get(&claim_id) == Some(&identity(handle))
    }

    fn identity(handle: &IntegrationsHandle) -> usize {
        std::sync::Arc::as_ptr(&handle.current) as usize
    }
}

/// The claim choice over the store's view of one route.
fn head_of(view: &ClaimView, now: OffsetDateTime) -> Option<i64> {
    claimable_head(&queued(&view.deliveries), view.destination_not_before, now)
}

/// The expiry sweep's choice over the store's due deliveries.
fn expiry_of(due: &[PendingDelivery], now: OffsetDateTime) -> Vec<i64> {
    expiry_set(now, &queued(due))
}

fn queued(deliveries: &[PendingDelivery]) -> Vec<QueuedDelivery> {
    deliveries
        .iter()
        .map(|d| QueuedDelivery {
            id: d.id,
            next_attempt_at: d.next_attempt_at,
            lease_until: d.lease_until,
            give_up_at: d.give_up_at,
        })
        .collect()
}

/// The event as a message may show it: as it happened (its lanes, its actor,
/// a deleted issue's title, from its outbox row), with the names read now
/// (D-8). `None` when its project is gone (or has no team to address it by),
/// or its issue is gone and the event cannot be told without it.
fn event_view(integrations: &Integrations, context: &DeliveryContext) -> Option<EventView> {
    let project = context.project.as_ref()?;
    let team_slug = project.team_slug.as_deref()?;
    let kind = EventKind::from_wire_name(&context.event_kind)?;
    let event = &context.event;
    let issue = context.issue.as_ref();
    if issue.is_none() && !kind.sendable_without_its_issue(event.issue_count) {
        return None;
    }
    let number = event.issue_number.or(issue.map(|i| i.number)).unwrap_or(0);
    let either =
        |first: Option<&String>, then: Option<&String>| first.or(then).cloned().unwrap_or_default();
    Some(EventView {
        kind,
        issue_key: format!("{}-{number}", project.key_prefix),
        title: either(issue.map(|i| &i.title), event.title.as_ref()),
        project_name: project.name.clone(),
        actor_name: either(
            event.actor_name.as_ref(),
            issue.and_then(|i| i.author_name.as_ref()),
        ),
        lane_label: either(event.lane_label.as_ref(), issue.map(|i| &i.lane_label)),
        from_lane_label: either(event.from_lane_label.as_ref(), None),
        issue_url: format!(
            "{}/team/{team_slug}/project/{}/issues/{number}",
            integrations.settings().public_url,
            project.slug,
        ),
        issue_count: event.issue_count,
    })
}

fn gone_reason(context: &DeliveryContext) -> ReasonCode {
    if context.project.is_none() {
        ReasonCode::ProjectDeleted
    } else {
        ReasonCode::IssueGone
    }
}

fn skipped(reason: ReasonCode) -> DeliveryResult {
    DeliveryResult::Skipped {
        final_reason: reason.wire_name().to_string(),
    }
}

fn delivery_result(settled: Settlement) -> DeliveryResult {
    match settled {
        Settlement::Delivered => DeliveryResult::Delivered,
        Settlement::RetryAt { at, reason } => DeliveryResult::Retry {
            next_attempt_at: at,
            vendor_reason: Some(reason.wire_name().to_string()),
        },
        Settlement::GaveUp { vendor_reason } => DeliveryResult::Failed {
            final_reason: ReasonCode::GaveUp24h.wire_name().to_string(),
            vendor_reason: Some(vendor_reason.wire_name().to_string()),
        },
    }
}
