<!-- markdownlint-disable MD024 -->

# User Stories — invite-accept-flow

## System Constraints (cross-cutting — apply to every story)

- The accept page is a **public (signed-out-accessible)** route — NOT behind the instance-admin gate
  (the invitee is not signed in yet). The state-changing POST is still CSRF-protected (NFR-6).
- The signed `InviteToken` (HMAC binds `invite_id`\|\|`expires_at`), the `invites` row, the
  `hash_password` (argon2id) primitive, the session layer, and `resolve_active_workspace` are all
  **shipped seams reused verbatim** — see `requirements.md` grounding table and `shared-artifacts-registry.md`.
- Consume (single-use) and password write occur in the **same transaction** (NFR-2).
- Scope is **first-admin invites only** (v1, user-ratified).
- **JTBD traceability**: JTBD was skipped per config (one clear job — "claim my account and get in").
  Every story below is user-visible (NOT `infrastructure-only`) and carries an explicit
  `job_id: claim-my-account-and-sign-in` in its metadata line. This is a descriptive job identifier;
  promoting it to a `docs/product/jobs.yaml` entry is tracked as Open decision OD-3 (a documentation
  formality, not a behavior change).

---

## US-01: Accept a valid invite and get signed in (Walking Skeleton)

`job_id: claim-my-account-and-sign-in`

### Elevator Pitch
- **Before**: Priya Nair clicks her invite link and hits a dead URL — she cannot sign in at all.
- **After**: Priya opens `GET /invites/accept?id=…&sig=…`, sets a password, and is dropped straight
  onto her "Northwind" workspace dashboard, signed in.
- **Decision enabled**: Priya can decide her own password and start working in her workspace
  immediately — without waiting on her instance admin for any further step.

### Problem
Priya Nair is the first-admin of the freshly provisioned "Northwind" workspace. Her account was created
with a generated password hash she has never seen. She finds it impossible to get in: the only artifact
she has is an invite link, and that link goes nowhere — clicking it does nothing useful.

### Who
- First-admin of a just-provisioned workspace | received an invite link out-of-band (email or pasted by
  the super-admin) | has never signed into Foundry | motivated to start using her workspace now.

### Solution
A `GET /invites/accept` that verifies the signed token and renders a set-password form naming the
workspace, and a `POST /invites/accept` that atomically consumes the invite, saves the chosen password
(argon2id, reused), establishes a session, and redirects onto the workspace — no separate login step.

### Domain Examples
1. **Happy path** — Priya Nair opens `https://foundry.northwind.example/invites/accept?id=018f…c3&sig=Yk9…wQ`
   (issued 2 hours ago, valid 7 days), sees "Set your password to join Northwind", enters
   `river-stone-lantern-92`, submits, and lands on the Northwind dashboard signed in as
   `priya.nair@northwind.example`.
2. **Edge: near-fresh link** — Dan Okoro provisions "Westgate" and immediately pastes Marcus Liu the
   link; Marcus opens it 30 seconds later and accepts successfully.
3. **Boundary: just inside expiry** — Priya opens her link 6 days 23 hours after issue (still
   unexpired); the set-password form renders and she accepts successfully.

### UAT Scenarios (BDD)
#### Scenario: First-admin sees a set-password form for a live invite
Given Priya Nair was provisioned the "Northwind" workspace with a live invite (issued 2 hours ago)
When Priya opens her invite link
Then she sees a set-password form naming the "Northwind" workspace
And no password has been set on her account yet

#### Scenario: Setting a valid password signs the admin in on her workspace
Given Priya has opened her valid invite for "Northwind"
When she submits "river-stone-lantern-92" and confirms it
Then she is signed in without a separate login step
And she lands on the "Northwind" workspace dashboard
And she sees no data from any other workspace

#### Scenario: A near-fresh invite accepts immediately
Given Dan Okoro provisioned "Westgate" and issued Marcus Liu an invite 30 seconds ago
When Marcus opens his invite link and sets a valid password
Then Marcus is signed in on the "Westgate" workspace

#### Scenario: An invite just inside its expiry window still works
Given Priya's invite was issued 6 days and 23 hours ago and has not been used
When Priya opens her invite link and sets a valid password
Then she is signed in on the "Northwind" workspace

### Acceptance Criteria
- [ ] GET on a live, valid, unused invite renders a set-password form that names the workspace.
- [ ] No state is mutated on GET (invite remains unconsumed; no password written).
- [ ] POST with a valid password consumes the invite, writes the password (argon2id), establishes a
      session, and 302-redirects to `/`.
