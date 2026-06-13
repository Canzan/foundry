# Walking Skeleton — web-provisioning-flow (DISTILL)

> Quinn (nw-acceptance-designer), DISTILL wave. Legacy per-feature layout. Trunk-based.
> Framework: cucumber-rs. Feature SSOT: `crates/foundry-acceptance/tests/features/us-mwt-web-provisioning.feature`.

## The one walking skeleton

```
@walking_skeleton @wiring_e2e @us-mwt07
Scenario: A super-admin provisions a new isolated workspace from the browser
  Given the super-admin is signed in on the web
  When the super-admin submits the provision form for workspace "Globex" with first admin "priya@globex.com"
  Then the new workspace "Globex" exists and is isolated from all others
  And the web page reports the new workspace and a first-admin invite link
```

This is the single `@walking_skeleton` scenario for the feature. All other scenarios are `@pending`
(DELIVER unskips one per RED→GREEN→COMMIT cycle).

## Why this is the skeleton (litmus test, AD critique Dim 5)

1. **Title = user goal, not technical flow** — "A super-admin provisions a new isolated workspace
   from the browser" describes what Sasha (a non-shell super-admin) achieves, not "request passes
   through session → CSRF → use-case → tx".
2. **Given/When = user actions/context** — "signed in on the web" + "submits the provision form"; no
   "database contains an instance_admins row" framing.
3. **Then = user observations** — "the new workspace exists and is isolated" + "the web page reports
   the new workspace and a first-admin invite link"; observable outcomes, not "an INSERT ran" or "a
   200 returned".
4. **Non-technical stakeholder confirms "yes, that is what users need"** — a non-shell operator
   gaining a browser path to provision is EXACTLY the gap the CLI-first v1 left open (the feature's
   reason to exist, architecture.md §6 Usability).

## What it proves end-to-end (the thinnest cut that wires the new surface)

The skeleton drives the NEW web driving adapter through the production composition root and asserts
it converges on the SHIPPED backend:

- the NEW route `POST /admin/instance/workspaces` exists and is reached over real HTTP;
- the SHIPPED session layer authenticates the signed-in super-admin (the `require_instance_admin`
  gate passes);
- the SHIPPED double-submit CSRF layer admits the valid-token POST;
- the form maps to a `ProvisionRequest{acting_user_id = session.user_id, …}` and calls the SHIPPED
  `Services::provision_workspace`;
- the SHIPPED atomic create+seed tx creates a real isolated tenant (workspace + first-admin user +
  membership + invite) — leaving existing tenants untouched (proven explicitly in scenario 10);
- the NEW htmx success fragment renders the new workspace id + the (informational, D5) invite link.

If this skeleton is green, the surface is wired; the remaining `@pending` scenarios harden the
security boundary (non-enumerability, CSRF), the grant operation, the legacy-route retirement, and
the isolation/untouched-tenant guarantees.

## Strategy (Architecture of Reference — port class → treatment)

Per the Project Infrastructure Policy (`docs/architecture/atdd-infrastructure-policy.md`, row
"Instance-admin web surface"), this is the SHIPPED Rust web idiom — NOT a per-feature A/B/C/D choice:

| Port (this feature) | Class | Treatment | Mechanism |
|---|---|---|---|
| `GET/POST /admin/instance/…` (the 3 new routes) | Driving (HTTP) | real adapter | `spawn_app()` in-process axum router over real HTTP (`reqwest::Client`) |
| real `foundry_session` cookie + `csrf_middleware` | Driven internal | real | shipped session/CSRF layers; the new routes mount UNDER them |
| `Services::provision_workspace` + tx, `is_instance_admin`, `grant_instance_admin`, `user_id_by_email`, `list_workspaces` (NEW thin read) | Driven internal | real | shared testcontainers PG16 + per-scenario schema rotation |
| invite link / `InviteToken` | Driven internal (deterministic) | real | shipped builder; the link is RENDERED, not followed (D5 — dead URL today) |

No driven-external/non-deterministic port is faked for this feature beyond what the shipped policy
already records (clock/email seams are not on the provisioning web path's assertion surface here).

WS tag is `@walking_skeleton @wiring_e2e` (matching slices 02/04/06 — `@real-io @driving_adapter` is
the feature-level tag). No `@in-memory` anywhere (LAYER-3 real-adapter feature throughout).
