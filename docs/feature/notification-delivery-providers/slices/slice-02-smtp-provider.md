# Slice 02 — SMTP provider (lettre) behind the port

**Goal**: realize a real **SMTP** provider behind the `NotificationProvider` port using the declared-but-unused
`lettre` dep → an operator sets `NOTIFICATION_PROVIDERS=smtp` + `SMTP_*` against `smtp.acme.internal:587`, hits
`POST /forgot-password` for `maria.santos@acme.example`, and a real reset email lands via the relay.
**Story**: US-02.

**IN scope**
- An **SMTP** provider implementing the `NotificationProvider` port, active when `smtp` is in
  `NOTIFICATION_PROVIDERS` **and** validly configured; delivers the notification as email via the relay.
- Config: `SMTP_HOST`, `SMTP_PORT` (default 587), `SMTP_USERNAME`, `SMTP_PASSWORD`, `SMTP_FROM` — read
  `std::env::var` style; **fail-fast** at startup if `smtp` is listed with a missing required setting (NFR-1),
  with a secret-free, provider-named error.
- **Secret non-leakage**: `SMTP_PASSWORD` never in logs/errors/metrics/`Debug` (NFR-2, ODD-8).
- **Best-effort**: a relay failure returns the request normally and never crashes (NFR-3); with `smtp`
  inactive, no SMTP connection is attempted (Noop-equivalent, NFR-5).
- Acceptance: delivered-via-relay (against a local mailhog/maildev in dogfood), unreachable-relay-isolated,
  missing-setting-fails-fast, inactive-no-attempt, no-password-in-logs.

**OUT of scope**: fan-out to multiple providers + the delivery metric (slice 03 — this slice may deliver
through SMTP as the single active provider); webhook/hosted-API (04/05); retry on failure (v1 best-effort,
NFR-6); routing the invite call sites (slice 03).

**Learning hypothesis**: disproves "the port shape from slice 01 cleanly hosts a real async transport
(`lettre` SMTP) with startup config validation and secret-safe handling" if `lettre`'s async transport doesn't
fit the port signature (ODD-1/ODD-3), if secret-safe `Debug` fights the port's `Debug` supertrait
(`email.rs:19`, ODD-8), or if config validation needs machinery the house `std::env::var` style lacks.

**Seams**: the `NotificationProvider` port + registry (slice 01); `lettre` dep (workspace `Cargo.toml:85-90`,
app `crates/foundry-app/Cargo.toml:78`); the documented-but-unbuilt SMTP seam (`email.rs:1-5`); config style
(`main.rs:99, 242-262`); `FakeEmailSender::set_failing()` (`email.rs:56`) as the failure-injection pattern for
the isolation test.
**Dependencies**: US-01 (port + registry). DESIGN ODD-1 (async signature), ODD-3 (async execution), ODD-8
(secret-safe Debug).
**Effort**: ~1 day (one transport adapter + config validation + secret handling).

> Honest brownfield note: there is **no real SMTP transport today** (`lettre` is declared and never called;
> the `email.rs:1-5` "env-gated SMTP" was never built). This slice is where Foundry email **first actually
> sends**. "Preserve existing email behavior" = preserve the best-effort/non-fatal *contract* of the call
> sites (today satisfied by the no-op), not a pre-existing real send.
