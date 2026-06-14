# Requirements — invite-accept-flow

## Context

When a super-admin provisions a workspace (`foundry doctor provision-workspace` on the CLI, or the
`POST /admin/instance/workspaces` browser form), the use-case seeds a first-admin user with a
generated password hash the operator never sees, inserts an `invites` row, and emits a signed link:

```
{public_url}/invites/accept?id=<invite_id>&sig=<signature>
```

That link is a **dead URL today** on BOTH surfaces — there is no route, no consume-invite transaction,
and no password-set handler (confirmed: `build_router` has no `/invites/accept` entry; `signin.rs` has
no accept path; the store has no consume function). The provisioned admin therefore **cannot sign in
at all**. This is the gating gap called out in `docs/evolution/2026-06-12-multi-workspace-provisioning.md`
and `2026-06-13-web-provisioning-flow.md` (ADR-005 deferred it as the highest-value follow-up).

This feature makes the link live: **verify the signed token -> render a set-password page -> consume
the invite atomically (single-use) -> set the password -> establish a signed-in session landing on the
workspace.** Auto sign-in, no separate login step (user-ratified).

## Scope (v1, user-ratified — do not re-litigate)

- **In scope**: the **first-admin invite** minted by `provision_workspace` only.
- **Out of scope**: general workspace-member invites (later feature); a CLI-native `foundry invite
  accept` TUI (the link is a web URL — the web route fixes both emit sites).

## Brownfield grounding (shipped seams — reuse, do not reinvent)

| Seam | Location | Reuse |
|------|----------|-------|
| `InviteToken` (sign / verify) | `crates/foundry-auth/src/lib.rs:354-385` | Carries `invite_id` + `expires_at`; `signature` = HMAC(SESSION_SECRET, `invite_id`\|\|`expires_at`). DB row is the primary single-use control; HMAC is defense-in-depth. Reused verbatim for GET/POST verification. |
| `invites` table + `insert_invite` | `crates/foundry-store/src/lib.rs:491`, provision tx `:1254` | Columns today: `id, workspace_id, invitee_email, created_by, expires_at`. This feature CONSUMES that row (single-use) — it does not invent a new store. |
| `hash_password` (argon2id, OWASP) | `crates/foundry-auth/src/lib.rs:319` | Reused verbatim to set the chosen password. |
| Session + double-submit CSRF | `crates/foundry-app/src/{session,csrf}.rs` | Reused. The accept page is a NEW **public (signed-out-accessible)** route — NOT behind the instance-admin gate (the invitee isn't signed in yet). CSRF still applies to the POST. |
| `resolve_active_workspace` | store; used at `signin.rs:149` | Reused to resolve the landing workspace. |
| Route registration | `crates/foundry-app/src/lib.rs::build_router:234` | Where the new GET/POST `/invites/accept` register (public layer). |

### Two grounded findings the DESIGN wave must resolve

1. **No `used_at` / single-use column observed** on the `invites` table (the shipped schema is
   `id, workspace_id, invitee_email, created_by, expires_at`; `insert_invite` writes no consumed-marker,
   and no `consume_invite` store fn exists). The single-use guarantee (NFR-2) requires a consumed marker
   with an atomic guarded UPDATE. **Open decision OD-1** — confirm/add the column. Requirements are
   written solution-neutrally ("the invite is recorded as used exactly once") so DESIGN owns the column choice.
2. **The shipped bootstrap claim flow does NOT enforce a password-strength minimum** (`bootstrap.rs:149`
   hashes whatever is submitted; `signin.rs` has no min-length check). There is therefore **no existing
   strength policy to "reuse"** as-is — only the argon2id *hashing* is shipped. NFR-4 specifies a
   minimum policy to introduce; **Open decision OD-2** asks the user to ratify the threshold.

## Functional requirements

- **FR-1** GET `/invites/accept?id&sig` verifies the signed token (HMAC binds `id`\|\|`expires_at`) and
  the invite's liveness (unexpired AND unconsumed), then renders a set-password form naming the
  workspace. Verification is **non-committal** (no state mutation on GET).
- **FR-2** POST `/invites/accept` (carrying `id`, `sig`, new password, confirmation, `_csrf`)
  atomically: re-verifies the signature, consumes the invite (single-use), writes the chosen password
  hash onto the first-admin user row, establishes a session, and redirects to `/` (the workspace).
- **FR-3** A valid accept lands the admin on **their** workspace (`invites.workspace_id` via
  `resolve_active_workspace`), seeing only that tenant's data.
- **FR-4** An invalid / expired / already-used / tampered link renders a **uniform, non-enumerable**
  refusal page (FR/NFR-3) — same body for all reasons.
- **FR-5** A weak or mismatched password is corrected **inline**; the invite is **NOT consumed** and
  stays live for retry.

## Non-Functional Requirements (Security — first-class)

### NFR-1 — Token expiry (bounded link lifetime)
Invite links expire. **Default: 7 days** (matches the shipped `bootstrap.rs:244` `now + 7 days` and the
"valid for 7 days" copy in the invite email — keeping the lifetime consistent with the already-emitted
promise). Rationale for 7d: long enough for an admin to act on an out-of-band email/paste without
operator hand-holding; short enough to bound the window in which a leaked/forwarded link is dangerous,
and the expiry is HMAC-bound so it cannot be extended by tampering. Expiry is enforced on BOTH GET
(liveness) and inside the consume TX (`expires_at > now`), so a link cannot be consumed in the gap
between GET and POST.
- Measurable: a link opened at `expires_at + 1s` is refused (uniform refusal); a link opened at
  `expires_at - 1s` is accepted.

### NFR-2 — Single-use atomicity (no double-consume, race-safe)
An invite is consumable **exactly once**. Consume is an atomic guarded operation — conceptually
`UPDATE invites SET used = now WHERE id = $1 AND not-yet-used AND expires_at > now`, where 0 rows
updated => refuse. The password write and the consume occur in the **same transaction**; neither
happens without the other.
- Measurable: two concurrent POSTs for one invite => exactly one sets the password and signs in; the
  other gets the uniform refusal; the invite is recorded used exactly once. Re-opening a consumed link
  => uniform refusal, no new password, no session.

### NFR-3 — Non-enumerable refusals (no existence oracle)
Every invalid-link reason (expired, already-used, invalid/tampered signature, unknown id) produces a
**byte-identical** user-visible refusal page. The page reveals nothing about whether an account, user,
workspace, or invite exists. Responses differ ONLY in internal `tracing`, never in the observable body
(or status code — they must not become a 410-vs-404 oracle). This mirrors the shipped instance-admin
surface's uniform-404 posture, but NOTE it **diverges** from the shipped *bootstrap* claim flow, which
returns distinct "already used" / "expired" / "not found" messages (`bootstrap.rs:124-139`) — the
invite-accept flow deliberately does NOT, per the user-ratified non-enumerable requirement.
- Measurable: a falsifiability litmus REDs if any refusal arm's body diverges (the revert-reds-it
  pattern used by the shipped instance-admin 404).

