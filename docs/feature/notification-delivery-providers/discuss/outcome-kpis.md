# Outcome KPIs — notification-delivery-providers

## Feature: notification-delivery-providers

### Objective
Within this feature's slices, let any operator route Foundry's notifications through the transports their org
already runs (SMTP, chat webhook, hosted email vendor) via config alone — so a developer can emit one
notification and have it delivered to every configured channel, safely (a broken channel never fails the
request or sinks the others) and observably (per-channel delivery is visible), replacing today's single
hard-wired no-op sender.

### Baseline (today)
A **single hard-wired email transport** whose only production implementation is a **no-op**
(`NoopEmailSender`, `main.rs:265`): every notification is silently dropped, no transport is configurable, and
there is no delivery visibility. `lettre` is a declared-but-unused dependency; the three existing
notifications (`signin.rs:235`, `bootstrap.rs:258`, `member_invites.rs:189`) reach no one.

### Outcome KPIs

| # | Who | Does What | By How Much | Baseline | Measured By | Type |
|---|-----|-----------|-------------|----------|-------------|------|
| KPI-1 | Operators running Foundry | observe that a notification was delivered through a channel they selected | 100% of `POST /forgot-password` requests with a provider active produce a per-provider delivery record (log line + counter) | 0% (no observable delivery today; sender is a no-op) | delivery-record count vs reset-request count in a dogfood run | Leading (Activation) |
| KPI-2 | Anyone whose request triggers a notification while a provider is failing | still gets a normal response, and other channels still deliver | 0 request failures attributable to a provider error; 100% of other active providers still deliver when one fails | undefined today (single no-op sender; no fan-out) | request-error rate attributable to delivery (must stay 0) + isolation `@property` in the acceptance suite | Guardrail |
| KPI-3 | Operators with a reachable configured transport (SMTP / hosted API) | actually reach recipients' inboxes through their own infrastructure | 95%+ of notifications with a healthy configured transport are counted `outcome=delivered` for that provider | 0% (no real transport exists; email never sends) | `foundry_notification_deliveries_total{provider,outcome="delivered"}` vs emitted count | Leading (Activation) |
| KPI-4 | Operators who run non-default channels (chat webhook, hosted vendor) | add a channel by config alone, with no code change | 2+ additional channel kinds (webhook, email_api) deliverable purely via `NOTIFICATION_PROVIDERS` + env | 0 (only a no-op email path today) | presence of `provider="webhook"` and `provider="email_api"` delivered counts in a dogfood run | Leading (Secondary) |
| KPI-5 | Developers adding a person-facing notification to a feature | add it by emitting one catalog event, with 0 transport code at the call site | 2 new event types (`member_removed`, `password_changed`) delivered end-to-end with 0 transport code at the call sites | 3 frozen email-only notifications, no extension path | code review confirms one catalog entry + one `notify` call per new event; `event=` labels present in the metric | Leading (Secondary) |
| KPI-6 | Anyone operating Foundry | never has a provider secret exposed | 0 occurrences of `SMTP_PASSWORD` / `WEBHOOK_SIGNING_SECRET` / `EMAIL_API_KEY` values in logs, errors, metrics, or `Debug` | undefined today (no secrets configured) | no-secret-leakage `@property` litmus (revert-reds-it) in the acceptance suite | Guardrail |

### Metric Hierarchy
- **North Star**: KPI-3 — **real delivery success rate** through an operator's own transport (issued
  notification → counted `delivered` on a configured provider). The whole point: notifications that actually
  reach people through channels the org trusts, replacing a silent no-op.
- **Leading Indicators**: KPI-1 (delivery is observable at all) is upstream of KPI-3 — you cannot trust
  delivery you cannot see. KPI-4 (more channels available) and KPI-5 (developers can add events) expand the
  surface KPI-3 measures.
- **Guardrail Metrics**: KPI-2 (failure isolation — 0 request failures from delivery) and KPI-6 (0 secret
  leakage) must NOT degrade — they are the security/operability invariants, not optimization targets. A
  regression in either blocks release.

### Measurement Plan
| KPI | Data Source | Collection Method | Frequency | Owner |
|-----|-------------|-------------------|-----------|-------|
| KPI-1 | delivery log lines + `foundry_notification_deliveries_total` | delivery-record count vs reset-request count, dogfood | per-slice dogfood, then weekly | platform-architect (DEVOPS) |
| KPI-2 | acceptance suite + request-error telemetry | isolation `@property` (revert-reds-it) — pass/fail gate; request-error-attributed-to-delivery rate must be 0 | every CI run + weekly | DELIVER + DEVOPS |
| KPI-3 | `foundry_notification_deliveries_total{provider,outcome}` on `/metrics` | `delivered` / (`delivered`+`failed`) per provider, trailing window | weekly | platform-architect (DEVOPS) |
| KPI-4 | `/metrics` label presence | presence of `webhook` + `email_api` delivered series | per-slice dogfood | platform-architect (DEVOPS) |
| KPI-5 | code review + `/metrics` `event` labels | review confirms no transport code at call site; `event=member_removed`/`password_changed` present | per-slice (US-06) | DISCUSS/DELIVER |
| KPI-6 | acceptance suite | no-secret-leakage `@property` litmus — pass/fail gate | every CI run | DELIVER |

> Instrumentation note for DEVOPS: KPI-1/KPI-3/KPI-4 are served by the single counter
> `foundry_notification_deliveries_total{provider,event,outcome}` on the existing `/metrics` sidecar
> (`metrics_server.rs:66`), emitted via the shipped `metrics` facade (mirroring
> `foundry_token_mutations_total`, `rate_limit.rs:198-203`), registered at 0 at startup (`main.rs:355-363`),
> bounded-label per ADR-011. No new dashboard infra is required — it rides the existing Prometheus scrape.
> KPI-2/KPI-6 are enforced as CI gates (acceptance-suite `@property` litmuses), not runtime dashboards.
> Guardrail alert thresholds (e.g. a spike in `outcome=failed` for a provider) are a DEVOPS follow-up.

### Hypothesis
We believe that a pluggable `NotificationProvider` abstraction with config-driven selection and best-effort
fan-out will let operators route Foundry's notifications through the transports they already run, and let
developers emit once and deliver everywhere, without patching code.
We will know this is true when 95%+ of notifications with a healthy configured transport are counted
`delivered` for that provider (KPI-3), while 0 request failures are ever caused by a provider error (KPI-2)
and no provider secret ever appears in observable output (KPI-6).
