# htmx Web Tier (Feature B) — DISCUSS Wave Decisions

> This is the file DESIGN reads FIRST. Feature B of the web-tier-extraction split —
> "Foundry looks like a product." Feature A (the first-class JSON API + machine-token auth +
> the presentation-neutral `foundry_services` seam + the CI boundary guard) has SHIPPED. This
> wave VALIDATED and refined the strawman that web-tier-extraction sketched for Feature B
> (US-W01..W04, jtbd-web-1/2/3), being explicit about what was carried as-is vs revised vs
> retired. **Strawman-validation pass — not a fresh greenfield discovery.**

## Feature Summary

Replace Foundry's inline `format!()`-string HTML (scattered across `foundry-app` handlers)
with a **real template engine + vendored htmx 2 + Alpine.js + a static-asset pipeline**, and
perform the deferred **htmx 1->2 normalization/upgrade**. Reuse the `foundry_services` seam
Feature A proved (browser handlers already call it). Outcome: a consistent, maintainable,
product-grade server-rendered UI — no SPA, still one binary.

Feature type: **user-facing** (web UI surfaces: board, issue+comments, sign-in) with a
contributor-maintainability payoff.

Target end state (rendering layer only; data/auth/API already in place from Feature A):

```
foundry (one binary)
├── foundry-api    JSON API (Feature A — shipped)
├── foundry_services / foundry-core / foundry-store / foundry-auth   (the seam — shipped)
└── foundry-app web handlers  ──►  templates/ + static/ (NEW: this feature)
                                    (template engine + vendored htmx2/Alpine/CSS)
```

## Phase 1 — Discovery & Job Grounding

### No DIVERGE directory (RISK, low impact)
There is no `docs/feature/htmx-web-tier/diverge/` (no validated `recommendation.md`/
`job-analysis.md`). Jobs are Luna-derived from (a) backend-mvp JTBD, (b) the
web-tier-extraction strawman, and (c) a fresh 2026-06 code reading. Mitigation: the work is a
rendering-layer refactor of an already-shipped, already-tested UI, so the blast radius of a
mis-ranked job is bounded (the surfaces and the regression net are fixed regardless).

### What was grounded by reading the actual 2026-06 code (not assumed)
- `templates/` and `static/` are EMPTY (confirmed via glob — no files). Template engine,
  vendored htmx/Alpine, and CSS are all genuinely net-new.
- The browser handlers ALREADY delegate to `foundry_services`: `projects::show_board` ->
  `foundry_services::board::list_board_issues`; `issues::submit_create` ->
  `issue_service::create_issue`; `issues::submit_state_change` -> `change_issue_state`;
  `comments::submit_comment`/`submit_edit_comment` -> `comment_service::create_comment`/
  `edit_comment`. **So the presentation-neutral seam jtbd-web-2 described is ALREADY DONE
  (Feature A).**
- The remaining inline-HTML concern is purely RENDERING: `render_board` (projects.rs),
  `render_issue_page`/`render_comment_card`/`render_comment_card_oob` (comments.rs),
  `render_issue_card` (issues.rs), `render_signin_form`/`render_forgot_form` (signin.rs) — all
  `format!()` literals.
