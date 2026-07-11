<!-- markdownlint-disable MD024 -->

# User Stories — notification-delivery-providers

## System Constraints (cross-cutting — apply to every story)

- **Config-driven selection, house style**: providers are selected via `NOTIFICATION_PROVIDERS` (a
  comma-separated list) plus per-provider env vars, read with direct `std::env::var` at the composition root
  (`main.rs:265`, alongside the existing env reads at `main.rs:242-262`). No config file, no runtime mutation
  (BR-5). Unset list ⇒ no active providers ⇒ Noop-equivalent (BR-1, NFR-5).
- **Best-effort isolation**: every provider delivery is best-effort — a failure is logged + counted, **never**
  fails the user request, **never** blocks other active providers (BR-2, NFR-3). This preserves the exact
  semantics the three shipped call sites already use (`bootstrap.rs:258`, `member_invites.rs:189`,
  `signin.rs:235`).
- **Secret non-leakage**: SMTP creds, webhook signing secrets, and hosted-API keys never appear in logs,
  errors, metric labels, or `Debug` output; delivery logging/metrics key on `provider`+`event`(+`outcome`),
  never on payload secrets or tokens (BR-4, NFR-2, ODD-8).
- **Observability**: per-provider delivery is counted as `foundry_notification_deliveries_total{provider,event,outcome}`
  via the shipped `metrics` facade, registered at 0 (`main.rs:355-363`), exposed on `/metrics`
  (`metrics_server.rs:66`), bounded-label per ADR-011 (NFR-4, BR-7).
- **Reused seams** (verbatim): the `EmailSender` port (`email.rs:19-22`) as the generalization base, the
  `AppState.email` DI field (`lib.rs:92`), the three best-effort call sites, the `metrics`/Prometheus sidecar,
  and the realtime `EventPayload` envelope pattern (`foundry-realtime/src/lib.rs:66-105`) as the house model
  for the notification catalog. See `requirements.md` grounding table + `shared-artifacts-registry.md`.
- **v1 boundary = US-01..US-03** (walking skeleton + SMTP + fan-out/observability). US-04..US-06 are
  fast-follow in this same feature. **Recipient preferences are OUT OF SCOPE** — carved out to the successor
  feature `recipient-notification-preferences`.
- **JTBD traceability**: JTBD is folded inline (no `docs/product/` SSOT in this repo — see `requirements.md`).
  Every story below is user-visible (none `infrastructure-only`) and carries an explicit `job_id` referencing
  one of the two jobs: `route-notifications-through-existing-transports` (Ops Olivia) or
  `emit-a-notification-once-deliver-everywhere` (Dev Dan).

---

## US-01: Route a notification through a provider I choose — Log/stdout (Walking Skeleton)

`job_id: emit-a-notification-once-deliver-everywhere`

### Elevator Pitch
- **Before**: Ops Olivia stands up Foundry for Acme, a user requests a password reset via
  `POST /forgot-password`, and… nothing happens — the only sender in the build is a no-op (`main.rs:265`), so
  the reset notification silently vanishes and Olivia has no way to even see that it fired.
- **After**: Olivia sets `NOTIFICATION_PROVIDERS=log` in `.env` and restarts; a user submits
  `POST /forgot-password` for `maria.santos@acme.example`, and Olivia sees one structured line in the
  container logs — `notify provider=log event=password_reset to=maria.santos@acme.example outcome=delivered`
  — the same reset notification, now flowing through the new pluggable provider abstraction instead of the
  hard-wired sender.
- **Decision enabled**: Olivia can confirm notifications are actually firing and choose which channel handles
  them — turning a silent black box into an observable, selectable pipeline she controls.

### Problem
Ops Olivia has just deployed Foundry. Password-reset requests come in, but because the only wired sender is
`NoopEmailSender`, every reset notification is dropped with no trace. She cannot tell whether the feature
works, where a notification went, or how to point it at a channel she runs. She needs a way to route a real
notification through a provider she selects and *see* that it was delivered.

### Who
- A workspace/instance **operator** running Foundry for their org | has shell/log access to the running
  process | wants to select the delivery channel via config | motivated to verify notifications fire before
  onboarding users. (Also serves Dev Dan: the call site now emits through the notifier, not the mailer.)

