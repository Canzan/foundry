# Evolution — notification-delivery-providers (a pluggable notification delivery-provider abstraction)

**Finalized**: 2026-07-11
**Commits**: DELIVER `30f5de7` → `a5e5077` (10 DES-monitored TDD steps across 6 thin slices) + review-remediation
`9159944`. Trunk-based; repo legacy multi-file convention; DES-monitored (5-phase contract, exempt at finalize).
Feature dir PRESERVED.
**Wave coverage**: full DES-monitored DELIVER — 10 roadmap steps, each a complete 5-phase trace
(PREPARE → RED_ACCEPTANCE → RED_UNIT → GREEN → COMMIT; RED_UNIT `SKIPPED`/NOT_APPLICABLE where a step added no
new unit surface). `des-verify-integrity` exit 0; 48/48 recorded phase events `PASS`.
**Scope**: notifications were hard-wired to a single `EmailSender`/`NoopEmailSender` port (`email.rs`) — one
destination, no fan-out, no operator choice, no delivery observability. This replaces that with a **pluggable
delivery-provider abstraction**: an operator selects any subset of {Log, SMTP, Webhook, Hosted-email-API} at the
composition root, notifications fan out **concurrently** to all active providers, each provider is failure-isolated
and time-bounded, and every attempt is counted on `/metrics`. Secrets flow through `secrecy::SecretString` and never
reach logs, errors, metric labels, or `Debug`. ZERO new crates, ZERO migrations, ZERO new infra.

## Milestone — notification delivery is now an operator-configurable, observable seam

The old port could send to exactly one place and told you nothing about whether it worked. The board now speaks
notifications through a **port + concurrent dispatcher** (`Notifier::notify`, INFALLIBLE): the operator picks
providers via `NOTIFICATION_PROVIDERS`, delivery fans out over a `tokio::JoinSet` with a per-provider timeout, one
slow or failing provider cannot stall or fail the others (or the caller), and
`foundry_notification_deliveries_total{provider,event,outcome}` makes every attempt a first-class signal. The old
`EmailSender`/`NoopEmailSender` port and `email.rs` were DELETED outright — one path, no dual-write, no legacy
shim (pre-stable dead-code policy).

## What shipped

| Slice | Steps | Commits | Delivered |
|---|---|---|---|
| 01 — port + dispatcher + Log provider | 01-01, 01-02 | `30f5de7`, `606cd76` | `NotificationProvider` trait, `Notifier` fan-out, Log provider, no-op default, fail-fast on unknown provider |
| 02 — SMTP provider | 02-01, 02-02 | `0c25ee0`, `a96f196` | lettre `AsyncSmtpTransport`; failure isolation; fail-fast on missing `SMTP_*`; `SMTP_PASSWORD` non-leakage |
| 03 — delivery metric | 03-01, 03-02 | `ac560f9`, `0a362dd` | concurrent fan-out + per-provider metric; register-at-0 over the active cross-product; bounded-label guard |
| 04 — Webhook provider | 04-01, 04-02 | `90f75ab`, `b3f4157` | reqwest POST + HMAC-SHA256 signature; host-reachability-only probe; fail-fast on missing `WEBHOOK_URL` |
| 05 — Hosted email API provider | 05-01 | `b898895` | reqwest + credential-header POST; no-retry failure isolation |
| 06 — 2 new events + emit sites | 06-01 | `a5e5077` | `member_removed` + `password_changed` events wired to real port-to-port trigger endpoints |
| — review remediation | — | `9159944` | reauth + password policy on password change; guard `member_removed` on 0-row removal; 500→404 on data-consistency |

### Port + dispatcher (`crates/foundry-app/src/notify.rs`)
- `NotificationProvider` trait with **NO `Debug` supertrait** (ADR-006 — a `Debug` bound would risk leaking secret
  provider state into formatted output).
- `Notification{event, recipient, subject, body}`; **closed** `NotificationEvent` enum; `ProviderKind`;
  `DeliveryError{Transient, Permanent}`; `DeliveryOutcome`.
- `Notifier::notify` is **INFALLIBLE** — concurrent `tokio::JoinSet` fan-out, per-provider timeout
  (`NOTIFICATION_DELIVERY_TIMEOUT_MS`, default 5000), await-bounded. One provider failing/timing out never fails the
  caller or its siblings.

### 4 providers
- **Log**, **SMTP** (lettre `AsyncSmtpTransport`), **Webhook** (reqwest POST + HMAC-SHA256 signature; probe is
  host-reachability-only, never a live POST), **Hosted email API** (reqwest + credential header).
- Secrets via `secrecy::SecretString` — never in logs, errors, metric labels, or `Debug`.

### Config — composition root (`main.rs`)
- `NOTIFICATION_PROVIDERS` comma-list + per-provider `SMTP_*` / `WEBHOOK_*` / `EMAIL_API_*` read via
  `std::env::var` at the composition root. **Fail-fast** (non-zero exit, secret-free error) on unknown or
  misconfigured providers; unset ⇒ Noop-equivalent. Admission is **wire → probe → use**.

