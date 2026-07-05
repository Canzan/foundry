# Evolution — dashboard-enhancements (rounding out the signed-in landing)

**Finalized**: 2026-07-04
**DELIVER commits**: `0db6ff7` (slice 01) → `8843f86` (slice 02) → `5883fac` (slice 03) → `386a671` (slice 04) — four elephant-carpaccio slices committed directly to `main` (trunk-based, no PRs). Builds on the base dashboard `51ba981`. Prior waves: DISCUSS `225bfd8`, DISTILL `7b78879`. Advisory-clearing prerequisite `5a5ca06` (see below).
**Wave coverage**: DISCUSS (DoR passed, 5 stories → 4 slices) → DISTILL (9 acceptance scenarios authored as the `dashboard-enhancements.feature` SSOT) → DELIVER (4 slices, Outside-In TDD). Ran in the repo's **legacy multi-file convention** (`docs/feature/dashboard-enhancements/`), NOT the nWave-3.21 SSOT/feature-delta model (per user direction — no repo migration). DES step-monitoring intentionally exempt (lean mode). The feature directory is PRESERVED.
**Scope**: the signed-in landing (`GET /` → `signin::dashboard_root`) was a bare "You are signed in" stub after `51ba981` gave it a project list. This feature makes it a real home base — personalized greeting, role-appropriate navigation, sign-out — and backfills the coverage the base shipped without.

## Milestone — the dashboard is a home base, not a stub

A signed-in user now lands somewhere that orients them (who they are + which workspace), lets them reach their projects and only the admin tools their role grants, and lets them sign out — the "Linear-feel home" the web tier was building toward. The instance super-admin sees an entry point to tenant provisioning that a member never does; everyone can end their session safely.

## What shipped

Four thin slices on the shipped `dashboard_root` handler + `DashboardRoot` view + `dashboard_root.html`, all reuse (no migration, no new crate):

- **Slice 01 — greeting (US-01)** — `GET /` greets "Welcome back, {display_name}" + "Workspace: {name}", loaded via the NEW `Store::dashboard_greeting(user_id, workspace_id)` (one tenant-scoped `query_as` read joining `users` × `workspaces`), scoped by the SESSION `SessionUser` (never a path/query id — ADR-002). Degrades to a neutral greeting at **status 200** on `None`/`Err` (D1), never a 500. `<h1>Foundry</h1>` preserved (US-R04). Askama auto-escaping renders a markup-bearing display name inert.
- **Slice 02 — instance-admin link, super-admin only (US-03)** — when `Store::is_instance_admin(session user_id)` (shipped seam, reused verbatim) is true, Quick actions shows an "Instance admin" link to `/admin/instance/workspaces`; for everyone else the link is **absent from the rendered HTML** (not CSS-hidden). Fail-closed on the query error path (link absent).
- **Slice 03 — sign out (US-02)** — a CSRF-protected `<form method="post" action="/sign-out">` (the shipped `submit_signout` + `/sign-out` route). Per **D2** the handler now mints a double-submit token via the shipped `ensure_csrf_cookie` and returns `(Set-Cookie, Html)` (mirroring `admin_tokens::show_index`); submitting destroys the session and 303s to `/sign-in`. A forged `_csrf` is refused by the shipped `csrf_middleware`, session intact.
- **Slice 04 — coverage backfill + style promotion (US-05 + US-04)** — the base dashboard shipped in `51ba981` was UNTESTED; this backfills a store test for `list_projects_for_workspace` (ordered-by-name, tenant-isolation, empty) and an acceptance scenario driving the project-card → board link. The inline `<style>` block is promoted into the vendored stylesheet (`foundry.870985fc.css` → `foundry.386eb83b.css`, a real content-hash rename with `base.html` bumped — D3), fixing the previously-unstyled "Workspace:" line. Behaviour-preserving; the US-04 scenario asserts no inline `<style>` + the hashed stylesheet serves the `.dash` rules.

## Security / correctness — the crux

