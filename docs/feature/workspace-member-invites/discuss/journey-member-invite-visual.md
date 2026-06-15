# Journey (visual): Workspace Member Invites — an admin invites a teammate; the teammate joins

> Feature: `workspace-member-invites` | Personas: **Dana Reyes** (workspace ADMIN, the inviter) and
> **Sam Okafor** (the invitee — has NO Foundry account yet)
> Goal: generalize the shipped first-admin `/invites/accept` flow to GENERAL workspace members. Two
> capabilities: (1) a workspace admin issues a member invite (creates an `invites` row + emits a signed
> `/invites/accept?id=…&sig=…` link, reusing `InviteToken`); (2) the invitee opens the link, sets a
> password, and JOINS — but unlike the first-admin case (where the user pre-exists as `created_by`), the
> member invitee has no account, so accept must CREATE the user + ADD a member-role membership + set
> password in ONE atomic tx, then auto-sign-in onto the workspace.
> Scope (v1, recommended defaults): **member role only**. Inviting as admin, bulk invites, and
> revocation/resend are deferred follow-ups.

## Why this is a thin generalization, not greenfield

The shipped `invite-accept-flow` (see `docs/feature/invite-accept-flow/`) already built: the signed
`InviteToken` (HMAC binds `invite_id`||`expires_at`), the `invites` table with `used_at`/`used_by`
single-use markers, the atomic guarded consume (`set_first_admin_password_and_consume`), the
non-committal GET, the uniform non-enumerable `invite_refusal_page()`, the min-12 `check_password_policy`,
the CSRF-on-public-POST seam, the session + `resolve_active_workspace` auto-sign-in, and the inline
recovery for weak/mismatched passwords. This feature REUSES all of it. Only two genuinely new things:

1. **An issuance surface** (the first-admin invite was minted by `provision_workspace`; there was no
   admin-facing issuance UI). New: a workspace-admin web form, `is_workspace_admin`-gated, CSRF-protected,
   non-enumerable for non-admins.
2. **An accept tx that CREATES the user** (the shipped consume writes the password onto the
   pre-existing `created_by` row). New: a `create_member_and_consume` tx that, on the same atomic 0-or-1
   row guard, creates the user + adds a `member`-role `workspace_memberships` row + sets the password.

## The personas, concretely

**Dana Reyes** (`dana.reyes@northwind.example`) is the admin of the "Northwind" workspace. Her teammate
**Sam Okafor** (`sam.okafor@northwind.example`) needs in. Dana opens the workspace member-invite form,
types Sam's email, and clicks "Send invite". The system mints an invite and emits/sends:

```
https://foundry.northwind.example/invites/accept?id=018f...d7&sig=Qp4...nK
This link is valid for 7 days.
```

Sam has never signed into Foundry — he has no account at all. This link is the ONLY way he establishes
a credential and joins Northwind.

## Emotional arc

Two arcs, one per persona.

### Inviter (Dana) — Confidence Building
```
INTENT                   FILL FORM                CONFIRM                   DONE
"I need to add      -->  "type Sam's email"  -->  "did it send?"      -->   "Sam's on the way in.
 Sam to Northwind"        low friction             brief suspense            I'm done — no IT ticket."
 mild task-focus          growing confidence                                 relief + control
```

### Invitee (Sam) — Problem Relief + Confidence Building (mirrors the shipped first-admin arc)
```
ARRIVAL                  SET PASSWORD              JOIN + SIGN IN            LANDED
"Is this legit? I   -->  "OK, my own         -->  "...did it take?"   -->   "I'm in Northwind.
 don't even have an       password"               brief suspense            I have a real account now."
 account"                 growing confidence       (atomic, invisible)       relief + belonging
 cautious trust
```

The inviter's peak tension is the send-confirm moment; collapse it with an immediate "invite sent"
fragment showing the emitted link (so Dana can also paste it out-of-band). The invitee's peak tension is
submit (create-user + membership + password + sign-in is one atomic, invisible step); a successful submit
lands Sam ON Northwind, no second login.

The SAD paths must stay calm. A non-admin who probes the issuance route gets a non-enumerable 404 (the
route does not even admit to existing). A bad accept link (expired/used/tampered/unknown) gets the
byte-identical uniform refusal — the security crux, reused verbatim.

---

