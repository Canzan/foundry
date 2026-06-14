# Acceptance Criteria — invite-accept-flow

All criteria are observable and testable. Given/When/Then scenarios live in
`journey-invite-accept.feature` and per-story in `user-stories.md`; this file is the consolidated,
traceable AC index for DISTILL.

## US-01 — Accept a valid invite and get signed in (Walking Skeleton)

| AC-ID | Criterion | Verifies |
|-------|-----------|----------|
| AC-01.1 | GET on a live/valid/unused invite renders a set-password form naming the workspace | FR-1 |
| AC-01.2 | GET mutates no state (invite stays unconsumed; no password written) | FR-1 (non-committal) |
| AC-01.3 | POST with a valid password consumes the invite, writes the argon2id hash, establishes a session, 302 -> `/` | FR-2 |
| AC-01.4 | The landed workspace == `invites.workspace_id`; only that tenant's data is visible | FR-3 |
| AC-01.5 | The admin reaches the workspace with no separate sign-in step | FR-2, decision 3 |
| AC-01.6 | A link opened at `expires_at - 1s` is accepted | NFR-1 |

## US-02 — Refuse invalid links safely (single-use, expiry, non-enumerable)

| AC-ID | Criterion | Verifies |
|-------|-----------|----------|
| AC-02.1 | Expired, already-used, tampered-sig, and unknown-id links all render the SAME refusal body | NFR-3 |
| AC-02.2 | The refusal body AND status code are byte-identical across all four reasons (litmus REDs on divergence) | NFR-3 |
| AC-02.3 | The refusal page discloses no account/workspace/invite existence; advises asking admin to re-issue | NFR-3, FR-4 |
| AC-02.4 | A link opened at `expires_at + 1s` is refused | NFR-1 |
| AC-02.5 | A consumed invite re-opened is refused; no new password, no session | NFR-2 |
| AC-02.6 | Two concurrent accepts of one invite: exactly one succeeds; invite used exactly once | NFR-2 |
| AC-02.7 | Expiry enforced on GET liveness AND inside the consume transaction | NFR-1, NFR-2 |
| AC-02.8 | A POST without a valid CSRF token is refused; no consume, no password write | NFR-6 |
| AC-02.9 | No `sig` value and no password appears in application logs after a full cycle | NFR-5 |

## US-03 — Correct password mistakes without losing the invite

| AC-ID | Criterion | Verifies |
|-------|-----------|----------|
| AC-03.1 | A password below the minimum length is refused inline; invite NOT consumed | FR-5, NFR-4 |
| AC-03.2 | A mismatched confirmation is refused inline; invite NOT consumed | FR-5 |
| AC-03.3 | A password at/above the minimum length is accepted | NFR-4 |
| AC-03.4 | After an inline error, a valid retry on the same invite completes the accept | FR-5 |

## Property-shaped criteria (tag @property for DISTILL)

- **@property non-enumerability**: for the set {expired, used, tampered, unknown-id}, the user-visible
  response is byte-identical (body + status). (AC-02.1, AC-02.2)
- **@property single-use under concurrency**: for any number of concurrent accepts of one live invite,
  exactly one succeeds and the consumed-count is exactly one. (AC-02.6)
- **@property no-secret-leakage**: across any accept/refusal sequence, logs contain neither the `sig`
  nor any submitted password. (AC-02.9)

## Traceability

Every AC traces to a functional requirement (FR-1..5) or security NFR (NFR-1..6) in `requirements.md`,
and every story traces to an outcome KPI in `outcome-kpis.md`. No orphan AC.
