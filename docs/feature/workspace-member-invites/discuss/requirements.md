# Requirements — workspace-member-invites

## Context

The shipped `invite-accept-flow` (see `docs/feature/invite-accept-flow/`) made the
`/invites/accept?id=…&sig=…` link live for the **first-admin** of a freshly provisioned workspace — a
user who already exists (seeded by `provision_workspace` as the invite's `created_by`). This feature
**generalizes** that flow to **general workspace members**, adding two capabilities:

1. **Member-invite issuance**: a workspace **admin** invites a person by email into their workspace as a
   member. The system creates an `invites` row (`insert_invite`) and emits a signed `/invites/accept?…`
   link (reusing `InviteToken`). This is NEW: the first-admin invite was minted by `provision_workspace`;
   there was no admin-facing issuance surface.
2. **Member-invite acceptance**: the invitee opens the link, sets a password, and **joins** — but unlike
   the first-admin case (where the user pre-exists), a member invitee likely has **no account yet**, so
   accept must **CREATE the user + ADD a member-role workspace membership + set the password**, in ONE
   atomic transaction, then auto-sign-in onto that workspace.

## Scope (v1, recommended defaults)

- **In scope**: member-invite issuance (admin-gated web form) and member-invite acceptance (creates the
  account + member membership + password, atomically; auto-sign-in).
- **Out of scope** (deferred follow-ups): inviting as the **admin** role (member role only in v1); bulk
  invites; invite **revocation/resend**; a **CLI-native** issuance command (the emitted link is a web
  URL — the web route suffices); **multi-workspace-membership-via-invite** for an email that is already a
  Foundry user (see OD-1 — recommend refusing non-enumerably in v1).

## Brownfield grounding (shipped seams — reuse, do not reinvent)

| Seam | Location | Reuse |
|------|----------|-------|
| `InviteToken::new` / `verify` | `crates/foundry-auth/src/lib.rs:354-386` | HMAC binds `invite_id`\|\|`expires_at`. Reused verbatim for issuance signing and accept verification. |
| `invites` table + `insert_invite` | `crates/foundry-store/src/lib.rs:541` | Columns: `id, workspace_id, invitee_email, created_by, expires_at, used_at, used_by`. Reused; `invitee_email` now holds the prospective member's email; `created_by` = the inviting admin. |
| `is_workspace_admin` | `crates/foundry-store/src/lib.rs:1222` | `EXISTS(... role='admin')`. The issuance authz gate. |
| `hash_password` (argon2id) + `check_password_policy` (min-12) | `crates/foundry-auth/src/lib.rs:319, :393, :406` | Reused verbatim; min-12 policy already SHIPPED (unlike at the first-admin flow's inception). |
| Accept GET handler + form + uniform refusal | `crates/foundry-app/src/invites_accept.rs::show_accept_form`, `invite_refusal_page`, `views::InviteAcceptPage` | GET reused verbatim; uniform refusal reused verbatim; POST swaps only the consume tx. |
| Issuance pattern to mirror | `crates/foundry-app/src/bootstrap.rs::create_invite:204`, `admin_tokens.rs::submit_mint:78` | The member-issuance handler mirrors `create_invite` (insert + sign + emit + best-effort email) plus the `admin_tokens`/`/admin/instance` admin-gate + `not_found()` non-enumerable posture. |
| Session + double-submit CSRF | `crates/foundry-app/src/{session,csrf}.rs` | Reused for the admin issuance POST and the public accept POST. |
| `resolve_active_workspace` | `crates/foundry-store/src/lib.rs:484`; `signin.rs:149` | Reused for the auto-sign-in landing. |
| Membership seeding pattern | `provision_workspace:1295` / `create_initial_workspace:357` (the `INSERT workspace_memberships ... role=...`) | The model for the NEW `create_member_and_consume` tx, with role swapped from `'admin'` to `'member'` and narrowed to one user. |
| Route registration | `crates/foundry-app/src/lib.rs::build_router:235` | The new GET/POST `/workspace/invites` register on the admin-gated layer; `/invites/accept` already registered (public). |

### The single genuinely-new store transaction (DESIGN owns the exact shape)

The shipped `set_first_admin_password_and_consume` (`lib.rs:290`) writes the password onto the
**pre-existing** `created_by` row. The member case has NO pre-existing user. DESIGN must add a sibling
tx — call it `create_member_and_consume(invite_id, password_hash, now)` — that, under the SAME atomic
0-or-1-row consume guard (`UPDATE invites SET used_at=$2 WHERE id=$1 AND used_at IS NULL AND expires_at >
$2 RETURNING workspace_id, invitee_email`), then **creates the users row** (email from
`invites.invitee_email`, the chosen `password_hash`), **inserts a `workspace_memberships` row with role
`'member'`**, and sets `invites.used_by` to the new user id — all in one transaction. Requirements are
written solution-neutrally; DESIGN owns the SQL.

## Alternatives considered (constraint rationale)

The v1 scope constraints are deliberate; each was weighed against alternatives:

- **7-day expiry** (vs 3d / 14d): chosen to match the already-emitted "valid for 7 days" promise in the
  shipped invite email and `provision_workspace` (consistency avoids a confusing mismatch between
  first-admin and member invites). 3d risks an invitee missing the window on an async/out-of-band hand-off;
  14d widens the leaked-link danger window. 7d is the established, HMAC-bound balance — rejected the
  others to keep one lifetime across all invite kinds.
- **Web-only issuance** (vs CLI-native `foundry workspace invite`): the emitted artifact is a web URL, so
  the web accept route already serves every emit site; a CLI issuance command adds a surface without
  changing the user outcome for v1. Deferred (not rejected) — it rides the same `insert_invite`/`InviteToken`
  seams when scoped.
- **Member role only** (vs member + admin-invitable): admin-role member invites raise privilege-escalation
  stakes (an invite that grants admin must be more tightly controlled) and are not needed for the core job
  ("bring a teammate in"). Scoping to `member` keeps the `create_member_and_consume` tx simple and the blast
  radius small; admin-role invites deferred to a follow-up.
- **Non-enumerable refusal reuse** (vs distinct messages like the shipped bootstrap claim flow): reused the
  mutation-hardened uniform-refusal posture because distinct messages are an enumeration oracle (the very
  flaw the shipped instance-admin surface remediated). Rejected per-reason copy on security grounds.
- **Email-already-a-user → refuse** (vs auto-join existing user): see OD-1 — the simplest coherent v1 that
  avoids silent cross-workspace joins and the multi-membership questions they raise. Recommended; revisit
  when multi-workspace-membership-via-invite is scoped.

> This feature is overwhelmingly an EXTENSION of the shipped, validated `invite-accept-flow` and
> `multi-workspace-provisioning` patterns; reuse-over-reinvent is the deliberate engineering choice, not
> availability bias. Where a genuine product choice exists (OD-1), it is flagged as an open decision.

## Functional requirements

### Issuance
- **FR-1** GET `/workspace/invites` renders a member-invite form (one email field) for a signed-in
  **workspace admin** of the active workspace. A non-admin or signed-out caller gets a **non-enumerable
  404** (the route does not reveal it exists).
- **FR-2** POST `/workspace/invites` (carrying `email`, `_csrf`), gated by `is_workspace_admin`,
  validates the email, creates an `invites` row (`workspace_id` = the admin's active workspace,
  `invitee_email` = the typed email, `created_by` = the admin, `expires_at` = now + 7 days), signs an
  `InviteToken`, emits the `/invites/accept?id&sig` link (shown to the admin AND best-effort emailed),
  and confirms.
- **FR-3** A blank or malformed email is corrected **inline**; no invite is created.

### Acceptance
- **FR-4** GET `/invites/accept?id&sig` verifies the signed token and liveness (unexpired AND unconsumed)
  and renders a set-password form naming the workspace ("join as a member"). Non-committal (no mutation).
  (Reused verbatim from the shipped flow.)
- **FR-5** POST `/invites/accept` (carrying `id`, `sig`, password, confirm, `_csrf`) atomically:
  re-verifies the signature, validates the password (min-12 + confirm match) BEFORE any mutation, then
  **creates the user, adds a member-role membership, consumes the invite (single-use), and writes the
  password** in ONE transaction, establishes a session, and 303-redirects to `/`.
- **FR-6** A valid accept lands the new member on **their** workspace (`invites.workspace_id` via
  `resolve_active_workspace`), seeing only that tenant's data, with **member** (not admin) privileges.
- **FR-7** An invalid / expired / already-used / tampered link — OR an invite whose email already maps to
  an existing user (OD-1) — renders the **uniform, non-enumerable** refusal page (same body for all).
- **FR-8** A weak or mismatched password is corrected **inline**; the invite is **NOT consumed**, **no
  account is created**, and the invite stays live for retry. (Reused US-03 inline-recovery path.)

## Non-Functional Requirements (Security — first-class)

### NFR-1 — Issuance authorization (admin-only, non-enumerable for non-admins)
Only a workspace **admin** of the active workspace may issue a member invite. Authz is `is_workspace_admin`,
checked on BOTH the GET (form) and the POST (send). A signed-in non-admin or a signed-out caller receives
a **non-enumerable 404** — byte-identical to a generic not-found — so the issuance surface does not admit
to existing (mirrors the shipped `/admin/tokens` and `/admin/instance/…` posture).
- Measurable: a non-admin GET/POST to `/workspace/invites` returns a response byte-identical to a generic
  404 and creates no invite; an admin GET renders the form.

### NFR-2 — Single-use atomic accept (create-user + membership + password in one tx, race/TOCTOU-safe)
An invite is consumable **exactly once**, and the account it creates is created **exactly once**. Consume
is an atomic guarded operation — conceptually `UPDATE invites SET used_at=now WHERE id=$1 AND not-yet-used
AND expires_at > now`, 0 rows ⇒ refuse. The user creation, the member-membership insert, the password
write, and the consume mark occur in the **same transaction**; none happen without the others.
- Measurable: two concurrent POSTs for one invite ⇒ exactly one creates the user + membership and signs
  in; the other gets the uniform refusal; exactly one user, one membership, and one consumed invite
  exist. Re-opening a consumed link ⇒ uniform refusal, no second account, no session.

### NFR-3 — Non-enumerable acceptance refusals (no existence oracle)
Every invalid-link reason (expired, already-used, invalid/tampered signature, unknown id) AND the
email-already-a-user case (OD-1) produces a **byte-identical** user-visible refusal page (body + status).
It reveals nothing about whether an account, user, workspace, invite, or email-collision exists.
Responses differ ONLY in internal `tracing`. (Reused `invite_refusal_page()`, extended with the A-E9 arm.)
- Measurable: a falsifiability litmus REDs if any refusal arm's body or status diverges (the
  revert-reds-it pattern shipped for the first-admin flow).

### NFR-4 — Password strength (min-12, reused)
The chosen password must meet the SHIPPED minimum strength policy (`check_password_policy`, min-12,
NIST 800-63B length-first) before it is accepted; argon2id `hash_password` is reused verbatim. A rejected
password creates no account and does not consume the invite (FR-8).
- Measurable: a password below 12 chars is refused inline; one at/above 12 is accepted.

### NFR-5 — No token/secret leakage (logs, URLs-at-rest)
The invite `sig` and the chosen password must never be written to application logs. Issuance and accept
logging key on `invite_id` only — never the signature, password, or the new user's password hash; error
logs must not echo the query string or form body.
- Measurable: a log scan after a full issue + accept + refusal cycle contains no `sig` value and no password.

### NFR-6 — Request-forgery protection on BOTH state-changing POSTs
The issuance POST (`/workspace/invites`, admin-gated) and the accept POST (`/invites/accept`, public) are
both state-changing and both protected by the shipped double-submit CSRF middleware. A POST without a
valid CSRF token is refused before any invite is created or consumed and before any account is created.
- Measurable: a forged issuance POST ⇒ no invite created; a forged accept POST ⇒ no consume, no account,
  no password write.

### NFR-7 — Accessibility (deferred to DESIGN/implementation review)
The two new public-facing forms (the admin issuance form and the invitee set-password form) should meet
**WCAG 2.1 AA** for form labeling, focus order, and error-message association. The DISCUSS wave does NOT
specify or gate a compliance level — accessibility conformance is deferred to DESIGN and implementation
review (the forms reuse the shipped `InviteAcceptPage` template and the existing admin web tier, which
set the accessibility baseline). Flagged here so it is not silently dropped, not because it blocks DoR.

## Business rules

- **BR-1** Only a workspace **admin** can issue a member invite for that workspace (v1).
- **BR-2** A member invite grants the **member** role only (v1; admin-role invites deferred).
- **BR-3** An invite is single-use and time-bounded (7 days); consuming it is the only way it transitions
  to used, and it both creates the account and joins the workspace atomically.
- **BR-4** Accepting a member invite is the account-creation event for an invitee with no prior account.
- **BR-5** If the invitee's email already maps to an existing Foundry user, the v1 behavior is a
  non-enumerable refusal — no second account, no silent cross-workspace join (OD-1; revisit when
  multi-workspace-membership-via-invite is scoped).
- **BR-6** A refusal (acceptance or issuance) must never disclose the existence of any
  account/workspace/invite/email.

## Risk assessment (surfaced, not managed)

| Risk | Category | Probability | Impact | Mitigation |
|------|----------|-------------|--------|------------|
| New `create_member_and_consume` tx must create user+membership+consume atomically and race-safely | Technical | Medium | High | Reuse the shipped 0-or-1-row consume guard; @property concurrency scenario; DESIGN owns SQL. |
| Email-already-a-user behavior contested (OD-1) | Business | Medium | Medium | Recommend simplest coherent v1 (non-enumerable refusal); flagged for ratification; defers multi-membership. |
| Unique-constraint vs non-enumerable refusal interplay (email collision must not leak via a 500/constraint error) | Technical | Medium | High | The collision must surface as the uniform refusal, NOT a DB error page; DESIGN + DISTILL assert this. |
| Member could inadvertently get admin-gated surfaces | Technical | Low | High | Membership role MUST be `member`; AC asserts the new member gets 404 on `/workspace/invites`. |
| Refusal arms diverge over time (regression) | Technical | Medium | High | Byte-identity litmus (revert-reds-it), reused from the shipped flow, extended to A-E9 + issuance. |
| Issuance authz hole (non-admin issues) | Security | Low | High | `is_workspace_admin` on BOTH GET and POST; non-enumerable 404; @property/AC coverage. |

## Glossary (ubiquitous language)

- **Member invite** — an `invites` row created by a workspace admin for a prospective member's email.
- **Issue** — the admin act of creating a member invite and emitting its signed link.
- **Accept** — the invitee act of setting a password via the link, which creates the account, joins the
  workspace as a member, and consumes the invite — atomically.
- **Consume** — to mark an invite used, atomically and exactly once.
- **Member** — a workspace participant with the `member` role (not `admin`).
- **Uniform refusal** — the single non-enumerable page shown for every invalid-link / email-collision reason.
- **Non-enumerable issuance** — the issuance surface returns a generic 404 to any non-admin, hiding its existence.
