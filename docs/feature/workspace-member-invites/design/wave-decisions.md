# DESIGN Decisions — workspace-member-invites

> Morgan (nw-solution-architect), DESIGN wave, application/component scope, **Propose** mode.
> This feature GENERALIZES the shipped first-admin `invite-accept-flow` to general workspace members.
> Two genuinely-new things: (1) an admin-gated member-invite ISSUANCE surface; (2) an account-CREATING
> accept tx (`create_member_and_consume`). Everything else is reused verbatim. Requirements are COMPLETE
> (DoR passed; DISCUSS is the SSOT — see `../discuss/`). Paradigm is ESTABLISHED and NOT re-decided:
> Rust, modular monolith, ports-and-adapters, functional-core / imperative-shell. Legacy per-feature
> layout. Trunk-based. Template generalized: `docs/feature/invite-accept-flow/design/`.

## Headline findings (grounded in shipped code — read first)

1. **No `kind`/`role` column on `invites`.** `0001_init.sql:93-102` is `id, workspace_id, invitee_email,
   created_by, expires_at, used_at, used_by, created_at`. The first-admin/member discriminator is
   **data-derived**, not a column → **NO migration** (D3, ADR-003).
2. **The discriminator already exists in the data.** First-admin invite: `created_by` IS the consumer
   (a pre-existing user). Member invite (ADR-001): `created_by` is the inviting ADMIN; the invitee has no
   account. So "does `invitee_email` map to an existing user?" distinguishes the kinds (D3).
3. **`users.email_lower` is already `UNIQUE`** (`0001:19`). The OD-1 collision is caught as a UNIQUE
   violation INSIDE the member tx and mapped to the uniform refusal — race-safe, non-enumerable, NOT a
   500 (D4, ADR-002/004).
4. **`workspace_memberships.role` already `CHECK (role IN ('admin','member'))`** (`0001:29`) — `'member'`
   is valid; **no role migration**.
5. **`insert_invite` already fits the member case** — `(id, workspace_id, invitee_email: Option<&str>,
   created_by: Uuid, expires_at)` (`store/lib.rs:541`). Bind `created_by = inviter`. **No new
   `insert_member_invite` fn** (D2, ADR-001).
6. **LAYER-1e: no new allow-list line.** The issuance handler resolves the workspace from the SESSION
   (not request input) and `is_workspace_admin` is not a `*_in_workspace(` call, so the `check_arch`
   detector (Pass-1/Pass-2, `check_arch.rs:332-380`) does not flag it (D7, ADR-001).

**Net: ONE new store tx (`create_member_and_consume`), ONE new web file (`member_invites.rs`, 2
handlers), a small extension to `submit_accept` (the kind dispatch), two thin templates, ZERO new crates,
ZERO migration.**

## Reading checklist

