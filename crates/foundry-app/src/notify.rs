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
//! INFALLIBLE and fans out concurrently over a `tokio::task::JoinSet`, one
//! timeout-wrapped `deliver()` task per provider (await-bounded), emitting the
//! per-provider delivery counter (ADR-004). ADR-005: `NotificationEvent` is a
//! closed enum. ADR-006: the port has NO `Debug` supertrait and no secret ever
//! appears in a log line, error, or debug output.

use async_trait::async_trait;
use hmac::{Hmac, Mac};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use secrecy::{ExposeSecret, SecretString};
use sha2::Sha256;
use std::sync::Arc;
use std::time::Duration;

/// ADR-002: SMTP submission port default. Operators override via `SMTP_PORT`.
const SMTP_DEFAULT_PORT: u16 = 587;

/// ADR-003: default per-provider delivery timeout. One hung/slow provider adds at
/// most this to the (concurrent) emit path, then is contained + counted `failed`.
/// Operators override via `NOTIFICATION_DELIVERY_TIMEOUT_MS`.
const DEFAULT_DELIVERY_TIMEOUT_MS: u64 = 5000;

/// ADR-004: the per-provider delivery counter. Emitted once per provider per
/// notification inside [`Notifier::notify`], labelled by the bounded triple
/// `{provider, event, outcome}`. Register-at-0 + the cardinality guard land in
/// slice 03-02; the increment (emit) is built here.
pub const NOTIFICATION_DELIVERIES_METRIC: &str = "foundry_notification_deliveries_total";

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

    /// The full closed catalogue. The register-at-0 cross-product
    /// ([`delivery_zero_series`]) enumerates over it so every event's series is
    /// present at zero on the first `/metrics` scrape (ADR-004). Slices adding
    /// events (US-06: `member_removed`, `password_changed`) extend this in
    /// lockstep with the enum.
    pub const ALL: [NotificationEvent; 3] = [
        NotificationEvent::PasswordReset,
        NotificationEvent::WorkspaceInvite,
        NotificationEvent::MemberInvite,
    ];
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

    /// The full closed set of channels — the bounded `provider` label domain
    /// (ADR-004). Used by the cardinality guards to assert every emitted
    /// `provider` value stays within this set.
    pub const ALL: [ProviderKind; 4] = [
        ProviderKind::Log,
        ProviderKind::Smtp,
        ProviderKind::Webhook,
        ProviderKind::EmailApi,
    ];
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

impl DeliveryError {
    /// The failure `class` carried in the structured log line (ADR-003/004): a
    /// forward-compat seam for the future durable-retry layer (ADR-007). NOT a
    /// metric label value — the metric `outcome` stays binary `{delivered,failed}`.
    pub fn class(&self) -> &'static str {
        match self {
            DeliveryError::Transient(_) => "transient",
            DeliveryError::Permanent(_) => "permanent",
        }
    }
}

/// The binary delivery outcome (ADR-003/004). The metric `outcome` label domain —
/// a timeout, transient error, permanent error, or contained panic all map to
/// `Failed`; only a clean `Ok(())` is `Delivered`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryOutcome {
    Delivered,
    Failed,
}

impl DeliveryOutcome {
    /// The bounded metric-label value for this outcome.
    pub fn as_str(&self) -> &'static str {
        match self {
            DeliveryOutcome::Delivered => "delivered",
            DeliveryOutcome::Failed => "failed",
        }
    }

    /// The binary outcome domain — the bounded `outcome` label values (ADR-004).
    pub const ALL: [DeliveryOutcome; 2] = [DeliveryOutcome::Delivered, DeliveryOutcome::Failed];
}

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
    /// ADR-003: per-provider delivery timeout applied to each concurrent
    /// `deliver()` task. Bounds the emit-path stall a hung provider can cause.
    delivery_timeout: Duration,
}

impl Notifier {
    /// Build a notifier over an ordered active provider set at the default
    /// per-provider delivery timeout ([`DEFAULT_DELIVERY_TIMEOUT_MS`]).
    pub fn new(providers: Vec<Arc<dyn NotificationProvider>>) -> Self {
        Self {
            providers,
            delivery_timeout: Duration::from_millis(DEFAULT_DELIVERY_TIMEOUT_MS),
        }
    }

    /// Override the per-provider delivery timeout (composition root reads
    /// `NOTIFICATION_DELIVERY_TIMEOUT_MS`; tests use a short window).
    pub fn with_delivery_timeout(mut self, delivery_timeout: Duration) -> Self {
        self.delivery_timeout = delivery_timeout;
        self
    }

