//! Each replica's reload (ADR-IIS-004 r4, r6, r8): every
//! `FOUNDRY_INTEGRATIONS_RELOAD_MS` it reads the config version; on a change
//! it reads the snapshot (the version and the state it rebuilds from, in one
//! `REPEATABLE READ` transaction), rebuilds through the one shared rebuild
//! ([`Integrations::rebuilt`]) and installs the result. A failed read keeps
//! the last good integrations and logs `integrations.reload.failed`. It
//! never touches the key file, and its one write is its own replica state
//! row: at boot (its first tick), on every applied reload and every 10 s of
//! the injected clock.

use super::keys::KeyMode;
use super::store_capabilities::IntegrationsReload;
use super::{Integrations, IntegrationsHandle};
use crate::Clock;
use foundry_store::integrations::ReplicaState;
use foundry_store::StoreError;
use std::sync::Arc;
use time::OffsetDateTime;

/// How often (on the clock) a replica beats its state row between reloads.
const BEAT_EVERY: time::Duration = time::Duration::seconds(10);

/// One replica's reload task.
pub struct Reload {
    store: Arc<dyn IntegrationsReload>,
    integrations: IntegrationsHandle,
    clock: Arc<dyn Clock>,
    replica: String,
    last_beat: Option<OffsetDateTime>,
}

impl Reload {
    pub fn new(
        store: Arc<dyn IntegrationsReload>,
        integrations: IntegrationsHandle,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            store,
            integrations,
            clock,
            replica: uuid::Uuid::now_v7().to_string(),
            last_beat: None,
        }
    }

    /// Tick every reload period until the runtime stops.
    pub async fn run(mut self) {
        loop {
            self.tick().await;
            let period = self.integrations.current().settings().reload;
            tokio::time::sleep(period).await;
        }
    }

    /// One reload: apply a changed version, then beat the state row when
    /// something was applied or the beat is due.
    pub async fn tick(&mut self) {
        let applied = match self.apply().await {
            Ok(applied) => applied,
            Err(err) => {
                tracing::warn!(
                    event = "integrations.reload.failed",
                    error = %err,
                    "reloading the instance integrations failed; the last good ones keep serving"
                );
                false
            }
        };
        let now = self.clock.now();
        let due = self.last_beat.is_none_or(|at| now - at >= BEAT_EVERY);
        if applied || due {
            self.beat(now).await;
        }
    }

    /// Rebuild and install when the stored version is not the applied one.
    async fn apply(&self) -> Result<bool, StoreError> {
        let seen = self.integrations.current();
        let version = self.store.config_version().await?;
        if seen.applied_version() == Some(version) {
            return Ok(false);
        }
        let snapshot = self.store.instance_snapshot().await?;
        let next = seen.rebuilt(Ok(snapshot));
        Ok(self.integrations.install_over(&seen, next))
    }

    async fn beat(&mut self, now: OffsetDateTime) {
        let state = replica_state(&self.replica, &self.integrations.current(), now);
        match self.store.beat_replica_state(&state).await {
            Ok(()) => self.last_beat = Some(now),
            Err(err) => tracing::warn!(
                event = "integrations.reload.failed",
                error = %err,
                "recording this replica's integrations state failed"
            ),
        }
    }
}

/// This replica's state row over the integrations it serves. Never a key or
/// a key id.
fn replica_state(replica: &str, integrations: &Integrations, now: OffsetDateTime) -> ReplicaState {
    let mode = integrations.key_mode();
    ReplicaState {
        replica: replica.to_string(),
        applied_version: integrations.applied_version().unwrap_or(0),
        key_mode: key_mode_name(mode).to_string(),
        opens_secrets_key: mode == KeyMode::Sealed,
        beat_at: now,
    }
}

/// The key mode's name in `integration_replica_states.key_mode`.
fn key_mode_name(mode: KeyMode) -> &'static str {
    match mode {
        KeyMode::NoInstanceKey(_) => "no_instance_key",
        KeyMode::Direct => "direct",
        KeyMode::AwaitingSecretsKey => "awaiting_secrets_key",
        KeyMode::Sealed => "sealed",
        KeyMode::Unopenable => "unopenable",
    }
}
