# ADR-005 — Reuse verdict & the LAYER-1e allow-list confirmation

## Status
**IMPLEMENTED / SHIPPED** (finalized 2026-06-14). DESIGN wave. Resolves the reuse-vs-new framing and the LAYER-1e (D7) question — confirmed against the real `cargo xtask check-arch` run: NO new allow-list line (uses the `resolve_active_workspace` seam like `signin`). See `docs/evolution/2026-06-14-invite-accept-flow.md`.

## Context
The task asks: maximize reuse; identify the genuinely-new pieces; and confirm whether the new handler
file owes a LAYER-1e (`is_tenant_scoping_allowlisted`, `check_arch.rs:387-402`) allow-list line. The
detector flags a handler that names a literal/parsed `workspace_id` OUTSIDE the resolution seam (the
provisioning paths — `bootstrap`, `admin_cli`, `instance_admin` — are allow-listed because they create
brand-new workspace ids; `signin` and `session` are allow-listed as the resolution seam itself).

## Reuse verdict (8 REUSE/EXTEND · 4 CREATE-NEW · 0 RETIRE · 0 MIGRATION)
**Reused verbatim/shape (shipped):** `InviteToken::verify` (HMAC), `hash_password` (argon2id), the
`claim_bootstrap_token` guarded-UPDATE idiom, `resolve_active_workspace`, the session insert + 303
idiom, `csrf_middleware` + `ensure_csrf_cookie`, the `resource_not_found_page` refusal shape, the
`invites.used_at`/`used_by` columns (NO migration — adr-001).

**Genuinely new:** (1) `Store::consume_invite` + `set_first_admin_password_and_consume` (the only real
new backend — a guarded-UPDATE + a one-TX wrapper, mirroring a shipped fn); (2) the
`invites_accept.rs` driving adapter (2 handlers); (3) the set-password + uniform-refusal templates;
(4) `foundry_auth::check_password_policy` (tiny, shared). Plus 2 `.route(...)` lines (EXTEND).

## The LAYER-1e question
The accept handler:
- READS an invite by id (no workspace id named directly).
- WRITES a credential onto `users WHERE id = invites.created_by` (still no workspace id named).
- RESOLVES the landing workspace via `resolve_active_workspace(user_id)` — **the resolution seam**,
  exactly as the allow-listed `signin.rs:149` does. It never handles a literal/parsed `workspace_id`
  the way provisioning does.

## Options considered
- **(a) NO allow-list line; the handler uses the resolution seam like `signin` (RECOMMENDED).** The
  detector should not trip on a handler that goes THROUGH `resolve_active_workspace` rather than naming
  a workspace id. Confirm against the real `check_arch` run at DELIVER.
- **(b) Pre-emptively add `invites_accept` to the allow-list.** Avoided unless needed — adding an
  unnecessary exemption widens the LAYER-1e blind spot. The cheap, reversible fallback is to add the
  one line IF (and only if) `check_arch` flags the new file.

## Decision
**(a)** — provisionally NO new allow-list line; the accept handler uses the `resolve_active_workspace`
seam, inheriting the `signin` rationale. DELIVER confirms against the real `cargo xtask check-arch`
run; if the detector flags the new file, add `Some("invites_accept")` to `is_tenant_scoping_allowlisted`
(one line) — a cheap, reversible fallback. This mirrors how `web-provisioning-flow` D6 handled its
allow-list question, but the verdict differs because THIS handler uses the resolution seam (a
non-provisioning, non-literal-workspace path).

## Consequences
- **Positive**: keeps the LAYER-1e allow-list minimal (no unnecessary exemption); the verdict is
  grounded in how `signin` (the closest analogue) is treated.
- **Negative**: a small DELIVER-time confirmation step (run check-arch, observe). Reversible either way.
- **Security**: not weakening the tenant-scoping detector by default is the conservative choice; the
  one-line fallback is applied only if empirically required.

## Relationship
Inherits the LAYER-1e reasoning from `multi-workspace-provisioning` ADR-003 and `web-provisioning-flow`
D6. The reuse verdict realizes the `web-provisioning-flow` ADR-005 deferred accept vertical with maximal
reuse of shipped seams.
