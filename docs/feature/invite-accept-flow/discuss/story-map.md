# Story Map: invite-accept-flow

## User: Priya Nair — first-admin of a freshly provisioned workspace (never signed in before)
## Goal: claim my account from the invite link and get into my workspace

## Scope (v1, user-ratified)

**In scope**: the **first-admin invite** minted by `provision_workspace` only.
**Out of scope** (explicit): general workspace-member invites (later feature); a CLI-native
`foundry invite accept` TUI (the emitted link is a web URL — making the web route live fixes the dead
link for BOTH the CLI and web emit sites at once).

## Backbone

| Receive invite | Open & verify link | Set password | Get signed in |
|----------------|--------------------|--------------|---------------|
| Click emitted link (web URL from CLI or web provision) | GET verifies signed token + liveness, renders set-password form | POST consumes invite (single-use, atomic) + saves password | Session established, lands on own workspace |
| (link arrives via email or pasted by admin — shipped emit) | Invalid/expired/used/tampered -> uniform non-enumerable refusal | Weak / mismatched password -> inline error, invite stays live | Tenant-isolated: sees only own workspace |
| | | CSRF-protected POST (public route, signed-out caller) | Concurrent accepts -> exactly one wins |

---

### Walking Skeleton (the accept happy-path — the thin end-to-end slice)

The single minimum task from each backbone activity that makes the dead link live end-to-end:

- **Receive invite**: (already shipped — the link is emitted; no new work)
- **Open & verify link**: GET `/invites/accept?id&sig` verifies signature + liveness, renders set-password form for a live valid invite.
- **Set password**: POST `/invites/accept` atomically consumes the invite (single-use) and writes the chosen password via shipped `hash_password`.
- **Get signed in**: establish session, 302 to `/`, land on the workspace via `resolve_active_workspace`.

This is **US-01** below. It is the riskiest assumption AND the highest-value slice: it proves the
entire credential-establishment vertical works end-to-end. Everything else hardens or refines it.

### Release 1: "The link works" — first-admins can sign in end-to-end

- **US-01 Accept a valid invite and get signed in** (the walking skeleton)
- Target outcome: a provisioned first-admin who clicks the link ends up signed in on their workspace
  with zero operator intervention. KPI: first-admin activation rate (see outcome-kpis.md, KPI-1).

### Release 2: "Invalid links are safe and honest" — the security crux

- **US-02 Refuse invalid / expired / used / tampered links non-enumerably** (uniform refusal, single-use enforcement)
- Target outcome: every bad link is refused with one calm, identical, non-enumerable message; no
  account/workspace existence leak; single-use is enforced atomically (no double-consume, race-safe).
  KPI: zero enumeration oracle (guardrail), 100% byte-identical refusals (see KPI-2, KPI-3).

> Note: US-02's single-use guard is technically exercised by the skeleton's consume, but its FULL
> behavior (uniform refusal across all four reasons + the concurrency race) is its own demonstrable
> security slice and carries the security NFRs. It is P2 because the happy path must work first, but
> it is a release gate — the feature does NOT ship without it.

### Release 3: "Mistakes don't lose momentum" — recoverable input errors

- **US-03 Correct weak or mismatched passwords inline without losing the invite**
- Target outcome: a first-admin who fumbles the password is gently corrected and can retry on the
  same live invite (the invite is NOT consumed on a rejected password). KPI: accept completion rate
  among admins who hit a password error (see KPI-4).

---

## Priority Rationale

1. **US-01 (Walking Skeleton, P1)** — validates the core assumption that the dead link can be made live
   end-to-end across the shipped seams (InviteToken, invites row, hash_password, session,
   resolve_active_workspace). Highest value: it is literally the feature's reason to exist. Nothing
   else matters if this does not work.
2. **US-02 (Security crux, P2 — release gate)** — the riskiest *quality* assumption. Token lifecycle
   (expire + single-use) and non-enumerable refusal are the security crux the user flagged for
   first-class treatment. Ordered after US-01 because you cannot harden a flow that does not yet
   exist; but the feature MUST NOT ship without it — it carries the security NFRs.
3. **US-03 (Inline recovery, P3)** — refinement. Real value (keeps momentum, avoids re-issue churn)
   but the flow is usable without it (a weak-password rejection that DID consume the invite would be
   bad — so US-03's "invite stays live" guarantee is partly load-bearing and is cross-referenced in
   US-01/US-02 AC). Lowest risk, lowest effort, sequenced last.

All three stories trace to outcome KPIs (no orphans). Slicing is by user outcome (get in / be safe /
recover), NOT by technical layer.
