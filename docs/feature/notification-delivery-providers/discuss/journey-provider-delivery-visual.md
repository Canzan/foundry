# Journey (visual): Notification Delivery Providers — configure channels, emit once, deliver everywhere

> Feature: `notification-delivery-providers` | Personas: **Ops Olivia** (operator — configures the channels)
> and **Dev Dan** (developer — emits a notification once and trusts delivery).
> Goal: generalize the shipped single `EmailSender` port into a pluggable `NotificationProvider` abstraction
> with a **provider registry** + **config-driven selection**, and **fan-out to multiple active providers**
> with **per-provider best-effort failure isolation**. Across six slices it delivers four provider kinds
> (Log/stdout, SMTP, Webhook, Hosted email API), routes today's three existing notifications through the
> abstraction, and adds a couple of new notification event types.
> Scope (v1 = slices 01–03): the port + registry + Log provider (skeleton), the SMTP provider, and fan-out +
> isolation + observability. Slices 04–06 (webhook, hosted API, new events) are fast-follow. **Recipient
> preferences are OUT OF SCOPE** — carved out to the successor feature `recipient-notification-preferences`.

## Why this is a thin generalization, not greenfield

Foundry already has the port, the injection point, the call sites, and the metrics seam:

- `EmailSender` (`email.rs:19-22`) is an async, `Result`-returning, `Debug`-bounded trait — the exact shape a
  `NotificationProvider` generalizes. Its only production impl is `NoopEmailSender` (`email.rs:26-34`), wired
  at `main.rs:265` into `AppState.email: Arc<dyn EmailSender>` (`lib.rs:92`).
- Three notifications already flow through `state.email.send(...)`, **all best-effort / non-fatal**: password
  reset (`signin.rs:235`), bootstrap invite (`bootstrap.rs:258`), member invite (`member_invites.rs:189`).
  That log-and-continue contract IS the failure-isolation semantics this feature generalizes to N providers.
- The `metrics` facade + Prometheus `/metrics` sidecar (`metrics_server.rs:45-77`) and the labelled-counter
  template `foundry_token_mutations_total` (`rate_limit.rs:98,198-203`) are the exact seam the per-provider
  delivery counter mirrors.
- `lettre` is a **declared-but-unused** dependency (`Cargo.toml:85-90`); the "env-gated SMTP transport" the
  `email.rs:1-5` module doc promises was **never built**. Slice 02 realizes it behind the port. (Honest
  brownfield note: "existing SMTP behavior" today = the Noop; slice 02 is where email first actually sends.)

Only the port generalization, the registry + config parse, the fan-out executor, and the four provider
adapters are new — everything else is reuse.

## The personas, concretely

**Ops Olivia** (`olivia.okonkwo@acme.example`) is the SRE who runs Foundry for Acme's 40-person eng org. Acme
already operates an internal SMTP relay (`smtp.acme.internal:587`), a chat incoming webhook
(`https://hooks.slack.example/services/T00/B00/xyz`), and a hosted email vendor
(`https://api.postmark.example/email`). She wants Foundry's notifications to flow through those, by config.

**Dev Dan** (`dan.novak@acme.example`) is adding a "remove a member" feature that must tell the removed
person. He wants to emit ONE notification event and have it reach whatever channels Olivia configured — no
transport code in his handler.

**Maria Santos** (`maria.santos@acme.example`) is the downstream recipient — an engineer who requests a
password reset, receives an invite, or gets removed. She simply receives more reliable notifications; her
*preferences* (opt-out, channel choice) are the carved-out successor feature, not built here.

## Emotional arc

Two arcs, one per persona — both **Confidence Building** (Olivia) and **Problem Relief → Confidence** (Dan).

### Operator (Olivia) — Confidence Building
```
DEPLOY                     CONFIGURE                  OBSERVE                    TRUST
"Are notifications    -->  "set NOTIFICATION_    -->  "did it deliver?"    -->   "I can see every channel's
 even firing? It's         PROVIDERS=log,smtp"        watch the log line +       success/failure on /metrics.
 a black box"              low-friction env            /metrics counter           Foundry speaks our stack."
 anxious / uncertain       growing confidence          brief suspense             relief + control
```

