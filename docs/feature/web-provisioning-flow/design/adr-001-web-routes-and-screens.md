# ADR-001 — Web provisioning routes + screens (v1 surface)

## Status
**IMPLEMENTED / SHIPPED** (ratified 2026-06-13; finalized 2026-06-13). DESIGN wave, Propose mode.
Realises the web flow DEFERRED by `multi-workspace-provisioning` ADR-002 (D2) and originally
sketched by `multi-workspace-tenancy` ADR-004 (option d). Shipped to `main` (`02029e7`→`0c32abd`);
`GET /admin/instance/workspaces` + the two POSTs live behind the gate. See
`docs/evolution/2026-06-13-web-provisioning-flow.md`.

## Context
The shipped feature ships a CLI provisioning surface (`foundry doctor provision-workspace`) and a
CLI grant (`grant-super-admin`). A non-shell super-admin has no browser path to either. This ADR
chooses the v1 web surface: which routes, which screens, how minimal.

Grounding (read the code):
- `/admin/tokens` (`admin_tokens.rs`, registered `lib.rs` `build_router`) is the closest precedent:
  a session-gated, CSRF-protected admin route group with a GET index page + POST actions, htmx
  fragments for action results. `GET show_index` + `POST submit_mint` + `POST submit_revoke`.
- The shipped backend exposes exactly two super-admin operations: **provision a workspace**
  (`Services::provision_workspace`) and **grant super-admin** (`grant_instance_admin`). Listing
  workspaces is a read the dashboard wants but is not yet a shipped query.
- Askama templates extend `base.html`; fragments under `templates/partials/` are returned as bare
  `Html` for htmx swaps (`views.rs`, `templates/`).

## Options considered
- **(a) One dashboard page + two POSTs (RECOMMENDED).** `GET /admin/instance/workspaces` renders a
  full page: a workspace list, a "provision workspace" form (name + first-admin email), and a
  "grant super-admin" form (email). `POST /admin/instance/workspaces` provisions; `POST
  /admin/instance/super-admins` grants. POST results return htmx fragments (success card with the
  new workspace id + invite link; grant confirmation). Mirrors `/admin/tokens` one-for-one. Smallest
  coherent surface that covers both shipped operations.
- **(b) Separate pages per operation** (`/admin/instance/workspaces/new`, `/admin/instance/grant`).
  More REST-shaped, but more routes + templates + navigation for a once-in-a-while operator action.
  Over-built for v1; the single-dashboard idiom (`/admin/tokens`) already reads well.
- **(c) List-only v1, forms deferred.** A read-only `/admin/instance/workspaces` list, with
  provisioning staying CLI-only. Rejected: it delivers no new *capability* (the CLI already lists
  via DB), defeating the feature's purpose (let a non-shell super-admin provision).
- **(d) Bundle a per-workspace detail/management screen.** Edit/suspend/delete a workspace from the
  browser. Rejected for v1: no shipped use-cases for suspend/delete; scope creep well beyond
  "realise the deferred provisioning surface."

## Decision
**(a) One dashboard page + two POSTs.** Routes:
- `GET /admin/instance/workspaces` — full page (extends `base.html`): workspace list + provision
  form + grant form. Behind the `require_instance_admin` gate (ADR-002).
- `POST /admin/instance/workspaces` — provision (name + admin_email); htmx success fragment with the
  new workspace id + the signed invite link.
- `POST /admin/instance/super-admins` — grant (email); htmx confirmation fragment.

Full page for GET (no-JS fallback); htmx fragments for POST feedback — matching the shipped web
tier's progressive-enhancement contract. The workspace list is rendered from a thin, non-tenant-
scoped read (a small new `list_workspaces` store query, the only candidate new read — D4/ADR-004).

## Consequences
- **Positive**: smallest coherent surface; one new handler file + 2-3 templates; mirrors a proven
  shipped idiom (`/admin/tokens`) so it inherits its review/test discipline; both shipped operations
  reachable from the browser.
- **Negative**: a single dense dashboard rather than dedicated pages (acceptable for an infrequent
  operator action). A thin new `list_workspaces` read is introduced (non-tenant-scoped, instance-
  level; covered by the D6 allow-list line).
- **Security**: every route behind the ADR-002 gate; POSTs CSRF-protected by the shipped layer.

## Relationship
Realises `multi-workspace-provisioning` ADR-002's deferred web flow and
`multi-workspace-tenancy` ADR-004's `/admin/instance/workspaces` sketch (option d), now grounded in
the shipped `/admin/tokens` idiom.
</content>
