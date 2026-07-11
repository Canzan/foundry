<!-- markdownlint-disable MD024 -->

# User Stories — recipient-notification-preferences (v1 = recipient unsubscribe)

## System Constraints (cross-cutting — apply to every story)

- **Per-workspace, email-keyed opt-out**: unsubscribe state keys on `(email_lower, workspace_id)` (BR-2). Many
  recipients are **invitees with no account** — the key is the **email** the notifier already targets
  (`Notification.recipient`, `notify.rs:117-122`), not a user id. Default (no row) = **subscribed** (BR-7).
- **Suppressible vs mandatory (bounded allow-list)**: only `workspace_invite` + `member_invite` are
  **suppressible**; `password_reset`, `password_changed`, `member_removed` are **mandatory** and **never**
  suppressed (BR-1, BR-3, NFR-3). Mandatory > unsubscribe, always.
- **Signed, non-enumerable token**: the unsubscribe link carries an HMAC-signed `UnsubscribeToken` binding
  `email` + `workspace_id` (the shipped `InviteToken` model, `foundry-auth/src/lib.rs:354-390`); any invalid /
  tampered / unknown token yields the **uniform non-enumerable refusal** (`invites_accept.rs:332-339`) and
  records nothing (NFR-1, BR-4).
- **Prefetch-safe**: a bare GET of the unsubscribe URL never mutates state; only an explicit confirm (POST /
  RFC 8058 one-click) does (NFR-2, ODD-2).
- **CSRF on both POSTs**: the public unsubscribe **confirm** and the signed-in **resubscribe** both sit under
  the shipped CSRF middleware (`csrf.rs:137`, layer `lib.rs:536-539`) (NFR-5).
- **Least-privilege signed-in surface**: a member manages **only their own** state, scoped to workspaces they
  belong to, identity from the **session** (`SessionUser`, `session.rs`), never client-supplied email (NFR-6).
- **Additive / backwards-compatible**: with an empty unsubscribe table the notifier behaves exactly as it does
  post `notification-delivery-providers`; the filter only ever *removes* a suppressible delivery for an
  *unsubscribed* pair (NFR-7).
- **Observability without PII**: a suppressed delivery is counted with `event` (+ at most `workspace`) labels —
  **never** the recipient email or token (NFR-4, ODD-5).
- **v1 boundary = US-01..US-04** (the mechanism proven end-to-end on both invite events, with the security
  invariant and the non-enumerable/prefetch-safety guarantees). US-05..US-07 (signed-in status, resubscribe,
  operator visibility) are fast-follow in this same feature.
- **JTBD traceability**: JTBD is folded inline (no `docs/product/` SSOT — see `requirements.md`). Every story
  carries an explicit `job_id` referencing one of three jobs:
  `stop-unwanted-workspace-notifications` (Sam, recipient), `manage-my-notification-subscription` (Maria,
  account holder), or `honor-unsubscribe-and-preserve-security` (Olivia, ops/compliance). No `infrastructure-only`.

---

## US-01: Stop a workspace's invite emails with one click (Walking Skeleton)

`job_id: stop-unwanted-workspace-notifications`

### Elevator Pitch
- **Before**: Sam Okafor (`sam.okafor@acme.example`) was invited to the "Northwind" workspace and keeps getting
  `workspace_invite` reminder emails he never asked for. He has **no account** and **no way** to stop them —
  his only option is a blunt inbox rule that would also bury a password-reset email.
- **After**: the invite email now carries an **Unsubscribe** link. Sam clicks it (logged-out), lands on a page
  that says *"Stop invitation emails from Northwind?"*, clicks **Confirm**, and sees *"Done — you won't receive
  further invitations from Northwind. You'll still get security-critical notifications."* The next
  `workspace_invite` for `(sam.okafor@acme.example, Northwind)` is **never delivered**.
- **Decision enabled**: Sam can silence exactly the workspace noise he didn't ask for, from the email itself,
  without an account and without fear of losing important mail.

### Problem
Sam is an invitee, not a user. Repeat `workspace_invite` emails arrive and he can't turn them off — there is no
opt-out anywhere in Foundry, and any account-gated screen wouldn't reach him because he has no account. He needs
a one-click, per-workspace unsubscribe that works straight from the email.

### Who
- A **notification recipient** (invitee or member) identified only by **email** | has an invite email in hand,
  no account required | wants to stop this workspace's non-essential emails | motivated to quiet the noise
  without risking security-critical mail.

