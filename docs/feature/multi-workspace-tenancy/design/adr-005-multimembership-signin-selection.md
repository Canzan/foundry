# ADR-005 — Multi-membership sign-in + active-workspace selection

## Status
IMPLEMENTED (milestone, 2026-06-11; slice 2). OD-2 ratified: multi-membership; the session carries the active workspace; `POST /workspace/switch` re-stamps it (membership-guarded, fail-closed).

## Context
A user/email MAY belong to several workspaces — the schema models this (`users` is global,
membership is the M:N `workspace_memberships`; no `users.workspace_id`). Sign-in must establish
EXACTLY ONE acting workspace per session, fail-closed (NFR-MWT-SEC-03). Today sign-in writes
`SessionUser{ user_id, workspace_id }` with `workspace_id = first_workspace()` — it never has to
choose because only one workspace exists. The session is intentionally thin: only `user_id` is
durable; memberships are looked up per request "so memberships can rotate without invalidating
sessions" (`session.rs` doc comment). This ADR designs the selection over that shipped seam.

## Options considered
- **(a) Always prompt for a workspace at sign-in.** Uniform, but adds a step for the common
  single-membership user — annoying and unnecessary.
- **(b) Single-membership auto-resolves; multi-membership prompts; a switcher re-stamps the active
  workspace; session carries the active workspace; no-membership ⇒ refuse.**
- **(c) Default to a stored "last/primary workspace" and never prompt.** Hidden state; a contractor
  could act on the wrong tenant without realizing — a cross-tenant hazard by ambiguity.

## Decision
**(b).** Resolution model:
- At sign-in, after the (unchanged) argon2id verification, look up the user's memberships
  (`memberships_for_user(user_id)` — a NEW thin store query; EXTEND the seam, replacing the
  `first_workspace()` call-site at `signin.rs:140`):
  - **0 memberships** → the session resolves to NO workspace; the request is **refused/empty**,
    never defaulted (US-MWT04 scenario 3).
  - **exactly 1 membership** → set it as the session's active workspace automatically; no prompt
    (the single-workspace install + most users — US-MWT04 scenario 1).
  - **≥2 memberships** → present an explicit selection; the chosen workspace is stamped into the
    session as the active workspace (US-MWT04 scenario 2).
- A **switcher** (a small web control) lets a multi-membership user change the active workspace; it
  re-stamps `SessionUser.workspace_id` after verifying membership. Switching does not re-authenticate.
- **Per request**, `SessionUser` resolution verifies the user is STILL a member of the session's
  active workspace; if membership was revoked, it fails closed (ties to ADR-001/SEC-03).
- The session stays thin: it carries `user_id` + the active `workspace_id`; membership/role is
  re-checked per request, so a revoked membership takes effect immediately without invalidating the
  session machinery.

The sign-in security contract (argon2id, brute-force delay, non-enumerable error, 30-day cookie,
double-submit CSRF) is UNCHANGED — selection is added AFTER verification (NFR-MWT-DATA-02).

## Consequences
- **Positive**: the common case (one membership, incl. every upgraded single-workspace install) is
  zero-friction and identical to today; multi-membership is always an EXPLICIT, intentional choice
  (no silent default); per-request membership re-check means revocation is immediate; smallest
  change to the shipped seam.
- **Negative**: a switcher UI is new surface (Askama template + a POST that re-stamps the session);
  multi-tab "two active workspaces" is not supported (the session has one) — consistent with ADR-001.
- **Edge**: a user whose membership is revoked mid-session is failed closed on the next request —
  correct, and exercised by US-MWT04.

## Slice alignment
Resolution lands in Slice 1 (defaulting the single workspace) and is completed in Slice 3 (US-MWT04:
single vs multi-membership, fail-closed). Slice 5 depends on the single-membership auto-resolution
for the migrated install.
