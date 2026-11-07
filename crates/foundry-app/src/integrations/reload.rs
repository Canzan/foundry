//! Each replica's reload (ADR-IIS-004 r4, r6, r8): every
//! `FOUNDRY_INTEGRATIONS_RELOAD_MS` it reads the config version; on a change
//! it reads the snapshot (the version and the state it rebuilds from, in one
//! `REPEATABLE READ` transaction), rebuilds through the one shared rebuild
//! ([`Integrations::rebuilt`]) and installs the result. A failed read keeps
//! the last good integrations and logs `integrations.reload.failed`. It
//! never touches the key file, and its one write is its own replica state
//! row: at boot (its first tick), on every applied reload and every 10 s of
//! the injected clock. A sealed value under a key id this replica does not
//! hold wakes it at once, at most once per second of the injected clock.

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
/// How often (on the clock) an unknown key id may wake the reload early.
const NUDGE_EVERY: time::Duration = time::Duration::seconds(1);

/// One replica's reload task.
pub struct Reload {
    store: Arc<dyn IntegrationsReload>,
    integrations: IntegrationsHandle,
    clock: Arc<dyn Clock>,
    replica: String,
    last_beat: Option<OffsetDateTime>,
    last_nudge: Option<OffsetDateTime>,
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
            last_nudge: None,
        }
    }

    /// Tick every reload period, or sooner when an unknown key id nudges,
    /// until the runtime stops.
    pub async fn run(mut self) {
        loop {
            self.tick().await;
            let current = self.integrations.current();
            let nudge = current.reload_nudge();
            let sleep = tokio::time::sleep(current.settings().reload);
            tokio::pin!(sleep);
            loop {
                tokio::select! {
                    () = &mut sleep => break,
                    () = nudge.notified() => {
                        if self.nudge_due() {
                            break;
                        }
                    }
                }
            }
        }
    }

    /// Whether a nudge may reload now, recording it when it may.
    fn nudge_due(&mut self) -> bool {
        let now = self.clock.now();
        let due = nudge_allowed(self.last_nudge, now);
        if due {
            self.last_nudge = Some(now);
        }
        due
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

/// Whether a nudge at `now` may reload, the last one having been at `last`.
fn nudge_allowed(last: Option<OffsetDateTime>, now: OffsetDateTime) -> bool {
    last.is_none_or(|at| now - at >= NUDGE_EVERY)
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

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    /// An unknown key id reloads at once, then at most once a second.
    #[test]
    fn a_nudge_reloads_at_most_once_a_second() {
        let at = datetime!(2026-10-21 14:02 UTC);
        let ms = time::Duration::milliseconds;
        #[rustfmt::skip]
        let table = [
            (None,     at,               true),
            (Some(at), at,               false),
            (Some(at), at + ms(999),     false),
            (Some(at), at + ms(1000),    true),
            (Some(at), at + ms(60_000),  true),
        ];
        for (last, now, allowed) in table {
            assert_eq!(nudge_allowed(last, now), allowed, "{last:?} -> {now}");
        }
    }
}