    /// A notifier with no active providers — delivery is a silent no-op.
    pub fn empty() -> Self {
        Self::new(Vec::new())
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
        // ADR-003: concurrent fan-out. One timeout-wrapped `deliver()` task per
        // provider in a `JoinSet` — so a slow/hung provider adds at most ONE
        // `delivery_timeout` to the (concurrent) wall-time regardless of N, and a
        // panicking provider is contained in its own task (a `JoinError`) and can
        // never unwind the request. `notify` stays INFALLIBLE.
        let mut set = tokio::task::JoinSet::new();
        for provider in &self.providers {
            let provider = Arc::clone(provider);
            let notification = notification.clone();
            let delivery_timeout = self.delivery_timeout;
            set.spawn(async move {
                let kind = provider.kind();
                let event = notification.event;
                let outcome =
                    match tokio::time::timeout(delivery_timeout, provider.deliver(&notification))
                        .await
                    {
                        Ok(Ok(())) => DeliveryOutcome::Delivered,
                        Ok(Err(err)) => {
                            tracing::warn!(
                                provider = kind.as_str(),
                                event = event.as_str(),
                                outcome = "failed",
                                class = err.class(),
                                %err,
                                "notification delivery failed (best-effort, contained)"
                            );
                            DeliveryOutcome::Failed
                        }
                        Err(_elapsed) => {
                            tracing::warn!(
                                provider = kind.as_str(),
                                event = event.as_str(),
                                outcome = "failed",
                                class = "transient",
                                "notification delivery timed out (best-effort, contained)"
                            );
                            DeliveryOutcome::Failed
                        }
                    };
                (kind, event, outcome)
            });
        }
        // Await-bounded: drain every task before returning so the shipped
        // synchronous delivery assertions (NFR-5) hold. ADR-004: emit the
        // per-provider counter HERE on the caller task (not inside the spawned
        // task) so a scoped/thread-local recorder observes it; the globally
        // installed Prometheus recorder sees it in production. A panicking
        // provider surfaces as a `JoinError` — contained; no v1 consumer branches
        // on the panic path, so it is logged, not counted.
        while let Some(joined) = set.join_next().await {
            match joined {
                Ok((kind, event, outcome)) => {
                    metrics::counter!(
                        NOTIFICATION_DELIVERIES_METRIC,
                        "provider" => kind.as_str(),
                        "event" => event.as_str(),
                        "outcome" => outcome.as_str(),
                    )
                    .increment(1);
                }
                Err(join_error) => {
                    tracing::warn!(
                        %join_error,
                        "notification delivery task panicked (best-effort, contained)"
                    );
                }
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

/// The header carrying the webhook body's HMAC-SHA256 digest (ADR-001/006). The
/// value is `sha256=<hex>` — a one-way derivation of the body under the signing
/// secret. Only the DIGEST ever rides on the wire; the raw secret never does.
const WEBHOOK_SIGNATURE_HEADER: &str = "x-foundry-signature";

/// Validated webhook settings (ADR-002/006). `WEBHOOK_URL` is required; the
/// optional `WEBHOOK_SIGNING_SECRET` is held in a [`SecretString`] and exposed
/// ONLY to the HMAC at request construction — never logged, echoed, nor
/// Debug-printed. This type deliberately does NOT derive `Debug`, so the secret
/// has no debug-leak vector.
pub struct WebhookConfig {
    url: String,
    signing_secret: Option<SecretString>,
}

impl WebhookConfig {
    /// Parse from the process environment: `WEBHOOK_URL` (required),
    /// `WEBHOOK_SIGNING_SECRET` (optional).
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// Parse from an arbitrary key→value lookup (so callers and tests need no
    /// global env mutation). A missing/blank `WEBHOOK_URL` fails fast, naming the
    /// provider AND the offending setting (ADR-002 / NFR-1) — secret-free by
    /// construction (it never echoes a value).
    pub fn from_lookup<F: Fn(&str) -> Option<String>>(get: F) -> anyhow::Result<Self> {
        let url = match get("WEBHOOK_URL") {
            Some(value) if !value.trim().is_empty() => value.trim().to_string(),
            _ => anyhow::bail!("provider 'webhook' is missing required setting 'WEBHOOK_URL'"),
        };
        let signing_secret = get("WEBHOOK_SIGNING_SECRET")
            .filter(|value| !value.trim().is_empty())
            .map(|value| SecretString::new(value.into()));
        Ok(Self {
            url,
            signing_secret,
        })
    }
}

/// Split an `http(s)://host[:port][/path]` URL into `(host, port)` for the
/// reachability probe (no `url` crate — the probe needs only the authority).
/// Defaults the port to 80/443 by scheme when absent.
fn webhook_host_port(url: &str) -> Option<(String, u16)> {
    let (rest, default_port) = url
        .strip_prefix("http://")
        .map(|rest| (rest, 80u16))
        .or_else(|| url.strip_prefix("https://").map(|rest| (rest, 443u16)))?;
    let authority = rest.split('/').next().unwrap_or(rest);
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    match authority.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() => Some((host.to_string(), port.parse().ok()?)),
        Some(_) => None,
        None if !authority.is_empty() => Some((authority.to_string(), default_port)),
        None => None,
    }
}

/// The `webhook` channel (ADR-001/002/006): renders the structured
/// [`Notification`] into a JSON body and POSTs it to `WEBHOOK_URL` over HTTP via
/// reqwest. When a signing secret is configured, the body is signed with
/// HMAC-SHA256 and the digest rides in the `x-foundry-signature` header — the raw
/// secret never leaves the process. NOT `Debug` (ADR-006): the secret has no
/// debug-leak vector. All [`DeliveryError`] messages are hand-built + secret-free.
pub struct WebhookProvider {
    url: String,
    host: String,
    port: u16,
    client: reqwest::Client,
    signing_secret: Option<SecretString>,
}

impl WebhookProvider {
    /// Build the provider from validated config. Parses the reachability
    /// authority once (fail fast on a malformed URL) and constructs the HTTP
    /// client. The signing secret (if any) is retained for per-request HMAC.
    pub fn new(config: WebhookConfig) -> Result<Self, DeliveryError> {
        let (host, port) = webhook_host_port(&config.url).ok_or_else(|| {
            DeliveryError::Permanent("webhook 'WEBHOOK_URL' is not a valid http(s) URL".to_string())
        })?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| {
                DeliveryError::Permanent("webhook HTTP client could not be built".to_string())
            })?;
        Ok(Self {
            url: config.url,
            host,
            port,
            client,
            signing_secret: config.signing_secret,
        })
    }

    /// The `sha256=<hex>` HMAC-SHA256 digest of `body` under `secret`. A one-way
    /// derivation — the raw secret is unrecoverable from it (ADR-006).
    fn sign(secret: &str, body: &str) -> String {
        let mut mac =
            Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
        mac.update(body.as_bytes());
        let hex: String = mac
            .finalize()
            .into_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        format!("sha256={hex}")
    }

    /// Render the vendor-neutral notification into the webhook's JSON body
    /// (ADR-001): `{event, recipient, subject, body}`.
    fn render_body(notification: &Notification) -> String {
        serde_json::json!({
            "event": notification.event.as_str(),
            "recipient": notification.recipient,
            "subject": notification.subject,
            "body": notification.body,
        })
        .to_string()
    }
}

#[async_trait]
impl NotificationProvider for WebhookProvider {
    async fn deliver(&self, notification: &Notification) -> Result<(), DeliveryError> {
        let body = Self::render_body(notification);
        let mut request = self
            .client
            .post(&self.url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body.clone());
        if let Some(secret) = &self.signing_secret {
            // ADR-006: expose the secret ONLY here, to the HMAC, and attach the
            // DIGEST — never the key. Nothing secret survives past this line.
            let signature = Self::sign(secret.expose_secret(), &body);
            request = request.header(WEBHOOK_SIGNATURE_HEADER, signature);
        }
        // Hand-built, secret-free classification (ADR-006): never interpolate the
        // raw transport error, the URL, or any secret into the delivery error.
        let response = request.send().await.map_err(|_| {
            DeliveryError::Transient("webhook endpoint unreachable or errored".to_string())
        })?;
        if response.status().is_success() {
            Ok(())
        } else if response.status().is_server_error() {
            Err(DeliveryError::Transient(
                "webhook receiver returned a server error".to_string(),
            ))
        } else {
            Err(DeliveryError::Permanent(
                "webhook receiver rejected the delivery".to_string(),
            ))
        }
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::Webhook
    }

    async fn probe(&self) -> Result<(), DeliveryError> {
        // Startup reachability probe (N-ODD-3, ADR-006 §Probe): a TCP connect to
        // the receiver host:port ONLY — never a POST. Admitting the channel must
        // not side-effect it (no phantom delivery on the receiver).
        tokio::net::TcpStream::connect((self.host.as_str(), self.port))
            .await
            .map(|_| ())
            .map_err(|_| DeliveryError::Transient("webhook host unreachable".to_string()))
    }
}

/// Validated hosted email vendor API settings (ADR-002/006). All three of
/// `EMAIL_API_URL`/`EMAIL_API_KEY`/`EMAIL_API_FROM` are required; `EMAIL_API_KEY`
/// is held in a [`SecretString`] and exposed ONLY to the request's credential
/// header at delivery — never logged, echoed, nor Debug-printed. This type
/// deliberately does NOT derive `Debug`, so the key has no debug-leak vector.
pub struct EmailApiConfig {
    url: String,
    api_key: SecretString,
    from: String,
}

impl EmailApiConfig {
    /// Parse from the process environment: `EMAIL_API_URL`, `EMAIL_API_KEY`,
    /// `EMAIL_API_FROM` (all required).
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// Parse from an arbitrary key→value lookup (so callers and tests need no
    /// global env mutation). A missing/blank required setting fails fast, naming
    /// the provider AND the offending setting (ADR-002 / NFR-1) — secret-free by
    /// construction (it never echoes a value). `EMAIL_API_KEY` is validated FIRST
    /// so a bare `email_api` listing fails fast naming the security-critical key
    /// (the missing-setting scenario lists the provider with the key removed).
    pub fn from_lookup<F: Fn(&str) -> Option<String>>(get: F) -> anyhow::Result<Self> {
        let required = |key: &str| -> anyhow::Result<String> {
            match get(key) {
                Some(value) if !value.trim().is_empty() => Ok(value.trim().to_string()),
                _ => anyhow::bail!("provider 'email_api' is missing required setting '{key}'"),
            }
        };
        let api_key = SecretString::new(required("EMAIL_API_KEY")?.into());
        let url = required("EMAIL_API_URL")?;
        let from = required("EMAIL_API_FROM")?;
        Ok(Self { url, api_key, from })
    }
}

/// The `email_api` channel (ADR-001/002/006): renders the structured
/// [`Notification`] into the vendor's JSON send body and POSTs it to
/// `EMAIL_API_URL` over HTTPS via reqwest, carrying `EMAIL_API_KEY` as a bearer
/// credential header — the key rides ONLY on that header (never the body, a log,
/// or an error) and never leaves the process anywhere else. NOT `Debug` (ADR-006):
/// the key has no debug-leak vector. Per ADR-007 (best-effort at-most-once for
/// v1) a `429`/`5xx` vendor response is classified `failed` and is NOT retried —
/// `deliver()` returns once, and the infallible fan-out never re-invokes it. All
/// [`DeliveryError`] messages are hand-built + secret-free.
pub struct EmailApiProvider {
    url: String,
    host: String,
    port: u16,
    from: String,
    client: reqwest::Client,
    api_key: SecretString,
}

impl EmailApiProvider {
    /// Build the provider from validated config. Parses the reachability
    /// authority once (fail fast on a malformed URL) and constructs the HTTP
    /// client. The API key is retained for the per-request credential header.
    pub fn new(config: EmailApiConfig) -> Result<Self, DeliveryError> {
        let (host, port) = webhook_host_port(&config.url).ok_or_else(|| {
            DeliveryError::Permanent(
                "email_api 'EMAIL_API_URL' is not a valid http(s) URL".to_string(),
            )
        })?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|_| {
                DeliveryError::Permanent("email_api HTTP client could not be built".to_string())
            })?;
        Ok(Self {
            url: config.url,
            host,
            port,
            from: config.from,
            client,
            api_key: config.api_key,
        })
    }

    /// Render the vendor-neutral notification into the hosted email API's JSON
    /// send body (ADR-001): `{from, to, subject, body, event}`. The API key is
    /// NEVER part of the body — it rides only on the credential header (ADR-006).
    fn render_body(from: &str, notification: &Notification) -> String {
        serde_json::json!({
            "from": from,
            "to": notification.recipient,
            "subject": notification.subject,
            "body": notification.body,
            "event": notification.event.as_str(),
        })
        .to_string()
    }
}

#[async_trait]
impl NotificationProvider for EmailApiProvider {
    async fn deliver(&self, notification: &Notification) -> Result<(), DeliveryError> {
        let body = Self::render_body(&self.from, notification);
        // ADR-006: expose the key ONLY here, as the bearer credential header, and
        // never elsewhere. Nothing secret survives past this request construction.
        let response = self
            .client
            .post(&self.url)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", self.api_key.expose_secret()),
            )
            .body(body)
            .send()
            .await
            // Hand-built, secret-free classification (ADR-006): never interpolate
            // the raw transport error, the URL, or the key into the delivery error.
            .map_err(|_| {
                DeliveryError::Transient("email_api endpoint unreachable or errored".to_string())
            })?;
        if response.status().is_success() {
            Ok(())
        } else if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS
            || response.status().is_server_error()
        {
            // ADR-007: a vendor rate-limit (429) or 5xx is a transient failure —
            // counted `failed` and NOT retried in v1 (best-effort at-most-once).
            Err(DeliveryError::Transient(
                "email_api vendor rate-limited or unavailable".to_string(),
            ))
        } else {
            Err(DeliveryError::Permanent(
                "email_api vendor rejected the delivery".to_string(),
            ))
        }
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::EmailApi
    }