### Solution
Add a signed `UnsubscribeToken` (binding `sam.okafor@acme.example` + Northwind's `workspace_id`, modelled on
`InviteToken`) to the `workspace_invite` email (`bootstrap.rs:266`). A **public** `/unsubscribe` route verifies
the token, shows a confirm page, and on **POST confirm** writes a row to the new
`notification_unsubscribes(email_lower, workspace_id, unsubscribed_at)` table (migration `0014`). The **notifier**
then suppresses future `workspace_invite` deliveries to that pair (hook per ODD-3). This is the whole
carry-the-link → click → verify → record → confirm → suppress loop on **one** suppressible event.

### Domain Examples
1. **Happy path** — Sam gets a `workspace_invite` for Northwind, clicks Unsubscribe, confirms; a row
   `(sam.okafor@acme.example, northwind-ws-id)` is written; the next Northwind invite to Sam is suppressed
   (not delivered by any provider), and a suppression is counted.
2. **Edge: still subscribed elsewhere** — Sam is also invited to "Contoso"; unsubscribing from Northwind does
   **not** affect Contoso — a `workspace_invite` for `(sam.okafor@acme.example, contoso-ws-id)` still delivers
   (per-workspace, FR-9).
3. **Boundary: already unsubscribed** — Sam clicks the same link twice; the second confirm is an idempotent
   no-op success ("You're already unsubscribed from Northwind"), no duplicate row, no error (BR-8).

### UAT Scenarios (BDD)
#### Scenario: One click from the invite email stops that workspace's invitations
Given Sam has a workspace-invite email for "Northwind" containing an unsubscribe link
When Sam opens the link and confirms unsubscribing
Then Sam sees a confirmation that Northwind invitations are stopped
And the next workspace-invite for Sam from Northwind is not delivered

#### Scenario: Unsubscribing from one workspace leaves another untouched
Given Sam has unsubscribed from "Northwind"
And Sam is also invited to "Contoso"
When a workspace-invite for "Contoso" is issued to Sam
Then the Contoso invitation is delivered normally

#### Scenario: Confirming an unsubscribe twice is a harmless no-op
Given Sam has already unsubscribed from "Northwind"
When Sam opens the same unsubscribe link and confirms again
Then Sam sees that he is already unsubscribed from Northwind
And no error occurs and no duplicate record is created

### Acceptance Criteria
- [ ] The `workspace_invite` email carries a signed unsubscribe link binding the recipient email + workspace, usable logged-out.
- [ ] Confirming the link records an opt-out for `(email_lower, workspace_id)` and shows a confirmation naming the workspace and reassuring about security notifications.
- [ ] After unsubscribe, a subsequent `workspace_invite` to that `(email, workspace)` is not delivered by any provider (suppressed).
- [ ] An unsubscribe in one workspace does not suppress deliveries in another workspace for the same email (FR-9).
- [ ] Unsubscribing an already-unsubscribed pair is an idempotent no-op success — no duplicate row, no error (BR-8).
- [ ] With no unsubscribe rows, `workspace_invite` delivery is unchanged from today (NFR-7).

### Outcome KPIs
- **Who**: notification recipients getting unwanted workspace invites
- **Does what**: stop a specific workspace's invitation emails from the email itself
- **By how much**: 100% of confirmed unsubscribes result in the next matching `workspace_invite` being suppressed (0 leaks)
- **Measured by**: suppression count vs post-unsubscribe `workspace_invite` emissions for unsubscribed pairs, in a dogfood run
- **Baseline**: 0% — no opt-out exists today; every invite is delivered

### Technical Notes
- New `UnsubscribeToken` mirrors `InviteToken` (`foundry-auth/src/lib.rs:354-390`); binds `email|workspace_id`; expiry stance ODD-1, GET-safety ODD-2.
- New table `0014_notification_unsubscribes` follows the `reset_tokens` shape (`0002_sessions_and_reset.sql:20-28`); shape/email-keying ODD-4.
- Public route joins the `/invites/accept` cluster (`lib.rs:371-374`); suppression hook per ODD-3 (notifier needs workspace context — `Notification` carries none today, `notify.rs:117-122`).
- Carries the whole feature's uncertainty (token, table, route, filter). Walking skeleton.

---

## US-02: Never lose a security-critical notification, even when unsubscribed

`job_id: stop-unwanted-workspace-notifications`