- [ ] The landed workspace equals `invites.workspace_id`; the admin sees only that tenant's data.
- [ ] The admin reaches the workspace with no separate sign-in step.

### Outcome KPIs
- **Who**: provisioned first-admins
- **Does what**: complete the accept flow and reach their workspace signed in
- **By how much**: 90%+ of issued first-admin invites result in a signed-in landing within 7 days
- **Measured by**: ratio of `invites` consumed-with-session to invites issued (provisioning + accept telemetry)
- **Baseline**: 0% (the link is dead today — no first-admin can sign in via it)

### Technical Notes
- Reuses `InviteToken::verify`, `hash_password`, session layer, `resolve_active_workspace`.
- New: GET/POST `/invites/accept` handlers (public layer in `build_router`), a set-password template,
  and a `consume_invite` store transaction.
- Depends on OD-1 (single-use column) being resolved in DESIGN.

---

## US-02: Refuse invalid links safely (single-use, expiry, non-enumerable)

`job_id: claim-my-account-and-sign-in`

### Elevator Pitch
- **Before**: there is no accept route, so there is no defined behavior for expired, reused, or
  tampered links — and the shipped bootstrap flow that does exist leaks distinct "already used" /
  "expired" / "not found" messages (an enumeration oracle).
- **After**: every bad link — expired, already-used, tampered, or unknown — shows one calm, identical
  page: "This invite is no longer valid… ask your instance administrator to re-issue."
- **Decision enabled**: a recipient of a bad link knows exactly what to do next (ask for a re-issue)
  while an attacker probing links learns nothing about who or what exists.

### Problem
Invite links get forwarded, re-clicked, and sat on past expiry. An attacker may probe random
`/invites/accept?id=…` URLs. Priya (or an attacker) who hits a non-live link must get a response that is
honest and actionable for Priya but reveals nothing about whether any account or workspace exists — and
a link must never be consumable twice.

### Who
- Any recipient of an expired/used/forwarded first-admin link (legitimate, e.g. Priya re-clicking) |
  AND "Malicious Mike" probing or tampering with invite URLs to enumerate accounts/workspaces.

### Solution
A single uniform, non-enumerable refusal page rendered for every invalid-link reason (expired,
already-used, invalid/tampered signature, unknown id), byte-identical across reasons; and an atomic,
race-safe single-use consume so a link is usable exactly once.

### Domain Examples
1. **Happy path (security working)** — "Malicious Mike" requests
   `/invites/accept?id=00000000-0000-0000-0000-000000000000&sig=AAAA`; he sees the same "invite is no
   longer valid" page Priya would see for an expired link — he cannot tell the id never existed.
2. **Edge: re-click after success** — Priya successfully accepted yesterday; today she clicks the same
   bookmarked link and sees the uniform refusal (no "you already did this" hint).
3. **Error/boundary: concurrent double-submit** — Priya double-clicks "Set password & sign in"; two
   POSTs race; exactly one consumes the invite and signs her in, the other returns the uniform refusal,
   and the invite is recorded used exactly once.

### UAT Scenarios (BDD)
#### Scenario: An expired link is refused without leaking existence
Given Priya's invite expired 1 day ago
When Priya opens her invite link
Then she sees the "invite is no longer valid" page advising her to ask her instance admin to re-issue
And the page reveals nothing about whether any account or workspace exists

#### Scenario: A tampered signature is refused identically to an expired link
Given Priya's invite is live but the signature in the URL has been altered by one character
When Priya opens the tampered link
Then she sees the "invite is no longer valid" page
And its body is byte-identical to the expired-link refusal

#### Scenario: An unknown invite id is refused identically to every other reason
Given an invite id that was never issued
When someone opens an accept link with that id
Then they see the "invite is no longer valid" page
And its body is byte-identical to the expired-link and tampered-link refusals

#### Scenario: A consumed invite cannot be used again
Given Priya already set her password via her invite yesterday
When Priya opens the same link today
Then she sees the "invite is no longer valid" page
And no new password is set and no session is created

#### Scenario: Concurrent accepts consume the invite exactly once
Given Priya's invite is live
When two accept submissions for the same invite arrive concurrently
Then exactly one sets the password and signs her in
And the other receives the "invite is no longer valid" page
And the invite is recorded as used exactly once

