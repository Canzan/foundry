# ADR-001: Member-invite issuance surface + authorization

## Status
IMPLEMENTED (2026-06-16 — see `docs/evolution/2026-06-16-workspace-member-invites.md`). Accepted (DESIGN, Propose mode; orchestrator auto-accepts the recommended option).

## Context
There is no admin-facing invite surface today. The first-admin invite is minted by `provision_workspace`
(an instance super-admin path). FR-1/FR-2/NFR-1/BR-1 require a **workspace admin** to invite a member
into THEIR workspace via a web form, session+CSRF gated, with a non-enumerable 404 for non-admins and
signed-out callers. The shipped admin web tier (`/admin/tokens`, `/admin/instance/*`) and
`bootstrap::create_invite` establish the exact precedents.

Shipped seams grounded:
- `is_workspace_admin(workspace_id, user_id)` = `EXISTS(workspace_memberships WHERE role='admin')`
  (`store/lib.rs:1222`).
- `insert_invite(id, workspace_id, invitee_email: Option<&str>, created_by: Uuid, expires_at)`
  (`store/lib.rs:541`) — `created_by` is a required (non-NULL-bound) `Uuid`.
- `bootstrap::create_invite` (`bootstrap.rs:204`) — the issuance body to mirror: resolve session user →
  `insert_invite(.., user.workspace_id, email, user.user_id, expires_at)` → `InviteToken::new` →
  `{public_url}/invites/accept?id&sig` → best-effort `email.send`.
- The admin-gate idiom: every shipped admin route gates INSIDE the handler via a session check and
  returns `resource_not_found_page()` (200/404 uniform) on failure — NOT a router-layer guard
  (`instance_admin.rs:82-83`, `admin_tokens.rs:47`).

## Decision
Add `GET /workspace/invites` (form) and `POST /workspace/invites` (issue), both mounted on the SHARED
web layer (UNDER `session_layer` + `csrf_middleware`, alongside `/admin/tokens` and `/workspace/switch`),
gated INSIDE the handler:

1. Resolve `SessionUser{user_id, workspace_id}` from the session. Signed-out → `resource_not_found_page()`.
2. `is_workspace_admin(session.workspace_id, session.user_id)` on BOTH the GET and the POST
   (defense-in-depth on the state-changing leg). False → `resource_not_found_page()` (byte-identical to a
   never-existed path — NFR-1, no 401/403/redirect oracle).
3. POST: CSRF screened by the shipped middleware first; validate email (blank/malformed → inline
   re-render, NO invite); `insert_invite(now_v7(), session.workspace_id, email, created_by =
   session.user_id, now + 7d)`; `InviteToken::new`; emit `{public_url}/invites/accept?id&sig` + best-effort
   email; render the "invite sent" fragment.

**Reuse `insert_invite` as-is.** It already takes `created_by: Uuid` and `invitee_email: Option<&str>`;
the member case binds `created_by = the inviting admin` and `invitee_email = the typed email`. No new
`insert_member_invite` fn is needed — the existing signature is exactly right. `created_by = inviter` is
also the natural member/first-admin discriminator the accept route uses (ADR-003).

**LAYER-1e: NO new allow-list line.** The handler resolves the workspace from the SESSION
(`session.workspace_id` — the trusted resolution seam), never from request input, and `is_workspace_admin`
is not a `*_in_workspace(` call. The `check_arch` detector (Pass-1 taints `Uuid::parse*`-of-request
locals; Pass-2 flags `*_in_workspace(` calls scoped by them — `check_arch.rs:332-380`) therefore neither
inspects nor flags `member_invites.rs`. Confirm at DELIVER; one-line fallback if a future refactor trips
it.

## Alternatives Considered
- **A new `insert_member_invite` store fn** — REJECTED. `insert_invite`'s signature already matches; a
  sibling fn would duplicate the INSERT with no behavioral difference (the row shape is identical). Adds
  surface for no gain. Reuse-over-reinvent.
- **A router-layer admin gate (middleware/`route_layer`)** — REJECTED. The shipped tier gates INSIDE the
  handler and returns the uniform 404; a router-layer 401/403 would be an enumeration oracle (NFR-1) and
  diverge from the established `instance_admin`/`admin_tokens` posture.
- **Adding `member_invites` to the LAYER-1e allow-list pre-emptively** — REJECTED. The handler is
  genuinely clean (session-resolved workspace); allow-listing it would weaken the guard's precision and
  hide a real violation if a later edit introduced a parsed-id scoped call. Confirm-then-fallback is the
  honest posture (mirrors the shipped flow's D7).
- **A CLI-native `foundry workspace invite`** — DEFERRED (out of v1 scope per DISCUSS); the emitted
  artifact is a web URL, so the web route serves every emit site.

## Consequences
- Positive: zero new store fn; mirrors a shipped, validated issuance handler; non-enumerable by the
  shipped 404 idiom; CSRF by the shipped middleware; the `created_by = inviter` choice doubles as the
  accept discriminator (ADR-003) at no cost.
- Negative: a second `is_workspace_admin` call on the POST (one extra cheap `EXISTS` query) — accepted as
  defense-in-depth, consistent with the shipped tier's double-check posture.
- Probe (Earned Trust): a non-admin GET AND POST to `/workspace/invites` returns a response byte-identical
  to a generic 404 and creates no invite (revert-reds-it litmus, @property issuance non-enumerability,
  AC-03.1); a forged-CSRF POST creates no invite (AC-03.9).
