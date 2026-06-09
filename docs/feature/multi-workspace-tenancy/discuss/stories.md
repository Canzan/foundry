<!-- markdownlint-disable MD024 -->
# Multi-Workspace Tenancy — User Stories

> This feature makes Foundry's tenancy REAL: multiple workspaces coexist in one instance with
> genuine per-tenant data isolation across every surface (web htmx tier + JSON `/api/v1` +
> machine-token auth + sign-in/sessions). Foundry is SINGLE-workspace today, enforced by
> `CREATE UNIQUE INDEX uniq_one_workspace ON workspaces ((true))`
> (`crates/foundry-store/migrations/0001_init.sql:15`). Every tenant table already carries a
> `workspace_id` FK and every query already scopes by it; the code already collapses "not found"
> and "not yours" into one non-enumerable response (`attachments.rs`). What is missing is that
> more than one workspace can exist, and a request→workspace RESOLUTION seam. Every story is
> solution-neutral on the things DESIGN owns (tenant model, resolution mechanism, selection UX,
> instance-super-admin role, per-surface refusal shape). Personas + jobs: see `jobs.yaml`. The
> open product decisions (OD-1..OD-5): see `wave-decisions.md`.

## System Constraints (cross-cutting)

Apply to every story; measurable forms live in `nfrs.md`.

- **Isolation is FAIL-CLOSED and NON-ENUMERABLE on EVERY surface.** A request for another
  tenant's resource is refused IDENTICALLY to a request for a non-existent one
  (NFR-MWT-SEC-01/02), generalizing the shipped `attachments.rs find_attachment_for_requester`
  pattern.
- **Reuse, don't rebuild, the substrate.** The per-table `workspace_id` scoping, the
  non-enumerable lookup pattern, `is_workspace_admin(workspace_id, user_id)`,
  `is_team_member(team_id, user_id)`, the machine-token verify path + `jti` denylist are SHIPPED
  and REUSED. This feature drops the single-workspace guard, adds a resolution seam, and proves
  the boundary — it does not reimplement scoping or auth.
- **The acting workspace is resolved at a SINGLE auditable seam.** Exactly one
  `${acting_workspace_id}` per request; fail-closed if none resolvable (NFR-MWT-SEC-03/06).
- **Migration is forward-only (ADR-003), no data rewrite, no data loss.** The existing single
  workspace becomes workspace 1; existing sessions/tokens/sign-in keep working (NFR-MWT-DATA-*).
- **One binary, no new runtime services.** The web admin surface preserves the existing browser
  auth/CSRF/session contract unchanged.
- **Solution-neutral.** Tenant model, resolution mechanism, selection UX, instance-super-admin
  role, per-surface refusal status/shape, and backup/restore granularity are DESIGN.

## Glossary (additions for this feature)

- **Tenant / workspace**: a `workspaces` row and everything FK'd to it. "Tenant" is the
  isolation concept; "workspace" is the existing schema name.
- **Acting workspace** (`${acting_workspace_id}`): the single workspace a request is resolved to
  act on.
- **Resolution seam**: the one place a request's acting workspace is determined (mechanism is
  DESIGN — DM1).
- **Non-enumerable refusal**: "missing" and "not yours" collapse to one indistinguishable
  response (no existence oracle).
- **Multi-membership**: one user/email may belong to several workspaces (the schema supports it;
  OD-2/DM4).
- **Instance super-admin**: a NEW instance-level role above workspace-`admin`, proposed for
  provisioning authority (OD-3/DM6).
- **`uniq_one_workspace`**: the unique index forbidding a second workspace today; this feature
  drops it.

---

# ==========================================================================
# Slice 1 — Walking Skeleton: two workspaces coexist + request→workspace resolution
# US-MWT00 (drop guard + resolution seam, @infrastructure) + US-MWT01 (coexist + resolve)
# ==========================================================================

## US-MWT00: Make more than one workspace possible and resolve a request to its workspace

- **job_id**: infrastructure-only
- **infrastructure_rationale**: This story enables no user decision on its own — it is the
  substrate every user-visible isolation story stands on. (1) Today
  `CREATE UNIQUE INDEX uniq_one_workspace ON workspaces ((true))` forbids a second workspace;
  until it is dropped, no multi-tenant behavior can exist. (2) With many workspaces possible, a
  request needs a single, auditable answer to "which workspace does this act on?" — the
  resolution seam. Neither is observable to a user by itself; both are folded into Slice 1 and
  never ship standalone. The user-visible outcome they enable is US-MWT01 (two workspaces
  coexist and a request sees only its own).

### Problem
The schema is multi-tenant-shaped — every tenant table FKs to `workspaces(id)` and every query
scopes by `workspace_id` — but `uniq_one_workspace` (migration `0001_init.sql:15`,
"I-W1: at most one workspace per instance") makes a second workspace impossible, and "the
workspace" is implicit because there is only one. Multi-tenant needs the guard gone AND an
explicit resolution of the acting workspace per request.

### Who
- **The platform** (no direct user). Enables Sasha (US-MWT01), Marco/Priya (US-MWT02/03), and
  the migration (US-MWT06).
- **Context**: `crates/foundry-store/migrations/0001_init.sql:15` (`uniq_one_workspace`); the
  per-table `workspace_id` scoping in `crates/foundry-store/src/lib.rs`; the session/bearer auth
  in `foundry-app`/`foundry-api` that establishes the acting user.
- **Motivation**: a safe foundation on which the isolation boundary can be proven.

