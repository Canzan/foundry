# DISCUSS Decisions — notification-delivery-providers

## Key Decisions

- [D1] **Generalize the port, don't add another mailer.** The single `EmailSender` port
  (`email.rs:19-22`, only prod impl `NoopEmailSender`) becomes a pluggable `NotificationProvider` abstraction
  with a **provider registry** + **config-driven selection** (`NOTIFICATION_PROVIDERS` + per-provider env).
  Rejected the email-only path — the org's real channels are SMTP **and** chat webhook **and** hosted vendor.
- [D2] **Fan-out with best-effort per-provider isolation.** One emitted notification is delivered to **all**
  active providers; a provider failing never fails/blocks the originating request and never sinks the others.
  This **generalizes the log-and-continue semantics the three shipped call sites already use**
  (`signin.rs:235`, `bootstrap.rs:258`, `member_invites.rs:189`) to N providers — not a new invention.
- [D3] **Four provider kinds across six thin slices, v1 boundary at 01–03.** Slice 01 (walking skeleton):
  port + registry + config selection + trivial **log/stdout** provider, re-route ONE existing notification
  (password reset). Slice 02: **SMTP** via the declared-but-unused `lettre`, behind the port. Slice 03:
  **fan-out** + best-effort isolation + per-provider observability (closes v1). Slices 04–06 (Webhook, Hosted
  email API, two new event types) are fast-follow in this same feature.
- [D4] **Config is env-only, house style.** Provider selection + settings are read with direct
  `std::env::var` at the composition root (`main.rs:265`), consistent with `DATABASE_URL`/`SESSION_SECRET`/etc.
  No config file, no figment. Unset `NOTIFICATION_PROVIDERS` ⇒ no active providers ⇒ Noop-equivalent.
- [D5] **Fail-fast config validation, non-enumerable inactivity.** A provider listed but misconfigured, or an
  unknown provider name, **aborts startup** with a secret-free, operator-actionable error and a non-zero exit;
  a provider not listed is simply inactive (never constructed). Secrets are never printed.
- [D6] **Secrets never observable.** SMTP creds, webhook signing secrets, and hosted-API keys are read from
  env into the provider only — never in logs, errors, metric labels, or `Debug` output. The port's `Debug`
  supertrait (`email.rs:19`) makes secret-safe `Debug` a first-class concern (ODD-8).
- [D7] **Per-provider observability on the existing seam.** Delivery is counted as
  `foundry_notification_deliveries_total{provider,event,outcome}` via the shipped `metrics` facade (mirroring
  `foundry_token_mutations_total`, `rate_limit.rs:198-203`), registered at 0 (`main.rs:355-363`), exposed on
  the existing `/metrics` sidecar (`metrics_server.rs:66`), bounded-label per ADR-011. No new dashboard infra.
- [D8] **Best-effort at-most-once (v1); durable retry deferred.** v1 preserves today's best-effort semantics
  exactly (a transient provider outage may drop that provider's copy, no retry/dedup). The repo's `outbox`
  seam (`main.rs:29`) could later back durable delivery — deferred (ODD-7, Risk R5).
- [D9] **Bounded notification catalog.** The `event` label domain is a bounded set (BR-7); a new event type is
  an explicit catalog addition. The catalog mirrors the house forward-compat envelope `EventPayload`
  (`foundry-realtime/src/lib.rs:66-105`); whether to align them is DESIGN's call (ODD-6).
- [D10] **Repo legacy multi-file convention; no `docs/product/` SSOT; JTBD folded inline.** JTBD (two jobs,
  four forces, ODI opportunity scores) lives in `requirements.md`, not a `jobs.yaml`. No SSOT files emitted.
  Matches all prior features on trunk.

## Requirements Summary
- Primary need: route Foundry's notifications through the transports an org already runs (SMTP, chat webhook,
  hosted email vendor) via config alone — emit once, deliver everywhere, safely and observably — replacing the
  single hard-wired no-op sender.
- Walking skeleton: slice 01 — the port + registry + a log provider + one re-routed notification (the whole
  configure→emit→deliver→observe loop at N=1).
- Feature type: cross-cutting (app delivery pipeline + operator config + observability), brownfield. One
  bounded context (`foundry-app` delivery side + the `metrics` seam + the `EventPayload` envelope pattern).

## Constraints Established
- Config-driven selection via `NOTIFICATION_PROVIDERS` + per-provider env (`SMTP_*`, `WEBHOOK_*`,
  `EMAIL_API_*`), `std::env::var` house style; no config file (BR-5).
- Best-effort per-provider isolation: a provider failure never fails/blocks the request nor sinks others
  (NFR-3, BR-2); a slow provider is bounded so it cannot stall the handler.
- Secrets never in logs/errors/metrics/`Debug` (NFR-2, BR-4, ODD-8).
- Fail-fast on listed-but-misconfigured or unknown provider; unlisted → inactive (NFR-1, BR-6).
- Per-provider delivery counter, bounded labels (ADR-011), registered at 0, on the existing `/metrics` sidecar
  (NFR-4).
- Existing notification behavior preserved exactly (best-effort, non-fatal); unset config ⇒ Noop-equivalent
  (NFR-5, BR-3); slices 01–02 regression-guarded.
- v1 delivery is best-effort at-most-once; no retry/dedup (NFR-6).
- No new user-facing form ⇒ no new accessibility surface (NFR-7, N/A stated explicitly).

