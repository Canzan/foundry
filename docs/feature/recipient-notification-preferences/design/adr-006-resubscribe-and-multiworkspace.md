# ADR-006: Resubscribe UX + multi-workspace interaction (ODD-6, ODD-7)

## Status
Accepted — 2026-07-11 (Morgan, DESIGN wave). Feature-local.

## Context
FR-6/FR-7/NFR-6 require a signed-in account holder to review per-workspace status and resubscribe, scoped to
their own state. R7/ODD-6 flag that **account-less** recipients cannot sign in, yet should be able to undo an
unsubscribe. FR-9/ODD-7 require an email in several workspaces to be muted independently. Shipped seams:
`SessionUser` (`session.rs`), `find_user_by_email` (`store lib.rs:930`), `workspace_memberships` +
`resolve_active_workspace` JOIN (`:811`), CSRF middleware (`csrf.rs:137`), the authed `/account/password`
neighbour (`lib.rs:415-418`).

## Decision
- **Account holders (Maria)** — `GET /account/notifications` lists every workspace they belong to
  (`workspaces_for_member(user_id)`) with a Subscribed/Muted status
  (`list_unsubscribed_workspace_ids(email_lower)`); `POST /account/notifications/resubscribe` (CSRF, identity
  from `SessionUser`, never client-supplied email) clears the row for their own `(email_lower, workspace_id)`.
  Idempotent (BR-8). Least-privilege: only the member's own memberships (NFR-6/BR-6).
- **Account-less recipients (Sam)** — the public `GET /unsubscribe` confirm page (ADR-002) is **state-aware**:
  when the pair is already unsubscribed it offers a token-authorized **Resubscribe** (the same token proves
  control of the pair and authorizes both directions), reachable any time from the same email link. Undo
  without an account.
- **Multi-workspace (ODD-7)** — independence is a **corollary of the composite key** (ADR-004): the token
  binds one `(email_lower, workspace_id)`, so a Northwind link cannot verify or write for Contoso; the
  settings page renders each membership's status independently. No separate mechanism.

## Alternatives Considered
- **Signed-in-only resubscribe** (no account-less undo) — rejected (R7): account-less recipients are the
  primary unsubscribers (they have no account); a one-way opt-out with no undo for them is a UX trap. The
  token they already hold is a sufficient, least-privilege authorizer.
- **A separate public `/resubscribe` route** — rejected: a state-aware `/unsubscribe` confirm page with an
  `action` field is one surface, one token, symmetric; a second route duplicates verification + refusal.
- **Client-supplied email on the signed-in page** — rejected (NFR-6): identity must come from the session to
  prevent cross-recipient enumeration/changes; a crafted email parameter returns only the member's own scope.
- **A single global mute toggle on the settings page** — rejected (FR-9/BR-2): granularity is per-workspace;
  a global toggle is the blunt instrument DISCUSS carved out.

## Consequences
- **Positive**: symmetric unsubscribe/resubscribe for BOTH audiences (account-less via token, account holder
  via session), each least-privilege; multi-workspace independence needs no extra code (composite key);
  reuses CSRF + session + membership seams.
- **Negative / accepted**: the account-less resubscribe requires the recipient to still have (or re-open) the
  email link — acceptable, since an account holder has the richer signed-in surface, and the link is
  non-expiring (ADR-001); the signed-in page needs one new store JOIN method (`workspaces_for_member`),
  trivial and mirrored on the shipped membership query.