- [x] `../discuss/requirements.md` (FR-1..8, NFR-1..7, BR-1..6 — the SSOT)
- [x] `../discuss/user-stories.md` (US-01 issuance, US-02 account-creating accept, US-03 non-enumerable+single-use, US-04 inline recovery)
- [x] `../discuss/acceptance-criteria.md` (AC-01.1..04.5 + the 4 @property criteria)
- [x] `../discuss/journey-member-invite-visual.md` (two emotional arcs; sad paths I-E1..5, A-E1..9; the uniform refusal copy)
- [x] `../discuss/shared-artifacts-registry.md` (12 tracked artifacts; the invitee_email/user_id/membership DELTAs are the new-account seam)
- [x] `docs/feature/invite-accept-flow/design/{architecture.md, wave-decisions.md, adr-001..005}` — the template this generalizes
- [x] `crates/foundry-store/migrations/0001_init.sql:8-102` (**users.email_lower UNIQUE; memberships.role CHECK; invites incl. used_at/used_by — NO kind column**) — latest migration `0011`
- [x] `crates/foundry-store/src/lib.rs:290-326` (`set_first_admin_password_and_consume` — the shipped consume+write tx to MIRROR; `used_by = created_by`)
- [x] `crates/foundry-store/src/lib.rs:357-438` (`create_initial_workspace` — the user+membership INSERT pattern the member tx reuses, role swapped to `'member'`, narrowed to one user)
- [x] `crates/foundry-store/src/lib.rs:484-501` (`resolve_active_workspace` — REUSE for the landing)
- [x] `crates/foundry-store/src/lib.rs:541-561` (`insert_invite` — `created_by: Uuid` required; REUSE as-is)
- [x] `crates/foundry-store/src/lib.rs:582-598` (`invite_accept_view` — TODAY returns only `(expires_at, used_at, workspace_name)`; **EXTEND** to ALSO surface `invitee_email` + `created_by` for the kind dispatch — load-bearing, D3/adr-003)
- [x] `crates/foundry-store/src/lib.rs:1222-1236` (`is_workspace_admin` — the issuance authz gate)
- [x] `crates/foundry-auth/src/lib.rs` (`InviteToken::new/verify`, `hash_password`, `check_password_policy` — REUSE verbatim)
- [x] `crates/foundry-app/src/invites_accept.rs` (the accept GET/POST to EXTEND with the kind dispatch + member arm; `invite_refusal_page()` reused unchanged)
- [x] `crates/foundry-app/src/bootstrap.rs:204-263` (`create_invite` — the issuance handler body to MIRROR) + `:299,:339` (`bootstrap_refusal_page`/`resource_not_found_page`)
- [x] `crates/foundry-app/src/instance_admin.rs:77-131` (the admin-gate-inside-handler + `resource_not_found_page()` non-enumerable idiom to MIRROR)
- [x] `crates/foundry-app/src/lib.rs:235-403` (`build_router` — the shared web layer mount point, UNDER session + CSRF; admin routes gate inside the handler)
- [x] `crates/foundry-app/src/csrf.rs` (`csrf_middleware`, double-submit) + `signin.rs` (`ensure_csrf_cookie` public-GET seam)
- [x] `xtask/src/check_arch.rs:314-402` (LAYER-1e detector + `is_tenant_scoping_allowlisted`)

## Key Decisions (DDD-numbered)