## Capability 1 — Issuance: a workspace admin invites a member

```
[Trigger]                  [Step I1: OPEN FORM]          [Step I2: SEND]
Dana wants to add     -->  GET /workspace/invites    --> POST /workspace/invites
Sam to Northwind           (admin-gated form)             email + _csrf
  Feels: task-focus          Sees: "Invite a member        Sees: "Invite sent to
                                    to Northwind" form            sam.okafor@…" + the link
  Artifacts:                 Artifacts:                    Artifacts:
   (none yet)                 ${csrf_token}                 ${invite_id} (new row)
                              ${workspace_id} (session)     ${signature}, ${invite_url}
                                                            ${invitee_email} stored
```

### Step I1 — Open the member-invite form: GET /workspace/invites

```
+-- GET /workspace/invites  (admin-gated) ---------------------------+
|                                                                    |
|   Foundry — Northwind                          Dana Reyes  v       |
|   ----------------------------------------------------------       |
|   Invite a member to "${workspace_name}"                           |
|                                                                    |
|   Email   [ sam.okafor@northwind.example__________ ]               |
|                                                                    |
|   They'll get a link to set a password and join as a member.       |
|                                                                    |
|   <hidden> _csrf = ${csrf_token}                                   |
|                                            [ Send invite ]         |
|                                                                    |
+--------------------------------------------------------------------+
```

The GET is gated by `is_workspace_admin(${workspace_id}, ${user_id})`. A signed-in NON-admin (a plain
member) — OR a signed-out caller — gets the **non-enumerable 404** (the same `not_found()` posture the
shipped `/admin/tokens` and `/admin/instance/…` surfaces use): the route does not reveal it exists.

### Step I2 — Send the invite: POST /workspace/invites

```
+-- POST /workspace/invites  (admin-gated, CSRF-enforced) -----------+
|   1. resolve signed-in admin; is_workspace_admin? else 404         |
|   2. validate email present + shape                                |
|   3. invite_id = uuid v7; expires_at = now + 7 days                |
|   4. insert_invite(invite_id, ${workspace_id}, ${invitee_email},   |
|        created_by=${admin_user_id}, expires_at)                    |
|   5. InviteToken::new(invite_id, expires_at, SESSION_SECRET)       |
|   6. invite_url = {public_url}/invites/accept?id=…&sig=…           |
|   7. email.send(invitee_email, invite_url)  (best-effort)          |
+--------------------------------------------------------------------+
                                  |
                                  v
+-- "Invite sent" fragment -----------------------------------------+
|   Invite sent to sam.okafor@northwind.example.                     |
|   Or share this link:  https://…/invites/accept?id=018f…d7&sig=…   |
|   Valid for 7 days.                                                |
+--------------------------------------------------------------------+
```

This mirrors the shipped `bootstrap::create_invite` issuance body almost exactly — the deltas are: the
admin gate (`is_workspace_admin`, not just signed-in) and the workspace-scoped form surface.

---

## Capability 2 — Acceptance: the invitee joins (CREATES the account)

```
[Trigger]                 [Step A1: ARRIVE]            [Step A2: SET PASSWORD/JOIN]     [Step A3: LAND]
Sam clicks the       -->  GET /invites/accept     -->  POST /invites/accept        --> 303 -> / (Northwind
member invite link        ?id=${invite_id}             id + sig + new password          dashboard, signed in)
                          &sig=${signature}            + _csrf
  Feels: cautious           Sees: set-password           Sees: brief submit              Sees: "Welcome,
         trust                    form, "join Northwind"       (no second login)               you're in Northwind"
  Artifacts:                Artifacts:                   Artifacts:                      Artifacts:
   ${invite_id}             ${invite_id} (hidden)        ${user_id} CREATED              ${session} cookie
   ${signature}            ${signature} (hidden)         ${membership} (member) ADDED    ${workspace_id} resolved
                           ${csrf_token}                 ${invites_row}.used_at set
                                                         ${password_hash} written
```

### Step A1 — Arrive: GET /invites/accept (reused verbatim from the shipped flow)