### Solution
Generalize the `EmailSender` port into a `NotificationProvider` port, add a **provider registry** built from
`NOTIFICATION_PROVIDERS`, ship a trivial **Log/stdout** provider, and re-route the **password-reset**
notification (`signin.rs:235`) through the notifier end-to-end. Replace `Arc::new(NoopEmailSender)` at
`main.rs:265` with the registry factory. With `log` active, each delivered notification is one structured,
secret-free stdout line.

### Domain Examples
1. **Happy path** — Olivia sets `NOTIFICATION_PROVIDERS=log`, restarts, and a user submits
   `POST /forgot-password` for `maria.santos@acme.example`; the logs show
   `notify provider=log event=password_reset to=maria.santos@acme.example outcome=delivered`, and the request
   returns its normal "if that email exists, we sent a link" response.
2. **Edge: no providers configured** — Olivia leaves `NOTIFICATION_PROVIDERS` unset; the same
   `POST /forgot-password` returns the identical response and delivers nothing (Noop-equivalent) — no error,
   no crash, no stdout line.
3. **Boundary: unknown provider name** — Olivia fat-fingers `NOTIFICATION_PROVIDERS=logg`; the app **fails
   fast at startup** with `unknown notification provider "logg" (known: log, smtp, webhook, email_api)` and
   exits non-zero, so the typo can't silently disable notifications.

### UAT Scenarios (BDD)
#### Scenario: A password-reset notification is delivered through the selected log provider
Given Olivia has set NOTIFICATION_PROVIDERS to "log" and started Foundry
When a user submits a password-reset request for "maria.santos@acme.example"
Then the reset notification is delivered through the log provider
And Olivia sees one structured log line naming the event and recipient
And the request returns its normal response

#### Scenario: With no providers configured, delivery is a silent no-op
Given Olivia has left NOTIFICATION_PROVIDERS unset
When a user submits a password-reset request
Then the request returns its normal response
And no notification is delivered and no error is raised

#### Scenario: An unknown provider name fails fast at startup
Given Olivia has set NOTIFICATION_PROVIDERS to "logg"
When Foundry starts
Then startup aborts with an error naming the unknown provider and the known ones
And the process exits non-zero

### Acceptance Criteria
- [ ] A `NotificationProvider` port exists that the password-reset call site emits through (not `EmailSender::send` directly).
- [ ] `NOTIFICATION_PROVIDERS=log` activates a Log/stdout provider built by the registry at startup.
- [ ] A `POST /forgot-password` with `log` active emits exactly one structured line keyed on `provider`, `event`, and recipient — and no reset token or secret.
- [ ] With `NOTIFICATION_PROVIDERS` unset, no delivery is attempted and the request response is unchanged from today.
- [ ] An unknown provider name aborts startup with a clear message and non-zero exit.
- [ ] The password-reset request's response and best-effort/non-fatal contract are unchanged (NFR-5).

### Outcome KPIs
- **Who**: operators standing up Foundry
- **Does what**: route a real notification through a selected provider and observe its delivery
- **By how much**: 100% of `POST /forgot-password` requests with `log` active produce exactly one delivery log line
- **Measured by**: log-line count vs reset-request count in a dogfood run
- **Baseline**: 0% (no observable delivery exists today — the sender is a no-op)

### Technical Notes
- Generalizes `EmailSender` (`email.rs:19-22`) → `NotificationProvider`; substitutes the registry factory at `main.rs:265`; keeps `AppState` DI field shape (`lib.rs:92`).
- Carries the abstraction's uncertainty: port shape (ODD-1), registry/config schema (ODD-2). Slice 01 is the walking skeleton.
- Reuses the best-effort call-site pattern from `signin.rs:235`.

---

## US-02: Send real email through our SMTP relay

`job_id: route-notifications-through-existing-transports`

### Elevator Pitch
- **Before**: Acme runs an internal SMTP relay at `smtp.acme.internal:587`, but Foundry can't use it — the
  `lettre` dependency is declared and never called, and the "env-gated SMTP" the code comments promise
  (`email.rs:1-5`) was never built, so invite and reset emails never actually reach anyone's inbox.
