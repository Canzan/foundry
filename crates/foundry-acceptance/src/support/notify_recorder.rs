//! In-process recording double for the `NotificationProvider` driven port.
//!
//! Extension Justification (quality-framework Mandate against Parallel
//! Implementations):
//!   WHY-NEW-FILE: crates/foundry-acceptance/src/support/notify_recorder.rs
//!   CLOSEST-EXISTING: crates/foundry-acceptance/src/support/harness.rs (owns the
//!     in-process AppState wiring + the FakeEmailSender field this replaces)
//!   EXTENSION-COST: harness.rs is already ~650 LOC of Postgres-container + HTTP
//!     plumbing whose single responsibility is app spawning; folding a port test
//!     double + a shared recorder into it would overload that module.
//!   PARALLEL-RATIONALE: the recorder is a reusable driven-port double consumed by
//!     FOUR distinct AppState builders (harness.rs, multi_replica_harness.rs ×2,
//!     us_03_backup_restore.rs) and has a different lifecycle (a shared `Arc`
//!     recorder) than the per-scenario harness struct — a shared support module is
//!     the natural single home so all four import ONE double.
//!
//! Per the DISTILL harness boundary: external transports are IN-PROCESS TEST
//! DOUBLES. The recording provider stands in for a real channel (e.g. "log"): it
//! captures every delivery as `{provider, event, outcome, recipient, ...}` so step
//! defs can assert "delivered through the log provider" and "recorded for provider
//! log, event password_reset, outcome delivered". It also preserves the old
//! `FakeEmailSender` query surface (`count_to`/`last_to`/`sent`/`set_failing`) so
//! the pre-existing bootstrap / member-invite / forgot-password scenarios that
//! assert on email delivery stay green through the port generalization.

use async_trait::async_trait;
use foundry_app::{DeliveryError, Notification, NotificationProvider, Notifier, ProviderKind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// One captured delivery observed at the provider boundary.
#[derive(Debug, Clone)]
pub struct RecordedDelivery {
    /// The provider kind's label (`log`, `smtp`, ...).
    pub provider: String,
    /// The event's label (`password_reset`, ...).
    pub event: String,
    /// The binary outcome (`delivered` | `failed`).
    pub outcome: String,
    /// The notification recipient (kept as `to` for the legacy email assertions).
    pub to: String,
    /// The rendered subject.
    pub subject: String,
    /// The rendered body (carries the reset/invite link for the legacy body
    /// assertions).
    pub body: String,
}

/// Shared, thread-safe recorder every [`RecordingProvider`] writes into. Held as
/// an `Arc` on the harness so step defs read it after driving an app flow.
#[derive(Default)]
pub struct DeliveryRecorder {
    inner: Mutex<Vec<RecordedDelivery>>,
    failing: AtomicBool,
}

impl DeliveryRecorder {
    /// A fresh, empty recorder.
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Put every provider sharing this recorder into failure mode: subsequent
    /// deliveries record NOTHING and return `Err` (modelling a transport outage,
    /// matching the old `FakeEmailSender::set_failing`).
    pub fn set_failing(&self) {
        self.failing.store(true, Ordering::SeqCst);
    }

    fn is_failing(&self) -> bool {
        self.failing.load(Ordering::SeqCst)
    }

    fn record(&self, delivery: RecordedDelivery) {
        self.inner
            .lock()
            .expect("delivery recorder mutex")
            .push(delivery);
    }

    /// Every delivery observed so far.
    pub fn sent(&self) -> Vec<RecordedDelivery> {
        self.inner.lock().expect("delivery recorder mutex").clone()
    }

    /// Count of deliveries to a given recipient (legacy email assertion surface).
    pub fn count_to(&self, addr: &str) -> usize {
        self.sent().iter().filter(|d| d.to == addr).count()
    }

    /// The most recent delivery to a given recipient (legacy email assertion).
    pub fn last_to(&self, addr: &str) -> Option<RecordedDelivery> {
        self.sent().into_iter().rev().find(|d| d.to == addr)
    }

    /// Count of deliveries recorded for the given `(provider, event, outcome)`.
    pub fn recorded(&self, provider: &str, event: &str, outcome: &str) -> usize {
        self.sent()
            .iter()
            .filter(|d| d.provider == provider && d.event == event && d.outcome == outcome)
            .count()
    }

    /// Count of successful deliveries observed through a given provider.
    pub fn delivered_through(&self, provider: &str) -> usize {
        self.sent()
            .iter()
            .filter(|d| d.provider == provider && d.outcome == "delivered")
            .count()
    }
}

/// A `NotificationProvider` that records every delivery into a shared
/// [`DeliveryRecorder`]. Stands in for the real transport of `kind`.
pub struct RecordingProvider {
    kind: ProviderKind,
    recorder: Arc<DeliveryRecorder>,
}

impl RecordingProvider {
    pub fn new(kind: ProviderKind, recorder: Arc<DeliveryRecorder>) -> Self {
        Self { kind, recorder }
    }
}

#[async_trait]
impl NotificationProvider for RecordingProvider {
    async fn deliver(&self, notification: &Notification) -> Result<(), DeliveryError> {
        if self.recorder.is_failing() {
            // Transport outage: record nothing and fail. The infallible notifier
            // contains this — the originating request is unaffected.
            return Err(DeliveryError::Transient(
                "recording provider: induced outage (test)".to_string(),
            ));
        }
        self.recorder.record(RecordedDelivery {
            provider: self.kind.as_str().to_string(),
            event: notification.event.as_str().to_string(),
            outcome: "delivered".to_string(),
            to: notification.recipient.clone(),
            subject: notification.subject.clone(),
            body: notification.body.clone(),
        });
        Ok(())
    }

    fn kind(&self) -> ProviderKind {
        self.kind
    }

    async fn probe(&self) -> Result<(), DeliveryError> {
        Ok(())
    }
}

/// Build an `Arc<Notifier>` whose active providers are [`RecordingProvider`]s —
/// one per `kind`, all recording into the shared `recorder`.
pub fn notifier_from_recorder(
    recorder: &Arc<DeliveryRecorder>,
    kinds: &[ProviderKind],
) -> Arc<Notifier> {
    let providers = kinds
        .iter()
        .map(|kind| {
            Arc::new(RecordingProvider::new(*kind, recorder.clone()))
                as Arc<dyn NotificationProvider>
        })
        .collect();
    Arc::new(Notifier::new(providers))
}
