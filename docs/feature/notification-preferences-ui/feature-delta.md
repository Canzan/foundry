# Feature Delta — notification-preferences-ui

> Lean DISTILL → DELIVER pass over a SHIPPED backend. The per-workspace notification
> mute/subscribe backend is already delivered by `recipient-notification-preferences`;
> this feature only makes it reachable + self-service from the signed-in UI (a sidebar
> entry point, a `/account/settings` shell hosting the notifications section, and the
> missing signed-in per-workspace MUTE action). DISCUSS/DESIGN are seed-grounded
> (`discuss/requirements.md`, `design/architecture.md`), summarized below as [REF] rows.

## Wave: DISCUSS

### [REF] Inherited commitments

| Origin | Commitment | DDD | Impact |
|--------|------------|-----|--------|
| n/a | FR-1 — a dedicated signed-in settings surface (`/account/settings`) hosts the notifications section | n/a | New authenticated page; first section is Notifications, reusing the shipped status data path |
| n/a | FR-2 — a sidebar entry point (footer user area) navigates to the settings surface on every authed page | n/a | Discoverability: preferences no longer reachable only via an emailed link |
| n/a | FR-3 — the surface renders the shipped per-workspace Muted/Subscribed list + resubscribe (`workspaces_for_member` + `list_unsubscribed_workspace_ids`) | n/a | Reuse-over-reinvent; no new read path invented |
| n/a | FR-4 — a signed-in per-workspace MUTE action so the surface is a complete subscribe/unsubscribe control (new POST, reuses `Store::insert_unsubscribe`) | n/a | Closes the resubscribe-only gap; the one genuinely new write driving port |
| n/a | FR-5 — backwards-compat: `/account/notifications` + public `/unsubscribe` keep working; existing acceptance coverage stays green | n/a | Regression-guarded; shipped recipient-unsubscribe behavior untouched |
| n/a | NFR-1 — least-privilege / non-enumerable: identity from `SessionUser` only; signed-out → uniform 404; no request-supplied email/workspace steers another recipient | n/a | The mute action mirrors the shipped resubscribe posture; foreign-injection is structurally refused |
| n/a | NFR-2 — every state-changing form is CSRF-checked by the shipped `csrf_middleware` (double-submit `nav.csrf`) | n/a | The mute POST mounts under `csrf_middleware`; a token-less POST is refused |
| n/a | NFR-3 — adding the settings link must not break the exactly-one-active-primary nav invariant (settings lives in the footer, not a third primary) | n/a | Guarded with the SAME oracle the navigation-bar sweep uses |

## Wave: DESIGN

### [REF] Inherited commitments

| Origin | Commitment | DDD | Impact |
|--------|------------|-----|--------|
| DISCUSS#FR-1 | The one new element is a signed-in settings shell `GET /account/settings` (session-gated, uniform 404 signed out; rail via `NavContext::home_for`, Home stays active) | n/a | New handler + template + views struct; assembled through the shipped `NavContext` — no new plumbing to make the link appear everywhere |
| DISCUSS#FR-2 | Sidebar link is a footer `<a class="sidebar__item">` in `sidebar__user`, alongside `Keyboard shortcuts` | n/a | Edit `partials/sidebar.html`; `NavContext` already carries csrf + identity on every authed page |
| DISCUSS#FR-4 | `POST /account/settings/mute`: session identity → `workspaces_for_member` membership check → `Store::insert_unsubscribe` → same result page as resubscribe; idempotent (`ON CONFLICT DO NOTHING`), CSRF-checked, non-enumerable | n/a | Mirrors `resubscribe_notifications` exactly, swapping delete→insert; mounts under `session_layer` + `csrf_middleware` in `build_router` |
| DISCUSS#FR-3 | Reuse the shipped `show_notifications` data path + `NotificationsPage`/`NotificationRow` (`data-status` marker) for the rendered list | n/a | The `data-status="muted|subscribed"` contract is the acceptance oracle; extended with a per-row mute control |
| DISCUSS#FR-5 | Router mounts the new routes on the authenticated layer under the shipped `session_layer` + `csrf_middleware`; existing routes unchanged | n/a | `build_router` gains two routes; nothing shipped is removed |

## Wave: DISTILL

### [REF] Inherited commitments

