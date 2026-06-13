# ADR-004 — Thin driving adapter (reuse vs new) + the LAYER-1e allow-list line

## Status
**IMPLEMENTED / SHIPPED** (ratified 2026-06-13; finalized 2026-06-13). DESIGN wave, Propose mode.
D4 + D6. The thin `instance_admin.rs` adapter over the shipped `provision_workspace` /
`grant_instance_admin`, the one thin non-tenant-scoped `list_workspaces` read, and the
`instance_admin` LAYER-1e allow-list line all shipped to `main` (check-arch PASSED). No new domain
logic, no migration. See `docs/evolution/2026-06-13-web-provisioning-flow.md`.

## Context
The framing of this feature is explicit: it is a **NEW DRIVING ADAPTER (web), not new domain logic**.
The provisioning use-case, the authz gate, the grant, the `instance_admins` table, and the atomic
seed transaction all SHIPPED with the parent feature. This ADR confirms the reuse boundary (no new
domain/store logic) and resolves the one build-time consequence of adding a new handler file: the
LAYER-1e tenant-scoping allow-list.

Grounding (read the code):
- `Services::provision_workspace(ProvisionRequest)` (`foundry-services/src/lib.rs:227-270`) checks
  `is_instance_admin(acting_user_id)` → `Forbidden`, hashes a generated password, mints ids, calls
  `Store::provision_workspace`, returns `Provisioned{workspace_id, invite_id, …}`. The CLI
  (`admin_cli.rs:395-551`) drives it; the web adapter drives it identically with
  `acting_user_id = session.user_id`.
- `grant_instance_admin` + `user_id_by_email` + `is_instance_admin` (`foundry-store/src/lib.rs:1162-1185`)
  are the shipped grant + authz seam.
- LAYER-1e (`check_arch.rs:332-396`): flags a `foundry-app` handler that scopes a tenant query
  (`*_in_workspace(`) by a *request-parsed* workspace id. `is_tenant_scoping_allowlisted` (`:387-396`)
  exempts files whose stem is `signin`/`bootstrap`/`admin_cli`/`session` — the resolution seam +
  provisioning paths that legitimately handle a literal/parsed id. The parent ADR-003 recorded:
  "**If the deferred web flow later lands in a NEW file, that file's stem MUST be added to
  `is_tenant_scoping_allowlisted`**."

## Options considered
### Reuse boundary
- **(a) Thin adapter; reuse the shipped use-case verbatim (RECOMMENDED).** The handler builds
  `ProvisionRequest` from the session + form and calls `Services::provision_workspace`; the grant
  handler resolves email and calls `grant_instance_admin`. NO new domain or store logic. The single
  candidate new read is a `list_workspaces` query for the dashboard (instance-level, non-tenant-
  scoped, no per-tenant data).
- **(b) Add a web-specific provisioning service method.** Rejected: the use-case is already the right
  shape and is mutation-hardened; a parallel method would duplicate the gate and the seed orchestration,
  inviting drift and re-opening the gate-inversion risk the parent already killed.
- **(c) Have the adapter call the store directly** (bypass services). Rejected outright: violates the
  dependency direction (adapter → services → store) and the api≠ad-hoc-authz rule; the authz gate
  lives in services for exactly this reason.

### The `list_workspaces` dashboard read
- **(d) A thin new `list_workspaces() -> Vec<(id, name)>` store query (RECOMMENDED).** Instance-level,
  no `workspace_id` argument, returns all workspaces for the dashboard list. Non-tenant-scoped by
  construction (it enumerates workspaces, it does not scope a query *by* a tenant id).
- **(e) Reuse `workspace_count()` only (count, no list).** Show a count, not a list. Less useful;
  the dashboard wants to show what exists. Acceptable degrade if the user wants zero new store fns,
  but (d) is a one-line query.

### LAYER-1e allow-list
- **(f) Add `instance_admin` to `is_tenant_scoping_allowlisted` (RECOMMENDED).** One line
  (`check_arch.rs:394`). Provisioning names a *literal new* workspace id; the list read is non-tenant-
  scoped — neither is a `*_in_workspace(` call fed a parsed request id, but the file is added pre-
  emptively (and harmlessly) exactly as the parent ADR-003 foresaw, so the guard stays precise and
  DELIVER does not rediscover the requirement.
- **(g) Keep the file off the allow-list and rely on it never making a scoped call.** Rejected: the
  parent ADR-003 explicitly recorded that a new web file owes this line; adding it pre-emptively is
  the documented contract and removes a latent build-break footgun.

## Decision
**(a) Thin adapter, reuse the shipped use-case verbatim; (d) one thin non-tenant-scoped
`list_workspaces` store read for the dashboard; (f) add `instance_admin` to
`is_tenant_scoping_allowlisted`.** No new domain logic, no migration. The adapter only reads the
session, maps form → `ProvisionRequest`, calls the shipped use-case, and maps the result to HTML.

## Consequences
- **Positive**: the entire provisioning correctness + authz + mutation-hardening is inherited; the
  feature is overwhelmingly REUSE (11 reuse/extend · 1 create-new); the allow-list line realises a
  pre-recorded contract; the dependency-direction + api≠ad-hoc-authz guards stay green.
- **Negative**: one thin new store read (`list_workspaces`) + one allow-list line. Both are minimal
  and foreseen.
- **Security**: authz stays in services/store; the adapter never re-implements it; the allow-list line
  keeps the LAYER-1e guard precise (it does not weaken any tenant-scoped path — `instance_admin` has
  no tenant-scoped queries).

## Relationship
Realises `multi-workspace-provisioning` ADR-003's recorded "a future web surface in a new file owes
one allow-list line" (D7 lineage) and confirms the parent's reuse verdict carries into the web flow:
the backend shipped; this is a driving adapter.
</content>