### Developer (Dan) — Problem Relief → Confidence
```
NEED                       EMIT                       ISOLATION                  DONE
"I have to notify     -->  "one notify(event)   -->  "what if a provider  -->   "It fans out to every
 the removed member,       call, no transport         is down?"                 configured channel and can't
 do I wire SMTP?"          plumbing"                  a broken channel           break my request. Shipped."
 mild dread                relief                      can't sink my request      confidence + belonging
                                                       growing confidence
```

Olivia's peak tension is the deploy-time black box ("is anything happening?"); collapse it with an immediate,
observable delivery line + a per-provider `/metrics` counter. Dan's peak tension is the fragility fear ("can a
broken channel fail my handler?"); collapse it with hard best-effort isolation — a provider failure is counted
and logged, never propagated. The SAD paths stay calm: a misconfigured provider **fails fast at startup** with
a clear, secret-free operator error (not a mid-request surprise); a runtime provider failure is silently
isolated and merely counted.

---

## Capability 1 — Configuration: the operator selects channels (startup)

```
[Trigger]                     [Step C1: SELECT]              [Step C2: VALIDATE & BUILD]
Olivia deploys Foundry   -->  set NOTIFICATION_PROVIDERS --> app startup builds the
for Acme                      + per-provider env vars        provider registry
  Feels: anxious                Sees: .env / compose env       Sees: clean start, OR a
         (black box)                                            fail-fast config error
  Artifacts:                    Artifacts:                     Artifacts:
   (none yet)                    ${provider_config}             ${provider_registry} (active set)
                                 ${secrets} (env only)          startup validation result
```

### Step C1 — Select the active providers (env, house `std::env::var` style)

```
+-- .env  (12-factor, mirrors main.rs config style) ----------------+
|  NOTIFICATION_PROVIDERS = log,smtp                                 |
|  SMTP_HOST     = smtp.acme.internal                               |
|  SMTP_PORT     = 587                                              |
|  SMTP_USERNAME = foundry-mailer                                   |
|  SMTP_PASSWORD = ${SECRET}          <- never logged (NFR-2)       |
|  SMTP_FROM     = foundry@acme.example                            |
+-------------------------------------------------------------------+
```

Provider selection is read at the composition root (`main.rs:265`, alongside the existing env block
`main.rs:242-262`) with direct `std::env::var` — no config file, no figment (BR-5). An unset
`NOTIFICATION_PROVIDERS` means **no active providers** — the notifier is a no-op, byte-for-byte equivalent to
today's `NoopEmailSender` (BR-1, NFR-5).

### Step C2 — Validate config & build the registry (fail-fast)

```
+-- startup: build_provider_registry(env) --------------------------+
|  parse NOTIFICATION_PROVIDERS -> [log, smtp]                       |
|  for each listed provider:                                        |
|    known name?          else ABORT "unknown provider \"x\""        |
|    required settings?   else ABORT "smtp missing SMTP_HOST"        |
|    construct provider (holds ${secrets}, secret-safe Debug ODD-8) |
|  -> ${provider_registry} = ordered active set                    |
+-------------------------------------------------------------------+
        clean -> app serves          |  invalid -> exit non-zero, clear msg (no secret)
```

A provider **listed but misconfigured** (missing a required setting) or an **unknown** name **fails fast** at
startup with an operator-actionable, secret-free error and a non-zero exit (NFR-1, BR-6). A provider **not
listed** is inactive — never constructed, consumes no config.

---

## Capability 2 — Delivery: emit once, fan out to every active provider (per request)

```
[Trigger]                  [Step D1: EMIT]              [Step D2: FAN-OUT + ISOLATE]      [Step D3: OBSERVE]
a user/dev action     -->  call site emits         -->  registry delivers to EACH     --> per-provider
(POST /forgot-password,    notify(${notification})       active provider, best-effort      outcome logged +
 issue invite, remove                                    isolated                          counted on /metrics
 member)                    Sees: normal request          Sees: (invisible to user;        Sees: log line(s) +
  Feels: (user) neutral           response                 per-provider attempts)            counter increments
  Artifacts:                Artifacts:                    Artifacts:                       Artifacts:
   user action              ${notification}(event,        ${delivery_outcome} per          foundry_notification_
                            recipient, content)            provider (delivered|failed)      deliveries_total{...}
```

### Step D1 — Emit the notification (developer / call-site view)

