# Journey — Machine-Token Admin Lifecycle (mint → display once → use → revoke)

> Comprehensive admin lifecycle journey for issuing, using, auditing, and revoking machine
> tokens. The crypto + storage substrate is SHIPPED (Feature A); this journey is about the
> NEW admin surface. Persona: **Priya Nandakumar**, workspace admin. Surface-neutral where
> possible (web admin UI vs JSON API is wave-decisions Q6); mockups show the web admin UI as
> the primary human surface with the API entry point noted alongside.

## Goal

Priya can grant an integration programmatic access, hand over the secret safely, see what
credentials exist, and shut down any that should no longer work — all from the product,
without touching env vars, a deploy, or the Postgres console.

## Emotional Arc

| Phase | Emotion | Why |
|-------|---------|-----|
| **Start** | Slightly anxious / wary | Issuing a credential feels risky; today it needs ops + a deploy. "Am I about to leak something?" |
| **Mint** | Focused, careful | The one-time secret demands attention — "copy it now or lose it." Peak tension. |
| **Hand-off + first call** | Relieved, then confident | The integration authenticates with the pasted token; it works. The risk paid off. |
| **List / audit** | In control | A workspace-scoped list of exactly what exists, who issued it, when last used. |
| **Revoke** | Decisive, then reassured | One click; the integration is refused on its next call. Final and immediate. |
| **End** | Trusting | "I can grant access safely and take it back instantly." |

Arc pattern: **Confidence Building** (anxious → focused → confident), with a single deliberate
high-tension moment at the one-time secret display — buffered by an explicit "copy now or
lose it" affordance rather than a jarring surprise.

## Draft Sketch (working hypothesis, refined below)

```
[Trigger: integration       [Mint token]        [Copy secret ONCE]     [Integration calls /api/v1]
 needs API access]    ──►    label/scope/exp ──► shown once, never ──►  bearer token verified
  Feels: wary                Feels: focused      re-shown               Feels: confident
                                                 Feels: careful
                                                      │
                       [List tokens] ◄──────── [Revoke when stale] ──► [Next API call refused]
                        who/scope/status         flip revoked_at        denylist (SHIPPED) refuses
                        Feels: in control         Feels: decisive        Feels: reassured
```

---

## Step 1 — Trigger: an integration needs API access

**Entry emotion**: wary. Priya's CI bot / triage agent needs to call `/api/v1`. Today she
has no product path — she would have to ask ops to set `MACHINE_TOKEN_SIGNING_KEY` and
redeploy. She navigates to the workspace's machine-token admin surface.

```
+-- Machine tokens — Acme workspace --------------------------------------+
|                                                                          |
|  Programmatic credentials that can call the Foundry API on your behalf.  |
|                                                                          |
|  [ + Issue a new token ]                                                 |
|                                                                          |
|  No tokens issued yet.                                                   |
+--------------------------------------------------------------------------+
```

- **Entry point (web)**: an admin-only "Machine tokens" screen under the workspace.
- **Entry point (API, if chosen — Q6)**: `GET /api/v1/workspaces/{ws}/tokens` returns the
  (empty) list; `POST .../tokens` mints.
- **Authz**: the surface is visible only to a workspace admin (`is_workspace_admin`).
- **Failure modes**: a non-admin reaching the URL gets a non-enumerable refusal (404/403 per
  DESIGN), never a leak that the surface exists.

## Step 2 — Mint: issue a new token

**Entry emotion**: focused. Priya names the token, picks a scope and an expiry, and confirms.

```
+-- Issue a new machine token -------------------------------------------+
|                                                                         |
|  Label        [ CI bot — files release issues            ]   (1–128)   |
|  Scope        ( ) Whole workspace                                       |
|               (•) One team:  [ Backend ▾ ]                              |
|  Expires      [ 30 days ▾ ]   (max 90 days)                             |
|                                                                         |
|  The token value is shown ONCE on the next screen and can never be      |
|  retrieved again. Copy it before you leave the page.                    |
|                                                                         |
|                                   [ Cancel ]   [ Issue token ]          |
+-------------------------------------------------------------------------+
```

- **Server-side mint**: on submit, the handler builds `MachineTokenClaims { sub = the
  principal user, scope = Some(team) | None, iat, exp = now + chosen TTL, jti = new uuid,
  iss/aud pinned }` and calls the SHIPPED `MachineTokenSigner::mint(&claims) ->
  SecretString` (requires the signer to be LIVE in AppState — see wave-decisions DM1/Q1).
- **Persist metadata only**: `insert_machine_token(jti, user_id, workspace_id,
  scope_team_id, expires_at, label, created_by = Priya.user_id)`. The `created_by` parameter
  is NEW (DM4). **The token value is NOT stored** — the table has no secret column.
- **Bounds**: the server enforces label length (1–128, the table CHECK), the TTL cap, and a
  default (Q3).
