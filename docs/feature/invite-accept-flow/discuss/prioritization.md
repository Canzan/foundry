# Prioritization: invite-accept-flow

## Release Priority

| Priority | Release | Target Outcome | KPI | Rationale |
|----------|---------|----------------|-----|-----------|
| 1 | Walking Skeleton (US-01) | A provisioned first-admin clicks the link and ends up signed in on their workspace | KPI-1 first-admin activation rate | Validates the core assumption: the dead link can be made live end-to-end across shipped seams. The feature's reason to exist. |
| 2 | Security crux (US-02) — release gate | Every invalid/expired/used/tampered link is refused identically and non-enumerably; single-use enforced atomically | KPI-2 (zero enumeration oracle, guardrail), KPI-3 (single-use integrity) | Riskiest quality assumption; the user-flagged security crux. Must ship WITH the feature; ordered after US-01 because you cannot harden a flow that does not exist. |
| 3 | Inline recovery (US-03) | A first-admin who fumbles the password is corrected inline and retries on the same live invite | KPI-4 accept completion after a password error | Refinement; lowest risk/effort. The "invite stays live on rejected password" guarantee is partly load-bearing (cross-referenced from US-01/US-02). |

## Prioritization Scores (Value x Urgency / Effort, 1-5)

| Story | Value | Urgency | Effort | Score | Notes |
|-------|-------|---------|--------|-------|-------|
| US-01 | 5 | 5 | 2 | 12.5 | Thin adapter over shipped seams; only new artifact is the consume_invite TX + GET/POST handlers + a template. Walking skeleton tie-break wins regardless. |
| US-02 | 5 | 5 | 2 | 12.5 | The security crux; uniform refusal + atomic single-use. Tie-break: riskiest assumption -> ranks just below the skeleton. |
| US-03 | 3 | 2 | 1 | 6.0 | Inline validation only; small. |

## Backlog Suggestions

| Story | Release | Priority | Outcome Link | Dependencies |
|-------|---------|----------|--------------|--------------|
| US-01 | WS | P1 | KPI-1 | Shipped: InviteToken, invites row, hash_password, session, resolve_active_workspace. Open: confirm invites.used_at column. |
| US-02 | R2 (gate) | P2 | KPI-2, KPI-3 | US-01 (consume path); the uniform-refusal page |
| US-03 | R3 | P3 | KPI-4 | US-01 (set-password POST); password strength policy (NFR-4) |

## Scope Assessment (Elephant Carpaccio Gate)

**PASS — 3 stories, 1 bounded context (web auth / foundry-app), estimated ~3-4 days total.**

Oversized signals checked: stories 3 (≤10 OK) | bounded contexts 1 (≤3 OK) | walking skeleton
integration points: InviteToken verify, invites row consume, hash_password, session, resolve_active_workspace
= 5 reused seams + 1 new store TX (within reason for a single auth vertical) | effort ~3-4 days (< 2 weeks) |
single user outcome (claim account & get in). Right-sized; no split needed. This is overwhelmingly
EXTEND over shipped, mutation-hardened seams — the only genuinely new logic is the `consume_invite`
transaction and the two public route handlers.
