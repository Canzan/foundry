# ADR-004: Unsubscribe state schema + email keying (ODD-4, ODD-7 key)

## Status
Accepted — 2026-07-11 (Morgan, DESIGN wave). Feature-local.

## Context
FR-9/FR-10/BR-2/BR-7 require per-`(email_lower, workspace_id)` opt-out state, default = subscribed (no row),
keyed on **email** (not user id) because many recipients are account-less invitees (`Notification.recipient`
is a bare email). This is the one migration the feature adds (`0014`, latest shipped is `0013`). The shipped
`reset_tokens` table (`0002_sessions_and_reset.sql:20-28`) and the `insert_reset_token` method-on-`Store`
(`store lib.rs:980`) are the cited precedents; `find_user_by_email(email_lower)` (`:930`) is the
normalization to match; `0013` established the `workspace_id … ON DELETE CASCADE` FK idiom. R8 flags
account-vs-email reconciliation.

## Decision
```sql
-- crates/foundry-store/migrations/0014_notification_unsubscribes.sql
CREATE TABLE notification_unsubscribes (
    email_lower     TEXT        NOT NULL,
    workspace_id    UUID        NOT NULL REFERENCES workspaces(id) ON DELETE CASCADE,
    unsubscribed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (email_lower, workspace_id)
);
```
- **Composite `PRIMARY KEY (email_lower, workspace_id)`** — the natural identity; enforces uniqueness (so
  unsubscribe is idempotent via `ON CONFLICT DO NOTHING`) AND is the covering index for the suppression
  point-read. No surrogate `id`.
- **`email_lower`** normalized with `to_ascii_lowercase()` at every read/write, matching `users.email_lower`;
  the token binds `email_lower` too, so write and suppression-read normalization are identical.
- **FK `workspace_id → workspaces(id) ON DELETE CASCADE`** (mirrors `0013`); **no FK on email** — the
  deliberate account-less keying.
- **Store methods**: `is_unsubscribed(email_lower, ws) -> bool`, `insert_unsubscribe(email_lower, ws)` (ON
  CONFLICT DO NOTHING), `delete_unsubscribe(email_lower, ws)`, `list_unsubscribed_workspace_ids(email_lower)
  -> HashSet<Uuid>`, `workspaces_for_member(user_id) -> Vec<(Uuid, String)>` (JOIN mirroring
  `resolve_active_workspace`, `:811`).

## Alternatives Considered
- **Surrogate `id UUID PRIMARY KEY` + a UNIQUE(email_lower, workspace_id)** (verbatim `reset_tokens` shape) —
  rejected: `reset_tokens` needs a token `id` because the token is single-use and looked up by id; here the
  pair IS the identity and every access is by the pair, so the surrogate is dead weight and a second index.
- **User-id-keyed state** (`user_id, workspace_id`) — rejected (BR-2): account-less invitees have no `users`
  row; email is what the notifier targets.
- **A soft `resubscribed_at` / status column instead of DELETE** — rejected: absence-of-row = subscribed is
  simpler, matches BR-7's opt-out model, and keeps the suppression read a pure existence check; resubscribe
  is a DELETE (idempotent).
- **FK on email to `users`** — rejected: would forbid account-less rows (BR-2).

## Consequences
- **Positive**: minimal shape (three columns, one index-that-is-the-PK); idempotent by construction;
  cascade-tied to workspace lifetime; **account reconciliation is automatic** — an invitee who later signs up
  with the same email inherits their opt-outs (the signed-in page reads by `email_lower`), no backfill (R8
  closed); **per-workspace independence (ODD-7) is a corollary of the composite key** — a row for one
  workspace cannot affect another (FR-9).
- **Negative / accepted**: an email that changes case upstream must always be lowercased before read/write
  (enforced by routing every access through the `email_lower` store methods); a workspace deletion silently
  drops its opt-out rows (intended — no orphan suppression).
