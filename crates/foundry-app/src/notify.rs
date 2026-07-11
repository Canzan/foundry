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
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use secrecy::{ExposeSecret, SecretString};
use std::sync::Arc;

/// ADR-002: SMTP submission port default. Operators override via `SMTP_PORT`.
const SMTP_DEFAULT_PORT: u16 = 587;

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
    /// which may embed a secret token (ADR-006). Public so the acceptance security
    /// scenario can assert the delivered line carries no reset token or secret.
    pub fn log_line(notification: &Notification) -> String {
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
        println!("{}", Self::log_line(notification));
        Ok(())
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::Log
    }

    async fn probe(&self) -> Result<(), DeliveryError> {
        Ok(())
    }
}

/// Validated SMTP relay settings (ADR-002/006). `password` is a [`SecretString`]
/// — read once, exposed ONLY at credential construction, never logged nor
/// Debug-printed. This type deliberately does NOT derive `Debug`, so the password
/// has no debug-leak vector anywhere it is held.
pub struct SmtpConfig {
    host: String,
    port: u16,
    username: String,
    password: SecretString,
    from: String,
}

impl SmtpConfig {
    /// Parse from the process environment: `SMTP_HOST`, `SMTP_PORT` (default
    /// [`SMTP_DEFAULT_PORT`]), `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_FROM`.
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// Parse from an arbitrary key→value lookup (so callers and tests need no
    /// global env mutation). A missing/blank required setting fails fast, naming
    /// the provider AND the offending setting (ADR-002 / NFR-1) — the message is
    /// secret-free by construction (it never echoes a value).
    pub fn from_lookup<F: Fn(&str) -> Option<String>>(get: F) -> anyhow::Result<Self> {
        let required = |key: &str| -> anyhow::Result<String> {
            match get(key) {
                Some(value) if !value.trim().is_empty() => Ok(value),
                _ => anyhow::bail!("provider 'smtp' is missing required setting '{key}'"),
            }
        };
        let host = required("SMTP_HOST")?;
        let username = required("SMTP_USERNAME")?;
        let password = SecretString::new(required("SMTP_PASSWORD")?.into());
        let from = required("SMTP_FROM")?;
        let port = match get("SMTP_PORT") {
            Some(raw) if !raw.trim().is_empty() => raw.trim().parse::<u16>().map_err(|_| {
                // Secret-free: SMTP_PORT is not a secret, so echoing it is safe.
                anyhow::anyhow!("provider 'smtp' setting 'SMTP_PORT' is not a valid port: {raw}")
            })?,
            _ => SMTP_DEFAULT_PORT,
        };
        Ok(Self {
            host,
            port,
            username,
            password,
            from,
        })
    }
}

/// The `smtp` channel (ADR-001/002): renders the structured [`Notification`] into
/// an email and delivers it through an SMTP relay via lettre's async STARTTLS
/// transport. Holds the relay credentials inside lettre's transport (built from a
/// [`SecretString`]); NOT `Debug` (ADR-006) — the password has no debug-leak
/// vector. All [`DeliveryError`] messages are hand-built and secret-free.
pub struct SmtpProvider {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: String,
}

impl SmtpProvider {
    /// Build the STARTTLS relay transport from validated config. The password is
    /// exposed exactly ONCE here (credential construction) and thereafter lives
    /// only inside lettre's transport — never surfaced again on any path.
    pub fn new(config: SmtpConfig) -> Result<Self, DeliveryError> {
        let credentials = Credentials::new(
            config.username.clone(),
            config.password.expose_secret().to_string(),
        );
        let transport = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.host)
            .map_err(|_| {
                DeliveryError::Permanent("smtp relay transport could not be built".to_string())
            })?
            .port(config.port)
            .credentials(credentials)
            .build();
        Ok(Self {
            transport,
            from: config.from,
        })
    }
}

#[async_trait]
impl NotificationProvider for SmtpProvider {
    async fn deliver(&self, notification: &Notification) -> Result<(), DeliveryError> {
        let email = Message::builder()
            .from(self.from.parse().map_err(|_| {
                DeliveryError::Permanent("smtp 'from' address is invalid".to_string())
            })?)
            .to(notification.recipient.parse().map_err(|_| {
                DeliveryError::Permanent("smtp recipient address is invalid".to_string())
            })?)
            .subject(notification.subject.clone())
            .body(notification.body.clone())
            .map_err(|_| {
                DeliveryError::Permanent("smtp message could not be constructed".to_string())
            })?;
        // Hand-built, secret-free classification (ADR-006): never interpolate the
        // raw transport error (or any credential) into the delivery error.
        self.transport.send(email).await.map_err(|err| {
            if err.is_permanent() {
                DeliveryError::Permanent("smtp relay rejected the message".to_string())
            } else {
                DeliveryError::Transient("smtp relay unreachable or errored".to_string())
            }
        })?;
        Ok(())
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::Smtp
    }

