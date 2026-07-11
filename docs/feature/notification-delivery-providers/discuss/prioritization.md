# Prioritization: notification-delivery-providers

## Release Priority

| Priority | Release | Target Outcome | KPI | Rationale |
|----------|---------|----------------|-----|-----------|
| 1 | Walking Skeleton (US-01) — port + registry + Log provider | An operator routes a real notification (password reset) through a config-selected provider and *sees* it delivered; the abstraction exists end-to-end | KPI-1 delivery observability | Carries the whole feature's uncertainty (port shape ODD-1, registry/config-selection ODD-2). Nothing else can be de-risked until one notification flows through the port to a selected provider. Highest learning leverage, smallest surface. |
| 1 | v1 core (US-02 SMTP, US-03 fan-out+isolation+observability) | Notifications delivered through a real transport (SMTP) AND, when multiple channels are active, emitted once and fanned out to all with best-effort isolation + per-provider visibility | KPI-2 isolation guardrail, KPI-3 SMTP delivery | US-02 is the first real transport (proves the port serves an actual mechanism + secret handling). US-03 is the core promise + the security/operability crux (a broken channel never fails/blocks the request nor sinks the others) and needs two real providers to fan out to. Together with US-01 they are the v1 boundary. |
| 2 | More channels (US-04 Webhook, US-05 Hosted email API) | An operator adds chat + hosted-vendor channels by config alone, each isolated and observable | KPI-4 channel adoption breadth | Additive reach. US-04 exercises the port's non-email shape for real (R1/ODD-1); US-05 reuses the HTTP client + secret handling. Both ride the fan-out+isolation machinery from US-03. |
| 3 | More events (US-06 new event types) | A developer adds a person-facing notification (member_removed, password_changed) by emitting one catalog event, delivered everywhere configured | KPI-5 developer extension without transport code | Lowest risk (a catalog entry + emit calls, no transport work); delivers new content over an already-de-risked pipeline. |

## Prioritization Scores (Value × Urgency / Effort, 1–5)

| Story | Value | Urgency | Effort | Score | Notes |
|-------|-------|---------|--------|-------|-------|
| US-01 | 5 | 5 | 2 | 12.5 | Port + registry + trivial log provider + route one notification. Small surface, but carries ALL the abstraction uncertainty — walking-skeleton tie-break wins regardless. |
| US-02 | 5 | 4 | 2 | 10.0 | Realizes the declared-but-unused `lettre` behind the port; first real transport; establishes config validation + secret handling. |
| US-03 | 5 | 5 | 3 | 8.3 | The core promise + the security/operability crux: fan-out with best-effort isolation + per-provider observability. Riskiest QUALITY assumption; needs US-01+US-02 to fan out to. |
| US-04 | 4 | 3 | 2 | 6.0 | Webhook/HTTP POST provider; exercises the non-email port shape; optional signing. Additive channel. |
| US-05 | 4 | 3 | 2 | 6.0 | Hosted email API provider; same HTTP shape as US-04; mostly reuses client + secret handling. Additive channel. |
| US-06 | 3 | 2 | 1 | 6.0 | Two new bounded-catalog event types + emit calls; no transport work. New content over a proven pipeline. |

> Tie-break (per user-story-mapping skill): Walking Skeleton > Riskiest Assumption > Highest Value.
> US-01 is the skeleton (P1). US-03 is the riskiest QUALITY assumption (isolation + observability) and closes
> the v1 boundary; US-02 is its precondition (a second real provider to fan out to). US-04/US-05 are additive
> channels; US-06 is additive content.

## Dependency rationale (per slice)

- **US-01** depends on nothing new — reuses `EmailSender` (`email.rs:19-22`), the injection point
  (`main.rs:265`), the DI field (`lib.rs:92`), and the best-effort call-site pattern (`signin.rs:235`).
- **US-02** depends on US-01 (the port + registry to plug SMTP into) + the declared `lettre` dep
  (`Cargo.toml:85-90`).
- **US-03** depends on US-01 + US-02 (two real providers to fan out to) + the `metrics`/Prometheus seam
  (`metrics_server.rs`, `rate_limit.rs:198-203`, `main.rs:355-363`).
- **US-04** depends on US-03 (fan-out + isolation + metric machinery) + an HTTP client transport.
- **US-05** depends on US-03 + US-04 (reuses the HTTP client + secret handling).
- **US-06** depends on US-03 (fan-out delivers whatever event flows through) — no transport dependency.

## Dogfood cadence

Each slice ships a **dogfood moment** the operator/developer verifies in one session:

| Slice | Dogfood moment |
|-------|----------------|
| US-01 | Set `NOTIFICATION_PROVIDERS=log`, hit `POST /forgot-password`, watch one structured delivery line in the logs. |
| US-02 | Point `smtp` at a local mailhog/maildev, issue a reset, see the email land. |
| US-03 | Set `log,smtp`, break the SMTP creds, confirm the log still delivers, the request still succeeds, and `/metrics` shows one `smtp` failed + one `log` delivered. |
| US-04 | Point `webhook` at a local endpoint (or a real chat incoming webhook), fire a notification, see the JSON arrive. |
| US-05 | Point `email_api` at a vendor sandbox / mock, fire a reset, see it accepted; confirm the key never appears in logs. |
| US-06 | Trigger a member removal, see `event=member_removed` delivered + counted across active channels. |

## Backlog Suggestions

| Story | Release | Priority | Outcome Link | Dependencies |
|-------|---------|----------|--------------|--------------|
| US-01 | WS | P1 | KPI-1 | Reused: `EmailSender`, `main.rs:265`, `lib.rs:92`, `signin.rs:235`, config style. |
| US-02 | v1 | P1 | KPI-3 | US-01 (port/registry); declared `lettre` dep. |
| US-03 | v1 (gate) | P1 | KPI-2, KPI-3 | US-01 + US-02; `metrics`/Prometheus seam; routes `bootstrap.rs:258` + `member_invites.rs:189`. |
| US-04 | R2 | P2 | KPI-4 | US-03; HTTP client. |
| US-05 | R2 | P2 | KPI-4 | US-03 + US-04 (HTTP client + secret handling). |
| US-06 | R3 | P3 | KPI-5 | US-03 (fan-out). |

## Scope Assessment (Elephant Carpaccio Gate)

**PASS — 6 stories, 1 bounded context (`foundry-app` delivery pipeline; touches `foundry-realtime`'s event
envelope pattern for the catalog only), estimated ~5–6 days total across the six thin slices.**

Oversized signals checked: stories 6 (≤10 OK) | bounded contexts 1 (≤3 OK — all delivery-side in `foundry-app`,
plus the `metrics` seam and the `EventPayload` envelope pattern) | walking-skeleton integration points:
`EmailSender` port, `main.rs:265` injection, `lib.rs:92` DI field, one call site (`signin.rs:235`) = ~4 seams,
1 new (the registry) — well under the >5 red line | effort ~5–6 days (< 2 weeks) | multiple channels but ONE
coherent capability (pluggable delivery). Each slice is a thin end-to-end increment that adds exactly one
provider or one capability, verifiable in a single dogfood session — no slice ships 4+ new components.

Right-sized; no split needed. The one genuine scope pressure — **recipient/per-user notification
preferences** — was carved OUT to the named successor feature `recipient-notification-preferences` before
this map was drawn (see `wave-decisions.md` Scope Assessment). This feature is the operator/developer-facing
delivery abstraction only.
