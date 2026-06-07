<!-- markdownlint-disable MD024 -->
# Machine-Token Admin UX — User Stories

> This feature adds the **product surface** for a workspace admin to mint, view, and revoke
> machine tokens. The crypto + storage + verify substrate SHIPPED in Feature A
> (`web-tier-extraction`): `MachineTokenSigner::mint`, the `machine_tokens` registry + `jti`
> revocation denylist, `MachineTokenVerifier`, and `is_workspace_admin`. Feature A DEFERRED
> the admin surface and the `created_by` audit column — this feature is that issuer
> call-site. Every story is solution-neutral on the things DESIGN owns (signing-key-at-rest
> mechanism, surface UI/API choice, exact TTL numbers, scope vocabulary). Personas and jobs:
> see `jobs.yaml`. The 6 open questions: see `wave-decisions.md`.

## System Constraints (cross-cutting)

Apply to every story; measurable forms live in `nfrs.md`.

- **Reuse, don't rebuild, the substrate.** Minting calls the SHIPPED
  `MachineTokenSigner::mint`; persistence calls the SHIPPED `machine_tokens` repo
  (`insert_machine_token` — gaining a `created_by` param —, `list_machine_tokens`,
  `revoke_machine_token`, `find_machine_token_by_jti`, `touch_machine_token_last_used`);
  verification + the per-request denylist are unchanged (`MachineTokenVerifier`); authz reuses
  `is_workspace_admin`.
- **The token value is shown EXACTLY ONCE at mint and is NEVER persisted, logged, or
  re-displayed.** The registry stores only `jti` + metadata (the table has no secret column,
  by design). The minted JWT is already a `SecretString` (no `Debug`/`Display`).
- **Revocation is a flag** (`revoked_at`), effective on the NEXT API request via the SHIPPED
  per-request denylist; the row survives revocation until expiry/GC.
- **Minting, listing, and revoking are workspace-admin-only** (`is_workspace_admin`);
  non-admins get a non-enumerable refusal.
- **Signer present ⇔ mint offered.** Issuing requires a live `MachineTokenSigner` in
  `AppState` (NEW posture, DM1/Q1); a verifier-only binary surfaces "issuing not enabled
  here", never a 500.
- **One binary, no new runtime services.** The admin UI (if chosen) preserves the existing
  browser auth/CSRF/session contract unchanged.
- **Solution-neutral.** Signing-key-at-rest mechanism, surface (UI/API), TTL numbers, and
  scope vocabulary are DESIGN.

## Glossary (additions for this feature)

- **Machine token**: an Ed25519-signed JWT presented as a bearer credential to `/api/v1`.
  The JWT itself IS the secret; the server verifies it offline.
- **Mint / issue**: produce a new signed token via `MachineTokenSigner::mint` and record its
  metadata in `machine_tokens`.
- **Registry**: the `machine_tokens` table — issuance metadata + a revocation flag, with NO
  secret/hash column.
- **Revoke**: set `machine_tokens.revoked_at`; the per-request denylist then refuses the
  `jti` on its next use.
- **`jti`**: the token id (row PK); safe to display; the revoke key.
- **`created_by`**: NEW column (this feature) recording the admin who minted a token.
- **Scope**: `machine_tokens.scope_team_id` — `NULL` = workspace-wide, else one team.

---

# ==========================================================================
# Slice 1 — Walking Skeleton: mint ONE token end-to-end, value shown once
# US-MT00 (signer-in-AppState + created_by migration, @infrastructure) + US-MT01 (mint)
# ==========================================================================

## US-MT00: Make the server able to issue tokens and to record who issued them

- **job_id**: infrastructure-only
- **infrastructure_rationale**: This story enables no user decision on its own — it is the
  substrate two user-visible jobs stand on. (1) Minting-through-the-product requires a
  `MachineTokenSigner` (the Ed25519 PRIVATE key) live in `AppState`; today `AppState` holds
  only `machine_token_verifier` and the signing key is read transiently at boot for the
  self-test. (2) Attributing issuance requires re-introducing the `created_by` column Feature
  A deferred. Neither is observable to a user by itself; both are folded into Slice 1 and
  never ship standalone. The user-visible outcome they enable is US-MT01 (mint a token).

