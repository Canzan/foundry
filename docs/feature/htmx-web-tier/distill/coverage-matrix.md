# DISTILL Coverage Matrix — Feature B "htmx Web Tier" (`htmx-web-tier`)

Mirrors `docs/feature/web-tier-extraction/distill/coverage-matrix.md` (Feature A).
Maps every US-B0x acceptance criterion to a scenario, OR records it as
**regression-net-only** (the move is covered by the EXISTING suite staying
green — NFR-WEBB-COMPAT-01 — so no new RED scenario is authored).

This is a **move-only refactor** (DESIGN ADR-B02, selector-and-substring-
identical). The binding rule from `render-contract.md`: *do NOT re-assert
existing green output*. The existing `us-08-file-issue.feature` (board),
`us-10-comments.feature` / `us-10-comment-edit-delete.feature` (comments),
`us-06-signin.feature` (sign-in) ARE the regression net for the unchanged
markup. New RED scenarios cover ONLY the genuine user-visible deltas.

## Lane / tag legend

| Tag | Meaning |
|---|---|
| `@feature-b` | all Feature B scenarios |
| `@walking_skeleton` | the one demo-proof scenario per surface (exactly one carries the slice's headline) |
| `@real-io` | real in-process axum router + real Postgres (Architecture of Reference: driving + driven-internal real) |
| `@driving_adapter` / `@driving_port` | entered through the HTTP surface (board route / sign-in route / `/static` ServeDir) |
| `@adapter-integration` | the static-serving driven adapter exercised with real filesystem I/O |
| `@error` | error/edge path |
| `@slice1..4` | release slice |
| `@acme` | uses the shared Acme workspace fixture |

No `@skip`/`@pending` marker is used — matching the existing harness convention
(Feature A). Scenarios run live; RED comes from MISSING_FUNCTIONALITY (missing
`/static` route, empty `static/`, the OOB-affordance bug, unvendored htmx).

## Per-story coverage

### US-B01 — Render the board from a template (Slice 1)

| AC | Scenario | RED reason / status |
|---|---|---|
| Board route renders from a template, not inline `format!` | (regression-net-only) | The existing `us-08` board scenarios stay green; "renders from a template" is a code-inspection AC (no inline `format!` HTML site), not user-observable. Verified by DELIVER + the green suite. |
| Board / create-fragment / state-change share ONE issue-card partial | (regression-net-only) | Code-inspection AC (one `issue_card.html`); the existing card-fragment scenarios stay green. |
| Card renders identically across full-page / OOB / SSE | US-B05 "render-contract data markers byte-unchanged" + existing OOB scenarios | Markers asserted; behaviour from existing suite. |
| Asserted substrings render byte-identically | (regression-net-only) | NFR-WEBB-COMPAT-02 — the existing suite IS the assertion. |
| Data reaches template via `foundry_services` (no new DB in render) | (regression-net-only / boundary guard) | NFR-WEBB-BND-01 — Feature A's CI boundary guard + no new `sqlx` in render path. |
| Empty board renders an inviting empty state with a call to action | **US-B01 "An empty board shows an inviting, templated empty state"** | RED: today emits bare `<p class="empty">No issues yet</p>` (no "press c"/"file the first"). |
| Styled board references vendored assets | **US-B01 WS "A member opens a styled board…"** | RED: today `render_board` emits no `<link>`/`<script>`; no `/static` route. |
| Previously-green board scenarios stay green | (regression-net-only) | The existing `us-08` board scenarios. |
| Keyboard carrier (`#kb-items`) preserved, ASC order | **US-B01 "preserves the hidden keyboard-navigation order"** | Asserts `#kb-items [data-issue-key]` ASC order (NFR-WEBB-A11Y-01). |
| Render failure → clean 500, not half-page | **US-B01 @error "board template that fails to render…"** | RED: the test-only render-failure seam does not exist; board returns 200 today. |

### US-B02 — Vendored assets served by the binary (Slice 1; folds US-B06)

| AC | Scenario | RED reason / status |
|---|---|---|
| htmx/Alpine/CSS vendored under static path, served (200 + content type) | **US-B02 WS "vendored htmx, Alpine, and stylesheet are served"** (`@adapter-integration`) | RED: `static/` empty + no `/static` route → 404. THE static-serving adapter's `@real-io` coverage (Mandate 6). |
| Assets are real (non-empty) | **US-B02 "vendored htmx asset is a real, non-empty script"** | RED: 404 / 0 bytes today. |
| No external-origin request on the board | **US-B01 WS "references no external origin"** | Asserted on the board markup (no `http(s)://` / `//` asset refs). |
| Long-lived cache header | **US-B02 WS** (Cache-Control assertion) | RED until ServeDir + `immutable` header. |
| Board keyboard-operable, visible focus (WCAG) | (regression-net-only / `@manual`) | NFR-WEBB-A11Y-01 keyboard carrier covered by US-B01; focus-indicator is a stylesheet/`@manual` a11y-lint concern (no Playwright — backend-mvp decision). |
| Missing referenced asset caught | **US-B02 @error "asset that is not vendored is refused"** | RED: 404 path; the build-time asset-resolution probe is a `@manual`/xtask CI check (documented in step-skeletons). |
| Static route refuses path traversal | **US-B02 @error "refuses to serve a file outside its own directory"** | Satisfied by ServeDir by construction once mounted; RED until mounted. |

### US-B06 — Template + static-asset pipeline (`@infrastructure`, folded into Slice 1)

| AC | Scenario | RED reason / status |
|---|---|---|
| Template engine wired; templates load from `templates/` | (regression-net-only / compile-time) | Askama compile-time check (a missing template = build error); proven once US-B01/B02 render through it. |
| Static route serves only `static/` (no traversal), no CDN | **US-B02 @error traversal** + **US-B01 "no external origin"** | Covered by the US-B02 traversal scenario + the no-egress assertion. |
| Base layout exists for full pages | **US-B04 "renders from the shared base layout"** | The sign-in/forgot pages assert base-layout shape. |
| Pipeline adds no runtime service / DB dep | (regression-net-only / boundary guard) | NFR-WEBB-INFRA-01 / BND-01. |

### US-B03 — One comment-card partial + the OOB-affordance bug fix (Slice 2)

| AC | Scenario | RED reason / status |
|---|---|---|
| Issue page + all comment paths use one comment-card partial | (regression-net-only) | Code-inspection AC; existing comment scenarios stay green. |
| **Live (OOB) card == reloaded card, including affordances** | **US-B03 WS "A live-posted comment card carries the same affordances as a reloaded one"** | **RED — THE bug fix made observable.** Today `render_comment_card_oob` (comments.rs:841) omits `.comment-actions`; live card actions=false, reloaded=true. |
| Edit/Delete decided in handler, rendered as flags (no authz in template) | **US-B03 "reader sees no edit/delete"**, **"admin sees delete not edit"** | These pass GREEN today (existing authz gating) — they are regression guards living beside the WS RED, proving the affordance RULE is unchanged. |
| Markdown sanitization stays in core | **US-B03 @error "dangerous comment is sanitized in core"** | GREEN today (existing core sanitization) — regression guard. |
| 400/403/410 error copy preserved | (regression-net-only) | Existing `us-10-comment-edit-delete` scenarios. |
| Author + body + edited-marker render | **US-B03 "issue page renders author and body"**, **"edited marker appears"** | GREEN today — regression guards on the unchanged render contract. |
| Previously-green comment scenarios stay green | (regression-net-only) | `us-10-comments` / `us-10-comment-edit-delete`. |

### US-B04 — Sign-in / forgot to the shared base layout (Slice 3)

| AC | Scenario | RED reason / status |
|---|---|---|
| Sign-in + forgot render from templates extending base layout, referencing vendored assets | **US-B04 WS "renders from the shared layout"**, **"forgot-password renders from the shared layout"** | RED: today `render_signin_form` emits a bare `<head>` with no `<link>` stylesheet. |
| Session-cookie attrs (HttpOnly/Secure/SameSite=Lax/30-day) unchanged | **US-B04 WS** (cookie-attr assertion) | GREEN today — regression guard (NFR-WEBB-COMPAT-04). |
| Non-enumerable "Invalid email or password" unchanged | **US-B04 @error "non-enumerable error"** | GREEN today — regression guard (NFR-WEBB-COMPAT-05). |
| CSRF contract unchanged (cookie on GET, hidden field, 403 on missing) | **US-B04 "anti-forgery contract is preserved"** | GREEN today — regression guard (NFR-WEBB-COMPAT-03). |
| Previously-green sign-in scenarios stay green | (regression-net-only) | `us-06-signin`. |

### US-B05 — Normalize htmx directives + pin htmx 2 (Slice 4)

| AC | Scenario | RED reason / status |
|---|---|---|
| All active htmx directives use one consistent convention | (regression-net-only / code-inspection) | DD6 — confirm/centralize; verified by inspection + the green suite. |
| htmx vendored at exactly one pinned 2.x version | **US-B05 WS "served htmx asset is a single pinned version 2"** | RED: htmx unvendored/unpinned today (`static/` empty). |
| Every hx-driven interaction has a green regression scenario after the bump | **US-B05 "filing an issue still appends…"**, **"posting and editing a comment still swap…"** | GREEN-after-bump regression net (DB4); plus existing OOB/edit scenarios. |
| `data-*` render-contract markers byte-unchanged | **US-B05 "render-contract data markers left byte-unchanged"** | Asserts `[data-column='backlog']`, `[data-issue-key]`, `[data-comment-list]`. |
| Full suite stays green after the bump | (regression-net-only) | NFR-WEBB-COMPAT-01. |

## Adapter coverage (Mandate 6)

| Driven adapter | `@real-io` scenario | Covered by |
|---|---|---|
| Static-asset serving (`tower_http::ServeDir` over `static/`, real filesystem) | YES | US-B02 WS "vendored htmx, Alpine, stylesheet are served" + "non-empty script" + traversal `@error` (`@adapter-integration`) |
| Postgres (board/issue/comment data via `foundry_services` seam) | YES | every `@real-io` board/comment scenario (real per-scenario schema) |
| Template engine (Askama, compiled-in) | n/a (not a runtime driven port) | compile-time presence probe + green render scenarios (US-B01/B04) |

No `NO — MISSING` rows. The only NEW driven adapter Feature B introduces is the
static-serving route; it has its own `@real-io @adapter-integration` coverage.

## Scenario counts

| File | Total | Happy/WS | Error/edge | RED today |
|---|---|---|---|---|
| us-b01-styled-board | 4 | 3 | 1 | 3 |
| us-b02-vendored-assets | 4 | 2 | 2 | 2 |
| us-b03-comment-partial | 7 | 5 | 2 | 1 |
| us-b04-signin-layout | 4 | 3 | 1 | 2 |
| us-b05-htmx2 | 4 | 4 | 0 | 1 |
| **Total** | **23** | **17** | **6** | **9** |

Error/edge ratio: **6/23 ≈ 26%**. Below the 40% target — JUSTIFIED: this is a
**move-only refactor of an already-tested UI**. The error/sad paths (invalid
input, authz refusal, 403/410/422, non-enumerable error, brute-force) are
ALREADY exhaustively covered by the existing `us-06`/`us-08`/`us-10` suite,
which is the binding regression net (NFR-WEBB-COMPAT-01) and MUST stay green.
Authoring duplicate error scenarios here would violate the render-contract
discipline ("do NOT re-assert existing green output", ADR-B02). The 6 new
error/edge scenarios cover only the genuine NEW failure modes the feature
introduces (missing asset → 404, path traversal, render-failure → clean 500,
non-author/admin affordance gating on the moved partial, non-enumerable error
on the moved sign-in form).

## Walking skeletons

One `@walking_skeleton` per surface slice (5 total across the feature, one per
released slice), each demo-able to a stakeholder:
- US-B01: "Mei opens a styled board that still shows the same issues" (Slice 1 headline).
- US-B02: "the vendored htmx/Alpine/stylesheet are served by the binary" (the offline-asset proof).
- US-B03: "a live-posted comment card carries the same affordances as a reloaded one" (the bug fix).
- US-B04: "the sign-in page renders from the shared layout and still signs the member in".
- US-B05: "the served htmx asset is a single pinned version 2".
