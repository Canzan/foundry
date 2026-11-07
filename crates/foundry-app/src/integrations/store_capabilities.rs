//! The delivery worker's store capabilities (check-arch
//! `integrations-store-capability`, DDR-26): each worker component holds only
//! the narrow trait it needs, implemented here over the store. This is the one
//! file of foundry-app's integrations module that names `foundry_store::Store`.
//!
//! - [`DispatchLog`]: the dispatcher's tick (outbox → deliveries, one
//!   transaction, under the dispatcher lock) and its heartbeat;
//! - [`DeliveryClaims`]: a sender's candidates (routes and members), its
//!   fenced claim and record;
//! - [`DeliverySweeps`]: the expiry sweep's fenced failure of the plan's ids
//!   and the retention sweep's deletion of the plan's ids;
//! - [`EventViewReads`]: read-only, what one claimed delivery needs at send
//!   time;
//! - [`ConnectionCredentials`]: read-only, a connection's sealed credential;
//! - [`ConnectionHealth`]: the events door's `connected → lost` transition
//!   on Slack's signed word;
//! - [`KeyProbe`]: the boot function's reads of the stored instance rows and
//!   its one write, the current instance key's check value;
//! - [`InstanceKeyWrites`]: the instance page's Generate (one write
//!   use-case, `store_secrets_key`), its Slack app save (`save_slack_app`),
//!   its activity read and the saver's name;
//! - [`IntegrationsReload`]: each replica's reload task (ADR-IIS-004 r4, r8):
//!   the config version, the snapshot, and its one write, its own replica
//!   state row.

use async_trait::async_trait;
use foundry_store::integrations::{
    ClaimOutcome, ClaimView, DeliveryContext, DeliveryResult, DeliveryTally, DispatchBatch,
    DispatchOutcome, EmailSendFacts, InstanceActivityRow, InstanceSnapshot, KeyCheck,
    LostConnection, NewDelivery, NewSecretsKey, PendingDelivery, Pruned, Pruning, RecordOutcome,
    ReplicaState, RetentionView, SealedSlot, SecretsKeyOutcome, SlackAppSave, SlackSaveOutcome,
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
/// The re-seal a Generate or Move applies to each stored sealed value:
/// `None` keeps it as it is; a refusal rolls the whole write back.
pub type Resealer<'a> =
    &'a (dyn Fn(SealedSlot, &[u8]) -> Result<Option<Vec<u8>>, ResealRefused> + Sync);

/// A stored value the re-seal could not open or seal again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResealRefused;

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
    ) -> Result<Vec<DeliveryTally>, StoreError>;

    async fn prune(
        &self,
        read_before: OffsetDateTime,
        plan: RetentionPlanner<'_>,
    ) -> Result<Pruned, StoreError>;

    /// Delete the replica state rows that last beat before `stale_before`.
    async fn prune_replica_states(&self, stale_before: OffsetDateTime) -> Result<u64, StoreError>;
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

    /// The members with a pending project email (ADR-EIP-007).
    async fn recipients_with_pending(&self, limit: i64) -> Result<Vec<Uuid>, StoreError>;

    /// A fenced claim on one member's project emails.
    async fn claim_email(
        &self,
        recipient_user_id: Uuid,
        now: OffsetDateTime,
        lease: Duration,
        choose: ClaimChooser<'_>,
    ) -> Result<ClaimOutcome, StoreError>;
}

/// Read-only: it has no method that writes.
#[async_trait]
pub trait EventViewReads: Send + Sync {
    async fn delivery_context(
        &self,
        delivery_id: i64,
    ) -> Result<Option<DeliveryContext>, StoreError>;

    /// What a project email's send-time checks read now.
    async fn email_send_facts(
        &self,
        delivery_id: i64,
    ) -> Result<Option<EmailSendFacts>, StoreError>;
}

/// Read-only: a connection's sealed credential, opened only by the adapters.
#[async_trait]
pub trait ConnectionCredentials: Send + Sync {
    async fn sealed_credential(&self, connection_id: Uuid) -> Result<Option<Vec<u8>>, StoreError>;
    /// Where the connection is (a Matrix homeserver's address).
    async fn connection_address(&self, connection_id: Uuid) -> Result<Option<String>, StoreError>;
}

/// A connection's health: Slack's signed word that the Foundry app left a
/// Slack workspace turns that connection `connected → lost`, once.
#[async_trait]
pub trait ConnectionHealth: Send + Sync {
    async fn slack_app_lost(
        &self,
        team_id: &str,
        app_id: &str,
        reason: &str,
        now: OffsetDateTime,
    ) -> Result<Option<LostConnection>, StoreError>;
}

/// The boot function's store (ADR-IIS-001 decision 9).
#[async_trait]
pub trait KeyProbe: Send + Sync {
    /// Record a key's check value unless one is stored.
    async fn record_key_check(&self, check: &KeyCheck) -> Result<(), StoreError>;
    /// The stored instance rows, read in one snapshot.
    async fn instance_snapshot(&self) -> Result<InstanceSnapshot, StoreError>;
}

