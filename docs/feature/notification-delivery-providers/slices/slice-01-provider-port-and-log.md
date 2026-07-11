# Slice 01 — NotificationProvider port + registry + config selection + log/stdout provider (walking skeleton)

**Goal**: generalize the single `EmailSender` port into a `NotificationProvider` abstraction with a
config-selected provider **registry**, ship a trivial **log/stdout** provider, and re-route ONE existing
notification (the password reset) through it end-to-end → an operator sets `NOTIFICATION_PROVIDERS=log`, hits
`POST /forgot-password`, and sees one structured delivery line.
**Story**: US-01.

**IN scope**
- A `NotificationProvider` **port** generalizing `EmailSender::send` (`email.rs:19-22`) — exact shape per
  DESIGN ODD-1 (email-centric vs structured `Notification`).
- A **provider registry** built once at the composition root, replacing `Arc::new(NoopEmailSender)` at
  `main.rs:265`, selected from `NOTIFICATION_PROVIDERS` via `std::env::var` (house style; ODD-2). Unset ⇒ no
  active providers ⇒ Noop-equivalent (BR-1, NFR-5).
- A trivial **log/stdout** provider: one structured, secret-free line per delivery, keyed on
  `provider`+`event`+recipient (NFR-2).
- Re-route the **password-reset** call site (`signin.rs:235`) to emit through the notifier instead of
  `state.email.send`, preserving the best-effort/non-fatal contract exactly.
- Startup validation: an **unknown** provider name fails fast, non-zero, with a clear message (NFR-1).
- Acceptance: port + registry selection + the log delivery line + the no-op-when-unset + unknown-name-fail-fast
  (store/handler level); the delivery line is dogfooded by running Foundry with `NOTIFICATION_PROVIDERS=log`.

**OUT of scope**: any real transport (SMTP is slice 02); fan-out to multiple providers (slice 03); the
delivery metric counter (slice 03); routing the two invite call sites (slice 03); webhook/hosted-API (04/05);
new event types (06). Only the port, the registry, the log provider, and one re-routed notification.

**Learning hypothesis**: disproves "one generalized `NotificationProvider` port + a config-selected registry
(replacing the `main.rs:265` injection) can carry a real notification end-to-end without changing the call
site's best-effort contract" if the port shape can't serve both email-centric and future non-email providers,
if the registry/config-selection needs machinery the house `std::env::var` style lacks, or if re-routing
`signin.rs:235` regresses the reset flow.

**Seams**: `EmailSender` (`crates/foundry-app/src/email.rs:19-22`) → generalize; `NoopEmailSender`
(`email.rs:26-34`) → the "no active providers" behavior; injection point `main.rs:265` (AppState literal
251-297) → registry factory; DI field `AppState.email` (`lib.rs:92`) → notifier handle; re-export
(`lib.rs:63`); password-reset call site (`signin.rs:235`, `submit_forgot` `:203`); config style
(`main.rs:99, 242-262`).
**Dependencies**: DESIGN ODD-1 (port shape), ODD-2 (config schema). No blockers — reuses shipped seams only.
**Effort**: ~1 day (carries the abstraction's uncertainty; small surface — a trivial provider + the registry).