### Problem
The running process is a verifier, not an issuer: `AppState.machine_token_verifier` exists,
but there is no signer, so no handler can call `MachineTokenSigner::mint`. And the
`machine_tokens` table has no `created_by` column (Feature A deferred it: "no issuer
call-site existed", confirmed in migration `0007_machine_tokens.sql` and the
`insert_machine_token` signature). Until both are addressed, the product cannot mint a token
or say who minted it.

### Who
- **The platform** (no direct user). Enables Priya (US-MT01) and Dana (US-MT06).
- **Context**: `crates/foundry-app/src/lib.rs` `AppState` (verifier-only today);
  `crates/foundry-store/migrations/0007_machine_tokens.sql` (no `created_by`);
  `insert_machine_token(jti, user_id, workspace_id, scope_team_id, expires_at, label)`.
- **Motivation**: a safe, attributable issuer call-site.

### Solution
(1) Place a `MachineTokenSigner` in `AppState` so issuer binaries can mint on demand
(mechanism — how the key is loaded/guarded, whether issuer capability is a separate
binary/config mode — is DESIGN, Q1). A verifier-only binary has no signer and offers no mint
surface. (2) Add a forward-only migration introducing `created_by UUID REFERENCES users(id)`
(nullable; pre-existing rows back-fill NULL) and extend `insert_machine_token` to accept and
persist `created_by`.

### Domain Examples
#### 1. Happy path — an issuer binary boots with a signer
The Acme deployment is configured to issue tokens. On boot, `AppState` is built with a
`MachineTokenSigner` alongside the existing verifier; the boot self-test still round-trips
the keypair. A mint handler can now call `MachineTokenSigner::mint`.
#### 2. Edge case — a verifier-only binary boots
A read-only replica is configured to verify but not issue. `AppState` has the verifier and
NO signer. The mint surface is not offered; nothing crashes.
#### 3. Error/boundary — created_by on a pre-feature row
A `machine_tokens` row that existed before this migration has `created_by = NULL`. The list
view (US-MT06) shows "minted by —" for it; new mints record the acting admin.

### UAT Scenarios (BDD)
#### Scenario: An issuer-configured server can mint tokens
Given the Acme server is configured to issue machine tokens
When the server finishes booting
Then a machine-token signer is available to the issuing surface
And the boot key self-test still confirms the signer and verifier are a matched keypair

#### Scenario: A verifier-only server does not offer issuing
Given the read-only replica is configured to verify but not issue tokens
When an admin opens the machine-token surface on that replica
Then issuing is reported as not enabled on this server
And the server does not error

#### Scenario: Issuance is attributable from the day the issuer ships
Given the registry can record who issued each token
When a token is minted by an admin
Then the stored token record includes the issuing admin's identity

### Acceptance Criteria
- [ ] An issuer-configured server exposes a signer to the issuing surface; a verifier-only
  server does not (and offers no mint surface).
- [ ] The boot key self-test continues to pass on an issuer-configured server.
- [ ] `machine_tokens` has a nullable `created_by` referencing `users(id)`; the migration is
  forward-only (ADR-003) and back-fills NULL for any pre-existing rows.
- [ ] `insert_machine_token` accepts and persists `created_by`.

### Outcome KPIs
- **Who**: issuer-configured Foundry deployments.
- **Does what**: expose a working mint call-site (signer present) and record the issuer on
  every new token.
- **By how much**: 100% of new tokens carry a non-NULL `created_by`; 0 verifier-only binaries
  expose a mint surface.
- **Measured by**: a row-level check on new `machine_tokens` rows + a startup capability check.
- **Baseline**: 0 (no signer in AppState today; no `created_by` column).

### Technical Notes
- Reuses `MachineTokenSigner::from_pkcs8_pem` / `MachineTokenSigner::mint` (shipped).
- **Open question Q1 (DM1)**: the signing key live in `AppState` is a security-posture
  change; DESIGN owns the at-rest mechanism, the guard, and whether issuer capability is a
  separate binary/config mode. DISCUSS captures the requirement + the risk only.
- Forward-only migration per ADR-003; `created_by` nullable.

---

## US-MT01: Mint a machine token and see its value exactly once

- **job_id**: mt-job-1 (Grant an integration programmatic access without leaking or losing the secret)

### Elevator Pitch
- **Before**: Priya needs her CI bot to call `/api/v1`, but the only way to mint a token is
  for an operator to set `MACHINE_TOKEN_SIGNING_KEY` and redeploy — she has no product path.
