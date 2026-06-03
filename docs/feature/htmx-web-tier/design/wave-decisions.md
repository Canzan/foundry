# htmx Web Tier (Feature B) — DESIGN Wave Decisions

Owner: solution-architect (Morgan). Scope: **Feature B only** per DISCUSS DB1-DB8 — replace inline
`format!()` HTML with a template engine + a pure-vendored static-asset pipeline (htmx 2.x / Alpine /
CSS), and perform the deferred htmx 1→2 normalization/upgrade (US-B01..B06). Interaction mode:
**Propose**. This file is the central wave record: decisions table, ADR-B01..B04 (MADR-style, kept
here rather than as separate files — mirroring Feature A and backend-mvp), the Reuse Analysis tally,
the technology stack with rationale, constraints honored, and the **Open decisions awaiting user
ratification** list.

Output uses the LEGACY per-feature layout under `docs/feature/htmx-web-tier/design/` (NOT
`docs/product/` SSOT), per DISCUSS DB8. Companion documents: `architecture.md`, `template-engine.md`,
`render-contract.md`, `assets.md`, `htmx2-migration.md`, `error-and-observability.md`.

## Architecture summary

The web tier becomes a **template-rendering driving adapter** inside the one `foundry` binary: it
keeps calling the shipped `foundry-services` seam for data, keeps browser auth/CSRF/sessions
byte-for-byte unchanged, and **moves markup out of handler `format!` literals into compiled-in
`.html` templates** rendered by **Askama 0.12** (the workspace-blessed-but-unwired engine). Static
assets (htmx 2.x / Alpine / hand-written CSS) are **pure pre-vendored, content-hashed, minified
blobs** committed into `static/` and served by **`tower_http::ServeDir`** (the `fs` feature is
already vendored — zero new serving dependency); no Node, no bundler, no CDN, at runtime or build
time. The riskiest DISCUSS assumption — "templating breaks the substring-asserting suite" — is
de-risked by reading the suite: it asserts on the **DOM via `scraper` + literal-copy `contains`**,
not byte whitespace, so the render contract is **selector-and-substring-identical**, not
byte-identical. The htmx 1→2 bump is a dedicated, atomic, regression-gated final slice (DB4); the
directive surface is small and core-only (no `hx-on`, no extensions), so the migration is mostly
verification. Net new runtime dependency: **ONE** — the template engine (`askama` + `askama_axum`),
already declared in `[workspace.dependencies]`.

## Decisions table (DDD-numbered, this wave)

