# Story Map: notification-delivery-providers

## Users: Ops Olivia (operator — configures channels) and Dev Dan (developer — emits once, trusts delivery)
## Goal: route Foundry's notifications through the transports an org already runs (SMTP, chat webhook, hosted email vendor), emitting each notification once and fanning it out to every configured channel with best-effort per-provider failure isolation and per-provider delivery visibility

## Scope (v1 = slices 01–03; 04–06 fast-follow in this feature)

**In scope**: the `NotificationProvider` port (generalizing `EmailSender`), a provider registry + config-driven
selection, four provider kinds (Log/stdout, SMTP, Webhook, Hosted email API), fan-out with best-effort
isolation, per-provider delivery observability, routing the three existing notifications through the
abstraction, and a couple of new notification event types.
**Out of scope** (explicit): **recipient / per-user notification preferences** (opt-in/out, per-channel
routing, digests, quiet-hours) — carved out to the named successor feature `recipient-notification-preferences`;
durable/retried delivery (v1 is best-effort at-most-once); a templating engine; a provider-management UI/CLI
(selection is env-config only).

## Backbone

| Configure channels (operator) | Emit a notification (developer) | Fan out to providers | Providers deliver | Observe delivery (operator) |
|-------------------------------|--------------------------------|----------------------|-------------------|-----------------------------|
| Set `NOTIFICATION_PROVIDERS` + per-provider env; registry built + validated at startup | Call site emits one `notify(event)` — transport-agnostic | Registry delivers to EACH active provider, best-effort & isolated | Log / SMTP / Webhook / Hosted-API adapter sends it | Per-provider `outcome` counted on `/metrics` + one structured log line |
| Unknown/misconfigured → fail-fast; unlisted → inactive; unset → Noop-equivalent | Existing 3 notifications routed through the notifier (best-effort preserved) | One provider failing never fails the request nor blocks others | A provider realizes a real transport (log→smtp→webhook→email_api) | `foundry_notification_deliveries_total{provider,event,outcome}` |
| Secrets read from env into providers only (never logged) | New event types = one catalog entry + one emit call | Slow provider bounded (can't stall the request) | New events delivered like existing ones | Bounded labels (ADR-011); registered at 0 at startup |

---

### Walking Skeleton (the thin end-to-end slice — configure → emit → deliver → observe)

The single minimum task from each backbone activity that makes the end-to-end delivery pipeline work:

- **Configure channels**: `NOTIFICATION_PROVIDERS=log` → registry builds a single Log provider (config parse
  + validation seam established).
- **Emit a notification**: the password-reset call site (`signin.rs:235`) emits through the notifier instead
  of `state.email.send`.
- **Fan out**: the registry delivers to the (single) active provider — the fan-out/isolation shape exists
  even at N=1.
- **Providers deliver**: the Log/stdout provider writes one structured, secret-free line.
- **Observe**: Olivia sees the delivery line (the observability loop established; the metric counter lands in
  slice 03 when there are multiple providers to compare).

This is **US-01**. It carries the whole feature's uncertainty: the **port shape** (ODD-1) and the
**registry + config-selection** seam (ODD-2). Everything after it is adding real transports and fan-out breadth
over a proven skeleton.

### Release 1 (v1): "Route notifications through real channels, emit once, deliver everywhere" — the v1 boundary

- **US-01 Route a notification through a provider I choose (Log/stdout)** — the port + registry + skeleton.
- **US-02 Send real email through our SMTP relay** — the first real transport (realizes `lettre` behind the port).
- **US-03 Emit once, deliver everywhere (fan-out, isolation, per-provider visibility)** — fan-out to all
  active providers with best-effort isolation + the per-provider delivery counter; routes the remaining two
  existing notifications.
- Target outcome: an operator can route Foundry's real notifications through channels they run and see, per
  channel, what delivered; a developer can emit once and trust delivery without transport coupling or
  fragility. KPIs: KPI-1 (delivery observability), KPI-2 (isolation guardrail), KPI-3 (SMTP delivery success).

### Release 2: "More channels" — reach people where they are

- **US-04 Deliver notifications into our chat via a webhook** — generic HTTP POST provider (+ optional signing).
- **US-05 Send email through our hosted email vendor's API** — SendGrid/SES/Postmark-style HTTP provider.
- Target outcome: an operator can add chat and hosted-vendor channels by config alone, each isolated and
  observable like the built-in ones. KPI: KPI-4 (channel adoption breadth).

### Release 3: "More events" — a small catalog of first consumers

- **US-06 Notify people about new events (member_removed, password_changed)** — two new bounded-catalog
  event types delivered through the abstraction with no transport code at the call sites.
- Target outcome: a developer can add a person-facing notification to any feature by emitting one catalog
  event. KPI: KPI-5 (developer extension without transport code).

---

## Priority Rationale

1. **US-01 (Walking Skeleton, P1)** — carries the abstraction's uncertainty: the port shape (email-centric vs
   structured, ODD-1) and the registry + config-selection seam (ODD-2). Until one notification flows through
   the port to a config-selected provider end-to-end, nothing else can be de-risked. Highest learning
   leverage; smallest new surface (a trivial log provider). It is the reason to build first.
2. **US-02 (SMTP, P1/v1)** — the first REAL transport and the first proof the port serves an actual delivery
   mechanism (email via `lettre`). It also establishes config validation + secret handling against a real
   provider. Ordered after the skeleton because you cannot put a transport behind a port that does not exist.
3. **US-03 (Fan-out + isolation + observability, P1/v1 — release gate)** — the riskiest QUALITY assumption:
   that one notification can reach N providers with hard best-effort isolation (a broken channel never fails
   or blocks the request, never sinks the others) AND that delivery is observable per channel. This is the
   feature's core promise ("emit once, deliver everywhere") and the security/operability crux. It needs at
   least two real providers (US-01 + US-02) to fan out to, so it is sequenced third — but the v1 boundary must
   include it; shipping fan-out without isolation + visibility would be shipping the risk, not the value.
4. **US-04 (Webhook, P2)** — adds a non-email channel; exercises the port's non-email shape (R1/ODD-1) for
   real. Depends on the fan-out + isolation machinery (US-03). Lower urgency than v1: email covers the
   existing notifications; chat is additive reach.
5. **US-05 (Hosted email API, P2)** — a second real transport of the same HTTP shape as US-04; mostly reuses
   the HTTP client + secret handling. Additive channel breadth.
6. **US-06 (New event types, P3)** — extends the catalog once the delivery pipeline is proven; lowest risk
   (a catalog entry + emit calls, no transport work). Sequenced last because it delivers new *content* over
   an already-de-risked delivery mechanism.

All six stories trace to outcome KPIs (no orphans). Slicing is by user outcome (observe delivery / send email /
deliver everywhere safely / reach chat / use our vendor / notify about new events), NOT by technical layer —
each slice is a thin end-to-end increment that adds one provider or one capability the operator or developer
can verify in a single dogfood session. The v1 boundary (US-01..US-03) is the minimum that delivers the core
promise safely and observably.