    async fn probe(&self) -> Result<(), DeliveryError> {
        // Startup reachability probe: a TLS handshake only, NO `MAIL FROM`
        // (design note — a handshake needs no envelope). Secret-free errors.
        match self.transport.test_connection().await {
            Ok(true) => Ok(()),
            Ok(false) => Err(DeliveryError::Transient(
                "smtp relay did not accept the connection".to_string(),
            )),
            Err(err) if err.is_permanent() => Err(DeliveryError::Permanent(
                "smtp relay refused the connection".to_string(),
            )),
            Err(_) => Err(DeliveryError::Transient(
                "smtp relay unreachable".to_string(),
            )),
        }
    }
}

/// Build the active [`Notifier`] at the composition root from the
/// `NOTIFICATION_PROVIDERS` env var (ADR-002): a comma-separated channel list.
/// Unset/empty ⇒ an empty notifier (delivery inactive). Each listed channel is
/// constructed, probed (wire → probe → use), and admitted only on a passing probe.
/// An unknown channel name fails fast.
pub async fn build_notifier() -> anyhow::Result<Notifier> {
    let spec = std::env::var("NOTIFICATION_PROVIDERS").unwrap_or_default();
    build_notifier_from(&spec, |key| std::env::var(key).ok()).await
}

/// The pure-ish core of [`build_notifier`]: parse a channel-list spec and build
/// the notifier, reading each provider's settings through `get` (the env lookup
/// in production; an injected map in tests). Separated from the env read so it is
/// directly unit-testable without global env mutation.
async fn build_notifier_from<F: Fn(&str) -> Option<String>>(
    spec: &str,
    get: F,
) -> anyhow::Result<Notifier> {
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
            "smtp" => {
                // Wire → probe → use (ADR-002): parse+validate the SMTP_* settings
                // (fail-fast naming the missing key), construct the relay
                // transport, and admit the channel only after its TLS-handshake
                // probe passes. Every error here is secret-free (ADR-006).
                let config = SmtpConfig::from_lookup(&get)?;
                let provider = SmtpProvider::new(config).map_err(|err| {
                    anyhow::anyhow!("provider 'smtp' could not be constructed: {err}")
                })?;
                provider.probe().await.map_err(|err| {
                    anyhow::anyhow!("provider 'smtp' failed its startup probe: {err}")
                })?;
                providers.push(Arc::new(provider));
            }
            other => {
                // Fail fast on a typo (ADR-002 / NFR-1). Name the offending
                // channel AND the full bounded known set so the operator can
                // fix a fat-fingered `logg`. Secret-free by construction.
                anyhow::bail!(
                    "unknown notification provider '{other}' \
                     (known: log, smtp, webhook, email_api)"
                )
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
        let inactive = build_notifier_from("", |_| None)
            .await
            .expect("empty spec builds");
        assert!(inactive.active_kinds().is_empty());

        let logging = build_notifier_from("log", |_| None)
            .await
            .expect("log spec builds");
        assert_eq!(logging.active_kinds(), vec![ProviderKind::Log]);
    }

    #[tokio::test]
    async fn build_notifier_rejects_an_unknown_channel_naming_it_and_the_known_set() {
        // An unknown/typo'd channel name fails fast (ADR-002 / NFR-1). The
        // operator-facing error must name BOTH the offending name and the full
        // bounded known set {log, smtp, webhook, email_api} so a fat-fingered
        // "logg" is diagnosable — and it must carry no secret value.
        // `Notifier` is deliberately not `Debug` (ADR-006), so `expect_err`
        // (which needs the `Ok` value to be `Debug`) can't be used here.
        let Err(err) = build_notifier_from("logg", |_| None).await else {
            panic!("an unknown channel must refuse to build");
        };
        let message = format!("{err:#}");
        assert!(
            message.contains("logg"),
            "error must name the unknown provider 'logg': {message}"
        );
        for known in ["log", "smtp", "webhook", "email_api"] {
            assert!(
                message.contains(known),
                "error must name the known provider '{known}': {message}"
            );
        }
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
        let line = LogProvider::log_line(&notification);
        assert!(line.contains("provider=log"));
        assert!(line.contains("event=password_reset"));
        assert!(line.contains("recipient=maria.santos@acme.example"));
        assert!(
            !line.contains("SUPER_SECRET_RESET_TOKEN"),
            "the reset token must never appear in the delivery log line: {line}"
        );
    }

    /// A distinctive value that would leak loudly if any layer echoed the SMTP
    /// password. Reused across the no-leak litmus tests + the acceptance step.
    const SMTP_PASSWORD_SENTINEL: &str = "ndp-smtp-password-must-never-leak-9f3a";

    fn smtp_env(password: &str, port: Option<&str>) -> impl Fn(&str) -> Option<String> {
        let password = password.to_string();
        let port = port.map(str::to_string);
        move |key: &str| match key {
            "SMTP_HOST" => Some("relay.acme.example".to_string()),
            "SMTP_USERNAME" => Some("mailer".to_string()),
            "SMTP_PASSWORD" => Some(password.clone()),
            "SMTP_FROM" => Some("noreply@acme.example".to_string()),
            "SMTP_PORT" => port.clone(),
            _ => None,
        }
    }

    #[test]
    fn smtp_config_parses_required_settings_and_defaults_the_port_to_587() {
        // Unset SMTP_PORT ⇒ the ADR-002 submission default (587). An explicit
        // value overrides it. Required strings land on the config verbatim.
        let defaulted =
            SmtpConfig::from_lookup(smtp_env("hunter2", None)).expect("valid smtp config parses");
        assert_eq!(defaulted.host, "relay.acme.example");
        assert_eq!(defaulted.username, "mailer");
        assert_eq!(defaulted.from, "noreply@acme.example");
        assert_eq!(defaulted.port, SMTP_DEFAULT_PORT);

        let overridden = SmtpConfig::from_lookup(smtp_env("hunter2", Some("2525")))
            .expect("explicit port parses");
        assert_eq!(overridden.port, 2525);
    }

    #[test]
    fn smtp_config_missing_a_required_setting_fails_fast_naming_provider_and_setting() {
        // Omit SMTP_HOST while a password is present: the fail-fast error must
        // name BOTH the provider and the offending key, and echo NO secret value
        // (ADR-002 / ADR-006).
        let get = |key: &str| match key {
            "SMTP_USERNAME" => Some("mailer".to_string()),
            "SMTP_PASSWORD" => Some(SMTP_PASSWORD_SENTINEL.to_string()),
            "SMTP_FROM" => Some("noreply@acme.example".to_string()),
            _ => None,
        };
        let Err(err) = SmtpConfig::from_lookup(get) else {
            panic!("a missing required SMTP setting must fail fast");
        };
        let message = format!("{err:#}");
        assert!(
            message.contains("smtp"),
            "error must name provider smtp: {message}"
        );
        assert!(
            message.contains("SMTP_HOST"),
            "error must name the missing setting SMTP_HOST: {message}"
        );
        assert!(
            !message.contains(SMTP_PASSWORD_SENTINEL),
            "the fail-fast error must carry no secret value: {message}"
        );
    }

    #[tokio::test]
    async fn smtp_password_never_appears_in_debug_output_or_a_delivery_error() {
        // The five-layer no-leak litmus at unit scope (ADR-006): the password is
        // held in a SecretString (redacted on Debug), the provider is not Debug,
        // and a real DeliveryError from a closed relay is hand-built + secret-free.
        let config = SmtpConfig::from_lookup(smtp_env(SMTP_PASSWORD_SENTINEL, Some("1")))
            .expect("sentinel smtp config parses");
        let secret_debug = format!("{:?}", config.password);
        assert!(
            !secret_debug.contains(SMTP_PASSWORD_SENTINEL),
            "SecretString must redact the password on Debug: {secret_debug}"
        );

        let provider = SmtpProvider::new(config).expect("smtp provider builds");
        let notification = Notification {
            event: NotificationEvent::PasswordReset,
            recipient: "maria.santos@acme.example".to_string(),
            subject: "Reset your Foundry password".to_string(),
            body: "follow this link ?token=RESET".to_string(),
        };
        // 127.0.0.1:1 is closed → connection refused → a genuine transport error.
        let err = provider
            .deliver(&notification)
            .await
            .expect_err("a closed relay must fail the delivery");
        let rendered = format!("{err} || {err:?}");
        assert!(
            !rendered.contains(SMTP_PASSWORD_SENTINEL),
            "the SMTP password must never appear in a delivery error or its debug output: {rendered}"
        );
    }

    #[tokio::test]
    async fn build_notifier_smtp_branch_rejects_when_the_relay_probe_cannot_connect() {
        // The smtp registry branch is wire → probe → use: with valid config but an
        // unreachable relay (closed localhost port), the startup probe fails and
        // the channel is refused — the error names smtp and leaks no secret.
        let get = smtp_env(SMTP_PASSWORD_SENTINEL, Some("1"));
        let Err(err) = build_notifier_from("smtp", get).await else {
            panic!("smtp with an unreachable relay must fail its startup probe");
        };
        let message = format!("{err:#}");
        assert!(
            message.contains("smtp"),
            "probe failure must name smtp: {message}"
        );
        assert!(
            !message.contains(SMTP_PASSWORD_SENTINEL),
            "the probe-failure error must carry no secret value: {message}"
        );
    }
}
