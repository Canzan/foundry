<!-- markdownlint-disable MD024 -->

# User Stories — workspace-member-invites

## System Constraints (cross-cutting — apply to every story)

- The issuance surface (`/workspace/invites`) is **admin-gated** (`is_workspace_admin` on GET and POST);
  a non-admin or signed-out caller gets a **non-enumerable 404** (NFR-1). The accept surface
  (`/invites/accept`) is a **public (signed-out-accessible)** route (the invitee is not signed in — has
  no account yet). BOTH state-changing POSTs are CSRF-protected (NFR-6).
- The signed `InviteToken` (HMAC binds `invite_id`\|\|`expires_at`), the `invites` table (incl. the
  shipped `used_at`/`used_by` single-use markers), `insert_invite`, `is_workspace_admin`, the accept GET
  handler + `InviteAcceptPage` template + `invite_refusal_page()`, `hash_password` + `check_password_policy`
  (min-12), the session layer, and `resolve_active_workspace` are all **shipped seams reused verbatim** —
  see `requirements.md` grounding table and `shared-artifacts-registry.md`.
- The ONE genuinely-new store tx is `create_member_and_consume`: under the same atomic 0-or-1-row consume
  guard as the shipped `set_first_admin_password_and_consume`, it **creates the user + adds a member-role
  membership + consumes the invite + writes the password**, all in one transaction (NFR-2).
- Scope is **member role only** (v1, recommended default). Inviting as admin, bulk, revoke/resend, and
  CLI issuance are deferred.
- **JTBD traceability**: JTBD was skipped per config — two clear jobs. Every story below is user-visible
  (NOT `infrastructure-only`) and carries an explicit `job_id`. The two jobs are
  `invite-a-teammate-into-my-workspace` (admin) and `join-the-workspace-i-was-invited-to` (invitee).
  Promoting these to `docs/product/jobs.yaml` entries is tracked as Open decision OD-3 (a documentation
  formality, not a behavior change — there is no `jobs.yaml` in the repo today).

---

## US-01: Issue a member invite (admin-gated) — Walking Skeleton (issuance half)

`job_id: invite-a-teammate-into-my-workspace`

### Elevator Pitch
- **Before**: Dana Reyes wants to add her teammate Sam to the Northwind workspace, but there is no way
  for a workspace admin to invite anyone — only the instance super-admin can create workspaces (and their
  first admin), so Dana has to file a ticket and wait.
- **After**: Dana opens `GET /workspace/invites`, types `sam.okafor@northwind.example`, clicks "Send
  invite", and sees "Invite sent — share this link: https://…/invites/accept?id=…&sig=…", valid 7 days.
- **Decision enabled**: Dana can decide who joins Northwind and add them herself, immediately, without
  involving the instance operator.

### Problem
Dana Reyes is an admin of the "Northwind" workspace. She needs to bring her teammate Sam Okafor in, but
the only invite path that exists is the one the instance super-admin uses to provision a brand-new
workspace and its first admin. Dana finds it impossible to add a plain member to her own existing
workspace — there is no admin-facing invite surface at all.

### Who
- A workspace **admin** of an existing workspace | wants to add a teammate as a member | does not have
  (and should not need) instance super-admin powers | motivated to onboard the teammate now.

### Solution
A `GET /workspace/invites` admin-gated form (one email field) and a `POST /workspace/invites` that, gated
by `is_workspace_admin`, creates an `invites` row for the typed email in the admin's workspace, signs an
`InviteToken`, and emits the `/invites/accept` link (shown to the admin and best-effort emailed) — a thin
mirror of the shipped `bootstrap::create_invite`.

### Domain Examples
1. **Happy path** — Dana Reyes, admin of "Northwind", opens `/workspace/invites`, submits
   `sam.okafor@northwind.example`, and sees "Invite sent to sam.okafor@northwind.example — or share this
   link: https://foundry.northwind.example/invites/accept?id=018f…d7&sig=Qp4…nK (valid 7 days)".
2. **Edge: paste-only** — Dana's SMTP is down; the email send fails, but the confirmation still shows the
   link so Dana copies it into Slack and pastes it to Sam directly.
3. **Boundary: second invite, same email** — Dana invites `sam.okafor@northwind.example` again the next
   day (the first link was lost); a second live invite row is created and a fresh link emitted (each
   invite is independent and single-use).

