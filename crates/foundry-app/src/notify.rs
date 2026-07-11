//! Notification port + fan-out dispatcher (notification-delivery-providers).
//!
//! Generalizes the single hard-wired email sender into a config-selected set of
//! [`NotificationProvider`]s behind an infallible fan-out [`Notifier`]. A member
//! action builds ONE structured, vendor-neutral [`Notification`] and calls
//! [`Notifier::notify`]; the notifier delivers it best-effort to every active
//! provider. A provider erroring is contained — it never fails the originating
//! request (NFR-5) and never stalls the others.
//!
//! ADR-001: the port carries a structured `Notification`, not an email-centric
//! `send(to, subject, body)` — providers render it. ADR-003: `notify()` is
//! INFALLIBLE (at N=1 a simple sequential await; the full `JoinSet` concurrency +
//! per-provider timeout lands in slice 03). ADR-005: `NotificationEvent` is a
//! closed enum. ADR-006: the port has NO `Debug` supertrait and no secret ever
//! appears in a log line, error, or debug output.

use async_trait::async_trait;
use std::sync::Arc;

/// The closed catalogue of notification-triggering events. Each variant's
/// [`NotificationEvent::as_str`] is the bounded metric-label value (ADR-004/005).
/// Slice 01 needs the three events the existing call sites emit; later slices add
/// `MemberRemoved` + `PasswordChanged`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationEvent {
    PasswordReset,
    WorkspaceInvite,
    MemberInvite,
}

impl NotificationEvent {
    /// The bounded metric-label value for this event.
    pub fn as_str(&self) -> &'static str {
        match self {
            NotificationEvent::PasswordReset => "password_reset",
            NotificationEvent::WorkspaceInvite => "workspace_invite",
            NotificationEvent::MemberInvite => "member_invite",
        }
    }
}

/// The closed set of delivery channels. [`ProviderKind::as_str`] is the bounded
/// `provider` metric-label value (ADR-004).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Log,
    Smtp,
    Webhook,
    EmailApi,
}

impl ProviderKind {
    /// The bounded metric-label value for this provider.
    pub fn as_str(&self) -> &'static str {
        match self {
            ProviderKind::Log => "log",
            ProviderKind::Smtp => "smtp",
            ProviderKind::Webhook => "webhook",
            ProviderKind::EmailApi => "email_api",
        }
    }
}

/// A structured, vendor-neutral notification (ADR-001). Providers render it into
/// their transport's shape. NOT `Debug`: `body` may carry a secret (e.g. a reset
/// token), so the type is deliberately un-derivable to keep it out of debug output
/// (ADR-006).
#[derive(Clone)]
pub struct Notification {
    pub event: NotificationEvent,
    pub recipient: String,
    pub subject: String,
    pub body: String,
}

/// A delivery failure. Messages are secret-free by construction (ADR-006) — never
/// interpolate a configured secret or a reset token into an error.
#[derive(Debug, Clone)]
pub enum DeliveryError {
    /// A transient condition (unreachable relay, timeout) — retriable in principle.
    Transient(String),
    /// A permanent rejection (bad request, auth refused) — not retriable.
    Permanent(String),
}

impl std::fmt::Display for DeliveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeliveryError::Transient(msg) => write!(f, "transient delivery failure: {msg}"),
            DeliveryError::Permanent(msg) => write!(f, "permanent delivery failure: {msg}"),
        }
    }
}

impl std::error::Error for DeliveryError {}

/// The driven port every delivery channel implements. Bound `Send + Sync +
/// 'static` for fan-out across the async runtime. Deliberately NO `Debug`
/// supertrait (ADR-006): a provider may hold a `SecretString`, and a `Debug`
/// bound would invite leaking it.
#[async_trait]
pub trait NotificationProvider: Send + Sync + 'static {
    /// Deliver one notification through this channel.
    async fn deliver(&self, notification: &Notification) -> Result<(), DeliveryError>;

    /// This provider's channel kind (the bounded `provider` metric label).
    fn kind(&self) -> ProviderKind;

    /// Startup health probe (wire → probe → use). A provider is admitted to the
    /// active set only after `probe()` returns `Ok`.
    async fn probe(&self) -> Result<(), DeliveryError>;
}

/// The fan-out dispatcher. Holds the ordered active provider set and delivers a
/// notification best-effort to each. `notify()` is INFALLIBLE (ADR-003): a
/// provider erroring is logged and contained — it never propagates to the caller.
pub struct Notifier {
    providers: Vec<Arc<dyn NotificationProvider>>,
}

impl Notifier {
    /// Build a notifier over an ordered active provider set.
    pub fn new(providers: Vec<Arc<dyn NotificationProvider>>) -> Self {
        Self { providers }
    }

    /// A notifier with no active providers — delivery is a silent no-op.
    pub fn empty() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// The kinds of the active providers, in order. Lets a caller (and tests)
    /// observe which channels this notifier will fan out to.
    pub fn active_kinds(&self) -> Vec<ProviderKind> {
        self.providers.iter().map(|p| p.kind()).collect()
    }

