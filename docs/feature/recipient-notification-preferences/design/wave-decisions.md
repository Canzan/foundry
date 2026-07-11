# DESIGN Decisions — recipient-notification-preferences (v1 = recipient unsubscribe)

> Morgan (nw-solution-architect), DESIGN wave, Propose mode. Resolves the seven DISCUSS ODDs into contracts +
> six feature-local ADRs. Paradigm inherited (modular monolith, ports-and-adapters via Rust traits, env
> config at the composition root) — NOT re-decided. Deliverables: `architecture.md`, `adr-001..006`, this
> file, `upstream-changes.md`.

## Key Decisions

- **[DD1] Reuse the HMAC primitives, not the `InviteToken` struct.** `UnsubscribeToken` is a self-contained
  HMAC over `"unsub|v1|{email_lower}|{workspace_id}"` via `foundry_auth::sign`/`verify` — domain-separated,
  versioned, keyed on `SESSION_SECRET`, **no expiry**. (ADR-001)
- **[DD2] In-body link → GET confirm → CSRF POST; no RFC 8058 one-click in v1.** GET is non-destructive +
  mints the CSRF cookie; the CSRF POST mutates. A `List-Unsubscribe` header + one-click POST are deferred
  (the one-click POST is CSRF-exempt by construction, in tension with NFR-5). (ADR-002)
- **[DD3] Suppress inside `Notifier::notify` behind a `SuppressionPolicy` port; `Notification` gains
  `workspace_id: Option<Uuid>`.** ONE guard, suppressible-events-only, bounded lookup, **fail-open** on error;
  early-return + count on suppress. Single enforcement point; dependency inversion; infallible + await-bounded
  preserved; NFR-3 structural (mandatory events never reach the lookup). (ADR-003 — the crux)
- **[DD4] `0014_notification_unsubscribes` with a composite PK `(email_lower, workspace_id)`.** Email-keyed
  (account-less invitees), FK to `workspaces ON DELETE CASCADE`, no FK on email; absence-of-row = subscribed;
  account reconciliation is automatic. (ADR-004)
- **[DD5] A sibling `foundry_notification_suppressions_total{event}` counter, NOT a widened
  `DeliveryOutcome`.** `event`-only bounded label (no provider, no workspace, no PII); register-at-0 over the
  full catalog so mandatory series show a permanent 0. The shipped delivery counter is untouched. (ADR-005)
- **[DD6] Symmetric resubscribe: signed-in page for account holders, token-authorized undo on the state-aware
  confirm page for account-less recipients.** Multi-workspace independence is a corollary of the composite
  key. (ADR-006)
- **[DD7] Additive + inert-by-default.** `AllowAllSuppression` is the composition-root default; with an empty
  table the fan-out is byte-for-byte unchanged (NFR-7). ZERO new crates, ONE migration.

## ODD → ADR resolution map

| ODD | Question | Resolution | ADR |
|-----|----------|------------|-----|
| ODD-1 | Token shape + expiry + secret | Self-contained HMAC `unsub\|v1\|email\|ws`, no expiry, `SESSION_SECRET`, reuse `sign`/`verify` | ADR-001 |
| ODD-2 | GET-safety / one-click / RFC 8058 | GET→confirm→CSRF-POST; no one-click POST in v1; `List-Unsubscribe` header deferred | ADR-002 |
| ODD-3 | Suppression hook + fail stance | Inside `notify()` via `SuppressionPolicy` port; `Notification.workspace_id`; suppressible-only; **fail-open** | ADR-003 |
| ODD-4 | State table + email keying | `0014` composite PK `(email_lower, workspace_id)`, workspace FK cascade, no email FK | ADR-004 |
| ODD-5 | Suppression metric | Sibling `foundry_notification_suppressions_total{event}`; not a widened `DeliveryOutcome` | ADR-005 |
| ODD-6 | Resubscribe for account-less | Token-authorized undo on the state-aware confirm page; signed-in page for holders | ADR-006 |
| ODD-7 | Multi-workspace independence | Corollary of the composite key + per-pair token; independent settings-page rows | ADR-004 + ADR-006 |

## Constraints for DISTILL / DELIVER (what acceptance must pin)

- **Mandatory-never-suppressed invariant (NFR-3)** — a never-suppress `@property`: any of
  `{password_reset, password_changed, member_removed}` under any unsubscribe config is **delivered**, never
  counted `suppressed`; plus an allow-list unit test that `is_suppressible()` is exactly
  `{workspace_invite, member_invite}`. This is structural (the event-class gate), not incidental.
- **Non-enumerable + prefetch-safe token (NFR-1, NFR-2)** — a tampered / swapped-workspace / unknown token
  yields the byte-identical uniform refusal and writes nothing; a bare **GET** of a valid link writes no row;
  only the CSRF **POST** mutates (revert-reds-it litmus on both).
