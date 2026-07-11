# Acceptance Criteria — notification-delivery-providers

All criteria are observable and testable. Given/When/Then scenarios live in
`journey-provider-delivery.feature` and per-story in `user-stories.md`; this file is the consolidated,
traceable AC index for DISTILL. Every AC traces to a functional requirement (FR-1..10) or a non-functional
requirement (NFR-1..7) in `requirements.md`, and every story traces to an outcome KPI in `outcome-kpis.md`.

## US-01 — Route a notification through a provider I choose (Log/stdout, Walking Skeleton)

| AC-ID | Criterion | Verifies |
|-------|-----------|----------|
| AC-01.1 | A `NotificationProvider` port exists; the password-reset call site emits through the notifier, not `EmailSender::send` directly | FR-1, FR-4 |
| AC-01.2 | `NOTIFICATION_PROVIDERS=log` activates a Log/stdout provider built by the registry at startup | FR-2, FR-3 |
| AC-01.3 | A `POST /forgot-password` with `log` active emits exactly one structured line keyed on `provider`+`event`+recipient — and no reset token or secret | FR-3, FR-4, NFR-2 |
| AC-01.4 | With `NOTIFICATION_PROVIDERS` unset, no delivery is attempted and the request response is unchanged from today | BR-1, NFR-5 |
| AC-01.5 | An unknown provider name aborts startup with a clear message and non-zero exit | NFR-1, BR-6 |
| AC-01.6 | The password-reset request's response and best-effort/non-fatal contract are unchanged | NFR-5, BR-3 |

## US-02 — Send real email through our SMTP relay

| AC-ID | Criterion | Verifies |
|-------|-----------|----------|
| AC-02.1 | An SMTP provider (via `lettre`) delivers email when `smtp` is active and validly configured | FR-5 |
| AC-02.2 | A delivered reset email uses `SMTP_FROM`, reaches the relay, and is counted `provider=smtp outcome=delivered` | FR-5, NFR-4 |
| AC-02.3 | A relay failure returns the request normally, counts `provider=smtp outcome=failed`, and never crashes | NFR-3 |
| AC-02.4 | `smtp` listed with a missing required setting fails fast at startup with a secret-free, provider-named error and non-zero exit | NFR-1, NFR-2, BR-6 |
| AC-02.5 | With `smtp` inactive, no SMTP connection is attempted and existing flows are unchanged | BR-1, NFR-5 |
| AC-02.6 | No `SMTP_PASSWORD` value appears in logs, errors, metrics, or `Debug` output | NFR-2, BR-4 |

## US-03 — Emit once, deliver everywhere (fan-out, isolation, per-provider visibility) — v1 gate

| AC-ID | Criterion | Verifies |
|-------|-----------|----------|
| AC-03.1 | With N active providers, one emitted notification produces N independent delivery attempts | FR-6 |
| AC-03.2 | A provider failure never fails the originating request and never prevents another provider from delivering | NFR-3, BR-2 |
| AC-03.3 | A slow/hanging provider does not stall the request beyond the fan-out execution bound | NFR-3 (ODD-3) |
| AC-03.4 | Each delivery increments `foundry_notification_deliveries_total{provider,event,outcome}` with the correct outcome | NFR-4 |
| AC-03.5 | All three existing notifications (reset, bootstrap invite, member invite) are emitted through the notifier and fan out | FR-4, FR-6, BR-3 |
| AC-03.6 | The counter is registered at 0 at startup and appears on `/metrics` on first scrape | NFR-4 |
| AC-03.7 | Zero request failures are caused by any provider error (guardrail) | NFR-3 |

## US-04 — Deliver notifications into our chat via a webhook

