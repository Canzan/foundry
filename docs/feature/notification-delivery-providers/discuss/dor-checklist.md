# Definition of Ready — notification-delivery-providers

9-item hard gate. Each item must PASS with evidence before DESIGN handoff.

## US-01 — Route a notification through a provider I choose (Log/stdout, Walking Skeleton)

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | "Ops Olivia deployed Foundry; password-reset requests fire but the only wired sender is a no-op, so every reset is dropped with no trace." |
| 2 | User/persona with specific characteristics | PASS | Operator running Foundry for an org, shell/log access, selects the channel via config; also serves Dev Dan (call site emits through the notifier). |
| 3 | 3+ domain examples with real data | PASS | `log` active → `POST /forgot-password` for maria.santos@acme.example → one delivery line; unset → silent no-op; `logg` typo → fail-fast. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 3 scenarios (delivered-through-log, no-op-when-unset, unknown-name-fails-fast). |
| 5 | AC derived from UAT | PASS | AC-01.1..01.6, traced to FR-1/2/3/4 + NFR-1/2/5. |
| 6 | Right-sized (1-3 days, 3-7 scenarios) | PASS | Port + registry + trivial log provider + route one call site; 3 scenarios; ~1 day (carries abstraction uncertainty). |
| 7 | Technical notes: constraints/dependencies | PASS | Generalizes `EmailSender` (email.rs:19-22); substitutes registry factory at main.rs:265; ODD-1 (port shape), ODD-2 (config schema). |
| 8 | Dependencies resolved or tracked | PASS | Reuses shipped seams only; no blockers. Port/registry shape flagged as DESIGN inputs (ODD-1/2). |
| 9 | Outcome KPIs with measurable targets | PASS | KPI-1 (100% of reset requests with a provider active produce a delivery record), baseline 0%. |

**US-01 DoR: PASSED**

## US-02 — Send real email through our SMTP relay

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | "Acme runs an SMTP relay every tool sends through, but Foundry has no SMTP transport (lettre is declared and never called)." |
| 2 | User/persona with specific characteristics | PASS | Operator with a relay host/port/credentials, wants Foundry email through it. |
| 3 | 3+ domain examples with real data | PASS | smtp.acme.internal:587 → reset email to maria.santos@acme.example; relay down → failed, request normal; smtp listed without SMTP_HOST → fail-fast. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 4 scenarios (delivered-via-relay, unreachable-relay-isolated, missing-setting-fails-fast, inactive-no-attempt). |
| 5 | AC derived from UAT | PASS | AC-02.1..02.6, traced to FR-5 + NFR-1/2/3/4/5. |
| 6 | Right-sized | PASS | One transport adapter (lettre) behind the port + config validation; 4 scenarios; ~1 day. |
| 7 | Technical notes: constraints/dependencies | PASS | Realizes declared lettre dep (Cargo.toml:85-90) + documented-but-unbuilt seam (email.rs:1-5); SMTP_* config; async shape ODD-1/3. |
| 8 | Dependencies resolved or tracked | PASS | Depends on US-01 (port/registry). No blockers. |
| 9 | Outcome KPIs with measurable targets | PASS | KPI-3 (95%+ delivered with a healthy relay), baseline 0%. |

**US-02 DoR: PASSED**

## US-03 — Emit once, deliver everywhere (fan-out, isolation, observability) — v1 gate

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | "With >1 provider, a failing channel could block/fail the request or break the others, and no one can see which channel delivered." |
| 2 | User/persona with specific characteristics | PASS | Developer emitting a notification (must not own transport/fragility) + operator running multiple channels (needs per-channel health). |
| 3 | 3+ domain examples with real data | PASS | log,smtp both deliver an invite for newadmin@acme.example; relay down → log delivers + smtp failed counted; slow relay → request not stalled. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 4 scenarios (fan-out-all, one-fails-isolated, slow-not-stalled, all-existing-fan-out) + isolation `@property`. |
| 5 | AC derived from UAT | PASS | AC-03.1..03.7, traced to FR-4/6 + NFR-3/4 + BR-2/3. |
| 6 | Right-sized | PASS | Fan-out executor + metric + route the two remaining call sites; 4 scenarios (+1 property); ~1.5 days (riskiest quality). |
| 7 | Technical notes: constraints/dependencies | PASS | Fan-out model ODD-3, error taxonomy ODD-4, metric contract ODD-5; mirrors foundry_token_mutations_total (rate_limit.rs:198-203). |
| 8 | Dependencies resolved or tracked | PASS | Depends on US-01 + US-02 (two providers to fan out to) + the metrics seam. Tracked. |
| 9 | Outcome KPIs with measurable targets | PASS | KPI-2 (0 request failures from delivery, guardrail) + KPI-3 (delivery success). |

**US-03 DoR: PASSED**

## US-04 — Deliver notifications into our chat via a webhook

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | "Acme's ops live in chat and want Foundry events posted there, but Foundry only speaks email — no generic HTTP delivery." |
| 2 | User/persona with specific characteristics | PASS | Operator running a chat/webhook endpoint, may need payload signing. |
| 3 | 3+ domain examples with real data | PASS | hooks.slack.example webhook → member invite JSON for sam.okafor@acme.example; signed payload (secret not leaked); receiver 500 → failed isolated. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 3 scenarios (posted-to-webhook, signed-no-leak, failing-receiver-isolated). |
| 5 | AC derived from UAT | PASS | AC-04.1..04.5, traced to FR-7 + NFR-1/2/3/4. |
| 6 | Right-sized | PASS | HTTP POST provider + optional signing; reuses fan-out/isolation/metric from US-03; 3 scenarios; ~1 day. |
| 7 | Technical notes: constraints/dependencies | PASS | WEBHOOK_URL (required), WEBHOOK_SIGNING_SECRET (optional); payload shape ODD-1; HTTP client. |
| 8 | Dependencies resolved or tracked | PASS | Depends on US-03 (fan-out/isolation/metric). Tracked. |
| 9 | Outcome KPIs with measurable targets | PASS | KPI-4 (webhook channel deliverable via config), baseline 0. |

