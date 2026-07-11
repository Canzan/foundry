# DESIGN Decisions — notification-delivery-providers

> Morgan (nw-solution-architect), DESIGN wave, application/component scope, **Propose** mode.
> This feature GENERALIZES the shipped single `EmailSender` port into a pluggable `NotificationProvider` port
> + a config-selected registry + a concurrent best-effort fan-out dispatcher, with four transport adapters
> across six thin slices (v1 = 01–03). Paradigm is ESTABLISHED and NOT re-decided: Rust, modular monolith,
> ports-and-adapters via traits, env-config at the composition root, functional-core / imperative-shell.
> Legacy per-feature layout; trunk-based. Requirements SSOT: `../discuss/`. Full design in `architecture.md`;
> decisions grounded in `adr-001..007`. Honest brownfield truth (verified): there is NO real email transport
> today (`main.rs:265` = `NoopEmailSender`; `lettre` declared-but-unused; the `email.rs:1-5` "env-gated SMTP"
> doc was never built) — "preserve existing behavior" = preserve the best-effort/non-fatal *contract* of the
> three call sites (today a no-op).

## Headline findings (grounded in shipped code — read first)

1. **The port to generalize is `EmailSender`** (`email.rs:19-22`): `#[async_trait] Send + Sync + Debug +
   'static`, `async fn send(&self, to, subject, body) -> anyhow::Result<()>`. Only prod impl `NoopEmailSender`
   (`email.rs:26-34`). Verified.