```
+-- GET /invites/accept?id=018f...d7&sig=Qp4...nK -------------------+
|                                                                    |
|   Foundry                                                          |
|   ----------------------------------------------------------       |
|   Set your password to join                                        |
|   "${workspace_name}"                          <- context, builds  |
|                                                   trust on arrival |
|   You've been invited to join as a member.                         |
|                                                                    |
|   New password   [______________________________]                 |
|   Confirm        [______________________________]                 |
|                                                                    |
|   <hidden> id=${invite_id}  sig=${signature}  _csrf=${csrf_token}  |
|                                   [ Set password & join ]          |
|                                                                    |
+--------------------------------------------------------------------+
```

This is the SAME `show_accept_form` handler + `InviteAcceptPage` template the shipped flow uses; the only
copy change is "join as a member" vs "first administrator" (a display nuance — the handler does not need
to know the role to render). Verification on GET is non-committal: verify the HMAC binds
`id`||`expires_at`, look up the invites row, confirm unexpired AND unconsumed. Any failure → uniform
refusal. NO state mutation on GET.

### Step A2 — Set password / join: POST /invites/accept (NEW atomic create-user tx)

```
+-- POST /invites/accept (server, <1s, invisible) -------------------+
|   1. re-verify sig binds id||expires_at        (defense-in-depth)  |
|   2. confirm-match + min-12 policy  (BEFORE consume; rejected      |
|        password leaves the invite live — reused US-03 path)        |
|   3. hash_password (argon2id, reused)                              |
|   4. BEGIN TX  — create_member_and_consume(id, email, hash, now)   |
|        consume guard: UPDATE invites SET used_at=now, used_by=NEW  |
|          WHERE id=$1 AND used_at IS NULL AND expires_at > now      |
|          RETURNING workspace_id, invitee_email                     |
|          -> 0 rows? ABORT -> uniform refusal                       |
|        create users row (email from invite, password_hash)         |
|          -> email-already-a-user? (see OD-1) -> ABORT -> refusal   |
|        INSERT workspace_memberships(workspace_id, user_id,         |
|          role='member')                                           |
|        UPDATE invites SET used_by = new user_id                   |
|      COMMIT                                                        |
|   5. establish ${session} for the new member user                  |
|   6. resolve_active_workspace(user) -> ${workspace_id}            |
+--------------------------------------------------------------------+
                                  |
                                  v
                         303 See Other -> "/"
```

This is the genuinely-new logic. The shipped `set_first_admin_password_and_consume` writes the password
onto `created_by` (the pre-existing admin). The member case has NO pre-existing user, so the new
`create_member_and_consume` tx CREATES the user and the membership inside the SAME atomic guard. The
consume guard (0-rows-or-1) is identical to the shipped one — single-use, race-safe, expiry-checked
inside the tx.

### Step A3 — Land: signed in on the workspace as a member (reused verbatim)

```
+-- GET / (now authenticated) ------------------------ Sam Okafor v -+
|   Northwind                                                        |
|   ----------------------------------------------------------       |
|   Welcome, Sam. You're a member of Northwind.                      |
|   [ Projects ]  [ Issues ]  [ Team ]                               |
|                                                                    |
|   (Sam sees ONLY Northwind's data — resolve_active_workspace       |
|    membership seam; member role, not admin)                       |
+--------------------------------------------------------------------+
```

Same `resolve_active_workspace` landing as the shipped flow. Sam's membership is `member`, so he sees
Northwind but does not get the admin-only surfaces (e.g. `/workspace/invites` returns 404 to him —
issuance is admin-gated).

---

## Sad / error paths — first-class

### Issuance sad paths

```
+-- Non-enumerable 404 (non-admin or signed-out hits issuance) ------+
|   Not Found                                                        |
|   (byte-identical to any unknown route; the issuance surface does  |
|    not admit to existing for a non-admin — mirrors /admin/tokens)  |
+--------------------------------------------------------------------+
```

| # | Sad path | Trigger | What they see | Handling |
|---|----------|---------|---------------|----------|
| I-E1 | **Non-admin issuance** | a plain member opens/POSTs `/workspace/invites` | non-enumerable 404 | the route does not reveal it exists (mirrors `/admin/tokens` `not_found()`) |
| I-E2 | **Signed-out issuance** | a signed-out caller hits the route | non-enumerable 404 (indistinguishable from I-E1) | no "sign in to invite" oracle |
| I-E3 | **Missing/invalid email** | empty or malformed email on POST | inline form error, NO invite created | gentle correction, keeps Dana in flow |
| I-E4 | **Missing/invalid CSRF** | forged/stale POST | refused by shipped CSRF middleware, no invite created | invisible to a real browser |
| I-E5 | **Email send fails** | SMTP down | invite still created; the link is shown for manual paste (best-effort email, like shipped `create_invite`) | Dana can still copy the link |

