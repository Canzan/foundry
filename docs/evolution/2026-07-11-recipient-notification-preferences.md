# Evolution — recipient-notification-preferences (a recipient unsubscribe layer over the notification pipeline)

**Finalized**: 2026-07-11
**Commits**: DELIVER `30731d9` → `ff96751` (10 DES-monitored 5-phase TDD steps across 7 thin slices) +
CI-remediation `3b9421b`. Trunk-based; repo legacy multi-file convention; DES-monitored (5-phase contract, exempt at
finalize). Feature dir PRESERVED.
**Wave coverage**: full DES-monitored DELIVER — 10 roadmap steps, each a complete 5-phase trace
(PREPARE → RED_ACCEPTANCE → RED_UNIT → GREEN → COMMIT; RED_UNIT `SKIPPED`/NOT_APPLICABLE on the 8 steps that added no
new unit surface, `EXECUTED` on 01-01 (the suppression gate) and 07-01 (the suppression metric)).
`des-verify-integrity` exit 0; 50/50 recorded phase events `PASS`.
**Scope**: the shipped notification-delivery pipeline fanned every event out to every recipient — a recipient had no
way to say "stop". This adds a **recipient UNSUBSCRIBE layer** on top of that pipeline: a per-workspace opt-out keyed
on the recipient's email, a constant-time HMAC token that needs no login to act on, a fail-open suppression gate at
the top of the INFALLIBLE `Notifier::notify`, structurally-exempt mandatory security events, signed-in self-service
status + resubscribe, and a PII-free suppression metric. ONE migration, ZERO new crates (reused
`hmac`/`sha2`/`secrecy`), ZERO new infra. Empty table ⇒ delivery is byte-for-byte unchanged.

## Milestone — recipients can now opt a workspace out of non-essential email, and delivery still can't be stalled

The pipeline used to speak *at* recipients; it now *listens* first. A `SuppressionPolicy` driven port is consulted at
the very top of `Notifier::notify` — `StoreSuppression` in production, an inert `AllowAllSuppression` by default — and
the gate is **fail-open and time-bounded (100ms)**: a slow or erroring suppression lookup never suppresses a
legitimate notification and never fails or stalls the INFALLIBLE notifier. Mandatory security events
(`password_reset`, `password_changed`, `member_removed`) are **structurally exempt** — they cannot be suppressed even
in principle, because only `{workspace_invite, member_invite}` are `is_suppressible()`. A recipient acts on a
constant-time HMAC token (no login, no expiry) from the email itself, or manages per-workspace status while signed in.
Every suppression is counted PII-free on `/metrics`. With an empty unsubscribe table the delivery path is
byte-for-byte identical to before the feature — the whole layer is inert until a recipient opts out.

## What shipped

| Slice | Steps | Commits | Delivered |
|---|---|---|---|
| 01 — token + route + state + suppress | 01-01, 01-02, 01-03 | `30731d9`, `6b46e0e`, `1080f19` | `UnsubscribeToken` (constant-time HMAC), migration `0014` composite-key state, suppression gate at top of `notify`; per-workspace opt-out independence + idempotent confirm + unchanged no-opt-out delivery; fail-open + await-bounded (100ms) edges + workspace-deletion FK CASCADE |
| 02 — mandatory never suppressed | 02-01 | `d746c88` | `password_reset` / `password_changed` / `member_removed` structurally exempt (`is_suppressible()` allow-list) — proven for an already-unsubscribed recipient |
| 03 — non-enumerable + prefetch-safe | 03-01, 03-02 | `c1f46de`, `5029c8b` | uniform byte-identical non-enumerable refusal for tampered/unknown/invalid tokens; non-destructive `GET /unsubscribe` (prefetch-safe) + CSRF-protected `POST /unsubscribe` confirm |
| 04 — member-invite coverage | 04-01 | `08d7a6c` | member-invite emails covered by the per-workspace opt-out; unsubscribe link wired into the member-invite emit body |
| 05 — signed-in status page | 05-01 | `fa1d028` | signed-in `GET /account/notifications` — per-workspace status, least-privilege (real-Postgres store JOIN) |
| 06 — signed-in resubscribe | 06-01 | `3008c48` | `POST /account/notifications/resubscribe` restores delivery (idempotent `delete_unsubscribe`); account-less token-undo resubscribe |
| 07 — suppression observability | 07-01 | `ff96751` | `foundry_notification_suppressions_total{event}` — PII-free, register-at-0 over the full event catalog; `DeliveryOutcome` untouched |
| — CI remediation | — | `3b9421b` | 3 new full-page renderers moved onto askama `base.html` (templating north-star KPI: 0 bare-`<head>` handlers) |

### Token (`UnsubscribeToken`)
- Constant-time HMAC over `unsub|v1|{email_lower}|{workspace_id}`, keyed on `SESSION_SECRET`.
- **NO expiry** (ADR-001) — an unsubscribe link that rots is a worse UX than one that persists; revocation is the
  resubscribe action, not a TTL. Verify is constant-time; a bad token yields the uniform non-enumerable refusal.

### Suppression state + gate (`notify.rs`, migration `0014_notification_unsubscribes`)
- The feature's **ONE migration**: `notification_unsubscribes` with composite PK `(email_lower, workspace_id)` and FK
  `workspaces ON DELETE CASCADE` (deleting a workspace reaps its opt-outs).
