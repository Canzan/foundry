//! The delivery worker (ADR-EIP-002 decisions 2, 4, 6 and 8): every replica
//! runs one, spawned beside the other background loops in `main.rs` (and by
//! the acceptance harness's `spawn_app`, through the same constructor).
//!
//! Each tick, while the installed integrations run a worker:
//!
//! - the [`Dispatcher`] turns whole committed source transactions into
//!   deliveries through the pure [`DispatchPlan::build`] (one active
//!   dispatcher, the store's advisory lock);
//! - the [`Sender`] claims each route's head delivery through the pure
//!   [`claimable_head`] under a fenced lease, sends it outside any
//!   transaction, and records the outcome fenced by its `claim_id`, settled by
//!   [`RetryPolicy::settle`].
//!
//! The components hold only the narrow capabilities of
//! [`super::store_capabilities`], never the store. Every time is the injected
//! [`Clock`]'s.

use super::store_capabilities::{
    ConnectionCredentials, DeliveryClaims, DispatchLog, EventViewReads,
};
use super::{Integrations, IntegrationsHandle};
use crate::Clock;
use foundry_integrations::{
    claimable_head, compose, encode_slack, ChannelKind, ConnectionState, ContentLevel,
    DeliveryChannel, DeliveryKey, DestinationRef, DispatchPlan, EventKind, EventView, OutboxRow,
    PlannedDelivery, QueuedDelivery, ReasonCode, RetryPolicy, RouteSpec, Settlement,
};
use foundry_store::integrations::{
    ClaimOutcome, ClaimView, ClaimedDelivery, DeliveryContext, DeliveryResult, DeliveryTarget,
    DispatchBatch, DispatchRoute, NewDelivery, OutboxEvent,
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
        C: DispatchLog + DeliveryClaims + EventViewReads + ConnectionCredentials + 'static,
    {
        Self {
            dispatcher: Dispatcher {
                log: capabilities.clone(),
            },
            sender: Sender {
                claims: capabilities.clone(),
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
        if let Err(err) = self.dispatcher.tick(worker.give_up_after).await {
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
}

impl Dispatcher {
    async fn tick(&self, give_up_after: Duration) -> Result<(), foundry_store::StoreError> {
        self.log
            .dispatch(DISPATCH_BATCH, give_up_after, &plan_deliveries)
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
    claims: Arc<dyn DeliveryClaims>,
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
        for route_id in self.claims.routes_with_pending(SEND_BATCH).await? {
            let now = clock.now();
            let choose = |view: &ClaimView| head_of(view, now);
            if let ClaimOutcome::Claimed(claim) =
                self.claims.claim(route_id, now, lease, &choose).await?
            {
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

/// The claim choice over the store's view of one route.
fn head_of(view: &ClaimView, now: OffsetDateTime) -> Option<i64> {
    let queued: Vec<QueuedDelivery> = view
        .deliveries
        .iter()
        .map(|d| QueuedDelivery {
            id: d.id,
            next_attempt_at: d.next_attempt_at,
            lease_until: d.lease_until,
            give_up_at: d.give_up_at,
        })
        .collect();
    claimable_head(&queued, view.destination_not_before, now)
}

/// The event as a message may show it, or `None` when its issue or project
/// is gone (or the project has no team to address it by).
fn event_view(integrations: &Integrations, context: &DeliveryContext) -> Option<EventView> {
    let project = context.project.as_ref()?;
    let issue = context.issue.as_ref()?;
    let team_slug = project.team_slug.as_deref()?;
    Some(EventView {
        kind: EventKind::from_wire_name(&context.event_kind)?,
        issue_key: format!("{}-{}", project.key_prefix, issue.number),
        title: issue.title.clone(),
        project_name: project.name.clone(),
        actor_name: issue.author_name.clone().unwrap_or_default(),
        lane_label: issue.lane_label.clone(),
        issue_url: format!(
            "{}/team/{team_slug}/project/{}/issues/{}",
            integrations.settings().public_url,
            project.slug,
            issue.number
        ),
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