### NFR-4 — Password strength (introduce a minimum; reuse argon2id hashing)
The chosen password must meet a minimum strength policy before it is accepted. **Proposed: at least 12
characters, no other composition rule** (length-first, aligned with current NIST 800-63B guidance —
favor length over arbitrary complexity). The argon2id **hashing** (`hash_password`, OWASP params) is
reused verbatim. A rejected password does not consume the invite (FR-5). **OD-2: ratify the threshold**
(shipped flows enforce none today, so this is a net-new policy, not a reuse).
- Measurable: a password below the threshold is refused inline; one at/above is accepted.

### NFR-5 — No token/secret leakage (logs, URLs-at-rest)
The invite `sig` and the chosen password must never be written to application logs. The `sig` appears
in the URL (unavoidable — it is the link), so refusal/consume logging keys on `invite_id` only, never
the signature or password; error logs for the accept handlers must not echo the query string or form
body. (The shipped instance-admin remediation already established "fail-closed authz-probe logging" —
the same discipline applies here.)
- Measurable: a log scan after a full accept + refusal cycle contains no `sig` value and no password.

### NFR-6 — Request-forgery protection on the public POST
The accept POST is a state-changing request on a public (signed-out) route; it is protected by the
shipped double-submit CSRF middleware. A POST without a valid CSRF token is refused before any consume
or password write.
- Measurable: a forged POST without a valid `_csrf` => refused, no invite consumed, no password written.

## Business rules

- **BR-1** Only the first-admin invite is acceptable in v1 (scope).
- **BR-2** An invite is single-use and time-bounded; consuming it is the only way it transitions to used.
- **BR-3** Setting the password is the credential-establishment event — it both writes the password and
  signs the user in, atomically tied to consuming the invite.
- **BR-4** Refusal must never disclose existence of any account/workspace/invite.

## Risk assessment (surfaced, not managed)

| Risk | Category | Probability | Impact | Mitigation |
|------|----------|-------------|--------|------------|
| `invites` table lacks a single-use column (OD-1) | Technical | Medium | High | DESIGN confirms/adds the consumed marker; requirements stay solution-neutral. |
| Password policy threshold contested (OD-2) | Business | Low | Low | Default proposed (12 chars); flagged for ratification before DELIVER. |
| Consume race under concurrency | Technical | Low | High | Atomic guarded UPDATE (NFR-2); @property concurrency scenario. |
| Refusal arms diverge over time (regression) | Technical | Medium | High | Byte-identity litmus (revert-reds-it), like the shipped 404. |
| Token replay after expiry via tampering | Technical | Low | High | Expiry HMAC-bound (InviteToken binds id\|\|expires_at) — already mutation-hardened. |

## Glossary (ubiquitous language)

- **Invite** — the `invites` row created at provisioning; the right to claim a specific first-admin account.
- **Accept** — the act of setting a password via the invite link, which consumes the invite and signs in.
- **Consume** — to mark an invite used, atomically and exactly once.
- **First-admin** — the initial administrator seeded for a workspace at provisioning.
- **Uniform refusal** — the single non-enumerable page shown for every invalid-link reason.