| # | Decision | Rationale | ADR |
|---|---|---|---|
| **D1** | **Pattern unchanged: modular monolith + ports-and-adapters.** Issuance = a NEW driving adapter (`member_invites.rs`, 2 handlers); accept = the EXTENDED driving adapter (`invites_accept.rs`, add the kind dispatch + member arm); the ONE new driven seam is `create_member_and_consume`. | Inherited and in force; the feature is an extension of a shipped vertical, not a new architecture. | architecture.md |
| **D2** | **Reuse `insert_invite` as-is for issuance; NO new store fn.** Bind `created_by = the inviting admin`, `invitee_email = the typed email`, `expires_at = now + 7d`. The handler mirrors `bootstrap::create_invite`. | The signature already matches (`created_by: Uuid`, `invitee_email: Option<&str>`). A sibling fn would duplicate the INSERT for no behavioral gain. | adr-001 |
| **D3** | **ONE `/invites/accept` route, internal DISPATCH on a data-derived kind discriminator — NO schema column.** First-admin (consumer == `created_by`, account exists) → SHIPPED `set_first_admin_password_and_consume`; member (no existing user for `invitee_email`) → NEW `create_member_and_consume`. | The kind is already derivable from `created_by` + `invitee_email`; `invites` has no `kind` column and needs none. Preserves the shipped first-admin flow byte-identically. | adr-003 |
| **D4** | **The NEW `create_member_and_consume` one-TX: guarded-UPDATE consume → INSERT user → INSERT member membership → set `used_by` → COMMIT.** Atomic (NFR-2): none of {consume, user, membership, password} happens without the others. | Mirrors the shipped consume guard fused with `create_initial_workspace`'s user+membership INSERT (role swapped to `'member'`, narrowed to one user). The first-admin user_id has no analogue here, so the new user_id is minted in-tx and set as `used_by`. | adr-002 |
| **D5** | **OD-1 email collision = catch `users.email_lower` UNIQUE violation (SQLSTATE 23505) INSIDE the tx → ROLLBACK → `EmailCollision` → uniform `invite_refusal_page()`.** NOT a pre-check SELECT; NOT a 500. The invite stays UNCONSUMED. | UNIQUE-catch is race-safe (no TOCTOU), non-enumerable (no SELECT oracle in the handler), and avoids a constraint-error 500 leaking the collision. Reuses the existing UNIQUE constraint — no migration. | adr-002, adr-004 |
| **D6** | **Refusal posture reused verbatim.** Accept refusals (expired/used/tampered/unknown/**email-collision**) → the SHIPPED `invite_refusal_page()` (200 OK, byte-identical). Issuance refusal (non-admin/signed-out) → the SHIPPED `resource_not_found_page()` (non-enumerable 404). | The shipped uniform-refusal + non-enumerable-404 idioms are mutation-hardened and already proven; the email-collision arm collapses into the SAME accept refusal (the only new arm). No new copy. | adr-002, adr-001 |
| **D7** | **LAYER-1e: NO new allow-list line.** The issuance handler resolves the workspace from `SessionUser.workspace_id` (the trusted seam), never from request input; `is_workspace_admin` is not a `*_in_workspace(` call. The accept extension stays in the already-clean `invites_accept.rs`. | The `check_arch` detector flags only `*_in_workspace(` calls scoped by a `Uuid::parse*`-of-request local. Neither new path does that. Confirm at DELIVER; one-line fallback if a future refactor trips it. | adr-001 |
| **D8** | **NO migration.** Reuse `invites.used_at`/`used_by`, `users.email_lower UNIQUE`, `workspace_memberships.role CHECK`. | Every column the feature writes already exists with the right constraint (headline findings 1, 3, 4). | adr-004 |

## Architecture Summary

- **Pattern**: modular monolith + ports-and-adapters (inherited). Issuance is a NEW driving adapter
  mirroring `bootstrap::create_invite`; accept is the EXTENDED shipped adapter with an internal kind
  dispatch; the genuinely-new backend is `create_member_and_consume` (one TX: consume + create user +
  member membership + password, with the UNIQUE-email collision arm).
- **Paradigm**: Rust, composition-over-inheritance, functional-core / imperative-shell — UNCHANGED.
- **Key components** (see `architecture.md` C4 L1+L2 + the component diagram):
  - `member_invites.rs` — `show_invite_form` (GET) + `submit_invite` (POST), the admin-gated issuance
    driving adapter (NEW).
  - `invites_accept::submit_accept` — EXTENDED with the kind dispatch + the member arm; `show_accept_form`
    + `invite_refusal_page()` reused unchanged.
  - `Store::create_member_and_consume` (NEW driven seam) + `set_first_admin_password_and_consume`
    (SHIPPED, reused for the first-admin arm).
  - 2 Askama templates: the member-invite form + the "invite sent" fragment (NEW); the
    `InviteAcceptPage` set-password template reused (only the "join as a member" copy nuance).
  - Two `.route("/workspace/invites", get().post())` lines in `build_router` on the SHARED layer (EXTEND).
  - Everything else — `InviteToken`, `hash_password`, `check_password_policy`, `is_workspace_admin`,
    `insert_invite`, `resolve_active_workspace`, session, CSRF, `resource_not_found_page` — SHIPPED, REUSED.

## Reuse Analysis (verdict: 13 REUSE/EXTEND · 2 CREATE-NEW · 0 RETIRE · **0 MIGRATION**)

| # | Component | File | Decision | Justification |
|---|---|---|---|---|
| 1 | `invites.used_at`/`used_by` single-use markers | `migrations/0001_init.sql:99-100` | **REUSE (verbatim)** | The single-use guard reuses them; no migration. |
| 2 | `users.email_lower UNIQUE` | `0001_init.sql:19` | **REUSE (verbatim)** | The OD-1 collision guard (D5). No migration. |
| 3 | `workspace_memberships.role CHECK(admin\|member)` | `0001_init.sql:29` | **REUSE (verbatim)** | `'member'` already valid; the new membership binds it. No migration. |
| 4 | `insert_invite` | `store/lib.rs:541` | **REUSE (as-is)** | Signature already fits the member case; bind `created_by = inviter` (D2). |
| 5 | `is_workspace_admin` | `store/lib.rs:1222` | **REUSE (verbatim)** | The issuance authz gate (GET + POST). |
| 6 | `set_first_admin_password_and_consume` | `store/lib.rs:290` | **REUSE (verbatim)** | The first-admin arm of the kind dispatch (D3). Untouched. |
| 7 | `create_initial_workspace` user+membership INSERT pattern | `store/lib.rs:380-398` | **REUSE (shape)** | The model for `create_member_and_consume`'s INSERTs (role → `'member'`, one user). |
| 8 | `resolve_active_workspace` | `store/lib.rs:484` | **REUSE (verbatim)** | Landing resolution post-consume. |
| 9 | `InviteToken::new`/`verify`, `hash_password`, `check_password_policy` | `foundry-auth/src/lib.rs` | **REUSE (verbatim)** | Sign/verify, hash, min-12 policy — all shipped. |
| 10 | accept GET + `invite_refusal_page()` + `InviteAcceptPage` | `invites_accept.rs` | **REUSE (verbatim)** | GET non-committal + uniform refusal unchanged; only the member arm + dispatch are added to POST. |
| 10a | `invite_accept_view` (the accept read) | `store/lib.rs:582` | **EXTEND** | Today returns `(expires_at, used_at, workspace_name)`; ADD `invitee_email` + `created_by` so the POST can dispatch on invite kind (D3, adr-003). The GET ignores the new fields. |
| 11 | `bootstrap::create_invite` issuance body | `bootstrap.rs:204` | **REUSE (shape)** | The issuance handler mirrors it + the admin gate. |
| 12 | `resource_not_found_page()` + admin-gate-inside-handler idiom | `bootstrap.rs:339`, `instance_admin.rs:82` | **REUSE (verbatim + shape)** | The non-enumerable issuance 404 (NFR-1). |
| 13 | session + `csrf_middleware` + `ensure_csrf_cookie` + `build_router` mount | `session.rs`, `csrf.rs`, `signin.rs`, `lib.rs` | **REUSE + EXTEND** | Both POSTs CSRF-protected; the two issuance routes register on the shared layer. |
| 14 | `Store::create_member_and_consume` | — (does not exist) | **CREATE NEW (driven)** | The one new tx: consume + create user + member membership + password + collision arm (D4/D5). |
| 15 | `member_invites.rs` (2 handlers) + 2 templates + `submit_accept` dispatch | — (handlers/templates new; dispatch extends shipped) | **CREATE NEW (driving) + EXTEND** | The issuance vertical + the accept kind dispatch (D2/D3). |

## Technology Stack

- **Rust** (inherited): axum, askama, tower_sessions, the shipped `csrf_middleware`, sqlx, `foundry-auth`.
  **ZERO new crates.**
- **PostgreSQL** (one instance, inherited): **ZERO migration** — every column already exists with the
  right constraint.
- **Enforcement**: `cargo xtask check-arch` (inherited; **ZERO new allow-list line**, D7).
- **OSS-first / license**: all inherited deps; no proprietary; no new dependency to license.

## Architecture Enforcement (for software-crafter)

Style: Modular Monolith + Hexagonal (ports-and-adapters). Language: Rust. Tool: `cargo xtask check-arch`.

Rules to enforce:
- `foundry-store` has zero inward dependency on the web adapter.
- The consume + create-user + membership-insert + password write live in ONE `foundry-store` TX fn
  (`create_member_and_consume`) — the handler must NOT issue the guarded-UPDATE or the INSERTs itself.
- LAYER-1e: the issuance handler scopes nothing by a request-parsed workspace id (it uses
  `session.workspace_id`); confirm `check_arch` does not flag `member_invites.rs` at DELIVER; one-line
  allow-list fallback only if flagged.

## Constraints honored

- ONE binary · ONE Postgres · NO Redis · NO Node · NO CDN · **ZERO new crates** · **ZERO migration**.
- Issuance is admin-gated (`is_workspace_admin` on GET + POST), non-enumerable 404 for non-admins.
- The accept page stays PUBLIC (the invitee is signed out); BOTH state-changing POSTs are CSRF-protected.
- Refusals are non-enumerable: byte-identical body AND status across expired/used/tampered/unknown AND
  email-collision (D6); issuance 404 byte-identical to a generic not-found.
- No `sig`, no password in logs; `tracing` keys on `invite_id` only.
- The shipped first-admin flow is preserved byte-identically (the kind dispatch routes it to the
  unchanged tx); the `foundry-acceptance` `@all` suite green-before stays green-after.

## Earned-Trust (probe-don't-assume) commitments for DISTILL/DELIVER

- **Issuance non-enumerability PROBED**: a non-admin AND a signed-out GET/POST to `/workspace/invites`
  returns a response byte-identical to a generic 404 and creates no invite (revert-reds-it litmus,
  @property, AC-03.1).
- **Account creation under concurrency PROBED**: two concurrent accepts of one live member invite ⇒
  exactly one creates the user + membership + signs in; the other gets the uniform refusal; exactly one
  user, one membership, one consumed invite (NFR-2, @property, AC-03.6).
- **Email-collision → uniform-refusal PROBED**: an `invitee_email` that already maps to an existing user
  ⇒ the tx aborts on the UNIQUE violation, the invite is NOT consumed, the response is byte-identical to
  the expired-link refusal (NOT a 500) — the HIGH-risk arm the DISCUSS flagged (AC-03.8, A-E9).
- **Atomicity PROBED**: a crash mid-tx leaves the invite live with NO orphan user and NO orphan
  membership (the one-TX rollback boundary).
- **Member-not-admin PROBED**: the new member's membership is `role='member'`; the new member 404s on
  `/workspace/invites` (privilege scope, AC-02.6).
- **First-admin regression PROBED**: a first-admin invite still routes to the SHIPPED tx; the `@all`
  acceptance suite stays GREEN (the kind dispatch does not change the shipped behavior).
- **CSRF PROBED**: a forged issuance POST creates no invite; a forged accept POST creates no account, no
  consume (NFR-6, AC-03.9).
- **No-secret-leakage PROBED**: a log scan after a full issue+accept+refusal cycle contains no `sig` and
  no password (NFR-5, @property, AC-03.10).
- **Tenant landing PROBED**: the landed `workspace_id == invites.workspace_id`; the new member sees only
  that tenant (AC-02.4).
- **LAYER-1e PROBED**: `cargo xtask check-arch` does not flag `member_invites.rs` (D7); if it does, the
  one-line allow-list add is applied (reversible fallback).

## Open decisions — RESOLVED (Propose mode; recommended option each; orchestrator auto-accepts)

| # | Open decision | Recommended option (RESOLVED) | ADR |
|---|---|---|---|
| **OD-A** | How ONE `/invites/accept` serves both first-admin AND member invites (the kind discriminator). | **Data-derived dispatch inside `submit_accept`** — first-admin (consumer == `created_by`, account exists) → SHIPPED tx; member (no existing user for `invitee_email`) → NEW `create_member_and_consume`. **NO `kind` column, NO migration.** | adr-003 (D3) |
| **OD-B** | New `insert_member_invite` fn, or reuse `insert_invite`? | **Reuse `insert_invite` as-is** — its `(created_by: Uuid, invitee_email: Option<&str>)` signature already fits; bind `created_by = inviter`. No new fn. | adr-001 (D2) |
| **OD-C** | LAYER-1e: does the workspace-scoped issuance handler need a `check_arch` allow-list line? | **NO line.** It resolves the workspace from the SESSION (trusted seam), parses no workspace id from request input, and `is_workspace_admin` is not a `*_in_workspace(` call. Confirm at DELIVER; one-line fallback if flagged. | adr-001 (D7) |
| **OD-D** | Migration needed? | **NO migration.** `invites.used_at`/`used_by`, `users.email_lower UNIQUE`, `workspace_memberships.role CHECK` all already exist. (If a future admin-role invite needed a column, it would be `0012_invites_role.sql` — reserved-on-paper, NOT created.) | adr-004 (D8) |
| **OD-1** (inherited, USER-RATIFIED) | Email-already-a-user collision behavior. | **Refuse non-enumerably** — UNIQUE-violation catch inside the tx → uniform refusal, invite NOT consumed, NOT a 500. Multi-workspace-membership deferred. | adr-002/004 (D5) |

## Residual open decisions

**None blocking.** All four task open decisions (OD-A..OD-D) are resolved with recommended options above.
Two non-blocking notes carried forward:
- **OD-3 (inherited)**: promote the two `job_id`s to `docs/product/jobs.yaml` — a documentation
  formality, not a behavior change; there is no `jobs.yaml` in the repo today. DEFERRED.
- **NFR-7 (accessibility)**: the two new forms should meet WCAG 2.1 AA (labeling, focus order, error
  association); they reuse the shipped template/admin-tier baseline. Flagged for implementation review,
  not gated at DESIGN.

## Upstream changes

See `upstream-changes.md` — one grounding correction (the DISCUSS grounding table listed the `invites`
columns without `used_at`/`used_by`, and did not note that `users.email_lower` is already UNIQUE or that
`memberships.role` already CHECK-allows `'member'`; all three are present in `0001_init.sql`, confirming
ZERO migration). No parent DISCUSS docs are modified; the corrections are recorded here.