### Elevator Pitch
- **Before**: Sam is nervous — if he unsubscribes from Northwind, will he also stop getting the emails that
  actually matter, like a password reset or a "you were removed from the workspace" notice? Today there's no
  opt-out at all, so the question is unanswered and scary.
- **After**: with `(sam.okafor@acme.example, Northwind)` unsubscribed, Sam requests a password reset and it
  **still arrives**; an admin removes him from Northwind and the `member_removed` email **still arrives** —
  because `password_reset`, `password_changed`, and `member_removed` are **mandatory** and can never be
  suppressed. The confirmation page told him this would be true, and it is.
- **Decision enabled**: Sam can unsubscribe from the noise **with confidence**, knowing security-critical mail
  is guaranteed to reach him regardless.

### Problem
An opt-out that could accidentally swallow a password-reset or an account-removal notice would be a security and
safety hazard — and would make recipients afraid to use it at all. The mechanism must make the security events
**structurally always-on**, and prove it.

### Who
- The **same recipient** who unsubscribed (Sam) | AND, in the background, **Ops/Compliance Olivia** who must
  guarantee security mail is never withheld | motivated by trust and safety.

### Solution
Partition the catalog into a **bounded suppressible allow-list** (`workspace_invite`, `member_invite`) and a
**mandatory** set (`password_reset`, `password_changed`, `member_removed`). The suppression filter (US-01)
applies **only** to the allow-list; mandatory events skip the unsubscribe check entirely and always deliver,
counted `delivered` (never `suppressed`). The rule is enforced at the single suppression point and guarded by a
property litmus.

### Domain Examples
1. **Happy path (reset)** — `(sam.okafor@acme.example, Northwind)` is unsubscribed; Sam requests a password
   reset; the `password_reset` email delivers normally through every active provider (counted `delivered`).
2. **Edge (removal)** — an admin removes Sam from Northwind while he's unsubscribed there; the `member_removed`
   email **still** delivers — he must learn he was removed (counted `delivered`, not `suppressed`).
3. **Boundary (password changed)** — Sam changes his password while unsubscribed everywhere; the
   `password_changed` confirmation still delivers; no mandatory event is ever counted `suppressed`.

### UAT Scenarios (BDD)
#### Scenario: A password reset reaches an unsubscribed recipient
Given Sam has unsubscribed from "Northwind"
When Sam requests a password reset
Then the password-reset notification is delivered to Sam
And it is not suppressed

#### Scenario: A removal notice reaches an unsubscribed recipient
Given Sam has unsubscribed from "Northwind"
When an admin removes Sam from "Northwind"
Then the member-removed notification is delivered to Sam
And it is not suppressed

#### Scenario: No mandatory event is ever suppressed
Given a recipient is unsubscribed from every workspace they belong to
When any of a password reset, a password change, or a removal fires for them
Then every one of those notifications is delivered
And none is recorded as suppressed

### Acceptance Criteria
- [ ] `password_reset`, `password_changed`, and `member_removed` are delivered regardless of any unsubscribe state (FR-4, BR-3).
- [ ] A mandatory event is never counted `suppressed` under any unsubscribe configuration.
- [ ] The suppressible set is a bounded allow-list ({`workspace_invite`, `member_invite`}); adding a new mandatory event does not accidentally make it suppressible.
- [ ] A regression that suppressed a mandatory event reds a dedicated never-suppress `@property` litmus.
- [ ] The confirmation page's promise ("you'll still receive security-critical notifications") is literally true.

### Outcome KPIs
- **Who**: recipients who have unsubscribed from one or more workspaces
- **Does what**: continue receiving 100% of security-critical notifications
- **By how much**: 0 mandatory events suppressed (hard guardrail), across every unsubscribe configuration
- **Measured by**: count of mandatory events with `outcome=suppressed` (must be 0) + the never-suppress `@property` in the acceptance suite
- **Baseline**: N/A today (no suppression exists) — establishes the invariant as suppression is introduced

### Technical Notes
- Enforced at the single suppression point (ODD-3); mandatory events skip the unsubscribe lookup entirely.
- Guardrail story: depends on US-01 (the suppression filter it constrains). No new persistence.
- Precedence rule BR-3 (mandatory > unsubscribe); litmus mirrors the predecessor's `@property` isolation style.

---

## US-03: A tampered or unknown unsubscribe link is safely refused without leaking who exists

`job_id: stop-unwanted-workspace-notifications`