### Acceptance sad paths (the security crux — reused verbatim from the shipped flow)

All four refusal paths render the **SAME** non-enumerable `invite_refusal_page()` (200 OK, byte-identical
body). They differ only in internal `tracing`.

```
+-- Uniform refusal (expired | used | invalid sig | unknown id) ------+
|   This invite is no longer valid                                    |
|   ----------------------------------------------------------        |
|   It may have expired, already been used, or been mistyped.         |
|   Ask your workspace administrator to re-issue the invitation.      |
|   (No account/workspace/invite existence leak.)                     |
+---------------------------------------------------------------------+
```

| # | Sad path | Trigger | What Sam sees | Handling |
|---|----------|---------|---------------|----------|
| A-E1 | **Expired** | clicked after 7 days | uniform refusal | "ask your admin to re-issue" — not his fault |
| A-E2 | **Already used** | re-clicked / forwarded a consumed link | uniform refusal | no "you already joined" oracle |
| A-E3 | **Invalid/tampered sig** | `sig` altered or `id` swapped | uniform refusal | attacker learns nothing |
| A-E4 | **Unknown id** | random / probed id | uniform refusal — byte-identical to A-E1..3 | non-enumerable |
| A-E5 | **Weak password** | below min-12 on POST | inline form error; invite NOT consumed, NO user created | reused US-03 inline recovery |
| A-E6 | **Passwords don't match** | confirm != new | inline form error; invite NOT consumed | reused US-03 inline recovery |
| A-E7 | **Lost the consume race** | two concurrent POSTs for one invite | exactly one creates-user+joins; the other gets uniform refusal; user created exactly once | winner never notices |
| A-E8 | **Missing/invalid CSRF** | forged/stale POST | refused by shipped CSRF middleware; no consume, no user, no password write | invisible to a real browser |
| A-E9 | **Email already a user** | invitee email already maps to an existing Foundry user | **uniform refusal** (v1, OD-1 recommended): the create-user step aborts the tx, invite is NOT consumed | non-enumerable; multi-workspace-membership-via-invite deferred |

> A-E5/A-E6 are recoverable inline (invite stays live). A-E1..4, A-E7, A-E8 are terminal for that link.
> A-E9 is the new branch — see Open decisions OD-1 (recommended: refuse non-enumerably and defer).

---

## Integration checkpoints

1. **Issuance → accept handoff**: the `${invite_id}` Dana's POST creates (with `${invitee_email}`,
   `${workspace_id}`, `created_by=${admin_user_id}`) is the SAME row Sam's accept consumes. The
   `${signature}` emitted by issuance must be the one accept re-verifies.
2. **GET sig == POST sig**: the `${signature}` rendered into the form's hidden field on GET must equal
   the value POST re-verifies — single source: the `sig` query param bound to `${invite_id}` by
   `InviteToken::verify` (reused).
3. **New user joins the RIGHT workspace**: the `${membership}` created on accept must reference
   `invites.workspace_id`; Sam must land on, and see ONLY, that tenant via `resolve_active_workspace`.
4. **Atomic create+join+consume**: the create-user, the member-membership insert, and the consume
   `used_at` mark are in ONE transaction — none happen without the others (mirrors the shipped
   consume+password-write atomicity).
5. **Uniform refusal byte-identity**: the acceptance refusal body must be byte-identical across
   A-E1..A-E4 AND A-E9 (a litmus must RED if any arm diverges) — reused invariant.
6. **Issuance non-enumerability**: I-E1 (non-admin) and I-E2 (signed-out) must be byte-identical to a
   generic 404 — the issuance route reveals nothing about its own existence to a non-admin.

## CLI parity note

Like the shipped flow, the emitted link is a web URL — making the web accept route handle member invites
fixes the link for any emit site. A CLI-native `foundry workspace invite <email>` issuance command is
explicitly OUT of v1 scope (the web admin form is the v1 issuance surface; CLI issuance is a deferred
follow-up alongside revocation/resend).
