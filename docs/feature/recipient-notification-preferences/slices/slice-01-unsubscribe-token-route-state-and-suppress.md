# Slice 01 — UnsubscribeToken + public route + `0014` state table + suppression filter on `workspace_invite` (walking skeleton)

**Goal**: build the thinnest end-to-end recipient unsubscribe loop on ONE suppressible event → a recipient
gets a `workspace_invite` email, clicks its signed unsubscribe link (logged-out), confirms, and the next
`workspace_invite` to that `(email, workspace)` is suppressed.
**Story**: US-01.

**IN scope**
- An `UnsubscribeToken` binding `email_lower` + `workspace_id`, HMAC-signed with `SESSION_SECRET`, modelled on
  `InviteToken` (`crates/foundry-auth/src/lib.rs:354-390`; `sign` `:251`, constant-time `verify` `:260`).
- The `workspace_invite` email (`crates/foundry-app/src/bootstrap.rs:266`) embeds the unsubscribe link at
  `${public_url}/unsubscribe?token=…` (host = `FOUNDRY_PUBLIC_URL` → `AppState.public_url`, `main.rs:122`).
- A **public** `/unsubscribe` route (GET confirm page + POST confirm) in the public token-route cluster
  (`crates/foundry-app/src/lib.rs:371-374`, beside `/invites/accept`). GET is **non-destructive**; POST is
  **CSRF-checked** (`csrf.rs:137`, cookie via `ensure_csrf_cookie` `:54`).
- New migration `0014_notification_unsubscribes(email_lower, workspace_id, unsubscribed_at)` following the
  `reset_tokens` shape (`crates/foundry-store/migrations/0002_sessions_and_reset.sql:20-28`; latest shipped is
  `0013_issue_change_events.sql`) + `Store` methods `insert_unsubscribe` / `exists_unsubscribe` (pattern:
  `insert_reset_token`, `crates/foundry-store/src/lib.rs:980`).
- A **suppression filter**: a `workspace_invite` to an unsubscribed `(email_lower, workspace_id)` is not
  delivered (hook per DESIGN ODD-3 — inside `Notifier::notify` `notify.rs:237` with workspace context, or at
  the emit site). Default (no row) = subscribed; idempotent insert (BR-8).
- Acceptance: link carried → GET confirm renders → POST confirm writes one row → next `workspace_invite`
  suppressed → other workspace unaffected → empty table = today's behaviour.

**OUT of scope**: the mandatory-never-suppressed invariant proof (US-02); non-enumerable/prefetch hardening as
its own litmus (US-03 — this slice keeps the raw refusal + non-destructive GET but US-03 proves them);
`member_invite` (US-04); the signed-in status page + resubscribe (US-05/06); the suppression metric (US-07).

**Learning hypothesis**: disproves "a signed `UnsubscribeToken` (InviteToken model) + a public token route +
a `(email_lower, workspace_id)` state table + a single notifier suppression point can carry a recipient opt-out
end-to-end, logged-out, without changing existing delivery for subscribed recipients" if the token can't bind
email+workspace cleanly, if the notifier can't obtain `workspace_id` at the suppression point (the
`Notification` carries none today, `notify.rs:117-122`), if the `0014` keying can't match
`find_user_by_email(email_lower)` normalisation, or if the filter regresses subscribed delivery.

**Seams**: `InviteToken` (`crates/foundry-auth/src/lib.rs:354-390`) → model; `Notifier::notify`
(`crates/foundry-app/src/notify.rs:237`, loop `:244-280`, `deliver()` `:252`) → suppression hook;
`NotificationEvent::WorkspaceInvite` (`notify.rs:48`); emit site `bootstrap.rs:266`; public route cluster
(`lib.rs:371-374`); CSRF (`csrf.rs:137,54`); migration/store pattern (`0002_sessions_and_reset.sql:20-28`,
`foundry-store/src/lib.rs:980,186`); `public_url` (`main.rs:122`).
**Dependencies**: DESIGN ODD-1 (token/expiry), ODD-2 (GET-safety), ODD-3 (suppression hook), ODD-4
(table/keying). No blockers — reuses shipped seams + one new migration.
**Effort**: ~1.5 days (carries the abstraction's uncertainty; the one new migration + a new public route).
</content>