- **After**: Olivia sets `NOTIFICATION_PROVIDERS=smtp` plus `SMTP_HOST=smtp.acme.internal`, `SMTP_PORT=587`,
  `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_FROM=foundry@acme.example`, restarts, and a `POST /forgot-password`
  for `maria.santos@acme.example` lands a real reset email in Maria's inbox via the Acme relay.
- **Decision enabled**: Olivia can deliver Foundry's notifications through the mail infrastructure Acme
  already trusts and monitors, instead of a mailer she'd have to bolt on herself.

### Problem
Olivia's org already operates an authenticated SMTP relay that every other internal tool sends through. Foundry
should too — but there is no SMTP transport in the build at all. She needs to point Foundry at
`smtp.acme.internal` with credentials and have the existing notifications (reset, invites) actually arrive,
without regressing their best-effort behavior.

### Who
- An **operator** whose org runs an SMTP relay | has the host/port/credentials | wants Foundry email delivered
  through that relay | motivated to reach users' inboxes reliably.

### Solution
Realize the **SMTP** provider behind the `NotificationProvider` port using the already-declared `lettre` dep
(workspace `Cargo.toml:85-90`). It is active when `smtp` is in `NOTIFICATION_PROVIDERS` and validly configured;
it delivers the notification as an email via the configured relay. When `smtp` is not active, no email is
attempted (Noop-equivalent) — existing behavior preserved exactly (NFR-5).

### Domain Examples
1. **Happy path** — Olivia configures `smtp` against `smtp.acme.internal:587` with a service account; a
   `POST /forgot-password` for `maria.santos@acme.example` delivers a real reset email from
   `foundry@acme.example`, and the delivery is counted `provider=smtp outcome=delivered`.
2. **Edge: relay temporarily refuses** — the relay is briefly down; the reset request still returns normally,
   the failure is counted `provider=smtp outcome=failed`, and no crash occurs (best-effort, NFR-3).
3. **Boundary: smtp listed but SMTP_HOST missing** — Olivia lists `smtp` but forgets `SMTP_HOST`; startup
   **fails fast** with `notification provider "smtp" is missing required setting SMTP_HOST` and exits non-zero
   (NFR-1) — no secret is printed.

### UAT Scenarios (BDD)
#### Scenario: A reset email is delivered through the configured SMTP relay
Given Olivia has configured the smtp provider against "smtp.acme.internal:587"
When a user submits a password-reset request for "maria.santos@acme.example"
Then a reset email is delivered from "foundry@acme.example" via the relay
And the delivery is counted as provider "smtp", outcome "delivered"

#### Scenario: A temporarily unreachable relay does not fail the request
Given the smtp provider is configured but the relay is refusing connections
When a user submits a password-reset request
Then the request returns its normal response
And the failure is counted as provider "smtp", outcome "failed"
And no other behavior is disrupted

#### Scenario: An smtp provider missing a required setting fails fast at startup
Given Olivia has listed "smtp" but not set SMTP_HOST
When Foundry starts
Then startup aborts naming the smtp provider and the missing SMTP_HOST
And no secret value appears in the error
And the process exits non-zero

#### Scenario: With smtp inactive, no email is attempted
Given "smtp" is not in NOTIFICATION_PROVIDERS
When any existing notification fires
Then no SMTP connection is attempted
And behavior is identical to before this feature

### Acceptance Criteria
- [ ] An SMTP provider (via `lettre`) delivers email when `smtp` is active and validly configured.
- [ ] A delivered reset email uses `SMTP_FROM` and reaches the configured relay; delivery is counted `provider=smtp outcome=delivered`.
- [ ] A relay failure returns the request normally, counts `provider=smtp outcome=failed`, and never crashes (NFR-3).
- [ ] `smtp` listed with a missing required setting fails fast at startup with a secret-free, provider-named error and non-zero exit (NFR-1).
- [ ] With `smtp` inactive, no SMTP connection is attempted and existing flows are unchanged (NFR-5).
- [ ] No `SMTP_PASSWORD` value appears in logs, errors, metrics, or `Debug` output (NFR-2).

