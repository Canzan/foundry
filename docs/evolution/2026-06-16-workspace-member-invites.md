# Evolution — workspace-member-invites (generalizing invite-accept to general workspace members)

**Finalized**: 2026-06-16
**DELIVER commits**: `3fa73a7` (01-01) → `a1953d7` (03-02) — the 14 DES-monitored TDD steps committed directly to `main` (trunk-based, no PRs) — plus review-remediation `392a9e4` (findings D1, D4) and mutation-hardening `c24e73a` (the three store survivors).
**Wave coverage**: requirements COMPLETE (DISCUSS DoR passed, SSOT); DESIGN ratified here (D1–D8, ADR-001..004, OD-A..D + inherited OD-1 user-ratified); DISTILL authored the 30 member-invite scenarios; DELIVER shipped 14 steps across 3 phases (integrity exit 0 — "All 14 steps have complete DES traces"). Legacy per-feature layout (`docs/feature/workspace-member-invites/`).
**Scope**: this GENERALIZES the shipped first-admin `invite-accept-flow` to general workspace members — adding the two genuinely-new surfaces (admin-gated member-invite ISSUANCE; an account-CREATING accept tx) while preserving the shipped first-admin path byte-identically. The feature directory is PRESERVED (same policy as the parents).

## Milestone — invites now reach general members

The shipped `invite-accept-flow` made the first-admin invite link LIVE but served first-admins ONLY: the invitee already had an account (`created_by` IS the consumer) and the accept tx only set a password. This feature extends the vertical end-to-end so a **workspace admin can invite a brand-new member**, and that member — with no prior account — can **set a password and have their account + membership created atomically on accept**, landing auto-signed-in on the workspace. The provisioning/credential arc (tenancy → provisioning → web-provisioning → invite-accept → **member-invites**) now serves the everyday "admin grows their team" job, not just bootstrap.

Generalized to members, **member role only (v1)**; admin-role member invites are deferred (see below).

## What shipped

Two surfaces, served through ONE `/invites/accept` route via data-derived dispatch:

- **ISSUANCE — `GET`/`POST /workspace/invites`** — an `is_workspace_admin`-gated driving adapter (`member_invites.rs`, mirroring `bootstrap::create_invite` + the admin-gate-inside-handler idiom). GET renders the invite form (CSRF cookie minted); POST validates, calls the SHIPPED `insert_invite` (bind `created_by = the inviting admin`, `invitee_email = the typed email`, `expires_at = now + 7d`), emits a signed invite link, and best-effort emails it. A non-admin OR signed-out request to either route gets a **non-enumerable 404** (byte-identical to a generic not-found via the shipped `resource_not_found_page()`) — never an authorization-leaking 403.
- **ACCEPTANCE — `POST /invites/accept` (member arm)** — the invitee (no prior account) sets a password; the accept POST runs the NEW one-tx `Store::create_member_and_consume`: **guarded-UPDATE consume → INSERT user (argon2id password) → INSERT `role='member'` membership → set `used_by` → COMMIT**. Atomic: none of {consume, user, membership, password} commits without the others. Then auto-signs-in onto the workspace.
- **Kind dispatch** — ONE `/invites/accept` route serves BOTH first-admin and member invites. `submit_accept` derives the kind (`is_first_admin_invite`: consumer == `created_by` with an existing account → first-admin; no existing user for `invitee_email` → member) and routes to the SHIPPED `set_first_admin_password_and_consume` or the NEW `create_member_and_consume`. The shipped first-admin path is unchanged and regression-guarded.
- **`Store::create_member_and_consume`** (NEW driven seam) — the only new backend tx. Fuses the shipped consume guard with `create_initial_workspace`'s user+membership INSERT shape (role swapped to `'member'`, narrowed to one user, the new user_id minted in-tx and set as `used_by`).
- **`invite_accept_view` EXTENDED** — now also surfaces `invitee_email` + `created_by` so the POST can dispatch on invite kind; the GET ignores the new fields.
- **`ROLE_MEMBER` / `ROLE_ADMIN` named constants** — replacing bare `'member'`/`'admin'` string literals at the membership write (review finding D4).
- **ZERO migration. ZERO new crate. No new check-arch LAYER-1e line** — issuance resolves the workspace from `SessionUser.workspace_id` (the trusted seam, not request input), and `is_workspace_admin` is not a `*_in_workspace(` call, so `cargo xtask check-arch` does not flag `member_invites.rs` (D7 held).

## Security — the crux

