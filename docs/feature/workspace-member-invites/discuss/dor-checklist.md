# Definition of Ready — workspace-member-invites

9-item hard gate. Each item must PASS with evidence before DESIGN handoff.

## US-01 — Issue a member invite (admin-gated)

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | "Dana Reyes, admin of Northwind, has no way to invite a plain member to her own workspace; only the instance super-admin can provision." |
| 2 | User/persona with specific characteristics | PASS | Workspace admin of an existing workspace, no instance super-admin powers, wants to onboard a teammate now. |
| 3 | 3+ domain examples with real data | PASS | Dana -> sam.okafor@northwind.example; SMTP-down paste-only; second invite to same email. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 3 scenarios (send + link, email-fails-link-still-shown, second invite). |
| 5 | AC derived from UAT | PASS | AC-01.1..01.5, each traced to FR-1/FR-2/NFR-1. |
| 6 | Right-sized (1-3 days, 3-7 scenarios) | PASS | Thin mirror of shipped `create_invite` + admin gate; 3 scenarios; ~0.5-1 day. |
| 7 | Technical notes: constraints/dependencies | PASS | Mirrors create_invite; adds is_workspace_admin gate; reuses insert_invite/InviteToken/email/CSRF. |
| 8 | Dependencies resolved or tracked | PASS | All shipped seams available; US-03 covers the non-admin refusal; no blockers. |
| 9 | Outcome KPIs with measurable targets | PASS | KPI-5 (95% issuance produces valid link), baseline 0%. |

**US-01 DoR: PASSED**

## US-02 — Accept a member invite and join (creates the account)

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | "Sam Okafor has no account; the existing accept flow assumes the user exists, so he cannot join." |
| 2 | User/persona with specific characteristics | PASS | Invitee with no Foundry account, link received out-of-band, becomes a member (not admin). |
| 3 | 3+ domain examples with real data | PASS | Sam (meadow-copper-violin-71); Priya Shah near-fresh; 6d23h boundary. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 4 scenarios (form renders, valid accept creates+joins+signs-in, near-fresh, near-expiry). |
| 5 | AC derived from UAT | PASS | AC-02.1..02.7, each traced to FR-4/5/6 + NFR-2. |
| 6 | Right-sized (1-3 days, 3-7 scenarios) | PASS | One new store tx + reused GET handler; 4 scenarios; ~1.5 days (the riskiest slice). |
| 7 | Technical notes: constraints/dependencies | PASS | NEW create_member_and_consume (create user + member membership + consume + password, atomic); reuses accept GET, policy, hash, session, resolve_active_workspace. |
| 8 | Dependencies resolved or tracked | PASS | Depends on US-01 (an invite) + OD-1 (email-already-a-user shapes the tx) — tracked as DESIGN input. |
| 9 | Outcome KPIs with measurable targets | PASS | KPI-1 (90% activation within 7d), baseline 0%. |

**US-02 DoR: PASSED**

## US-03 — Keep invites safe and honest (non-enumerable + single-use, release gate)

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | Probed issuance must be invisible to non-admins; bad/colliding accept links honest for invitee yet non-enumerable; never two accounts / twice-consumed. |
| 2 | User/persona with specific characteristics | PASS | Admin issuer; Malicious Mike (member/signed-out) probing both surfaces; Sam re-clicking; email-collision invitee. |
| 3 | 3+ domain examples with real data | PASS | Mike's generic-404 issuance + all-zero accept id; email-already-a-user (existing.user@northwind.example); double-click concurrency. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 6 scenarios (non-admin issuance, expired, tampered/unknown/used identical, email-collision, concurrency, CSRF). |
| 5 | AC derived from UAT | PASS | AC-03.1..03.10, each traced to NFR-1/2/3/5/6. |
| 6 | Right-sized (1-3 days, 3-7 scenarios) | PASS | 6 scenarios; mostly reuses shipped uniform refusal + atomic consume; new arms = email-collision + issuance gate; ~1 day. |
| 7 | Technical notes: constraints/dependencies | PASS | Reuses invite_refusal_page() + not_found() gate; email collision -> uniform refusal (not 500); CSRF both POSTs; no-secret logging. |
| 8 | Dependencies tracked | PASS | Depends on US-01 + US-02; OD-1 refusal arm. |
| 9 | Outcome KPIs | PASS | KPI-2 (100% byte-identical refusals, both surfaces, guardrail) + KPI-3 (0 double-creates/consumes, guardrail). |

**US-03 DoR: PASSED**

## US-04 — Correct mistakes without losing the invite (inline recovery)

| # | DoR Item | Status | Evidence |
|---|----------|--------|----------|
| 1 | Problem statement clear, domain language | PASS | A rejected password must not strand the invitee or create a half account; a blank email must not create a junk invite. |
| 2 | User/persona with specific characteristics | PASS | First-time password-setter (Careless Cathy); admin mistyping/blanking the email. |
| 3 | 3+ domain examples with real data | PASS | Sam pizza->meadow-copper-violin-71; Priya mismatch; Dana blank email. |
| 4 | UAT in Given/When/Then (3-7) | PASS | 4 scenarios (weak pwd, mismatch, blank email, valid retry). |
| 5 | AC derived from UAT | PASS | AC-04.1..04.5, traced to FR-3/FR-8/NFR-4. |
| 6 | Right-sized | PASS | 4 scenarios; reuses shipped inline-recovery path + a small issuance email check; <1 day. |
| 7 | Technical notes | PASS | Reuses re_render_with_error + check_password_policy (min-12); hash/tx only after validation; new issuance email validation. |
| 8 | Dependencies tracked | PASS | Depends on US-01 (email validation) + US-02 (set-password POST). |
| 9 | Outcome KPIs | PASS | KPI-4 (80% recovery-completion), baseline 0%. |

**US-04 DoR: PASSED**

---

## Overall DoR: PASSED (pending peer-review gate)

All four stories pass all 9 items. Two open decisions are **DESIGN-wave inputs**, not DoR blockers
(requirements are written solution-neutrally and the decisions are explicitly tracked):
- **OD-1** (email-already-a-user behavior) — recommended v1: non-enumerable refusal, defer
  multi-workspace-membership-via-invite. Shapes the `create_member_and_consume` tx.
- **OD-2** (member membership-role value / surface naming) — recommended: role `'member'` (mirrors the
  shipped `'admin'` seed), issuance at `/workspace/invites`. A naming/values choice DESIGN ratifies.
- **OD-3** (promote the two `job_id`s to `docs/product/jobs.yaml`) — documentation formality; no
  `jobs.yaml` exists in the repo today.

### Peer review (nw-product-owner-reviewer) — gate

Verdict: **conditionally_approved** — 0 critical, 1 high, 2 medium; all documentation refinements (no
scope or logic change). All three conditions addressed in this revision pass:

1. **(high)** `public_url` now documented as a shared artifact (`shared-artifacts-registry.md`) — single
   source `AppState.public_url`, the value the shipped `create_invite` already uses.
2. **(medium)** "Alternatives considered (constraint rationale)" section added to `requirements.md`
   (7d expiry, web-only issuance, member-role-only, non-enumerable refusal reuse, OD-1).
3. **(high)** NFR-7 (Accessibility / WCAG 2.1 AA, deferred to DESIGN) added to `requirements.md`.

Dimension 0 (Elevator Pitch) PASSED for all 4 stories; JTBD traceability PASSED (every story has a
`job_id`); zero LeanUX anti-patterns; both non-enumerable invariants (accept refusal incl. email-collision
arm, issuance 404) verified. **DoR gate: PASSED** post-revision.
