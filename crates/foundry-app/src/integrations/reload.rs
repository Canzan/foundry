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
//! Boot and every install (an applied reload, the serving server's rebuild
//! after its own write) set the secrets alert inputs through
//! [`raise_alert_inputs`] (DEVOPS D5, task 8).

use super::keys::KeyMode;
use super::store_capabilities::IntegrationsReload;
use super::{Integrations, IntegrationsHandle, SecretsCheck};
use crate::Clock;
use foundry_store::integrations::ReplicaState;
use foundry_store::StoreError;
use std::sync::Arc;
use time::OffsetDateTime;

/// How often (on the clock) a replica beats its state row between reloads.
const BEAT_EVERY: time::Duration = time::Duration::seconds(10);
/// How often (on the clock) an unknown key id may wake the reload early.
const NUDGE_EVERY: time::Duration = time::Duration::seconds(1);

/// The degraded gauge: 1 while this server cannot open its secrets key
/// (I-26) or some stored value (I-4).
pub const SECRETS_DEGRADED: &str = "foundry_integration_secrets_degraded";
/// The startup and reload probe failure counter, labelled `probe_name`.
pub const PROBE_FAILURES_TOTAL: &str = "probe_failures_total";
/// The secrets probe's `probe_name`.
pub const SECRETS_PROBE: &str = "secrets";

/// The secrets alert inputs over one boot's or applied reload's outcome
/// (DEVOPS D5, task 8): the degraded gauge's value, the secrets probe's
/// failure count to add, and whether the `health.degraded` line is logged.
#[derive(Debug, Clone, Copy, PartialEq)]
struct AlertInputs {
    degraded: f64,
    failures: u64,
    logs_degraded: bool,
}

/// The alert inputs over the key mode, and whether some stored value opens
/// under no held key (I-4).
fn alert_inputs(mode: KeyMode, check: &SecretsCheck) -> AlertInputs {
    let degraded = mode == KeyMode::Unopenable || check.degraded();
    AlertInputs {
        degraded: if degraded { 1.0 } else { 0.0 },
        failures: u64::from(degraded),
        logs_degraded: degraded,
    }
}