## Scope Assessment: PASS

**PASS — 6 stories, 1 bounded context, ~5–6 days total across six thin slices.** Right-sized; no split needed.

**Oversized→split analysis performed.** The initial framing ("a notification system") threatened to bundle
delivery **and** recipient/per-user preferences (opt-in/out, per-channel routing, digests, quiet-hours), which
would push past the Elephant-Carpaccio gate (>10 stories, a second bounded context around per-user preference
state, multiple independent user outcomes). **Resolution (confirmed with the user before this map was drawn):
recipient/per-user notification preferences are CARVED OUT into a separate follow-on feature named
`recipient-notification-preferences`** — OUT OF SCOPE here. This feature is the **operator/developer-facing
delivery abstraction only**; the downstream recipient is acknowledged (more channels reach them) but their
preferences are not built here. Building the delivery abstraction first gives preferences a concrete pipeline
to route over.

Post-carve-out signals: stories 6 (≤10) | bounded contexts 1 (≤3) | walking-skeleton integration points ~4
reused seams + 1 new registry (well under the >5 red line) | effort ~5–6 days (<2 weeks) | one coherent
capability (pluggable delivery) sliced into thin per-provider/per-capability increments, each dogfoodable in a
single session. No slice ships 4+ new components.

## Handoff to DESIGN

DISCUSS deliberately leaves the genuine architecture choices open (requirements are solution-neutral). The
solution-architect must resolve these Open Design Decisions:

- **ODD-1 — Port shape / async signature.** Keep it email-centric (`async fn send(&self, to, subject, body)
  -> Result`, minimal change from `EmailSender`) or introduce a structured `Notification{event, recipient,
  payload}` each provider renders (better for webhook/chat, more work). The tension: email providers want
  `to/subject/body`; webhook/chat want structure. Slice 01 (skeleton) carries this. (Risk R1.)
- **ODD-2 — Provider registry & config schema.** The exact env var names/format (`NOTIFICATION_PROVIDERS` as
  a comma list + `SMTP_*`/`WEBHOOK_*`/`EMAIL_API_*` per provider), how "listed" maps to "configured", and the
  registry's ordered-active-set representation.
- **ODD-3 — Fan-out execution model & failure semantics.** Sequential vs concurrent delivery; per-provider
  timeout; spawn-and-detach vs await-all; and precisely how "a provider failure/slowness must never fail or
  stall the originating request" is guaranteed (this is the crux, NFR-3). (Risk R2.)
- **ODD-4 — Provider trait error taxonomy.** How a provider reports retryable vs permanent failure and how
  that maps to the `outcome` metric label + the log line (today providers just return `anyhow::Result`).
- **ODD-5 — Observability/metrics contract.** Confirm the metric name/labels
  (`foundry_notification_deliveries_total{provider,event,outcome}`), the register-at-0 zero-series set, and
  the cardinality bound (reuse the fail-closed test pattern, `metrics_server.rs:374-428`). (Risk R6.)
- **ODD-6 — New-event-type taxonomy.** The notification catalog shape (a bounded Rust enum vs a stringly-typed
  discriminator like the realtime `EventPayload`), and whether the notification catalog aligns with the
  realtime `event_type` model (they are distinct concerns today — `events.rs` is the SSE handler).
- **ODD-7 — Retry / idempotency / durability stance.** Recommended: best-effort at-most-once for v1 (matches
  today); defer outbox-backed durable retry. DESIGN ratifies and decides whether to leave a seam for it.
  (Risk R5.)
- **ODD-8 — Secret handling.** Where provider secrets are read and how they are kept out of `Debug`/log/metric
  paths, given the port carries a `Debug` supertrait (`email.rs:19`) — e.g. a hand-written `Debug` that
  redacts, or dropping the `Debug` bound. (Risk R3.)

Handoff package: `requirements.md` (context, scope + carve-out, brownfield grounding table with real
`file:line`, inline JTBD, FR/NFR/BR, alternatives, risk table, glossary), `user-stories.md` (US-01..06 with
job_id + Elevator Pitch), `acceptance-criteria.md`, the journey trio (`journey-provider-delivery-visual.md`,
`.yaml`, `.feature`), `shared-artifacts-registry.md`, `story-map.md`, `prioritization.md`, `outcome-kpis.md`,
`dor-checklist.md`, and the six slice briefs under `../slices/`.

## Upstream Changes

**None to a prior wave's assumptions — this is a greenfield feature grounded in brownfield seams.** No DISCOVER
or DIVERGE artifacts exist for this feature (no `docs/feature/notification-delivery-providers/diverge/`); the
job statement and personas were established directly in this DISCUSS pass and folded into `requirements.md`
(inline JTBD, no `docs/product/` SSOT — house convention). All seams cited (`EmailSender`, the injection point
`main.rs:265`, the three call sites, the `metrics`/Prometheus sidecar, the `EventPayload` envelope, the
declared `lettre` dep) are shipped and verified by `file:line` in the grounding table. The one honest
brownfield caveat, recorded so DESIGN is not surprised: **there is no real SMTP transport today** — `lettre`
is declared-but-unused and the "env-gated SMTP" the `email.rs:1-5` module doc promises was never built; slice
02 realizes it. "Preserving existing email behavior" therefore means preserving the best-effort/non-fatal
*contract* of the three call sites (today satisfied by the no-op), not preserving a real send that does not yet
exist.
