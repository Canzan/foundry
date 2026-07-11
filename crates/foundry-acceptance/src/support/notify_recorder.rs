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
use foundry_app::{
    DeliveryError, EmailApiConfig, EmailApiProvider, Notification, NotificationProvider, Notifier,
    ProviderKind, WebhookConfig, WebhookProvider,
};
use std::collections::HashSet;
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
    /// Provider kinds whose transport is "unreachable": a delivery through one
    /// of these is RECORDED with `outcome=failed` and then returns `Err`,
    /// modelling a temporarily-down relay (US-02 failure isolation). Distinct
    /// from `failing` (a full outage that records NOTHING), and scoped per
    /// provider so a fan-out slice can down one channel while its siblings still
    /// deliver.
    unreachable: Mutex<HashSet<&'static str>>,
    /// Provider kinds whose transport "hangs on connect": a delivery through one
    /// RECORDS `outcome=failed` and then sleeps past any realistic per-provider
    /// timeout, so the notifier's timeout wrapper fires and CONTAINS the stall
    /// (US-03 slow-provider isolation). The record is written BEFORE the sleep so
    /// it survives the notifier cancelling the timed-out `deliver()` future.
    slow: Mutex<HashSet<&'static str>>,
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

    /// Mark `kind`'s transport as unreachable: subsequent deliveries through a
    /// provider of that kind record `outcome=failed` and return `Err`, so the
    /// infallible notifier contains the failure without failing the request.
    pub fn set_unreachable(&self, kind: ProviderKind) {
        self.unreachable
            .lock()
            .expect("delivery recorder unreachable set")
            .insert(kind.as_str());
    }

    fn is_unreachable(&self, kind: ProviderKind) -> bool {
        self.unreachable
            .lock()
            .expect("delivery recorder unreachable set")
            .contains(kind.as_str())
    }

    /// Mark `kind`'s transport as hanging on connect: a delivery through a
    /// provider of that kind records `outcome=failed` then blocks past the
    /// notifier's per-provider timeout, so the concurrent fan-out contains the
    /// stall (the request is never made to wait on it).
    pub fn set_slow(&self, kind: ProviderKind) {
        self.slow
            .lock()
            .expect("delivery recorder slow set")
            .insert(kind.as_str());
    }

    fn is_slow(&self, kind: ProviderKind) -> bool {
        self.slow
            .lock()
            .expect("delivery recorder slow set")
            .contains(kind.as_str())
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
            // Full transport outage: record nothing and fail. The infallible
            // notifier contains this — the originating request is unaffected.
            return Err(DeliveryError::Transient(
                "recording provider: induced outage (test)".to_string(),
            ));
        }
        if self.recorder.is_slow(self.kind) {
            // Hangs on connect (US-03): the attempt is OBSERVED and recorded as a
            // timeout `failed`, THEN the future blocks well past the notifier's
            // per-provider timeout. The notifier drops this future on timeout —
            // the record (written first) survives, and the request never waits
            // out the full block.
            self.recorder.record(RecordedDelivery {
                provider: self.kind.as_str().to_string(),
                event: notification.event.as_str().to_string(),
                outcome: "failed".to_string(),
                to: notification.recipient.clone(),
                subject: notification.subject.clone(),
                body: notification.body.clone(),
            });
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            return Err(DeliveryError::Transient(
                "recording provider: endpoint hangs on connect (test)".to_string(),
            ));
        }
        if self.recorder.is_unreachable(self.kind) {
            // Temporarily-unreachable relay (US-02): the notifier still OBSERVES
            // the attempt, so record it as `failed`, then return the transient
            // error the notifier contains (the request is never failed/stalled).
            self.recorder.record(RecordedDelivery {
                provider: self.kind.as_str().to_string(),
                event: notification.event.as_str().to_string(),
                outcome: "failed".to_string(),
                to: notification.recipient.clone(),
                subject: notification.subject.clone(),
                body: notification.body.clone(),
            });
            return Err(DeliveryError::Transient(
                "recording provider: endpoint unreachable (test)".to_string(),
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
    // A SHORT per-provider timeout keeps the slow-provider isolation scenario
    // fast: an in-memory recording delivery completes far inside it, while the
    // "hangs on connect" double (which sleeps 5s) is bounded to this window.
    Arc::new(Notifier::new(providers).with_delivery_timeout(std::time::Duration::from_millis(500)))
}

/// The `webhook` channel wired with the SHIPPED [`WebhookProvider`] (a real
/// reqwest POST to the local receiver double) decorated so the delivery is ALSO
/// recorded into the shared [`DeliveryRecorder`]. The happy path thus asserts
/// BOTH a genuine POST (observed at the receiver) and the per-provider/event/
/// outcome record (at the recorder), through one delivery — a real `@real-io`
/// exercise of the production adapter, not a stand-in double.
pub struct RecordingWebhookProvider {
    inner: WebhookProvider,
    recorder: Arc<DeliveryRecorder>,
}

impl RecordingWebhookProvider {
    pub fn new(inner: WebhookProvider, recorder: Arc<DeliveryRecorder>) -> Self {
        Self { inner, recorder }
    }
}

#[async_trait]
impl NotificationProvider for RecordingWebhookProvider {
    async fn deliver(&self, notification: &Notification) -> Result<(), DeliveryError> {
        let result = self.inner.deliver(notification).await;
        let outcome = if result.is_ok() {
            "delivered"
        } else {
            "failed"
        };
        self.recorder.record(RecordedDelivery {
            provider: ProviderKind::Webhook.as_str().to_string(),
            event: notification.event.as_str().to_string(),
            outcome: outcome.to_string(),
            to: notification.recipient.clone(),
            subject: notification.subject.clone(),
            body: notification.body.clone(),
        });
        result
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::Webhook
    }

    async fn probe(&self) -> Result<(), DeliveryError> {
        self.inner.probe().await
    }
}

/// The `email_api` channel wired with the SHIPPED [`EmailApiProvider`] (a real
/// reqwest POST to the local vendor receiver double, carrying the key on the
/// Authorization credential header) decorated so the delivery is ALSO recorded
/// into the shared [`DeliveryRecorder`]. The happy path thus asserts BOTH a
/// genuine POST (observed at the vendor receiver) and the per-provider/event/
/// outcome record (at the recorder), through one delivery — a real `@real-io`
/// exercise of the production adapter, not a stand-in double.
pub struct RecordingEmailApiProvider {
    inner: EmailApiProvider,
    recorder: Arc<DeliveryRecorder>,
}

impl RecordingEmailApiProvider {
    pub fn new(inner: EmailApiProvider, recorder: Arc<DeliveryRecorder>) -> Self {
        Self { inner, recorder }
    }
}

#[async_trait]
impl NotificationProvider for RecordingEmailApiProvider {
    async fn deliver(&self, notification: &Notification) -> Result<(), DeliveryError> {
        let result = self.inner.deliver(notification).await;
        let outcome = if result.is_ok() {
            "delivered"
        } else {
            "failed"
        };
        self.recorder.record(RecordedDelivery {
            provider: ProviderKind::EmailApi.as_str().to_string(),
            event: notification.event.as_str().to_string(),
            outcome: outcome.to_string(),
            to: notification.recipient.clone(),
            subject: notification.subject.clone(),
            body: notification.body.clone(),
        });
        result
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::EmailApi
    }

    async fn probe(&self) -> Result<(), DeliveryError> {
        self.inner.probe().await
    }
}

/// A fixed hosted-email-API key handed to the harness's shipped
/// [`EmailApiProvider`]. NOT a production secret — it exists only so the real
/// adapter carries a credential header on its POST to the local vendor receiver.
pub const HARNESS_EMAIL_API_KEY: &str = "ndp-harness-email-api-key-value";

/// Build an `Arc<Notifier>` over `kinds`, recording every delivery into the
/// shared `recorder`. A `Webhook` kind is wired with the SHIPPED
/// [`WebhookProvider`] pointed at `webhook_url` (a real POST to the local
/// receiver, optionally HMAC-signed by `webhook_secret`); an `EmailApi` kind is
/// wired with the SHIPPED [`EmailApiProvider`] pointed at `email_api_url` (a real
/// POST to the local vendor receiver, keyed by `HARNESS_EMAIL_API_KEY`); every
/// other kind is an in-memory [`RecordingProvider`]. Each provider is PROBED here
/// (wire → probe → use) so the webhook/email_api no-side-effect probe scenarios
/// genuinely exercise `probe()`.
pub async fn notifier_for_kinds(
    recorder: &Arc<DeliveryRecorder>,
    kinds: &[ProviderKind],
    webhook_url: Option<&str>,
    webhook_secret: Option<String>,
    email_api_url: Option<&str>,
) -> Arc<Notifier> {
    let mut providers: Vec<Arc<dyn NotificationProvider>> = Vec::new();
    for kind in kinds {
        match kind {
            ProviderKind::EmailApi => {
                let url = email_api_url
                    .expect("an EmailApi kind requires the local vendor receiver URL")
                    .to_string();
                let config = EmailApiConfig::from_lookup(|key| match key {
                    "EMAIL_API_URL" => Some(url.clone()),
                    "EMAIL_API_KEY" => Some(HARNESS_EMAIL_API_KEY.to_string()),
                    "EMAIL_API_FROM" => Some("noreply@acme.example".to_string()),
                    _ => None,
                })
                .expect("email_api config parses in the harness");
                let inner = EmailApiProvider::new(config).expect("email_api provider builds");
                // Wire → probe → use: exercise the real host-reachability probe so
                // admission connects (but never sends) to the vendor receiver.
                inner
                    .probe()
                    .await
                    .expect("email_api probe reaches the local vendor receiver");
                providers.push(Arc::new(RecordingEmailApiProvider::new(
                    inner,
                    recorder.clone(),
                )));
            }
            ProviderKind::Webhook => {
                let url = webhook_url
                    .expect("a Webhook kind requires the local receiver URL")
                    .to_string();
                let secret = webhook_secret.clone();
                let config = WebhookConfig::from_lookup(|key| match key {
                    "WEBHOOK_URL" => Some(url.clone()),
                    "WEBHOOK_SIGNING_SECRET" => secret.clone(),
                    _ => None,
                })
                .expect("webhook config parses in the harness");
                let inner = WebhookProvider::new(config).expect("webhook provider builds");
                // Wire → probe → use: exercise the real host-reachability probe so
                // the "probe makes no POST" scenario asserts against a probe that
                // actually ran (it connects but never POSTs, N-ODD-3).
                inner
                    .probe()
                    .await
                    .expect("webhook probe reaches the local receiver");
                providers.push(Arc::new(RecordingWebhookProvider::new(
                    inner,
                    recorder.clone(),
                )));
            }
            other => {
                providers.push(Arc::new(RecordingProvider::new(*other, recorder.clone()))
                    as Arc<dyn NotificationProvider>);
            }
        }
    }
    Arc::new(Notifier::new(providers).with_delivery_timeout(std::time::Duration::from_millis(500)))
}