- **After**: she opens the workspace's machine-token surface, clicks **Issue a new token**
  (or `POST /api/v1/workspaces/{ws}/tokens`), and sees the token value displayed **exactly
  once** with a **Copy** button and a clear "this is the only time you'll see this" warning;
  the response also shows the `jti`, label, scope, and expiry.
- **Decision enabled**: Priya decides she can grant her CI bot working API access right now,
  copies the secret, and pastes it into the bot's config — no ops ticket, no redeploy.

### Problem
Priya (workspace admin) cannot issue a machine token through the product. Tokens are minted
only via env/test keys (`foundry_auth::test_keys` + the boot self-test). Granting an
integration access means operator-level access, an env change, and a redeploy — slow,
over-privileged, and error-prone. The signing primitive (`MachineTokenSigner::mint`) and the
registry (`insert_machine_token`) already exist; only the issuer surface is missing.

### Who
- **Priya Nandakumar**, workspace admin, needs to grant her CI bot API access in minutes.
- **Marco Bianchi**, integration owner, receives the token and pastes it into the bot config.
- **Context**: a workspace-scoped, admin-only machine-token surface; mint calls
  `MachineTokenSigner::mint` server-side and records metadata via `insert_machine_token`.
- **Motivation**: a working bearer credential, in hand, without touching a deploy or a key.

