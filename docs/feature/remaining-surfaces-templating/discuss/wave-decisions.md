# Remaining-Surfaces Templating — DISCUSS Wave Decisions

> This is the file DESIGN reads FIRST. It is the DEFERRED FOLLOW-UP to Feature B
> (htmx-web-tier) — finishing the inline-`format!()`→template MOVE for the
> surfaces Feature B's `out-of-scope.md` enumerated. DESIGN here is near-trivial:
> the Askama engine, `base.html`, vendored `/static` assets, `views.rs`
> view-model layer, and the selector-and-substring-identical render contract ALL
> ALREADY EXIST and shipped with Feature B. This wave adds only the per-surface
> stories.

## Feature Summary

Move the REMAINING inline-`format!()` HTML surfaces in `foundry-app` into Askama
templates extending the EXISTING `base.html` and linking the EXISTING `/static`
assets, reusing Feature B's shipped render contract. Move-only: no new behavior,
no new infra, no auth change. Outcome: every Foundry web surface is template-
driven (0 inline `format!()` HTML left); the acceptance suite stays green.

Feature type: **user-facing** (web UI surfaces) with a contributor-maintainability
payoff — same as Feature B.

Target end state (rendering layer only; engine/base/assets already shipped):

```
foundry-app web handlers ──► templates/ + static/  (Feature B: board, issue, sign-in)
                              + project_create.html, new_issue_modal partial,
                                dashboard_root.html, events page, attachment_row partial,
                                bootstrap/claim/invite pages, shared invalid_page.html  (THIS feature)
```

## Phase 1 — Discovery & Job Grounding

### No DIVERGE directory (RISK, low impact)
No `docs/feature/remaining-surfaces-templating/diverge/`. Jobs are INHERITED
wholesale from Feature B's validated `jobs.yaml` (htmx-web-1, htmx-web-2) — not
re-derived. Mitigation: a move-only refactor of a shipped, tested UI; the
surfaces and the regression net are fixed regardless of job ranking, so the blast
radius of a mis-ranked job is nil.

### Grounded by reading the actual 2026-06 code (not assumed)
- **Confirmed still inline `format!()`** (need templating): `projects.rs::render_create_form`
  + `render_error_fragment`; `keyboard.rs::render_modal_fragment` + `render_modal_full_page`
  (+ `render_search_fragment`, `show_keyboard_help` overlay — optional tail);
  `issues.rs::bad_request_fragment` + the state-change `<span>`; `attachments.rs`
  upload-error + `render_attachment_row_oob` + `payload_too_large` + `not_found_page`;
  `bootstrap.rs::dashboard` + `render_claim_form` + `create_invite` invite-link page;
  `events.rs::unauthorized_response`; `signin.rs::dashboard_root` + the shared `invalid_page`.
- **Confirmed ALREADY templated by Feature B** (NOT in scope): `projects.rs::render_board`
  (builds a view-model, calls Askama), `signin.rs::render_signin_form` (returns
  `views::SigninPage`), the issue page + comment cards. The brief's "find dashboard_root"
  resolved to `signin.rs::dashboard_root` (still inline — in scope).
- **Two surfaces beyond the brief's list** found in `keyboard.rs`: the search-results
  fragment and the keyboard-help overlay. Recorded as an optional tail (lowest risk),
  fold into US-R02 or defer.

## Phase 2 — Scope Assessment (Elephant Carpaccio Gate)

### Scope Assessment: PASS — 6 stories (all move-only, ≤1 day each), 1 bounded context (foundry-app render layer), ~5-7 days
No oversize signal trips (full table in `story-map.md`). The feature is already
pre-split into 6 independent per-surface thin slices. No further split needed.