```
+-- signin.rs::submit_forgot  (POST /forgot-password) --------------+
|  ... build reset link ...                                         |
|  // BEFORE: state.email.send(&email_lower, subject, &body).await  |
|  // AFTER:  state.notifier.notify(                                |
|  //           Notification::password_reset(&email_lower, &body)   |
|  //         ).await   // best-effort, non-fatal (unchanged)       |
+-------------------------------------------------------------------+
```

The call site emits ONE `${notification}` through the notifier; it does not know or care which transports are
active (JOB-2). The best-effort/non-fatal contract is identical to the shipped `state.email.send(...)` (NFR-5,
BR-3). Slice 01 routes the password-reset site; slice 03 routes the two invite sites.

### Step D2 — Fan out to every active provider, isolated (registry view)

```
+-- registry.notify(${notification})  (best-effort, isolated) ------+
|  for each active provider in ${provider_registry}:                |
|     spawn/await bounded attempt (ODD-3):                          |
|        provider.deliver(${notification})                          |
|           Ok  -> ${delivery_outcome}=delivered                    |
|           Err -> ${delivery_outcome}=failed  (log + count; NEVER  |
|                   propagate to the request or other providers)    |
|  return () to the call site  (request never waits on / fails      |
|    because of delivery — NFR-3)                                   |
+-------------------------------------------------------------------+
   log ----[delivered]---->  stdout line
   smtp ---[relay down]--->  failed (counted), log unaffected, request unaffected
```

This is the crux (NFR-3, US-03). A provider failing — refused connection, 5xx, timeout — is caught, logged,
counted, and **contained**: the user's request returns normally, and every OTHER active provider still
delivers. A slow provider is bounded by the fan-out execution model (ODD-3) so it cannot stall the handler.

### Step D3 — Observe per-provider delivery (operator view)

```
+-- GET /metrics  (existing Prometheus sidecar) --------------------+
|  foundry_notification_deliveries_total{provider="log",           |
|      event="password_reset",outcome="delivered"}  12             |
|  foundry_notification_deliveries_total{provider="smtp",          |
|      event="password_reset",outcome="delivered"}  11             |
|  foundry_notification_deliveries_total{provider="smtp",          |
|      event="password_reset",outcome="failed"}      1             |
+-------------------------------------------------------------------+
```

Each attempt increments `foundry_notification_deliveries_total{provider,event,outcome}` — the exact
`metrics::counter!` pattern of `foundry_token_mutations_total` (`rate_limit.rs:198-203`), registered at 0 at
startup (`main.rs:355-363`), bounded-label per ADR-011 (`metrics_server.rs:99-108`). Olivia now sees, per
channel and per event, exactly what delivered and what failed (NFR-4).

---

## Capability 3 — New event types (slice 06): a developer adds a first consumer

```
+-- members.rs::remove_member handler ------------------------------+
|  ... remove Maria from Northwind ...                              |
|  state.notifier.notify(                                           |
|     Notification::member_removed(                                 |
|        "maria.santos@acme.example", "Northwind")                  |
|  ).await;   // one emit call, no transport code                   |
+-------------------------------------------------------------------+
        -> fans out to every active provider (log line, email, chat post),
           counted event="member_removed" per provider
```

`member_removed` and `password_changed` are two new bounded-catalog entries (BR-7). Adding one is a catalog
entry + a single `notify` call — no transport plumbing (JOB-2). The catalog mirrors the house
forward-compatible envelope pattern (`EventPayload`, `foundry-realtime/src/lib.rs:66-105`); whether the
notification catalog aligns with that realtime `event_type` model is DESIGN's call (ODD-6).

---

## Sad / error paths — first-class

### Startup / config sad paths (fail-fast, operator-facing)

```
+-- startup config error (non-zero exit, no secret) ----------------+
|  error: notification provider "smtp" is missing required setting  |
|         SMTP_HOST                                                 |
|  (or)   unknown notification provider "logg"                      |
|         (known: log, smtp, webhook, email_api)                   |
+-------------------------------------------------------------------+
```

