# DISTILL — Test Scenario Catalog: notification-delivery-providers

> Quinn (nw-acceptance-designer), DISTILL wave. Core feature, functional acceptance only
> (no DEVOPS wave — consistent with prior features). Framework: **cucumber-rs** (house
> standard). SSOT for the executable specification is
> `crates/foundry-acceptance/tests/features/notification-delivery-providers.feature`;
> step definitions in `crates/foundry-acceptance/src/steps/feature_notification_delivery_providers.rs`.

## Wave-decision reconciliation (HARD GATE) — PASSED, 0 contradictions

DESIGN's `../design/wave-decisions.md` § "ODD resolution index" resolves each DISCUSS
ODD-1..8 explicitly and consistently (port shape, registry/config, fan-out semantics,
error taxonomy, observability, event catalog, retry policy, secret handling). No DISCUSS
decision is contradicted by DESIGN. No DEVOPS wave ran for this feature. Proceeded to
scenario writing.

## Harness boundary (the external-double rationale)

| Port class | Treatment | Mechanism in this feature |
|---|---|---|
| Driving — operator config | real | `build_notifier()` composition-root loader (fail-fast on unknown/misconfigured) |
| Driving — app flows | real | shipped `POST /forgot-password`, bootstrap + member invites, remove-member, password-change through the in-process axum harness |
| Driving — observability | real | `/metrics` sidecar + recording-provider double |
| Driven internal — store | real | Postgres via testcontainers (the shipped `InProcHarness`) |
| Driven external — transports | **fake, in-process** | recording log provider, local webhook receiver, fake SMTP / hosted-API recorder — mirrors how `FakeEmailSender` is wired today; NO real third-party SMTP/SendGrid/webhook call leaves the process |

The webhook `probe()` is asserted to make **no POST** (host-reachability only, N-ODD-3);
the webhook happy path asserts a **real POST** to the local receiver. Fan-out is
await-bounded so the synchronous recorder assertions hold (N-ODD-2).

Every scenario enters through a driving port (Mandate 1) — never an internal function.
DELIVER owns the harness provider seam (a `spawn_with_providers`-style composition root +
the recording doubles); DISTILL ships the compiling `@pending` scaffolds that pin the
observable contract.

## Scenario catalog (27 scenarios; SSOT = the `.feature` file)