## Key Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| **DR1** | **Jobs are INHERITED from Feature B (htmx-web-1, htmx-web-2), NOT reinvented.** | Every remaining surface is an instance of "edit markup in a template not Rust" and/or "the screen looks styled, offline". No surface needed a new job. htmx-web-3 (htmx-2 normalization) is NOT carried — it was Feature B's done slice. |
| **DR2** | **Move-only, selector-and-substring-identical.** | Reuse Feature B's render contract verbatim. No new behavior, no redesign; visual gain comes only from now linking the existing `/static` via `base.html`. The acceptance suite is the regression net. |
| **DR3** | **Reuse Feature B's shipped infra — DESIGN is near-trivial.** | Askama engine, `base.html`, `/static` assets, `views.rs` pattern, and the fragment-vs-full-page split all exist. DESIGN decides only which existing base block / partial each surface reuses. No new engine, dependency, service, or asset. |
| **DR4** | **Walking skeleton = the project-create form (US-R01), Slice 1.** | Cheapest surface that exercises every mechanic (full page + `base.html` + `/static` + `_csrf` + a `data-hx-fragment` error fragment). Green here proves the pattern; the rest are mechanical repeats. |
| **DR5** | **Fragments stay bare; only full pages extend `base.html`.** | Inherited Feature B split. htmx swaps (modals, error divs, OOB rows, state `<span>`) must NOT re-wrap in `base.html` or the swap double-wraps. Enforced per-story AC. |
| **DR6** | **One partial per repeated component.** | New-issue modal (US-R02), attachment row (US-R05), and the shared `invalid_page` (US-R06) each get ONE definition reused across paths/callers — generalizing Feature B's MAINT-02. The shared `invalid_page` move restyles all not-found paths at once. |
| **DR7** | **Browser auth/CSRF/sessions UNCHANGED; only markup moves.** | `_csrf` field, `/bootstrap` CSRF exemption, redirect-when-signed-out, and all status codes (200/400/401/413/SEE_OTHER) are invariants (NFR-WEBB-COMPAT-03/04). |
| **DR8** | **Output uses the LEGACY per-feature layout** (separate files under `discuss/`); story IDs use the `US-R0x` namespace. | Decided with the user; `docs/product/` does not exist and we are not migrating. Mirrors `foundry-backend-mvp/discuss/` + `htmx-web-tier/discuss/`. `US-R0x` distinguishes from Feature A's `US-W0x` and Feature B's `US-B0x`. |

## Requirements Summary

- 6 user stories, all move-only, all traced to inherited jobs htmx-web-1 / htmx-web-2.
  - US-R01 — project-create form + error fragment (Slice 1, walking skeleton).
  - US-R02 — new-issue modal fragment + full-page fallback (one partial) (Slice 2).
  - US-R03 — issue-create-error + state-change fragments (Slice 3).
  - US-R04 — dashboard landing `/` + events sign-in-required page → `base.html` (Slice 4).
  - US-R05 — attachment surfaces (upload-error, OOB row, too-large, not-found) (Slice 5).
  - US-R06 — bootstrap dashboard + claim form + invite-link page + shared `invalid_page` (Slice 6).
- NFRs: INHERITED from Feature B (`nfrs.md` cites the WEBB namespace) — boundary
  preservation, render-budget no-regression, acceptance suite green, render contract
  preserved, browser auth/CSRF/sessions unchanged, WCAG 2.2 AA, markup-in-templates,
  one-partial-per-component, no new runtime services.

## Constraints Established

- ONE binary, ONE Postgres, no Redis, no Node runtime, no CDN. Reuse the existing
  vendored `/static` assets; add no dependency or service.
- Reuse (don't rebuild) Feature B's Askama engine + `base.html` + `views.rs` + render contract.
- Templates render data already fetched in the handler; no DB in the render path.
- Existing `foundry-acceptance` suite stays green throughout.
- Render contract (asserted substrings + `data-*` markers + `hx-*` directives) preserved.
- Browser auth/CSRF/sessions/status-codes unchanged; only markup moves.
- Solution-neutral choices already made by Feature B (engine = Askama); nothing new to pick.

## Risks Surfaced (for DESIGN's risk register)

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|-----------|
| A fragment surface (modal, error div, OOB row, state `<span>`) accidentally re-wraps in `base.html` and breaks the htmx swap | Medium | Medium | DR5 / per-story AC: fragments stay bare; only full pages extend `base.html`. Acceptance suite catches a broken swap. |
| A move silently changes a `data-*` marker / `hx-swap-oob` target / error copy | Medium | Medium | Selector-and-substring-identical contract (NFR-WEBB-COMPAT-02); per-story AC lists the byte-stable markers; suite green = proof. |
| Optional-tail surfaces (keyboard search/help) forgotten, leaving residual inline HTML | Low | Low | Tracked explicitly in `out-of-scope.md` + `story-map.md`; the north-star KPI (0 inline `format!()` HTML) surfaces any leftover. |
| No DIVERGE validation of inherited jobs | Low | Low | Move-only refactor of a shipped UI; surfaces + regression net fixed regardless of job ranking; jobs inherited from Feature B's validated set. |
| Render latency regression on a touched interaction surface | Low | Medium | Askama compiled-in, parity with `format!`; spot-check the bench (NFR-WEBB-PERF-01); surfaces are small. |

## Note: this is the htmx-web-tier deferred follow-up
This feature exists solely to finish what Feature B (`htmx-web-tier`) deferred. It
adds no new product capability — it generalizes Feature B's templating win to the
last remaining surfaces so that NO inline `format!()` HTML render site remains in
`foundry-app`. DESIGN should treat it as a mechanical extension of Feature B's
`render-contract.md` + `template-engine.md`, not a fresh design problem.