### Outcome KPIs
- **Who**: operators who run an SMTP relay
- **Does what**: deliver Foundry email through their own relay instead of a no-op
- **By how much**: 95%+ of notifications with a reachable configured relay result in a `provider=smtp outcome=delivered` count
- **Measured by**: `foundry_notification_deliveries_total{provider="smtp",outcome="delivered"}` vs emitted count, over a dogfood window
- **Baseline**: 0% (no SMTP transport exists today; `lettre` is unused)

### Technical Notes
- Realizes the declared-but-unused `lettre` dep + the documented-but-unbuilt seam (`email.rs:1-5`) behind the port.
- Config: `SMTP_HOST`, `SMTP_PORT` (default 587), `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_FROM`; validated at startup (NFR-1).
- Depends on US-01 (port + registry). Async transport shape per ODD-1/ODD-3.

---

## US-03: Emit once, deliver everywhere — fan-out with best-effort isolation & per-provider visibility

`job_id: emit-a-notification-once-deliver-everywhere`

### Elevator Pitch
- **Before**: even with two channels wired, Dev Dan would have to call each transport at his call site and
  worry that a broken chat webhook could throw and fail — or silently swallow — the user's request; and Olivia
  couldn't tell which channel actually delivered.
- **After**: Olivia sets `NOTIFICATION_PROVIDERS=log,smtp`; a single `POST /forgot-password` for
  `maria.santos@acme.example` fans out to **both** — even when the SMTP relay is unreachable, the log line
  still appears, the request still returns normally, and `/metrics` shows
  `foundry_notification_deliveries_total{provider="log",outcome="delivered"} 1` next to
  `{provider="smtp",outcome="failed"} 1`.
- **Decision enabled**: Dan emits one notification and trusts it reaches every configured channel without
  coupling or fragility; Olivia can see, per channel, exactly what delivered and what failed.

### Problem
With more than one active provider, two hazards appear: a failing provider could block or fail the user's
request, and a failing provider could break the *other* providers' delivery. Meanwhile no one can see which
channel succeeded. Dan needs emit-once-deliver-everywhere with hard isolation; Olivia needs per-provider
delivery visibility. The three shipped call sites (reset, bootstrap invite, member invite) must all fan out.

### Who
- A **developer** emitting a notification who must not own transport wiring or fragility | AND an **operator**
  who runs multiple channels and needs per-channel delivery health | motivated by reliability + visibility.

### Solution
A **fan-out executor** delivers one emitted notification to **all** active providers independently, each
best-effort and isolated (one failing neither fails the request nor blocks the others, NFR-3). Each delivery
increments `foundry_notification_deliveries_total{provider,event,outcome}` (NFR-4). The remaining call sites
(`bootstrap.rs:258`, `member_invites.rs:189`) are routed through the notifier so **every** existing
notification fans out.

### Domain Examples
1. **Happy path** — `NOTIFICATION_PROVIDERS=log,smtp`; a bootstrap invite for `newadmin@acme.example` is both
   logged and emailed; `/metrics` shows `{provider="log",outcome="delivered"} 1` and
   `{provider="smtp",outcome="delivered"} 1`.
2. **Edge: one provider down, others deliver** — SMTP relay unreachable; a `POST /forgot-password` still logs
   the delivery, still returns normally, and records `{provider="smtp",outcome="failed"} 1` while
   `{provider="log",outcome="delivered"} 1` — the failure is isolated.
3. **Error/boundary: a slow provider** — the SMTP relay hangs; the request does **not** wait on it (bounded
   by the fan-out execution model, ODD-3) — the user gets their response promptly and the slow provider is
   counted `failed` (timeout) without stalling the handler.

### UAT Scenarios (BDD)
#### Scenario: One notification fans out to all active providers
Given Olivia has set NOTIFICATION_PROVIDERS to "log,smtp" and both are reachable
When a bootstrap workspace invite is issued for "newadmin@acme.example"
Then the invite notification is delivered through both the log and smtp providers
And the delivery metric records one delivered outcome for each provider

