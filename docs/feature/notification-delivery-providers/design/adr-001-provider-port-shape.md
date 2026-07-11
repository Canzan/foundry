# ADR-001: NotificationProvider port shape (structured envelope vs email-centric send)

## Status
Accepted (DESIGN, Propose mode; orchestrator auto-accepts the recommended option). Resolves **ODD-1** (Risk R1).

## Context
The shipped port is `EmailSender` (`crates/foundry-app/src/email.rs:19-22`):
```rust
#[async_trait]
pub trait EmailSender: Send + Sync + Debug + 'static {
    async fn send(&self, to: &str, subject: &str, body: &str) -> anyhow::Result<()>;
}
```
Its only production impl is `NoopEmailSender` (silently drops). The three call sites pass an email-shaped
`(to, subject, body)` triple they render inline (`signin.rs:227-235`, `bootstrap.rs:254-258`,
`member_invites.rs:184-189`). The feature must serve **email AND non-email** transports (webhook/chat,
hosted-API) and must emit a bounded `event` label for the delivery metric (NFR-4). ODD-1 is the crux the
walking skeleton (slice 01) carries: keep the port email-centric, or introduce a structured notification.

Two hard constraints decide it:
1. The `event` metric label (`foundry_notification_deliveries_total{provider,event,outcome}`, NFR-4/ADR-004)
   **cannot be reconstructed** from `(to, subject, body)` — the event identity must travel with the payload.
2. A webhook/chat provider needs **structure** (an event discriminator + recipient), not a rendered email body.

## Decision
Introduce a **structured, vendor-neutral envelope** and a generalized port:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationEvent { PasswordReset, WorkspaceInvite, MemberInvite, MemberRemoved, PasswordChanged }

#[derive(Debug, Clone)]
pub struct Notification { pub event: NotificationEvent, pub recipient: String, pub subject: String, pub body: String }

#[derive(Debug)]
pub enum DeliveryError { Transient(String), Permanent(String) }  // ADR-004

#[async_trait]
pub trait NotificationProvider: Send + Sync + 'static {          // NO Debug supertrait — ADR-006
    fn kind(&self) -> ProviderKind;
    async fn deliver(&self, notification: &Notification) -> Result<(), DeliveryError>;
    async fn probe(&self) -> Result<(), DeliveryError>;          // Earned Trust — ADR-006
}
```
The envelope carries the **bounded `event`** (for the metric + provider-side routing), the **recipient**, and
the **already-rendered email-shaped `subject`/`body`** the three call sites already produce. Email-shaped
adapters (`log`, `smtp`, `email_api`) read `subject`/`body` directly — so backwards-compat (NFR-5) is *exact*,
the existing rendered copy is preserved byte-for-byte. The `webhook` adapter serializes the whole struct to
JSON. The domain stays vendor-neutral: no SMTP/webhook/vendor concept leaks into `Notification` or the port.

The return type is `Result<(), DeliveryError>` (a domain taxonomy, ADR-004), **not** `anyhow::Result<()>` —
so the dispatcher can classify outcomes without string-sniffing and no adapter-internal error (which might
embed a secret) escapes unclassified.

## Alternatives Considered
- **Keep email-centric `send(&self, to, subject, body) -> Result` (minimal change from `EmailSender`)** —
  REJECTED. It cannot carry the `event` discriminator the metric label requires (NFR-4) without a side channel,
  and it forces every non-email provider (webhook/chat) to reverse-engineer structure from a rendered email
  body. It would re-lock the abstraction to email, failing JOB-1 (the org's real channels are SMTP **and**
  webhook **and** hosted vendor) — the exact "email-only path" the DISCUSS alternatives already rejected.
- **Fully abstract `Notification { event, recipient, payload: serde_json::Value }` — each provider renders
  everything from structured data** — REJECTED for v1. It maximizes flexibility but (a) discards the
  already-rendered `subject`/`body` the three call sites produce, forcing a new templating layer inside every
  email-shaped adapter (templating is an explicit OUT-OF-SCOPE carve-out), and (b) makes backwards-compat
  (NFR-5) a re-render rather than a pass-through, adding regression risk (R7) for no v1 benefit. The chosen
  envelope keeps a structured discriminator + recipient AND the rendered content — the pragmatic middle.
- **Two ports (an `EmailProvider` and a `WebhookProvider`)** — REJECTED. It duplicates the fan-out, registry,
  and metric machinery per shape and forces the dispatcher to branch on provider family. One port with one
  envelope keeps the dispatcher uniform (ADR-003) and every provider substitutable.
- **Keep the `Debug` supertrait** — REJECTED (see ADR-006): a secret-holding adapter would leak via `{:?}`.

## Consequences
- Positive: one uniform port + envelope serves all four transports; the `event` label is first-class; the
  rendered email content passes through unchanged (exact NFR-5); the domain is transport-neutral and
  unit-testable with a recording double (the reused `FakeEmailSender` seam, generalized).
- Positive: `DeliveryError` gives the dispatcher a clean, secret-free classification (ADR-004) and the future
  retry layer its input (ADR-007) at zero extra cost now.
- Negative: call sites change from `email.send(to,subject,body)` to building a `Notification{..}` and calling
  `notify(&n)` — a mechanical edit at three sites (slices 01/03), covered by the backwards-compat regression
  guard (NFR-5).
- Negative: the envelope carries `subject`/`body` even for a pure-structured webhook consumer (which mostly
  wants `event`+`recipient`); accepted — the fields are cheap and the webhook can ignore or forward them.
- Probe (Earned Trust): the walking skeleton (slice 01) drives one real notification (password reset) end-to-end
  through the port at N=1 (log provider), proving the envelope + port shape *before* any transport is built —
  the R1 uncertainty is retired first, exactly as the DISCUSS intended.
