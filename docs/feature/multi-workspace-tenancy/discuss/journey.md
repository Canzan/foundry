# Multi-Workspace Tenancy — Journeys

> Two journeys: (A) the **instance operator** consolidating several teams onto one instance and
> trusting the isolation, and (B) the **tenant-isolation enforcement** journey — what a member
> of workspace A experiences (and is denied) when they reach for workspace B. The emotional
> arc is about TRUST: from "I'd never share a box between two clients" to "I can prove A can't
> see B." Surfaces in scope: web htmx tier (`foundry-app`), JSON `/api/v1` (`foundry-api`),
> machine-token auth (`foundry-auth`), sign-in/sessions, backup/restore. Personas + jobs: see
> `jobs.yaml`.

## Shared artifacts (single source of truth for every `${variable}`)

| `${artifact}` | Source of truth | Consumed by |
|---------------|-----------------|-------------|
| `${acting_workspace_id}` | The request→workspace RESOLUTION seam (DM1) — DESIGN owns the mechanism (session claim / URL / token claim). EXACTLY one per request, fail-closed if none. | Every tenant-scoped query (already takes `workspace_id`); `is_workspace_admin`; the non-enumerable lookups. |
| `${acting_user_id}` | The session (web) or the `MachinePrincipal` bearer (API) — already established by the shipped auth. | Authz checks (`is_workspace_admin`, `is_team_member`); audit (`created_by`). |
| `${workspace_a}`, `${workspace_b}` | Real two-workspace fixtures (US-MWT08) — replacing synthetic uuids. | Every isolation acceptance scenario from Slice 2 onward. |
| `${token.workspace_id}` | `machine_tokens.workspace_id` (shipped) — the workspace a bearer token is bound to. | `/api/v1` resolution for machine principals (US-MWT03). |
| `${non_enumerable_refusal}` | Generalized from `attachments.rs` `find_attachment_for_requester` — "missing" and "not yours" collapse to one response. | Every cross-tenant denial path on every surface (US-MWT05). |
| `${first_workspace}` | The single pre-feature workspace row; after migration it is "workspace 1" (US-MWT06). | The forward-only migration; the default resolution for an upgraded single-tenant install. |

---

## Journey A — Operator consolidates several teams onto one instance

### Mental model
Sasha (instance operator) thinks in "I run a box for my clients." Today she believes "one box =
one team" because that is all Foundry allows. The shift this journey delivers: "one box = many
teams, each sealed off from the others." She does NOT think about `workspace_id` columns — she
thinks about clients/teams that must never see each other.

### Happy path

```
  Sasha runs ONE Foundry instance ............ today: capped at ONE team (uniq_one_workspace)
        │
        ▼
  [Slice 5/6] She upgrades; her existing team is now "workspace 1", unchanged ... output: existing data intact, users sign in as before
        │
        ▼
  [Slice 6] She provisions a SECOND workspace for a new client + seeds its first admin ... output: workspace B exists, reachable, isolated
        │
        ▼
  Client A's people sign in → act ONLY on A's data ......... output: A sees A, never B
  Client B's people sign in → act ONLY on B's data ......... output: B sees B, never A
        │
        ▼
  [Slice 6] Dana (security reviewer) is shown real A/B fixtures proving A cannot read,
  mutate, or even enumerate B ............................. output: isolation demonstrated, not promised
```

### Emotional arc (confidence builds toward trust)

| Step | Feeling | Why |
|------|---------|-----|
| Capped at one team today | Resigned / costly | "Every client means another deploy and another DB bill." |
| Upgrade keeps the existing team intact | Relief | "Nothing broke; my one team still works exactly as before." |
| Provision a second workspace | Empowered | "I onboarded a tenant myself, no DB surgery, no redeploy." |
| Two tenants coexist, each sealed | Cautious confidence | "They share a box — is that really safe?" |
| Shown the isolation proof with real fixtures | **Trust** | "A genuinely cannot see B. I can put real clients here." |

### Error / anxiety paths
- **Upgrade fear** → US-MWT06: forward-only, no data rewrite; existing sessions/tokens keep
  working; the migration only drops the guard.
- **"Can a neighbour leak my data?"** → Journey B + US-MWT05: every cross-tenant path refused
  non-enumerably; proven with real fixtures.
- **Ambiguous resolution** (a request maps to zero or >1 workspace) → fail-closed; the request
  is refused, never defaulted to the wrong tenant (US-MWT04).

---

## Journey B — Tenant-isolation enforcement (the security heart)