### UAT Scenarios (BDD)
#### Scenario: An admin sends a member invite and gets a shareable link
Given Dana Reyes is signed in as an admin of the "Northwind" workspace
When Dana submits "sam.okafor@northwind.example" on the member-invite form
Then an invite to "Northwind" is created for that email
And Dana sees a confirmation with a shareable accept link valid for 7 days

#### Scenario: The link is shown even when the invite email fails to send
Given Dana is an admin of "Northwind" and the mail service is unavailable
When Dana submits "sam.okafor@northwind.example"
Then the invite is still created
And Dana still sees the shareable accept link to paste manually

#### Scenario: An admin can issue a second invite to the same email
Given Dana already issued Sam an invite yesterday that was never used
When Dana issues another invite to "sam.okafor@northwind.example"
Then a second independent live invite is created with its own link

### Acceptance Criteria
- [ ] GET `/workspace/invites` renders a one-email-field member-invite form for a signed-in workspace admin.
- [ ] POST with a valid email creates an `invites` row (`workspace_id` = the admin's active workspace,
      `invitee_email` = the typed email, `created_by` = the admin, `expires_at` = now + 7 days).
- [ ] The response shows the emitted `/invites/accept?id&sig` link; the link is also best-effort emailed.
- [ ] An email-send failure does not fail the request — the invite is created and the link is shown.
- [ ] The emitted signature verifies against the invite (`InviteToken::verify`).

### Outcome KPIs
- **Who**: workspace admins
- **Does what**: issue a member invite that produces a working accept link
- **By how much**: 95%+ of issuance attempts by an admin produce a valid emitted link
- **Measured by**: ratio of successful invite-row-creations to admin issuance POSTs
- **Baseline**: 0% (no admin issuance surface exists today)

### Technical Notes
- Mirrors `bootstrap::create_invite` (insert + sign + emit + best-effort email); adds the
  `is_workspace_admin` gate + `not_found()` non-enumerable posture (US-03 covers the refusal).
- New: GET/POST `/workspace/invites` handlers + a member-invite form template + the "invite sent" fragment.
- Reuses `insert_invite`, `InviteToken::new`, the email seam, CSRF.

---

## US-02: Accept a member invite and join — creates the account (Walking Skeleton, accept half)

`job_id: join-the-workspace-i-was-invited-to`

### Elevator Pitch
- **Before**: Sam Okafor receives an invite link but has no Foundry account at all — clicking it on the
  shipped flow would only work for a pre-existing user (the first-admin), so a brand-new invitee has no
  way to become a real, signed-in member.
- **After**: Sam opens `GET /invites/accept?id=…&sig=…`, sets a password, and is dropped straight onto
  the "Northwind" dashboard — a new account created for him, joined as a member, signed in, in one step.
- **Decision enabled**: Sam can choose his own password and start working in Northwind immediately,
  turning an invite link into a real account without any further admin involvement.

### Problem
Sam Okafor was invited to "Northwind" but has never used Foundry — he has no account. He finds it
impossible to join: the invite link is the only artifact he has, and the existing accept flow assumes the
user already exists. Setting a password needs to also bring his account into being and join him to the
workspace.

### Who
- An invitee with **no Foundry account** | received a member invite link out-of-band (email or pasted by
  an admin) | motivated to join the workspace now | will become a `member` (not admin).

### Solution
The reused `GET /invites/accept` renders the set-password form ("join as a member"); a `POST
/invites/accept` validates the password, then runs the NEW `create_member_and_consume` tx — create the
user (email from the invite), add a `member`-role membership, consume the invite, write the password — all
atomically, establishes a session, and 303-redirects onto the workspace. No separate login.

### Domain Examples
1. **Happy path** — Sam Okafor opens `https://foundry.northwind.example/invites/accept?id=018f…d7&sig=Qp4…nK`
   (issued 2 hours ago), sees "Set your password to join Northwind", enters `meadow-copper-violin-71`,
   submits, and lands on the Northwind dashboard signed in as `sam.okafor@northwind.example`, a new
   member account.
2. **Edge: near-fresh link** — Dana pastes Priya Shah the link the moment she issues it; Priya opens it 20
   seconds later, sets a password, and joins as a member immediately.
3. **Boundary: just inside expiry** — Sam opens his link 6 days 23 hours after issue (still unexpired);
   the form renders and he joins successfully; his membership role is `member`.