### Elevator Pitch
- **Before**: an unsubscribe link that behaved differently for a real recipient than for a made-up one would let
  an attacker (call her Mallory) **probe** whether an address is a live Foundry recipient — and a destructive
  GET could let a mail scanner's **prefetch** silently unsubscribe Sam without his intent.
- **After**: Mallory flips a byte of Sam's token, or swaps in a different `workspace_id`, or invents a token
  wholesale — every case returns the **identical** refusal page (fixed 200, byte-identical body), records
  **nothing**, and reveals nothing about whether Sam, the workspace, or an account exists. A bare **GET**
  prefetch of Sam's real link changes **no** state; only Sam's explicit **Confirm** does.
- **Decision enabled**: Sam (and Olivia) can trust the link is safe to click and safe to exist in an inbox —
  it can't be turned into an enumeration oracle or a prefetch trap.

### Problem
A public, token-verified endpoint is an attack surface: it must not leak existence via differential responses,
and it must not mutate state on a prefetch. Both are classic email-link failure modes; the shipped invite flow
already solved the first (uniform refusal) and this must match it.

### Who
- Every **recipient** (protected from silent/prefetch unsubscribe and from having their address confirmed to an
  attacker) | AND **Malicious Mallory** (the adversary the design must defeat) | motivated by trust + privacy.

### Solution
Verify the `UnsubscribeToken` with a **constant-time** HMAC check (the `foundry-auth` `verify`, `lib.rs:260`);
on any failure return the **uniform non-enumerable refusal** (`invite_refusal_page` model,
`invites_accept.rs:332-339`) — a fixed status and byte-identical body for every invalid reason — and record
nothing. Make the raw **GET** non-destructive: state changes only on the explicit confirm (POST / RFC 8058
one-click, ODD-2).

### Domain Examples
1. **Happy path (tamper rejected)** — Mallory alters one character of Sam's token; the endpoint returns the
   standard refusal page, writes no row, and is indistinguishable from a totally invalid token.
2. **Edge (existence not leaked)** — Mallory requests unsubscribe for `nobody@nowhere.example` with a random
   token vs Sam's real address with a bad token; both return the **same** page — Mallory learns nothing about
   which address is real.
3. **Error/boundary (prefetch-safe)** — a mail security scanner prefetches (GETs) Sam's real unsubscribe URL;
   **no** unsubscribe row is created — only Sam's later Confirm (POST) records the opt-out (NFR-2).

### UAT Scenarios (BDD)
#### Scenario: A tampered token is refused exactly like an invalid one
Given an unsubscribe link whose token has been altered
When the link is opened
Then the uniform "no longer valid" refusal page is shown
And no unsubscribe is recorded

#### Scenario: The response does not reveal whether an address exists
Given one request for a real recipient with an invalid token
And another request for a non-existent address with an invalid token
When both links are opened
Then both return an identical refusal response
And neither reveals whether the address, workspace, or account exists

#### Scenario: Prefetching the link does not unsubscribe anyone
Given Sam has a valid unsubscribe link he has not yet confirmed
When an automated client fetches (GETs) the link without confirming
Then no unsubscribe is recorded
And Sam remains subscribed until he explicitly confirms