    async fn probe(&self) -> Result<(), DeliveryError> {
        // Startup reachability probe (wire → probe → use, N-ODD-3): a TCP connect
        // to the vendor host:port ONLY — never a send. Admitting the channel must
        // not side-effect the vendor (no phantom email). Secret-free error.
        tokio::net::TcpStream::connect((self.host.as_str(), self.port))
            .await
            .map(|_| ())
            .map_err(|_| DeliveryError::Transient("email_api host unreachable".to_string()))
    }
}

/// Build the active [`Notifier`] at the composition root from the
/// `NOTIFICATION_PROVIDERS` env var (ADR-002): a comma-separated channel list.
/// Unset/empty ⇒ an empty notifier (delivery inactive). Each listed channel is
/// constructed, probed (wire → probe → use), and admitted only on a passing probe.
/// An unknown channel name fails fast.
/// Read the per-provider delivery timeout from `NOTIFICATION_DELIVERY_TIMEOUT_MS`
/// (ADR-003), defaulting to [`DEFAULT_DELIVERY_TIMEOUT_MS`]. A non-numeric or
/// zero value falls back to the default. Single source of the env key + default
/// so the composition root (`main.rs`) wires the timeout without duplicating it.
pub fn delivery_timeout_from_env() -> Duration {
    std::env::var("NOTIFICATION_DELIVERY_TIMEOUT_MS")
        .ok()
        .and_then(|raw| raw.trim().parse::<u64>().ok())
        .filter(|ms| *ms > 0)
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_millis(DEFAULT_DELIVERY_TIMEOUT_MS))
}