**US-04 DoR: PASSED**

## US-05 — Send email through our hosted email vendor's API

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | "Acme sends transactional email via a hosted vendor over HTTPS, not raw SMTP; Foundry can't use it." |
| 2 | User/persona with specific characteristics | PASS | Operator whose org uses a hosted email vendor (SendGrid/SES/Postmark-style), has endpoint + key. |
| 3 | 3+ domain examples with real data | PASS | api.postmark.example/email → reset for maria.santos@acme.example; vendor 429 → failed, no retry; missing EMAIL_API_KEY → fail-fast, no key printed. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 3 scenarios (delivered-via-api, rate-limit-isolated-no-retry, key-never-leaks-and-fail-fast). |
| 5 | AC derived from UAT | PASS | AC-05.1..05.5, traced to FR-8 + NFR-1/2/3/4/6. |
| 6 | Right-sized | PASS | HTTP API provider of the same shape as US-04; reuses HTTP client + secret handling; 3 scenarios; ~1 day. |
| 7 | Technical notes: constraints/dependencies | PASS | EMAIL_API_URL/KEY/FROM; secret handling ODD-8; no retry v1 (ODD-7). |
| 8 | Dependencies resolved or tracked | PASS | Depends on US-03 + US-04 (HTTP client + secret handling). Tracked. |
| 9 | Outcome KPIs with measurable targets | PASS | KPI-4 (email_api channel deliverable via config), baseline 0. |

**US-05 DoR: PASSED**

## US-06 — Notify people about new events (member_removed, password_changed)

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | "The catalog is frozen at 3 emails; new features that should tell someone something have nowhere to plug in without bespoke transport wiring." |
| 2 | User/persona with specific characteristics | PASS | Developer adding a feature that must notify a person of an event; wants a catalog entry + one emit call. |
| 3 | 3+ domain examples with real data | PASS | admin removes maria.santos@acme.example from Northwind → member_removed; Maria changes password → password_changed; catalog stays bounded. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 3 scenarios (member_removed-through-channels, password_changed-to-owner, new-event-fans-out-and-isolates). |
| 5 | AC derived from UAT | PASS | AC-06.1..06.5, traced to FR-9 + NFR-3/4 + BR-7. |
| 6 | Right-sized | PASS | Two catalog entries + emit calls; no transport work; 3 scenarios; <1 day. |
| 7 | Technical notes: constraints/dependencies | PASS | Catalog shape ODD-6 (mirrors EventPayload foundry-realtime/src/lib.rs:66-105); keeps event label bounded (BR-7). |
| 8 | Dependencies resolved or tracked | PASS | Depends on US-03 (fan-out delivers whatever event flows through). Tracked. |
| 9 | Outcome KPIs with measurable targets | PASS | KPI-5 (2 new events delivered with 0 transport code at call sites), baseline 3 frozen notifications. |

**US-06 DoR: PASSED**

---

## Overall DoR: PASSED (pending peer-review gate)

All six stories pass all 9 items. The open decisions below are **DESIGN-wave inputs**, not DoR blockers —
requirements are written solution-neutrally and each decision is explicitly tracked in `wave-decisions.md`:

- **ODD-1** Port shape / async signature (email-centric `send(to,subject,body)` vs structured
  `Notification{event,recipient,payload}` each provider renders).
- **ODD-2** Provider registry & config schema (env var names/format + listed→configured validation mapping).
- **ODD-3** Fan-out execution model & failure semantics (sequential vs concurrent, per-provider timeout,
  detach vs await-all; how "must not fail/stall the request" is guaranteed).
- **ODD-4** Provider trait error taxonomy (retryable vs permanent → outcome labels).
- **ODD-5** Observability/metrics contract (exact name/labels, register-at-0 series, cardinality bound).
- **ODD-6** New-event-type taxonomy (bounded enum vs stringly-typed; alignment with the realtime
  `event_type` model).
- **ODD-7** Retry/idempotency/durability stance (recommended: best-effort at-most-once v1; defer
  outbox-backed durable retry).
- **ODD-8** Secret handling (where secrets are read; keeping them out of `Debug`/log/metric paths, given the
  port's `Debug` supertrait).

### Peer review (nw-product-owner-reviewer) — gate

To be run via Task before `*handoff-design`. Expected verdict recorded in `wave-decisions.md` on completion.
Dimension 0 (Elevator Pitch) self-check: every story has an `### Elevator Pitch` with Before/After/Decision,
each "After" anchored to a **real user-invocable entry point** (`POST /forgot-password`, issuing an invite,
removing a member) plus a **concrete observable output** (a structured log line, a delivered email, a
`/metrics` counter series) — no internal-only entry points. JTBD traceability: every story carries a `job_id`
(no `infrastructure-only`). No LeanUX anti-patterns (real personas + data, outcome-focused AC, right-sized
slices). **DoR gate: PASSED pending the peer-review pass.**
