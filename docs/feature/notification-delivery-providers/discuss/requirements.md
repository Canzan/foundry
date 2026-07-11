# Requirements — notification-delivery-providers

## Context

Foundry emits a handful of person-facing notifications today — a password-reset email, a bootstrap
workspace-invite email, and a workspace member-invite email — but they all go through a **single, hard-wired
email port** (`EmailSender`) whose only production implementation is a **no-op** (`NoopEmailSender`). There
is no real transport wired in: `lettre` is a declared dependency that is never called, and the "env-gated
SMTP transport" the `email.rs` module doc promises is a documented-but-unbuilt seam. So in production today,
every notification is silently dropped.

This feature **generalizes the single `EmailSender` port into a pluggable `NotificationProvider` abstraction**
with a provider **registry** + **config-driven selection**, and **fan-out to multiple active providers** with
**per-provider best-effort failure isolation**. It targets, across six thin slices, four provider kinds —
**Log/stdout**, **SMTP** (the declared-but-unused `lettre`, realized behind the port), **Webhook / generic
HTTP POST**, and a **Hosted email API** (SendGrid/SES/Postmark-style) — routes today's existing notifications
through the abstraction, and adds a small catalog of **new notification event types** as first consumers.

The value is two-sided:

1. **Operators** ("Ops Olivia") can route Foundry's notifications through the transports their org already
   runs (an SMTP relay, a chat webhook, a hosted email vendor) via config alone — no patching, no fork.
2. **Developers** ("Dev Dan") can emit one notification event and have it delivered through whatever
   channels are configured, without hand-wiring each transport or blocking their request on delivery.

## Scope (v1 boundary = slices 01–03; 04–06 fast-follow in this feature)

- **In scope**:
  - `NotificationProvider` port generalizing the shipped `EmailSender::send`, a provider **registry**, and
    **config-driven selection** (`NOTIFICATION_PROVIDERS` + per-provider env, house `std::env::var` style).
  - A **Log/stdout** provider (walking skeleton), the **SMTP** provider (realizing `lettre` behind the port),
    **fan-out** to all active providers with **best-effort per-provider isolation**, a **Webhook/HTTP POST**
    provider, a **Hosted email API** provider.
  - Routing the three existing notifications (password reset, bootstrap invite, member invite) through the
    abstraction, preserving their exact best-effort/non-fatal behavior.
  - **Per-provider delivery observability** (a Prometheus counter per `provider`+`event`+`outcome`).
  - A **couple of new notification event types** (e.g. `member_removed`, `password_changed`) as first
    consumers of the abstraction.
- **Out of scope** (deliberate carve-outs):
  - **Recipient / per-user notification preferences** (opt-in/opt-out, per-channel routing, digests,
    quiet-hours) — **carved out into the named successor feature `recipient-notification-preferences`**.
    This feature is the **operator/developer-facing delivery abstraction only**; the downstream recipient
    is acknowledged (more channels reach them) but their preferences are not built here.
  - **Durable retry / guaranteed delivery** (outbox-backed re-send, dead-lettering). v1 preserves today's
    **best-effort at-most-once** semantics per provider; durable retry is deferred (see ODD-7, Risk R5).
  - **Templating / rich rendering engine** — providers deliver the notification content they are handed;
    a themable template system is a follow-up.
  - **A management UI / CLI for providers** — selection is env-config-driven (12-factor), consistent with
    the app's existing configuration style; no web form is introduced (see NFR-7).

## Brownfield grounding (shipped seams — reuse, do not reinvent)

