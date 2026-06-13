# ADR-003 — Migrating the legacy `POST /workspaces` 409 guard

## Status
**IMPLEMENTED / SHIPPED** (ratified RETIRE 2026-06-13; finalized 2026-06-13). DESIGN wave, Propose
mode. Realises the "replace point" recorded in the parent provisioning feature's
`upstream-changes.md` (Finding 2). Option (a) chosen: the legacy `POST /workspaces` 409 route was
DELETED outright (per the 2026-06-13 AGENTS.md "## Dead code" policy); a POST now 404s. The gated
`/admin/instance/workspaces` POST is the sole web provisioning path. See
`docs/evolution/2026-06-13-web-provisioning-flow.md`.

## Context
The parent provisioning feature's upstream Finding 2 established that `bootstrap.rs:301`
`create_workspace` (the handler behind `POST /workspaces`) is STILL PRESENT and hard-returns
`409 CONFLICT` ("Only one workspace per instance") for any second workspace, via `workspace_count()`
— ignoring the requester's identity. The DB index (`uniq_one_workspace`) was dropped by `0009`, but
this application handler was not removed. The parent feature provisioned via the CLI, so it never
touched this handler. This feature builds the web provisioning path, so it must decide what happens
to the legacy route.

Grounding (read the code):
- `bootstrap.rs:301-333` `create_workspace`: matches `workspace_count()` → `Ok(0)` 400 / `Ok(_)`
  409 / `Err` 500. It NEVER creates a workspace; it is purely a guard. Its own comment calls it a
  "boring-monolith taste filter (cheap human-readable 409)".
- It is registered as `POST /workspaces` in `build_router` (`lib.rs`).
- The NEW provisioning path is `POST /admin/instance/workspaces` (ADR-001), gated by
  `is_instance_admin` (ADR-002), calling `Services::provision_workspace` (the real, atomic create).
- There is no template or flow that still posts to `/workspaces` for real creation — the only real
  creation path was always the bootstrap CLAIM (workspace 1) and now the gated admin POST.

## Options considered
- **(a) RETIRE the legacy `POST /workspaces` route (RECOMMENDED).** Remove the route registration
  and the `create_workspace` 409 handler. The new gated `/admin/instance/workspaces` POST is the sole
  web provisioning path. Removes a dead, identity-blind route that can only ever 409 — no behaviour
  is lost (no one can legitimately create a second workspace through it).
- **(b) Leave the 409 handler inert as defence-in-depth.** Keep `POST /workspaces` returning 409.
  Harmless today, but it leaves TWO "create workspace" POST routes (one that always fails, one that
  works), which is confusing for maintainers and a latent footgun (a future caller might wire a form
  to the wrong one). Kept on record as the conservative fallback.
- **(c) REPURPOSE `POST /workspaces` as the gated provisioning route** (remove the 409, add the
  `is_instance_admin` gate + real creation, keep the path). Rejected: `/workspaces` is an
  unauthenticated-looking top-level path that predates the admin surface; the instance-admin surface
  belongs under the `/admin/instance/…` namespace (ADR-001) for a coherent, gated route group. Also,
  `bootstrap.rs` is on the LAYER-1e allow-list for *bootstrap* reasons; putting the gated provisioning
  there muddies the allow-list rationale vs a dedicated `instance_admin.rs` file (ADR-004 D6).

## Decision
**(a) RETIRE the legacy `POST /workspaces` route + the `create_workspace` 409 handler.** The gated
`POST /admin/instance/workspaces` (ADR-001/002) is the sole web provisioning path. The bootstrap
CLAIM remains the sole creator of workspace 1. If the user prefers caution, option (b) (leave it
inert) is the accepted fallback — but the recommendation is to remove the dead route.

## Consequences
- **Positive**: one unambiguous web provisioning path; a dead, identity-blind route is removed;
  no second "create workspace" POST to confuse maintainers or mis-wire a form to.
- **Negative**: a small deletion in `bootstrap.rs` + `build_router` (the `WorkspaceForm` /
  `create_workspace` symbols may become dead and should be removed too). Any test asserting the old
  409 must be retired (DELIVER concern). If a downstream caller somewhere posts to `/workspaces`
  (none found), it would now 404 — acceptable, it only ever 409'd.
- **Security**: removes an identity-blind workspace-creation-shaped route entirely; the only creation
  path is the gated admin POST + the bootstrap claim.

## Relationship
Realises `multi-workspace-provisioning` upstream Finding 2's recorded "EXTEND/replace point" for the
web flow. Does NOT modify the parent feature's docs (per the back-propagation contract); this ADR is
the record of the migration decision.
</content>
