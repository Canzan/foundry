# Acceptance Criteria — workspace-member-invites

All criteria are observable and testable. Given/When/Then scenarios live in
`journey-member-invite.feature` and per-story in `user-stories.md`; this file is the consolidated,
traceable AC index for DISTILL.

## US-01 — Issue a member invite (admin-gated)

| AC-ID | Criterion | Verifies |
|-------|-----------|----------|
| AC-01.1 | GET `/workspace/invites` renders a one-email-field form for a signed-in workspace admin | FR-1 |
| AC-01.2 | POST with a valid email creates an `invites` row (workspace = admin's active, invitee_email = typed, created_by = admin, expires_at = now + 7d) | FR-2, NFR-1 |
| AC-01.3 | The response shows the emitted `/invites/accept?id&sig` link | FR-2 |
| AC-01.4 | The link is also best-effort emailed; a send failure does not fail the request (link still shown) | FR-2 |
| AC-01.5 | The emitted signature verifies against the invite (`InviteToken::verify`) | FR-2 |

## US-02 — Accept a member invite and join (creates the account)

| AC-ID | Criterion | Verifies |
|-------|-----------|----------|
| AC-02.1 | GET on a live/valid/unused invite renders a set-password form naming the workspace ("join as a member") | FR-4 |
| AC-02.2 | GET mutates no state (no account created; invite unconsumed) | FR-4 (non-committal) |
| AC-02.3 | POST with a valid password creates the user (email = invites.invitee_email), adds a `member` membership, consumes the invite, writes the argon2id hash, establishes a session, 302/303 -> `/` — in ONE atomic tx | FR-5, NFR-2 |
| AC-02.4 | The new member lands on `invites.workspace_id` and sees only that tenant's data | FR-6 |
| AC-02.5 | The new member reaches the workspace with no separate sign-in step | FR-5 |
| AC-02.6 | The new member has `member` (not admin) privileges (cannot reach `/workspace/invites`) | FR-6, BR-2 |
| AC-02.7 | A link opened at `expires_at - 1s` is accepted | NFR-2 |

## US-03 — Keep invites safe and honest (non-enumerable + single-use, release gate)

| AC-ID | Criterion | Verifies |
|-------|-----------|----------|
| AC-03.1 | A non-admin or signed-out GET/POST to `/workspace/invites` returns a response byte-identical to a generic 404; no invite created | NFR-1 |
| AC-03.2 | Expired, already-used, tampered-sig, unknown-id, AND email-already-a-user accepts all render the SAME refusal body + status (litmus REDs on divergence) | NFR-3, FR-7 |
| AC-03.3 | The accept refusal discloses no account/workspace/invite/email existence; advises asking the admin to re-issue | NFR-3, FR-7 |
| AC-03.4 | A link opened at `expires_at + 1s` is refused | NFR-2 |
| AC-03.5 | An invite creates exactly one account and is consumable exactly once; re-opening a consumed link is refused (no second account, no session) | NFR-2 |
| AC-03.6 | Two concurrent accepts of one invite: exactly one succeeds; exactly one user and one membership exist | NFR-2 |
| AC-03.7 | Expiry enforced on GET liveness AND inside the consume transaction | NFR-2 |
| AC-03.8 | An email collision surfaces as the uniform refusal — never as a DB/constraint error page | NFR-3, BR-5 |
| AC-03.9 | A POST without a valid CSRF token (issuance OR accept) is refused; no invite created/consumed, no account | NFR-6 |
| AC-03.10 | No `sig` value and no password appears in application logs after a full cycle | NFR-5 |

## US-04 — Correct mistakes without losing the invite (inline recovery)

| AC-ID | Criterion | Verifies |
|-------|-----------|----------|
| AC-04.1 | A password below the minimum length is refused inline; invite NOT consumed; NO account created | FR-8, NFR-4 |
| AC-04.2 | A mismatched confirmation is refused inline; invite NOT consumed; no account | FR-8 |
| AC-04.3 | A blank/malformed email on the issuance form is refused inline; NO invite created | FR-3 |
| AC-04.4 | A password at/above the minimum length is accepted | NFR-4 |
| AC-04.5 | After an inline error, a valid retry on the same invite completes the join | FR-8 |

## Property-shaped criteria (tag @property for DISTILL)

- **@property non-enumerability (accept)**: for the set {expired, used, tampered, unknown-id,
  email-already-a-user}, the user-visible response is byte-identical (body + status). (AC-03.2, AC-03.3, AC-03.8)
- **@property non-enumerability (issuance)**: for {non-admin, signed-out}, the response is byte-identical
  to a generic 404. (AC-03.1)
- **@property single-use + single-create under concurrency**: for any number of concurrent accepts of one
  live invite, exactly one succeeds and exactly one user + one membership + one consumed-invite exist. (AC-03.6)
- **@property no-secret-leakage**: across any issue/accept/refusal sequence, logs contain neither the
  `sig` nor any submitted password. (AC-03.10)

## Traceability

Every AC traces to a functional requirement (FR-1..8) or security NFR (NFR-1..6) in `requirements.md`,
and every story traces to an outcome KPI in `outcome-kpis.md`. No orphan AC.
