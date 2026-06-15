# Prioritization: workspace-member-invites

## Release Priority

| Priority | Release | Target Outcome | KPI | Rationale |
|----------|---------|----------------|-----|-----------|
| 1 | Walking Skeleton (US-01 issuance + US-02 member-accept) | A workspace admin invites a teammate by email; the teammate goes from no-account to signed-in member by clicking one link | KPI-1 member-invite activation rate | Validates the core assumption: the shipped accept flow generalizes to a member who has no account (new `create_member_and_consume` tx). The feature's reason to exist. |
| 2 | Security crux (US-03) — release gate | Every invalid/expired/used/tampered link AND email-already-a-user is refused identically and non-enumerably; unauthorized issuance is a generic 404; single-use enforced atomically; account created exactly once | KPI-2 (byte-identical refusals, guardrail), KPI-3 (single-use/single-create integrity, guardrail) | Riskiest quality assumption; the user-flagged security crux, extended with the new issuance-authz and email-already-a-user arms. Must ship WITH the feature; ordered after the skeleton because you cannot harden a flow that does not exist. |
| 3 | Inline recovery (US-04) | An invitee who fumbles the password (and an admin who fumbles the email) is corrected inline and retries; no account/invite is wrongly created | KPI-4 accept completion after a password error | Refinement; lowest risk/effort. The "invite stays live / no account on rejected password" guarantee is partly load-bearing (cross-referenced from US-02/US-03). |

## Prioritization Scores (Value x Urgency / Effort, 1-5)

| Story | Value | Urgency | Effort | Score | Notes |
|-------|-------|---------|--------|-------|-------|
| US-01 | 5 | 5 | 1 | 25.0 | Issuance form: a thin mirror of shipped `create_invite` + the `is_workspace_admin`/`not_found()` gate. Smallest new surface. |
| US-02 | 5 | 5 | 3 | 8.3 | The riskiest NEW logic: `create_member_and_consume` (create user + member membership + consume, atomic). Walking-skeleton tie-break wins regardless. |
| US-03 | 5 | 5 | 2 | 12.5 | Security crux. Mostly reuses the shipped uniform refusal + atomic consume; new work is the email-already-a-user refusal arm + the issuance non-enumerable gate. |
| US-04 | 3 | 2 | 1 | 6.0 | Inline validation on both surfaces; small, reuses the shipped US-03 inline-recovery path. |

> Tie-break (per user-story-mapping skill): Walking Skeleton > Riskiest Assumption > Highest Value.
> US-01 and US-02 are the skeleton (both P1). US-02 is also the riskiest NEW assumption, so within the
> skeleton it is built immediately after its precondition US-01.

## Backlog Suggestions

| Story | Release | Priority | Outcome Link | Dependencies |
|-------|---------|----------|--------------|--------------|
| US-01 | WS | P1 | KPI-1 | Shipped: insert_invite, InviteToken, is_workspace_admin, CSRF, not_found() idiom, email seam. |
| US-02 | WS | P1 | KPI-1 | US-01 (an invite to accept); NEW `create_member_and_consume` tx; reused accept GET handler, hash_password, min-12 policy, session, resolve_active_workspace. OD-1 (email-already-a-user) shapes the tx. |
| US-03 | R2 (gate) | P2 | KPI-2, KPI-3 | US-01 + US-02; reused uniform `invite_refusal_page()`; the issuance gate; OD-1 refusal arm. |
| US-04 | R3 | P3 | KPI-4 | US-01 (email validation) + US-02 (set-password POST); reused US-03 inline-recovery path; min-12 policy. |

## Scope Assessment (Elephant Carpaccio Gate)

**PASS — 4 stories, 1 bounded context (web auth / foundry-app + foundry-store), estimated ~3-4 days total.**

Oversized signals checked: stories 4 (<=10 OK) | bounded contexts 1 (<=3 OK) | walking-skeleton
integration points: insert_invite, InviteToken, is_workspace_admin, accept GET handler, hash_password,
min-12 policy, session, resolve_active_workspace = ~8 reused seams + 2 new (the issuance handler pair and
the `create_member_and_consume` tx) | effort ~3-4 days (< 2 weeks) | single coherent user outcome
(invite a teammate / join a workspace). Right-sized; no split needed.

This is overwhelmingly EXTEND over the shipped, mutation-hardened `invite-accept-flow` and
`multi-workspace-provisioning` seams. The only genuinely new logic is (a) the admin-gated issuance handler
(a near-clone of `create_invite`) and (b) the `create_member_and_consume` store tx (the `provision_workspace`
seeding pattern, narrowed to one user + one member membership, fused with the shipped consume guard).
