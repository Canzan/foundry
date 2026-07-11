# DISTILL — Test Scenarios: recipient-notification-preferences (v1 = recipient unsubscribe)

> Quinn (nw-acceptance-designer), DISTILL wave. Executable SSOT = the cucumber-rs feature
> file `crates/foundry-acceptance/tests/features/recipient-notification-preferences.feature`
> (27 scenarios) + step module
> `crates/foundry-acceptance/src/steps/feature_recipient_notification_preferences.rs`
> (compiling `@pending` panic scaffolds). This document is the scenario -> US/AC/ADR map and
> the harness-boundary rationale. Reconciliation across DISCUSS/DESIGN (DEVOPS not run,
> consistent with prior trunk features) passed — 0 contradictions.

## House cucumber-rs conventions (mirrors the shipped `notification-delivery-providers`)

- Feature tag `@recipient-unsubscribe @driving_port`; per-story `@us-01`..`@us-07`; `@pending`
  on **every** scenario, **per-scenario** (never feature-level), so DELIVER unskips one slice
  at a time. `@pending` is excluded from EVERY lane by `acceptance.rs` (`!has("pending")`), so
  this file keeps the `@all` lane green until DELIVER acts.
- Run one slice with `FOUNDRY_ACCEPTANCE_TAGS=recipient-unsubscribe` — the feature-specific
  tag, chosen deliberately to avoid the `@us-0N` cross-feature tag collisions (the predecessor
  and other features also carry `@us-01`..).
- Tags: `@real-io` (real app + Postgres via the in-process harness / a real `foundry`
  subprocess for the metric scenarios); `@property` (house convention = an example-based
  INVARIANT litmus, per Mandate 11 layer-3 sad paths stay example-based — NOT PBT-generated);
  `@security`, `@error`, `@edge`, `@nfr` as facet markers; `@walking_skeleton` on the one WS.

## Driving ports (Mandate 1 — every scenario enters through one)

1. The signed unsubscribe **link** in the suppressible email body (minted at the emit site).
2. Public **`GET /unsubscribe`** (non-destructive confirm page) + CSRF **`POST /unsubscribe`**.
3. A real shipped **emit flow** — bootstrap/member invites, forgot-password, remove-member,
   password-change — each emitting ONE notification through `notify()`.
4. Signed-in **`GET /account/notifications`** + CSRF **`POST /account/notifications/resubscribe`**.
5. The **recording provider double** (delivered-vs-suppressed) + the **`/metrics`** sidecar.

## Scenario -> US / AC / ADR map (27 scenarios)

| # | Scenario | Tags | US | AC / contract | ADR |
|---|----------|------|----|---------------|-----|
| 1 | One click from the invite email stops that workspace's invitations | walking_skeleton, driving_port | US-01 | link recorded + next `workspace_invite` suppressed + suppression counted | ADR-001/002/003/004/005 |
| 2 | Unsubscribing from one workspace leaves another untouched | — | US-01 | per-workspace independence (FR-9) | ADR-004 |
| 3 | Confirming an unsubscribe twice is a harmless no-op | error | US-01 | idempotent, no dup row (BR-8) | ADR-004 |
| 4 | With no opt-out on record, a workspace-invite is delivered unchanged | — | US-01 | backwards-compat (NFR-7) | ADR-003 |
| 5 | A password reset reaches an unsubscribed recipient | property | US-02 | mandatory never suppressed (FR-4/BR-3/NFR-3) | ADR-003 |
| 6 | A removal notice reaches an unsubscribed recipient | — | US-02 | `member_removed` mandatory | ADR-003 |
| 7 | No mandatory event is ever suppressed for an unsubscribed recipient | property | US-02 | never-suppress `@property` litmus | ADR-003 |
| 8 | A tampered token is refused exactly like an invalid one | security | US-03 | uniform refusal, records nothing (NFR-1/BR-4) | ADR-001/002 |
| 9 | The response does not reveal whether an address exists | security | US-03 | byte-identical refusal, no existence leak | ADR-002 |
| 10 | Prefetching the link does not unsubscribe anyone | security | US-03 | GET non-destructive (NFR-2) | ADR-002 |
| 11 | An unsubscribe confirm without a valid CSRF token is refused | security, error | US-03 | CSRF on the public POST (NFR-5) | ADR-002 |
| 12 | A refused unsubscribe request leaks no token or recipient email | security | US-03 | no PII in logs on refusal | ADR-001/005 |
| 13 | One opt-out covers member-invite emails for a workspace | — | US-04 | `member_invite` suppressed by same row | ADR-003/004 |
| 14 | The member-invite email carries its own unsubscribe link | — | US-04 | one confirm mutes both invite events | ADR-002/003 |
| 15 | The settings page shows per-workspace subscription status | — | US-05 | `GET /account/notifications` status (FR-6) | ADR-006 |
| 16 | A request cannot be steered to another recipient's status | security | US-05 | session identity, least-privilege (NFR-6/BR-6) | ADR-006 |
| 17 | Resubscribing a muted workspace restores its notifications | — | US-06 | signed-in resubscribe clears row (FR-7) | ADR-006 |
| 18 | A resubscribe without a valid CSRF token is rejected | security, error | US-06 | CSRF on resubscribe POST (NFR-5) | ADR-006 |
| 19 | An account-less recipient resubscribes from the confirm page | — | US-06 | token-undo on the state-aware page (ODD-6) | ADR-006 |
| 20 | Suppressed deliveries are visible as a count on the metrics endpoint | — | US-07 | `foundry_notification_suppressions_total{event}` (FR-8) | ADR-005 |
| 21 | The suppression metric exposes no recipient PII | property, security | US-07 | PII-free bounded label (NFR-4) | ADR-005 |
| 22 | Mandatory events never appear as suppressed | property | US-07 | register-at-0, mandatory series permanently 0 | ADR-005 |
| 23 | A failing suppression lookup still delivers (fail-open) | nfr, property, error | R5 | infallible `notify()` fail-open on error | ADR-003 |
| 24 | A slow suppression lookup does not stall the emit (await-bounded) | nfr, property, error | R5 | bounded lookup, `notify()` returns | ADR-003 |
| 25 | Deleting a workspace clears its opt-out rows | edge | ADR-004 | FK `ON DELETE CASCADE`, no orphan | ADR-004 |
| 26 | Unsubscribing via a member-invite still leaves security mail intact | property | US-04 | AC-04.4 — mandatory delivered after a `member_invite`-path unsubscribe, not counted as suppressed (FR-4, NFR-3) | ADR-003 |
| 27 | Resubscribing an already-subscribed workspace is harmless | error | US-06 | AC-06.3 — idempotent resubscribe no-op success: same confirmation both times, subsequent event still delivers (BR-8) | ADR-006 |