2. **`reqwest` 0.12 (rustls-tls, json) is ALREADY a workspace dep** (`Cargo.toml:104`) — the webhook +
   hosted-API adapters reuse it. **NO new HTTP client** (this corrects the DISCUSS "verify what HTTP client the
   repo uses" — the answer is reqwest, already present).
3. **`lettre` 0.11 is declared with the async tokio rustls SMTP features already on** (`Cargo.toml:85-90`:
   `tokio1`, `tokio1-rustls-tls`, `smtp-transport`, `builder`) — the `SmtpProvider` realizes it with **no
   feature change, no new crate**.
4. **`hmac` 0.12 + `sha2` 0.10 + `secrecy` 0.10 are all present** (`Cargo.toml:61-62,68`) — webhook HMAC
   signing + secret wrapping reuse them. `SecretString` already wraps `SESSION_SECRET` (`main.rs:253`) + the
   machine-token signer (`main.rs:208`). **Net: ZERO new crates for the entire feature.**
5. **The Earned-Trust probe idiom is SHIPPED**: `PROBE_NAMES = &["store","metrics","machine_token"]`
   (`main.rs:46`), `health.startup.refused` structured events + `PROBE_FAILURES_TOTAL{probe_name}`
   (`main.rs:178-232,320-324`), and the metrics self-scrape probe (`metrics_server.rs:227-340`). Provider
   probes plug straight into this (ADR-006).
6. **`AppState` derives only `Clone`, NOT `Debug`** (`lib.rs:65`) — so dropping the `Debug` supertrait from the
   port (ADR-006) compiles cleanly; nothing requires the notifier field to be `Debug`.
7. **The delivery metric reuses the exact shipped machinery**: emission pattern `rate_limit.rs:198-203`,
   register-at-0 `main.rs:355-369`, bounded-label ADR-011 + fail-closed cardinality test
   `metrics_server.rs:99-108,374-428`, `/metrics` sidecar `metrics_server.rs:66`. Verified.
8. **The realtime `EventPayload`** (`foundry-realtime/src/lib.rs:66-105`) is stringly-typed
   (`event_type: String`, `:68`) for an SSE bus with no cardinality bound — a DISTINCT concern from the
   notification catalog, which has a metric-cardinality bound. Do NOT align (ADR-005).

**Net: ONE new port + envelope, ONE dispatcher, FOUR adapters (one per slice 01/02/04/05), ONE composition-root
config+probe loader, ONE delivery-metric seam. ZERO new crates. ZERO migration.**

## Reading checklist

- [x] `../discuss/wave-decisions.md` (D1–D10 + ODD-1..8 — the handoff to resolve)
- [x] `../discuss/requirements.md` (FR-1..10, NFR-1..7, BR-1..7, R1..R7, grounding table — the SSOT)
- [x] `../discuss/user-stories.md` (US-01..06, one per slice) + `../discuss/acceptance-criteria.md` (AC + the 5 @property criteria)
- [x] `../slices/slice-01..06-*.md` (the six thin slice briefs)
- [x] `crates/foundry-app/src/email.rs:1-89` (`EmailSender`, `NoopEmailSender`, `FakeEmailSender`+`set_failing` — the port + doubles to generalize)
- [x] `crates/foundry-app/src/main.rs:99-369` (env reads, injection point `:265`, register-at-0 `:355-369`, the machine-token probe idiom `:178-232`, `PROBE_NAMES` `:46`)
- [x] `crates/foundry-app/src/lib.rs:63,65-92,293` (port re-export, `AppState`+`Clone`-only, `email` field, `build_router`)
- [x] `crates/foundry-app/src/signin.rs:203-247`, `bootstrap.rs:240-267`, `member_invites.rs:170-208` (the three best-effort call sites — verified the `if let Err(..) { warn }` shape + the exact rendered subject/body)
- [x] `crates/foundry-app/src/rate_limit.rs:90-205` (`TOKEN_MUTATIONS_METRIC` const + `counter!(..).increment(1)` — the metric emission to mirror)
- [x] `crates/foundry-app/src/metrics_server.rs:45-108,227-340,374-428` (sidecar, self-scrape probe, bounded-label cardinality test)
- [x] `crates/foundry-realtime/src/lib.rs:60-108` (`EventPayload` stringly-typed envelope — the pattern to mirror-not-align)
- [x] `Cargo.toml:61-62,68,85-90,104` (hmac, sha2, secrecy, lettre, reqwest — all present; zero new crate)
- [x] `docs/feature/workspace-member-invites/design/{architecture.md, wave-decisions.md, adr-001, upstream-changes.md}` — the house DESIGN format mirrored

## Key Decisions (DD-numbered)

| # | Decision | Rationale | ADR / ODD |
|---|---|---|---|
| **DD1** | **Pattern unchanged: modular monolith + ports-and-adapters.** Generalize `EmailSender` → `NotificationProvider` (driven port); add a `Notifier` fan-out dispatcher (domain-side) + four driven adapters + a composition-root config+probe loader. Dependencies point inward: adapters depend on the port; the dispatcher depends on the port; the domain depends on nothing transport-shaped. | Inherited and in force; the feature is an extension of a shipped vertical, not a new architecture. | architecture.md |
| **DD2** | **Structured vendor-neutral `Notification{event,recipient,subject,body}` envelope + port `async deliver(&Notification)->Result<(),DeliveryError>` + `kind()` + `probe()`.** NOT email-centric `send(to,subject,body)`. | The `event` metric label can't be reconstructed from `to/subject/body` (NFR-4); webhook/chat need structure; carrying the rendered subject/body keeps NFR-5 exact. | adr-001 (ODD-1) |
| **DD3** | **Registry = ordered `Vec<Arc<dyn NotificationProvider>>` built by `build_notifier()` at `main.rs`** from `NOTIFICATION_PROVIDERS` + per-provider `std::env::var`; listed→active only if config validates AND probe passes; unset ⇒ empty ⇒ Noop-equivalent. | House 12-factor style (no config file); ordered = deterministic logs/tests; eager construct-and-probe = fail-fast. | adr-002 (ODD-2) |
| **DD4** | **Fan-out = concurrent `JoinSet` (one timeout-wrapped `deliver()` task per provider) awaited within one `NOTIFICATION_DELIVERY_TIMEOUT_MS` (default 5000); `notify()` is INFALLIBLE.** A failing/slow/panicking provider is contained + counted `failed`, never fails/stalls the request nor blocks others. | Structural isolation (concurrency + per-provider timeout + task containment + infallible return) generalizes the call sites' hand-coded log-and-continue to N; await-bounded preserves the shipped synchronous fake assertions (NFR-5). | adr-003 (ODD-3) |
| **DD5** | **Error taxonomy `DeliveryError{Transient,Permanent}` (secret-free msg); metric `outcome` stays binary `{delivered,failed}`; the class lives in the log + error type.** | No retry in v1 (NFR-6) so the class has no runtime effect yet; a third outcome value would widen the bounded label (R6). The class is the forward-compat retry seam (ADR-007). | adr-003 (ODD-4) |
| **DD6** | **`foundry_notification_deliveries_total{provider,event,outcome}`** via the shipped `metrics` facade, register-at-0 over the bounded cross-product (active providers × catalog × outcomes), on the `/metrics` sidecar, bounded labels + mirrored fail-closed cardinality test. | Reuses the shipped emission/register/sidecar/cardinality-test machinery verbatim; no new infra (D7); R6 bounded by construction. | adr-004 (ODD-5) |
| **DD7** | **Bounded closed enum `NotificationEvent` (5 variants incl. the two new events); NOT aligned with the stringly-typed realtime `EventPayload`.** | The `event` label needs a compile-time-bounded domain (BR-7, R6); the SSE bus has no such constraint. Mirror the forward-compat *discipline*, keep the catalogs distinct. | adr-005 (ODD-6) |
| **DD8** | **Best-effort at-most-once for v1; the `Transient|Permanent` class is the ONLY retry seam left; the `outbox` (`main.rs:29`) is named as the deferred backing but untouched.** | Durable retry is a separate, larger reliability effort (its own bounded context); v1's job is possible+observable delivery (NFR-6). | adr-007 (ODD-7) |
| **DD9** | **Drop the `Debug` supertrait from the port + wrap every secret in `SecretString`; secrets read once at the composition root, exposed only at transport-request construction.** Five defense-in-depth no-leak layers. | Dropping the bound removes the leak *class* by construction (enforceable, principle 11); hand-written redacting `Debug` erodes without enforcement. `AppState` is `Clone`-only so it compiles. | adr-006 (ODD-8) |
| **DD10** | **Earned-Trust: every provider implements `probe()`; `build_notifier` enforces wire→probe→use; config-strict / network-soft policy (config lies fail-fast always; network lies degrade by default, hard-fail under `NOTIFICATION_PROBE_STRICT`).** Three-layer enforcement (subtype/structural/behavioral) + probe-of-the-probe. | Principle 12: every external dep is probed; but a best-effort transport being *currently* down must not take the whole app down — so network-soft default reconciles Earned-Trust with NFR-3/NFR-6. Reuses the shipped `health.startup.*` + `PROBE_FAILURES_TOTAL` idiom. | adr-006 |

## ODD resolution index (the DISCUSS handoff, one line each → ADR)

| ODD | Resolution | ADR |
|---|---|---|
| **ODD-1** Port shape | Structured vendor-neutral `Notification{event,recipient,subject,body}` + `NotificationProvider::deliver(&Notification)->Result<(),DeliveryError>` (+`kind`,`probe`); NOT email-centric. | adr-001 |
| **ODD-2** Registry & config | `NOTIFICATION_PROVIDERS` comma-list + `SMTP_*`/`WEBHOOK_*`/`EMAIL_API_*` per-provider `std::env::var`; ordered `Vec` active set; `build_notifier()` at `main.rs`; fail-fast on missing/unknown. | adr-002 |
| **ODD-3** Fan-out & failure semantics | Concurrent `JoinSet` + per-provider timeout, await-bounded, infallible `notify()`; slow/failing/panicking provider contained + counted `failed`, never fails/stalls/blocks. | adr-003 |
| **ODD-4** Error taxonomy | `DeliveryError{Transient,Permanent}` (secret-free); binary metric `outcome`, class in the log; forward-compat retry seam. | adr-003 |
| **ODD-5** Observability | `foundry_notification_deliveries_total{provider,event,outcome}`, bounded labels, register-at-0 cross-product, `/metrics` sidecar, mirrored fail-closed cardinality test. | adr-004 |
| **ODD-6** Event taxonomy | Closed enum `NotificationEvent` (5 variants incl. `member_removed`, `password_changed`); NOT aligned with realtime `EventPayload`; mirror the forward-compat discipline only. | adr-005 |
| **ODD-7** Retry/durability | Ratify best-effort at-most-once v1; `Transient|Permanent` class is the only seam; `outbox` deferred, untouched. | adr-007 |
| **ODD-8** Secret handling | Drop `Debug` supertrait + `SecretString`; read at composition root, expose only at request construction; five no-leak layers + three-layer enforcement. | adr-006 |

## Per-slice architecture notes (each slice ≤3 net-new components; v1 = 01–03)

- **Slice 01 (walking skeleton, US-01)** — NEW: the `NotificationProvider` port + `Notification`/`NotificationEvent`
  value objects, the `Notifier` dispatcher (at N=1), the `LogProvider`, and `build_notifier()` config (unknown-name
  fail-fast). Route ONE call site (`signin.rs:235` password reset) through `notify()`; generalize `AppState.email` →
  `AppState.notifier`. Observable = the structured stdout line (NO metric yet). This slice carries ODD-1 + ODD-2. Net
  new: port+envelope (1 cohesive), dispatcher (1), log adapter (1). Config lives in the composition root.
- **Slice 02 (SMTP, US-02)** — NEW: `SmtpProvider` (realizes `lettre`), its `SecretString` password + TLS-handshake
  `probe()`, fail-fast on missing `SMTP_*`. Net new: 1 adapter. Carries ODD-8 (first real secret) + ODD-6-config half.
- **Slice 03 (fan-out + observability, US-03) — v1 GATE** — the `Notifier` is already fan-out (N≥1); ADD the delivery
  metric seam (register-at-0 + emit + cardinality test) and route the remaining two call sites (`bootstrap.rs:258`,
  `member_invites.rs:189`). Carries ODD-3/ODD-4/ODD-5. Net new: 1 metric seam + 2 call-site extensions.
- **Slice 04 (Webhook, US-04)** — NEW: `WebhookProvider` (reqwest POST JSON + optional HMAC via hmac/sha2 +
  `SecretString` signing secret + reachability `probe()`). Net new: 1 adapter.
- **Slice 05 (Hosted email API, US-05)** — NEW: `EmailApiProvider` (reqwest + `SecretString` API key + reachability
  `probe()`). Net new: 1 adapter.
- **Slice 06 (new events, US-06)** — ADD two `NotificationEvent` variants (`member_removed`, `password_changed`) +
  emit calls at the relevant handlers. Net new: 0 components (catalog + emit only).

## Architecture Summary

- **Pattern**: modular monolith + ports-and-adapters (inherited). The generalized `NotificationProvider` port; a
  concurrent best-effort `Notifier` dispatcher; four driven adapters (log/smtp/webhook/email_api); a composition-root
  `build_notifier()` that validates+constructs+probes; a bounded delivery-metric seam.
- **Paradigm**: Rust, async-trait, tokio, composition-over-inheritance, functional-core / imperative-shell — UNCHANGED.
- **Key components**: see `architecture.md` C4 L1 (System Context) + L2 (Container) + L3 (the fan-out dispatcher).

## Technology Stack

- **Rust** (inherited): async-trait, tokio (`JoinSet` + `tokio::time::timeout`), axum. ZERO new crates.
- **SMTP**: `lettre` 0.11 (declared, unused → realized; async tokio rustls features already on). MIT/Apache-2.0.
- **HTTP (webhook + email_api)**: `reqwest` 0.12 (already a workspace dep; rustls-tls, json). MIT/Apache-2.0. NO
  redundant second HTTP client.
- **Webhook signing**: `hmac` 0.12 + `sha2` 0.10 (present). **Secrets**: `secrecy` 0.10 (present). **Observability**:
  `metrics` 0.23 + `metrics-exporter-prometheus` 0.15 (shipped). All OSS, MIT/Apache-2.0; no proprietary; no new license.
- **Enforcement**: `cargo xtask check-arch` (inherited) + the mirrored bounded-label cardinality unit test + a
  secret-leak/probe-presence AST check (ADR-006 §three-layer enforcement).

## Constraints honored

- ONE binary · ONE Postgres · SHIPPED `/metrics` sidecar · **ZERO new crates · ZERO migration · ZERO new infra**.
- Unset `NOTIFICATION_PROVIDERS` ⇒ Noop-equivalent, byte-for-byte as today (BR-1, NFR-5); slices 01–02 regression-guarded.
- Best-effort per-provider isolation is structural (NFR-3); a provider never fails/stalls the request nor blocks others.
- Fail-fast on listed-but-misconfigured / unknown provider (NFR-1, BR-6); unlisted ⇒ inactive, never constructed.
- Secrets never in logs/errors/metric-labels/`Debug` (NFR-2, BR-4); bounded metric labels (NFR-4, BR-7, ADR-011).
- The domain (`Notification`,`Notifier`,`NotificationEvent`) imports no transport crate; dependencies point inward.

## Constraints for DISTILL / DELIVER

- The v1 boundary is US-01..US-03; DISTILL pins slices 01–03 acceptance first, 04–06 extend the same guarantees.
- The design owns WHAT (port contract, dispatcher semantics, config schema, metric contract, probe contract); the
  software-crafter owns HOW (module decomposition, exact `lettre`/`reqwest` calls, `JoinSet` wiring, `as_str()` bodies).
- No slice ships 4+ new components (see per-slice notes); keep 01–03 independently shippable and thin.

## Earned-Trust (probe-don't-assume) commitments for DISTILL/DELIVER

- **Fan-out isolation PROBED**: with `log,smtp` and SMTP unreachable/hanging, a `POST /forgot-password` returns its
  normal response, the log provider still delivers, and the metric shows `{smtp,failed}` + `{log,delivered}`;
  making `notify()` propagate a provider `Err` REDs the @property (AC-03.2/03.3/03.7, NFR-3).
- **Config fail-fast PROBED**: `smtp` listed with `SMTP_HOST` unset aborts non-zero naming `smtp`+`SMTP_HOST` (no
  secret); `NOTIFICATION_PROVIDERS=logg` aborts naming the unknown+known set; unset ⇒ boots + delivers nothing
  (@property config fail-fast, AC-01.4/01.5/02.4).
- **Secret non-leakage PROBED**: a full four-provider deliver cycle's logs + `/metrics` scrape contain no
  `SMTP_PASSWORD`/`WEBHOOK_SIGNING_SECRET`/`EMAIL_API_KEY` value and no reset/invite token; reverting a redaction
  REDs the litmus (@property secret non-leakage, AC-01.3/02.6/04.2/05.3).
- **Startup probe PROBED**: a provider pointed at an unreachable host degrades (`health.startup.degraded
  probe=notification_<kind>`, default) or refuses under `NOTIFICATION_PROBE_STRICT`; a `probe()` that returns `Ok`
  without connecting fails the self-application gold test (ADR-006).
- **Fan-out completeness + bounded labels PROBED**: N active providers × one notification ⇒ exactly N delivery
  attempts + N counter increments split by outcome; a cardinality test fails closed on an added label; an
  out-of-domain `event`/`provider`/`outcome` value fails the bounded-value @property (AC-03.1/03.4/06.5).
- **Backwards-compat PROBED**: the shipped `FakeEmailSender`-based invite/reset acceptance coverage passes unchanged
  with the notifier substituted; with `NOTIFICATION_PROVIDERS` unset every existing flow's response is identical
  (NFR-5, R7, AC-01.6/02.5/03.5).

## Handoff to DISTILL

The acceptance-designer must pin (mapping to the DISCUSS @property criteria):
1. **Fan-out isolation property** — a single failing/slow/panicking provider never fails/stalls the originating
   request nor blocks the other active providers (the crux, NFR-3; @property failure isolation).
2. **Fail-fast / non-enumerable config** — listed-but-misconfigured OR unknown provider ⇒ non-zero startup with a
   provider-named, secret-free error; unlisted ⇒ inactive, never constructed; unset ⇒ Noop-equivalent (@property config
   fail-fast).
3. **Secret non-leakage** — no secret value + no token in any log/error/metric-label/`Debug` across a full
   four-provider cycle; revert-reds-it (@property secret non-leakage).
4. **Metric zero-series + cardinality bound** — the delivery family is present at 0 on first scrape; labels stay the
   bounded `{provider,event,outcome}` triple with bounded values; cardinality test fails closed (@property bounded labels).
5. **Fan-out completeness** — N active providers ⇒ exactly N attempts + N counter increments per notification
   (@property fan-out completeness).
6. **Backwards-compat** — the three existing notifications behave exactly as today (best-effort, non-fatal, same
   recipient/content) through the notifier; the shipped fake-based coverage stays green.

**Handoff to platform-architect (DEVOPS) — External Integrations Requiring Contract Tests:**
```
- SMTP relay (SMTP/TLS via lettre): Foundry sends transactional email through an org relay.
    Recommended: recorded-interaction / integration tests against a containerized SMTP sink (e.g. MailHog) in CI.
- Chat/Webhook endpoint (HTTPS POST via reqwest, optional HMAC): Foundry POSTs a JSON notification.
    Recommended: consumer-driven contract test (e.g. Pact) for the {event,recipient,subject,body} JSON body +
    the X-Foundry-Signature header shape, in the CI acceptance stage.
- Hosted email vendor API (HTTPS + key via reqwest): Foundry sends email through a vendor (SendGrid/SES/Postmark-style).
    Recommended: consumer-driven contract test against the vendor's documented request schema, in CI.
The Earned-Trust startup probe (ADR-006) is the RUNTIME half of this guarantee; contract tests are the BUILD-TIME half.
Also expose NOTIFICATION_DELIVERY_TIMEOUT_MS + NOTIFICATION_PROBE_STRICT as documented deploy knobs.
```

## Open decisions — RESOLVED (Propose mode; recommended option each; orchestrator auto-accepts)

All eight DISCUSS ODDs (ODD-1..8) are resolved above (ODD index → ADR). No residual blocking decision.

## New ODDs / risks surfaced for DISTILL/DELIVER

- **N-ODD-1 (probe vs best-effort tension)** — RESOLVED as the config-strict/network-soft policy + the
  `NOTIFICATION_PROBE_STRICT` opt-in (ADR-006). Flagged for DELIVER to confirm the default (soft) is the operator's
  expected posture; the flag makes it reversible per-deploy.
- **N-ODD-2 (await-bounded vs spawn-detach emit latency)** — RESOLVED as await-bounded to preserve the shipped
  synchronous fake assertions (NFR-5). Flagged: if a future NFR demands zero emit-path latency, spawn-detach is a
  clean follow-up (the `notify` signature does not change) — DELIVER should not pre-emptively detach.
- **N-ODD-3 (webhook probe side-effect)** — the webhook `probe()` is host-reachability only (no POST), because a
  receiver must not be side-effected by a health ping (ADR-006). Flagged for DISTILL: the webhook happy-path
  acceptance asserts a real POST, but the *probe* must be asserted to make NO POST.
- **N-RISK-1 (lettre async ergonomics)** — the `SmtpProvider` uses `lettre::AsyncSmtpTransport<Tokio1Executor>`;
  DELIVER should confirm the declared feature set builds the async rustls transport as-is (findings say yes) and
  that the TLS-handshake probe does not require sending a MAIL FROM.