| Seam | Location | Reuse / Role |
|------|----------|--------------|
| `EmailSender` trait — `#[async_trait] async fn send(&self, to, subject, body) -> anyhow::Result<()>`, supertraits `Send + Sync + Debug + 'static` | `crates/foundry-app/src/email.rs:19-22` | **The port to generalize** into `NotificationProvider`. Its async, `Result`-returning, `Debug`-bounded shape is the starting point (ODD-1, ODD-8). |
| `NoopEmailSender` (the **only** production impl today) | `crates/foundry-app/src/email.rs:26-34` | Today's default = silently drops. Becomes the **"no active providers"** behavior (BR-1), preserving backwards-compat. |
| `FakeEmailSender` + `set_failing()` outage-injection | `crates/foundry-app/src/email.rs:38-89` (fail flag `:56`) | Test double + **failure-injection seam** reused to test per-provider isolation (NFR-3). |
| Injection point — `email: Arc::new(NoopEmailSender)` inside the `AppState { … }` literal (251-297) | `crates/foundry-app/src/main.rs:265` | **Composition root** where a provider-registry factory is substituted (no branch exists today; provider selection is added here alongside the env reads at `main.rs:242-262`). |
| `AppState.email: Arc<dyn EmailSender>` | `crates/foundry-app/src/lib.rs:92` | The DI field **generalized to the notifier handle** consumed by every call site. |
| Port re-export | `crates/foundry-app/src/lib.rs:63` (`pub use email::{EmailSender, NoopEmailSender, SentEmail}`) | Extended to export the new port + registry types. |
| `build_router(state: AppState)` | `crates/foundry-app/src/lib.rs:293` | Unchanged — consumes the already-constructed `AppState`; the notifier is injected upstream in `main.rs`. |
| **Password-reset** email send (best-effort, non-fatal) | `crates/foundry-app/src/signin.rs:235` (`submit_forgot` `:203`; trigger `POST /forgot-password`) | The **ONE notification re-routed in slice 01** end-to-end. |
| **Bootstrap workspace-invite** email send (best-effort) | `crates/foundry-app/src/bootstrap.rs:258` (`create_invite` `:204`, route `/invites` `lib.rs:347`) | Routed through the notifier in slice 03 (fan-out). |
| **Workspace member-invite** email send (best-effort) | `crates/foundry-app/src/member_invites.rs:189` | Routed through the notifier in slice 03 (fan-out). |
| `lettre` dependency — **declared but never called** | workspace `Cargo.toml:85-90`; app `crates/foundry-app/Cargo.toml:78` | The **SMTP provider is realized behind the port in slice 02** using this already-present dep. |
| Documented-but-unbuilt "env-gated SMTP transport" seam | `crates/foundry-app/src/email.rs:1-5` (module doc) | Slice 02 **realizes** the SMTP transport this doc promises (honest brownfield note: it is not built today). |
| `metrics` facade + `metrics-exporter-prometheus` sidecar; `install_recorder()` + `GET /metrics` | `crates/foundry-app/src/metrics_server.rs:45-49, 56-77` (recorder installed `main.rs:120`) | Where the **per-provider delivery counter** is registered and exposed (NFR-4). |
| Labelled-counter template — `foundry_token_mutations_total{principal,outcome}` via `metrics::counter!(…).increment(1)` | `crates/foundry-app/src/rate_limit.rs:98, 198-203` | The **exact emission pattern** the delivery metric mirrors. |
| Register-at-0 + `describe_counter!` convention | `crates/foundry-app/src/main.rs:355-363` | New metric family **registered at 0** so it is present on first scrape. |
| ADR-011 bounded-label discipline + fail-closed cardinality test | `crates/foundry-app/src/metrics_server.rs:99-108, 374-428` | The **bounded-label rule** the delivery metric obeys (`provider`,`event`,`outcome` only). |
| Outbox pending-jobs gauge — `outbox_pending_jobs` | `crates/foundry-app/src/main.rs:29` | Existing **delivery-queue precedent** cited for the durable-retry risk discussion (R5, ODD-7). |
| Realtime `EventPayload` — forward-compatible envelope, **stringly-typed** `event_type: String` | `crates/foundry-realtime/src/lib.rs:66-105` (`event_type` `:68`) | The **house event-envelope pattern** DESIGN mirrors for the notification catalog (slice 06). Note: this is the SSE broadcast model (`events.rs` is the SSE HTTP handler), a **distinct** concern from notification delivery — DESIGN decides whether to align them (ODD-6). |
| Config style — direct `std::env::var`, `.env` via `dotenvy::dotenv()`, `DEFAULT_*` consts, `.context()` for required | `crates/foundry-app/src/main.rs:99, 102-262` | Provider selection (`NOTIFICATION_PROVIDERS`, `SMTP_*`, `WEBHOOK_*`, `EMAIL_API_*`) follows this **exactly** — no config-file loader, no figment. |

### The genuinely-new surface (DESIGN owns the exact shapes)

Everything user-visible is a thin adapter over shipped seams, EXCEPT four new pieces, each isolated behind
an open decision so requirements stay solution-neutral:

1. **The `NotificationProvider` port** — a generalization of `EmailSender::send`. Whether it stays
   email-centric (`send(to, subject, body)`) or becomes a structured `Notification{event, recipient,
   payload}` each provider renders is **ODD-1**.