- **Failure modes**: empty/over-long label rejected with inline copy; TTL over the cap
  rejected; chosen team not in the workspace rejected; signer absent → the mint surface is
  not offered (a verifier-only binary cannot issue) — observable as "issuing is not enabled
  on this server" rather than a 500.

## Step 3 — Display once: the secret, shown exactly one time

**Entry emotion**: careful (peak tension). The token value appears ONCE. This is the only
moment it is ever visible.

```
+-- Token issued — copy it now ------------------------------------------+
|                                                                         |
|  ⚠ This is the only time you will see this value. It cannot be          |
|    retrieved later. If you lose it, revoke it and issue a new one.      |
|                                                                         |
|  Token value                                                            |
|  ┌───────────────────────────────────────────────────────────┐ [Copy] |
|  │ eyJhbGciOiJFZERTQSIsInR5cCI6IkpXVCJ9.eyJzdWIiOiI…<once>    │        |
|  └───────────────────────────────────────────────────────────┘        |
|                                                                         |
|  Label    CI bot — files release issues                                 |
|  Scope    Backend team                                                  |
|  Expires  2026-07-06  (30 days)                                         |
|  Token id 0190c3af-…  (the jti — safe to record; not the secret)        |
|                                                                         |
|                                          [ I've copied it — done ]      |
+-------------------------------------------------------------------------+
```

- **Shared artifact — token value (`${token_value}`)**: the `SecretString` returned by
  `mint`. Source of truth: produced in-memory by `MachineTokenSigner::mint`; consumer: this
  ONE screen / the ONE `POST` response body. **NEVER persisted, never logged, never
  re-shown.** Already a `SecretString` (no `Debug`/`Display`) — the NFR forbids any log or
  re-display path.
- **Shared artifact — `jti` (`${jti}`)**: the token id. Source of truth:
  `machine_tokens.jti` (the row PK). Safe to display and record; appears here, in the list
  view, and is the revoke key.
- **API equivalent (Q6)**: `POST .../tokens` returns `201` with a body carrying
  `{ token: "<once>", jti, label, scope, expires_at }`. The `token` field is in the response
  body ONLY; no GET ever returns it.
- **Failure modes**: the admin navigates away / closes the tab before copying → the value is
  gone (by design); recovery is Step 6 (revoke + reissue). A render error must be
  all-or-nothing (never a half-page that leaks a partial token).

## Step 4 — Hand-off + first call: the integration authenticates

**Entry emotion**: relieved → confident. Priya (or Marco, the integration owner) pastes the
token into the integration config; the integration calls `/api/v1` with
`Authorization: Bearer <token>`.

```
$ curl -H "Authorization: Bearer eyJhbGciOiJFZERTQSI…" \
       https://foundry.acme.dev/api/v1/issues
HTTP/1.1 200 OK
[ … issues … ]
```

- **Verification path (SHIPPED)**: `MachineTokenVerifier::verify` (EdDSA-pinned, iss/aud
  pinned) recovers the claims; the per-request denylist checks
  `find_machine_token_by_jti(jti).revoked_at IS NULL`; `touch_machine_token_last_used(jti)`
  updates `last_used_at`. **No new verification code — this already works.**
- **Failure modes**: expired token → refused by `Validation` before any DB hit; wrong
  scope/principal → authorization refused downstream; revoked token → refused (Step 6).

## Step 5 — List / audit: see what exists

**Entry emotion**: in control. Priya returns to the machine-token screen and sees the
workspace's tokens, newest first (`list_machine_tokens`).

```
+-- Machine tokens — Acme workspace --------------------------------------+
|  [ + Issue a new token ]                                                |
|                                                                          |
|  Label                       Scope     Expires     Minted by   Last used  Status   |
|  ─────────────────────────── ───────── ─────────── ────────── ────────── ──────── |
|  CI bot — files release iss… Backend   2026-07-06  Priya N.   2 min ago  ● Active  [Revoke] |
|  Slack relay                 Workspace 2026-06-20  Priya N.   1 day ago  ● Active  [Revoke] |
|  Old triage agent            Workspace 2026-05-01  Dana W.    —          ⨯ Revoked          |
+--------------------------------------------------------------------------+
```

- **Shared artifacts shown**: `label`, scope (workspace vs `scope_team_id` → team name),
  `expires_at`, `created_by` → "minted by {admin}" (NEW, DM4), `last_used_at`, and status
  derived from `revoked_at` (`NULL` = Active, else Revoked). **The token value is absent —
  there is no column and no API field that returns it.**
- **API equivalent (Q6)**: `GET .../tokens` returns the same metadata array; no `token`
  field.
- **Failure modes**: a workspace with zero tokens shows the empty state; a row whose
  `created_by` is NULL (a pre-feature row, if any) shows "minted by —".

## Step 6 — Revoke: shut down a credential

