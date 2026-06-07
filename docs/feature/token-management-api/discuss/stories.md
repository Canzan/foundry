<!-- markdownlint-disable MD024 -->
# Token-Management API — User Stories

> This feature adds the **machine-facing JSON counterpart** to the shipped `machine-token-admin-ux`
> web UI: programmatic LIST + REVOKE (and, deferred, MINT) of machine tokens under `/api/v1`,
> bearer-authenticated, for integrators / CI / automation / agents. The mint/list/revoke USE-CASES
> already SHIPPED + are mutation-hardened (100%) in `foundry_services::tokens`; the `/api/v1` bearer
> extractor (`MachinePrincipal`/`token_auth`) and JSON error envelope (`status_for`) SHIPPED in
> `foundry-api`. This feature adds the JSON token ROUTES + the authz/escalation gate on top.
>
> Stories reflect the RECOMMENDED authz model (wave-decisions.md Q-AUTHZ → option c): v1 exposes
> LIST + REVOKE to bearer tokens; programmatic MINT is DEFERRED (the escalation-sensitive op).
> Personas + jobs: `jobs.yaml`. Open questions (esp. the Q-AUTHZ crux): `wave-decisions.md`.
> Measurable NFR forms: `nfrs.md`.

## System Constraints (cross-cutting)

Apply to every story; measurable forms live in `nfrs.md`.

- **Reuse, don't rebuild, the use-cases.** LIST calls SHIPPED `list_tokens(store, principal)`;
  REVOKE calls SHIPPED `revoke_token(store, principal, jti)`. Both already enforce
  `is_workspace_admin` authz, workspace isolation, and (revoke) non-enumerable `NotFound`. The
  bearer extractor `MachinePrincipal`/`token_auth::authenticate` and the JSON envelope `status_for`
  are reused unchanged.
- **No token value on any read path.** The LIST exposes only `jti` + metadata (mirrors `TokenView`,
  which has no value field). No value/token/secret/hash field, ever (NFR-TMA-SEC-02).
- **Revocation is a flag** (`revoked_at`), effective on the credential's NEXT `/api/v1` request via
  the SHIPPED per-request denylist; idempotent.
- **Refusals are non-enumerable.** Non-management caller → 403; cross-workspace/unknown jti →
  identical 404; any bearer auth failure → identical 401. No existence oracle (NFR-TMA-SEC-03).
- **The authz/escalation model is the CRUX (Q-AUTHZ), user-ratified before DESIGN.** v1: bearer may
  LIST + REVOKE (incl. self), gated by `is_workspace_admin` on the bound user; bearer may NOT MINT.
- **Bearer-only, CSRF-exempt by construction.** Token routes mount on `/api/v1` OUTSIDE
  session+CSRF (a machine request carries a JWT, no cookie). The browser path is unchanged.
- **One binary, no new runtime services.** foundry-api reaches the use-cases via the `Services`
  handle and never names `foundry_store::Store` (boundary guard).
- **Solution-neutral.** The revoke verb shape (`DELETE` vs `POST .../revoke`), JSON field names, the
  signer wiring (only relevant to a future mint), and any `tokens:manage` capability representation
  are DESIGN.

## Glossary (additions for this feature)

- **Machine token / bearer**: an Ed25519-signed JWT presented as `Authorization: Bearer <jwt>` to
  `/api/v1`. Verified offline by `MachineTokenVerifier`.
- **Management-capable caller**: a bearer whose bound `user_id` `is_workspace_admin` (v1 model). It
  may LIST + REVOKE token-management routes. (If option c+b is ratified, "management-capable" also
  requires an explicit `tokens:manage` capability.)
- **Revoke-self**: a bearer revoking its own `jti` (a subset of `revoke_token` — the caller's own
  token is in its own workspace). The core of hands-free rotation.
- **Non-enumerable**: a refusal that does not reveal whether the target exists (cross-workspace and
  unknown jti both return the identical 404).
- **`jti`**: the token id (row PK); safe to display; the revoke key.

---