| AC-ID | Criterion | Verifies |
|-------|-----------|----------|
| AC-04.1 | A Webhook provider POSTs each notification as JSON to `WEBHOOK_URL` when `webhook` is active | FR-7 |
| AC-04.2 | With `WEBHOOK_SIGNING_SECRET` set, the POST includes a verifiable signature header, and the secret never appears in body/logs/metrics | NFR-2, BR-4 |
| AC-04.3 | A non-2xx or unreachable receiver counts `provider=webhook outcome=failed` and never fails the request or other providers | NFR-3 |
| AC-04.4 | `webhook` listed without `WEBHOOK_URL` fails fast at startup with a provider-named error | NFR-1, BR-6 |
| AC-04.5 | Successful deliveries count `provider=webhook outcome=delivered` | NFR-4 |

## US-05 — Send email through our hosted email vendor's API

| AC-ID | Criterion | Verifies |
|-------|-----------|----------|
| AC-05.1 | A Hosted email API provider sends email via `EMAIL_API_URL` with `EMAIL_API_KEY` when `email_api` is active | FR-8 |
| AC-05.2 | A 2xx vendor response counts `provider=email_api outcome=delivered`; a non-2xx counts `failed` and is isolated | FR-8, NFR-3, NFR-4 |
| AC-05.3 | No `EMAIL_API_KEY` value appears in logs, errors, metrics, or `Debug` output | NFR-2, BR-4 |
| AC-05.4 | `email_api` listed without a required setting fails fast at startup with a secret-free, provider-named error | NFR-1, NFR-2, BR-6 |
| AC-05.5 | No automatic retry occurs on failure in v1 | NFR-6 |

## US-06 — Notify people about new events (a small catalog of first consumers)

| AC-ID | Criterion | Verifies |
|-------|-----------|----------|
| AC-06.1 | `member_removed` and `password_changed` are catalog event types emittable through the notifier | FR-9 |
| AC-06.2 | Each new event fans out to all active providers and is counted with its own bounded `event` label | FR-9, NFR-4, BR-7 |
| AC-06.3 | Each new event obeys best-effort isolation exactly like the existing notifications | NFR-3 |
| AC-06.4 | Emitting a new event requires no transport code at the call site (one catalog entry + one `notify` call) | FR-9 |
| AC-06.5 | The `event` metric label set remains bounded (a cardinality test fails closed on an unbounded value) | NFR-4, BR-7 |

## Property-shaped criteria (tag `@property` for DISTILL)

- **@property failure isolation**: for any active-provider set and any single provider failing (refused,
  5xx, timeout), every other active provider still delivers AND the originating request returns its normal
  response — zero request failures are attributable to delivery. (AC-03.2, AC-03.3, AC-03.7, AC-04.3, AC-05.2)
- **@property secret non-leakage**: across any full deliver cycle over {log, smtp, webhook, email_api}, no
  `SMTP_PASSWORD`, `WEBHOOK_SIGNING_SECRET`, `EMAIL_API_KEY` value, and no reset/invite token, appears in any
  log line, error, metric label, or `Debug` output; reverting the redaction REDs the litmus. (AC-01.3,
  AC-02.6, AC-04.2, AC-05.3)
- **@property fan-out completeness**: for N active providers and one emitted notification, exactly N delivery
  attempts occur and exactly N counter increments are recorded (one per provider), split by outcome. (AC-03.1,
  AC-03.4)
- **@property config fail-fast**: for any provider listed in `NOTIFICATION_PROVIDERS` that is missing a
  required setting, or any unknown provider name, startup aborts non-zero with a provider-named, secret-free
  error; and any provider NOT listed is inactive and never constructed. (AC-01.5, AC-02.4, AC-04.4, AC-05.4)
- **@property bounded metric labels**: the `{provider,event,outcome}` label domains stay within their bounded
  sets across every notification the catalog can emit; a cardinality test fails closed on an unbounded value.
  (AC-03.4, AC-06.5)

## Traceability

Every AC above maps to a functional (FR-1..10) or non-functional (NFR-1..7) requirement and a business rule
(BR-1..7) in `requirements.md`, and every story traces to an outcome KPI in `outcome-kpis.md`. No orphan AC.
The v1 boundary (US-01..US-03) is fully covered by AC-01.\*, AC-02.\*, AC-03.\*; US-04..US-06 extend the same
guarantees (isolation, non-leakage, bounded observability) to the remaining providers and new events.