#### Scenario: One provider failing does not affect the others or the request
Given NOTIFICATION_PROVIDERS is "log,smtp" and the smtp relay is unreachable
When a user submits a password-reset request for "maria.santos@acme.example"
Then the log provider still delivers the notification
And the request returns its normal response
And the metric records provider "smtp" outcome "failed" and provider "log" outcome "delivered"

#### Scenario: A slow provider does not stall the originating request
Given NOTIFICATION_PROVIDERS is "log,smtp" and the smtp relay hangs on connect
When a user submits a password-reset request
Then the request returns its normal response without waiting on the slow provider
And the smtp delivery is counted as a failure (timeout)

#### Scenario: Every existing notification fans out through the abstraction
Given NOTIFICATION_PROVIDERS is "log,smtp"
When a member invite, a bootstrap invite, and a password reset each fire
Then each is delivered through both active providers
And each delivery is counted per provider and event

### Acceptance Criteria
- [ ] With N active providers, one emitted notification produces N independent delivery attempts.
- [ ] A provider failure never fails the originating request and never prevents another provider from delivering (NFR-3).
- [ ] A slow/hanging provider does not stall the request beyond the fan-out execution bound (ODD-3).
- [ ] Each delivery increments `foundry_notification_deliveries_total{provider,event,outcome}` with the correct outcome (NFR-4).
- [ ] All three existing notifications (reset, bootstrap invite, member invite) are emitted through the notifier and fan out.
- [ ] The counter is registered at 0 at startup and appears on `/metrics` on first scrape (`main.rs:355-363`, `metrics_server.rs:66`).

### Outcome KPIs
- **Who**: developers emitting notifications, and operators running multiple channels
- **Does what**: emit once and have it delivered to every configured channel, with per-channel visibility
- **By how much**: 0 request failures caused by a provider error (guardrail); 100% of deliveries counted with a correct outcome
- **Measured by**: request-error rate attributable to delivery (must stay 0) + delivery-count vs (emitted × active-providers)
- **Baseline**: N/A today (single no-op sender; no fan-out, no visibility)

### Technical Notes
- Fan-out execution model + failure/timeout semantics: ODD-3; provider error taxonomy: ODD-4; metric contract: ODD-5.
- Routes `bootstrap.rs:258` + `member_invites.rs:189` through the notifier (US-01 already routed `signin.rs:235`).
- Metric mirrors `foundry_token_mutations_total` emission (`rate_limit.rs:198-203`), bounded-label per ADR-011.
- Completes the **v1 boundary** (US-01..US-03). Depends on US-01 (port/registry) + US-02 (a second real provider to fan out to).

---

## US-04: Deliver notifications into our chat via a webhook

`job_id: route-notifications-through-existing-transports`

### Elevator Pitch
- **Before**: Acme's ops team lives in a chat channel and wants Foundry security events (like a member invite
  or a password reset firing) to show up there, but Foundry can only speak email — there's no way to POST to
  their incoming webhook.
- **After**: Olivia adds `webhook` to `NOTIFICATION_PROVIDERS` and sets
  `WEBHOOK_URL=https://hooks.slack.example/services/T00/B00/xyz` (and an optional signing secret); the next
  member invite for `sam.okafor@acme.example` posts a JSON payload into the channel, and the delivery is
  counted `provider=webhook outcome=delivered`.
- **Decision enabled**: Olivia can route Foundry notifications into the tools her team already watches, not
  just email — reaching people where they actually are.

### Problem
Not every notification should be an email — Acme's operators want certain events visible in their chat channel
in real time. Foundry has no generic HTTP delivery, so today that's impossible without forking. Olivia needs a
provider that POSTs each notification as JSON to a configured URL, optionally signed so the receiver can verify
authenticity.

### Who
- An **operator** who runs a chat/webhook endpoint (Slack/Teams/generic) | wants Foundry events posted there |
  may need payload signing for authenticity | motivated to reach the team in their working channel.

### Solution
A **Webhook / generic HTTP POST** provider, active when `webhook` is listed and `WEBHOOK_URL` is configured.
It POSTs each notification as a JSON body; if `WEBHOOK_SIGNING_SECRET` is set, it adds a signature header the
receiver can verify. It participates in fan-out and best-effort isolation like every other provider.

