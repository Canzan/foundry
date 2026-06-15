# Story Map: workspace-member-invites

## Users: Dana Reyes (workspace ADMIN, inviter) and Sam Okafor (invitee, no account yet)
## Goal: an admin invites a teammate by email; the teammate sets a password, gets an account, and joins the workspace as a member

## Scope (v1, recommended defaults)

**In scope**: member-invite ISSUANCE by a workspace admin; member-invite ACCEPTANCE that CREATES the
user + adds a member-role membership + sets password + auto-signs-in, atomically.
**Out of scope** (explicit, deferred follow-ups): inviting as the admin role (member role only); bulk
invites; invite revocation/resend; a CLI-native issuance command (the emitted link is a web URL — the
web route suffices); multi-workspace-membership-via-invite for an email that is already a user (OD-1).

## Backbone

| Issue invite (admin) | Receive invite | Open & verify link | Set password & join | Get signed in |
|----------------------|----------------|--------------------|---------------------|---------------|
| Admin opens admin-gated form, types email, sends | Invitee gets link (email or pasted) | GET verifies signed token + liveness, renders set-password form | POST creates user + member membership + consumes invite (one atomic tx) + saves password | Session established, lands on the workspace as a member |
| Non-admin/signed-out -> non-enumerable 404 | (best-effort email; link also shown to admin) | Invalid/expired/used/tampered -> uniform non-enumerable refusal | Weak/mismatched password -> inline error, invite stays live, no account | Tenant-isolated: sees only this workspace, member privileges |
| Blank/invalid email -> inline error, no invite | | | Email-already-a-user -> uniform refusal (OD-1) | Concurrent accepts -> exactly one account created |
| CSRF-protected POST | | | CSRF-protected public POST | |

---

### Walking Skeleton (the thin end-to-end slice — admin invites, member joins)

The single minimum task from each backbone activity that makes the end-to-end member-invite flow work:

- **Issue invite**: admin POSTs `/workspace/invites` with an email -> `insert_invite` + emit signed link (mirrors shipped `create_invite`, adds the `is_workspace_admin` gate).
- **Receive invite**: (no new work — email/paste of the emitted link, reused).
- **Open & verify link**: GET `/invites/accept?id&sig` verifies + renders the set-password form (reused verbatim from the shipped first-admin flow).
- **Set password & join**: POST `/invites/accept` runs the NEW `create_member_and_consume` tx — creates the user + member membership + consumes the invite, atomically.
- **Get signed in**: establish session, 303 to `/`, land on the workspace via `resolve_active_workspace` (reused).

This is **US-01 + US-02** below (issuance and member-accept are two halves of the one end-to-end slice;
they are split as separate stories because each is independently demonstrable and right-sized, but
together they are the skeleton). The riskiest NEW assumption is the create-user-in-the-consume-tx
(US-02); everything else is a thin adapter over shipped, mutation-hardened seams.

### Release 1: "An admin can invite a member end-to-end" — the skeleton

- **US-01 Issue a member invite (admin-gated)** — the issuance surface.
- **US-02 Accept a member invite and join (creates the account)** — the new atomic create+join+consume.
- Target outcome: a workspace admin can add a teammate without an operator/IT ticket; the teammate goes
  from no-account to signed-in member by clicking one link. KPI: member-invite activation rate (KPI-1).

### Release 2: "Invites are safe and honest" — the security crux (release gate)

- **US-03 Refuse invalid links and unauthorized issuance non-enumerably** — uniform acceptance refusal
  (expired/used/tampered/unknown + email-already-a-user), single-use atomicity under concurrency, and the
  non-enumerable issuance gate (non-admin/signed-out -> generic 404).
- Target outcome: every bad accept link and every unauthorized issuance attempt is refused identically
  and non-enumerably; single-use is enforced atomically; the account is created exactly once.
  KPI: 100% byte-identical refusals (guardrail KPI-2), 0 double-creates (guardrail KPI-3).

### Release 3: "Mistakes don't lose momentum" — recoverable input errors

- **US-04 Correct weak/mismatched passwords and blank emails inline** — inline recovery on both surfaces
  (invitee password mistakes leave the invite live and create no account; admin blank/invalid email
  creates no invite). KPI: accept completion after a password error (KPI-4).

---

## Priority Rationale

1. **US-01 + US-02 (Walking Skeleton, P1)** — validate the core assumption that the shipped accept flow
   generalizes to a member who has no account: issuance creates the row + link, and accept creates the
   user + member membership + consumes atomically. Highest value: this is the feature's reason to exist.
   US-02 carries the riskiest NEW logic (`create_member_and_consume`), so it is the riskiest-assumption
   slice; US-01 is its precondition (no invite to accept without issuance).
2. **US-03 (Security crux, P2 — release gate)** — the riskiest QUALITY assumption. Token lifecycle
   (expire + single-use), non-enumerable acceptance refusal (extended to the new email-already-a-user
   arm), and the non-enumerable issuance gate are the security crux flagged for first-class treatment.
   Ordered after the skeleton because you cannot harden a flow that does not exist; but the feature MUST
   NOT ship without it — it carries the security NFRs.
3. **US-04 (Inline recovery, P3)** — refinement. Real value (keeps both admin and invitee in flow, avoids
   re-issue churn) but the flow is usable without it. The "invite stays live / no account on a rejected
   password" guarantee is partly load-bearing and is cross-referenced from US-02/US-03 AC. Lowest
   risk/effort, sequenced last.

All four stories trace to outcome KPIs (no orphans). Slicing is by user outcome (invite / join / be safe
/ recover), NOT by technical layer — each release touches both the issuance and acceptance surfaces as
the outcome requires.
