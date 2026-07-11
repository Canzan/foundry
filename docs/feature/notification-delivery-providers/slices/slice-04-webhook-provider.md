# Slice 04 — Webhook / generic HTTP POST provider

**Goal**: deliver notifications into a chat/webhook endpoint → an operator adds `webhook` to
`NOTIFICATION_PROVIDERS` with `WEBHOOK_URL=https://hooks.slack.example/services/T00/B00/xyz` (and an optional
signing secret), and the next member invite for `sam.okafor@acme.example` posts a JSON payload into the
channel.
**Story**: US-04.

**IN scope**
- A **Webhook** provider implementing the `NotificationProvider` port, active when `webhook` is listed and
  `WEBHOOK_URL` is configured; POSTs each notification as a JSON body to the URL.
- Optional **payload signing**: with `WEBHOOK_SIGNING_SECRET` set, add a signature header derived from the
  secret + body; the secret never appears in the body/logs/metrics (NFR-2, ODD-8).
- Participates in **fan-out + best-effort isolation + counting** exactly like the built-in providers: a non-2xx
  or unreachable receiver counts `provider=webhook outcome=failed` and never fails the request or other
  providers (NFR-3, NFR-4).
- **Fail-fast** if `webhook` is listed without `WEBHOOK_URL` (NFR-1).
- Acceptance: posted-to-webhook, signed-no-leak, failing-receiver-isolated; dogfooded against a local endpoint
  (or a real chat incoming webhook).

**OUT of scope**: hosted email API (slice 05); new event types (06); retry on failure (v1 best-effort, NFR-6);
per-event routing/filtering (that edges toward recipient preferences — out of scope, successor feature).

**Learning hypothesis**: disproves "the port shape hosts a NON-email provider (webhook/chat JSON) cleanly"
if delivering a chat-shaped JSON payload forces the port to change (R1/ODD-1 — email-centric `send(to,subject,
body)` may not carry structured event data a webhook needs), or if signing/secret-safety fights the port's
`Debug` supertrait (ODD-8).

**Seams**: the fan-out + isolation + metric machinery (slice 03); the `NotificationProvider` port (slice 01);
config style (`main.rs:242-262`); an HTTP client (workspace deps — DESIGN selects; the app already pulls HTTP
crates transitively). Payload shape depends on ODD-1.
**Dependencies**: US-03 (fan-out/isolation/metric). DESIGN ODD-1 (structured vs email-centric payload).
**Effort**: ~1 day (an HTTP POST adapter + optional signing; reuses fan-out/isolation/metric).