### Acceptance Criteria
- [ ] A tampered / malformed / unknown token returns the uniform non-enumerable refusal (fixed status + byte-identical body) and records nothing (NFR-1, BR-4).
- [ ] The refusal response is indistinguishable between a real recipient and a non-existent address (no existence leak).
- [ ] Token verification uses a constant-time comparison (reusing `foundry-auth`'s `verify`).
- [ ] A bare GET of a valid unsubscribe URL creates no unsubscribe row; only an explicit confirm mutates state (NFR-2).
- [ ] No unsubscribe token or recipient email appears in logs or error output for a refused request.

### Outcome KPIs
- **Who**: all recipients and would-be attackers of the unsubscribe endpoint
- **Does what**: are prevented from enumerating recipients or triggering prefetch/silent unsubscribes
- **By how much**: 0 differential responses between real and non-existent addresses; 0 state changes from a GET prefetch
- **Measured by**: response-equality litmus (real vs fake) + prefetch-safety litmus, both in the acceptance suite (revert-reds-it)
- **Baseline**: N/A today (no public unsubscribe endpoint exists) — establishes the guarantee with the endpoint

### Technical Notes
- Reuses the uniform refusal (`invites_accept.rs:332-339`) and constant-time verify (`foundry-auth/src/lib.rs:260`).
- GET-safety / one-click POST stance is ODD-2 (RFC 8058 `List-Unsubscribe-Post` vs GET→confirm→POST).
- Depends on US-01 (the token + route it hardens). Security story; no new persistence.

---

## US-04: The same one-click unsubscribe works for member-invite emails

`job_id: stop-unwanted-workspace-notifications`

### Elevator Pitch
- **Before**: US-01 proved unsubscribe on `workspace_invite`, but the *other* invite email — `member_invite`,
  sent when an admin adds someone to a workspace (`member_invites.rs:204`) — still has no opt-out, so a
  recipient getting repeated member-invite reminders is stuck.
- **After**: the `member_invite` email carries the **same** signed unsubscribe link, bound to the recipient +
  that workspace. Sam confirms once and both invite-type emails from that workspace stop — the mechanism
  generalises across the whole suppressible set, not just one event.
- **Decision enabled**: a recipient can silence **all** of a workspace's invitation noise (both invite events)
  with the same one-click action, proving the opt-out is a general per-workspace mechanism.

### Problem
An opt-out that only covered one of the two invite events would be a half-measure — a recipient would still get
member-invite reminders after unsubscribing. The suppressible allow-list is two events; both must honour the
same `(email, workspace)` opt-out.

### Who
- A **recipient** getting `member_invite` reminders (an admin keeps re-adding / re-inviting them) | wants the
  same per-workspace opt-out to cover this event too | motivated by complete, not partial, quiet.

### Solution
Attach the same `UnsubscribeToken` link to the `member_invite` emit site (`member_invites.rs:204`), and include
`member_invite` in the suppressible allow-list so the notifier suppresses it for an unsubscribed pair exactly as
it does `workspace_invite`. One opt-out row covers both suppressible events for that `(email, workspace)`.

### Domain Examples
1. **Happy path** — Sam, unsubscribed from Northwind via a `workspace_invite` link (US-01), is then re-added by
   an admin; the resulting `member_invite` to `(sam.okafor@acme.example, Northwind)` is **suppressed** by the
   same opt-out — no new action needed.
2. **Edge: member_invite is the first contact** — Sam's first email from Northwind is a `member_invite`; its
   unsubscribe link works identically to the workspace-invite one and mutes both events for Northwind.
3. **Boundary: mandatory still delivers** — after unsubscribing via a `member_invite` link, a `member_removed`
   for Sam from Northwind still delivers (mandatory, US-02) — only the invites are suppressed.

### UAT Scenarios (BDD)
#### Scenario: One opt-out covers both invite events for a workspace
Given Sam has unsubscribed from "Northwind" via a workspace-invite link
When an admin re-adds Sam and a member-invite for Northwind fires
Then the member-invite is not delivered to Sam

#### Scenario: The member-invite email carries its own unsubscribe link
Given Sam's first contact from "Northwind" is a member-invite email
When Sam opens its unsubscribe link and confirms
Then Sam is unsubscribed from Northwind
And both member-invite and workspace-invite emails from Northwind are suppressed

#### Scenario: Unsubscribing via a member-invite still leaves security mail intact
Given Sam has unsubscribed from "Northwind" via a member-invite link
When a member-removed notification for Sam from Northwind fires
Then it is still delivered to Sam

### Acceptance Criteria
- [ ] The `member_invite` email carries the same signed unsubscribe link (bound to recipient + workspace) as `workspace_invite`.
- [ ] `member_invite` is in the suppressible allow-list; it is suppressed for an unsubscribed `(email, workspace)`.
- [ ] A single opt-out row suppresses both suppressible events for that pair (one confirm covers both).
- [ ] Mandatory events remain delivered after an unsubscribe made via a `member_invite` link (US-02 holds).
- [ ] With no unsubscribe rows, `member_invite` delivery is unchanged from today (NFR-7).

### Outcome KPIs
- **Who**: recipients getting member-invite reminders
- **Does what**: silence member-invite emails with the same per-workspace opt-out
- **By how much**: 100% of `member_invite` deliveries to an unsubscribed pair are suppressed; both suppressible events covered by one opt-out
- **Measured by**: suppression count for `event=member_invite` vs emissions to unsubscribed pairs, dogfood run
- **Baseline**: 0% (no opt-out for member invites today)

### Technical Notes
- Extends US-01's mechanism to `member_invites.rs:204`; no new token/table/route — reuses all of US-01.
- Completes the **v1 boundary** (US-01..US-04): mechanism proven on the full suppressible set, security exempt, non-enumerable + prefetch-safe.
- Depends on US-01 (token/table/route/filter) + US-02 (mandatory exemption).

---

## US-05: See my per-workspace notification status when I'm signed in

`job_id: manage-my-notification-subscription`

### Elevator Pitch
- **Before**: Maria Santos (`maria.santos@acme.example`) belongs to three workspaces and thinks she
  unsubscribed from one months ago via an email link — but she has no idea which, and the email is long gone.
  There is nowhere in Foundry to check.
- **After**: Maria signs in, opens **Account → Notifications** (`GET /account/notifications`), and sees a list:
  *Northwind — Muted*, *Contoso — Subscribed*, *Initech — Subscribed*. Her status for every workspace she
  belongs to, in one place, derived from her session identity — showing only her own state.
- **Decision enabled**: Maria can see at a glance which workspaces can email her and which she has muted, so she
  knows exactly where she stands without hunting for an old link.

### Problem
Once a recipient unsubscribes via an email link, there is no way for an account holder to review that state.
Maria can't tell whether a past unsubscribe is still in effect or which workspace it applied to. She needs a
signed-in, per-workspace status view — scoped strictly to herself.

### Who
- An **account-holding member** of one or more workspaces | signed in | wants to review her subscription status
  per workspace | motivated by control and clarity, and must never see anyone else's state.

### Solution
Add a signed-in `GET /account/notifications` page (neighbour of `/account/password`, `lib.rs:415-418`) that
resolves Maria's own email + her workspace memberships from the session and store (`SessionUser`;
`find_user_by_email`, workspace membership lookups) and renders, per workspace, whether
`(maria's email_lower, workspace_id)` has an unsubscribe row (Muted) or not (Subscribed). Least-privilege: only
her workspaces, only her state (NFR-6). A11y-compliant page (NFR-8).