### Domain Examples
1. **Happy path** — `webhook` active with `WEBHOOK_URL=https://hooks.slack.example/...`; a member invite for
   `sam.okafor@acme.example` POSTs `{event:"member_invite", to:"sam.okafor@acme.example", ...}` and returns
   2xx; counted `provider=webhook outcome=delivered`.
2. **Edge: signed payload** — `WEBHOOK_SIGNING_SECRET` set; the POST carries a signature header derived from
   the secret + body; the secret itself never appears in the body, logs, or metrics (NFR-2).
3. **Error/boundary: receiver returns 500** — the webhook endpoint 500s; the delivery is counted
   `provider=webhook outcome=failed`, the request and other providers are unaffected (NFR-3).

### UAT Scenarios (BDD)
#### Scenario: A notification is posted to the configured webhook
Given Olivia has activated the webhook provider with a valid WEBHOOK_URL
When a member invite is issued for "sam.okafor@acme.example"
Then a JSON payload describing the event is POSTed to the webhook URL
And the delivery is counted as provider "webhook", outcome "delivered"

#### Scenario: A signed webhook payload carries a verifiable signature without leaking the secret
Given the webhook provider is configured with a WEBHOOK_SIGNING_SECRET
When a notification is delivered
Then the POST includes a signature header derived from the secret
And the signing secret does not appear in the body, logs, or metrics

#### Scenario: A failing webhook receiver is isolated
Given the webhook provider is active and the receiver returns HTTP 500
When a notification fires
Then the delivery is counted as provider "webhook", outcome "failed"
And the originating request and other providers are unaffected

### Acceptance Criteria
- [ ] A Webhook provider POSTs each notification as JSON to `WEBHOOK_URL` when `webhook` is active.
- [ ] With `WEBHOOK_SIGNING_SECRET` set, the POST includes a verifiable signature header, and the secret never appears in body/logs/metrics (NFR-2).
- [ ] A non-2xx or unreachable receiver counts `provider=webhook outcome=failed` and never fails the request or other providers (NFR-3).
- [ ] `webhook` listed without `WEBHOOK_URL` fails fast at startup with a provider-named error (NFR-1).
- [ ] Successful deliveries count `provider=webhook outcome=delivered` (NFR-4).

### Outcome KPIs
- **Who**: operators who run a chat/webhook endpoint
- **Does what**: receive Foundry notifications in their chat channel
- **By how much**: 95%+ of notifications with a reachable webhook result in `provider=webhook outcome=delivered`
- **Measured by**: `foundry_notification_deliveries_total{provider="webhook",outcome="delivered"}` vs emitted count
- **Baseline**: 0% (no HTTP delivery exists today)

### Technical Notes
- Config: `WEBHOOK_URL` (required when active), `WEBHOOK_SIGNING_SECRET` (optional).
- Reuses the fan-out + isolation + metric machinery from US-03; adds an HTTP client transport.
- Payload shape depends on ODD-1 (structured vs email-centric notification). Depends on US-03.

---

## US-05: Send email through our hosted email vendor's API

`job_id: route-notifications-through-existing-transports`

### Elevator Pitch
- **Before**: Acme sends transactional email through a hosted vendor (Postmark/SendGrid/SES) over HTTPS —
  never raw SMTP — for deliverability and analytics; Foundry can't use it, so its email either doesn't send or
  would have to go through a relay Acme doesn't want in the path.
- **After**: Olivia sets `NOTIFICATION_PROVIDERS=email_api`, `EMAIL_API_URL=https://api.postmark.example/email`,
  `EMAIL_API_KEY=…`, `EMAIL_API_FROM=foundry@acme.example`, restarts, and a `POST /forgot-password` for
  `maria.santos@acme.example` sends the reset via the vendor API, counted `provider=email_api outcome=delivered`.
- **Decision enabled**: Olivia can route Foundry email through the deliverability-managed vendor Acme already
  pays for, with the API key kept out of every log line.

### Problem
Olivia's org standardized on a hosted email API for transactional mail and doesn't run open SMTP egress.
Foundry needs to deliver through that vendor's HTTPS API with an API key, without the key ever leaking, and
with the same best-effort/observable semantics as every other provider.