2. **The provider registry + config selection** — the parse of `NOTIFICATION_PROVIDERS` + per-provider env
   into an ordered active set, and startup validation. Env schema is **ODD-2**.
3. **The fan-out executor** — how one notification reaches N providers with best-effort isolation and
   without stalling/failing the request. Sequential-vs-concurrent, timeout, and detach semantics are
   **ODD-3**; the error taxonomy is **ODD-4**.
4. **The notification catalog** — the bounded set of event types (`password_reset`, `workspace_invite`,
   `member_invite`, + the two new ones). Its shape is **ODD-6**.

> This feature is overwhelmingly an **EXTENSION** of the shipped `EmailSender` port, its best-effort call
> sites, and the shipped `metrics`/Prometheus seam — reuse-over-reinvent is the deliberate engineering
> choice, not availability bias. Where a genuine architecture choice exists it is flagged as an ODD, and the
> one genuine **product** carve-out (recipient preferences) is named as the successor feature.

## Jobs To Be Done (inline — no `docs/product/` SSOT in this repo)

This repo deliberately does not use a `docs/product/` SSOT; JTBD is folded in here (as in prior features).
Two jobs drive the feature. Every user story carries a `job_id` referencing one of them.

### JOB-1 `route-notifications-through-existing-transports` — Ops Olivia (operator)

> **When** I stand up Foundry for my org, **I want to** route its notifications through the transports we
> already operate (our SMTP relay, our chat webhook, our hosted email vendor), **so I can** reach my people
> reliably without patching or forking code.

- **Functional**: get every Foundry notification delivered through existing, trusted org channels.
- **Emotional**: confidence that invites and reset links actually arrive; not anxious about a black-box mailer
  or a config typo taking the app down.
- **Social**: be the operator who integrated Foundry cleanly into the org's comms stack in an afternoon.
- **Four forces**:
  - **Push**: today Foundry ships only a no-op sender (`main.rs:265`) — notifications silently vanish; Olivia
    cannot trust it or explain where an invite went.
  - **Pull**: config-driven multi-channel delivery through channels she already runs, observable per channel.
  - **Anxiety**: "Will a bad provider config crash the app? Will our SMTP password or API key leak into logs?"
  - **Habit**: she already sets `SMTP_*` env vars and webhook URLs for other tools; wants the same 12-factor
    ergonomics Foundry already uses (`std::env::var` throughout `main.rs`).
- **Opportunity score (ODI)**: Importance 9, Satisfaction 2 → **Opportunity = 9 + (9−2) = 16 (very high)** —
  today's satisfaction is near-zero because delivery is a no-op.

### JOB-2 `emit-a-notification-once-deliver-everywhere` — Dev Dan (developer)

> **When** I add a feature that must notify someone, **I want to** emit one notification event and have it
> delivered through whatever channels are configured, **so I can** ship without hand-wiring each transport or
> blocking my request on delivery.

- **Functional**: emit once; delivered everywhere configured; zero transport coupling in the handler.
- **Emotional**: relief that he does not own SMTP/HTTP plumbing; confidence his feature won't break if a
  provider is down.
- **Social**: ships the feature without a cross-cutting refactor; trusted not to introduce fragility.
- **Four forces**:
  - **Push**: today he'd call `state.email.send()` directly and know about email; a chat notification would
    need brand-new plumbing at his call site.
  - **Pull**: a single `notify(event)` that fans out to all configured providers.
  - **Anxiety**: "Will a slow or broken provider stall or fail my request handler?"
  - **Habit**: used to the best-effort, non-fatal `state.email.send()` ergonomics (`signin.rs:235`); wants the
    same "fire and keep going."
- **Opportunity score (ODI)**: Importance 8, Satisfaction 3 → **Opportunity = 8 + (8−3) = 13 (high)**.

## Functional requirements

- **FR-1** A `NotificationProvider` **port** generalizes the shipped `EmailSender::send` so that a
  notification can be delivered by any transport adapter. (Exact signature/shape is ODD-1.)
- **FR-2** A **provider registry** selects the **active** providers from configuration
  (`NOTIFICATION_PROVIDERS`, a comma-separated list; per-provider settings via env), built once at startup at
  the composition root (`main.rs:265`). (Env schema is ODD-2.)
- **FR-3** A **Log/stdout** provider writes each delivered notification as a single structured line keyed on
  `provider`, `event`, and recipient — never on payload secrets or unnecessary PII (NFR-2).