### Domain Examples
1. **Happy path** — Maria belongs to Northwind (muted), Contoso, Initech; the page shows Northwind "Muted" and
   the other two "Subscribed".
2. **Edge: all subscribed** — Maria never unsubscribed; every workspace shows "Subscribed" with a mute option
   available (mute-from-settings is a could-have; status view is the requirement here).
3. **Boundary: no other recipient visible** — a crafted request adding another user's email as a parameter
   still returns only Maria's own workspaces and status — identity comes from the session, not the request
   (NFR-6).

### UAT Scenarios (BDD)
#### Scenario: The settings page shows per-workspace subscription status
Given Maria is signed in and belongs to "Northwind", "Contoso", and "Initech"
And Maria is unsubscribed from "Northwind"
When Maria opens the notification settings page
Then she sees "Northwind" as muted
And she sees "Contoso" and "Initech" as subscribed

#### Scenario: The page shows only the signed-in member's own workspaces
Given Maria is signed in
When Maria opens the notification settings page
Then only workspaces Maria belongs to are listed
And no other recipient's status is shown

#### Scenario: A request cannot be steered to another recipient's status
Given Maria is signed in
When a request attempts to view notification status for another user's email
Then only Maria's own status is returned

### Acceptance Criteria
- [ ] `GET /account/notifications` lists each workspace the signed-in member belongs to with a Subscribed/Muted status.
- [ ] Status reflects whether `(member's email_lower, workspace_id)` has an unsubscribe row (FR-6).
- [ ] Only the signed-in member's own workspaces and own status are shown; identity is from the session, not client input (NFR-6, BR-6).
- [ ] The page is reachable from the account/nav surface and requires an authenticated session.
- [ ] The page meets WCAG 2.1 AA basics (labelled controls, status as text, keyboard-navigable) (NFR-8).

### Outcome KPIs
- **Who**: account-holding members who have (or wonder if they have) unsubscribed
- **Does what**: view their per-workspace subscription status in one signed-in place
- **By how much**: 100% of a member's workspaces show an accurate Subscribed/Muted status; 0 other-recipient rows ever shown
- **Measured by**: page-render correctness vs unsubscribe table for the member + a least-privilege scope test
- **Baseline**: 0 — no way to view subscription status today

### Technical Notes
- New authed route beside `/account/password` (`lib.rs:415-418`); session identity via `SessionUser`; workspace membership + `find_user_by_email` (`foundry-store/src/lib.rs:930`, membership lookups).
- Multi-workspace listing interaction is ODD-7; a11y is a new surface (NFR-8) — unlike the predecessor's no-UI NFR-7 (N/A).
- Depends on US-01 (the unsubscribe state it reads). Read-only (resubscribe is US-06).