### Who
- An **operator** whose org uses a hosted email vendor (SendGrid/SES/Postmark-style) | has an API endpoint +
  key | wants managed deliverability | motivated to keep secrets safe and delivery observable.

### Solution
A **Hosted email API** provider, active when `email_api` is listed and `EMAIL_API_URL` + `EMAIL_API_KEY` are
configured. It sends each email-shaped notification via the vendor's HTTPS API using the key as a credential
header. The key is used only to construct the request — never logged, never a metric label, never in `Debug`
(NFR-2, ODD-8). It participates in fan-out + isolation + counting like every other provider.

### Domain Examples
1. **Happy path** — `email_api` active against `https://api.postmark.example/email`; a
   `POST /forgot-password` for `maria.santos@acme.example` returns 2xx from the vendor; counted
   `provider=email_api outcome=delivered`.
2. **Edge: vendor rate-limits (429)** — the vendor returns 429; delivery is counted
   `provider=email_api outcome=failed`; request + other providers unaffected (NFR-3); no retry in v1 (NFR-6).
3. **Boundary: email_api listed without EMAIL_API_KEY** — startup **fails fast** naming `email_api` +
   `EMAIL_API_KEY`, with no key value printed (NFR-1, NFR-2).

### UAT Scenarios (BDD)
#### Scenario: A reset email is delivered through the hosted email API
Given Olivia has configured the email_api provider against a hosted vendor endpoint
When a user submits a password-reset request for "maria.santos@acme.example"
Then the reset email is sent via the vendor API from "foundry@acme.example"
And the delivery is counted as provider "email_api", outcome "delivered"

#### Scenario: A vendor rate-limit response is isolated and not retried in v1
Given the email_api provider is active and the vendor returns HTTP 429
When a notification fires
Then the delivery is counted as provider "email_api", outcome "failed"
And the request and other providers are unaffected
And no automatic retry is attempted

#### Scenario: The API key never leaks and a missing key fails fast
Given Olivia has listed "email_api" but not set EMAIL_API_KEY
When Foundry starts
Then startup aborts naming email_api and the missing EMAIL_API_KEY
And no key value appears anywhere in the error or logs

### Acceptance Criteria
- [ ] A Hosted email API provider sends email via `EMAIL_API_URL` with `EMAIL_API_KEY` when `email_api` is active.
- [ ] A 2xx vendor response counts `provider=email_api outcome=delivered`; a non-2xx counts `failed` and is isolated (NFR-3).
- [ ] No `EMAIL_API_KEY` value appears in logs, errors, metrics, or `Debug` output (NFR-2).
- [ ] `email_api` listed without a required setting fails fast at startup with a secret-free, provider-named error (NFR-1).
- [ ] No automatic retry occurs on failure in v1 (NFR-6).

### Outcome KPIs
- **Who**: operators who use a hosted email vendor
- **Does what**: deliver Foundry email through their vendor's managed API
- **By how much**: 95%+ of notifications with a healthy vendor result in `provider=email_api outcome=delivered`
- **Measured by**: `foundry_notification_deliveries_total{provider="email_api",outcome="delivered"}` vs emitted count
- **Baseline**: 0% (no hosted-API transport exists today)

### Technical Notes
- Config: `EMAIL_API_URL`, `EMAIL_API_KEY`, `EMAIL_API_FROM`; validated at startup (NFR-1).
- Reuses fan-out + isolation + metric machinery (US-03) + the HTTP client (US-04). Depends on US-03.
- Secret handling per ODD-8 (secret-safe `Debug`); no retry in v1 (ODD-7).

---

## US-06: Notify people about new events — a small catalog of first consumers

`job_id: emit-a-notification-once-deliver-everywhere`

### Elevator Pitch
- **Before**: when Dev Dan builds "remove a member" or when a user changes their password, there's no way to
  tell the affected person — the only notifications that exist are the three invite/reset emails, and adding a
  new one would mean new transport plumbing at his call site.
- **After**: Dan emits `notify(Notification::member_removed(maria.santos@acme.example, "Northwind"))` from the
  remove-member handler; with any providers configured, Maria is notified through every active channel — a log
  line, an email, and/or a chat post — and the delivery is counted `event=member_removed` per provider, with
  no transport code in Dan's handler.