# ==========================================================================
# Slice 1 — Walking Skeleton: prove the authz model on the SAFEST op (GET list)
# US-TMA00 (route-group + authz-gate seam, @infrastructure) + US-TMA01 (GET list)
# ==========================================================================

## US-TMA00: Stand up the /api/v1 token route group and the authz-gate seam

- **job_id**: infrastructure-only
- **infrastructure_rationale**: This story enables no user decision on its own — it is the seam two
  user-visible jobs stand on. The foundry-api adapter today has issue/comment routes but NO token
  routes; standing up the `/api/v1/.../tokens` route group + the single place where the
  ratified authz decision (Q-AUTHZ) is applied (the "management-capable?" gate) is pure scaffold. It
  is observable to no caller by itself and is folded into Slice 1, never shipped standalone. The
  user-visible outcome it enables is US-TMA01 (LIST).

### Problem
`crates/foundry-api/src/lib.rs` serves JSON issue/comment routes under `/api/v1` but has no
token-management routes. The use-cases (`list_tokens`, `revoke_token`) exist and are
mutation-hardened, but there is no JSON adapter call-site for them and no place where the ratified
authz/escalation decision is applied. Until the route group + the authz-gate seam exist, no
programmatic caller can manage tokens.

### Who
- **The platform** (no direct user). Enables Sven/Dana's automation (US-TMA01, US-TMA02) and the
  security-automation pipeline.
- **Context**: `crates/foundry-api/src/lib.rs` `routes<S>()` (issue/comment routes today, requires
  `Services: FromRef<S>` + `Arc<MachineTokenVerifier>: FromRef<S>`); the `MachinePrincipal`
  extractor; `status_for`.
- **Motivation**: a single, tested place to apply the ratified authz model and dispatch to the
  SHIPPED use-cases.

### Solution
Add a `/api/v1/teams/{team}/projects/{project}/tokens` route group to `routes()`, authenticated by
the existing `MachinePrincipal` extractor, dispatching to `list_tokens` / `revoke_token` via the
`Services` handle. Introduce ONE authz-gate seam where the ratified Q-AUTHZ decision is enforced
(v1: the use-cases' own `is_workspace_admin` gate is sufficient for LIST+REVOKE; the seam is the
documented place that decision lives so a future capability check has a home). Refusals/validation
use `status_for`. Exact path/verb shapes are DESIGN.

### Domain Examples
#### 1. Happy path — the route group is reachable and authenticated
A `GET /api/v1/teams/platform/projects/infra/tokens` with a valid bearer reaches the handler; the
`MachinePrincipal` extractor has already produced `Principal::Machine`. The handler can call
`list_tokens`.
#### 2. Edge case — no bearer
The same path with no `Authorization` header is refused by the extractor with the identical 401
BEFORE the handler runs (reused fail-closed behaviour).
#### 3. Error/boundary — the authz decision has exactly one home
A non-management caller is refused by the single authz gate (403), not by ad-hoc checks scattered
per route — so the ratified Q-AUTHZ model is enforced uniformly.

### UAT Scenarios (BDD)
#### Scenario: The token route group is reachable and bearer-authenticated
Given the Acme server exposes the token-management API
When an integrator calls the token list endpoint with a valid bearer credential
Then the request reaches the token-management surface as an authenticated machine caller
And no browser session or CSRF token is required

#### Scenario: A request with no credential is refused before any token logic runs
Given the token-management API is exposed
When a caller requests the token list with no bearer credential
Then the request is refused as unauthorized
And no token data is returned

#### Scenario: The authorization decision is applied in one place
Given the ratified model allows only management-capable callers to manage tokens
When any token-management route is called
Then the management-capability decision is applied uniformly before the use-case runs

### Acceptance Criteria
- [ ] A `/api/v1/.../tokens` route group exists and is authenticated by the SHIPPED bearer extractor.
- [ ] A credential-less request is refused (identical 401) before any token logic.
- [ ] The management-capability authz decision has exactly one enforcement point (the ratified
      Q-AUTHZ model), applied to every token route.