/// A replica's reload task (ADR-IIS-004 r4, r8): reads, and one write, its
/// own replica state row. Never the key file, never another row.
#[async_trait]
pub trait IntegrationsReload: Send + Sync {
    /// The config version, polled every reload period.
    async fn config_version(&self) -> Result<i64, StoreError>;
    /// The stored instance rows and their version, read in one snapshot.
    async fn instance_snapshot(&self) -> Result<InstanceSnapshot, StoreError>;
    /// Upsert this replica's state row.
    async fn beat_replica_state(&self, state: &ReplicaState) -> Result<(), StoreError>;
}

/// The instance integrations page's writes and its activity (ADR-IIS-001
/// decision 5).
#[async_trait]
pub trait InstanceKeyWrites: Send + Sync {
    /// Create-if-absent the secrets key, re-seal through `reseal`, write the
    /// canary and the activity line, bump the version: one transaction.
    async fn store_secrets_key(
        &self,
        key: &NewSecretsKey,
        canary: &KeyCheck,
        at: OffsetDateTime,
        reseal: Resealer<'_>,
    ) -> Result<SecretsKeyOutcome<ResealRefused>, StoreError>;

    /// Save the Slack app, fenced on the key its secrets were sealed under,
    /// with its activity line and the version bump: one transaction.
    async fn save_slack_app(&self, app: &SlackAppSave) -> Result<SlackSaveOutcome, StoreError>;
    /// The instance's latest activity lines, newest first.
    async fn instance_activity(&self, limit: i64) -> Result<Vec<InstanceActivityRow>, StoreError>;
    /// A person's display name (who saved the Slack app, I-6).
    async fn display_name(&self, user_id: Uuid) -> Result<Option<String>, StoreError>;
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
    ) -> Result<Vec<DeliveryTally>, StoreError> {
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

    async fn prune_replica_states(&self, stale_before: OffsetDateTime) -> Result<u64, StoreError> {
        self.0.prune_replica_states(stale_before).await
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

    async fn recipients_with_pending(&self, limit: i64) -> Result<Vec<Uuid>, StoreError> {
        self.0.recipients_with_pending_deliveries(limit).await
    }

    async fn claim_email(
        &self,
        recipient_user_id: Uuid,
        now: OffsetDateTime,
        lease: Duration,
        choose: ClaimChooser<'_>,
    ) -> Result<ClaimOutcome, StoreError> {
        self.0
            .claim_email_delivery(recipient_user_id, now, lease, |view| choose(view))
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

    async fn email_send_facts(
        &self,
        delivery_id: i64,
    ) -> Result<Option<EmailSendFacts>, StoreError> {
        self.0.email_send_facts(delivery_id).await
    }
}

#[async_trait]
impl ConnectionCredentials for StoreCapabilities {
    async fn sealed_credential(&self, connection_id: Uuid) -> Result<Option<Vec<u8>>, StoreError> {
        self.0.sealed_credential(connection_id).await
    }

    async fn connection_address(&self, connection_id: Uuid) -> Result<Option<String>, StoreError> {
        self.0.connection_external_id(connection_id).await
    }
}

#[async_trait]
impl ConnectionHealth for StoreCapabilities {
    async fn slack_app_lost(
        &self,
        team_id: &str,
        app_id: &str,
        reason: &str,
        now: OffsetDateTime,
    ) -> Result<Option<LostConnection>, StoreError> {
        self.0.slack_app_lost(team_id, app_id, reason, now).await
    }
}

#[async_trait]
impl KeyProbe for StoreCapabilities {
    async fn record_key_check(&self, check: &KeyCheck) -> Result<(), StoreError> {
        self.0.record_key_check(check).await
    }

    async fn instance_snapshot(&self) -> Result<InstanceSnapshot, StoreError> {
        self.0.instance_snapshot().await
    }
}

#[async_trait]
impl IntegrationsReload for StoreCapabilities {
    async fn config_version(&self) -> Result<i64, StoreError> {
        self.0.config_version().await
    }

    async fn instance_snapshot(&self) -> Result<InstanceSnapshot, StoreError> {
        self.0.instance_snapshot().await
    }

    async fn beat_replica_state(&self, state: &ReplicaState) -> Result<(), StoreError> {
        self.0.beat_replica_state(state).await
    }
}

#[async_trait]
impl InstanceKeyWrites for StoreCapabilities {
    async fn store_secrets_key(
        &self,
        key: &NewSecretsKey,
        canary: &KeyCheck,
        at: OffsetDateTime,
        reseal: Resealer<'_>,
    ) -> Result<SecretsKeyOutcome<ResealRefused>, StoreError> {
        self.0
            .store_secrets_key(key, canary, at, |slot, sealed| reseal(slot, sealed))
            .await
    }

    async fn save_slack_app(&self, app: &SlackAppSave) -> Result<SlackSaveOutcome, StoreError> {
        self.0.save_slack_app(app).await
    }

    async fn instance_activity(&self, limit: i64) -> Result<Vec<InstanceActivityRow>, StoreError> {
        self.0.instance_activity(limit).await
    }

    async fn display_name(&self, user_id: Uuid) -> Result<Option<String>, StoreError> {
        self.0.user_display_name(user_id).await
    }
}