### Solution
(1) A forward-only migration DROPS `uniq_one_workspace` (no other schema change; existing data
untouched). (2) Introduce a request→workspace RESOLUTION seam that yields EXACTLY one
`${acting_workspace_id}` per request and is fail-closed if none resolves (the mechanism — session
claim, URL segment, host, or token claim — is DESIGN, DM1/OD-1/OD-2). Tenant-scoped queries are
fed the resolved workspace (they already take `workspace_id`). For an upgraded single-workspace
install, resolution defaults to the one existing workspace (ties into US-MWT06).

### Domain Examples
#### 1. Happy path — a second workspace can now exist
On a fresh build, "Acme" exists. The platform inserts a second `workspaces` row "Globex"; the
insert succeeds (the unique guard is gone). Both rows coexist.
#### 2. Edge case — an upgraded single-workspace install
A pre-feature instance has exactly one workspace. After the migration drops the guard, that one
workspace still resolves for every existing session — nothing about the single-tenant experience
changes (the user-visible safety of this is US-MWT06).
#### 3. Error/boundary — a request that resolves to no workspace
A request arrives that cannot be resolved to any workspace the actor belongs to. Resolution
fails CLOSED — the request is refused, never defaulted to "the first" workspace.

### UAT Scenarios (BDD)
#### Scenario: A second workspace can be created where none could before
Given an instance that already has one workspace
When a second workspace is created
Then both workspaces exist on the instance
And neither creation is blocked by a single-workspace limit

#### Scenario: A request acts on exactly the workspace it resolves to
Given two workspaces exist and a request is associated with one of them
When the request is handled
Then it acts on exactly that one workspace's data
And it cannot reach the other workspace's data

#### Scenario: A request that resolves to no workspace is refused, not defaulted
Given a request that cannot be resolved to any workspace the actor belongs to
When the request is handled
Then it is refused
And it is not served against any workspace

### Acceptance Criteria
- [ ] The single-workspace guard is removed by a forward-only migration; a second workspace can
  be created.
- [ ] A request resolves to exactly one acting workspace, available to the tenant-scoped query
  layer.
- [ ] Resolution is fail-closed: a request with no resolvable workspace is refused, never
  defaulted to an arbitrary workspace.
- [ ] On an upgraded single-workspace install, the one existing workspace resolves unchanged for
  existing sessions.
- [ ] No existing tenant data is rewritten or moved by the migration.

### Outcome KPIs
- See KPI-MWT-01 (US-MWT01) — this story is the substrate; its own measurable is the migration +
  resolution being in place: a second workspace exists, and 0 requests are served against an
  unresolved workspace.

### Technical Notes
- Drops `uniq_one_workspace` (`0001_init.sql:15`) via a NEW forward-only migration (ADR-003).
- The resolution mechanism is DESIGN (DM1): session claim vs URL vs host vs token claim; must
  account for multi-membership (OD-2).