- [ ] foundry-api still does not name `foundry_store::Store` (boundary guard holds).

### Technical Notes
- Extend `routes<S>()` in `crates/foundry-api/src/lib.rs`; reuse `MachinePrincipal`, `status_for`.
- Dispatch via the `Services` handle (the issue/comment routes' pattern).
- The authz-gate seam is where a future `tokens:manage` capability check (option c+b) would live.

---

## US-TMA01: List a workspace's machine tokens as JSON, with the authz model proven

- **job_id**: mt-api-job-1

### Elevator Pitch
- **Before**: a security-automation pipeline can only see machine tokens by scraping the human
  `/admin/tokens` HTML page or by querying Postgres directly — neither is automatable or
  appropriately-privileged.
- **After**: a `GET /api/v1/teams/platform/projects/infra/tokens` with a management bearer returns a
  JSON array like `[{"jti":"…","label":"ci-issue-filer","scope_team_id":null,"expires_at":"2026-09-05T00:00:00Z","revoked":false,"last_used_at":"2026-06-07T03:11:00Z","minted_by":"priya@acme.dev"}]`
  (never a token value); a non-management bearer gets `403 {"error":{"code":"forbidden","message":"forbidden"}}` with no registry leaked.
- **Decision enabled**: the pipeline decides which credentials are stale or over-scoped and flags
  them for rotation/revocation — from machine-readable JSON, no human, no DB.

### Problem
Dana's audit pipeline (security automation) needs a current inventory of which machine tokens can
reach the workspace's API. Today the only sources are the human HTML admin page (brittle to scrape)
or direct DB access (over-privileged). `list_tokens` is SHIPPED and returns a value-free,
workspace-scoped `TokenView`, but there is no JSON surface for it — so the pipeline is blind without
a human or a DB login.

### Who
- **Security automation** (Dana Whitfield's audit pipeline) | non-human, scheduled | wants a stable
  JSON inventory over the same bearer-authenticated `/api/v1` the integrations use.
- Also: an integrator's pipeline listing the credentials it owns.

### Solution
`GET /api/v1/.../tokens` with a management bearer returns the workspace's tokens as a JSON array
mirroring `TokenView` (jti, label, scope_team_id + resolved name, expires_at, revoked, last_used_at,
minted_by) — **never a value**. A non-management bearer is refused with a non-enumerable 403. This
slice PROVES the ratified authz model on the safest (read-only) op. JSON field names = DESIGN.

### Domain Examples
#### 1. Happy path — management bearer lists two tokens
Dana's pipeline (bearer bound to an admin user) calls the list endpoint. It receives a JSON array of
two tokens: `ci-issue-filer` (workspace-wide, used 4 minutes ago, minted by priya@acme.dev) and
`slack-relay` (team-scoped to "platform", never used, minted by priya@acme.dev). Neither carries a
value.
#### 2. Edge case — empty registry
A fresh workspace with no tokens returns `[]` with 200 (not 404) — the pipeline records "zero
credentials" cleanly.
#### 3. Error/boundary — non-management bearer
Sven's plain CI token (bound to a non-admin user) calls the list endpoint and gets 403 forbidden
with no token data — the registry is not leaked to a non-management caller.

### UAT Scenarios (BDD)
#### Scenario: An audit pipeline lists the workspace's tokens as JSON
Given Dana's audit pipeline holds a management-capable bearer credential
And the workspace has a token "ci-issue-filer" used 4 minutes ago and a token "slack-relay" never used
When the pipeline requests the token list
Then it receives a JSON array of both tokens with their label, scope, expiry, status, last-used, and who minted each
And no token value appears anywhere in the response

#### Scenario: An empty registry returns an empty list, not an error
Given a fresh workspace has issued no machine tokens
And Dana's pipeline holds a management-capable bearer
When the pipeline requests the token list
Then it receives an empty JSON array with a success status

#### Scenario: A non-management caller is refused without leaking the registry
Given Sven's CI credential is bound to a non-admin user
When Sven's pipeline requests the token list
Then the request is refused as forbidden
And no token data is returned in the response

#### Scenario: The list never exposes a token value
Given the workspace has issued machine tokens
And a management-capable bearer requests the list
When the response is inspected
Then no field carries a token, secret, or hash value

### Acceptance Criteria
- [ ] A management bearer receives a JSON array of the workspace's tokens with label/scope/expiry/
      status/last-used/minted-by; no value field.
- [ ] An empty registry returns `[]` with a 200, not a 404.
- [ ] A non-management bearer is refused (403, non-enumerable — no registry data).
- [ ] No response on any read path contains a token/secret/hash value (NFR-TMA-SEC-02).
- [ ] Cross-workspace tokens never appear (workspace-scoped read, reused from `list_tokens`).

### Outcome KPIs
- **Who**: security-automation / integrator pipelines
- **Does what**: pull the token registry as JSON instead of scraping HTML or querying the DB
- **By how much**: 100% of inventory pulls succeed via `/api/v1` with zero browser/DB access
- **Measured by**: token-list API call success rate + a count of audit pulls via the API
- **Baseline**: 0 (no JSON surface exists today)

### Technical Notes
- Calls SHIPPED `foundry_services::tokens::list_tokens` via the `Services` handle.
- Response mirrors `TokenView` (no value field by construction).
- The authz refusal reuses the use-case's `is_workspace_admin` gate → `Forbidden` → `status_for` 403.

---

# ==========================================================================
# Slice 2 — Revoke + Rotate: hands-free credential lifecycle
# US-TMA02 (revoke) + US-TMA03 (revoke-self / rotation)
# ==========================================================================

## US-TMA02: Revoke a machine token via the API; dead on its next call

- **job_id**: mt-api-job-2

### Elevator Pitch
- **Before**: revoking a credential requires a human to open `/admin/tokens` and click — not
  automatable, slow during an incident, a bottleneck for scheduled rotation.
- **After**: a `DELETE /api/v1/teams/platform/projects/infra/tokens/{jti}` with a management bearer
  returns `204 No Content`; the targeted credential's very next `/api/v1` call returns
  `401 {"error":{"code":"unauthorized","message":"unauthorized"}}`. A cross-workspace or unknown jti
  returns the identical `404 {"error":{"code":"not_found","message":"not found"}}`.
- **Decision enabled**: an incident-response runbook or a rotation job decides to kill a compromised
  or stale credential and confirms — in its own log — that it is dead, without waiting for a human.

### Problem
The automation agent (a rotation job; an incident runbook) needs to kill a credential
programmatically and trust it is dead immediately. `revoke_token` is SHIPPED (authz →
non-enumerable cross-workspace `NotFound` → idempotent `revoked_at` flip; the per-request denylist
refuses the jti next call), but there is no JSON route — so revoke needs a human clicking a UI.

### Who
- **Automation agent** (rotation job / incident runbook) | non-human | wants a single idempotent API
  call that kills a credential and is provably effective on its next use.

### Solution
A revoke route (`DELETE /api/v1/.../tokens/{jti}`, verb = Q-REVOKE-VERB) with a management bearer
calls `revoke_token`. Success returns 204 (idempotent — re-revoke also succeeds). The SHIPPED
denylist refuses the targeted jti on its next call (401). A cross-workspace or unknown jti returns
the identical non-enumerable 404. A non-management bearer → 403.

### Domain Examples
#### 1. Happy path — kill a leaked CI token
The Acme runbook spots `ci-issue-filer` in a leaked build log. It calls
`DELETE .../tokens/{ci-issue-filer-jti}` with the management bearer → 204. The leaked token's next
API call → 401. The runbook records "credential dead, confirmed."
#### 2. Edge case — idempotent re-revoke
A retry storm sends the same DELETE twice. The second returns success too (idempotent re-stamp); no
error, no double-effect.
#### 3. Error/boundary — cross-workspace probe
An attacker's management bearer (workspace A) tries to revoke a jti it guessed from workspace B →
404 not_found, identical to a jti that does not exist. The attacker learns nothing.

### UAT Scenarios (BDD)
#### Scenario: A rotation job revokes a credential and it is dead on the next call
Given the runbook holds a management-capable bearer for the workspace
And a credential "ci-issue-filer" is active
When the runbook revokes "ci-issue-filer" via the API
Then the revoke succeeds
And the very next API call made with "ci-issue-filer" is refused as unauthorized

#### Scenario: Revoking an already-revoked credential is a harmless success
Given "ci-issue-filer" has already been revoked
When the runbook revokes it again via the API
Then the request succeeds without error
And the credential remains dead

#### Scenario: Revoking a credential from another workspace reveals nothing
Given an attacker holds a management bearer for workspace A
And a credential exists in workspace B
When the attacker attempts to revoke that credential's id
Then the request is refused as not found
And the refusal is identical to revoking an id that does not exist anywhere

#### Scenario: A non-management caller cannot revoke
Given Sven's CI credential is bound to a non-admin user
When Sven's pipeline attempts to revoke any token
Then the request is refused as forbidden
And no token is revoked

### Acceptance Criteria
- [ ] A management bearer revokes a workspace token; the targeted credential's next `/api/v1` call is
      refused (401) — one-request latency (NFR-TMA-SEC-04).
- [ ] Re-revoking an already-revoked token succeeds (idempotent; NFR-TMA-REL-01).
- [ ] A cross-workspace or unknown jti returns the identical non-enumerable 404 (NFR-TMA-SEC-03).
- [ ] A non-management bearer is refused (403), no token revoked.

### Outcome KPIs
- **Who**: rotation jobs / incident runbooks
- **Does what**: revoke a credential programmatically and confirm it is dead, instead of a human
  clicking the UI
- **By how much**: revoke-to-refusal latency = one request (next call); 100% of revokes verifiable in
  the automation log
- **Measured by**: revoke API success rate + a "next call refused" assertion in acceptance + the
  revoke→refusal latency guardrail
- **Baseline**: 0 (revoke is human-UI-only today)

### Technical Notes
- Calls SHIPPED `foundry_services::tokens::revoke_token`; effectiveness is the SHIPPED per-request
  denylist in `token_auth::authenticate` (no new mechanism).
- Verb shape = Q-REVOKE-VERB (DESIGN).

---

## US-TMA03: Revoke-self — a token retires its own future use (rotation)

- **job_id**: mt-api-job-2

### Elevator Pitch
- **Before**: a rotation script that mints a new credential (via a human/UI step in v1) has no way to
  programmatically retire the OLD one it is still authenticating with — it waits for a human.
- **After**: the script, authenticated with `$OLD_TOKEN`, calls
  `DELETE /api/v1/teams/platform/projects/infra/tokens/{old_jti}` → `204 No Content`; its next call
  with `$OLD_TOKEN` returns `401 unauthorized`. The script has retired itself.
- **Decision enabled**: a rotation job decides "the new credential is live and verified — retire the
  old one now," completing a hands-free rotation with no human and no leftover live credential.

### Problem
Hands-free rotation needs the final step — retiring the old credential — to be programmatic. The old
credential is the one the rotation script is currently authenticating with; it must be able to revoke
ITSELF. `revoke_token` accepts any jti in the caller's workspace, and the caller's own jti is in its
own workspace, so revoke-self is a subset — but it has never been exercised as a deliberate flow.

### Who
- **Automation agent** (scheduled rotation job) | non-human | wants to promote a new credential, then
  retire the old one it still holds, with no human in the loop.

### Solution
The same revoke route, called with the caller's OWN jti. `revoke_token` flips `revoked_at`; the
SHIPPED denylist then refuses that credential on its NEXT call (the current call already passed
authentication, so it succeeds; the next one fails). The runbook MUST switch to the new credential
before revoking-self (operational ordering — documented).

### Domain Examples
#### 1. Happy path — clean rotation
The Acme nightly job receives a new token (minted by a human in the UI), switches its config to it,
verifies a test call succeeds, then revokes its OLD token via the API with the old token → 204. The
old token's next use → 401. Rotation complete, no live leftover.
#### 2. Edge case — revoke-self then immediately reuse old token
The job revokes-self, then (by mistake) makes another call with the OLD token → 401. The job's
health check catches it; the fix is to switch credentials first (habit-path scenario).
#### 3. Error/boundary — revoke-self on an already-rotated token
The job re-runs and tries to revoke an old jti that was already revoked last cycle → idempotent
success; no error.

### UAT Scenarios (BDD)
#### Scenario: A rotation job retires its own credential after promoting a new one
Given a rotation job holds an old credential and has switched to a verified new credential
When the job revokes its old credential via the API using the old credential
Then the revoke succeeds on the current call
And the next API call made with the old credential is refused as unauthorized

#### Scenario: Revoking self does not break the in-flight request
Given a rotation job is authenticated with its current credential
When the job revokes that same credential
Then the revoke request itself completes successfully
And only subsequent calls with that credential are refused

#### Scenario: Re-running rotation against an already-retired credential is harmless
Given a credential was retired in a previous rotation cycle
When the rotation job revokes it again
Then the request succeeds without error

### Acceptance Criteria
- [ ] A bearer can revoke its OWN jti; the current request succeeds, the NEXT request with that
      credential is refused (401).
- [ ] Revoke-self is idempotent across rotation cycles.
- [ ] Revoke-self is gated by the same authz as revoke-other (a non-management bearer cannot
      revoke-self either — though revoke-self is the safest mutation, the gate is uniform).

### Outcome KPIs
- **Who**: scheduled rotation jobs
- **Does what**: complete a credential rotation hands-free, retiring the old credential themselves
- **By how much**: 100% of rotations leave zero live leftover credentials, with no human step for the
  retire phase
- **Measured by**: rotation-job success rate + a "old credential refused after self-revoke" assertion
- **Baseline**: 0 (the retire step is human-UI-only today)

### Technical Notes
- Revoke-self is a subset of `revoke_token` (the caller's jti is in its workspace).
- The in-flight request succeeds because authentication already happened; the denylist bites the
  NEXT call. No special-casing needed.

---

# ==========================================================================
# Slice 3 — Trust the contract: stable codes + non-enumerable refusal boundary
# US-TMA04 (stable contract) + US-TMA05 (evil-caller boundary + rate guardrail)
# ==========================================================================

## US-TMA04: A stable, machine-readable contract across the token routes

- **job_id**: mt-api-job-3

### Elevator Pitch
- **Before**: without a guaranteed-stable contract, an integrator must string-match prose errors and
  guess whether a 403 means "no such token" or "not allowed" — brittle and leaky.
- **After**: every token-route outcome is the stable envelope — a 2xx with the resource (e.g. the
  `[…]` token array, a `204` on revoke) or `{"error":{"code":"forbidden","message":"forbidden"}}`
  with the conventional HTTP status — so a script branches on `error.code` reliably; a read-after-
  write LIST returns the same shape it returned before.
- **Decision enabled**: an integrator decides to build an SDK/automation against a published,
  predictable contract, confident the codes and shapes will not surprise them.

### Problem
The integrator (Sven) wants to write the integration ONCE against a stable contract. The shipped
`status_for` envelope + status conventions + non-enumerable 401 exist for issue/comment routes; this
story guarantees the NEW token routes inherit the SAME contract rather than inventing new shapes.

### Who
- **Integrator** (Sven Aarø) | writes the integration/SDK once | wants stable `error.code`s,
  conventional statuses, and read-after-write consistency.

### Solution
The token routes use `status_for` for every refusal/validation (401/403/404/422 + specific code) and
return resources in the SHIPPED envelope shape. The LIST shape is consistent across reads
(read-after-write equality). No new error shapes are introduced.

### Domain Examples
#### 1. Happy path — branchable codes
Sven's SDK switches on `error.code`: `forbidden` → "ask an admin to grant management"; `not_found` →
"token already gone"; `unauthorized` → "credential invalid/revoked". Each maps to one HTTP status.
#### 2. Edge case — read-after-write
Sven lists tokens, revokes one, lists again. The revoked token now shows `"revoked": true`; every
other field is byte-identical to the first read.
#### 3. Error/boundary — no prose-only error
Every refusal carries a stable `code`, never just a human message — Sven never string-matches prose.

### UAT Scenarios (BDD)
#### Scenario: Every token-route refusal carries a stable machine-readable code
Given Sven's integration calls the token routes
When any call is refused
Then the response is a JSON envelope with a stable error code and the conventional HTTP status
And the integration can branch on the code without parsing prose

#### Scenario: A listed token reflects its revocation on the next read
Given a workspace has an active token
When a management caller revokes it and then lists tokens again
Then that token now shows as revoked
And all its other fields are unchanged from the previous read

#### Scenario: The token routes use the same envelope as the rest of the API
Given the API already returns a stable error envelope for issues and comments
When a token route returns an error
Then it uses the identical envelope shape and status conventions

### Acceptance Criteria
- [ ] Every non-2xx token-route response is the SHIPPED `{error:{code,message}}` envelope with the
      conventional status (NFR-TMA-CON-01).
- [ ] The LIST shape is consistent read-after-write (NFR-TMA-CON-02).
- [ ] No new error shape is introduced; `status_for` is reused unchanged.

### Outcome KPIs
- **Who**: integrators building automation/SDKs
- **Does what**: branch on stable `error.code`s instead of string-matching prose
- **By how much**: 100% of token-route refusals carry a stable code + conventional status
- **Measured by**: a contract assertion per route + integrator-reported breakage count (target 0)
- **Baseline**: N/A (routes are new; inherits the shipped envelope guarantee)

### Technical Notes
- Reuse `status_for` and the `ErrorBody`/`ErrorDetail` shapes unchanged.

---

## US-TMA05: The refusal boundary — non-enumerable, escalation-bounded, rate-guarded

- **job_id**: mt-api-job-1

### Elevator Pitch
- **Before**: a programmatic management surface invites probing — a script can hammer it, guess
  cross-workspace ids, or try to escalate; without an explicit boundary it can leak existence or
  enable a DoS.
- **After**: a non-management bearer hitting any token route gets
  `403 {"error":{"code":"forbidden"}}`; a cross-workspace or unknown jti on revoke gets the identical
  `404 {"error":{"code":"not_found"}}`; a revoked/expired/forged bearer gets the identical
  `401 {"error":{"code":"unauthorized"}}`; and a revoke storm beyond the guardrail is throttled.
- **Decision enabled**: a security reviewer decides the surface is safe to publish — it leaks no
  existence oracle, confines escalation to the workspace, and cannot be turned into a runaway.

### Problem
A machine-token management surface must be safe against a hostile caller ("Malicious Mike"): no
existence oracle across workspaces, no privilege escalation (the Q-AUTHZ crux), and no abuse loop.
The use-cases already enforce non-enumerable refusals and workspace isolation; this story makes those
guarantees EXPLICIT and TESTED on the token routes, and adds the SEC-07 rate guardrail.

### Who
- **Security automation / reviewer** (Dana) and the "evil caller" (Malicious Mike) | validates the
  boundary holds | wants proof of non-enumerability + escalation bounding before publishing.

### Solution
Adversarial scenarios assert: non-management → 403 (non-enumerable); cross-workspace/unknown jti →
identical 404; revoked/expired/forged/`alg:none`/wrong-alg bearer → identical 401; revoke storm →
throttled per the SEC-07 guardrail. **And the escalation boundary: there is NO bearer-reachable mint
route in v1** (no self-replication possible) — asserted as a route-surface check.

### Domain Examples
#### 1. Happy path (defense holds) — cross-workspace probe yields nothing
Mike (management bearer in workspace A) iterates guessed jtis against workspace B's revoke route →
every one returns the identical 404. No id is ever confirmed.
#### 2. Edge case — non-management probe of LIST
Mike's non-admin-bound bearer probes the list endpoint repeatedly → always 403, never any registry
content; no signal about how many tokens exist.
#### 3. Error/boundary — no mint surface to escalate through
Mike tries `POST /api/v1/.../tokens` to mint → there is no such route in v1 (404/405 per DESIGN); he
cannot self-replicate. The only mint surface is the human session UI.

### UAT Scenarios (BDD)
#### Scenario: A non-management caller is refused non-enumerably on every token route
Given a bearer bound to a non-admin user
When it calls the list and revoke routes
Then every call is refused as forbidden
And no response reveals how many tokens exist or whether any specific token exists

#### Scenario: Cross-workspace and unknown ids are indistinguishable
Given a management bearer for workspace A
When it attempts to revoke an id belonging to workspace B and an id that exists nowhere
Then both attempts return the identical not-found refusal

#### Scenario: An invalid or revoked credential is refused identically
Given a credential that is expired, forged, revoked, or uses a disallowed algorithm
When it calls any token route
Then every case is refused as unauthorized with the byte-identical response

#### Scenario: There is no programmatic mint surface to escalate through
Given the ratified v1 model keeps minting human-session-only
When a machine caller attempts to mint a token via the API
Then no programmatic mint route exists
And the caller cannot create a new credential from a bearer token

#### Scenario: A burst of revocations is throttled
Given a caller issues management revocations far beyond the normal rate
When the burst exceeds the configured guardrail
Then excess requests are throttled
And the per-principal mutation rate is observable as a guardrail metric

### Acceptance Criteria
- [ ] A non-management bearer is refused (403) on every token route, non-enumerably (NFR-TMA-SEC-03).
- [ ] Cross-workspace and unknown jti return the identical non-enumerable 404.
- [ ] Expired/forged/revoked/`alg:none`/wrong-alg bearers all return the identical 401
      (reused from `token_auth`).
- [ ] No bearer-reachable mint route exists in v1 (route-surface assertion; NFR-TMA-SEC-08).
- [ ] A management-mutation burst beyond the guardrail is throttled; the per-principal rate is a
      guardrail metric (NFR-TMA-SEC-07).

### Outcome KPIs
- **Who**: the hostile caller (negative outcome) / the security reviewer (assurance)
- **Does what**: fails to enumerate, escalate, or run away — and the reviewer confirms it
- **By how much**: 100% of adversarial calls refuse non-enumerably; 0 bearer mint routes; bursts
  throttled
- **Measured by**: the adversarial acceptance suite + the route-surface assertion + the rate-guardrail
  metric
- **Baseline**: N/A (new surface; inherits the shipped non-enumerable refusals + the v1 no-mint rule)

### Technical Notes
- Non-enumerable refusals + identical 401 are reused from `revoke_token` + `token_auth`.
- The no-mint-route guarantee is the v1 expression of the Q-AUTHZ ratification (option c).
- The rate guardrail mechanism + numbers are DESIGN (Q-RATE-LIMIT).

---

# ==========================================================================
# DEFERRED (NOT in v1): programmatic MINT — documented for the follow-on slice
# ==========================================================================

> A future **US-TMA06 (mt-api-job-4)** would expose `POST /api/v1/.../tokens` to mint a credential
> and return the one-time value once. It is **OUT of v1** under the ratified authz model (option c):
> a bearer that can MINT is a self-replication surface (the mint loop — see `wave-decisions.md`
> escalation analysis). If ratified later, it ships as its OWN slice with: option (b)'s explicit
> `tokens:manage` capability claim (never reachable by a plain admin-bound token); a "management
> tokens cannot mint management tokens" anti-self-replication rule; the SEC-07 mint-rate guardrail;
> the SHIPPED one-time-value guarantees (SecretString, never persisted/logged, never re-fetchable);
> and the Q-SIGNER-WIRING work to reach `AppState.machine_token_signer` from foundry-api. See
> `out-of-scope.md`.
