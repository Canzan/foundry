# DISTILL — Walking Skeleton: recipient-notification-preferences (v1 = recipient unsubscribe)

> The one `@walking_skeleton @driving_port` scenario, and why it is the right end-to-end proof.

## The walking-skeleton scenario (US-01, scenario #1)

```gherkin
@pending @us-01 @walking_skeleton @driving_port @real-io
Scenario: One click from the invite email stops that workspace's invitations
  Given Sam has a workspace-invite email for "Northwind" carrying a signed unsubscribe link
  When Sam opens the unsubscribe link and confirms unsubscribing from "Northwind"
  Then Sam sees a confirmation that "Northwind" invitations are stopped
  And a subsequent workspace-invite for Sam from "Northwind" is not delivered
  And one suppression is counted for the "workspace_invite" event
```

## Walking Skeleton Strategy

**Tier A / Strategy A (production composition root).** The US-01 walking skeleton runs the
**real `foundry` app** — the shipped in-process axum harness + Postgres testcontainers — driving
the **real `GET`/`POST /unsubscribe`** routes and the **real `StoreSuppression` adapter wired
into the notifier**. The default `AllowAllSuppression` policy is inert (delivery byte-for-byte
unchanged); DELIVER wires `StoreSuppression` into the harness composition root for the E2E
scenarios so the suppression gate is actually exercised. The only doubles are the shipped
`notify_recorder` delivery transports, standing in for the external SMTP/webhook/email-API/log
providers (the driven-external/non-deterministic port class) so a `Then` can observe
delivered-vs-suppressed.

**Scope**: one thin E2E slice — link → confirm → suppress → observable non-delivery. This single
slice explicitly proves, end-to-end through the production composition root: (1) **token wiring**
(mint at the emit site → verify at the route), (2) the **suppression gate** inside the infallible
`Notifier::notify`, (3) **DB persistence** of the `0014_notification_unsubscribes` row, and
(4) **route dispatch** across the non-destructive `GET` confirm page + the CSRF `POST` mutate.
A Tested-But-Unwired defect is therefore structurally impossible for the core plumbing.

## Litmus (non-technical stakeholder): "yes, that is what users need"

Sam — an account-less invitee — receives an invitation email he did not ask for, clicks its
Unsubscribe link, confirms, and the next invitation from that workspace never arrives, while
he is reassured his security-critical mail is untouched. That is the whole user job
(`stop-unwanted-workspace-notifications`) end-to-end. It is demo-able to a stakeholder without
a single technical term.

## Why this closes the whole loop (carries the feature's uncertainty)

This one scenario forces DELIVER to build **every new production seam** the feature needs,
proving they compose:

1. **Mint** — `UnsubscribeToken` (`unsub|v1|{email_lower}|{workspace_id}`, `foundry_auth::sign`)
   appended to the `workspace_invite` email body at the emit site (ADR-001).
2. **Carry** — the link rides the shipped provider fan-out to the recording double.
3. **Confirm** — `GET /unsubscribe` verifies the token and renders the state-aware confirm
   page (non-destructive); the CSRF `POST /unsubscribe` writes the row (ADR-002).
4. **Record** — the `0014_notification_unsubscribes(email_lower, workspace_id, ...)` row via
   the new `Store` methods (ADR-004).
5. **Suppress** — the `SuppressionPolicy` gate inside the infallible `Notifier::notify`
   early-returns for the suppressible event on the unsubscribed pair (ADR-003).
6. **Observe** — `foundry_notification_suppressions_total{event="workspace_invite"}` +1 and the
   recording double shows the invite NOT delivered (ADR-005).

Everything downstream (mandatory exemption US-02, the security hardening US-03, `member_invite`
US-04, the signed-in surfaces US-05/06, the observability US-07) is an increment on top of this
skeleton — it does not re-lay the token/route/table/gate/counter plumbing.

## Architecture-of-reference treatment

| Port class | This feature | Test treatment |
|---|---|---|
| Driving (HTTP routes, email link, emit flow) | `/unsubscribe` GET/POST, `/account/notifications`, the emit sites | REAL via the composition root (in-process axum harness; real `foundry` subprocess for the metric scenarios) |
| Driven internal (store) | `0014` table + `Store` methods, `SuppressionPolicy`/`StoreSuppression` | REAL via testcontainers Postgres (`@real-io`) |
| Driven external / non-deterministic (delivery transports) | SMTP / webhook / email API / log providers | In-process recording doubles (`support::notify_recorder`) so `Then` can observe delivered-vs-suppressed |

This matches the shipped `notification-delivery-providers` harness exactly — no new mechanism.