- **CSRF on both POSTs (NFR-5)** — the public confirm and the signed-in resubscribe are both rejected `403`
  without a valid `_csrf`, changing no state.
- **Per-workspace independence (FR-9)** — unsubscribing `(email, A)` leaves `(email, B)` delivering.
- **Workspace-deletion cascade (ADR-004)** — deleting a workspace clears its opt-out rows (FK `ON DELETE
  CASCADE`) without error and leaves no orphan suppression; pin as an acceptance edge.
- **PII-free bounded metric (NFR-4)** — `foundry_notification_suppressions_total` carries only `event`; a
  full `/metrics` scrape + the logs contain no recipient email or token; the label-key set fails closed if
  widened; mandatory-event suppressed series are always 0.
- **Infallible + await-bounded `notify()` (R5)** — a failing/slow `SuppressionPolicy` still delivers (fail-open)
  and `notify()` returns; the lookup is timeout-bounded.
- **Backwards-compat (NFR-7)** — with an empty table / inert default, every shipped delivery scenario passes
  unchanged.
- **Idempotence (BR-8)** — double unsubscribe / double resubscribe are no-op successes; no duplicate row.
- **A11y (NFR-8)** — the confirm page + the settings page pass the automated a11y check (labelled controls,
  keyboard-operable, status as text).

## Per-slice architecture notes

- **US-01 (walking skeleton)** — introduces ALL new surface: `UnsubscribeToken` (ADR-001), the `0014` table +
  `is_unsubscribed`/`insert_unsubscribe` (ADR-004), the public `GET`/`POST /unsubscribe` (ADR-002), the
  `SuppressionPolicy` port + `StoreSuppression` + `AllowAllSuppression` + the `notify()` gate (ADR-003),
  proven on `workspace_invite` (emit at `bootstrap.rs:266`, `workspace_id = user.workspace_id`).
- **US-02 (mandatory exemption)** — `NotificationEvent::is_suppressible()` allow-list + the never-suppress
  `@property`; mandatory emit sites set `workspace_id: None`. No new persistence. (ADR-003)
- **US-03 (non-enumerable + prefetch-safe)** — the uniform refusal on bad token + GET-non-destructive
  litmus; constant-time verify. (ADR-001/002)
- **US-04 (member_invite)** — attach the link at `member_invites.rs:204` + include `member_invite` in the
  allow-list; one opt-out row covers both suppressible events. Completes the v1 boundary. (ADR-003/004)
- **US-05 (signed-in status)** — `GET /account/notifications` + `workspaces_for_member` +
  `list_unsubscribed_workspace_ids`; least-privilege via `SessionUser`; a11y. (ADR-006)
- **US-06 (resubscribe)** — `POST /account/notifications/resubscribe` (CSRF, session) + `delete_unsubscribe`;
  the account-less undo path on the confirm page. (ADR-006)
- **US-07 (observability)** — `foundry_notification_suppressions_total{event}` + register-at-0 + the
  fail-closed cardinality + no-PII litmus. (ADR-005)

## Handoff to DISTILL (acceptance-designer)

- **Architecture + contracts**: `architecture.md` (C4 L1+L2+L3, the resolved token/route/table/hook/metric
  contracts, reuse-vs-new, FR/NFR traceability, enforcement rules).
- **Six ADRs** with alternatives + why-rejected, grounded in verified `file:line` seams.
- **External integrations**: **none new.** The unsubscribe link rides the shipped provider transports whose
  consumer-driven-contract-test recommendation already lives in the predecessor's handoff — no additional
  contract-test annotation is owed to platform-architect.
- **Paradigm for software-crafter**: Rust, OOP-adjacent trait-based ports-and-adapters (the shipped style);
  the domain (`Notification`, `NotificationEvent`, `Notifier`, `SuppressionPolicy`) stays transport- and
  store-free; adapters (`StoreSuppression`, the axum handlers) hold the I/O.
- **The two provable invariants** the acceptance suite must guard as revert-reds-it litmuses: (1) mandatory >
  unsubscribe (never-suppress `@property`); (2) infallible + await-bounded `notify()` under a failing/slow
  `SuppressionPolicy` (fail-open).

## Peer Review
- **Status**: COMPLETE (iteration 1 of max 2) — `nw-solution-architect-reviewer`, 2026-07-11.
- **Verdict**: **approved** — `critical_issues_count: 0`, `high_issues_count: 0`. All ADRs pass (context +
  ≥2 real alternatives + consequences); C4 L1/L2/L3 present + valid; never-suppress invariant confirmed
  structural; infallible + await-bounded `notify()` confirmed preserved; fail-open justified; no
  over-engineering (SuppressionPolicy port + sibling metric both justified). Two non-blocking notes folded
  in: a design-recommended ~50–100 ms lookup bound; the workspace-deletion cascade pinned for DISTILL.
- **Handoff to DISTILL (acceptance-designer): CLEARED.**