- **Decision enabled**: Dan can add a person-facing notification to any new feature by emitting one catalog
  event, trusting the abstraction to deliver it everywhere configured.

### Problem
The notification catalog is effectively frozen at the three shipped emails. New features that should tell
someone something (a member was removed; your password was changed) have nowhere to plug in without bespoke
transport wiring. Dan needs to add new event types as first-class catalog entries that flow through the same
fan-out, isolation, and observability as the existing ones.

### Who
- A **developer** adding a feature that must notify a person of an event | wants a catalog entry + one emit
  call | motivated to ship person-facing notifications without owning transports.

### Solution
Add a **couple of new notification event types** to the bounded catalog — `member_removed` (tell a person
they were removed from a workspace) and `password_changed` (tell a user their password changed) — each
emittable via the notifier and delivered through all active providers, counted with its own bounded `event`
label. The catalog shape mirrors the house forward-compatible envelope pattern (`EventPayload`,
`foundry-realtime/src/lib.rs:66-105`); DESIGN decides whether to align them (ODD-6).

### Domain Examples
1. **Happy path (member_removed)** — an admin removes `maria.santos@acme.example` from "Northwind"; Dan's
   handler emits `member_removed`; with `NOTIFICATION_PROVIDERS=log,smtp` active, Maria gets an email and a
   log line appears; counted `event=member_removed` for each provider.
2. **Edge (password_changed)** — `maria.santos@acme.example` changes her password; a `password_changed`
   notification fires to her configured channels ("Your Foundry password was changed"); counted
   `event=password_changed`.
3. **Boundary: catalog stays bounded** — a new event type is an explicit catalog addition (BR-7), so the
   `event` metric label never becomes unbounded; adding one is a one-line catalog entry + an emit call, no
   transport change.

### UAT Scenarios (BDD)
#### Scenario: Removing a member notifies that person through configured channels
Given NOTIFICATION_PROVIDERS is "log,smtp" and an admin removes "maria.santos@acme.example" from "Northwind"
When the remove-member action completes
Then a member_removed notification is delivered to Maria through both providers
And each delivery is counted with event "member_removed"

#### Scenario: Changing a password notifies the account owner
Given "maria.santos@acme.example" changes her password with at least one provider active
When the password change completes
Then a password_changed notification is delivered to her through the active providers
And each delivery is counted with event "password_changed"

#### Scenario: A new event type flows through fan-out and isolation like the existing ones
Given NOTIFICATION_PROVIDERS is "log,smtp" and the smtp relay is unreachable
When a member_removed notification fires
Then the log provider still delivers it and the request is unaffected
And the failure is counted with provider "smtp", event "member_removed", outcome "failed"

### Acceptance Criteria
- [ ] `member_removed` and `password_changed` are catalog event types emittable through the notifier.
- [ ] Each new event fans out to all active providers and is counted with its own bounded `event` label (NFR-4, BR-7).
- [ ] Each new event obeys best-effort isolation exactly like the existing notifications (NFR-3).
- [ ] Emitting a new event requires no transport code at the call site (one catalog entry + one `notify` call).
- [ ] The `event` metric label set remains bounded (a cardinality test fails closed on an unbounded value).

### Outcome KPIs
- **Who**: developers adding person-facing notifications to new features
- **Does what**: add a notification by emitting one catalog event, delivered through all configured channels
- **By how much**: 2 new event types delivered end-to-end via the abstraction with 0 transport code at the call sites
- **Measured by**: presence of `event=member_removed` and `event=password_changed` in the delivery metric during a dogfood run
- **Baseline**: 3 frozen email-only notifications, no extension path today

### Technical Notes
- Catalog shape / alignment with the realtime `event_type` model: ODD-6; mirrors the `EventPayload` forward-compat envelope (`foundry-realtime/src/lib.rs:66-105`).
- Adds two catalog entries + emit calls at the relevant handlers; no transport changes. Depends on US-03 (fan-out) — providers already deliver whatever event flows through.
- Keeps `event` bounded (BR-7) so the metric label cardinality stays safe (ADR-011).