| Origin | Commitment | DDD | Impact |
|--------|------------|-----|--------|
| DESIGN#FR-1 | `GET /account/settings` is exercised through a real reqwest GET against the production `build_router` composition root | n/a | Tier-A acceptance, `@real-io` in-process axum + testcontainers Postgres; uniform-404 asserted for signed-out |
| DESIGN#FR-4 | `POST /account/settings/mute` is exercised through the real signed-in POST (session + double-submit CSRF), incl. CSRF-less, idempotent, non-member, foreign-injection, signed-out sad paths | n/a | The new write driving port gets happy + 5 example-based sad paths (Mandate 11 — layer-3 sad paths are example-based, not PBT) |
| DESIGN#FR-2 | The sidebar link is asserted by `href="/account/settings"` in `.sidebar__user`, reusing the navigation-bar step glue | n/a | OD-2 label is free; the exactly-one-primary invariant is co-asserted (NFR-3) |
| DISCUSS#FR-5 | The shipped `/account/notifications/resubscribe` is regression-guarded from the surface | n/a | Reuses the shipped route; proves the surface is a complete control without disturbing shipped behavior |

### [REF] Scenario list with tags

`.feature` SSOT: `crates/foundry-acceptance/tests/features/notification-preferences-ui.feature` (12 scenarios; all `@pending`).

| # | Scenario | Tags |
|---|----------|------|
| 1 | A member reaches notification settings from the sidebar and mutes a workspace | `@us-01 @walking_skeleton @driving_port @real-io @pending` |
| 2 | The sidebar footer offers a link to notification settings | `@us-01 @driving_port @real-io @pending` |
| 3 | Reaching settings keeps Home the only current primary navigation item | `@us-01 @property @driving_port @real-io @pending` |
| 4 | The settings surface shows each workspace's mute status | `@us-02 @driving_port @real-io @pending` |
| 5 | A signed-out visitor cannot see the settings surface | `@us-02 @error @security @driving_port @real-io @pending` |
| 6 | A subscribed workspace can be muted from the settings surface | `@us-03 @driving_port @real-io @pending` |
| 7 | A mute without a valid request token is refused | `@us-03 @error @security @real-io @pending` |
| 8 | Muting an already-muted workspace twice is harmless | `@us-03 @error @real-io @pending` |
| 9 | Muting a workspace the member does not belong to is refused non-enumerably | `@us-03 @error @security @real-io @pending` |
| 10 | A crafted foreign workspace cannot steer another recipient's state | `@us-03 @security @real-io @pending` |
| 11 | A signed-out visitor cannot mute a workspace | `@us-03 @error @security @real-io @pending` |
| 12 | A muted workspace can be resubscribed from the settings surface | `@us-04 @driving_port @real-io @pending` |

Error/edge ratio: 6 of 12 scenarios carry `@error`/`@security`-only (5, 7, 8, 9, 10, 11) = **50%** (target ≥ 40%). Exactly one `@walking_skeleton` (scenario 1) drives the full sidebar → surface → mute loop end-to-end over real HTTP.

### [REF] Walking Skeleton strategy

Architecture-of-Reference default: driving port (HTTP) → real adapter via `spawn`; driven-internal (Postgres opt-out state) → real via testcontainers per the Project Infrastructure Policy (`docs/architecture/atdd-infrastructure-policy.md`, "HTTP API (axum routes)" + "PgPool" rows). No driven-external/non-deterministic port is in scope (no email delivery observed), so no fake is added. Single `@walking_skeleton @driving_port @real-io` scenario closes the loop through the production composition root; a non-technical stakeholder confirms "yes — a member finds settings in the sidebar and mutes a workspace."

### [REF] Driving-adapter coverage

| Driving adapter (DESIGN entry point) | Protocol | Scenario(s) |
|--------------------------------------|----------|-------------|
| Sidebar rail `<a>` on an authed page | HTTP GET + link follow | 1 (WS), 2, 3 |
| `GET /account/settings` | HTTP GET | 1, 4, 5 |
| `POST /account/settings/mute` (NEW) | HTTP POST (session + CSRF) | 1, 6, 7, 8, 9, 10, 11 |
| `POST /account/notifications/resubscribe` (shipped, regression) | HTTP POST (session + CSRF) | 12 |

Zero uncovered entry points. Each is invoked via its real protocol against `build_router` (not a direct service call) per RCA-fix P1.

### [REF] Adapter (driven) coverage

