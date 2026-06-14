# Journey (visual): Invite Accept — first-admin claims their account

> Feature: `invite-accept-flow` | Persona: **Priya Nair**, the first-admin of a freshly provisioned workspace
> Goal: turn the emitted `/invites/accept?id=…&sig=…` link from a DEAD URL into a real
> claim-your-account flow — verify token → set password → consume invite (single-use) → signed in, landed on the workspace.
> Scope (v1, user-ratified): **first-admin invites only**. General workspace-member invites are a later feature.

## The persona, concretely

**Priya Nair** (`priya.nair@northwind.example`) was just provisioned a workspace by her instance
super-admin, **Dan Okoro**, who ran the browser provision form (or `foundry doctor provision-workspace`).
Dan pasted Priya the link, or she got the invite email:

```
https://foundry.northwind.example/invites/accept?id=018f...c3&sig=Yk9...wQ
This link is valid for 7 days.
```

Priya has never signed into Foundry. The provisioning step created her admin row with a password hash
**she has never seen**. This link is the ONLY way she can establish a credential and get in.

## Emotional arc (Problem Relief + Confidence Building)

```
ARRIVAL                  SET PASSWORD              CONSUME + SIGN IN          LANDED
"Is this legit?"   -->   "OK, I'm choosing    -->  "...did it take?"    -->   "I'm in. This is
 cautious trust          my own password"          brief suspense              MY workspace."
                         growing confidence                                    relief + ownership
```

The peak tension is the moment of submit (consume + sign-in is atomic and invisible). The design must
collapse that suspense fast: a successful submit lands her ON the workspace, no second login step.

The SAD paths must NOT spike anxiety into confusion. An expired / used / tampered link is the most
likely failure (links get forwarded, re-clicked, sat on for >7 days). The message must be calm,
honest, and actionable ("ask your instance admin to re-issue") WITHOUT leaking whether the account
or workspace exists.

---

## Happy path — ASCII flow

```
[Trigger]                [Step 1: ARRIVE]            [Step 2: SET PASSWORD]        [Step 3: LAND]
Priya clicks the     -->  GET /invites/accept    -->  POST /invites/accept    -->  302 -> /  (her
invite link              ?id=${invite_id}             id + sig + new password       workspace dashboard)
                         &sig=${signature}            + _csrf
  Feels: cautious          Sees: set-password           Sees: brief submit            Sees: "Welcome,
         trust                  form, workspace                (no second login)            Priya" + her
                                name for context                                            workspace data
  Artifacts:               Artifacts:                   Artifacts:                    Artifacts:
   ${invite_id}             ${invite_id} (hidden)        ${invite_id} consumed         ${session} cookie
   ${signature}             ${signature} (hidden)        ${invites_row}.used_at set    ${workspace_id}
                            ${csrf_token}                ${password_hash} written      resolved
```

### Step 1 — Arrive: GET /invites/accept (verify token, render set-password form)

```
+-- GET /invites/accept?id=018f...c3&sig=Yk9...wQ -------------------+
|                                                                    |
|   Foundry                                                          |
|   ----------------------------------------------------------       |
|   Set your password to join                                        |
|   "${workspace_name}"                          <- context, builds  |
|                                                   trust on arrival |
|   You've been invited as the first administrator.                  |
|                                                                    |
|   New password   [______________________________]                 |
|   Confirm        [______________________________]                 |
|                                                                    |
|   <hidden> id  = ${invite_id}                                      |
|   <hidden> sig = ${signature}                                     |
|   <hidden> _csrf = ${csrf_token}                                  |
|                                                                    |
|                                   [ Set password & sign in ]       |
|                                                                    |
+--------------------------------------------------------------------+
```

Verification on GET is **non-committal**: it checks the HMAC binds (`id`||`expires_at`), looks up the
invites row, and confirms it is unexpired AND unconsumed. If any check fails -> the uniform
non-enumerable refusal page (below). The form is rendered ONLY for a live, valid invite.

### Step 2 — Set password: POST /invites/accept (atomic consume + set password + session)

