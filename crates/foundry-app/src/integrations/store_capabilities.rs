//! The delivery worker's store capabilities (check-arch
//! `integrations-store-capability`, DDR-26): each worker component holds only
//! the narrow trait it needs, implemented here over the store. This is the one
//! file of foundry-app's integrations module that names `foundry_store::Store`.
//!
//! - [`DispatchLog`]: the dispatcher's tick (outbox → deliveries, one
//!   transaction, under the dispatcher lock) and its heartbeat;
//! - [`DeliveryClaims`]: a sender's candidates, its fenced claim and record;
//! - [`DeliverySweeps`]: the expiry sweep's fenced failure of the plan's ids
//!   and the retention sweep's deletion of the plan's ids;
//! - [`EventViewReads`]: read-only, what one claimed delivery needs at send
//!   time;
//! - [`ConnectionCredentials`]: read-only, a connection's sealed credential.

use async_trait::async_trait;
use foundry_store::integrations::{
    ClaimOutcome, ClaimView, DeliveryContext, DeliveryResult, DispatchBatch, DispatchOutcome,
    NewDelivery, PendingDelivery, Pruned, Pruning, RecordOutcome, RetentionView,
};
use foundry_store::{Store, StoreError};
use std::sync::Arc;
use std::time::Duration;
use time::OffsetDateTime;
use uuid::Uuid;

/// The dispatch plan a tick applies.
pub type DispatchPlanner<'a> = &'a (dyn Fn(&DispatchBatch) -> Vec<NewDelivery> + Sync);
/// The claim choice a sender applies.
pub type ClaimChooser<'a> = &'a (dyn Fn(&ClaimView) -> Option<i64> + Sync);
/// The expiry plan a sweep applies.
pub type ExpiryPlanner<'a> = &'a (dyn Fn(&[PendingDelivery]) -> Vec<i64> + Sync);
/// The retention plan a sweep applies.
pub type RetentionPlanner<'a> = &'a (dyn Fn(&RetentionView) -> Pruning + Sync);

#[async_trait]
pub trait DispatchLog: Send + Sync {
    async fn dispatch(
        &self,
        max_transactions: i64,
        dispatched_at: OffsetDateTime,
        give_up_after: Duration,
        plan: DispatchPlanner<'_>,
    ) -> Result<DispatchOutcome, StoreError>;

    async fn beat(&self, role: &str, replica: &str, at: OffsetDateTime) -> Result<(), StoreError>;
}

#[async_trait]
pub trait DeliverySweeps: Send + Sync {
    async fn expire(
        &self,
        now: OffsetDateTime,
        final_reason: &str,
        plan: ExpiryPlanner<'_>,
    ) -> Result<u64, StoreError>;

    async fn prune(
        &self,
        read_before: OffsetDateTime,
        plan: RetentionPlanner<'_>,
    ) -> Result<Pruned, StoreError>;
}

#[async_trait]
pub trait DeliveryClaims: Send + Sync {
    async fn routes_with_pending(&self, limit: i64) -> Result<Vec<Uuid>, StoreError>;

    async fn claim(
        &self,
        route_id: Uuid,
        now: OffsetDateTime,
        lease: Duration,
        choose: ClaimChooser<'_>,
    ) -> Result<ClaimOutcome, StoreError>;

    async fn record(
        &self,
        delivery_id: i64,
        claim_id: Uuid,
        now: OffsetDateTime,
        result: DeliveryResult,
    ) -> Result<RecordOutcome, StoreError>;
}

/// Read-only: it has no method that writes.
#[async_trait]
pub trait EventViewReads: Send + Sync {
    async fn delivery_context(
        &self,
        delivery_id: i64,
    ) -> Result<Option<DeliveryContext>, StoreError>;
}

/// Read-only: a connection's sealed credential, opened only by the adapters.
#[async_trait]
pub trait ConnectionCredentials: Send + Sync {
    async fn sealed_credential(&self, connection_id: Uuid) -> Result<Option<Vec<u8>>, StoreError>;
}

/// Every capability, over the store.
pub struct StoreCapabilities(Arc<Store>);

impl StoreCapabilities {
    pub fn new(store: Arc<Store>) -> Arc<Self> {
        Arc::new(Self(store))
    }
}

#[async_trait]
impl DispatchLog for StoreCapabilities {
    async fn dispatch(
        &self,
        max_transactions: i64,
        dispatched_at: OffsetDateTime,
        give_up_after: Duration,
        plan: DispatchPlanner<'_>,
    ) -> Result<DispatchOutcome, StoreError> {
        self.0
            .dispatch_tick(max_transactions, dispatched_at, give_up_after, |batch| {
                plan(batch)
            })
            .await
    }

    async fn beat(&self, role: &str, replica: &str, at: OffsetDateTime) -> Result<(), StoreError> {
        self.0.beat_worker(role, replica, at).await
    }
}

#[async_trait]
impl DeliverySweeps for StoreCapabilities {
    async fn expire(
        &self,
        now: OffsetDateTime,
        final_reason: &str,
        plan: ExpiryPlanner<'_>,
    ) -> Result<u64, StoreError> {
        self.0
            .expire_deliveries(now, final_reason, |due| plan(due))
            .await
    }

    async fn prune(
        &self,
        read_before: OffsetDateTime,
        plan: RetentionPlanner<'_>,
    ) -> Result<Pruned, StoreError> {
        self.0
            .prune_deliveries(read_before, |view| plan(view))
            .await
    }
}

#[async_trait]
impl DeliveryClaims for StoreCapabilities {
    async fn routes_with_pending(&self, limit: i64) -> Result<Vec<Uuid>, StoreError> {
        self.0.routes_with_pending_deliveries(limit).await
    }

    async fn claim(
        &self,
        route_id: Uuid,
        now: OffsetDateTime,
        lease: Duration,
        choose: ClaimChooser<'_>,
    ) -> Result<ClaimOutcome, StoreError> {
        self.0
            .claim_delivery(route_id, now, lease, |view| choose(view))
            .await
    }

    async fn record(
        &self,
        delivery_id: i64,
        claim_id: Uuid,
        now: OffsetDateTime,
        result: DeliveryResult,
    ) -> Result<RecordOutcome, StoreError> {
        self.0
            .record_delivery(delivery_id, claim_id, now, result)
            .await
    }
}

#[async_trait]
impl EventViewReads for StoreCapabilities {
    async fn delivery_context(
        &self,
        delivery_id: i64,
    ) -> Result<Option<DeliveryContext>, StoreError> {
        self.0.delivery_context(delivery_id).await
    }
}

#[async_trait]
impl ConnectionCredentials for StoreCapabilities {
    async fn sealed_credential(&self, connection_id: Uuid) -> Result<Option<Vec<u8>>, StoreError> {
        self.0.sealed_credential(connection_id).await
    }
}