### UAT Scenarios (BDD)
#### Scenario: An invitee with no account sees a set-password form for a live invite
Given Dana issued Sam a live member invite for "Northwind" 2 hours ago
And Sam has no Foundry account yet
When Sam opens his invite link
Then he sees a set-password form naming the "Northwind" workspace

#### Scenario: Setting a valid password creates the account, joins the workspace, and signs in
Given Sam has opened his valid member invite for "Northwind"
When he submits "meadow-copper-violin-71" and confirms it
Then a new account is created for "sam.okafor@northwind.example"
And he is added to "Northwind" as a member
And he is signed in without a separate login step
And he lands on the "Northwind" workspace dashboard
And he sees no data from any other workspace

#### Scenario: A near-fresh member invite accepts immediately
Given Dana issued Priya Shah a member invite for "Northwind" 20 seconds ago
When Priya opens her link and sets a valid password
Then a new member account is created for Priya and she is signed in on "Northwind"

#### Scenario: A member invite just inside its expiry window still works
Given Sam's invite was issued 6 days and 23 hours ago and has not been used
When Sam opens his link and sets a valid password
Then his member account is created and he is signed in on "Northwind"

### Acceptance Criteria
- [ ] GET on a live, valid, unused invite renders a set-password form naming the workspace ("join as a member").
- [ ] No state is mutated on GET (no account created, invite unconsumed).
- [ ] POST with a valid password creates the user (email = `invites.invitee_email`), adds a
      `member`-role membership for `invites.workspace_id`, consumes the invite, writes the argon2id hash,
      establishes a session, and 303-redirects to `/` — all in one atomic transaction.
- [ ] The new member lands on `invites.workspace_id` and sees only that tenant's data.
- [ ] The new member reaches the workspace with no separate sign-in step and has `member` (not admin) privileges.

### Outcome KPIs
- **Who**: invitees with no prior Foundry account
- **Does what**: complete the accept flow — account created, joined as member, signed in on the workspace
- **By how much**: 90%+ of issued member invites result in a signed-in member landing within 7 days
- **Measured by**: ratio of invites consumed-with-account-and-session to invites issued (issuance + accept telemetry)
- **Baseline**: 0% (no member-accept path that creates an account exists today)

### Technical Notes
- NEW `create_member_and_consume` store tx (create user + member membership + consume + password, atomic),
  modeled on `provision_workspace`'s membership seeding fused with the shipped consume guard.
- Reuses the accept GET handler, `InviteAcceptPage`, `check_password_policy` (min-12), `hash_password`,
  session, `resolve_active_workspace`.
- Depends on US-01 (an invite to accept) and OD-1 (email-already-a-user behavior shapes the tx).

---

## US-03: Keep invites safe and honest — non-enumerable refusals + single-use (release gate)

`job_id: join-the-workspace-i-was-invited-to`

### Elevator Pitch
- **Before**: with a new issuance surface and an account-creating accept, an attacker could probe
  `/workspace/invites` to discover it exists, probe `/invites/accept?id=…` to enumerate invites, replay a
  used link to create a duplicate, or learn that an email already has an account — and a naive accept
  could create two accounts under a race.