| # | Slice | Scenario | US | AC | ADR / ODD | Tags |
|---|---|---|---|---|---|---|
| 1 | 01 | Password reset delivered through chosen log provider (**walking skeleton**) | US-01 | AC-01.1/01.2/01.3 | adr-001, adr-002 | `@walking_skeleton @real-io` |
| 2 | 01 | No providers configured ⇒ silent no-op, response unchanged | US-01 | AC-01.4 | adr-002, NFR-5 | `@real-io` |
| 3 | 01 | Unknown provider name fails fast at startup | US-01 | AC-01.5 | adr-002, @property config fail-fast | `@config @error @property @real-io` |
| 4 | 01 | Delivered log line carries no reset token or secret | US-01 | AC-01.3 | adr-006, @property secret non-leakage | `@security @real-io` |
| 5 | 02 | Reset email delivered through configured SMTP relay | US-02 | AC-02.1/02.2 | adr-001, NFR-4 | `@real-io` |
| 6 | 02 | Temporarily unreachable relay does not fail the request | US-02 | AC-02.3 | adr-003, NFR-3 | `@error @real-io` |
| 7 | 02 | SMTP missing required setting fails fast | US-02 | AC-02.4 | adr-002/006, @property config fail-fast | `@config @error @property @real-io` |
| 8 | 02 | SMTP inactive ⇒ no email attempted, behavior unchanged | US-02 | AC-02.5 | adr-002, NFR-5 | `@real-io` |
| 9 | 02 | SMTP password never leaks across a delivery cycle | US-02 | AC-02.6 | adr-006, @property secret non-leakage | `@security @real-io` |
| 10 | 03 | One notification fans out to every active provider | US-03 | AC-03.1 | adr-003, @property fan-out completeness | `@property @real-io` |
| 11 | 03 | One failing provider affects neither the others nor the request | US-03 | AC-03.2/03.7 | adr-003, @property failure isolation | `@error @property @real-io` |
| 12 | 03 | Slow provider does not stall the originating request | US-03 | AC-03.3 | adr-003 (ODD-3) | `@error @real-io` |
| 13 | 03 | Every existing notification fans out through the abstraction | US-03 | AC-03.5 | adr-003, FR-4/FR-6 | `@real-io` |
| 14 | 03 | Delivery metric registered at zero on first scrape | US-03 | AC-03.6 | adr-004 | `@real-io` |
| 15 | 03 | Delivery metric labels stay bounded (cardinality fails closed) | US-03 | AC-03.4 | adr-004, @property bounded labels | `@property @real-io` |
| 16 | 04 | Notification posted to configured webhook (real POST) | US-04 | AC-04.1/04.5 | adr-001, NFR-4 | `@real-io` |
| 17 | 04 | Webhook health probe makes **no POST** to the receiver | US-04 | AC-04.1 | adr-006, N-ODD-3 | `@probe @real-io` |
| 18 | 04 | Signed webhook payload carries signature without leaking secret | US-04 | AC-04.2 | adr-006, @property secret non-leakage | `@security @real-io` |
| 19 | 04 | Rejecting webhook receiver is isolated | US-04 | AC-04.3 | adr-003, NFR-3 | `@error @real-io` |
| 20 | 04 | Webhook missing URL fails fast at startup | US-04 | AC-04.4 | adr-002, @property config fail-fast | `@config @error @property @real-io` |
| 21 | 05 | Reset email delivered through hosted email API | US-05 | AC-05.1/05.2 | adr-001, NFR-4 | `@real-io` |
| 22 | 05 | Vendor rate-limit isolated and not retried in v1 | US-05 | AC-05.2/05.5 | adr-003/007, NFR-3/NFR-6 | `@error @real-io` |
| 23 | 05 | Hosted email API missing key fails fast without leaking it | US-05 | AC-05.3/05.4 | adr-002/006, @property config fail-fast + secret non-leakage | `@config @security @error @property @real-io` |
| 24 | 06 | Removing a member notifies that person through configured channels | US-06 | AC-06.1/06.2 | adr-005 | `@real-io` |
| 25 | 06 | Changing a password notifies the account owner | US-06 | AC-06.1/06.2 | adr-005 | `@real-io` |
| 26 | 06 | New event flows through fan-out + isolation like the existing ones | US-06 | AC-06.3 | adr-003/005, @property failure isolation | `@error @property @real-io` |
| 27 | 06 | Event label set stays bounded as the catalog grows | US-06 | AC-06.5 | adr-004/005, @property bounded labels | `@property @real-io` |

## Coverage summary

- **US coverage**: US-01 (1-4), US-02 (5-9), US-03 (10-15), US-04 (16-20), US-05 (21-23), US-06 (24-27). Every US and every AC-0N.* is pinned.
- **@property criteria (5/5)**: failure isolation (11, 26), secret non-leakage (4, 9, 18, 23), fan-out completeness (10), config fail-fast (3, 7, 20, 23), bounded metric labels (15, 27).
- **Error / edge / security / config ratio**: 15 of 27 (~56%) carry `@error`, `@security`, or `@config` — well above the 40% floor.
- **N-ODD watch-items pinned**: N-ODD-2 (await-bounded, scenario 12), N-ODD-3 (webhook probe no-POST, scenario 17).

## Adapter coverage (Mandate 6 — every driven transport has a delivery scenario)

| Provider adapter | Delivered scenario | Failed/isolated scenario | Config fail-fast |
|---|---|---|---|
| log | 1, 4 | — (log cannot fail in-process) | 3 (unknown-name path) |
| smtp | 5 | 6, 12 | 7 |
| webhook | 16, 18 | 19 | 20 |
| email_api | 21 | 22 | 23 |

## Pinned observable contract (the metric label VALUES DELIVER must honor)

- `provider` ∈ { log, smtp, webhook, email_api }
- `event` ∈ { password_reset, workspace_invite, member_invite, member_removed, password_changed }
- `outcome` ∈ { delivered, failed }
- Metric family: `foundry_notification_deliveries_total{provider,event,outcome}` on `/metrics`.

DELIVER confirms the exact Rust enum spellings; the label VALUES above are the acceptance
contract (event names align with the US text and the two new US-06 events).

## One-at-a-time @pending strategy

All 27 scenarios carry `@pending` (excluded from every lane via `acceptance.rs::filter_run`).
DELIVER removes `@pending` one slice at a time, in US-01 → US-06 order, replacing each step
stub's `panic!` scaffold with a real body wired to the harness provider seam it builds,
turning each scenario GREEN before moving on. The `@all` lane stays green throughout.