---

## US-06: Resubscribe a workspace I previously muted

`job_id: manage-my-notification-subscription`

### Elevator Pitch
- **Before**: Maria sees on the settings page that Northwind is **Muted**, but she now *wants* those emails
  back — and her only historical unsubscribe was a one-way email link with no "undo". She's stuck muted.
- **After**: next to *Northwind — Muted*, Maria clicks **Resubscribe**; a CSRF-protected POST clears the opt-out
  for `(maria.santos@acme.example, Northwind)`, the row flips to **Subscribed**, and the next Northwind invite
  reaches her again.
- **Decision enabled**: Maria can reverse a past unsubscribe herself, any time, without asking an admin or
  hunting for a link — full control over her own subscription.

### Problem
Unsubscribe is currently one-way. An account holder who changes her mind has no path back to subscribed. She
needs a safe, self-service resubscribe on her own state.

### Who
- An **account-holding member** viewing her settings page | wants to undo a previous mute for a workspace |
  motivated to receive that workspace's notifications again, on her own initiative.

### Solution
Add a **CSRF-protected** `POST /account/notifications/resubscribe` (under the shipped CSRF middleware,
`csrf.rs:137`) that, scoped to the session identity, clears the unsubscribe row for `(member's email_lower,
workspace_id)` and re-renders the page showing that workspace as Subscribed. Idempotent (resubscribing an
already-subscribed workspace is a no-op success, BR-8). Least-privilege: only the member's own pairs (NFR-6).

### Domain Examples
1. **Happy path** — Maria resubscribes Northwind; the opt-out row is cleared; the next `workspace_invite` /
   `member_invite` for `(maria.santos@acme.example, Northwind)` delivers again.
2. **Edge: idempotent** — Maria clicks Resubscribe on an already-subscribed Contoso (double-click / stale page);
   it's a no-op success, still Subscribed, no error (BR-8).
3. **Boundary: CSRF required** — a forged cross-site POST to resubscribe (no valid `_csrf`) is rejected `403`
   and changes nothing (NFR-5).

### UAT Scenarios (BDD)
#### Scenario: Resubscribing a muted workspace restores its notifications
Given Maria is signed in and "Northwind" shows as muted
When Maria clicks Resubscribe for Northwind
Then Northwind shows as subscribed
And the next Northwind invitation is delivered to Maria again

#### Scenario: Resubscribing an already-subscribed workspace is harmless
Given Maria is signed in and "Contoso" shows as subscribed
When Maria submits a resubscribe for Contoso
Then Contoso remains subscribed
And no error occurs

#### Scenario: A resubscribe without a valid CSRF token is rejected
Given a cross-site request attempts to resubscribe Maria to "Northwind" without a valid CSRF token
When the request is submitted
Then it is rejected
And Maria's subscription state is unchanged

### Acceptance Criteria
- [ ] `POST /account/notifications/resubscribe` clears the unsubscribe row for `(member's email_lower, workspace_id)` and shows the workspace as Subscribed (FR-7).
- [ ] After resubscribe, a subsequent suppressible notification to that pair is delivered again.
- [ ] Resubscribing an already-subscribed pair is an idempotent no-op success (BR-8).
- [ ] The POST is CSRF-protected; a request without a valid token is rejected `403` and changes no state (NFR-5).
- [ ] A member can resubscribe only their own pairs, scoped to workspaces they belong to (NFR-6, BR-6).

### Outcome KPIs
- **Who**: account-holding members who previously muted a workspace
- **Does what**: restore that workspace's notifications themselves
- **By how much**: 100% of resubscribes restore delivery for that pair; 0 cross-recipient or CSRF-forged state changes succeed
- **Measured by**: post-resubscribe delivery check + CSRF-rejection and least-privilege tests
- **Baseline**: 0 — unsubscribe is one-way today (no resubscribe path)

