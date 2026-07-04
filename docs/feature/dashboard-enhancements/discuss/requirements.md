# Requirements — dashboard-enhancements

## Context

The signed-in landing (`GET /` → `signin::dashboard_root`) shipped in commit `51ba981` as a first
data-driven slice: it lists the acting workspace's projects (scoped by the SESSION `workspace_id`) plus a
"Quick actions" row (invite / tokens / keyboard help). That slice landed **without unit or acceptance
tests** and with **inline `<style>`** in the template (a deliberate dev-speed shortcut).

This feature hardens and rounds out that surface across five thin slices, all sitting on **already-shipped
seams** — no new architecture, no migration. The centre of gravity is getting the acceptance criteria
right, because they drive Outside-In TDD for the behavioural slices and the retroactive coverage of the
already-shipped base.

## JTBD (anchor job)

> **When** I sign in to Foundry, **I want** to immediately see who I am and which workspace I'm in, reach
> my projects, and get to the admin tools I'm actually allowed to use, **so I can** start real work
> without hunting for URLs or second-guessing my permissions.

- **Functional**: orient (identity + workspace), navigate (projects, actions), act (sign out).
- **Emotional**: confidence ("this is my space, I'm in the right place").
- **Social**: an admin/super-admin sees the extra controls their role grants — the UI reflects standing.

## Personas

| ID | Persona | Cares about |
|----|---------|-------------|
| P1 | Workspace **member** | Sees identity + workspace + projects; no admin controls leak in. |
| P2 | Workspace **admin** | Same, plus member-invite issuance (already linked). |
| P3 | Instance **super-admin** | Additionally sees the instance-provisioning entry point — but ONLY them. |

## Scope (v1)

- **In scope**: (1) personalized greeting (display name + workspace name); (2) a working sign-out control
  (double-submit CSRF); (3) an instance-admin nav link shown **only** to super-admins; (4) promotion of the
  inline dashboard styles into the vendored stylesheet; (5) retroactive test coverage for the base
  dashboard query + render and the new behaviours.
- **Out of scope** (deferred): workspace switcher UI on the dashboard (route `/workspace/switch` exists but
  multi-membership UX is its own feature); project search/filter on the dashboard; per-project stats
  (issue counts); avatar/gravatar; a full design system (slice 4 promotes THIS surface's styles only, it
  does not introduce a token system).

## Brownfield grounding (shipped seams — reuse, do not reinvent)

| Seam | Location | Reuse |
|------|----------|-------|
| `dashboard_root` handler | `crates/foundry-app/src/signin.rs:252` | The surface being extended. Already resolves `SessionUser {user_id, workspace_id}` and loads projects. |
| `SessionUser` | `crates/foundry-app/src/bootstrap.rs:23` | Carries `user_id` + `workspace_id` — the only trusted identity origin (ADR-002 / tenancy rule). |
| `Store::list_projects_for_workspace` | `crates/foundry-store/src/lib.rs` (added `51ba981`) | The already-shipped project query (untested — slice 5 covers it). |
| `users.display_name`, `workspaces.name` | `migrations/0001_init.sql:21`, `:workspaces` | Slice 1 reads these via ONE new tenant-scoped query. No migration. |
| `Store::is_instance_admin(user_id)` | `crates/foundry-store/src/lib.rs:1541` | Slice 3 reuses verbatim — the super-admin predicate for conditional nav. |
| `signin::submit_signout` + `/sign-out` route | `crates/foundry-app/src/signin.rs:190`, `lib.rs:291` | Slice 2's POST target — already live; only a form + CSRF token are missing on the dashboard. |
| `ensure_csrf_cookie` + `build_csrf_cookie` / `generate_token` | `crates/foundry-app/src/signin.rs:305`, `csrf.rs:32,40` | Slice 2 mirrors `admin_tokens::show_index` (`admin_tokens.rs:61`): mint token → pass to view → emit `SET_COOKIE`. |
| Static assets + `base.html` stylesheet link | `static/css/foundry.870985fc.css`, `templates/base.html:5`, ServeDir in `lib.rs:265` | Slice 4 moves inline `<style>` here and bumps the hash. |
| Acceptance harness | `crates/foundry-acceptance/` (`.feature` + step defs + `world.rs`) | Slice 5's dashboard scenario mounts here, mirroring existing web scenarios. |
| CSRF middleware / session layer | `crates/foundry-app/src/{csrf,session}.rs` | The dashboard POST (sign-out) sits under both, unchanged. |

## Constraints

- **Tenancy (ADR-002)**: every query scopes by the SESSION-resolved `workspace_id`/`user_id`, never a
  path/query/body id. Slices 1 and 3 obey this by construction (identity comes from `SessionUser`).
- **US-R04 copy contract**: keep `<h1>Foundry</h1>` present. (No test asserts the welcome sentence, but the
  greeting change in slice 1 REPLACES that sentence — see D1.)
- **US-R07 source-tree guard**: all full-page HTML stays in templates extending `base.html`; no handler
  emits inline `<!doctype>` documents. (Inline `<style>` in a template is permitted by the guard, but
  slice 4 promotes it anyway for cleanliness.)
- **Non-enumerability**: slice 3 must not turn the presence/absence of the admin link into an oracle beyond
  what `is_instance_admin` already governs — the link is additive nav to an already-non-enumerable route.
- **No migration, no new crate.**

## Open decisions (resolved in wave-decisions.md)

- OD-1: greeting wording when `display_name`/`workspace_name` load fails (D1).
- OD-2: does slice 2's CSRF change ripple to the response type of `dashboard_root`? (D2 — yes, `(headers, Html)`.)
- OD-3: hash-bump mechanics for slice 4 with no hashing build step (D3).
