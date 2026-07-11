# Slice 03 — fan-out to all active providers, best-effort isolation, per-provider observability (v1 gate)

**Goal**: deliver one emitted notification to **all** active providers with hard best-effort isolation (a
broken channel never fails/blocks the request nor sinks the others) and per-provider visibility → an operator
sets `NOTIFICATION_PROVIDERS=log,smtp`, breaks the SMTP creds, and sees the log still deliver, the request
still succeed, and `/metrics` show one `smtp` failed next to one `log` delivered.
**Story**: US-03. **Closes the v1 boundary (slices 01–03).**

**IN scope**
- A **fan-out executor** delivering one `${notification}` to EACH active provider in the registry,
  independently and best-effort (ODD-3); a provider failing (refused/5xx/timeout) is caught, logged, and
  **contained** — never propagated to the request, never blocking other providers (NFR-3, BR-2).
- A **slow provider bound** so a hanging provider can't stall the handler (per ODD-3).
- **Per-provider delivery counter** `foundry_notification_deliveries_total{provider,event,outcome}` via the
  `metrics` facade, registered at 0 at startup, on the existing `/metrics` sidecar, bounded-label (NFR-4).
- **Route the two remaining call sites** through the notifier: bootstrap invite (`bootstrap.rs:258`) and
  member invite (`member_invites.rs:189`) — so ALL three existing notifications fan out.
- Acceptance: fan-out-to-all, one-fails-isolated, slow-not-stalled, all-existing-fan-out, isolation
  `@property`, and the metric visible on `/metrics`.

**OUT of scope**: webhook/hosted-API providers (04/05); new event types (06); durable retry (v1 best-effort,
NFR-6); alert thresholds on the counter (DEVOPS follow-up).

**Learning hypothesis**: disproves "one emitted notification can reach N providers with hard best-effort
isolation (zero request failures attributable to delivery, no provider suppressing another) AND per-provider
observability on the existing metrics seam" if isolation can't be guaranteed without stalling/failing the
request under a slow or erroring provider (ODD-3/ODD-4), or if the delivery counter can't stay bounded-label
(ADR-011) on the shipped `metrics` facade.

**Seams**: the registry (slices 01–02); two real providers to fan out to (log + smtp); `metrics` facade +
`/metrics` sidecar (`metrics_server.rs:45-77`, route `:66`); counter template `foundry_token_mutations_total`
(`rate_limit.rs:98, 198-203`); register-at-0 + `describe_counter!` (`main.rs:355-363`); ADR-011 bounded labels
+ cardinality test (`metrics_server.rs:99-108, 374-428`); call sites `bootstrap.rs:258`, `member_invites.rs:189`;
`FakeEmailSender::set_failing()` (`email.rs:56`) for the isolation test.
**Dependencies**: US-01 + US-02 (two providers). DESIGN ODD-3 (fan-out/timeout), ODD-4 (error taxonomy), ODD-5
(metric contract).
**Effort**: ~1–1.5 days (fan-out executor + the metric + routing two call sites — the riskiest quality slice).
