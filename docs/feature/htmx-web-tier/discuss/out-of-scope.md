# htmx Web Tier (Feature B) — Out of Scope

> Feature B of the web-tier-extraction split — "Foundry looks like a product." Feature A
> (JSON API + machine-token auth + the presentation-neutral core seam + the boundary guard)
> has SHIPPED and is OUT of this feature entirely. This file states what remains OUT so
> DESIGN/DELIVER do not do hidden work and "does the web tier also do X?" has a clean answer.

The single most important framing: **this is a RENDERING-LAYER change inside one binary — a
move from inline `format!()` HTML to templates + vendored assets. It is NOT a new API, NOT a
service split, NOT a frontend rewrite, and NOT an auth change.**

---

## Hard non-goals (Won't have — defining the shape of the feature)

### NOT a new or changed JSON API
- The JSON API (read + write, machine-token auth) is **Feature A — shipped**. Feature B adds
  NO API endpoints and changes NO API behavior. Web handlers render HTML; they do not emit JSON.
- **Why**: separation of concerns; the API is done and stable.

### NOT a SPA / React / Vue / Svelte rewrite
- The web tier is **server-rendered HTML + htmx fragments + Alpine.js**, exactly as today —
  just rendered from templates instead of `format!` literals. NO client-side SPA framework,
  NO client-side routing, NO JSON-hydrated frontend, NO client state model.
- **Why**: server-rendered htmx is the chosen architecture (README + backend-mvp). An SPA
  would contradict it and require a Node toolchain.

### NOT a Node/JS runtime service or a CDN dependency
- Assets are vendored and served by the binary. NO Node server, NO runtime JS bundler
  service, NO CDN. (A *build-time* asset step is an OPEN DESIGN question — see below; a
  *runtime* service is a hard non-goal.)
- **Why**: "no new runtime services", air-gap friendliness, one-binary ethos.

### NOT a change to the authentication / authorization model
- The browser path is **unchanged**: CSRF (double-submit + `HX-CSRF` header), sessions
  (tower-sessions Postgres), password auth (argon2id), brute-force delay, non-enumerable
  errors, membership/admin/authorship checks. Only the MARKUP of the sign-in/forgot screens
  moves to templates; the handlers, contracts, and secrets are untouched.
- **Why**: the auth path is where bugs cluster; templating must not perturb it.

### NOT new infrastructure
- No Redis, no S3, no message broker, no second database, no new container. Topology stays
  foundry + postgres.

### NOT a boundary-honesty refactor
- Making web/api/core peer consumers of a presentation-neutral core was **Feature A — shipped**
  (the `foundry_services` seam + the CI boundary guard). Feature B REUSES that seam and must
  not regress it (NFR-WEBB-BND-*), but does not re-do it. (This is why jtbd-web-2 is RETIRED —
  see `jobs.yaml`.)

---

## RESOLVED (user 2026-06-02) — formerly the carried web-tier-extraction D4 open question

### Assets are PURE PRE-VENDORED BLOBS — a build-time Node/esbuild step is OUT of scope
- **Decided**: htmx 2.x, Alpine, and CSS are **minified blobs committed (pinned) directly into
  `static/`** and served by the binary. There is **NO JS toolchain at all** — no runtime Node
  service AND no build-time Node/esbuild/minifier step — and **NO CDN**.
- **Consequence for DESIGN**: do NOT introduce a `package.json`, bundler, or any Node/JS build
  dependency. Asset updates are a manual blob swap (pin the version, commit the minified file).
  DESIGN decides only the template engine and how `static/` is organized + cache-busted.
- **Why**: keeps the build dependency-free and air-gap/reproducible-friendly, matching the
  "one binary, one Postgres, no extra toolchain" ethos (NFR-WEBB-INFRA-01/02). Manual asset
  curation is the accepted trade-off.

---

## Deferred to DESIGN (decisions this wave deliberately does NOT make)

### Template engine choice
- **Why deferred**: solution-neutral in DISCUSS. The NFR (render budget ≤200 ms, one-partial
  rule, no DB in render path) is the constraint; the engine (askama, minijinja, maud, tera,
  etc.) is DESIGN's to pick.

### htmx 2 version pin
- **Why deferred**: per web-tier-extraction D3. US-B05 establishes the REQUIREMENT to vendor
  + pin a single htmx 2.x version and regression-test every hx-driven interaction; the exact
  2.x version is DESIGN. DISCUSS records that the active directives are bare `hx-*` (small
  surface) and the `data-*` markers are scraper hooks (NOT htmx directives, NOT migrated).

### CSS strategy
- **Why deferred**: the CSS in scope is "looks intentional, accessible (WCAG 2.2 AA),
  consistent" — the exact approach (hand-written stylesheet, utility classes, etc.) is DESIGN.

### Asset pipeline details (directory layout, minification, cache-busting)
- **Why deferred**: constrained only by "no runtime service, no CDN, image identical across
  deployments"; the layout and cache strategy are DESIGN (and tied to the build-step open
  question above).

---

## Deferred to later feature versions (smaller scope)

### Extracting remaining HTML surfaces to templates
- **Why deferred**: this feature templatizes the THREE highest-value surfaces (board,
  issue+comments, sign-in). The remaining surfaces (`projects::show_create_form`,
  `attachments.rs` HTML, `bootstrap.rs`, `events.rs` HTML, the `dashboard_root` landing) follow
  the same pattern and can be extracted incrementally once it is proven green.
- **Target**: follow-on slices after this feature lands; same approach, lower risk.

### Mobile-responsive polish
- **Why deferred**: inherits backend-mvp's desktop-first scope. The template/CSS pipeline makes
  responsive work easier later, but explicit mobile polish is out of scope here.
- **Target**: v0.4 (per backend-mvp out-of-scope).

### Design system / theming / dark mode
- **Why deferred**: the CSS in scope is "looks intentional, accessible, consistent" — not a
  token-based design system, theming, or dark mode.
- **Target**: post-extraction enhancement.

### Client-side richness beyond current htmx/Alpine behavior
- **Why deferred**: this feature preserves existing interactivity (htmx fragments, Alpine j/k
  nav, `c`-to-create). New client-side features (drag-and-drop reorder, rich-text editor,
  client-side filtering) are separate.
- **Target**: separate features as demand appears.

---

## Re-evaluation Triggers

| Item | Trigger | Likely target |
|------|---------|---------------|
| Remaining surface extraction | board/issue/sign-in pattern proven green | follow-on slices |
| Build-time asset step | DESIGN decides the open question above | DESIGN of this feature |
| Template engine | DESIGN picks within the NFR constraints | DESIGN of this feature |
| htmx 2 version | DESIGN picks the 2.x pin | DESIGN of this feature (US-B05) |
| Mobile polish | ≥3 reports of unusable mobile board | v0.4 |
| Design system / theming | sustained demand for theming/dark mode | post-extraction |
| SPA / client framework | (not foreseen — contradicts the architecture) | none planned |