- **FR-4** At least **one existing notification** (the **password reset**, `signin.rs:235`) is emitted through
  the abstraction **end-to-end**: `POST /forgot-password` → notifier → active provider(s) → observable
  delivery. Its existing best-effort/non-fatal behavior is preserved exactly (NFR-5).
- **FR-5** An **SMTP** provider (realizing the declared `lettre` dep behind the port) delivers email when
  `smtp` is active and validly configured (`SMTP_HOST`, `SMTP_PORT` default 587, `SMTP_USERNAME`,
  `SMTP_PASSWORD`, `SMTP_FROM`). When `smtp` is not active, no email is attempted (equivalent to today's Noop).
- **FR-6** When **multiple** providers are active, a single emitted notification **fans out to all of them**;
  each provider's delivery is independent (FR/NFR-3). The originating request never waits on, nor fails
  because of, delivery.
- **FR-7** A **Webhook / generic HTTP POST** provider delivers each notification as a JSON body to a
  configured URL (`WEBHOOK_URL`), optionally signed with `WEBHOOK_SIGNING_SECRET`.
- **FR-8** A **Hosted email API** provider (SendGrid/SES/Postmark-style HTTP) delivers email via a configured
  endpoint + API key (`EMAIL_API_URL`, `EMAIL_API_KEY`, `EMAIL_API_FROM`).
- **FR-9** A **couple of new notification event types** (e.g. `member_removed`, `password_changed`) are
  defined as first consumers, emitted through the notifier like the existing ones. (Catalog shape is ODD-6.)
- **FR-10** Configuration is **validated at startup**: a provider that is **listed** in
  `NOTIFICATION_PROVIDERS` but **misconfigured** (missing a required setting) **fails fast** with a clear
  operator error; a provider that is **not listed** is simply **inactive** — never a crash (NFR-1, BR-1).

## Non-Functional Requirements (security & operability — first-class)

### NFR-1 — Config validation & fail-fast (non-enumerable inactivity)
Invalid or incomplete configuration for an **active** provider must fail fast at startup with a clear,
operator-actionable error naming the provider and the missing setting (never a stack trace, never a secret
value). A provider that is **not** in `NOTIFICATION_PROVIDERS` is simply inactive and consumes no config.
An **unknown** provider name in the list is a fail-fast startup error (typo protection).
- **Measurable**: with `NOTIFICATION_PROVIDERS=smtp` and `SMTP_HOST` unset, startup aborts with a message
  naming `smtp` + `SMTP_HOST` and exits non-zero; with `NOTIFICATION_PROVIDERS` unset the app starts and
  delivers nothing (Noop-equivalent); with `NOTIFICATION_PROVIDERS=slack` (unknown) startup aborts naming the
  unknown provider.

### NFR-2 — Secret & PII non-leakage
SMTP credentials, webhook signing secrets, and hosted-API keys must **never** appear in application logs,
error messages, metric labels, or `Debug` output. Delivery logging and metrics key on `provider` name +
`event` type (+ `outcome`), and on the recipient address only where operationally necessary — never on the
notification body, credentials, tokens, or reset/invite `sig` values. Because the port today carries a
`Debug` supertrait (`email.rs:19`), providers holding secrets must not derive a `Debug` that prints them
(ODD-8).
- **Measurable**: a log + `/metrics` scrape after a full `POST /forgot-password` → deliver cycle across all
  four providers contains **no** `SMTP_PASSWORD`, `WEBHOOK_SIGNING_SECRET`, or `EMAIL_API_KEY` value and no
  reset token; reverting the redaction REDs a `@property` no-leak litmus.

### NFR-3 — Failure isolation (best-effort fan-out)
One provider erroring (connection refused, 5xx, timeout, malformed response) must **not** prevent other
active providers from delivering, and must **not** fail — or block — the originating request. This preserves
the semantics already exercised by the three shipped call sites, which log-and-continue on send failure
(`bootstrap.rs:258`, `member_invites.rs:189`, `signin.rs:235`).
- **Measurable**: with `NOTIFICATION_PROVIDERS=log,smtp` and SMTP pointed at an unreachable host, a
  `POST /forgot-password` still returns its normal response, the **log** provider still emits its line, and
  `foundry_notification_deliveries_total{provider="smtp",outcome="failed"}` increments by 1 while
  `{provider="log",outcome="delivered"}` increments by 1.

### NFR-4 — Per-provider delivery observability
Per-provider delivery success/failure is observable as a Prometheus counter
`foundry_notification_deliveries_total{provider,event,outcome}`, emitted via the shipped `metrics` facade
(mirroring `foundry_token_mutations_total`, `rate_limit.rs:198-203`), registered at 0 at startup
(`main.rs:355-363`), and exposed on the existing `/metrics` sidecar (`metrics_server.rs:66`). Labels are
**bounded** (`provider` ∈ {log,smtp,webhook,email_api}; `event` ∈ the notification catalog; `outcome` ∈
{delivered,failed}) per ADR-011 (`metrics_server.rs:99-108`).
- **Measurable**: after N notifications across M active providers, the counter families sum to N×M with the
  correct `outcome` split; a cardinality test (pattern at `metrics_server.rs:374-428`) fails closed if an
  unbounded label is introduced.

### NFR-5 — Backwards-compat (existing notifications preserved, regression-guarded)
The three existing notifications (password reset, bootstrap invite, member invite) must behave **exactly** as
today when routed through the abstraction: best-effort, non-fatal, same recipient, same content. With no
providers configured, behavior is byte-for-byte equivalent to today's `NoopEmailSender` (nothing delivered,
request unaffected). Slices 01–02 are explicitly regression-guarded.
- **Measurable**: the existing acceptance coverage for invite/reset flows (which uses `FakeEmailSender`)
  passes unchanged with the notifier substituted; with `NOTIFICATION_PROVIDERS` unset, no delivery is
  attempted and every existing flow's response is identical.