- **The email-collision arm (OD-1, HIGH-risk)** — an `invitee_email` that already maps to an existing user is caught as a `users.email_lower` **UNIQUE violation (SQLSTATE 23505) INSIDE the tx** → ROLLBACK → `EmailCollision` → the uniform `invite_refusal_page()` (200 OK, byte-identical to the expired-link refusal). It is **NEVER a 500**, never a pre-check SELECT oracle, and the **invite is NOT consumed**. The 23505 catch is specifically scoped to the email-unique constraint — a different constraint failure still surfaces as an error, not a silent refusal. (Multi-workspace-membership-via-invite is deferred; today this case is refused non-enumerably.)
- **Issuance non-enumerability** — non-admin AND signed-out GET/POST to `/workspace/invites` returns byte-identical to a generic 404 and creates no invite.
- **Account creation under concurrency** — two concurrent accepts of one live member invite ⇒ exactly one creates the user + membership + signs in; the other gets the uniform refusal. Exactly one user, one membership, one consumed invite.
- **Atomicity** — a crash mid-tx leaves the invite live with NO orphan user and NO orphan membership (the one-TX rollback boundary).
- **Privilege scope** — the new member's membership is `role='member'`; the new member 404s on `/workspace/invites`.
- **First-admin regression** — a first-admin invite still routes to the SHIPPED tx byte-identically; the dispatch does not change shipped behavior.
- **CSRF** on BOTH state-changing POSTs (issuance + accept) via the shipped double-submit middleware; a forged POST creates no invite / no account / no consume.
- **No leakage** — no `sig` and no password in logs or responses across a full issue+accept+refusal cycle; `tracing` keys on `invite_id` only.
- **Refusal posture reused verbatim** — accept refusals (expired/used/tampered/unknown/**email-collision**) collapse to the SHIPPED `invite_refusal_page()`; issuance refusal collapses to the SHIPPED `resource_not_found_page()`. No new copy.

## Decisions realized (D1–D8)

| # | Decision | Status |
|---|---|---|
| **D1** | Pattern unchanged: modular monolith + ports-and-adapters. Issuance = NEW driving adapter (`member_invites.rs`); accept = EXTENDED driving adapter (kind dispatch + member arm); ONE new driven seam (`create_member_and_consume`). | **IMPLEMENTED** |
| **D2** | Reuse `insert_invite` as-is for issuance (`created_by = inviter`, `invitee_email = typed email`, `expires_at = now+7d`); NO new store fn. | **IMPLEMENTED** |
| **D3** | ONE `/invites/accept` route, internal dispatch on a data-derived kind discriminator (`is_first_admin_invite`) — NO schema column. First-admin → shipped tx; member → new tx. Preserves the shipped first-admin flow byte-identically. | **IMPLEMENTED** |
| **D4** | The NEW `create_member_and_consume` one-TX: guarded-UPDATE consume → INSERT user → INSERT member membership → set `used_by` → COMMIT. Atomic (NFR-2). | **IMPLEMENTED** |
| **D5** | OD-1 collision = catch `users.email_lower` UNIQUE violation (SQLSTATE 23505) INSIDE the tx → ROLLBACK → uniform refusal, invite UNCONSUMED, NOT a 500. No pre-check SELECT, no migration. | **IMPLEMENTED** |
| **D6** | Refusal posture reused verbatim. Accept refusals (incl. email-collision) → shipped `invite_refusal_page()` (200 OK, byte-identical); issuance refusal → shipped `resource_not_found_page()` (non-enumerable 404). | **IMPLEMENTED** |
| **D7** | LAYER-1e: NO new allow-list line. Issuance resolves the workspace from `SessionUser.workspace_id`; `is_workspace_admin` is not a `*_in_workspace(` call. Confirmed against the real `check-arch` run. | **IMPLEMENTED** (no line needed) |
| **D8** | NO migration. Reuse `invites.used_at`/`used_by`, `users.email_lower UNIQUE`, `workspace_memberships.role CHECK`. | **IMPLEMENTED** |

OD-A..OD-D were resolved at DESIGN (Propose mode, recommended options); the inherited OD-1 (email-collision = non-enumerable refusal) was user-ratified before DISTILL.

## How it was built (DELIVER) — the 14-step TDD arc

**14 DES-monitored TDD steps grouped into 30 acceptance scenarios across 3 phases**, each driven by `@real-io` cucumber scenarios over the real surfaces (real axum router, real session + CSRF layers, real testcontainers PG16), every step running all 5 DES phases (integrity exit 0).

| Phase | What it proved |
|---|---|
| **01 — issuance walking skeleton (US-01, D1/D2/D7)** | an admin invites a member at `/workspace/invites` (GET form + POST), `insert_invite` persists with `created_by = inviter`, a signed link is emitted + best-effort emailed; a non-admin/signed-out request 404s non-enumerably and creates no invite; `check-arch` clean (no new LAYER-1e line). |
| **02 — account-creating accept + dispatch + collision (US-02/US-03, D3/D4/D5/D6)** | a member (no account) sets a password and the kind dispatch routes to `create_member_and_consume`: consume + create user + member membership + password in ONE tx, then auto-sign-in; the email-collision arm aborts on the 23505 UNIQUE violation → uniform refusal, invite unconsumed, NOT a 500; concurrent accepts succeed exactly once; CSRF-forged POSTs refused; the shipped first-admin path is regression-guarded byte-identically; no sig/password leaks. |
| **03 — inline recovery + boundaries (US-04, D5)** | a weak / mismatched password corrected inline with the invite left LIVE; a valid retry on the SAME invite completes; member-not-admin privilege scope (the new member 404s on `/workspace/invites`); landing tenant == `invites.workspace_id`. |

## Quality at ship

`cargo xtask ci` — **ALL GATES GREEN**:

- **fmt**: `cargo fmt --all --check` clean.
- **clippy**: `cargo clippy --all-targets --release -- -D warnings` clean.
- **`cargo xtask check-arch`**: PASSED (D7 confirmed — no new LAYER-1e allow-list line; issuance resolves the workspace from the session).
- **`@all` acceptance**: **334 scenarios / 2623 steps** green (parent suites plus this feature's 30 new member-invite scenarios; green-before stays green-after).
- **Adversarial review**: **APPROVED** — **Testing Theater: none found**; the reviewer specifically praised the `used_at`-then-`used_by` FK-ordering-in-one-tx and the specifically-scoped SQLSTATE-23505 catch.
- **Scoped mutation testing**: **store scope 100%** — the three store survivors (`invite_accept_view`, `is_workspace_admin`, `display_name_from_email`) were killed by added store tests (`c24e73a`).
- **Reuse verdict**: **13 reuse · 2 new · 0 migration · 0 new crate.**
- **Review findings fixed (DONE, not outstanding)**:
  - **D1** — the isolation scenario now drives the real board HTTP route, not the store seam (`392a9e4`).
  - **D4** — named `ROLE_MEMBER` / `ROLE_ADMIN` constants replacing bare role string literals (`392a9e4`).
  - **D2** — moot: the five-arm byte-identity scenario is active in the lane.
  - **D3** — a long test helper; deferred as cosmetic.

## Process note

One step's COMMIT-phase DES log (03-02) was logged AFTER its commit (so it was uncommitted), and a later step that reverted `execution-log.json` wiped it. The commit `a1953d7` genuinely exists; the log entry was recovered via the DES CLI and is committed here. **Lesson: commit the `execution-log.json` between steps** to avoid a later revert dropping a trailing uncommitted entry.

## Deferred / follow-ups

**Direct next increments for invites:**
- **Admin-role member invites** — v1 issues member-role only.
- **Bulk invites**, **invite revocation / resend**.
- **Multi-workspace-membership-via-invite** — the "email already maps to an existing user" case is currently refused non-enumerably (OD-1); allowing an existing user to JOIN another workspace via invite is the natural extension of the collision arm.

**Carried from prior features:**
- The deferred `web-provisioning-flow` follow-ups.
- The bootstrap claim-flow enumeration oracle (`bootstrap.rs:124-139` leaks distinct expired/used/not-found — bootstrap NOT modified here; this feature's refusal deliberately does not replicate it).
- Prometheus exporter for `foundry_token_mutations_total`.
- Per-workspace backup/restore (OD-5) — whole-instance backup unchanged.
- Key-rotation UX.
- A nightly/follow-up scoped mutation pass on the web adapter.

## Pointers

- Spec (preserved): `docs/feature/workspace-member-invites/{discuss,design,distill,deliver}/` — notably `design/wave-decisions.md` (D1–D8 + OD-A..D + inherited OD-1), the 4 ADRs (`adr-001..004`), `design/upstream-changes.md` (the grounding correction: `used_at`/`used_by` + `users.email_lower UNIQUE` + `memberships.role CHECK` all already shipped → ZERO migration), and the DISTILL scenarios.
- DES roadmap + execution log (the audit trail, preserved): `docs/feature/workspace-member-invites/deliver/roadmap.json` (3 phases / 14 steps) + `execution-log.json` (DES-verify-integrity clean, exit 0; includes the recovered 03-02 COMMIT entry) + `.develop-progress.json`.
- Core production files:
  - Issuance adapter (NEW): `crates/foundry-app/src/member_invites.rs` (`show_invite_form` GET + `submit_invite` POST, `is_workspace_admin`-gated) + its Askama templates (invite form + "invite sent" fragment).
  - Accept adapter (EXTENDED): `crates/foundry-app/src/invites_accept.rs` (`submit_accept` + the `is_first_admin_invite` kind dispatch + the member arm; `show_accept_form` + `invite_refusal_page()` reused unchanged).
  - Route registration: `crates/foundry-app/src/lib.rs` (`build_router` — the two new `/workspace/invites` routes on the shared layer).
  - Store seam (NEW + EXTEND): `crates/foundry-store/src/lib.rs` (`create_member_and_consume` NEW; `invite_accept_view` EXTENDED to surface `invitee_email` + `created_by`; `ROLE_MEMBER`/`ROLE_ADMIN` constants).
  - Reused verbatim (shipped): `insert_invite`, `set_first_admin_password_and_consume`, `is_workspace_admin`, `resolve_active_workspace`, `InviteToken::verify`, `hash_password`, `check_password_policy`, the session + CSRF layers, `resource_not_found_page` / `invite_refusal_page`.
  - Acceptance: the member-invites feature file + step defs in `crates/foundry-acceptance/`.
- Predecessor (the vertical this generalizes): `docs/evolution/2026-06-14-invite-accept-flow.md` (the first-admin `/invites/accept` flow), and the provisioning arc behind it: `2026-06-13-web-provisioning-flow.md`, `2026-06-12-multi-workspace-provisioning.md`, `2026-06-11-multi-workspace-tenancy.md`.