| # | Sad path | Trigger | What Olivia sees | Handling |
|---|----------|---------|------------------|----------|
| C-E1 | **Missing required setting** | `smtp` listed, `SMTP_HOST` unset | fail-fast startup error naming provider + setting | non-zero exit; no secret printed (NFR-1, NFR-2) |
| C-E2 | **Unknown provider name** | `NOTIFICATION_PROVIDERS=logg` | fail-fast naming unknown + known set | typo can't silently disable notifications (NFR-1) |
| C-E3 | **No providers configured** | `NOTIFICATION_PROVIDERS` unset | app starts, delivers nothing | Noop-equivalent; existing flows unchanged (BR-1, NFR-5) |
| C-E4 | **Secret in the wrong place** | operator worries about leakage | secrets only ever read from env into the provider | never in logs/errors/metrics/`Debug` (NFR-2, ODD-8) |

### Runtime delivery sad paths (isolated, best-effort — the crux)

```
+-- runtime provider failure (isolated) ----------------------------+
|  smtp: connection refused / 5xx / timeout                         |
|    -> logged (provider=smtp event=... outcome=failed)             |
|    -> counted foundry_notification_deliveries_total{...failed}    |
|    -> request returns normally; log/webhook/email_api unaffected  |
+-------------------------------------------------------------------+
```

| # | Sad path | Trigger | What happens | Handling |
|---|----------|---------|--------------|----------|
| D-E1 | **Provider refused/5xx** | relay down, webhook 500, vendor 4xx | counted `outcome=failed`, logged | request + other providers unaffected (NFR-3) |
| D-E2 | **Provider hangs (slow)** | SMTP/HTTP connect stalls | bounded by fan-out timeout (ODD-3) → counted `failed` | request not stalled (NFR-3) |
| D-E3 | **Vendor rate-limit (429)** | hosted API throttles | counted `failed`; **no retry in v1** | best-effort at-most-once (NFR-6) |
| D-E4 | **All providers fail** | every channel down | each counted `failed`; request STILL returns normally | notification lost (best-effort v1), request never fails (NFR-3, NFR-6) |
| D-E5 | **Transient outage, no retry** | brief blip | that provider's copy dropped, counted `failed` | durable retry deferred (ODD-7, Risk R5) |

> D-E1..D-E5 all share the invariant: **no provider failure ever fails or blocks the originating request, and
> no provider failure ever prevents another provider from delivering.** This generalizes the log-and-continue
> semantics the three shipped call sites already use.

---

## Integration checkpoints

1. **Config → registry**: `${provider_config}` parsed at startup (`main.rs:265`) yields the
   `${provider_registry}` active set; a listed-but-misconfigured or unknown provider fails fast BEFORE the app
   serves (C-E1/C-E2). Single source: the env, read once at the composition root.
2. **Emit → fan-out**: the `${notification}` a call site emits is delivered to EVERY provider in
   `${provider_registry}`; the call site is transport-agnostic (JOB-2). The best-effort/non-fatal contract at
   the call site is identical to the shipped `state.email.send`.
3. **Isolation invariant**: for any active set and any single provider failing, every other active provider
   still delivers AND the request returns normally (NFR-3). A litmus must RED if a provider error propagates
   to the request or suppresses another provider.
4. **Delivery → metric**: each `${delivery_outcome}` increments exactly one
   `foundry_notification_deliveries_total{provider,event,outcome}` series; the sum over one notification with
   N active providers is N, split by outcome (NFR-4). Labels stay bounded (ADR-011).
5. **Secret non-leakage**: `${secrets}` (SMTP password, webhook signing secret, API key) are read from env
   into the provider ONLY; they appear in no log line, error, metric label, or `Debug` output (NFR-2, ODD-8).
   A litmus must RED if a secret value appears in any observable output.
6. **Backwards-compat**: with `NOTIFICATION_PROVIDERS` unset, every existing notification flow behaves exactly
   as today (nothing delivered, request unchanged); slices 01–02 are regression-guarded (NFR-5).

## CLI / config parity note

Selection is env-config only — 12-factor, consistent with `main.rs`'s `std::env::var` style (`DATABASE_URL`,
`SESSION_SECRET`, etc.). No web form or CLI subcommand is introduced (NFR-7: no new accessibility surface).
A provider-management UI/CLI is explicitly out of v1 scope. Operators dogfood by setting env vars and watching
the delivery log line + the `/metrics` counter — the same feedback loop the walking skeleton (slice 01)
establishes.
