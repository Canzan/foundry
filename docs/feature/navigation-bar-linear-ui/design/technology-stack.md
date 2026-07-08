# Technology Stack — navigation-bar-linear-ui

**Reuse-only. No new dependencies.** This feature adds zero crates to `Cargo.lock`, zero runtime
services, and zero build steps. Everything below is already wired in `crates/foundry-app`.

## Components used (all existing)

| Concern | Technology | Version / source | License | Role in this feature |
|---|---|---|---|---|
| Server-side templating | **Askama** + `askama_axum` | `askama = "0.12"`, `askama_axum = "0.4"` (already in `Cargo.toml:41-42`; ADR-B01) | MIT / Apache-2.0 | Compile-time typed templates. Powers the new `app_shell.html` intermediate layout, the `partials/sidebar.html` partial, and multi-level inheritance (base → shell → page). A missing `nav` field or typo'd variable is a **build error**, not a runtime 500 — the enforcement mechanism for the chrome-scope invariant. |
| Progressive enhancement (JS) | HTMX 2.x + Alpine 3.x | Pre-vendored `.min.js` blobs under `static/vendor/` (ADR-B04) | MIT / MIT | Already loaded by `base.html`. The user-menu disclosure (footer popover) can use Alpine (`x-data`/`x-show`) exactly as other interactive surfaces do; **no new JS file** is required. The rail itself is plain HTML/CSS and works with JS disabled. |
| Styling | Single hand-written CSS file | `static/css/foundry.<hash>.css` (ADR-B03, content-hash-in-filename convention) | n/a (project asset) | Sidebar rules (`.app-shell`, `.sidebar`, …) are **appended** to the existing stylesheet; the file is then re-hashed and renamed (ADR-004). No CSS framework, no utility-class tool, no minifier, no build step. |
| Static serving | `tower_http::services::ServeDir` | already enabled (`fs` feature, `Cargo.toml:51`) | MIT | Serves the re-hashed CSS with the existing `immutable` long-cache. Unchanged. |
| Auth / session / CSRF | `tower-sessions`, existing CSRF middleware | already wired | MIT/Apache-2.0 | Source of truth for `display_name`, `workspace_name`, `is_instance_admin`, `csrf`. `NavContext` reads these; **no new auth or CSRF mechanism is introduced** (NFR-4). Sign-out reuses `POST /sign-out` + `_csrf` verbatim. |
| Data access | `sqlx` (existing store) | already wired | MIT/Apache-2.0 | One cheap read reused: "first project for the workspace" to resolve the Board deep-link (ADR-003). Same query family the dashboard already runs for "Your projects"; no schema change. |

## Explicitly NOT added

- **No new crate** — nothing appended to `[dependencies]` or `Cargo.lock`.
- **No CSS framework / Tailwind / build tool** — forbidden by the air-gap / no-toolchain posture
  (DB6, ADR-B03); a hand-written stylesheet meets the "looks like Linear" scope.
- **No new JS bundle or npm** — the vendored-blob discipline (ADR-B04) is preserved; Alpine (already
  present) covers the footer-menu disclosure.
- **No new route/crate for a projects index** — deferred; the Board item deep-links instead (ADR-003).
- **No DB migration.**

## OSS-preference validation

Every technology in the stack is mature, permissively licensed OSS (MIT / Apache-2.0), already
committed to the tree and already exercised by the acceptance suite. No proprietary dependency is
introduced or required. There is nothing to newly evaluate — the feature is a pure reuse of the
Feature-B web-tier stack.
</content>