| Driven adapter | `@real-io` scenario | Covered by |
|----------------|---------------------|------------|
| `Store` opt-out state (`notification_unsubscribes` via `insert_unsubscribe`/`delete_unsubscribe`/`is_unsubscribed`) | YES | Real testcontainers Postgres, per-scenario schema (all scenarios); state read directly at the store boundary in the refusal/idempotency Thens |
| `Store` membership + status read (`workspaces_for_member`, `list_unsubscribed_workspace_ids`) | YES | Rendered through the real `GET /account/settings` (scenarios 1, 4) |

No NEW driven adapter is introduced by this feature (the writes/reads are shipped `Store` methods), so no new `@real-io @adapter-integration` seam is required.

### [REF] Scaffolds (RED-ready, Mandate 7)

Per THIS project's convention, no NEW production panic-stub is committed: the shipped `build_router`, `unsubscribe.rs`, `sidebar.html`, `views.rs`, and templates are left untouched, and the new endpoints/link are referenced ONLY as HTTP path / CSS-selector string literals. The step module therefore COMPILES against current production (no ImportError-class BROKEN); an unskipped scenario fails at an assertion (404 / absent link) = RED. DELIVER mounts the routes + renders the surface/link + turns each GREEN (Outside-In).

| Artifact | Kind | Status |
|----------|------|--------|
| `crates/foundry-acceptance/tests/features/notification-preferences-ui.feature` | Tier-A Gherkin (12 scenarios, all `@pending`) | created |
| `crates/foundry-acceptance/src/steps/feature_notification_preferences_ui.rs` | Step defs (`__SCAFFOLD__` / `SCAFFOLD: true` markers) | created |
| `crates/foundry-acceptance/src/lib.rs` (`pub mod feature_notification_preferences_ui;`) | Module registration | edited |
| `crates/foundry-acceptance/tests/acceptance.rs` (force-link `use`) | Link registration | edited |
| `crates/foundry-acceptance/src/world.rs` (`npui_*` fields) | Per-scenario state | edited |

DELIVER production targets (not scaffolded here): new `GET /account/settings` handler + `POST /account/settings/mute` handler (mirror `resubscribe_notifications`), a settings-shell template + views struct, the `sidebar__user` `<a>` link, and the two `build_router` route mounts under `session_layer` + `csrf_middleware`.

### [REF] Test placement

`crates/foundry-acceptance/tests/features/<name>.feature` + `crates/foundry-acceptance/src/steps/feature_<name>.rs`, registered in `tests/acceptance.rs` — the established Rust/Cucumber precedent (mirrors `navigation-bar-linear-ui` + `recipient-notification-preferences`). No Python `tests/{path}/acceptance/` layout applies (polyglot: Rust row of the adapter matrix — `<feature>_scenarios.rs`, here the cucumber-rs `.feature` + step module idiom).

### [REF] Layered-test discipline notes

- Tier A only (Mandate 10): the journey is a single 3-step chain reached through the production composition root; input space is not domain-rich (workspace names + membership booleans), so no Tier-B state-machine PBT is warranted.
- Mandate 9: all scenarios run at layer 3+ (real axum + real Postgres) → example-only; no `@given`/PBT machinery.
- Mandate 11: the 6 sad paths (signed-out ×2, CSRF-less, idempotent double-submit, non-member, foreign-injection) are named example-based scenarios, not PBT-generated.
- Mandate 8: universe-bound `assert_state_delta` is the Python pilot contract; this Rust host asserts the equivalent observable universe directly (rendered `data-status` markers + `is_unsubscribed` store reads), never internal struct fields.

### [REF] Pre-requisites

- DESIGN driving ports: the two new routes mount under the shipped `session_layer` + `csrf_middleware` in `build_router`.
- DEVOPS environment: the shipped in-process axum harness + a testcontainers Postgres 16 (per the Project Infrastructure Policy). No new environment matrix.

### Open decisions for DELIVER

- **OD-1** `/account/settings` canonical shell vs. folding in `/account/notifications`. Scenarios pin the OBSERVABLE surface at `/account/settings`; whether the old URL 302s in or embeds is a wiring choice below the assertions.
- **OD-2** Sidebar label ("Settings" vs "Notifications"). The link is asserted by `href`, NOT text — either label passes.
- **OD-3** The mute POST path — pinned to `/account/settings/mute`. If DELIVER lands a different path, `notification-preferences-ui.feature` + `feature_notification_preferences_ui.rs` move together (single constant `MUTE_PATH`).