### Observability
- `foundry_notification_deliveries_total{provider,event,outcome}` on the `/metrics` sidecar; **register-at-0** over
  the active provider×event cross-product; **bounded-label cardinality guard** (fail-closed).

### Events
- Routes the 3 existing notifications (password_reset, workspace_invite, member_invite) + 2 NEW (`member_removed`,
  `password_changed`).

## Decisions realized (ADRs)
| # | Decision | Status |
|---|---|---|
| ADR-006 | `NotificationProvider` trait has NO `Debug` supertrait (prevent secret leakage via formatting) | IMPLEMENTED |
| — | `Notifier::notify` INFALLIBLE; concurrent `JoinSet` fan-out; per-provider timeout; failure-isolated | IMPLEMENTED |
| — | Closed `NotificationEvent` enum; secrets via `secrecy::SecretString`, never logged/labelled/`Debug`'d | IMPLEMENTED |
| — | Config at composition root; fail-fast on unknown/misconfigured; unset ⇒ Noop; wire→probe→use admission | IMPLEMENTED |
| — | Metric register-at-0 over active cross-product; bounded-label guard fail-closed | IMPLEMENTED |
| — | Old `EmailSender`/`NoopEmailSender` + `email.rs` DELETED — no dual path (pre-stable dead-code policy) | IMPLEMENTED |
| — | ZERO new crates / migrations / infra (`reqwest`/`lettre`/`hmac`/`sha2`/`secrecy` already workspace deps) | IMPLEMENTED |

## Deviations (recorded honestly — DISCUSS/DESIGN gaps DELIVER had to absorb)

1. **US-06 scope addition — events assumed triggers that did not exist.** `member_removed` and `password_changed`
   had NO existing trigger handlers anywhere in the app; the DISCUSS/DESIGN waves specified the events without
   confirming a fire site. DELIVER had to add **two minimal real port-to-port endpoints** beyond the original
   delivery-abstraction scope — `POST /workspace/members/remove` (admin-gated) and `POST /account/password`
   (signed-in) — plus store methods `remove_workspace_member`, `update_user_password`, and
   `find_user_password_hash_by_id`. This should have been surfaced upstream: an event without a trigger is an
   incomplete requirement.

2. **Adversarial review REJECTED the initial two endpoints (remediated in `9159944`).** Three blockers + one
   defect:
   - **False `member_removed`** — a removal that matched 0 rows still emitted the notification. Now guarded by
     rows-affected → a non-enumerable 404 (no false notification, no membership enumeration).
   - **Password change without reauth** — the endpoint changed the password with no current-password check. Now
     verifies via `find_user_password_hash_by_id` + `foundry_auth::verify_password` before mutating.
   - **No password policy** on the new change path. Now reuses `check_password_policy` (min-12), matching the rest
     of the app.
   - **D8** — a data-consistency error returned 500 where 404 was correct; fixed.
   - All four closed with **3 regression scenarios** added to the feature-isolated lane.

## Deferred follow-ups (out of scope, tracked)

- **(a) Per-feature MUTATION testing deferred.** cargo-mutants + testcontainers OOM makes a local full run
  impractical on the shared Docker host. `notify.rs` is security-sensitive (secret handling, fan-out isolation) —
  recommend a **scoped nightly/CI mutation pass on `notify.rs`** rather than a whole-workspace run.
- **(b) Real-I/O coverage for SMTP + Hosted-email-API (review findings D5/D6).** The Webhook adapter's "delivered"
  scenarios exercise a REAL local receiver over reqwest; the SMTP and Hosted-email-API "delivered" scenarios go
  through **recording doubles**. Add ≥1 real-I/O scenario per SMTP / EmailApi adapter as a follow-up.
- **(c) Leaked testcontainers on the shared Docker host.** ~39 orphaned `postgres:16-alpine` containers from prior
  sessions drive the full-lane OOM (57P03 recovery-mode cascade); the FULL default acceptance lane could not
  complete in one shot locally. Clean the leaked containers, then run `cargo xtask ci` (the repo's mandated green
  gate) before push.

## Verification
- **Acceptance**: 27 scenarios (all un-`@pending` + green) + 3 review-remediation regression scenarios =
  **30/30 green**, feature-isolated. The FULL default acceptance lane (`cargo xtask ci`) could not complete
  locally in one shot due to deferred item (c) — run it after cleaning leaked containers, before push.
- **DES**: all 10 roadmap steps have complete 5-phase traces; 48/48 recorded phase events `PASS`;
  `des-verify-integrity` exit 0.
- **Cost**: ZERO new crates, ZERO migrations, ZERO new infra.
- **Finalize**: feature dir PRESERVED (wave matrix); DES session markers removed; tree clean. Trunk-based — push
  confirmed separately with the user.
</content>
</invoke>