    /// Deliver `notification` best-effort to every active provider. Infallible: a
    /// provider error is logged at `warn` and contained, so the originating
    /// request is never failed or stalled by a delivery problem (NFR-5, ADR-003).
    pub async fn notify(&self, notification: &Notification) {
        for provider in &self.providers {
            if let Err(err) = provider.deliver(notification).await {
                tracing::warn!(
                    provider = provider.kind().as_str(),
                    event = notification.event.as_str(),
                    %err,
                    "notification delivery failed (best-effort, contained)"
                );
            }
        }
    }
}

/// The `log` channel: emit ONE structured, secret-free stdout line per delivery,
/// keyed on provider + event + recipient. NEVER interpolates the reset token or
/// any secret (the body is not logged — slice 01 security scenario).
pub struct LogProvider;

impl LogProvider {
    pub fn new() -> Self {
        Self
    }

    /// Render the single structured log line for a delivery. Deliberately carries
    /// ONLY the safe fields (provider, event, recipient) — never `subject`/`body`,
    /// which may embed a secret token (ADR-006).
    fn render_line(notification: &Notification) -> String {
        format!(
            "notification.delivered provider=log event={} recipient={}",
            notification.event.as_str(),
            notification.recipient,
        )
    }
}

impl Default for LogProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl NotificationProvider for LogProvider {
    async fn deliver(&self, notification: &Notification) -> Result<(), DeliveryError> {
        println!("{}", Self::render_line(notification));
        Ok(())
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::Log
    }

    async fn probe(&self) -> Result<(), DeliveryError> {
        Ok(())
    }
}

/// Build the active [`Notifier`] at the composition root from the
/// `NOTIFICATION_PROVIDERS` env var (ADR-002): a comma-separated channel list.
/// Unset/empty ⇒ an empty notifier (delivery inactive). Each listed channel is
/// constructed, probed (wire → probe → use), and admitted only on a passing probe.
/// An unknown channel name fails fast.
pub async fn build_notifier() -> anyhow::Result<Notifier> {
    let spec = std::env::var("NOTIFICATION_PROVIDERS").unwrap_or_default();
    build_notifier_from(&spec).await
}

/// The pure-ish core of [`build_notifier`]: parse a channel-list spec and build
/// the notifier. Separated from the env read so it is directly unit-testable.
async fn build_notifier_from(spec: &str) -> anyhow::Result<Notifier> {
    let mut providers: Vec<Arc<dyn NotificationProvider>> = Vec::new();
    for name in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        match name {
            "log" => {
                let provider = LogProvider::new();
                provider.probe().await.map_err(|err| {
                    anyhow::anyhow!("provider 'log' failed its startup probe: {err}")
                })?;
                providers.push(Arc::new(provider));
            }
            other => {
                anyhow::bail!("unknown notification provider '{other}' (known providers: log)")
            }
        }
    }
    Ok(Notifier::new(providers))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_and_provider_labels_are_bounded_snake_case() {
        // The metric-label values are the bounded, snake_case contract other
        // slices assert on (ADR-004/005). A rename here breaks the label domain.
        assert_eq!(NotificationEvent::PasswordReset.as_str(), "password_reset");
        assert_eq!(
            NotificationEvent::WorkspaceInvite.as_str(),
            "workspace_invite"
        );
        assert_eq!(NotificationEvent::MemberInvite.as_str(), "member_invite");
        assert_eq!(ProviderKind::Log.as_str(), "log");
        assert_eq!(ProviderKind::Smtp.as_str(), "smtp");
        assert_eq!(ProviderKind::Webhook.as_str(), "webhook");
        assert_eq!(ProviderKind::EmailApi.as_str(), "email_api");
    }

    #[tokio::test]
    async fn build_notifier_admits_the_log_channel_and_leaves_unset_inactive() {
        // Unset/empty ⇒ inactive (no providers). Listing "log" admits exactly the
        // Log channel after its probe passes (wire → probe → use, ADR-002).
        let inactive = build_notifier_from("").await.expect("empty spec builds");
        assert!(inactive.active_kinds().is_empty());

        let logging = build_notifier_from("log").await.expect("log spec builds");
        assert_eq!(logging.active_kinds(), vec![ProviderKind::Log]);
    }

    #[test]
    fn log_line_carries_provider_event_recipient_but_never_a_secret() {
        // The delivery log line keys on the safe fields only. A reset token living
        // in the body MUST NOT reach the line (ADR-006 no-secret-leak).
        let notification = Notification {
            event: NotificationEvent::PasswordReset,
            recipient: "maria.santos@acme.example".to_string(),
            subject: "Reset your Foundry password".to_string(),
            body: "follow this link ?token=SUPER_SECRET_RESET_TOKEN".to_string(),
        };
        let line = LogProvider::render_line(&notification);
        assert!(line.contains("provider=log"));
        assert!(line.contains("event=password_reset"));
        assert!(line.contains("recipient=maria.santos@acme.example"));
        assert!(
            !line.contains("SUPER_SECRET_RESET_TOKEN"),
            "the reset token must never appear in the delivery log line: {line}"
        );
    }
}