### Acceptance Criteria
- [ ] Expired, already-used, tampered-signature, and unknown-id links all render the SAME refusal body.
- [ ] The refusal body and status are byte-identical across all four reasons (a litmus REDs if any arm diverges).
- [ ] The refusal page discloses no account/workspace/invite existence and advises asking the admin to re-issue.
- [ ] A link is consumable exactly once; re-opening a consumed link is refused (no password, no session).
- [ ] Under concurrent accepts of one invite, exactly one succeeds and the invite is used exactly once.
- [ ] Expiry is enforced on both GET (liveness) and inside the consume transaction (`expires_at > now`).

### Outcome KPIs
- **Who**: anyone (legitimate or hostile) who opens a non-live invite link
- **Does what**: receives a uniform non-enumerable refusal AND cannot consume an invite twice
- **By how much**: 100% byte-identical refusals across reasons; 0 successful double-consumes (guardrail)
- **Measured by**: refusal-arm byte-identity litmus + concurrency property test in the acceptance suite
- **Baseline**: undefined behavior today (no accept route); the only comparable shipped flow (bootstrap claim) IS enumerable

### Technical Notes
- Atomic guarded consume (NFR-2); uniform refusal page (NFR-3); CSRF on POST (NFR-6); no token/secret
  in logs (NFR-5).
- Deliberately diverges from the shipped bootstrap claim flow's distinct messages (`bootstrap.rs:124-139`).
- Depends on US-01 (the consume path) and OD-1 (single-use column).

---

## US-03: Correct password mistakes without losing the invite

`job_id: claim-my-account-and-sign-in`

### Elevator Pitch
- **Before**: (no accept route exists) — and a naive implementation might consume the invite on a
  rejected password, stranding the admin with a dead link after a typo.
- **After**: Priya enters too-short a password (or a mismatched confirmation), sees a gentle inline
  error, and her invite is still live — she just fixes it and submits again.
- **Decision enabled**: Priya can safely experiment with a password and recover from a typo herself,
  without having to ask her admin to re-issue the invite.

### Problem
First-admins fumble passwords — too short, or the confirmation does not match. If a rejected attempt
consumed the invite, Priya would be stranded with a dead link and forced to ask for a re-issue. She
needs to correct the mistake inline and retry on the same live invite.

### Who
- A provisioned first-admin entering her password for the first time | likely to mistype or pick a weak
  password | "Careless Cathy" who does not read the requirement first.

### Solution
On a weak or mismatched password, re-render the set-password form with a clear inline error and the
invite **untouched** (not consumed) so the admin can retry.

### Domain Examples
1. **Happy path (recovery)** — Priya enters `pizza` (5 chars, below the 12-char policy); she sees
   "Password must be at least 12 characters", the form re-renders, she enters `river-stone-lantern-92`,
   and accepts successfully — same invite, never re-issued.
2. **Edge: mismatch** — Marcus Liu enters `correct-horse-battery` in new but `correct-horse-bttery` in
   confirm; he sees "Passwords do not match"; he fixes the confirmation and accepts.
3. **Boundary: at the threshold** — Priya enters exactly 12 characters `abcdefghijkl`; it is accepted
   (the policy is "at least 12").

### UAT Scenarios (BDD)
#### Scenario: A weak password is corrected inline and the invite stays live
Given Priya has opened her valid invite for "Northwind"
When she submits "pizza" (below the strength policy)
Then she sees an inline error stating the minimum password length
And her invite is still live and unconsumed
And no session is created

#### Scenario: A mismatched confirmation is corrected inline
Given Marcus Liu has opened his valid invite for "Westgate"
When his confirmation "correct-horse-bttery" does not match his new password "correct-horse-battery"
Then he sees an inline error that the passwords do not match
And his invite is still live and unconsumed

#### Scenario: A password exactly at the threshold is accepted
Given Priya has opened her valid invite for "Northwind"
When she submits a 12-character password and confirms it
Then her password is accepted and she is signed in

### Acceptance Criteria
- [ ] A password below the minimum length is refused inline with a clear message; the invite is NOT consumed.
- [ ] A confirmation that does not match the new password is refused inline; the invite is NOT consumed.
- [ ] A password at or above the minimum length is accepted.
- [ ] After an inline error, re-submitting a valid password on the same invite completes the accept.

### Outcome KPIs
- **Who**: first-admins who hit a password validation error on their first attempt
- **Does what**: recover and complete the accept on the same invite (no re-issue needed)
- **By how much**: 80%+ of admins who hit a password error go on to complete accept on the same invite
- **Measured by**: accept-completion rate among sessions that recorded a password validation error
- **Baseline**: 0% (no accept flow exists)

### Technical Notes
- Password strength policy NFR-4 (proposed min 12 chars — OD-2 ratification).
- Reuses `hash_password` only after validation passes.
- Depends on US-01 (set-password POST).