| # | Decision | Rationale |
|---|---|---|
| DD1 | **Template engine = Askama 0.12** (ADR-B01), compiled-in, typed, Jinja-like `.html` files. | Moves markup into template FILES (the feature's primary job htmx-web-1 — which Maud fails); strongest compile-time safety (missing template = build error); no runtime template I/O (best ≤200 ms + air-gap fit); already the workspace-blessed intent (`Cargo.toml:38`, absent from lock = unwired). Minijinja's runtime-load/weaker-safety dominated. |
| DD2 | **Render contract = selector-and-substring-identical, NOT byte-identical** (ADR-B02). Move-only; defer visual rework. | The acceptance suite reads the DOM via `scraper` (CSS+text) + `body.contains` copy checks (`html_assertions.rs`), not whitespace. Reproduce asserted elements/`data-*` markers/`hx-*` targets/literal copy; incidental whitespace is free. No existing scenario is edited during the move; the suite stays the net (NFR-WEBB-COMPAT-01/02). |
| DD3 | **Static serving = `tower_http::ServeDir` at `/static`, zero new dep** (ADR-B03). | `tower-http` already has the `fs` feature in the workspace (`Cargo.toml:35`). Path-traversal-safe by construction (US-B06 scenario 2). Mounted like the existing `attachment_routes` sub-router. |
| DD4 | **Assets = pure pre-vendored, minified, content-hashed blobs in `static/vendor` + `static/css`** (ADR-B03), with provenance+sha256 in `VENDOR.md`. | DB6: no Node/bundler/minifier/CDN. We vendor the upstream pre-minified `.min.js` artifacts directly; CSS is hand-written. Asset update = manual pinned-blob swap. |
| DD5 | **Cache-busting = content-hash/version in the committed filename + `immutable` long-cache** (ADR-B03). | Correct + aggressive caching with zero build step; the one failure mode (forgotten rename) is caught by the asset-resolution probe. Query-string (proxy-ignored) and ETag-only (no immutable) options dominated. |
| DD6 | **htmx 1→2 = direct normalize-and-bump as ONE atomic, regression-gated slice (Slice 4)** (ADR-B04). | DB4. The directive surface is small and core-only (`hx-get/patch/delete/target/swap/swap-oob`; no `hx-on`, no extensions) — htmx 2 preserves all of them, so it is mostly verification. A staged compat path would re-create the mixed-version window DB4 avoids (Principle 8). |
| DD7 | **htmx 2.x pin = latest stable 2.0.x at implementation time; Alpine = latest stable 3.14.x** (ADR-B04), one pinned blob each, recorded in `VENDOR.md`. | Stable major series + latest patch fixes; exactly one htmx file with a recorded version (US-B05 AC). Exact patch is a pin-at-build detail, not an architecture decision. |
| DD8 | **CSS = a single hand-written `foundry.css`** (ADR-B03). | Scope is "intentional, accessible, consistent" — not a design system/theming (out-of-scope.md). A utility framework needs a build step (forbidden) or ships a huge unused blob; hand CSS keeps semantic classes (which the suite selects on) and a tiny payload. |
| DD9 | **No new crate (`foundry-web` extraction DEFERRED); Feature B touches only `foundry-app` internals + `templates/`/`static/`** (ADR-B01 consequence). | The templating is the value; the crate split is orthogonal and multiplies blast radius (relocating `build_router`/`spawn_app` touches ~4 acceptance `AppState` sites). web≠DB is already guard-enforceable without it (Principle 8). Flagged as an open question for the user. |
| DD10 | **One partial per repeated component (`issue_card.html`, `comment_card.html`); OOB wrappers `{% include %}` the SAME partial** (render-contract.md). | NFR-WEBB-MAINT-02 + fixes today's `render_comment_card_oob` affordance divergence (`comments.rs:841`) — the live card now matches the reloaded card by construction. |
| DD11 | **Affordances/authz/sanitization stay in handler/core; templates render flags + pre-sanitized `body_html` (Askama `|safe`)** (architecture.md). | NFR-WEBB-BND-01/03: zero DB/authz/sanitization in templates. `render_comment_markdown` stays in `foundry-core`; the template embeds the result verbatim, as `format!` does today (`comments.rs:820`). |
| DD12 | **CSRF: templates emit only the hidden `_csrf` field; middleware/cookie/header UNCHANGED** (render-contract.md §CSRF). | DB7 / NFR-WEBB-COMPAT-03: `csrf.rs`, the `foundry_csrf` cookie, the `hx-csrf` header, constant-time compare, `/bootstrap` exemption are invariants. Only markup moves. |
| DD13 | **No new external integration → no new contract-test surface; SMTP unchanged** (architecture.md). | Assets are local; no CDN/third-party/OAuth/webhook. The inherited SMTP recommendation stands. |
| DD14 | **No new metric required; existing per-request duration histogram covers the render budget** (error-and-observability.md). | `metrics_server::request_tracking_layer()` (`lib.rs:277`) already measures boundary latency (NFR-WEBB-PERF-01). A per-template histogram is optional, non-blocking, and unjustified surface for Feature B (Principle 8). |

## ADR-B01 — Template engine (Askama 0.12)

- **Status**: Proposed (awaiting ratification — Open Decision #1).
- **Context**: Move markup out of handler `format!` into template files (htmx-web-1 /
  NFR-WEBB-MAINT-01) without missing the ≤200 ms budget (NFR-WEBB-PERF-01), without a JS toolchain
  (DB6), keeping the `scraper`/`contains` acceptance suite green (NFR-WEBB-COMPAT-01/02), and
  supporting one-definition partials for htmx OOB swaps (NFR-WEBB-MAINT-02).
- **Decision**: **Askama 0.12** (+ `askama_axum`). Compiled-in, typed, Jinja-like `.html` templates;
  missing template / bad field = compile error; no runtime template I/O. Honor the existing workspace
  pin (`Cargo.toml:38`).
- **Alternatives considered**: *Maud 0.26* — markup stays in Rust, which **fails the feature's
  primary job** (template FILES); rejected. *Minijinja 2* — runtime interpreter, weaker (runtime)
  safety, pulls toward runtime template loading that fights the air-gap/one-binary posture; strictly
  dominated; rejected. Full trade-off matrix in `template-engine.md`.
- **Consequences**: + compile-time safety (best Earned-Trust posture), ≤200 ms-friendly, air-gap
  clean, lowest-process-cost (already workspace-blessed). − template changes require recompilation
  (no default hot-reload); minor compile-time cost (~10 templates, negligible). Adds `askama` +
  `askama_axum` to `Cargo.lock` (MIT/Apache-2.0; re-run `cargo deny check`).

## ADR-B02 — Render contract (selector-and-substring-identical, move-only)

- **Status**: Proposed (awaiting ratification — Open Decision #2).
- **Context**: The DISCUSS risk register frames substring-asserting tests as a HIGH whitespace risk.
  Reading the suite shows it parses the DOM (`scraper` CSS + trimmed text) and does literal-copy
  `contains` — not byte comparison.
- **Decision**: Reproduce the **asserted contract** (CSS-selectable elements/attributes, `data-*`
  markers, `hx-*`/`hx-swap-oob` targets, literal copy) exactly; let incidental whitespace and
  in-tag attribute order vary. **Move only**; do not edit existing scenarios during Slices 1-3; defer
  visual/markup improvement to CSS + post-feature slices. The one in-scope behavior change (OOB card
  gains Edit/Delete) gets a NEW scenario.
- **Alternatives considered**: *Byte-identical reproduction* — over-constrains (freezes whitespace
  the suite never checks), brittle, no payoff. *Intentionally-improved markup + test edits during the
  move* — destroys the regression net at the moment it is most needed; rejected (sequence improvement
  AFTER the move).
- **Consequences**: + lowest-risk; a green suite IS the equivalence proof; appearance still improves
  via CSS. − contributors must know "the suite reads the DOM, keep the markers/copy" (documented in
  `render-contract.md`).

## ADR-B03 — Static-asset pipeline (ServeDir + pure vendored content-hashed blobs)

- **Status**: Proposed (awaiting ratification — Open Decision #4 is the cache-busting sub-part).
- **Context**: Serve htmx/Alpine/CSS from the binary, no Node/bundler/minifier/CDN (DB6), cacheable,
  air-gap-clean, path-traversal-safe, image identical across deployments (NFR-WEBB-INFRA-01/02,
  PERF-03).
- **Decision**: `tower_http::ServeDir` at `/static` (the `fs` feature is already vendored — zero new
  dep). Assets are committed, pre-minified (upstream artifacts), pinned blobs under
  `static/vendor` + `static/css`, with provenance+sha256 in `VENDOR.md`. **Cache-busting =
  content-hash/version in the committed filename + `Cache-Control: immutable` long-cache.** CSS is a
  single hand-written stylesheet (a11y: contrast, focus, target size, labels). An asset-resolution CI
  probe asserts referenced `static/` paths exist (US-B02 scenario 3).
- **Alternatives considered (cache-busting)**: *version query string* — proxy/cache-ignored,
  forget-to-bump risk without the self-correcting 404; weaker. *ETag/Last-Modified only* — no safe
  `immutable` caching; revalidation round-trips; fallback only. (serving) *a build-time
  minify/bundle step* — forbidden by DB6.
- **Consequences**: + correct aggressive caching with no build step; air-gap auditable
  (`VENDOR.md`); zero new serving dep. − manual blob curation (accepted DB6 trade-off); a forgotten
  filename rename is caught by the probe; `static/` must be `COPY`'d into the image (the one
  deployment delta).

## ADR-B04 — htmx 1→2 migration (direct atomic bump, latest stable 2.0.x)

- **Status**: Proposed (awaiting ratification — Open Decision #3 + the version-series sub-part).
- **Context**: The deferred htmx-2 migration (web-tier-extraction D3) needs a home; DB4 fixes it as a
  dedicated final slice. The active directive surface is small and core-only.
- **Decision**: Slice 4 = direct **normalize-and-bump** in one atomic, regression-gated change:
  confirm/centralize the bare `hx-*` directives in partials, swap in ONE pinned **latest-stable
  htmx 2.0.x** blob (Alpine = latest stable 3.14.x), green regression scenario per hx-driven
  interaction, `data-*` markers byte-stable. Slices 1-3 move directives AS-IS (no version bump).
- **Alternatives considered**: *staged compat path (dual-vendor htmx 1+2 behind a flag)* —
  re-creates the mixed-version window DB4 avoids, doubles the blob, no payoff for a core-only
  directive set; rejected (Principle 8).
- **Consequences**: + one atomic, fully regression-tested bump; version pinned once after directives
  are consistent. − bump + normalization land together (low risk given htmx 2 preserves the
  directives Foundry uses; see `htmx2-migration.md` breaking-change table).

## Reuse Analysis tally (full table in `architecture.md`)

- **EXTEND = 9**: board/issue/comment data fetch (foundry-services), markdown sanitization (core),
  affordance gating (handler), keyboard carrier (moved into template), CSRF contract
  (untouched; field emitted from template), session/sign-in security (untouched), router composition
  (+`/static`), static-file serving (`tower-http fs` already vendored).
- **CREATE NEW = 7**, each with no existing alternative: (1) the **templates** (by extraction of the
  `format!` sites — `templates/` is empty); (2) the **template engine wiring** (Askama — declared but
  unwired); (3) the **`foundry-app::views` module** (typed view-models); (4) the **vendored
  htmx/Alpine/CSS blobs** (`static/` empty); (5) the **asset-resolution/template-presence check**;
  (6) the **htmx 1→2 normalization + pin** (Slice 4); (7) the **content-hash cache-busting
  convention**.
- The entire data/auth/CSRF/sanitization/authz core is **100% reused** — Feature B moves *markup*,
  not behavior, the structural basis for the render contract.

## Technology stack (versions pinned; OSS-first; from `Cargo.lock`/workspace where possible)

| Concern | Choice | Version | License | New dep? |
|---|---|---|---|---|
| HTTP / routing | `axum` | 0.8 | MIT | no (existing) |
| **Template engine** | **`askama` + `askama_axum`** | **0.12 (workspace pin; matching `askama_axum`)** | **MIT/Apache-2.0** | **YES — wired now (declared in `Cargo.toml:38`, absent from lock)** |
| Static file serving | `tower-http` (`fs` feature) | 0.6 | MIT | no — feature ALREADY enabled (`Cargo.toml:35`) |
| Compression | `tower-http` (`compression-gzip`) | 0.6 | MIT | no (existing) |
| Vendored htmx | htmx (pinned blob, served, not a crate) | latest stable 2.0.x | BSD-2-Clause / 0BSD | no (vendored asset, not a dep) |
| Vendored Alpine.js | Alpine (pinned blob, served) | latest stable 3.14.x | MIT | no (vendored asset) |
| CSS | hand-written `foundry.css` | — | AGPL-3.0 (repo) | no |
| Sanitization (reused) | `ammonia` + `pulldown-cmark` (via foundry-core) | 4 / 0.12 | MIT / MIT | no (existing) |
| Observability | existing `metrics` request layer | — | MIT/Apache-2.0 | no (existing) |
| Asset/probe tooling | `xtask` (custom) | — | AGPL-3.0 (repo) | no new runtime dep |

**Net new runtime dependencies introduced by Feature B: ONE — the template engine (`askama` +
`askama_axum`), already declared in `[workspace.dependencies]`.** `tower-http`'s `fs` feature is
already on, so static serving adds nothing. The vendored htmx/Alpine are *assets*, not crates. Re-run
`cargo deny check` after wiring Askama (MIT/Apache-2.0, in the allowed set — no `deny.toml` change
anticipated; verify on the lock update).

## Constraints honored (NFR traceability)

- One binary, one Postgres, no Redis, no Node, no bundler, no CDN, no new runtime service
  (NFR-WEBB-BND-04, NFR-WEBB-INFRA-01/02, DB6) — templates compile into the binary; assets are
  committed blobs served by `ServeDir`; `docker compose` topology unchanged.
- Web/template tier gains NO DB access; data reaches templates via the existing `foundry-services`
  seam (NFR-WEBB-BND-01).
- Sanitization (`foundry-core`) and authz (handler) stay out of templates (NFR-WEBB-BND-03).
- Browser auth/CSRF/sessions/non-enumerable error UNCHANGED — only markup moves (DB7,
  NFR-WEBB-COMPAT-03/04/05).
- Existing acceptance suite stays green; render contract is selector-and-substring-identical
  (NFR-WEBB-COMPAT-01/02).
- ≤200 ms P95 render budget held (compiled-in templates, no DB in render path), proven on the board
  first (DB3) via criterion + the backend-mvp NFR-PERF-01 load harness (NFR-WEBB-PERF-01).
- Keyboard carrier + WCAG 2.2 AA preserved (NFR-WEBB-A11Y-01/02).
- Markup in templates, ONE base layout, ONE partial per repeated component (NFR-WEBB-MAINT-01/02).
- htmx vendored + pinned at one 2.x version; `data-*` markers untouched by normalization (US-B05,
  NFR-WEBB-INFRA-01).
- Default architecture preserved: modular monolith with dependency inversion (ports-and-adapters);
  the web tier is a driving adapter rendering over `foundry-services`. No crate split (Principle 8).

## Priority validation (reviewer Dimension 5)

- **Q1 largest bottleneck?** The confirmed PRIMARY jobs (DB2) are htmx-web-1 (restyle without Rust)
  and htmx-web-2 (styled first screen). The design leads with the engine (markup → template files)
  and the vendored-asset pipeline (styled offline) on the highest-traffic surface first (the board,
  DB3 walking skeleton). **YES.**
- **Q2 simpler alternatives considered?** Yes — ADR-B01 weighs Maud + Minijinja and rejects the
  crate-split (DD9, Principle 8); ADR-B02 weighs byte-identical + test-editing and rejects both;
  ADR-B03 weighs query-string + ETag-only cache-busting; ADR-B04 weighs a staged htmx path. Multiple
  simpler-or-different options documented and rejected with rationale.
- **Q3 constraint prioritization?** The riskiest assumption (templating breaks the suite) is examined
  FIRST and found over-stated (the suite reads the DOM, not bytes), reframing the contract to the
  cheaper selector-and-substring form. The ≤200 ms risk is gated on Slice 1's board (hottest
  surface). Not inverted.
- **Q4 data-justified?** The engine, render-contract, and reuse claims are grounded in specific
  file:line evidence (`Cargo.toml:38`/lock absence; `html_assertions.rs` selector/text API; the
  `render_*` `format!` sites; `tower-http` `fs` feature), not assumption.

## DISCUSS assumptions challenged

One material refinement, recorded in `upstream-changes.md`: the DISCUSS risk register frames the
acceptance-compatibility risk as "scenarios assert on HTML **substrings**; templating changes
**whitespace/markup**" (HIGH probability). Reading the suite shows the structural assertions go
through **`scraper` (a DOM parser, CSS selectors + trimmed text)**, with only error *copy* and
`data-*` *markers* checked by `body.contains`. So whitespace/indentation/attribute-order changes are
**already safe**; the contract is selector-and-substring-identical, not byte-identical. This LOWERS
the risk (does not contradict the surfaces or the regression net) and shapes ADR-B02. No story or
scope change. Two secondary refinements also recorded: backend-mvp/Feature-A named "askama" but it
was never wired (lock-absent), and `tower-http`'s `fs` feature is already enabled — both reduce
net-new surface.

## Open decisions — RATIFIED by user 2026-06-02 (Propose mode)

All four open decisions were ratified AS RECOMMENDED, plus #5 (defer crate split) and #6 (no metric):
- **#1 Template engine → Askama 0.12** ✅ ratified
- **#2 Render contract → selector-and-substring-identical, move-only** ✅ ratified
- **#3 htmx 2 → direct normalize-and-bump, pin htmx 2.0.x (Slice 4)** ✅ ratified
- **#4 Asset cache-busting → content-hash in committed filename + `immutable`** ✅ ratified (accepted with #1)
- **#5 `foundry-web` crate extraction → DEFERRED; templating stays inside `foundry-app`** ✅ ratified
- **#6 Per-template render metric → NOT added** ✅ ratified

Original recommendations (now ratified) preserved below for the record.

DESIGN presents these with recommendations; the user ratifies (or overrides) before DISTILL.

1. **Template engine (Open Q1 / ADR-B01)** — DESIGN recommends **Askama 0.12** (compiled-in, typed,
   workspace-blessed; moves markup to files; ≤200 ms-friendly). Alternatives weighed: Maud (rejected
   — markup stays in Rust, misses the feature's job), Minijinja (rejected — runtime/weaker safety).
2. **Render contract (Open Q2 / ADR-B02)** — DESIGN recommends **selector-and-substring-identical,
   move-only, defer visual rework** (the suite reads the DOM, not bytes). Alternative: byte-identical
   (over-constrained) / improve-and-edit-tests (destroys the net).
3. **htmx 2 approach + pin (Open Q3 / ADR-B04)** — DESIGN recommends **direct normalize-and-bump as
   one atomic Slice-4 change, pinning the latest stable htmx 2.0.x** (small core-only directive
   surface; staged compat path rejected as overhead).
4. **Asset cache-busting (Open Q4 / ADR-B03)** — DESIGN recommends **content-hash/version in the
   committed filename + `immutable` long-cache**, guarded by the asset-resolution probe (query-string
   and ETag-only options dominated).

### Additional open question flagged for the user (not one of the four, but consequential)

5. **`foundry-web` crate extraction (DD9)** — Feature A's `architecture.md` anticipated Feature B
   extracting a separate `foundry-web` crate to make web≠DB a crate-graph fact. **DESIGN recommends
   DEFERRING it** — it is orthogonal to the templating value, multiplies blast radius
   (`build_router`/`spawn_app` relocation + ~4 acceptance `AppState` sites), and web≠DB is already
   guard-enforceable without it (Principle 8). Ratify: keep the templating change inside `foundry-app`
   (recommended) vs perform the `foundry-web` extraction in this feature.

### Non-blocking / deferrable

6. **Per-template render metric (DD14)** — DESIGN recommends NOT adding one (the existing request
   histogram suffices). The user may opt to add `template_render_duration_seconds{template}` later.