> **Reworded assertions (testing-theater fix — observable, not DB-row):** three `Then` steps
> were changed from internal state-query assertions to observable outcomes:
> (a) #3 `no duplicate opt-out row is created and no error occurs` → `Sam sees the same
> confirmation both times with no error` (observable idempotence);
> (b) #10 `no opt-out is recorded for Sam in "Northwind"` → `a subsequent workspace-invite to
> Sam in "Northwind" is still delivered` (proves the bare GET did NOT unsubscribe him);
> (c) #25 `the workspace's opt-out rows are cleared without error` → `deleting the workspace
> succeeds` + `a previously-unsubscribed recipient of that workspace resumes delivery`
> (observable resumption instead of a cascade-row assertion).

## Coverage across US-01..07 + NFR/edge

- US-01: 4 (1 walking skeleton) · US-02: 3 · US-03: 5 · US-04: 3 · US-05: 2 · US-06: 4 ·
  US-07: 3 · NFR fail-open (R5): 2 · edge (workspace cascade): 1. **Total 27.**
- Error/edge/security scenarios: #3, #8, #9, #10, #11, #12, #16, #18, #23, #24, #25, #27 =
  **12 / 27 ≈ 44%** (>= 40% target).

## Adapter / driving-adapter coverage

| Driven / driving surface (NEW) | Exercised REAL by |
|---|---|
| `GET /unsubscribe` (confirm page, non-destructive) | #1, #10 (prefetch), #19 |
| `POST /unsubscribe` (CSRF mutate: unsubscribe + resubscribe) | #1, #3, #11 (CSRF), #14, #19 |
| Uniform non-enumerable refusal (bad token) | #8, #9, #12 |
| `SuppressionPolicy` gate inside `notify()` (StoreSuppression) | #1, #4, #5, #6, #7, #13, #23, #24, #26 |
| `0014_notification_unsubscribes` + `Store` methods | #1, #2, #3, #25 |
| `GET /account/notifications` (status) | #15, #16 |
| `POST /account/notifications/resubscribe` (CSRF) | #17, #18, #27 |
| `foundry_notification_suppressions_total{event}` + `/metrics` sidecar | #20, #21, #22 (subprocess) |

## Harness boundary (why this split)

Real app + Postgres (in-process axum harness + testcontainers, `@real-io`), mirroring the
predecessor. The NEW seams — token, `0014` table, `SuppressionPolicy`/`StoreSuppression`, the
two route surfaces, the suppression counter — are exercised REAL through the composition root.
The DELIVERY TRANSPORTS remain in-process recording doubles (the shipped
`support::notify_recorder` providers) so a `Then` can distinguish delivered-vs-suppressed
without a live SMTP/webhook call. The register-at-0 + bounded-label metric scenarios (#20-22)
drive a REAL `foundry` subprocess and scrape its `/metrics` sidecar, because the in-process
harness installs no Prometheus recorder — the exact split the predecessor established.

## Pre-DELIVER RED classification

Every scenario is `@pending` and its steps `panic!` (assertion-class = RED, not BROKEN). The
step module compiles (`cargo test -p foundry-acceptance --no-run` green) because it imports
only `FoundryWorld` + `cucumber::{given,when,then}` and references no not-yet-existing seam.
DELIVER's RED phase per slice: remove `@pending` on that slice's scenarios, watch them fail
for the right reason (missing production behaviour), implement to GREEN.