- **After**: the issuance surface is invisible to non-admins (generic 404); every bad accept link AND the
  email-already-a-user case show one calm, identical page ("invite is no longer valid… ask your
  administrator to re-issue"); and an invite creates exactly one account, even under concurrency.
- **Decision enabled**: a recipient of a bad link knows what to do next (ask for a re-issue) while an
  attacker probing either surface learns nothing about who, what, or which email exists.

### Problem
Invite links get forwarded, re-clicked, and sat on past expiry; attackers probe issuance and accept URLs.
A non-admin must not be able to tell the issuance surface exists; a bad accept link must be honest for the
invitee yet reveal nothing about accounts/workspaces/invites/emails; and a link must never create two
accounts or be consumed twice.

### Who
- A workspace admin (legitimate issuer) | a plain member or signed-out caller probing `/workspace/invites`
  ("Malicious Mike") | a recipient of an expired/used/forwarded accept link (legitimate, e.g. Sam
  re-clicking) | Mike probing/tampering with accept URLs to enumerate invites or emails.

### Solution
An `is_workspace_admin`-gated issuance surface that returns a generic, non-enumerable 404 to any
non-admin; the reused uniform `invite_refusal_page()` for every invalid accept reason, extended to the
email-already-a-user case; and the atomic single-use `create_member_and_consume` guard so exactly one
account is created per invite.

### Domain Examples
1. **Happy path (security working)** — "Malicious Mike", a plain member of Northwind, opens
   `/workspace/invites`; he sees a generic "not found" — identical to any unknown route — and cannot tell
   the issuance surface exists. He then opens
   `/invites/accept?id=00000000-0000-0000-0000-000000000000&sig=AAAA`; he sees the same "invite is no
   longer valid" page Sam would see for an expired link.
2. **Edge: email already a user** — Dana invites `existing.user@northwind.example`, who already has a
   Foundry account; that person opens the link, sets a valid password, and sees the uniform "invite is no
   longer valid" page — no second account is created, no hint that the email already exists, and the
   invite is not consumed.
3. **Error/boundary: concurrent double-submit** — Sam double-clicks "Set password & join"; two POSTs
   race; exactly one creates his account, joins him, and signs him in; the other returns the uniform
   refusal; exactly one user and one membership exist.

### UAT Scenarios (BDD)
#### Scenario: A non-admin cannot tell the issuance surface exists
Given Sam is signed in as a plain member of "Northwind" (and separately, no one is signed in)
When the member-invite page is opened in each case
Then each response is byte-identical to a generic "not found"
And nothing reveals that an issuance surface exists

#### Scenario: An expired accept link is refused without leaking existence
Given Sam's member invite expired 1 day ago
When Sam opens his invite link
Then he sees the "invite is no longer valid" page advising him to ask his workspace admin to re-issue
And the page reveals nothing about whether any account or workspace exists

#### Scenario: Tampered, unknown, and used links are refused identically to an expired one
Given a tampered-signature link, an unknown invite id, and an already-used invite
When each accept is attempted
Then each shows the "invite is no longer valid" page
And each body is byte-identical to the expired-link refusal

#### Scenario: An invite whose email already has an account is refused without leaking that fact
Given Dana issued a member invite for an email that already has a Foundry account
When that invitee opens the link and submits a valid password
Then they see the "invite is no longer valid" page, byte-identical to the expired-link refusal
And no second account is created and the invite is not consumed

#### Scenario: Concurrent accepts create the account exactly once
Given Sam's member invite is live
When two accept submissions for the same invite arrive concurrently
Then exactly one creates the account, joins, and signs him in
And the other receives the "invite is no longer valid" page
And exactly one user and one membership are created

#### Scenario: Both POSTs are refused without a valid CSRF token
Given a forged issuance POST and a forged accept POST, each without a valid CSRF token
When each reaches its endpoint
Then each is refused by the request-forgery protection
And no invite is created, no invite is consumed, and no account is created

### Acceptance Criteria
- [ ] A non-admin or signed-out GET/POST to `/workspace/invites` returns a response byte-identical to a
      generic 404 and creates no invite.
- [ ] Expired, already-used, tampered-signature, unknown-id, AND email-already-a-user accepts all render
      the SAME refusal body and status (a litmus REDs if any arm diverges).
- [ ] The accept refusal discloses no account/workspace/invite/email existence and advises asking the
      admin to re-issue.
- [ ] An invite creates exactly one account and is consumable exactly once; re-opening a consumed link is
      refused (no second account, no session).
- [ ] Under concurrent accepts of one invite, exactly one succeeds; exactly one user and one membership exist.
- [ ] Expiry is enforced on both GET (liveness) and inside the consume transaction.
- [ ] An email collision surfaces as the uniform refusal — never as a DB/constraint error page.
- [ ] No `sig` value and no password appears in application logs after a full cycle.

### Outcome KPIs
- **Who**: anyone (legitimate or hostile) who probes the issuance surface or opens a non-live/colliding accept link
- **Does what**: receives a uniform non-enumerable refusal AND cannot create a duplicate account or consume an invite twice
- **By how much**: 100% byte-identical refusals across {non-admin issuance, expired, used, tampered,
  unknown, email-collision}; 0 successful double-creates / double-consumes (guardrail)
- **Measured by**: refusal-arm byte-identity litmus + single-use/single-create @property test in the acceptance suite
- **Baseline**: undefined today (no member issuance/accept route)

### Technical Notes
- Reuses `invite_refusal_page()` (extended to the email-collision arm) + the `not_found()` issuance gate.
- Atomic guarded `create_member_and_consume` (NFR-2); CSRF on both POSTs (NFR-6); no token/secret in logs (NFR-5).
- The email collision MUST be caught inside the tx and mapped to the uniform refusal (not a 500).
- Depends on US-01 + US-02; OD-1 (email-already-a-user policy).

---

## US-04: Correct mistakes without losing the invite (inline recovery)

`job_id: join-the-workspace-i-was-invited-to`

### Elevator Pitch
- **Before**: a naive accept might consume the invite (and create a half-formed account) on a rejected
  password, stranding the invitee with a dead link; and an admin who fat-fingers an email would create a
  junk invite.
- **After**: Sam enters too-short a password (or a mismatched confirmation), sees a gentle inline error,
  and his invite is still live with no account created — he just fixes it and submits again; Dana's blank
  email is rejected inline with no invite created.
- **Decision enabled**: Sam can safely recover from a typo himself without asking Dana to re-issue, and
  Dana can correct a mis-typed email without leaving a dead invite behind.

### Problem
Invitees fumble passwords (too short, or confirmation mismatch) and admins fumble emails. If a rejected
password consumed the invite or created an account, Sam would be stranded; if a blank email created an
invite, the backlog would fill with junk. Both surfaces need inline correction that leaves no side effect.

### Who
- An invitee setting a password for the first time | likely to mistype or pick a weak password ("Careless
  Cathy") | AND an admin who mistypes or leaves the email blank on the issuance form.

### Solution
On the accept POST, a weak or mismatched password re-renders the set-password form with a clear inline
error, the invite **untouched** and **no account created** (reused US-03 inline-recovery path from the
shipped flow). On the issuance POST, a blank/invalid email re-renders the form with an inline error and
**no invite created**.

### Domain Examples
1. **Happy path (invitee recovery)** — Sam enters `pizza` (5 chars, below the 12-char policy); he sees
   "Password must be at least 12 characters", the form re-renders, he enters `meadow-copper-violin-71`,
   and joins successfully — same invite, never re-issued, one account created.
2. **Edge: confirmation mismatch** — Priya Shah enters `correct-horse-battery` in new but
   `correct-horse-bttery` in confirm; she sees "Passwords do not match"; she fixes it and joins.
3. **Boundary: admin blank email** — Dana submits the issuance form with an empty email; she sees "Please
   enter an email address"; no invite is created; she types Sam's email and succeeds.

### UAT Scenarios (BDD)
#### Scenario: A weak password is corrected inline and creates no account
Given Sam has opened his valid member invite for "Northwind"
When he submits "pizza" (below the strength policy)
Then he sees an inline error stating the minimum password length
And his invite is still live and unconsumed
And no account is created and no session is created

#### Scenario: A mismatched confirmation is corrected inline
Given Priya Shah has opened her valid member invite for "Northwind"
When her confirmation "correct-horse-bttery" does not match her new password "correct-horse-battery"
Then she sees an inline error that the passwords do not match
And her invite is still live and unconsumed and no account is created

#### Scenario: A blank email on the issuance form is corrected inline
Given Dana is on the member-invite form
When Dana submits the form with an empty email
Then she sees an inline error asking for an email address
And no invite is created

#### Scenario: A valid retry after an inline error completes the join
Given Sam saw a password error on his first attempt on his live invite
When he resubmits a 12-character password and confirms it
Then his member account is created and he is signed in on "Northwind"

### Acceptance Criteria
- [ ] A password below the minimum length is refused inline; the invite is NOT consumed and NO account is created.
- [ ] A confirmation that does not match the new password is refused inline; the invite is NOT consumed; no account.
- [ ] A blank or malformed email on the issuance form is refused inline; NO invite is created.
- [ ] A password at or above the minimum length is accepted.
- [ ] After an inline error, re-submitting a valid password on the same invite completes the join.

### Outcome KPIs
- **Who**: invitees who hit a password validation error (and admins who hit an email validation error)
- **Does what**: recover and complete the action on the same invite/form (no re-issue, no junk invite)
- **By how much**: 80%+ of invitees who hit a password error go on to complete the join on the same invite
- **Measured by**: join-completion rate among sessions that recorded a password validation error
- **Baseline**: 0% (no accept flow exists)

### Technical Notes
- Reuses the shipped US-03 inline-recovery path (`re_render_with_error`) and `check_password_policy` (min-12).
- Issuance email validation is a small new inline check on the issuance POST.
- `hash_password` runs only after validation passes; the tx opens only on a valid password.
- Depends on US-01 (issuance email validation) and US-02 (set-password POST).