- **Tenancy (ADR-002)**: the greeting and the project list scope by the SESSION `user_id`/`workspace_id` — never request input — so no new `check-arch` LAYER-1e allow-list line (confirmed: `xtask check-arch` PASSED). The store isolation test proves a foreign workspace's projects never leak.
- **Role-correct nav**: the instance-admin link is a data-derived render off the shipped `is_instance_admin` predicate; a non-super-admin's HTML never contains the link (asserted on body absence, not CSS), and the target route stays non-enumerable on its own gate. Fail-closed on error.
- **CSRF on sign-out**: double-submit token via the shipped middleware; a forged POST does not sign the user out.
- **XSS-inert output**: Askama auto-escaping renders a `<b>`-bearing display name as escaped text.
- **Graceful degradation (D1)**: a failed identity read renders a neutral greeting at 200 (pinned by `signin::tests::greeting_degrades_to_neutral_when_identity_absent`), never a 500.

## Decisions realized (D1–D7)

| # | Decision | Status |
|---|---|---|
| **D1** | Greeting degrades to a neutral 200 on `None`/`Err`, not a 500. | **IMPLEMENTED** (unit-pinned) |
| **D2** | Sign-out CSRF ripples `dashboard_root` to `(Set-Cookie, Html)` via the shipped `ensure_csrf_cookie`; sequenced last of the additive slices to isolate the change. Ripple contained — `dashboard_root` is the only view caller. | **IMPLEMENTED** |
| **D3** | Manual CSS hash bump (`870985fc` → `386eb83b`), old file removed, `base.html` updated. | **IMPLEMENTED** |
| **D4** | Keep `<h1>Foundry</h1>`; the US-R04 welcome sentence is replaced by the personalized greeting. | **IMPLEMENTED** |
| **D5** | Session-scoped tenancy only; no new `check-arch` line. | **IMPLEMENTED** (no line needed) |
| **D6** | Retroactive coverage of the untested base (`51ba981`) is a first-class slice. | **IMPLEMENTED** |
| **D7** | Follow the repo's legacy multi-file nWave convention; do NOT adopt the 3.21 SSOT model or migrate. | **IMPLEMENTED** |

## How it was built (DELIVER) — the 4-slice TDD arc

Each slice: un-`@pending` its scenarios (RED for the right business reason) → GREEN via the reuse seam → `fmt` + full-workspace release `clippy` → COMMIT. Feature-scoped acceptance grew 2 → 4 → 6 → 8 green scenarios (`FOUNDRY_ACCEPTANCE_TAGS=dashboard-enhancements`).

| Slice | Commit | Proved |
|---|---|---|
| 01 greeting | `0db6ff7` | greets by name + workspace; markup inert; degrades to 200 (unit) |
| 02 instance-admin link | `8843f86` | super-admin sees the link; a member never does |
| 03 sign-out | `5883fac` | CSRF sign-out → `/sign-in`; forged `_csrf` refused, session intact |
| 04 coverage + styles | `386a671` | project-list store test (isolation/order/empty); styles from the stylesheet, no inline `<style>` |

One acceptance scenario (greeting-degrades-on-error) is deliberately `@pending` — no in-process fault-injection seam exists, so D1 is pinned by a unit test instead (the R1 decision from DISTILL).

## Verification

- **`cargo xtask ci` — ALL GATES GREEN**: fmt · clippy · `check-arch` PASSED · release build · workspace unit tests (incl. the two `signin::tests` greeting tests) · `cargo deny` (advisories ok) · **full `@all` acceptance lane: 54 features / 375 scenarios / 2934 steps, all passed**.
- **Feature-scoped acceptance**: 8/8 dashboard-enhancements scenarios, 54 steps, green.
- **Store**: `dashboard_greeting` + `list_projects_for_workspace` tests green (real testcontainers Postgres).

## Prerequisite fix (separate concern)

`5a5ca06` — `cargo deny` surfaced two pre-existing RUSTSEC advisories unrelated to this feature that blocked the finalize gate (and any push): anyhow 1.0.102 soundness → 1.0.103, and **RUSTSEC-2026-0193 mXSS in ammonia** (foundry-core's markdown sanitizer) → 4.1.3. Lockfile-only, in-range bumps; the markdown-sanitizer tests continue to pass.

## Deferred (out of scope, v1)

Workspace-switcher UI on the dashboard (the `/workspace/switch` route exists; multi-membership UX is its own feature); project search/filter and per-project issue counts on the dashboard; avatars; a design-token system (slice 04 promoted THIS surface's styles only). The greeting fault-path acceptance scenario awaits an in-process fault-injection seam.