- `Notification` gains `workspace_id: Option<Uuid>`; `NotificationEvent::is_suppressible()` = `{workspace_invite,
  member_invite}`.
- A `SuppressionPolicy` driven port (`StoreSuppression` prod, inert `AllowAllSuppression` default) is gated at the top
  of the INFALLIBLE `Notifier::notify`, **fail-open** on error/timeout (100ms bound). Mandatory events
  (`password_reset`, `password_changed`, `member_removed`) are STRUCTURALLY exempt. Empty table ⇒ delivery byte-for-byte
  unchanged.

### Surfaces
- Public, non-destructive `GET /unsubscribe` (prefetch-safe) + CSRF-protected `POST /unsubscribe`; a bad token gets a
  **uniform, non-enumerable refusal** (via the shared `invalid_page`).
- Signed-in `GET /account/notifications` (per-workspace status, least-privilege) +
  `POST /account/notifications/resubscribe`; account-less token-undo resubscribe.
- The unsubscribe link is wired into the two suppressible emit bodies (workspace-invite, member-invite).

### Observability
- Sibling metric `foundry_notification_suppressions_total{event}` on the `/metrics` sidecar — **PII-free**,
  **register-at-0** over the full event catalog. `DeliveryOutcome` was left untouched (suppression is a distinct
  signal from a delivery attempt).

## Decisions realized (ADRs)

| # | Decision | Status |
|---|---|---|
| ADR-001 | `UnsubscribeToken`: constant-time HMAC over `unsub|v1|{email_lower}|{workspace_id}`, NO expiry, `SESSION_SECRET`-keyed | IMPLEMENTED |
| ADR-002 | GET-safety / one-click: non-destructive prefetch-safe `GET`, CSRF-protected `POST` confirm (RFC 8058-aligned) | IMPLEMENTED |
| ADR-003 | Suppression hook at top of INFALLIBLE `notify`; `Notification.workspace_id`; fail-open + 100ms-bounded | IMPLEMENTED |
| ADR-004 | Unsubscribe state schema: composite PK `(email_lower, workspace_id)`, FK `workspaces ON DELETE CASCADE`, email keying | IMPLEMENTED |
| ADR-005 | Suppression observability: `foundry_notification_suppressions_total{event}`, PII-free, register-at-0 over full catalog | IMPLEMENTED |
| ADR-006 | Resubscribe UX + multi-workspace: signed-in per-workspace status/resubscribe + account-less token-undo | IMPLEMENTED |
| — | Mandatory events (`password_reset`,`password_changed`,`member_removed`) STRUCTURALLY exempt via `is_suppressible()` allow-list | IMPLEMENTED |
| — | ONE migration, ZERO new crates (`hmac`/`sha2`/`secrecy` already workspace deps), ZERO new infra | IMPLEMENTED |

## Deviations (recorded honestly)

1. **Integration-gate regression caught + fixed (`3b9421b`).** The 3 new full-page renderers in `unsubscribe.rs`
   (status, confirm, result) initially emitted **bare-`<head>` inline HTML**, violating the shipped templating
   north-star KPI (the `us-r07-completion-check` guard: 0 bare-`<head>` handlers). The **feature-scoped** acceptance
   runs (the `recipient-unsubscribe` tag) did NOT catch it — only `cargo xtask ci`'s `@all` lane did. FIX: the
   status / confirm / result pages were moved to askama templates extending `base.html`; the refusal page was already
   compliant via the shared `invalid_page`. **LESSON**: a DELIVER step that adds a new full-page HTML surface MUST run
   the `@all` / `completion-check` guard, not just the feature tag — a feature-isolated lane cannot see a
   whole-app north-star KPI. Final `cargo xtask ci` after the fix = **all gates green, 511/511 scenarios**.

2. **Adversarial review APPROVED with 0 findings.** Constant-time HMAC (no timing oracle), no cross-workspace token
   replay (workspace_id is inside the signed payload), CSRF on both POSTs, least-privilege signed-in surfaces,
   fail-open bounded gate, structurally-exempt mandatory events, PII-free register-at-0 metric, and idempotence of
   both unsubscribe and resubscribe were all verified. Nothing to remediate.

## Deferred follow-ups (out of scope, tracked)

- **Per-feature MUTATION testing deferred** (cargo-mutants + testcontainer cost). Recommend a **scoped nightly/CI
  mutation pass** on the security-sensitive surfaces rather than a whole-workspace run: the `notify.rs` suppression
  gate (fail-open edge, mandatory-exempt allow-list) and the `unsubscribe.rs` token/refusal paths (constant-time
  verify, non-enumerable refusal) are the highest-value targets.

## Verification

- **Acceptance**: 27 feature scenarios, all un-`@pending` and green. The FULL default acceptance lane
  (`cargo xtask ci`, the repo's mandated green gate) is **all gates green, 511/511 scenarios** after the `3b9421b`
  templating-KPI fix.
- **DES**: all 10 roadmap steps have complete 5-phase traces; 50/50 recorded phase events `PASS`;
  `des-verify-integrity` exit 0.
- **Cost**: ONE migration (`0014_notification_unsubscribes`), ZERO new crates, ZERO new infra.
- **Finalize**: feature dir PRESERVED (wave matrix); DES session markers removed; tree clean. Trunk-based — push
  confirmed separately with the user.
</content>
</invoke>