```
+-- POST /invites/accept ---------------------------------------------+
|   (server, invisible to user — happens in <1s)                      |
|                                                                     |
|   1. re-verify signature binds id||expires_at      (defense-depth)  |
|   2. BEGIN TX                                                       |
|        consume_invite(id): mark used_at=now WHERE used_at IS NULL   |
|          AND expires_at > now   -- single-use, atomic, race-safe    |
|        -> 0 rows updated? ABORT -> uniform refusal (lost the race / |
|                                     already used / expired)         |
|        write ${password_hash} onto the first-admin user row         |
|      COMMIT                                                         |
|   3. establish ${session} for the first-admin user                  |
|   4. resolve_active_workspace(user) -> ${workspace_id}              |
+---------------------------------------------------------------------+
                                  |
                                  v
                         302 Found -> "/"
```

### Step 3 — Land: signed in on the workspace

```
+-- GET / (now authenticated) ---------------------------------------+
|   Foundry                                          Priya Nair  v    |
|   ----------------------------------------------------------       |
|   ${workspace_name}                                                 |
|                                                                    |
|   Welcome, Priya. This is your workspace.                          |
|   [ Projects ]  [ Issues ]  [ Team ]                               |
|                                                                    |
|   (Priya sees ONLY this workspace's data — resolve_active_workspace |
|    membership seam; no other tenant is visible)                    |
+--------------------------------------------------------------------+
```

---

## Sad / error paths — first-class

All four refusal paths render the **SAME non-enumerable page** (uniform copy, no account/workspace
existence leak). They differ only in internal `tracing`, never in the user-visible body.

```
+-- Uniform refusal (expired | used | invalid sig | unknown id | tampered) ---+
|                                                                             |
|   This invite is no longer valid                                            |
|   ----------------------------------------------------------                |
|   This invitation link can't be used. It may have expired, already          |
|   been used, or been mistyped.                                              |
|                                                                             |
|   Ask your instance administrator to re-provision your workspace or          |
|   re-issue the invitation.                                                  |
|                                                                             |
|   (No "account not found" / "workspace X" / "user Y" — nothing that         |
|    confirms or denies that any account or workspace exists.)                |
+-----------------------------------------------------------------------------+
```

| # | Sad path | Trigger | What Priya sees | Emotional handling |
|---|----------|---------|-----------------|--------------------|
| E1 | **Expired** | clicked after 7 days | uniform refusal | calm, "ask admin to re-issue" — not her fault |
| E2 | **Already used** | re-clicked / forwarded a consumed link | uniform refusal | same copy — no "you already did this" oracle |
| E3 | **Invalid / tampered sig** | `sig` altered, or `id` swapped | uniform refusal | same copy — attacker learns nothing |
| E4 | **Unknown id** | random / probed id | uniform refusal | byte-identical to E1/E2/E3 — non-enumerable |
| E5 | **Weak password** | password below policy on POST | inline form error, form re-renders WITH a valid live invite (invite NOT consumed) | gentle correction, keeps momentum |
| E6 | **Passwords don't match** | confirm != new | inline form error, invite NOT consumed | gentle correction |
| E7 | **Lost the consume race** | two concurrent POSTs for one invite | exactly one wins -> lands; the other gets uniform refusal | the winner never notices; loser gets calm refusal |
| E8 | **Missing/invalid CSRF** | forged/stale POST | refused by shipped CSRF middleware (no consume, no password write) | invisible to a real browser |

> Note E5/E6 are **recoverable inline** (the invite stays live — set-password can be retried), which is
> why they are NOT the uniform refusal. E1-E4, E7, E8 are terminal for that link.

---

## CLI parity note

The same `/invites/accept` link is emitted by `foundry doctor provision-workspace` (CLI) and the web
provision success fragment. This feature makes the link live on the **web** surface; because the link
is a URL, fixing the web route fixes the dead link for BOTH emit sites at once (the CLI emits a web URL,
it does not need its own accept handler). A CLI-native `foundry invite accept` TUI is explicitly OUT of
v1 scope (see Open decisions).

## Integration checkpoints

1. The `${signature}` rendered into the form's hidden field on GET must be the SAME value that POST
   re-verifies — single source: the `sig` query param, bound to `${invite_id}` by `InviteToken::verify`.
2. The `${invite_id}` consumed by the TX must be the same row `insert_invite` created during provisioning.
3. The `${workspace_id}` Priya lands on must equal `invites.workspace_id` for the consumed invite,
   resolved through `resolve_active_workspace` — she must never land on another tenant.
4. The uniform-refusal body must be byte-identical across E1-E4 (a litmus test must RED if the arms diverge).