**Entry emotion**: decisive → reassured. Priya clicks Revoke on the stale Slack relay;
confirms; the row flips to Revoked, and the integration is refused on its next call.

```
+-- Revoke "Slack relay"? -----------------------------------------------+
|                                                                         |
|  Any integration using this token will be refused on its NEXT API       |
|  request. This cannot be undone — to restore access, issue a new token. |
|                                                                         |
|                                   [ Keep it ]   [ Revoke now ]          |
+-------------------------------------------------------------------------+
```

- **Revoke path (SHIPPED)**: `revoke_machine_token(jti)` → `SET revoked_at = now()`. The row
  SURVIVES (not deleted) so the per-request denylist keeps refusing until expiry/GC.
- **Effective immediately**: the very next `/api/v1` call presenting that token is refused —
  the denylist is checked per-request, with no token cache (mt-job-2 anxiety resolved).
- **API equivalent (Q6)**: `DELETE .../tokens/{jti}` (or `POST .../tokens/{jti}/revoke`) →
  `204`; idempotent (revoking an already-revoked token is a no-op success).
- **Shared artifact — `jti`**: the revoke key; must match the row's PK exactly.
- **Failure modes**: revoke a non-existent/other-workspace `jti` → not found, non-enumerable;
  revoke an already-revoked token → idempotent success; concurrent revoke by two admins →
  both observe Revoked (last write is still `revoked_at = now()`).

---

## Shared Artifacts (tracked across steps)

| Artifact | Source of truth | Displayed as | Consumers (steps) | Integration risk |
|----------|-----------------|--------------|-------------------|------------------|
| Token value | `MachineTokenSigner::mint` (in-memory `SecretString`) | `${token_value}` | Step 3 mint screen / `POST` response body ONLY | **HIGH** — must be shown once, never persisted, never logged, never re-shown. Already `SecretString` (no Debug/Display). |
| `jti` (token id) | `machine_tokens.jti` (row PK) | `${jti}` | Steps 3, 5, 6 (revoke key) | MEDIUM — safe to display; must match exactly for revoke. |
| Label | `machine_tokens.label` | `${label}` | Steps 2, 3, 5, 6 | LOW — 1–128 char CHECK enforced by the table. |
| Scope | `machine_tokens.scope_team_id` (NULL = workspace) | "Workspace" / "{team}" | Steps 2, 3, 5 | MEDIUM — team must belong to the workspace. |
| Expiry | `machine_tokens.expires_at` (mirrors claim `exp`) | `${expires_at}` | Steps 2, 3, 5 | MEDIUM — server caps + defaults (Q3). |
| Minted by | `machine_tokens.created_by` (NEW column, DM4) | "minted by {admin}" | Steps 2 (write), 5 | LOW — nullable; pre-feature rows show "—". |
| Last used | `machine_tokens.last_used_at` (touched on use) | `${last_used_at}` | Step 5 | LOW — operational visibility only. |
| Status | derived from `machine_tokens.revoked_at` | "Active"/"Revoked" | Steps 5, 6 | MEDIUM — single source: the per-request denylist reads the same column. |

## Integration Checkpoints

1. **Signer present ⇔ mint offered.** The mint surface (Step 2/3) is only reachable on a
   binary whose `AppState` holds a `MachineTokenSigner` (DM1). A verifier-only binary shows
   "issuing not enabled here", not a 500.
2. **One source for status.** The list-view "Active/Revoked" (Step 5) and the per-request
   refusal (Step 6) both derive from `machine_tokens.revoked_at` — no second source.
3. **No secret leaves the mint moment.** Grep the whole flow: the token value appears only in
   `mint`'s `SecretString` and the one Step-3 surface. No log line, no list field, no GET,
   no DB column carries it.
4. **`jti` round-trips.** The `jti` shown at mint (Step 3) is the same one listed (Step 5)
   and the same one revoked (Step 6) and the same one the denylist checks (Step 4/6).

## Error & Recovery Paths (summary)

| Failure | Where | What the admin sees | Recovery |
|---------|-------|---------------------|----------|
| Lost the token before copying | Step 3→4 | (nothing — value is gone by design) | Revoke the lost `jti`, issue a new token. |
| Empty / over-long label | Step 2 | Inline "Label must be 1–128 characters." | Fix and resubmit. |
| TTL over the cap | Step 2 | Inline "Maximum expiry is {cap} days." | Pick a shorter expiry. |
| Issuing not enabled (verifier-only binary) | Step 2 | "Issuing tokens is not enabled on this server." | Operator enables the signer (DESIGN). |
| Non-admin reaches the surface | Steps 1/2/5/6 | Non-enumerable refusal (404/403). | N/A — by design. |
| Revoked token still tried | Step 4/6 | The integration gets a 401/403 on its next call. | Issue a new token if access is still wanted. |
| Revoke a stale/wrong `jti` | Step 6 | Idempotent success (already revoked) or non-enumerable not-found (other workspace). | N/A. |
