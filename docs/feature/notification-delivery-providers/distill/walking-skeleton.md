# DISTILL — Walking Skeleton: notification-delivery-providers

> The single `@walking_skeleton` scenario (slice 01, US-01) that closes the whole
> configure → emit → deliver → observe loop at N=1 through real driving ports.

## The skeleton

`crates/foundry-acceptance/tests/features/notification-delivery-providers.feature`,
scenario 1:

```gherkin
@us-01 @walking_skeleton @real-io
Scenario: A password reset is delivered through the operator's chosen log provider
  Given the operator has activated providers "log"
  When a member requests a password reset for "maria.santos@acme.example"
  Then the notification is delivered through the "log" provider
  And the delivery is recorded for provider "log", event "password_reset", outcome "delivered"
  And the request returns its normal response
```

## Why this is the walking skeleton (litmus test)

1. **Title describes a user goal** — an operator's chosen channel carries a real
   notification — not a technical layer traversal.
2. **Given/When are user/operator actions** — the operator selects a provider by
   configuration; a member triggers the real shipped `POST /forgot-password` flow
   (`signin.rs:235`). No internal state is hand-set.
3. **Then are observable outcomes** — the notification is seen at the recording provider,
   the delivery is counted on `/metrics`, and the originating request still returns its
   normal response. No internal struct field is asserted.
4. **A non-technical stakeholder confirms "yes, that is what users need"** — Ops Olivia
   turns a silent black box (`NoopEmailSender` at `main.rs:265`) into an observable,
   selectable pipeline she controls.

## The thin vertical slice it proves

Config (composition-root registry loader) → the real password-reset app flow →
`notify()` fan-out at N=1 → the log adapter → the delivery-metric seam + `/metrics`.
This is the smallest end-to-end path that exercises the port, the registry, the
dispatcher, one adapter, and the observability seam together — every subsequent slice
(SMTP, fan-out at N>1, webhook, hosted API, new events) thickens this same spine.

## Real vs faked at the skeleton boundary

- **Real**: the axum app, Postgres (testcontainers), the `POST /forgot-password` handler,
  the registry loader, the dispatcher, the `/metrics` sidecar.
- **Faked (in-process)**: the log transport is a recording double — the acceptance
  observable is "the recorder saw one delivery" + "the counter incremented", not a real
  syslog sink. This mirrors the shipped `FakeEmailSender` pattern exactly.

If the log adapter were deleted, this skeleton would fail — it proves real wiring from the
operator's config through to the observable delivery, not just that layers link.

## DELIVER entry point

DELIVER unskips this scenario first (remove `@pending`), builds the harness provider seam
(a `spawn_with_providers`-style composition root + the recording log double), and turns it
GREEN. It is the demo proof for slice 01 and the foundation the other 26 scenarios build on.