- htmx attribute reality (refines the strawman's "mixed prefixes"): ACTIVE htmx directives are
  bare `hx-*` (`hx-patch`/`hx-get`/`hx-target`/`hx-swap`/`hx-delete` in the comment edit-form;
  `hx-swap-oob` in the create card + comment OOB). The `data-hx-fragment`/`data-column`/
  `data-comment-list`/`data-issue-key` attributes are PASSIVE SCRAPER MARKERS the acceptance
  suite asserts on — NOT htmx directives. The real htmx-2 migration surface is small.
- `render_comment_card_oob` deliberately OMITS Edit/Delete "for simplicity" — so the live card
  already differs from a reloaded card. The one-partial work FIXES this (a real UX improvement).
- Browser auth/CSRF/sessions are intact in `signin.rs`/`csrf.rs` (argon2id, brute-force delay,
  `GENERIC_SIGNIN_ERROR`, double-submit cookie + `HX-CSRF`, tower-sessions Postgres, 30-day
  cookie) — all to be PRESERVED unchanged.

## Phase 2 — Scope Assessment (Elephant Carpaccio Gate)

### Scope Assessment: PASS — 6 stories (1 `@infrastructure`, folded), 1 templating concern, ~7-10 days
No oversize signal trips for Feature B in isolation (full table in `story-map.md`). This is
exactly the independently-shippable Feature B that web-tier-extraction D8/D9 carved out. No
further split needed.

## Strawman validation — KEPT vs REVISED vs RETIRED

| Strawman item (web-tier-extraction) | Verdict | What changed |
|-------------------------------------|---------|--------------|
| jtbd-web-1 (restyle without Rust) | **KEPT + PROMOTED** -> `htmx-web-1` | Was secondary (imp 7); now PRIMARY (imp 8). Validated: every on-screen string is in handler `format!()`. |
| jtbd-web-3 (first screen looks like a product) | **KEPT** -> `htmx-web-2` | PRIMARY (imp 7). Validated: `static/` genuinely empty, so satisfaction nudged 4->3. |
| jtbd-web-2 (web/api peer consumers of one core) | **RETIRED as a job** | Code already delegates to `foundry_services` (Feature A). Now carried as a CONSTRAINT (NFR-WEBB-BND-*), not a Feature-B driver. **USER MUST CONFIRM.** |
| (none) htmx-1->2 migration | **NEW story** -> `htmx-web-3` / US-B05 | Carries the deferred D3 migration; reframed (small `hx-*` surface; `data-*` markers untouched). |
| US-W01 board extraction | **KEPT, re-traced** -> US-B01 | Job changed from jtbd-web-2 to htmx-web-1 (boundary is done; restyle is the point). |
| US-W02 vendored assets | **KEPT** -> US-B02 | Air-gap framing sharpened (static/ empty). |
| US-W03 issue+comments | **KEPT** -> US-B03 | Adds explicit fix for the OOB-omits-affordances divergence. |
| US-W04 sign-in/base layout | **KEPT** -> US-B04 | Unchanged in intent; grounded in `signin.rs`. |
| US-W05a/b/c, US-W06 | **DROPPED** | Feature A (shipped) — out of Feature B. |

## Key Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| **DB1** | **jtbd-web-2 RETIRED as a Feature-B job; carried as a constraint.** | The browser handlers already consume the `foundry_services` seam and Feature A shipped the api peer + boundary guard, so "web/api are peer consumers of one neutral core" is DONE, not a job to do. Feature B must not REGRESS it (NFR-WEBB-BND-*). **CONFIRMED by user 2026-06-02.** |
| **DB2** | **PRIMARY jobs for Feature B = htmx-web-1 (restyle without Rust) + htmx-web-2 (styled first screen).** htmx-web-3 (htmx normalize/upgrade) is supporting. | With Feature A shipped, the contributor's restyle pain and the self-hoster's unstyled-first-screen are the real drivers. These are no longer a "secondary track". |
| **DB3** | **Walking skeleton = the board (US-B01 + US-B02 + US-B06), Slice 1.** | Prove the templating + vendored-asset pipeline + the ≤200 ms budget on the highest-traffic real surface first; riskiest assumption (engine in budget, assets without a runtime service). |
| **DB4** | **htmx 1->2 migration is a DEDICATED slice (Slice 4, US-B05), NOT per-surface.** | After templating, the directive set is small and centralized; one atomic, fully regression-tested bump beats N per-surface upgrades with a mixed-version window. Slices 1-3 move directives AS-IS (no version bump). The `data-*` scraper markers are NOT htmx and are untouched. |
| **DB5** | **Solution-neutral.** Template engine, htmx 2.x version pin, and CSS strategy are DESIGN. | DISCUSS fixes the constraints (render budget, one-partial rule, no DB in render, no runtime service, no CDN, render contract); DESIGN picks the tools. |
| **DB6** | **RESOLVED (user 2026-06-02): PURE VENDORED BLOBS — no build-time Node/esbuild step.** | Carried from web-tier-extraction D4, now decided. Minified htmx 2.x / Alpine / CSS are committed (pinned) directly into `static/` and served by the binary; NO Node toolchain at runtime OR image-build time, NO CDN. A build-time JS toolchain is now explicitly OUT of scope (see out-of-scope.md). DESIGN picks only the template engine + how the blobs are organized/cache-busted. |
| **DB7** | **Browser auth/CSRF/sessions UNCHANGED; only markup moves.** | The sign-in/forgot handlers, CSRF contract, cookie attrs, brute-force delay, and non-enumerable error are invariants (NFR-WEBB-COMPAT-03/04/05). Templating touches markup only. |
| **DB8** | **Output uses the LEGACY per-feature layout** (separate files under `discuss/`), NOT the SSOT/feature-delta model; story IDs use the `US-B0x` namespace. | Decided with the user; `docs/product/` does not exist and we are intentionally not migrating. Mirrors `foundry-backend-mvp/discuss/`. `US-B0x` distinguishes Feature B from Feature A's `US-W0x`. |

## Requirements Summary

- 6 user stories, 1 explicitly `@infrastructure` (US-B06, pipeline scaffolding, folded into Slice 1).
- **Primary track — contributor restyle + styled first screen (htmx-web-1 / htmx-web-2):**
  - US-B01 — board -> template over the existing seam (Slice 1).
  - US-B02 — board from vendored htmx/Alpine/CSS, no CDN (Slice 1).
  - US-B03 — issue detail + comment thread -> one templated card partial (Slice 2).
  - US-B04 — sign-in + forgot-password -> shared base layout (Slice 3).
- **Supporting track — htmx readiness (htmx-web-3):**
  - US-B05 — normalize htmx directives + vendor/pin htmx 2.x (Slice 4).
- **Infrastructure (folded):** US-B06 — template engine + static-asset pipeline (Slice 1).
- NFRs: boundary preservation (carried) + render-budget no-regression + acceptance suite green
  + render contract preserved + browser CSRF/sessions/error unchanged + WCAG 2.2 AA keyboard +
  markup-in-templates + one-partial-per-component + no new runtime services + htmx vendored/pinned.

## Constraints Established

- ONE binary, ONE Postgres, no Redis, no Node runtime service, no CDN. Assets vendored, served
  by the binary.
- Reuse (don't rebuild) the `foundry_services` seam; templates render data already fetched;
  sanitization/authz stay in core/handler; web tier gains no DB pool.
- Existing `foundry-acceptance` suite stays green throughout, including after the htmx-2 bump.
- Render contract (asserted substrings + `data-*` scraper markers) preserved byte-for-byte.
- Browser auth/CSRF/sessions/non-enumerable-error unchanged; only markup moves.
- Solution-neutral: template engine, htmx 2.x version, CSS strategy are DESIGN.

## Risks Surfaced (for DESIGN's risk register)

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|-----------|
| Template engine adds render latency above the ≤200 ms budget | Medium | High | Benchmark the render path on Slice 1's board before extracting other surfaces; reject engines that miss the budget (NFR-WEBB-PERF-01). |
| Acceptance scenarios assert on HTML substrings; templating changes whitespace/markup | High | Medium | Keep asserted substrings + `data-*` markers byte-stable; treat as a render contract (NFR-WEBB-COMPAT-02). |
| htmx-2 normalization breaks an existing hx-driven swap, or touches a `data-*` scraper marker | Medium | Medium | Dedicated slice with a green regression scenario per interaction; `data-*` markers explicitly out of the htmx migration (US-B05). |
| Build-time asset step decision adds a Node/CI toolchain dependency or hurts air-gap builds | Medium | Medium | Flagged as a DESIGN open question (DB6); constraint fixed (no runtime service, no CDN, reproducible image). |
| jtbd-web-2 wrongly retired (boundary not actually done) | Low | Medium | Verified by code reading (handlers call `foundry_services`); confirm with user; the boundary guard from Feature A still enforces it regardless. |
| Secondary jobs lack DIVERGE validation | Medium | Low | Rendering-layer refactor of a shipped UI; surfaces + regression net fixed regardless of job ranking; confirm ranking before DESIGN. |
| Keyboard/j-k nav (`#kb-items` carrier) breaks in templating | Low | Medium | NFR-WEBB-A11Y-01 requires the templated board to preserve the hidden keyboard carrier; backend-mvp keyboard scenarios stay green. |