- Reuses the per-table `workspace_id` scoping already in `foundry-store`.
- **Assumption to verify (wave-decisions #6)**: no query depends on `uniq_one_workspace` for
  correctness; audit for un-scoped `FROM teams|projects|issues|invites` before dropping it.

---

## US-MWT01: Two workspaces coexist and each request sees only its own data

- **job_id**: mwt-job-1 (Host several independent teams in one instance without standing up a separate instance each)

### Elevator Pitch
- **Before**: Sasha can host only one team per Foundry instance — a second team means a second
  Postgres, a second process, a second deploy.
- **After**: she runs two workspaces ("Acme" and "Globex") in ONE instance; a member of Acme who
  lists issues (the web board, or `GET /api/v1/issues`) sees ONLY Acme's issues, and a member of
  Globex sees ONLY Globex's — proven with real coexisting data, not synthetic uuids.
- **Decision enabled**: Sasha decides she can consolidate two clients onto one box, because each
  request demonstrably acts only on its own workspace.

### Problem
Sasha (instance operator) cannot host two teams in one instance: `uniq_one_workspace` caps her
at one. Even though the schema and queries are already workspace-scoped, that scoping has never
been exercised against a real second tenant — so she has no basis to trust consolidation.

### Who
- **Sasha Okonkwo**, instance operator, wants to host Acme and Globex on one box.
- **Marco Bianchi**, a member of Acme, who must see only Acme's data.
- **Context**: two real `workspaces` rows coexist (US-MWT00 dropped the guard); each request
  resolves to its workspace; one read path (e.g. listing issues) returns only the acting
  workspace's rows.
- **Motivation**: trustworthy consolidation — one instance, many sealed tenants.

### Solution
With the guard dropped and resolution in place (US-MWT00), demonstrate the thinnest end-to-end
multi-tenant flow on ONE surface: two real workspaces, each with their own members and issues;
a member of A lists issues and sees ONLY A's; a member of B sees ONLY B's. The read path already
filters by `workspace_id` — this story proves it with REAL A/B fixtures (the walking skeleton
for the isolation boundary). Cross-tenant reach is covered by US-MWT02+; here the focus is the
happy-path coexistence + own-workspace-only read.

### Domain Examples
#### 1. Happy path — Marco sees only Acme's issues
Acme has issues ACME-1..3; Globex has GLOBEX-1..2. Marco (Acme member) lists his issues and sees
exactly ACME-1..3 — no Globex issue appears.
#### 2. Edge case — a Globex member sees only Globex
Lucia (Globex member) lists her issues and sees exactly GLOBEX-1..2 — no Acme issue appears.
#### 3. Error/boundary — empty second workspace
A freshly created "Sandbox" workspace has no issues; its member sees an empty list, not Acme's
or Globex's data.

### UAT Scenarios (BDD)
#### Scenario: Two workspaces coexist and a member sees only their own
Given workspace "Acme" and workspace "Globex" both exist on one instance
And Marco is a member of "Acme" with issues in "Acme"
When Marco lists his issues
Then he sees only "Acme" issues
And no "Globex" issue appears

#### Scenario: Each workspace's members see a disjoint set of data
Given "Acme" and "Globex" each have their own members and issues
When an "Acme" member and a "Globex" member each list their issues
Then each sees only their own workspace's issues
And neither set contains the other's

#### Scenario: A brand-new workspace starts empty, not populated from a neighbour
Given a newly created "Sandbox" workspace with no issues
When its member lists issues
Then the list is empty
And no other workspace's issues appear

### Acceptance Criteria
- [ ] Two (or more) workspaces coexist in one instance with their own members and data.
- [ ] A member of one workspace, on the chosen read path, sees only that workspace's data.
- [ ] No data from another coexisting workspace appears in any member's read.
- [ ] The scenario uses REAL coexisting workspaces (real members, real issues), not synthetic
  uuids.

### Outcome KPIs
- See KPI-MWT-01 and KPI-MWT-E1: from a hard cap of 1 workspace to N coexisting isolated
  workspaces; 100% of reads return only the acting workspace's data on the proven path.

### Technical Notes
- Depends on **US-MWT00** (guard dropped + resolution seam).
- Reuses the shipped `workspace_id`-scoped read path (e.g. `list_issues`-style queries).
- Surface-neutral: the proof can be the web board or `GET /api/v1/issues` (DESIGN picks the
  skeleton surface; Slice 2 generalizes to the full web tier).

---

# ==========================================================================
# Slice 2 — Tenant-scoped authz + non-enumerable refusal on the WEB htmx tier
# ==========================================================================

## US-MWT02: A member or admin of one workspace cannot reach another's data or admin functions on the web

- **job_id**: mwt-job-2 (Be certain my tenant's data — and even its existence — is invisible to every other tenant)

### Elevator Pitch
- **Before**: Marco's isolation on the web tier is only ASSUMED — it has never been tested
  against a real second workspace, so a stale link or crafted id to a Globex resource has
  unknown behavior.
- **After**: on the web htmx tier, every read/write Marco makes is scoped to Acme; a request for
  a Globex issue/project/attachment by id is refused identically to a non-existent one, and an
  Acme admin's attempt to manage Globex's members is refused — proven with real A/B fixtures.
- **Decision enabled**: Marco's workspace owner (and Sasha) decide the web tier is safe for
  shared hosting, because a neighbour — even a crafted request — cannot read, write, or
  enumerate their data.

### Problem
The web htmx tier's tenant scoping has never been exercised against a real second tenant. A
member of A with a crafted or stale id pointing at B's resource, or an A-admin reaching for B's
member list, must be refused — but today (one workspace) this is untested. A single un-scoped
query or an existence-revealing refusal would leak B to A.

### Who
- **Marco Bianchi**, member of Acme ("Malicious Mike"/"Careless Cathy" stand-in for the
  evil-user paths).
- **Priya Nandakumar**, admin of Acme but NOT of Globex.
- **Context**: every web read/write path scoped by `${acting_workspace_id}`; admin actions gated
  by `is_workspace_admin(${acting_workspace_id}, acting_user)`; the non-enumerable lookup
  generalized from `attachments.rs`.
- **Motivation**: a provably sealed web tier on a shared instance.

### Solution
On the web htmx tier, feed every tenant-scoped read/write the resolved `${acting_workspace_id}`
(reusing the shipped scoped queries), gate every admin action with
`is_workspace_admin(${acting_workspace_id}, …)`, and make a request for a resource outside the
acting workspace return the SAME non-enumerable refusal as a non-existent resource (generalizing
`find_attachment_for_requester`'s `WHERE id = $1 AND workspace_id = $2` idiom). Prove it with
real Acme/Globex fixtures.

### Domain Examples
#### 1. Happy path — Marco works entirely within Acme
Marco opens his board, an issue, an attachment — all Acme, every time; he never sees a workspace
concept.
#### 2. Edge case — Priya admins Acme but not Globex
Priya manages Acme's members and teams freely; when a crafted request tries to manage Globex's
members while she acts on Acme, it is refused (she is not a Globex admin).
#### 3. Error/boundary — a crafted id pointing at Globex
Marco follows a stale/crafted link to a Globex issue's id. The page is refused identically to a
non-existent issue — nothing reveals the Globex issue is real.

### UAT Scenarios (BDD)
#### Scenario: A member sees and edits only their own workspace's resources on the web
Given Marco is a member of "Acme" and resources exist in both "Acme" and "Globex"
When Marco browses boards, issues, and attachments on the web
Then he sees only "Acme" resources
And he can edit only "Acme" resources

#### Scenario: Reaching another workspace's resource by id is refused non-enumerably (evil-user)
Given an issue (or attachment, or project) belongs to "Globex"
When Marco (a member of "Acme") requests that resource by its id on the web
Then the request is refused identically to a request for a non-existent resource
And nothing reveals that the "Globex" resource exists

#### Scenario: An admin of one workspace cannot manage another's members (evil-user)
Given Priya is an admin of "Acme" but not of "Globex"
When a request tries to manage "Globex" members or teams while Priya acts on "Acme"
Then it is refused
And no "Globex" membership is changed

### Acceptance Criteria
- [ ] Every web read/write is scoped to the acting workspace; no other workspace's data appears
  or can be edited.
- [ ] A request for a resource outside the acting workspace is refused identically to a
  non-existent resource (no existence oracle).
- [ ] Admin actions on the web are gated by `is_workspace_admin` for the acting workspace; an
  admin of A cannot manage B.
- [ ] All scenarios use REAL Acme/Globex fixtures (real members, admins, issues, projects,
  attachments), not synthetic uuids.

### Outcome KPIs
- See KPI-MWT-02: 0 B-rows in any of A's web reads; 0 A-writes affecting B; A-admin authority in
  B refused 100%.

### Technical Notes
- Depends on **US-MWT00/01** (resolution + coexistence).
- Reuses `is_workspace_admin`, `is_team_member`, the per-table `workspace_id` scoping, and the
  `attachments.rs` non-enumerable lookup pattern.
- Solution-neutral on the refusal status/shape (404 vs 403 — DESIGN; must be uniform per
  NFR-MWT-SEC-02).

---

# ==========================================================================
# Slice 3 — Propagate isolation to the JSON /api/v1 + machine-token + sign-in/session surfaces
# ==========================================================================

## US-MWT03: A machine token or API caller bound to one workspace cannot act on another

- **job_id**: mwt-job-2 (Be certain my tenant's data — and even its existence — is invisible to every other tenant)

### Elevator Pitch
- **Before**: a machine token's workspace binding is enforced only against the single existing
  workspace; a token's ability to reach ANOTHER workspace's resources has never been tested for
  real.
- **After**: a token bound to Acme that calls `GET /api/v1/...` or `DELETE
  /api/v1/.../tokens/{jti}` acts ONLY on Acme; a call targeting a Globex resource is refused
  identically to a non-existent one — proven with a real Acme-bound token against real Globex
  data.
- **Decision enabled**: an integration owner (Marco) and a security reviewer (Dana) decide the
  API is safe for multi-tenant hosting, because a leaked or misused token is confined to its own
  workspace.

### Problem
`/api/v1` callers and machine-token principals carry a workspace binding
(`machine_tokens.workspace_id`), but with one workspace that binding has never had to REFUSE a
cross-tenant call. A token bound to A that could reach B would be a cross-tenant credential leak
— the highest-impact isolation failure.

### Who
- **Marco Bianchi**, integration owner, whose Acme-bound token must never touch Globex.
- **Dana Whitfield**, security reviewer, who must be able to prove this.
- **Context**: the `MachinePrincipal` bearer extractor + the `is_workspace_admin` gate + the
  per-request `jti` denylist (shipped); the token's `workspace_id` is the authoritative acting
  workspace for `/api/v1`.
- **Motivation**: a token's blast radius is exactly its own workspace.

### Solution
On `/api/v1`, resolve the acting workspace from the token's `${token.workspace_id}` (for bearer
principals) or the session (for session-authenticated API calls), feed it to the scoped queries,
and refuse any call targeting a resource outside it identically to a non-existent resource.
Reuse the shipped verify path + denylist unchanged — this scopes WHICH workspace, not how the
token is verified. Prove with a real Acme-bound token against real Globex resources.

### Domain Examples
#### 1. Happy path — Acme token acts on Acme
An Acme-bound token lists/edits Acme issues and lists/revokes Acme tokens via `/api/v1` — all
succeed within Acme.
#### 2. Edge case — Acme token revokes only Acme tokens
A `DELETE /api/v1/.../tokens/{jti}` for an Acme token succeeds; the same call for a Globex token
is refused as not-found (the jti is invisible to the Acme-bound caller).
#### 3. Error/boundary — Acme token targets a Globex project/issue
An Acme-bound token requests a Globex project by id → refused identically to a non-existent
project; nothing reveals it exists.

### UAT Scenarios (BDD)
#### Scenario: A workspace-bound token acts only on its own workspace via the API
Given a machine token is bound to "Acme"
When a caller uses it to list and edit resources on /api/v1
Then it can act only on "Acme" resources
And it cannot read or change any "Globex" resource

#### Scenario: A cross-workspace API call is refused non-enumerably (evil-user)
Given a machine token is bound to "Acme"
When a caller uses it against a "Globex" resource on /api/v1
Then the call is refused identically to a request for a non-existent resource
And nothing reveals the "Globex" resource exists

#### Scenario: A token can revoke only its own workspace's tokens
Given an "Acme"-bound token and a token belonging to "Globex"
When the "Acme" token tries to revoke the "Globex" token via /api/v1
Then the revoke is refused as not-found
And the "Globex" token remains active

### Acceptance Criteria
- [ ] A `/api/v1` request resolves its acting workspace from the token binding (bearer) or
  session, and acts only on that workspace.
- [ ] A call targeting a resource outside the acting workspace is refused identically to a
  non-existent resource (NFR-MWT-SEC-02/05).
- [ ] Revoke/list on `/api/v1` is confined to the acting workspace's tokens; a foreign jti is
  not-found.
- [ ] The shipped verify path, `jti` denylist, and `iss`/`aud`/EdDSA pinning are unchanged.
- [ ] Scenarios use a REAL Acme-bound token against REAL Globex resources.

### Outcome KPIs
- See KPI-MWT-03: 0 cross-tenant bearer calls succeed; foreign-id ≡ missing-id on `/api/v1`.

### Technical Notes
- Depends on **US-MWT00/01**; complements **US-MWT02** (same boundary, API surface).
- Reuses the `MachinePrincipal` extractor, `is_workspace_admin`, the `jti` denylist, and
  `foundry_services::tokens` use-cases (already workspace-scoped + 100% mutation-hardened).
- **Assumption to verify (wave-decisions #5)**: a token's `workspace_id` is the authoritative
  acting workspace for `/api/v1`.

---

## US-MWT04: A signed-in session resolves to exactly one workspace, fail-closed

- **job_id**: mwt-job-2 (Be certain my tenant's data — and even its existence — is invisible to every other tenant)

### Elevator Pitch
- **Before**: "the workspace" is implicit because only one exists; sign-in never has to choose,
  and a session never has to resolve which of several workspaces it acts on.
- **After**: when Marco signs in, his session resolves to exactly one acting workspace; if he
  belongs to several (multi-membership, OD-2), the choice is explicit; a session that cannot be
  resolved to a workspace is refused, never defaulted.
- **Decision enabled**: Sasha (and the protected tenant) trust that no request is ever served
  against a guessed or defaulted workspace — every session acts on exactly one, intentionally.

### Problem
With multiple workspaces and multi-membership (a user may belong to several — the schema
supports it: `users` is global, membership is the M:N `workspace_memberships`), sign-in and
session handling must establish EXACTLY ONE acting workspace per request. An ambiguous or absent
resolution that silently defaults to "the first" workspace would be a cross-tenant hazard.

### Who
- **Marco Bianchi**, member of Acme (and possibly others), signing in.
- **A multi-membership user** (e.g. a contractor in both Acme and Globex).
- **Context**: the shipped sign-in (argon2id, brute-force delay, non-enumerable error,
  tower-sessions Postgres store, 30-day cookie) plus the NEW resolution seam; selection UX is
  DESIGN (OD-2).
- **Motivation**: an unambiguous, fail-closed acting workspace for every session.

### Solution
Resolve the acting workspace for each session at the single resolution seam. A single-membership
user resolves to their one workspace automatically. A multi-membership user has an EXPLICIT
choice (selection/switcher UX — DESIGN, OD-2). A session that resolves to no workspace the user
belongs to is refused, never defaulted. The sign-in security contract (argon2id, brute-force
delay, non-enumerable error) is unchanged; resolution is added on top.

### Domain Examples
#### 1. Happy path — single-membership user
Marco belongs only to Acme; he signs in and his session acts on Acme with no extra step.
#### 2. Edge case — multi-membership user
A contractor belongs to Acme and Globex; on sign-in (or via a switcher) she explicitly picks one;
her session acts on exactly that one until she switches.
#### 3. Error/boundary — no resolvable workspace
A user whose membership has been removed signs in but resolves to no workspace; the session is
refused/empty, never defaulted to an arbitrary workspace.

### UAT Scenarios (BDD)
#### Scenario: A single-membership user is resolved to their one workspace automatically
Given Marco belongs to exactly one workspace "Acme"
When he signs in
Then his session acts on "Acme"
And he is never asked to choose

#### Scenario: A multi-membership user chooses which workspace they are acting in
Given a user belongs to both "Acme" and "Globex"
When she signs in
Then she acts on exactly one chosen workspace
And her requests are scoped to that chosen workspace until she changes it

#### Scenario: A session with no resolvable workspace is refused, not defaulted
Given a user who currently belongs to no workspace
When their session is handled
Then it is refused or empty
And it is not served against any workspace's data

### Acceptance Criteria
- [ ] A single-membership user's session resolves to their one workspace with no extra step.
- [ ] A multi-membership user explicitly determines the acting workspace; their requests are
  scoped to it until changed.
- [ ] A session that resolves to no workspace is refused, never defaulted (fail-closed).
- [ ] The existing sign-in security contract (argon2id, brute-force delay, non-enumerable error,
  session cookie attrs) is unchanged.

### Outcome KPIs
- See KPI-MWT-04: 100% of sessions resolve to exactly one workspace; 0 served against an
  unresolved workspace.

### Technical Notes
- Depends on **US-MWT00** (resolution seam).
- **OD-2 (multi-membership) must be ratified by the user before DESIGN** — it determines whether
  a selection UX is needed at all and what shape it takes.
- Reuses the shipped sign-in + tower-sessions machinery; adds resolution, does not weaken auth.

---

# ==========================================================================
# Slice 4 — Cross-tenant non-enumerability hardening
# ==========================================================================

## US-MWT05: Another tenant's existence and resources are invisible from my workspace on every surface

- **job_id**: mwt-job-2 (Be certain my tenant's data — and even its existence — is invisible to every other tenant)

### Elevator Pitch
- **Before**: refusals are non-enumerable for attachments (the shipped pattern) but not yet
  uniformly proven across every surface — a 403-vs-404 (or timing/shape) difference somewhere
  could reveal that a foreign resource exists.
- **After**: on EVERY surface (web, `/api/v1`, token revoke, admin actions), a request for
  another tenant's resource is refused IDENTICALLY to a non-existent one — no status, body,
  timing, or shape oracle — covered by adversarial tests across all surfaces.
- **Decision enabled**: Dana (security reviewer) certifies that tenant A cannot even DISCOVER
  tenant B, let alone read it — the shared instance meets the non-enumerability bar.

### Problem
Isolation is only complete if a cross-tenant actor learns NOTHING — not even that a foreign
resource exists. A single surface where "exists but forbidden" (403) differs from "does not
exist" (404), or where timing/shape leaks existence, is an enumeration oracle. The shipped
`attachments.rs` pattern is the right idiom; this story makes it UNIFORM and adversarially
proven everywhere.

### Who
- **Marco Bianchi** (member of Acme), as the probing/hostile actor.
- **Dana Whitfield**, security reviewer, who must certify non-enumerability.
- **Context**: every cross-tenant refusal path on every surface (web reads/writes, admin
  actions, `/api/v1` reads/revokes); the uniform non-enumerable refusal contract
  (NFR-MWT-SEC-02).
- **Motivation**: a neighbour cannot enumerate or even detect another tenant.

### Solution
Make the cross-tenant refusal UNIFORM across surfaces: foreign-resource-id and never-existed-id
produce the same observable response (status + body shape; no timing/shape oracle), generalizing
`find_attachment_for_requester`. Add adversarial acceptance coverage on each surface (web,
`/api/v1`, token revoke, admin actions) asserting foreign-id ≡ missing-id. This story is the
explicit hardening + adversarial proof of the boundary that US-MWT02/03/04 establish.

### Domain Examples
#### 1. Happy path — uniform refusal on the web
Marco requests a Globex issue id and a never-existed id on the web; both responses are
indistinguishable.
#### 2. Edge case — uniform refusal on the API
An Acme-bound token requests a Globex project id and a never-existed id on `/api/v1`; both
responses are indistinguishable.
#### 3. Error/boundary — token revoke enumeration attempt
An Acme caller tries to revoke a Globex jti and a random never-existed jti; both are not-found,
identically — no way to tell which jti is real.

### UAT Scenarios (BDD)
#### Scenario: A foreign resource and a non-existent resource are indistinguishable on the web (evil-user)
Given a resource belongs to "Globex" and another id has never existed
When Marco (a member of "Acme") requests each on the web
Then the two responses are indistinguishable
And neither reveals which id corresponds to a real resource

#### Scenario: A foreign resource and a non-existent resource are indistinguishable on the API (evil-user)
Given a resource belongs to "Globex" and another id has never existed
When an "Acme"-bound caller requests each on /api/v1
Then the two responses are indistinguishable

#### Scenario: Probing for another workspace's tokens reveals nothing (evil-user)
Given a token belongs to "Globex" and a random jti has never existed
When an "Acme" caller tries to revoke each
Then both are refused as not-found identically
And nothing reveals which jti is real

### Acceptance Criteria
- [ ] On every surface, a request for a foreign-workspace resource is observationally identical
  to a request for a non-existent resource (status + body shape; no timing/shape oracle).
- [ ] No surface exposes a 403-vs-404 (or analogous) existence oracle for cross-tenant access.
- [ ] Adversarial scenarios cover web reads/writes, admin actions, `/api/v1` reads, and token
  revoke.
- [ ] All scenarios use REAL Acme/Globex fixtures.

### Outcome KPIs
- See KPI-MWT-05: 0 surfaces expose an existence oracle; foreign-id ≡ missing-id everywhere.

### Technical Notes
- Depends on **US-MWT02/03/04** (the surfaces whose refusals it unifies).
- Generalizes the shipped `attachments.rs find_attachment_for_requester` non-enumerable idiom.
- Solution-neutral on the chosen refusal status/shape — DISCUSS fixes that it is UNIFORM.

---

# ==========================================================================
# Slice 5 — Migrate an existing single-workspace install to workspace 1
# ==========================================================================

## US-MWT06: My existing single-workspace install upgrades to workspace 1 with no data loss

- **job_id**: mwt-job-3 (Bring my existing single-workspace install across to multi-tenant without losing or exposing data)

### Elevator Pitch
- **Before**: there is no path from a single-workspace install to multi-workspace; upgrading is
  unknown territory and feels risky.
- **After**: Sasha upgrades; her existing workspace, users, teams, projects, and issues are
  intact and become "workspace 1"; her users sign in and work exactly as before, their sessions
  and machine tokens still resolve — the migration only drops the guard, it rewrites nothing.
- **Decision enabled**: Sasha decides to upgrade in place, confident the change is forward-only
  and loses nothing.

### Problem
Every existing Foundry install is single-workspace. The upgrade must convert that one workspace
into "workspace 1" of a multi-tenant world with ZERO data loss and ZERO change to how its users
sign in and work. A migration that rewrites data, changes the workspace id, or breaks existing
sessions/tokens would be unacceptable.

### Who
- **Sasha Okonkwo**, operator of an existing single-workspace install.
- **Her existing users** (e.g. Marco), whose sessions/tokens must keep working.
- **Context**: a forward-only migration (ADR-003) that drops `uniq_one_workspace` and adds
  resolution support; the existing workspace's FKs already point at it.
- **Motivation**: a seamless, safe, in-place upgrade.

### Solution
Apply the forward-only migration (US-MWT00) to a REAL pre-feature DB: drop `uniq_one_workspace`,
add resolution support, leave the one existing workspace as workspace 1 — its id unchanged, its
data untouched (FKs already point at it). Resolution defaults every existing session to that one
workspace. Existing machine tokens (bound to that workspace) keep working. Assert row-level
before/after equality across all tenant tables and that the existing auth suites stay green.

### Domain Examples
#### 1. Happy path — upgrade keeps everything
A pre-feature DB with one workspace, 5 users, 3 teams, 12 issues, 4 tokens upgrades; afterward
all rows are present and unchanged; the workspace is now workspace 1; users sign in as before.
#### 2. Edge case — a live session/token across the upgrade
A user with an active session and a valid machine token before the upgrade still has a working
session/token after — both resolve to workspace 1.
#### 3. Error/boundary — re-running the migration
The forward-only migration is idempotent in effect (already-applied = no-op); re-running it does
not duplicate or alter the workspace.

### UAT Scenarios (BDD)
#### Scenario: Upgrading a single-workspace install keeps it working as workspace 1
Given a pre-feature instance with one workspace and live users, sessions, and tokens
When the instance upgrades to multi-workspace support
Then the existing workspace becomes the first workspace with all its data intact
And its users sign in and work exactly as before

#### Scenario: No tenant data is lost or changed by the upgrade
Given a pre-feature database snapshot with users, teams, projects, and issues
When the multi-workspace migration is applied
Then every tenant row is present and unchanged afterward
And the existing workspace's identity is unchanged

#### Scenario: Existing sessions and machine tokens still resolve after the upgrade
Given an active session and a valid machine token before the upgrade
When the upgrade is applied
Then the session and the token still work
And both resolve to the first workspace

### Acceptance Criteria
- [ ] The migration is forward-only (ADR-003) and does not edit any prior migration.
- [ ] The existing workspace becomes workspace 1 with its id and all its data unchanged
  (before/after row equality across all tenant tables).
- [ ] Existing users sign in and work exactly as before; existing sessions and machine tokens
  still resolve to workspace 1.
- [ ] The migration acceptance runs against a REAL pre-feature DB snapshot.
- [ ] The existing auth + workspace acceptance suites stay green post-migration.

### Outcome KPIs
- See KPI-MWT-06: 0 rows lost/changed; existing auth suites 100% green post-migration.

### Technical Notes
- Depends on **US-MWT00** (the migration that drops the guard) and **US-MWT04** (session
  resolution defaulting the single workspace).
- Forward-only per ADR-003; the existing workspace's FKs already point at it, so no data rewrite
  is needed.
- Mirrors the migration-safety discipline of the shipped features (no-rewrite, no-loss).

---

# ==========================================================================
# Slice 6 — Provision a new tenant + close the two residuals
# ==========================================================================

## US-MWT07: An operator creates a new workspace and seeds its first admin

- **job_id**: mwt-job-4 (Create and provision a new tenant on a running instance)

### Elevator Pitch
- **Before**: there is no way to make a second workspace; the bootstrap flow creates exactly one
  and `uniq_one_workspace` forbade more.
- **After**: Sasha provisions a new workspace (e.g. via an operator action or a seeded
  bootstrap/invite) and its first admin, and the new tenant is immediately reachable and
  isolated — no redeploy, no manual DB insert.
- **Decision enabled**: Sasha decides to onboard a new client same-day, because she can stand up
  an isolated tenant herself.

### Problem
Once more than one workspace can exist (US-MWT00), the operator needs a product path to CREATE a
workspace and seed its first admin. Today the single workspace is created at bootstrap; there is
no path to a second. WHO may create one (instance super-admin vs self-serve) is an open product
decision (OD-3).

### Who
- **Sasha Okonkwo**, instance operator (or a NEW instance super-admin — OD-3).
- **The new workspace's first admin** (seeded at creation).
- **Context**: a create-workspace path (operator action and/or a seeded bootstrap/invite),
  reusing the bootstrap-token idiom; the new workspace is isolated from creation.
- **Motivation**: self-sufficient tenant onboarding.

### Solution
Provide a path to create a new workspace with a name and a seeded first admin (mechanism +
authority are DESIGN, OD-3: instance-operator/super-admin default, no self-serve in v1). The new
workspace is isolated from all others from creation; creating it does not touch any existing
workspace. The first admin can then sign in and resolve to the new workspace.

### Domain Examples
#### 1. Happy path — provision Globex
Sasha creates "Globex" and seeds Priya as its first admin; Priya signs in, resolves to Globex,
and sees an empty, isolated workspace ready to use.
#### 2. Edge case — creating Globex does not touch Acme
After creating Globex, Acme's data, members, and sessions are entirely unaffected.
#### 3. Error/boundary — non-operator attempts to create a workspace
A workspace member (non-operator) attempts to create a workspace; it is refused (provisioning is
instance-operator/super-admin only, OD-3).

### UAT Scenarios (BDD)
#### Scenario: An operator provisions a new isolated workspace with a first admin
Given an instance is running with an existing workspace
When the operator creates a new workspace and seeds its first admin
Then the new workspace exists and is isolated from all others
And its first admin can sign in and act on it

#### Scenario: Creating a new workspace does not affect existing ones
Given an existing workspace "Acme" with data and members
When a new workspace "Globex" is created
Then "Acme" and its data and members are unchanged
And "Globex" starts empty and isolated

#### Scenario: A non-operator cannot create a workspace (authz path)
Given a regular workspace member who is not an operator
When they attempt to create a new workspace
Then the attempt is refused

### Acceptance Criteria
- [ ] An authorized operator can create a new workspace with a name and a seeded first admin.
- [ ] The new workspace is isolated from all others from the moment of creation.
- [ ] Creating a workspace does not change any existing workspace's data or members.
- [ ] A non-authorized actor cannot create a workspace (per OD-3's ratified authority).

### Outcome KPIs
- See KPI-MWT-07: time-to-new-tenant under a few minutes; the new workspace is isolated from
  creation; creating B does not touch A.

### Technical Notes
- Depends on **US-MWT00** (multiple workspaces possible).
- **OD-3 (provisioning authority + instance super-admin role) must be ratified before DESIGN** —
  it determines the surface and whether a new role concept is introduced.
- Reuses the bootstrap-token / invite idiom for seeding the first admin (the single-workspace
  bootstrap is the precedent).

---

## US-MWT08: Prove the boundary with real two-workspace fixtures and bound the per-tenant rate-bucket map

- **job_id**: mwt-job-5 (Prove the tenant boundary holds with real two-workspace fixtures and bounded per-tenant resources)

### Elevator Pitch
- **Before**: cross-workspace evil-user paths are tested with SYNTHETIC uuids (only one
  workspace could exist), and the per-principal revoke-storm rate-bucket map is bounded only by
  the single-workspace admin count — both are accepted residuals.
- **After**: the isolation acceptance suites use REAL coexisting workspaces A and B (real
  members, tokens, issues), and the rate-bucket map evicts idle/stale principals so it stays
  bounded under many tenants — the two residuals are closed.
- **Decision enabled**: Dana (security reviewer) accepts the isolation evidence as genuine (real
  fixtures, not stand-ins), and the operator trusts that per-tenant memory growth is bounded.

### Problem
Two accepted residuals waited explicitly on multi-workspace: (1) cross-workspace evil-user paths
are tested with synthetic uuids because only one workspace could exist (UI-1,
`docs/evolution/2026-06-07` + `…06-08`); (2) the per-principal revoke-storm rate-bucket `HashMap`
in `crates/foundry-app/src/rate_limit.rs` is bounded today only by the single-workspace admin
count (residual F2, "LRU / idle-eviction is the tracked mitigation for multi-workspace"). With
many tenants both become real concerns.

### Who
- **Dana Whitfield**, security reviewer, who needs genuine cross-tenant evidence.
- **Sasha Okonkwo**, operator, who needs bounded per-tenant memory.
- **Context**: the isolation acceptance fixtures (replace synthetic uuids with real A/B); the
  `rate_limit` per-principal token-bucket map (add eviction).
- **Motivation**: genuine proof + bounded resources.

### Solution
(1) Replace/augment the synthetic-uuid cross-workspace acceptance fixtures with REAL two-
workspace fixtures (A and B coexisting, each with real members, tokens, issues/projects) — these
back the US-MWT02/03/05 scenarios. (2) Add an eviction policy (LRU or idle-timeout) to the
per-principal rate-bucket map so its size is bounded by active principals, not by total
historical principals, while preserving the shipped throttle correctness for active principals
(mirror the existing 100%-mutation `rate_limit` tests).

### Domain Examples
#### 1. Happy path — real A/B fixtures prove isolation
The cross-tenant scenarios run against real Acme and Globex (real members/tokens/issues), and
the boundary holds — no synthetic uuids.
#### 2. Edge case — rate-bucket eviction under many principals
Many one-off principals across many workspaces hit the revoke endpoint once each; the bucket map
evicts idle entries and stays bounded; an active principal's throttle is unchanged.
#### 3. Error/boundary — eviction does not weaken throttling
A principal that is actively storming is NOT evicted mid-burst; the 429 throttle still fires
correctly for active principals.

### UAT Scenarios (BDD)
#### Scenario: Cross-tenant isolation is proven with real coexisting workspaces
Given real workspaces "Acme" and "Globex" each with real members, tokens, and issues
When the cross-tenant isolation scenarios run
Then the boundary holds using these real fixtures
And no scenario relies on a synthetic, non-existent workspace id

#### Scenario: The per-principal rate-bucket map stays bounded under many tenants
Given many distinct principals across many workspaces each make a one-off revoke request
When the rate guardrail processes them
Then the per-principal bucket map size stays bounded
And it does not grow with the total count of historical principals

#### Scenario: Eviction does not weaken throttling for an active principal
Given a single principal sends a burst beyond the rate capacity
When the guardrail processes the burst
Then the active principal is still throttled correctly
And it is not evicted mid-burst

### Acceptance Criteria
- [ ] The cross-workspace isolation acceptance scenarios use REAL two-workspace fixtures, not
  synthetic uuids (closes UI-1).
- [ ] The per-principal rate-bucket map evicts idle/stale principals so its size is bounded by
  active principals, not by total historical principals (closes residual F2).
- [ ] Eviction preserves the shipped throttle correctness for active principals (the 429
  guardrail still fires as before).
- [ ] The `rate_limit` eviction logic has unit/property coverage consistent with the shipped
  100%-mutation discipline.

### Outcome KPIs
- See KPI-MWT-08: synthetic-uuid cross-workspace tests replaced/augmented with real A/B
  fixtures; rate-bucket map size bounded (cap or idle-eviction).

### Technical Notes
- Depends on **US-MWT01** (real coexisting workspaces) and **US-MWT02/03/05** (the scenarios the
  real fixtures back).
- Reuses `crates/foundry-app/src/rate_limit.rs` (the per-principal token bucket, keyed by bound
  `user_id`, 100%-mutation-hardened) — add eviction without changing the throttle semantics.
- Closes the two accepted residuals documented in `docs/evolution/2026-06-07-machine-token-admin-ux.md`
  and `docs/evolution/2026-06-08-token-management-api.md` (UI-1 + F2).