### NFR-6 — Delivery durability stance (best-effort, at-most-once — v1)
v1 delivery is **best-effort, at-most-once per provider**: a transient provider outage may drop that
provider's copy of a notification, with no automatic retry or de-duplication. This matches today's shipped
behavior exactly. Durable/retried delivery (which the repo's `outbox` seam, `main.rs:29`, could later back)
is explicitly deferred (ODD-7, Risk R5). Stated here so the limitation is a conscious product decision, not a
silent gap.
- **Measurable**: a provider that fails once is not re-invoked for the same notification within the same
  request; the failure is counted (NFR-4) and logged (NFR-2), and the request proceeds (NFR-3).

### NFR-7 — Accessibility (N/A — no new user-facing form)
Provider selection and configuration are **env-based** (12-factor, consistent with `main.rs`). This feature
introduces **no new web form or user-facing UI**, so there is **no new accessibility surface**; WCAG does not
apply here. Flagged explicitly so the omission is intentional, not overlooked. (If a future provider-management
UI is scoped, accessibility re-enters — but that is out of scope, see Scope.)

## Business rules

- **BR-1** A provider **not listed** in `NOTIFICATION_PROVIDERS` is **inactive** — it consumes no config and
  is never constructed. With the list empty/unset, the notifier is a no-op (today's `NoopEmailSender`).
- **BR-2** A provider failure is **best-effort**: logged + counted, **never** fails the user request, **never**
  blocks other active providers.
- **BR-3** Existing notification behavior (password reset, bootstrap invite, member invite) is **preserved
  exactly** — same recipient, same content, same best-effort/non-fatal contract — when routed through the port.
- **BR-4** Provider **secrets never appear** in logs, errors, metric labels, or `Debug` output.
- **BR-5** Provider selection and configuration are **config-driven via environment variables**, consistent
  with the app's existing `std::env::var` house style — no config file, no runtime mutation.
- **BR-6** A provider **listed but misconfigured** is a **fail-fast startup error**; an **unknown** provider
  name in the list is a fail-fast startup error. Only a valid, fully-configured, listed provider is active.
- **BR-7** The **notification catalog** is a **bounded set** of event types; a new event type is an explicit
  catalog addition (so the `event` metric label stays bounded, NFR-4).

## Alternatives considered (constraint rationale)

- **Generalize the port to `NotificationProvider`** (vs keep `EmailSender` and add SMTP only): rejected the
  email-only path — the org's real channels are SMTP **and** a chat webhook **and** a hosted vendor; a
  mailer-only seam re-locks the transport and fails JOB-1. Generalizing once, now, is cheaper than three
  bespoke email-shaped hacks later.
- **Best-effort per-provider isolation** (vs fail-fast on any provider error, or all-or-nothing fan-out):
  chose best-effort because a broken chat webhook must not sink an invite email or fail a user's
  password-reset request. This also **matches the semantics the three shipped call sites already use**
  (log-and-continue on send failure), so it is the least-surprising choice, not a new invention.
- **Best-effort at-most-once (v1)** (vs outbox-backed durable retry now): deferred durable retry. The repo
  has an `outbox` seam (`main.rs:29`) that could back it, but v1's job is to make delivery **possible and
  observable** through real channels; guaranteed delivery is a separate, larger reliability effort (ODD-7).
- **Env-var config** (vs a YAML/figment provider config file): rejected a config file — the house style is
  direct `std::env::var` in `main.rs` (no figment/envy anywhere), and 12-factor env keeps operator ergonomics
  consistent with `DATABASE_URL`, `SESSION_SECRET`, `SMTP_*`-to-be, etc.
- **Carve out recipient preferences** (vs build opt-in/opt-out + per-channel routing now): carved out to the
  named successor feature `recipient-notification-preferences`. Building the delivery abstraction first gives
  preferences something concrete to route over; bundling them would blow scope past the Elephant-Carpaccio
  gate (see `wave-decisions.md` Scope Assessment).
- **Bounded notification catalog** (vs free-form event strings like the realtime `EventPayload`): chose a
  bounded catalog for the notification `event` so the Prometheus label stays bounded (ADR-011). The realtime
  SSE model is stringly-typed for forward-compat over a broadcast bus; notification delivery has a metric
  cardinality constraint the SSE bus does not. DESIGN decides whether/how to align them (ODD-6).

## Risk assessment (surfaced, not managed)

| # | Risk | Category | Probability | Impact | Mitigation |
|---|------|----------|-------------|--------|------------|
| R1 | The port shape must serve email **and** non-email (webhook/chat) providers; an email-centric `send(to,subject,body)` may not fit a chat payload | Technical | Medium | High | ODD-1 (email-centric vs structured `Notification`); slice 01 (walking skeleton) carries this uncertainty first, before any transport is built. |
| R2 | A slow/hanging provider could stall the originating request even with error isolation | Technical | Medium | High | ODD-3 (fan-out execution: concurrency + per-provider timeout + detach); NFR-3 measurable ("request never waits on delivery"). |
| R3 | Provider secrets (SMTP password, signing secret, API key) leak into logs, metrics, or `Debug` | Security | Low | High | NFR-2 + BR-4; logging/metrics key on `provider`/`event`/`outcome` only; ODD-8 (secret-safe `Debug`); `@property` no-leak litmus. |
| R4 | Config misvalidation crashes the app instead of a clean fail-fast, or a typo silently disables a channel | Technical | Medium | Medium | NFR-1 + BR-6 (fail-fast on listed-but-misconfigured and on unknown names; unlisted → inactive); startup-validation acceptance test. |
| R5 | No durable retry → a notification is lost on a transient provider outage | Reliability | Medium | Medium | NFR-6 states best-effort at-most-once (matches today) as a conscious v1 stance; durable retry deferred to ODD-7; `outbox` seam (`main.rs:29`) noted as the future backing. |
| R6 | Metric label cardinality blow-up (unbounded `event` or `provider`) | Technical | Low | Medium | Bounded labels per ADR-011 (`metrics_server.rs:99-108`); BR-7 (bounded catalog); reuse the fail-closed cardinality test pattern (`metrics_server.rs:374-428`). |
| R7 | Regression in the existing invite/reset emails when moved behind the port | Technical | Medium | High | NFR-5 backwards-compat; slices 01–02 regression-guarded; call sites keep exact best-effort semantics; existing `FakeEmailSender`-based acceptance passes unchanged. |

## Glossary (ubiquitous language)

- **Notification** — an event emitted at a call site that should reach a person or channel (e.g. a password
  reset, a member invite).
- **Notification event type** — a member of the bounded **notification catalog**
  (`password_reset`, `workspace_invite`, `member_invite`, + new `member_removed`, `password_changed`).
- **Provider** — a transport adapter implementing the `NotificationProvider` port (`log`, `smtp`, `webhook`,
  `email_api`).
- **Provider registry** — the config-selected, ordered set of **active** providers, built once at startup.
- **Active provider** — one listed in `NOTIFICATION_PROVIDERS` **and** validly configured.
- **Fan-out** — delivering one notification to **all** active providers.
- **Best-effort isolation** — a provider failure is logged + counted, never fails the request, never blocks
  other providers.
- **Notifier** — the `AppState` handle call sites use to emit a notification (generalizes `AppState.email`).
- **Delivery outcome** — a per-provider result (`delivered` | `failed`) recorded to logs + the delivery metric.
- **Recipient preferences** — per-user opt-in/opt-out and channel routing — **out of scope**, owned by the
  successor feature `recipient-notification-preferences`.