### Mental model
Marco (member of A) thinks he is "using Foundry." He has no concept of "workspace B" and MUST
NOT gain one. Every action he takes is implicitly scoped to A; any attempt — accidental
(a stale bookmark to a B resource) or hostile (a crafted id) — to reach B is refused as if B's
resource simply does not exist.

### Happy path (isolation is invisible when you stay in your lane)

```
  Marco signs in to workspace A ........................... ${acting_workspace_id} = A
        │
        ▼
  He opens his board / lists issues / uploads an attachment . output: only A's data, every time
        │
        ▼
  A machine token bound to A calls /api/v1 ................ output: acts only on A's data (US-MWT03)
        │
        ▼
  Everything just works — he never sees a workspace concept . output: isolation is silent
```

### Isolation-violation paths (the denials — must all be non-enumerable)

```
  Marco (A) crafts a request for an ISSUE id that belongs to B
        │
        ▼
  Refused IDENTICALLY to a non-existent issue ............. ${non_enumerable_refusal}: no "exists but forbidden" oracle
        ─────────────────────────────────────────────────────
  Marco (A) tries to GET an ATTACHMENT in B (the shipped attachments pattern, now real)
        │
        ▼
  WHERE id = $1 AND workspace_id = A → not found ......... refused; B's attachment invisible
        ─────────────────────────────────────────────────────
  A machine token bound to A calls /api/v1 targeting B's project
        │
        ▼
  Token's workspace_id = A ≠ B → refused non-enumerably .. cross-tenant bearer call denied (US-MWT03)
        ─────────────────────────────────────────────────────
  An ADMIN of A tries to list/manage B's members or tokens
        │
        ▼
  is_workspace_admin(B, admin_of_A) = false → refused .... admin authority is per-tenant (US-MWT02/05)
        ─────────────────────────────────────────────────────
  A revoke-storm from one principal across many tenants
        │
        ▼
  Rate-bucket bounded + evicting → no unbounded growth ... residual F2 closed (US-MWT08)
```

### Emotional arc (for the would-be cross-tenant actor + the tenant being protected)

| Actor | Step | Feeling | Why |
|-------|------|---------|-----|
| Member of A (honest) | Works within A | Unaware / frictionless | Isolation is invisible when you stay in your lane. |
| Crafted request (hostile) | Reaches for B | Stonewalled, learns nothing | Refused as "does not exist"; no signal that B is real. |
| Member of B (protected) | — | Safe | "Nobody outside B can see B, even that it exists." |
| Dana (reviewer) | Reviews the proof | Convinced | Real A/B fixtures, every surface covered, no existence oracle. |

### Error / boundary paths (per surface)
- **Web htmx tier** (US-MWT02): a stale link or crafted id to a B resource → non-enumerable
  refusal; admin actions gated by `is_workspace_admin(${acting_workspace_id}, …)`.
- **JSON `/api/v1`** (US-MWT03): a session/bearer call for a foreign resource → uniform
  not-found; a token bound to A cannot act on B.
- **Sign-in / sessions / resolution** (US-MWT04): resolution yields exactly one workspace;
  multi-membership (OD-2) means selection is explicit; a session with no resolvable workspace is
  refused, not defaulted.
- **Backup/restore** (OD-5, out of v1 scope as per-tenant): whole-instance for v1; per-tenant
  export deferred so a per-workspace restore cannot clobber a sibling.

## Gherkin (happy + key isolation-violation-denied scenarios)

> Embedded per-story in `stories.md` (no standalone `.feature` file in the legacy layout — see
> the prior features). The headline isolation scenarios:

```gherkin
Scenario: Two workspaces coexist and a request sees only its own
  Given workspace "Acme" and workspace "Globex" both exist on one instance
  And Marco is a member of "Acme" with issues in "Acme"
  When Marco lists his issues
  Then he sees only "Acme" issues
  And no "Globex" issue appears

Scenario: A member of one workspace cannot reach another's resource (evil-user)
  Given an issue belongs to "Globex"
  When Marco (a member of "Acme") requests that issue by its id
  Then the request is refused identically to a request for a non-existent issue
  And nothing reveals that the "Globex" issue exists

Scenario: A machine token bound to one workspace cannot act on another (evil-user)
  Given a machine token is bound to "Acme"
  When a caller uses it against a "Globex" project on /api/v1
  Then the call is refused without revealing the "Globex" project exists

Scenario: Upgrading a single-workspace install keeps it working as workspace 1
  Given a pre-feature instance with one workspace and live users, sessions, and tokens
  When the instance upgrades to multi-workspace support
  Then the existing workspace becomes the first workspace with all its data intact
  And its users sign in and work exactly as before
```