/// Set the secrets alert inputs from `integrations`: what boot ends with and
/// every applied reload. The line names the key mode and key ids, never a key.
pub fn raise_alert_inputs(integrations: &Integrations) {
    let mode = integrations.key_mode();
    let check = integrations.secrets_check();
    let inputs = alert_inputs(mode, check);
    metrics::gauge!(SECRETS_DEGRADED).set(inputs.degraded);
    metrics::counter!(PROBE_FAILURES_TOTAL, "probe_name" => SECRETS_PROBE)
        .increment(inputs.failures);
    if inputs.logs_degraded {
        tracing::warn!(
            event = "health.degraded",
            probe = SECRETS_PROBE,
            key_mode = key_mode_name(mode),
            missing_key_ids = %check.missing_key_ids.join(","),
            "this Foundry cannot open its secrets key or some stored integration credential; \
             the board runs, those connections are unavailable"
        );
    }
}

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
    use crate::integrations::keys::NoKeyReason;
    use time::macros::datetime;

    /// Every key mode, with and without I-4: only I-26 or I-4 sets the gauge,
    /// adds one secrets probe failure and logs `health.degraded`.
    #[test]
    fn only_an_unopenable_key_or_value_raises_the_alarm() {
        let clean = SecretsCheck::default();
        let i4 = SecretsCheck {
            missing_key_ids: vec!["k-old".to_string()],
        };
        let calm = AlertInputs {
            degraded: 0.0,
            failures: 0,
            logs_degraded: false,
        };
        let alarm = AlertInputs {
            degraded: 1.0,
            failures: 1,
            logs_degraded: true,
        };
        #[rustfmt::skip]
        let table = [
            (KeyMode::NoInstanceKey(NoKeyReason::Unwritable), &clean, calm),
            (KeyMode::NoInstanceKey(NoKeyReason::Unreadable), &clean, calm),
            (KeyMode::Direct,                                 &clean, calm),
            (KeyMode::AwaitingSecretsKey,                     &clean, calm),
            (KeyMode::Sealed,                                 &clean, calm),
            (KeyMode::Unopenable,                             &clean, alarm),
            (KeyMode::NoInstanceKey(NoKeyReason::Unwritable), &i4,    alarm),
            (KeyMode::NoInstanceKey(NoKeyReason::Unreadable), &i4,    alarm),
            (KeyMode::Direct,                                 &i4,    alarm),
            (KeyMode::AwaitingSecretsKey,                     &i4,    alarm),
            (KeyMode::Sealed,                                 &i4,    alarm),
            (KeyMode::Unopenable,                             &i4,    alarm),
        ];
        for (mode, check, inputs) in table {
            assert_eq!(alert_inputs(mode, check), inputs, "{mode:?}, {check:?}");
        }
    }

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

    /// The integrations an instance with no key and nothing stored serves,
    /// judged over config version 1.
    fn judged() -> Integrations {
        let settings =
            super::super::config::IntegrationSettings::off("https://foundry.example.org");
        Integrations::new(settings).rebuilt(Ok(snapshot()))
    }

    fn snapshot() -> foundry_store::integrations::InstanceSnapshot {
        foundry_store::integrations::InstanceSnapshot {
            version: 1,
            secrets_key: None,
            slack_app: None,
            key_checks: Vec::new(),
            sealed_credentials: Vec::new(),
        }
    }

    /// A store at config version 1 recording every beat.
    #[derive(Default)]
    struct Beats(std::sync::Mutex<Vec<ReplicaState>>);

    #[async_trait::async_trait]
    impl IntegrationsReload for Beats {
        async fn config_version(&self) -> Result<i64, StoreError> {
            Ok(1)
        }
        async fn instance_snapshot(
            &self,
        ) -> Result<foundry_store::integrations::InstanceSnapshot, StoreError> {
            Ok(snapshot())
        }
        async fn beat_replica_state(&self, state: &ReplicaState) -> Result<(), StoreError> {
            self.0.lock().unwrap().push(state.clone());
            Ok(())
        }
    }

    fn beats(store: &Beats) -> usize {
        store.0.lock().unwrap().len()
    }

    /// The state row names the key mode, and opens the secrets key only in
    /// `Sealed` mode; never a key or a key id.
    #[test]
    fn a_replica_state_names_its_key_mode() {
        #[rustfmt::skip]
        let names = [
            (KeyMode::NoInstanceKey(NoKeyReason::Unwritable), "no_instance_key"),
            (KeyMode::NoInstanceKey(NoKeyReason::Unreadable), "no_instance_key"),
            (KeyMode::Direct,                                 "direct"),
            (KeyMode::AwaitingSecretsKey,                     "awaiting_secrets_key"),
            (KeyMode::Sealed,                                 "sealed"),
            (KeyMode::Unopenable,                             "unopenable"),
        ];
        for (mode, name) in names {
            assert_eq!(key_mode_name(mode), name, "{mode:?}");
        }
        let at = datetime!(2026-10-21 14:02 UTC);
        let state = replica_state("replica-a", &judged(), at);
        assert_eq!(
            state,
            ReplicaState {
                replica: "replica-a".to_string(),
                applied_version: 1,
                key_mode: "no_instance_key".to_string(),
                opens_secrets_key: false,
                beat_at: at,
            }
        );
    }

    /// With nothing to apply, a tick beats on the first tick and then once
    /// ten seconds have passed since the last beat.
    #[tokio::test]
    async fn a_tick_with_nothing_to_apply_beats_every_ten_seconds() {
        let store = Arc::new(Beats::default());
        let clock = crate::clock::MockClock::at(datetime!(2026-10-21 14:02 UTC));
        let mut reload = Reload::new(
            store.clone(),
            IntegrationsHandle::new(judged()),
            clock.clone(),
        );
        reload.tick().await;
        assert_eq!(beats(&store), 1, "the first tick beats");
        clock.advance(std::time::Duration::from_secs(9));
        reload.tick().await;
        assert_eq!(beats(&store), 1, "nine seconds on");
        clock.advance(std::time::Duration::from_secs(1));
        reload.tick().await;
        assert_eq!(beats(&store), 2, "ten seconds on");
    }

    /// A nudge is due at once, then not again within a second.
    #[test]
    fn a_nudge_is_due_once_a_second() {
        let clock = crate::clock::MockClock::at(datetime!(2026-10-21 14:02 UTC));
        let mut reload = Reload::new(
            Arc::new(Beats::default()),
            IntegrationsHandle::new(judged()),
            clock.clone(),
        );
        assert!(reload.nudge_due(), "the first");
        assert!(!reload.nudge_due(), "at once again");
        clock.advance(std::time::Duration::from_secs(1));
        assert!(reload.nudge_due(), "a second later");
    }

    /// The reload task ticks as soon as it runs.
    #[tokio::test]
    async fn the_reload_task_ticks_when_it_runs() {
        let store = Arc::new(Beats::default());
        let clock = crate::clock::MockClock::at(datetime!(2026-10-21 14:02 UTC));
        let reload = Reload::new(store.clone(), IntegrationsHandle::new(judged()), clock);
        let task = tokio::spawn(reload.run());
        let ticked = tokio::time::timeout(std::time::Duration::from_secs(10), async {
            while beats(&store) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await;
        task.abort();
        assert!(ticked.is_ok(), "no beat within ten seconds");
    }
}