### Solution
On an admin mint action (web button or `POST .../tokens`), the server builds
`MachineTokenClaims { sub = principal user, scope = default-or-chosen, iat, exp = now + TTL,
jti = new uuid }` and calls `MachineTokenSigner::mint(&claims) -> SecretString`. It persists
metadata only via `insert_machine_token(..., created_by = the admin)` — the token value is
NOT stored. It returns ONE view (web screen or `201` body) showing the token value once with
a copy affordance and the warning, plus the `jti`, label, scope, and expiry. No screen or
endpoint ever re-displays the value. For Slice 1 the scope + expiry use safe DEFAULTS
(admin's *choice* arrives in US-MT04).

### Domain Examples
#### 1. Happy path — Priya mints a token for her CI bot
Priya opens Acme's machine-token surface, clicks **Issue a new token**, labels it "CI bot —
files release issues", and confirms. The next screen shows the JWT value once with a Copy
button and "This is the only time you will see this value." She copies it, sees `jti`
`0190c3af-…`, expiry `2026-07-06`, and clicks "I've copied it — done". Marco pastes the
token into the bot; `curl -H "Authorization: Bearer …" /api/v1/issues` returns 200.
#### 2. Edge case — Priya navigates away before copying
Priya mints "Slack relay" but closes the tab before copying. The value is gone — there is no
way to retrieve it. She revokes the new `jti` (US-MT03) and mints a fresh token.
#### 3. Error/boundary — issuing not enabled on this server
On a verifier-only replica, Priya opens the surface and sees "Issuing tokens is not enabled
on this server." Nothing crashes; no partial token is shown.

### UAT Scenarios (BDD)
#### Scenario: An admin issues a working token and sees its value once
Given Priya is a workspace admin of "Acme" on an issuer-configured server
When she issues a token labelled "CI bot — files release issues"
Then she sees the token value exactly once with a clear "only time you'll see this" warning and a way to copy it
And she sees the token id, label, scope, and expiry
And the integration can call the API with that token and succeed

#### Scenario: The token value is never shown again after issuance
Given Priya has just issued the "CI bot" token and left the issuance screen
When she returns to the machine-token surface
Then the token value is nowhere on the surface
And only the token's id, label, scope, expiry, and status are shown

#### Scenario: Losing the token before copying has no recovery except reissue (anxiety path)
Given Priya issued a token but closed the page before copying its value
When she looks for the value again
Then it cannot be retrieved anywhere
And the guidance tells her to revoke that token and issue a new one

#### Scenario: Issuing is refused gracefully where it is not enabled (habit/edge path)
Given Priya opens the machine-token surface on a verifier-only server
When she attempts to issue a token
Then she is told issuing is not enabled on this server
And no token value or partial value is shown
And the server does not error

### Acceptance Criteria
- [ ] An admin can issue a token; the token value is displayed exactly once with a copy
  affordance and an unmistakable "only time you'll see this" warning.
- [ ] The issuance view also shows the `jti`, label, scope, and expiry.
- [ ] A token issued this way authenticates successfully against `/api/v1`.
- [ ] After leaving the issuance view, the token value is not retrievable from any screen or
  endpoint; only `jti` + metadata are shown.
- [ ] The token value is never written to the database or to logs.
- [ ] On a verifier-only server, issuing is reported as not enabled; no value/partial value
  is shown; no error.

### Outcome KPIs
- **Who**: workspace admins on issuer-configured deployments.
- **Does what**: issue a working machine token from the product and hand the secret over,
  without an ops ticket or a redeploy.
- **By how much**: time-to-first-working-token under 2 minutes; 0 token values persisted or
  logged.
- **Measured by**: mint-to-first-successful-API-call timing in an acceptance scenario + a
  log/DB scan asserting no token value appears.
- **Baseline**: impossible today without operator env/test-key access (effectively ∞).

### Technical Notes
- Reuses `MachineTokenSigner::mint` + `insert_machine_token` (+ `created_by`, US-MT00).
- Depends on **US-MT00** (signer in AppState; `created_by`).
- Admin gating reuses `is_workspace_admin` (the surface is admin-only from day one — see
  US-MT05; you cannot expose mint without authz).
- Solution-neutral on surface (web screen vs `201` JSON body — Q6) and on the at-rest key
  mechanism (Q1).

---

# ==========================================================================
# Slice 2 — See what exists (the registry)
# ==========================================================================

## US-MT02: List the workspace's issued machine tokens

- **job_id**: mt-job-2 (Know which programmatic credentials exist and shut down the risky ones)

### Elevator Pitch
- **Before**: Dana wants to audit Acme's programmatic credentials, but the only way to see
  what tokens exist is to query the `machine_tokens` table in Postgres directly.
- **After**: she opens the workspace's machine-token surface (or `GET
  /api/v1/workspaces/{ws}/tokens`) and sees a list — label, scope, expiry, status
  (Active/Revoked) — newest first.
- **Decision enabled**: Dana decides which credentials are stale or unexpected and should be
  revoked, working entirely from the list instead of the database.

### Problem
There is no product view of issued tokens. A reviewer or admin who wants to know what
programmatic access exists must read the `machine_tokens` table directly. The data exists
(`list_machine_tokens(workspace_id)`, newest first) but there is no surface for it.

### Who
- **Dana Whitfield**, security reviewer, audits which credentials can reach the workspace.
- **Priya Nandakumar**, workspace admin, checks what she has issued before revoking.
- **Context**: workspace-scoped, admin-only list backed by `list_machine_tokens`.
- **Motivation**: see the whole picture without DB access.

### Solution
Render a workspace-scoped list (admin-only) from `list_machine_tokens(workspace_id)`, newest
first, showing label, scope (workspace vs team name), expiry, and status derived from
`revoked_at` (`NULL` = Active, else Revoked). No token value is shown — there is no column or
field that carries it. Empty workspace shows an inviting empty state. ("Minted by" and "last
used" arrive in US-MT06; revoke action arrives in US-MT03.)

### Domain Examples
#### 1. Happy path — Dana sees three tokens
Dana opens Acme's machine-token surface and sees: "CI bot — files release issues" (Backend,
expires 2026-07-06, Active), "Slack relay" (Workspace, expires 2026-06-20, Active), "Old
triage agent" (Workspace, expired/Revoked) — newest first.
#### 2. Edge case — empty workspace
Priya opens a brand-new "Sandbox" workspace's surface and sees "No tokens issued yet — issue
one to grant an integration access," not a blank table.
#### 3. Error/boundary — a revoked token in the list
The "Old triage agent" row shows status Revoked (its `revoked_at` is set); the list still
shows it (the row survives revocation) so Dana can see it was shut down.

### UAT Scenarios (BDD)
#### Scenario: A reviewer sees the workspace's issued tokens
Given Acme has three issued machine tokens, one of them revoked
When Dana opens the machine-token surface for Acme
Then she sees all three tokens, newest first
And each shows its label, scope, expiry, and status
And no token value appears anywhere in the list

#### Scenario: A reviewer of one workspace cannot see another workspace's tokens
Given Acme and Globex each have issued tokens
When Dana opens the machine-token surface for Acme
Then she sees only Acme's tokens
And none of Globex's tokens appear

#### Scenario: An empty workspace shows guidance, not a blank table
Given the "Sandbox" workspace has issued no tokens
When Priya opens its machine-token surface
Then she sees a clear empty state inviting her to issue the first token

### Acceptance Criteria
- [ ] The surface lists the workspace's tokens, newest first, with label, scope, expiry, and
  status (Active/Revoked).
- [ ] No token value is shown in the list (no column, no field).
- [ ] The list is scoped to the current workspace; other workspaces' tokens never appear.
- [ ] An empty workspace shows an inviting empty state.

### Outcome KPIs
- **Who**: workspace admins and security reviewers.
- **Does what**: enumerate the workspace's programmatic credentials without DB access.
- **By how much**: 100% of issued (non-GC'd) tokens for the workspace are visible; DB access
  needed for the audit drops to 0.
- **Measured by**: an acceptance scenario asserting the listed set equals the issued set;
  qualitative confirmation that reviewers stop querying Postgres.
- **Baseline**: 0 (no list surface today).

### Technical Notes
- Reuses `list_machine_tokens(workspace_id)` (shipped, newest first).
- Admin gating reuses `is_workspace_admin` (US-MT05).
- Solution-neutral on surface (web table vs `GET` JSON array — Q6).

---

# ==========================================================================
# Slice 3 — Take it back (revocation)
# ==========================================================================

## US-MT03: Revoke a machine token so it is refused on its next use

- **job_id**: mt-job-2 (Know which programmatic credentials exist and shut down the risky ones)

### Elevator Pitch
- **Before**: Dana spots a stale "Slack relay" token but has no product way to disable it —
  she would have to `UPDATE machine_tokens SET revoked_at = now()` in Postgres.
- **After**: she clicks **Revoke** on the token's row (or `DELETE
  /api/v1/workspaces/{ws}/tokens/{jti}`), confirms, and the row flips to **Revoked**; the
  integration using it is refused on its very next API call.
- **Decision enabled**: Dana decides to shut down a risky credential and immediately trusts
  that it can no longer call the API.

### Problem
There is no product way to revoke a token. The mechanism exists
(`revoke_machine_token(jti)` sets `revoked_at`, and the per-request denylist already refuses
revoked `jti`s) but there is no control surface — revoking means editing the database.

### Who
- **Dana Whitfield**, security reviewer, needs an immediate kill switch for a credential.
- **Priya Nandakumar**, workspace admin, revokes a lost or rotated token.
- **Context**: a **Revoke** action on each list row (US-MT02), backed by
  `revoke_machine_token(jti)`; refusal is enforced per-request by the SHIPPED denylist.
- **Motivation**: stop a credential immediately and verifiably.

### Solution
Add an admin-only **Revoke** action per list row (with a confirmation that warns it is
immediate and irreversible). On confirm, call `revoke_machine_token(jti)` (`SET revoked_at =
now()`); the row survives and flips to Revoked. The SHIPPED per-request denylist
(`find_machine_token_by_jti(jti).revoked_at IS NULL`) refuses the credential on its next
`/api/v1` call. Revoke is idempotent; revoking an already-revoked token is a no-op success.

### Domain Examples
#### 1. Happy path — revoke the Slack relay, next call refused
Dana clicks Revoke on "Slack relay", confirms. The row shows Revoked. The Slack integration's
next `GET /api/v1/issues` with that token returns 401/403.
#### 2. Edge case — revoke an already-revoked token
Priya clicks Revoke on "Old triage agent" (already revoked). The action succeeds silently
(idempotent); the row stays Revoked.
#### 3. Error/boundary — revoke a token from another workspace
A crafted request tries to revoke a `jti` that belongs to Globex while acting on Acme. The
request is refused non-enumerably (not found); Globex's token is untouched.

### UAT Scenarios (BDD)
#### Scenario: A revoked token is refused on its very next API call
Given the "Slack relay" token is Active and an integration is calling the API with it
When Dana revokes the "Slack relay" token
Then the row shows as Revoked
And the integration's next API call with that token is refused

#### Scenario: Revoking is immediate and final, with a clear warning before it happens
Given Priya is about to revoke a token
When she triggers the revoke action
Then she is warned that the revoke is immediate and cannot be undone before she confirms
And on confirming, the token is shown as Revoked

#### Scenario: Revoking an already-revoked token is harmless (idempotent edge path)
Given the "Old triage agent" token is already revoked
When Priya revokes it again
Then the action succeeds without error
And the token remains Revoked

#### Scenario: An admin cannot revoke a token outside their workspace (evil-user path)
Given a token belongs to the "Globex" workspace
When a crafted request tries to revoke it while acting on "Acme"
Then the request is refused without revealing whether that token exists
And the Globex token remains Active

### Acceptance Criteria
- [ ] An admin can revoke any token in their workspace from its list row.
- [ ] Revoking shows a clear, immediate-and-irreversible warning before it takes effect.
- [ ] A revoked token's next `/api/v1` call is refused (the per-request denylist honors it).
- [ ] The revoked row survives and shows status Revoked.
- [ ] Revoke is idempotent (already-revoked → no-op success).
- [ ] Revoking a token outside the acting workspace is refused non-enumerably; that token is
  untouched.

### Outcome KPIs
- **Who**: workspace admins and security reviewers.
- **Does what**: shut down a programmatic credential and have it refused on its next use,
  without DB access.
- **By how much**: revoke-to-refusal effective within one API request (next call); DB access
  needed to revoke drops to 0.
- **Measured by**: an acceptance scenario that revokes then asserts the next call is refused.
- **Baseline**: 0 (no revoke surface today).

### Technical Notes
- Reuses `revoke_machine_token(jti)` + the SHIPPED per-request denylist (no new refusal code).
- Depends on **US-MT02** (revoke from the list row).
- Admin gating reuses `is_workspace_admin` (US-MT05).
- Solution-neutral on surface (web button vs `DELETE`/`POST .../revoke` — Q6).

---

# ==========================================================================
# Slice 4 — Make issuance trustworthy: least-privilege, bounded, attributable, admin-only
# ==========================================================================

## US-MT04: Choose scope and expiry within server-enforced bounds when issuing

- **job_id**: mt-job-3 (Grant least-privilege, time-bounded access instead of an all-powerful forever key)

### Elevator Pitch
- **Before**: Priya can issue a token (US-MT01) but only with the default scope and expiry —
  she cannot narrow a token to one team or shorten its life.
- **After**: the issue form (or `POST .../tokens` body) lets her pick a scope (whole
  workspace or a specific team) and an expiry within the allowed range; the issued token and
  the list both show the scope and expiry she chose.
- **Decision enabled**: Priya decides to grant her CI bot access to just the Backend team for
  30 days, confident she did not over-grant or issue a forever-key.

### Problem
Issuing with only defaults forces over-granting: every token is workspace-wide and uses the
default lifetime. The claim set already supports `scope: Option<Uuid>` and `exp`, and
`insert_machine_token` takes `scope_team_id` + `expires_at` — the substrate supports
least-privilege, but the admin cannot express the choice.

### Who
- **Priya Nandakumar**, workspace admin, wants to grant only what an integration needs.
- **Context**: the issue surface from US-MT01, extended with scope + expiry inputs; server
  enforces a max TTL cap and a default (Q3).
- **Motivation**: least-privilege, time-bounded grants.

### Solution
Extend the issue surface with a scope choice (whole workspace, or one team → `scope_team_id`)
and an expiry choice within a server-enforced range (default + max cap; numbers are DESIGN,
Q3). The server validates the team belongs to the workspace and the expiry is within bounds,
then mints with the chosen `scope` + `exp`. The issued view and the list (US-MT02) show the
chosen scope + expiry.

### Domain Examples
#### 1. Happy path — Backend-only, 30-day token
Priya issues "CI bot" scoped to the Backend team, expiry 30 days. The token's claims carry
`scope = Backend team id` and `exp = now + 30d`; the list shows "Backend / 2026-07-06".
#### 2. Edge case — expiry at the cap
Priya picks the maximum allowed expiry (e.g. 90 days). It is accepted exactly at the cap.
#### 3. Error/boundary — expiry over the cap, or a team not in the workspace
Priya tries a 1-year expiry → rejected with "Maximum expiry is {cap} days." A crafted request
picks a team from another workspace → rejected.

### UAT Scenarios (BDD)
#### Scenario: An admin issues a team-scoped, time-bounded token
Given Priya is issuing a token for "Acme"
When she scopes it to the "Backend" team and sets a 30-day expiry
Then the issued token is limited to that team and that lifetime
And the list shows that scope and expiry

#### Scenario: Expiry beyond the server cap is refused
Given the server caps token lifetime at its maximum
When Priya tries to issue a token that outlives the cap
Then issuance is refused with the maximum stated
And no token is issued

#### Scenario: A scope that is not part of the workspace is refused (evil-user path)
Given Priya is issuing a token for "Acme"
When a crafted request scopes the token to a team in another workspace
Then issuance is refused
And no token is issued

### Acceptance Criteria
- [ ] The issue surface lets the admin choose a scope (whole workspace or one team) and an
  expiry within the server-enforced range.
- [ ] The issued token's claims reflect the chosen scope and expiry; the list shows them.
- [ ] An expiry beyond the cap is refused with the maximum stated; no token is issued.
- [ ] A scope referencing a team outside the workspace is refused; no token is issued.

### Outcome KPIs
- **Who**: workspace admins issuing tokens.
- **Does what**: grant scoped, time-bounded credentials instead of workspace-wide forever-keys.
- **By how much**: share of tokens issued with a non-default (narrower) scope or shorter
  expiry rises above 0; 0 tokens exceed the cap.
- **Measured by**: distribution of `scope_team_id` non-NULL and `expires_at` across issued
  tokens; an acceptance scenario asserting the cap is enforced.
- **Baseline**: 0 (US-MT01 issues defaults only).

### Technical Notes
- Reuses `MachineTokenClaims.scope`/`exp` + `insert_machine_token(scope_team_id, expires_at)`.
- Depends on **US-MT01**.
- **Open question Q3**: scope vocabulary (workspace/team; read/write?) and the TTL
  default/cap numbers are DESIGN.

---

## US-MT05: Restrict minting, listing, and revoking to workspace admins

- **job_id**: mt-job-1 (Grant an integration programmatic access without leaking or losing the secret)

### Elevator Pitch
- **Before**: the machine-token surface exists, but without an enforced boundary a
  non-admin member could reach it and issue or revoke credentials.
- **After**: only a workspace admin can open the machine-token surface or call the
  mint/list/revoke endpoints; a non-admin gets a non-enumerable refusal that does not even
  reveal the surface exists.
- **Decision enabled**: Priya (and her security reviewer) trust that only admins can grant or
  revoke programmatic access — issuance authority is bounded.

### Problem
Minting and revoking programmatic credentials is a privileged action. Without an enforced
admin boundary, a non-admin member (or a crafted request) could issue a token or revoke
someone else's, undermining the whole feature. The check exists
(`is_workspace_admin(workspace_id, user_id)`) but must gate every entry point.

### Who
- **Carlos Mendez**, a non-admin workspace member ("Careless Cathy"/"Malicious Mike"
  stand-in), must be refused.
- **Priya Nandakumar**, admin, must be allowed.
- **Context**: every machine-token entry point (mint, list, revoke; web + API) wraps
  `is_workspace_admin`.
- **Motivation**: bounded issuance authority; no leak of the surface to non-admins.

### Solution
Gate every machine-token entry point with `is_workspace_admin(workspace_id, acting_user)`.
Admins proceed; non-admins (and members of other workspaces) get a non-enumerable refusal
(404/403 per DESIGN) that does not confirm the surface or any token exists. This check is
REUSED by US-MT01/02/03 from day one; this story makes the boundary explicit and adds the
adversarial tests.

### Domain Examples
#### 1. Happy path — admin allowed
Priya (admin of Acme) opens the surface and issues/lists/revokes normally.
#### 2. Edge case — non-admin member refused
Carlos (a non-admin member of Acme) navigates to the machine-token URL and gets a
non-enumerable refusal; he cannot tell whether the surface or any token exists.
#### 3. Error/boundary — cross-workspace request
A crafted request from an Acme admin targets Globex's mint endpoint; it is refused because he
is not a Globex admin.

### UAT Scenarios (BDD)
#### Scenario: A workspace admin can use the machine-token surface
Given Priya is an admin of "Acme"
When she opens the machine-token surface and issues, lists, and revokes tokens
Then every action is permitted

#### Scenario: A non-admin member is refused without learning the surface exists (evil-user path)
Given Carlos is a non-admin member of "Acme"
When he tries to open the surface or issue, list, or revoke a token
Then he is refused in a way that does not reveal whether the surface or any token exists

#### Scenario: An admin of one workspace cannot manage another workspace's tokens (evil-user path)
Given Priya is an admin of "Acme" but not of "Globex"
When she tries to issue, list, or revoke tokens for "Globex"
Then she is refused

### Acceptance Criteria
- [ ] Every machine-token entry point (mint, list, revoke; all surfaces) requires
  `is_workspace_admin` for the target workspace.
- [ ] Non-admins receive a non-enumerable refusal that does not reveal the surface or token
  existence.
- [ ] An admin of one workspace cannot mint/list/revoke for a workspace they do not admin.

### Outcome KPIs
- **Who**: all workspace members who reach a machine-token entry point.
- **Does what**: only admins proceed; everyone else is refused non-enumerably.
- **By how much**: 100% of non-admin attempts refused; 0 surface/existence leaks.
- **Measured by**: adversarial acceptance scenarios for non-admin and cross-workspace
  attempts.
- **Baseline**: enforced informally by US-MT01/02/03's reuse of the check; this story
  guarantees + tests it across all entry points.

### Technical Notes
- Reuses `is_workspace_admin(workspace_id, user_id)` (shipped).
- The check is already reused by US-MT01/02/03; this story is the explicit boundary +
  adversarial coverage (see `nfrs.md` NFR-MT-AUTHZ-*).
- Solution-neutral on the refusal shape (404 vs 403 — DESIGN).

---

## US-MT06: Show who minted each token and when it was last used

- **job_id**: mt-job-4 (Account for who granted each piece of programmatic access)

### Elevator Pitch
- **Before**: the token list (US-MT02) shows label/scope/expiry/status, but not who issued a
  token or whether it is still being used.
- **After**: each list row also shows **"minted by {admin}"** (from `created_by`) and **last
  used** (from `last_used_at`), so Dana can attribute issuance and spot stale credentials.
- **Decision enabled**: Dana decides which credentials to revoke based on who issued them and
  whether they are still in use (e.g. a token from a departed admin, unused for months).

### Problem
The list shows what exists but not who created it or whether it is live. Issuance is
anonymous as to its issuer (until `created_by` exists, US-MT00) and the existing
`last_used_at` is unsurfaced. A reviewer cannot attribute a credential or judge staleness.

### Who
- **Dana Whitfield**, security reviewer, attributes and triages credentials for revocation.
- **Context**: the list from US-MT02, enriched with `created_by` (US-MT00) and the existing
  `last_used_at` (touched on use by the shipped path).
- **Motivation**: accountability + staleness signal.

### Solution
Add two columns to the list (US-MT02): "minted by {admin}" resolved from `created_by` → the
issuing user, and "last used" from `last_used_at` (or "never" if NULL). A row with a NULL
`created_by` (a pre-feature token, if any) shows "minted by —".

### Domain Examples
#### 1. Happy path — attribute and triage
Dana sees "CI bot" was minted by Priya N. and last used 2 minutes ago (live), while "Old
triage agent" was minted by Dana W. and never used since revocation.
#### 2. Edge case — a never-used token
A freshly minted token shows "last used: never" until its first API call touches
`last_used_at`.
#### 3. Error/boundary — a pre-feature token with NULL created_by
A token issued before this feature (if any) shows "minted by —"; new tokens always show the
issuing admin.

### UAT Scenarios (BDD)
#### Scenario: The list attributes each token to who issued it
Given Priya issued the "CI bot" token and Dana issued the "Old triage agent" token
When Dana opens the machine-token list
Then "CI bot" shows it was minted by Priya
And "Old triage agent" shows it was minted by Dana

#### Scenario: The list shows whether a token is still being used
Given the "CI bot" token was used 2 minutes ago and a freshly issued token has never been used
When Dana opens the list
Then "CI bot" shows a recent last-used time
And the fresh token shows it has never been used

#### Scenario: A token issued before issuer attribution shows an unknown issuer (edge path)
Given a token exists with no recorded issuer
When Dana opens the list
Then that token shows its issuer as unknown
And newly issued tokens always show the issuing admin

### Acceptance Criteria
- [ ] The list shows "minted by {admin}" for each token, resolved from `created_by`.
- [ ] The list shows each token's last-used time, or "never" if it has not been used.
- [ ] A token with no recorded issuer shows an unknown/— issuer; new tokens always show the
  issuing admin.

### Outcome KPIs
- **Who**: security reviewers and admins auditing credentials.
- **Does what**: attribute every credential to its issuer and judge staleness from last-used.
- **By how much**: 100% of tokens issued after this feature show a named issuer; reviewers
  can identify stale (unused-N-days) tokens without DB access.
- **Measured by**: an acceptance scenario asserting issuer + last-used are shown; row-level
  check that new tokens carry `created_by`.
- **Baseline**: 0 (no issuer column today; `last_used_at` unsurfaced).

### Technical Notes
- Depends on **US-MT00** (`created_by`) and **US-MT02** (the list).
- Reuses the existing `last_used_at` (touched on use by the shipped verify path).
- Solution-neutral on surface (web columns vs `GET` JSON fields — Q6).