/// The register-at-0 cross-product for a set of ACTIVE providers: one entry per
/// `(provider, event, outcome)` over the bounded `active × NotificationEvent
/// catalog × {delivered,failed}`. The composition root (`main.rs`) registers each
/// at 0 BEFORE any delivery fires so every series is present on the first
/// `/metrics` scrape (ADR-004) — an absent series never reads as "missing" on the
/// Grafana panel. Only ACTIVE providers mint series (an inactive channel is never
/// wired, so it has no series). The label domain stays bounded: every value is
/// drawn from a closed enum's `as_str()` (ADR-004/ADR-011 cardinality discipline).
pub fn delivery_zero_series(
    active: &[ProviderKind],
) -> Vec<(ProviderKind, NotificationEvent, DeliveryOutcome)> {
    let mut series = Vec::with_capacity(
        active.len() * NotificationEvent::ALL.len() * DeliveryOutcome::ALL.len(),
    );
    for provider in active {
        for event in NotificationEvent::ALL {
            for outcome in DeliveryOutcome::ALL {
                series.push((*provider, event, outcome));
            }
        }
    }
    series
}

pub async fn build_notifier(delivery_timeout: Duration) -> anyhow::Result<Notifier> {
    let spec = std::env::var("NOTIFICATION_PROVIDERS").unwrap_or_default();
    let notifier = build_notifier_from(&spec, |key| std::env::var(key).ok()).await?;
    Ok(notifier.with_delivery_timeout(delivery_timeout))
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
            "webhook" => {
                // Wire → probe → use (ADR-002): parse+validate WEBHOOK_* (fail-fast
                // naming the missing key), construct the HTTP provider, and admit
                // the channel only after its host-reachability probe passes (NO
                // POST, N-ODD-3). Every error here is secret-free (ADR-006).
                let config = WebhookConfig::from_lookup(&get)?;
                let provider = WebhookProvider::new(config).map_err(|err| {
                    anyhow::anyhow!("provider 'webhook' could not be constructed: {err}")
                })?;
                provider.probe().await.map_err(|err| {
                    anyhow::anyhow!("provider 'webhook' failed its startup probe: {err}")
                })?;
                providers.push(Arc::new(provider));
            }
            "email_api" => {
                // Wire → probe → use (ADR-002): parse+validate EMAIL_API_* (fail-
                // fast naming the missing key), construct the HTTPS provider, and
                // admit the channel only after its host-reachability probe passes
                // (NO send, N-ODD-3). Every error here is secret-free (ADR-006).
                let config = EmailApiConfig::from_lookup(&get)?;
                let provider = EmailApiProvider::new(config).map_err(|err| {
                    anyhow::anyhow!("provider 'email_api' could not be constructed: {err}")
                })?;
                provider.probe().await.map_err(|err| {
                    anyhow::anyhow!("provider 'email_api' failed its startup probe: {err}")
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

    // ── Slice 03 — concurrent fan-out, isolation, per-provider metric emit ──

    use metrics_exporter_prometheus::PrometheusBuilder;
    use std::collections::BTreeSet;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Instant;

    /// Behaviour a [`ControllableProvider`] test double exhibits.
    enum TestBehavior {
        Deliver,
        Fail,
        Hang(Duration),
    }

    /// A `NotificationProvider` double whose delivery outcome (and latency) the
    /// test controls, counting successful deliveries into a shared atomic so a
    /// test can assert a fast provider still delivered beside a slow/failing one.
    struct ControllableProvider {
        kind: ProviderKind,
        behavior: TestBehavior,
        delivered: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl NotificationProvider for ControllableProvider {
        async fn deliver(&self, _notification: &Notification) -> Result<(), DeliveryError> {
            match &self.behavior {
                TestBehavior::Deliver => {
                    self.delivered.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
                TestBehavior::Fail => Err(DeliveryError::Transient(
                    "induced delivery failure (test)".to_string(),
                )),
                TestBehavior::Hang(duration) => {
                    tokio::time::sleep(*duration).await;
                    self.delivered.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            }
        }

        fn kind(&self) -> ProviderKind {
            self.kind
        }

        async fn probe(&self) -> Result<(), DeliveryError> {
            Ok(())
        }
    }

    fn reset_notification() -> Notification {
        Notification {
            event: NotificationEvent::PasswordReset,
            recipient: "maria.santos@acme.example".to_string(),
            subject: "Reset your Foundry password".to_string(),
            body: "follow this link".to_string(),
        }
    }

    /// Locate the delivery-counter exposition line for a `(provider, outcome)`.
    fn find_delivery_line<'a>(body: &'a str, provider: &str, outcome: &str) -> Option<&'a str> {
        body.lines().find(|line| {
            line.starts_with(&format!("{NOTIFICATION_DELIVERIES_METRIC}{{"))
                && line.contains(&format!("provider=\"{provider}\""))
                && line.contains(&format!("outcome=\"{outcome}\""))
        })
    }

    /// The label KEY set on a Prometheus exposition line (fail-closed cardinality).
    fn label_keys(line: &str) -> BTreeSet<String> {
        let open = line.find('{').expect("line has `{`");
        let close = line.rfind('}').expect("line has `}`");
        line[open + 1..close]
            .split(',')
            .filter_map(|kv| kv.split('=').next())
            .map(|key| key.trim().to_string())
            .collect()
    }

    #[test]
    fn notify_emits_the_delivery_counter_per_provider_with_a_bounded_binary_outcome() {
        // Behaviour: each provider's outcome is counted once on
        // `foundry_notification_deliveries_total{provider,event,outcome}` — a clean
        // delivery as `delivered`, a failing provider as `failed` (binary outcome).
        // The label KEY set is EXACTLY {provider,event,outcome} — fails closed if a
        // future contributor widens it (ADR-004 cardinality discipline).
        let recorder = PrometheusBuilder::new().build_recorder();
        let handle = recorder.handle();
        let delivered = Arc::new(AtomicUsize::new(0));
        let providers: Vec<Arc<dyn NotificationProvider>> = vec![
            Arc::new(ControllableProvider {
                kind: ProviderKind::Log,
                behavior: TestBehavior::Deliver,
                delivered: delivered.clone(),
            }),
            Arc::new(ControllableProvider {
                kind: ProviderKind::Smtp,
                behavior: TestBehavior::Fail,
                delivered: delivered.clone(),
            }),
        ];
        let notifier = Notifier::new(providers);
        let notification = reset_notification();

        // A current-thread runtime so the scoped (thread-local) recorder set by
        // `with_local_recorder` covers the whole fan-out.
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build current-thread runtime");
        metrics::with_local_recorder(&recorder, || {
            runtime.block_on(async {
                notifier.notify(&notification).await;
            });
        });
        let body = handle.render();

        let log_line = find_delivery_line(&body, "log", "delivered")
            .unwrap_or_else(|| panic!("no log/delivered series in scrape:\n{body}"));
        assert!(
            log_line.trim_end().ends_with(" 1"),
            "the log delivery must count exactly 1: {log_line}"
        );
        let expected_keys: BTreeSet<String> = ["event", "outcome", "provider"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            label_keys(log_line),
            expected_keys,
            "delivery counter labels must be exactly {{provider,event,outcome}}: {log_line}"
        );

        let smtp_line = find_delivery_line(&body, "smtp", "failed")
            .unwrap_or_else(|| panic!("no smtp/failed series in scrape:\n{body}"));
        assert!(
            smtp_line.trim_end().ends_with(" 1"),
            "the failing smtp delivery must count exactly 1 under outcome=failed: {smtp_line}"
        );
    }

    #[test]
    fn register_at_zero_mints_the_active_cross_product_with_bounded_labels() {
        // Behaviour (ADR-004): register-at-0 mints EXACTLY the bounded cross-
        // product `active providers × NotificationEvent catalog × {delivered,
        // failed}`, every series at 0, with the label KEY set fail-closed at
        // {provider,event,outcome}. Only ACTIVE providers mint series — an
        // inactive channel (webhook/email_api here) appears nowhere. Exhaustive
        // over the closed domains = the property this pins for the real /metrics
        // register-at-0 the composition root performs.
        let recorder = PrometheusBuilder::new().build_recorder();
        let handle = recorder.handle();
        let active = [ProviderKind::Log, ProviderKind::Smtp];

        metrics::with_local_recorder(&recorder, || {
            for (provider, event, outcome) in delivery_zero_series(&active) {
                metrics::counter!(
                    NOTIFICATION_DELIVERIES_METRIC,
                    "provider" => provider.as_str(),
                    "event" => event.as_str(),
                    "outcome" => outcome.as_str(),
                )
                .absolute(0);
            }
        });
        let body = handle.render();

        let expected_keys: BTreeSet<String> = ["event", "outcome", "provider"]
            .iter()
            .map(|key| key.to_string())
            .collect();

        // Every active × event × outcome series is present at zero with exactly
        // the bounded label keys.
        let mut minted = 0;
        for provider in active {
            for event in NotificationEvent::ALL {
                for outcome in DeliveryOutcome::ALL {
                    let line = body
                        .lines()
                        .find(|line| {
                            line.starts_with(&format!("{NOTIFICATION_DELIVERIES_METRIC}{{"))
                                && line.contains(&format!("provider=\"{}\"", provider.as_str()))
                                && line.contains(&format!("event=\"{}\"", event.as_str()))
                                && line.contains(&format!("outcome=\"{}\"", outcome.as_str()))
                        })
                        .unwrap_or_else(|| {
                            panic!(
                                "register-at-0 must mint provider={} event={} outcome={} at zero:\n{body}",
                                provider.as_str(),
                                event.as_str(),
                                outcome.as_str(),
                            )
                        });
                    assert!(
                        line.trim_end().ends_with(" 0"),
                        "the registered series must sit at zero: {line}"
                    );
                    assert_eq!(
                        label_keys(line),
                        expected_keys,
                        "delivery series labels must be exactly {{provider,event,outcome}}: {line}"
                    );
                    minted += 1;
                }
            }
        }
        assert_eq!(
            minted,
            active.len() * NotificationEvent::ALL.len() * DeliveryOutcome::ALL.len(),
            "the minted series count must equal the active cross-product exactly"
        );

        // Only ACTIVE providers mint series — the inactive channels are absent.
        for inactive in [ProviderKind::Webhook, ProviderKind::EmailApi] {
            assert!(
                !body.contains(&format!("provider=\"{}\"", inactive.as_str())),
                "an inactive provider must mint NO series, found {}:\n{body}",
                inactive.as_str()
            );
        }
    }

    #[tokio::test]
    async fn a_slow_provider_does_not_extend_wall_time_beyond_one_timeout_window() {
        // Behaviour: fan-out is concurrent + per-provider-timeout-bounded, so a
        // provider that hangs does NOT extend the emit path beyond ~one timeout
        // window, and its fast sibling still delivers. A sequential (or unbounded)
        // notify would take the full hang — this reds it.
        let delivered = Arc::new(AtomicUsize::new(0));
        let providers: Vec<Arc<dyn NotificationProvider>> = vec![
            Arc::new(ControllableProvider {
                kind: ProviderKind::Log,
                behavior: TestBehavior::Deliver,
                delivered: delivered.clone(),
            }),
            Arc::new(ControllableProvider {
                kind: ProviderKind::Smtp,
                behavior: TestBehavior::Hang(Duration::from_secs(3)),
                delivered: delivered.clone(),
            }),
        ];
        let notifier = Notifier::new(providers).with_delivery_timeout(Duration::from_millis(200));
        let notification = reset_notification();

        let started = Instant::now();
        notifier.notify(&notification).await;
        let elapsed = started.elapsed();

        assert!(
            elapsed < Duration::from_secs(2),
            "concurrent, timeout-bounded fan-out must not wait out a hung provider, took {elapsed:?}"
        );
        assert_eq!(
            delivered.load(Ordering::SeqCst),
            1,
            "the fast provider must still deliver despite the slow sibling"
        );
    }

    // ── Slice 04 — webhook adapter: JSON POST, no-POST probe, HMAC signature ──

    use std::sync::atomic::AtomicBool;
    use tokio::io::AsyncReadExt;

    /// A distinctive value that would leak loudly if any layer echoed the webhook
    /// signing secret. Reused across the no-leak litmus + the acceptance step.
    const WEBHOOK_SIGNING_SECRET_SENTINEL: &str = "ndp-webhook-signing-secret-must-never-leak-7c1e";

    fn member_invite_notification() -> Notification {
        Notification {
            event: NotificationEvent::MemberInvite,
            recipient: "sam.okafor@acme.example".to_string(),
            subject: "You are invited to Acme".to_string(),
            body: "Accept: https://foundry.example/invite?token=INVITE_SECRET".to_string(),
        }
    }

    #[test]
    fn webhook_signature_is_hmac_sha256_over_the_body_as_sha256_hex() {
        // Independent oracle (NOT the production path recomputed): the well-known
        // HMAC-SHA256 vector for key "key" over the pangram. Pins that the header
        // value is genuinely HMAC-SHA256(body) under the secret, hex-encoded.
        let signature = WebhookProvider::sign("key", "The quick brown fox jumps over the lazy dog");
        assert_eq!(
            signature, "sha256=f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8",
            "the webhook signature must be the sha256= HMAC-SHA256 hex of the body"
        );
    }

    #[test]
    fn webhook_render_body_carries_the_vendor_neutral_contract() {
        // ADR-001: the webhook renders the structured notification to
        // `{event, recipient, subject, body}` — the JSON wire contract.
        let body = WebhookProvider::render_body(&member_invite_notification());
        let value: serde_json::Value = serde_json::from_str(&body).expect("body is valid JSON");
        assert_eq!(value["event"], "member_invite");
        assert_eq!(value["recipient"], "sam.okafor@acme.example");
        assert_eq!(value["subject"], "You are invited to Acme");
        assert_eq!(
            value["body"],
            "Accept: https://foundry.example/invite?token=INVITE_SECRET"
        );
    }

    #[test]
    fn webhook_config_requires_the_url_and_fails_fast_naming_provider_and_setting() {
        // A missing WEBHOOK_URL fails fast naming BOTH the provider and the
        // offending key (ADR-002 / NFR-1), secret-free by construction.
        let Err(err) = WebhookConfig::from_lookup(|_| None) else {
            panic!("a missing WEBHOOK_URL must fail fast");
        };
        let message = format!("{err:#}");
        assert!(
            message.contains("webhook"),
            "error must name provider webhook: {message}"
        );
        assert!(
            message.contains("WEBHOOK_URL"),
            "error must name the missing setting WEBHOOK_URL: {message}"
        );
    }

    #[tokio::test]
    async fn webhook_probe_connects_without_posting_to_the_receiver() {
        // N-ODD-3 (ADR-006 §Probe): probe() is host-reachability ONLY — a TCP
        // connect, never a POST. A local listener accepts the probe connection and
        // asserts NO HTTP request bytes (let alone a POST) ever arrive.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind probe listener");
        let addr = listener.local_addr().expect("probe listener addr");
        let posted = Arc::new(AtomicBool::new(false));
        let observed = posted.clone();
        let server = tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0u8; 8];
                let read = tokio::time::timeout(Duration::from_millis(300), stream.read(&mut buf))
                    .await
                    .ok()
                    .and_then(Result::ok)
                    .unwrap_or(0);
                if read > 0 {
                    observed.store(true, Ordering::SeqCst);
                }
            }
        });

        let config = WebhookConfig::from_lookup(|key| match key {
            "WEBHOOK_URL" => Some(format!("http://{addr}/hook")),
            _ => None,
        })
        .expect("webhook config parses");
        let provider = WebhookProvider::new(config).expect("webhook provider builds");
        provider
            .probe()
            .await
            .expect("probe must succeed against a reachable host");
        let _ = server.await;

        assert!(
            !posted.load(Ordering::SeqCst),
            "the webhook probe must send NO bytes (no POST) to the receiver"
        );
    }

    #[tokio::test]
    async fn webhook_signing_secret_never_leaks_in_the_signature_error_or_debug() {
        // The five-layer no-leak litmus at unit scope (ADR-006): the secret is held
        // in a SecretString (redacted on Debug), the provider is not Debug, the
        // wire-borne signature is a DIGEST (not the secret), and a real
        // DeliveryError from a closed endpoint is hand-built + secret-free.
        let config = WebhookConfig::from_lookup(|key| match key {
            "WEBHOOK_URL" => Some("http://127.0.0.1:1/hook".to_string()),
            "WEBHOOK_SIGNING_SECRET" => Some(WEBHOOK_SIGNING_SECRET_SENTINEL.to_string()),
            _ => None,
        })
        .expect("sentinel webhook config parses");

        let secret_debug = format!("{:?}", config.signing_secret);
        assert!(
            !secret_debug.contains(WEBHOOK_SIGNING_SECRET_SENTINEL),
            "SecretString must redact the signing secret on Debug: {secret_debug}"
        );

        let signature = WebhookProvider::sign(
            WEBHOOK_SIGNING_SECRET_SENTINEL,
            &WebhookProvider::render_body(&member_invite_notification()),
        );
        assert!(
            signature.starts_with("sha256=")
                && !signature.contains(WEBHOOK_SIGNING_SECRET_SENTINEL),
            "the signature must be a digest, never the raw secret: {signature}"
        );

        let provider = WebhookProvider::new(config).expect("webhook provider builds");
        // 127.0.0.1:1 is closed → connection refused → a genuine transport error.
        let err = provider
            .deliver(&member_invite_notification())
            .await
            .expect_err("a closed endpoint must fail the delivery");
        let rendered = format!("{err} || {err:?}");
        assert!(
            !rendered.contains(WEBHOOK_SIGNING_SECRET_SENTINEL),
            "the signing secret must never appear in a delivery error or its debug output: {rendered}"
        );
    }

    #[tokio::test]
    async fn build_notifier_webhook_branch_rejects_when_the_host_is_unreachable() {
        // The webhook registry branch is wire → probe → use: valid config but an
        // unreachable host (closed localhost port) fails the startup probe and the
        // channel is refused — the error names webhook.
        let get = |key: &str| match key {
            "WEBHOOK_URL" => Some("http://127.0.0.1:1/hook".to_string()),
            _ => None,
        };
        let Err(err) = build_notifier_from("webhook", get).await else {
            panic!("webhook with an unreachable host must fail its startup probe");
        };
        let message = format!("{err:#}");
        assert!(
            message.contains("webhook"),
            "probe failure must name webhook: {message}"
        );
    }

    // ── Slice 05 — hosted email API adapter: credential-header POST, no-retry ──

    /// A distinctive value that would leak loudly if any layer echoed the hosted
    /// email API key. Reused across the no-leak litmus + the credential-header test.
    const EMAIL_API_KEY_SENTINEL: &str = "ndp-email-api-key-must-never-leak-4d2b";

    fn email_api_env(url: &str) -> impl Fn(&str) -> Option<String> + '_ {
        move |key: &str| match key {
            "EMAIL_API_URL" => Some(url.to_string()),
            "EMAIL_API_KEY" => Some(EMAIL_API_KEY_SENTINEL.to_string()),
            "EMAIL_API_FROM" => Some("noreply@acme.example".to_string()),
            _ => None,
        }
    }

    #[test]
    fn email_api_config_parses_all_required_settings() {
        // All three required settings land on the config; a valid lookup parses.
        let config = EmailApiConfig::from_lookup(email_api_env("https://api.vendor.example/send"))
            .expect("valid email_api config parses");
        assert_eq!(config.url, "https://api.vendor.example/send");
        assert_eq!(config.from, "noreply@acme.example");
    }

    #[test]
    fn email_api_config_missing_key_fails_fast_naming_provider_and_setting() {
        // A bare `email_api` listing (no settings) fails fast naming BOTH the
        // provider and the security-critical EMAIL_API_KEY (ADR-002 / NFR-1),
        // secret-free by construction.
        let Err(err) = EmailApiConfig::from_lookup(|_| None) else {
            panic!("a missing EMAIL_API_KEY must fail fast");
        };
        let message = format!("{err:#}");
        assert!(
            message.contains("email_api"),
            "error must name provider email_api: {message}"
        );
        assert!(
            message.contains("EMAIL_API_KEY"),
            "error must name the missing setting EMAIL_API_KEY: {message}"
        );
    }

    #[test]
    fn email_api_render_body_carries_the_vendor_neutral_contract() {
        // ADR-001: the hosted API renders the structured notification to
        // `{from, to, subject, body, event}` — and NEVER the key.
        let body = EmailApiProvider::render_body("noreply@acme.example", &reset_notification());
        let value: serde_json::Value = serde_json::from_str(&body).expect("body is valid JSON");
        assert_eq!(value["from"], "noreply@acme.example");
        assert_eq!(value["to"], "maria.santos@acme.example");
        assert_eq!(value["event"], "password_reset");
        assert!(
            value.get("subject").is_some() && value.get("body").is_some(),
            "the send body must carry subject + body: {body}"
        );
    }

    #[tokio::test]
    async fn email_api_deliver_sends_the_key_as_a_credential_header_only() {
        // The key rides ONLY on the Authorization credential header — never in the
        // JSON body. A local listener captures the raw request bytes; the sentinel
        // appears in the `authorization:` line and NOT in the JSON payload.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind email_api listener");
        let addr = listener.local_addr().expect("email_api listener addr");
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept email_api POST");
            let mut buf = vec![0u8; 4096];
            let read = tokio::time::timeout(Duration::from_millis(500), stream.read(&mut buf))
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or(0);
            // Answer 200 so the provider records a clean delivery.
            let _ = tokio::io::AsyncWriteExt::write_all(
                &mut stream,
                b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n",
            )
            .await;
            String::from_utf8_lossy(&buf[..read]).to_string()
        });

        let config = EmailApiConfig::from_lookup(email_api_env(&format!("http://{addr}/send")))
            .expect("email_api config parses");
        let provider = EmailApiProvider::new(config).expect("email_api provider builds");
        provider
            .deliver(&reset_notification())
            .await
            .expect("delivery to the local vendor listener succeeds");

        let raw = server.await.expect("collect raw request");
        let lower = raw.to_ascii_lowercase();
        assert!(
            lower.contains(&format!("authorization: bearer {EMAIL_API_KEY_SENTINEL}")),
            "the key must ride on the Authorization credential header: {raw}"
        );
        // The body is the last line (after the blank line); the key must NOT be in it.
        let body = raw.split("\r\n\r\n").nth(1).unwrap_or("");
        assert!(
            !body.contains(EMAIL_API_KEY_SENTINEL),
            "the key must never appear in the request body: {body}"
        );
    }

    #[tokio::test]
    async fn email_api_key_never_leaks_in_a_delivery_error_or_debug() {
        // The no-leak litmus at unit scope (ADR-006): the key is held in a
        // SecretString (redacted on Debug), the provider is not Debug, and a real
        // DeliveryError from a closed endpoint is hand-built + secret-free.
        let config = EmailApiConfig::from_lookup(email_api_env("http://127.0.0.1:1/send"))
            .expect("sentinel email_api config parses");
        let secret_debug = format!("{:?}", config.api_key);
        assert!(
            !secret_debug.contains(EMAIL_API_KEY_SENTINEL),
            "SecretString must redact the key on Debug: {secret_debug}"
        );
        let provider = EmailApiProvider::new(config).expect("email_api provider builds");
        // 127.0.0.1:1 is closed → connection refused → a genuine transport error.
        let err = provider
            .deliver(&reset_notification())
            .await
            .expect_err("a closed endpoint must fail the delivery");
        let rendered = format!("{err} || {err:?}");
        assert!(
            !rendered.contains(EMAIL_API_KEY_SENTINEL),
            "the email_api key must never appear in a delivery error or its debug output: {rendered}"
        );
    }

    #[tokio::test]
    async fn build_notifier_email_api_branch_rejects_when_the_host_is_unreachable() {
        // The email_api registry branch is wire → probe → use: valid config but an
        // unreachable host (closed localhost port) fails the startup probe and the
        // channel is refused — the error names email_api and leaks no secret.
        let get = email_api_env("http://127.0.0.1:1/send");
        let Err(err) = build_notifier_from("email_api", get).await else {
            panic!("email_api with an unreachable host must fail its startup probe");
        };
        let message = format!("{err:#}");
        assert!(
            message.contains("email_api"),
            "probe failure must name email_api: {message}"
        );
        assert!(
            !message.contains(EMAIL_API_KEY_SENTINEL),
            "the probe-failure error must carry no secret value: {message}"
        );
    }
}