### Technical Notes
- CSRF-protected POST under the shipped middleware (`csrf.rs:137`, layer `lib.rs:536-539`); session-scoped identity (NFR-6).
- Resubscribe UX for **account-less** recipients (who can't sign in) is ODD-6 — a token-based resubscribe / an undo on the confirmation page is a candidate; v1 signed-in resubscribe covers account holders.
- Depends on US-05 (the page it acts on) + US-01 (the state it clears).

---

## US-07: See how much notification opt-out is happening, without exposing who

`job_id: honor-unsubscribe-and-preserve-security`

### Elevator Pitch
- **Before**: once unsubscribe exists, Ops/Compliance Olivia needs to know it's actually working and how much
  opt-out is happening — but she must **not** be handed a list of who unsubscribed (that's PII she doesn't want
  in `/metrics` or logs).
- **After**: Olivia scrapes `/metrics` and sees suppressed deliveries counted by event —
  e.g. `…{event="workspace_invite",outcome="suppressed"} 34` — so she can see opt-out volume and confirm
  suppression is enforced, with **no** recipient email or token anywhere in the labels.
- **Decision enabled**: Olivia can monitor opt-out volume and prove suppression is working for compliance,
  without ever exposing an individual recipient's choice.

### Problem
Unsubscribe that can't be observed can't be trusted or reported on — but observability that leaks who
unsubscribed is itself a privacy problem. Olivia needs aggregate, PII-free visibility into suppression.

### Who
- **Ops / Compliance Olivia** running Foundry | watches `/metrics` | needs opt-out volume + proof suppression is
  enforced | must never see recipient PII in the process.

### Solution
Count each suppressed suppressible-delivery on the existing observability seam — a `suppressed` outcome on
`foundry_notification_deliveries_total` or a sibling `foundry_notification_suppressions_total{event}` (ODD-5) —
bounded-label, on the shipped `/metrics` sidecar. Labels carry `event` (+ at most `workspace`), **never** the
recipient email or token. The label/cardinality guard fails closed on an unbounded or PII label.

### Domain Examples
1. **Happy path** — after 34 `workspace_invite` suppressions and 8 `member_invite` suppressions, `/metrics`
   shows the suppression count split by `event`; Olivia reads total opt-out volume at a glance.
2. **Edge: no PII** — Olivia greps a full `/metrics` scrape and the delivery/suppression logs; **no** recipient
   email address or unsubscribe token value appears anywhere (NFR-4).
3. **Boundary: mandatory never counted suppressed** — the suppression series for `password_reset` /
   `password_changed` / `member_removed` is always 0 (they can't be suppressed, US-02), which Olivia can verify.

### UAT Scenarios (BDD)
#### Scenario: Suppressed deliveries are visible as a count on /metrics
Given several workspace-invite and member-invite deliveries have been suppressed
When Olivia scrapes /metrics
Then a suppression count is present, split by event
And the counts reflect how many suppressible deliveries were suppressed

#### Scenario: The suppression metric exposes no recipient PII
Given suppressions have occurred for known recipients
When Olivia scrapes /metrics and inspects the logs
Then no recipient email address or unsubscribe token appears in any label or line

#### Scenario: Mandatory events never appear as suppressed
Given recipients are unsubscribed and mandatory events have fired for them
When Olivia inspects the suppression metric
Then the suppressed count for password_reset, password_changed, and member_removed is zero

### Acceptance Criteria
- [ ] A suppressed suppressible-delivery increments a bounded-label suppression count on the existing `/metrics` sidecar (FR-8, NFR-4).
- [ ] Suppression labels carry `event` (∈ the bounded catalog) and at most `workspace` — never recipient email or token.
- [ ] A full `/metrics` scrape and the delivery logs contain no recipient PII for a suppressed delivery.
- [ ] The suppressed count for every mandatory event is always 0 (US-02 invariant, observable).
- [ ] A label/cardinality guard fails closed on an unbounded or PII label (per the shipped bounded-label discipline).

### Outcome KPIs
- **Who**: operators / compliance monitoring Foundry
- **Does what**: observe opt-out volume and confirm suppression is enforced, without seeing who
- **By how much**: 100% of suppressed deliveries counted; 0 recipient-PII occurrences in metrics/labels
- **Measured by**: suppression count vs actual suppressions + a no-PII-in-metrics `@property` litmus
- **Baseline**: 0 — no suppression and no suppression metric exist today

### Technical Notes
- Extends the shipped `foundry_notification_deliveries_total` seam (`notify.rs:39, 291-297`); `suppressed` outcome vs sibling counter is ODD-5; bounded-label per the shipped ADR discipline.
- Depends on US-01 (the suppression event it counts). No new persistence; observability only.
- Guardrail alert thresholds (e.g. a spike in suppression) are a DEVOPS follow-up, not built here.
</content>
