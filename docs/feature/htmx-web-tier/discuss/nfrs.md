# htmx Web Tier (Feature B) — Non-Functional Requirements

> Feature B of the web-tier-extraction split. Feature A (JSON API + the
> presentation-neutral `foundry_services` seam + the CI boundary guard) has SHIPPED. These
> NFRs are scoped to the web-tier templating work. Where Feature B must PRESERVE an existing
> NFR (from backend-mvp or Feature A), that is stated explicitly and carried as a CONSTRAINT
> rather than re-derived. Each NFR is testable, measurable, and traceable to a job.

> **Source of truth**: `stories.md` for functional behavior; this file for NFRs;
> `out-of-scope.md` for deferred items. NFR ids use the `WEBB` namespace (web-tier Feature
> B) to avoid collision with web-tier-extraction's `NFR-WEB-*`.

---

## NFR-WEBB-BND — Boundary preservation (CARRIED from Feature A as constraints)

> These are NOT new jobs for Feature B (jtbd-web-2 is RETIRED — see jobs.yaml). They are
> invariants Feature B must not regress. Feature A's CI boundary guard already enforces them;
> templating must not break them.

### NFR-WEBB-BND-01: The web/template tier gains no direct database access
- **Requirement**: The templating work adds NO database-pool dependency to the web tier and
  NO new `sqlx` call site in the render path. Templates render data already fetched through
  `foundry_services`/core/store.
- **Test**: Feature A's crate-graph boundary guard stays green; zero new `sqlx::query*` sites
  in the render path; board/issue/sign-in data reaches templates only via existing service calls.
- **Linked stories**: US-B01, US-B03, US-B04, US-B06.
- **Constraint origin**: Feature A NFR-WEB-BND-01.

### NFR-WEBB-BND-03: Sanitization and authorization stay in core/handler, not the template
- **Requirement**: Markdown sanitization (`foundry_core::render_comment_markdown`, ammonia)
  and authorization decisions (membership, admin via `is_workspace_admin`, authorship)
  remain in core/handler. Templates render the RESULT (pre-sanitized HTML, boolean affordance
  flags); they perform neither.
- **Test**: Zero sanitization and zero `is_team_member`/`is_workspace_admin` call sites inside
  templates; the comment body the template renders is already sanitized (the `[x](javascript:…)`
  scenario stays green); affordance flags are computed in the handler.
- **Linked stories**: US-B03.
- **Constraint origin**: Feature A NFR-WEB-BND-03.

### NFR-WEBB-BND-04: One binary, in-process, no network hop, no new service
- **Requirement**: The templating + asset pipeline introduces NO new runtime service, NO
  second process, NO Redis, NO Node runtime, NO CDN. Still one `foundry` binary + Postgres,
  `docker compose up`.
- **Test**: `docker compose up` runs exactly one foundry container (plus Postgres) as today;
  no new inter-service socket; no outbound origin on page render.
- **Linked stories**: US-B02, US-B06.
- **Constraint origin**: Feature A NFR-WEB-BND-04 / README brand promise.

---

## NFR-WEBB-PERF — Performance (no regression)

### NFR-WEBB-PERF-01: Template rendering stays within the existing render budget
- **Requirement**: P95 server-render latency for the board, issue page, and sign-in,
  rendered via templates, is ≤200 ms at the application boundary on the backend-mvp
  reference hardware (2 vCPU, 4 GB, Postgres on host, ≤1 ms DB RTT) — i.e. templating adds
  no measurable regression vs the current `format!` path.
- **Test**: `criterion` bench on the template render path vs the `format!` baseline;
  synthetic HTTP load (50 RPS, 1,000 issues seeded). Reuses backend-mvp NFR-PERF-01 harness.
- **Linked stories**: US-B01, US-B02, US-B03, US-B04.
- **Job link**: jtbd-outcome-4 (Linear-feel speed must not degrade).

### NFR-WEBB-PERF-03: Static assets are cacheable and served locally
- **Requirement**: Vendored static assets (htmx, Alpine, CSS) are served by the binary with
  cache-friendly headers and resolve with no external origin.
- **Test**: Asset requests return 200 with a content type and a cache header; external-origin
  request count on the board = 0 (no-egress host).
- **Linked stories**: US-B02.
- **Job link**: htmx-web-2.

---

## NFR-WEBB-COMPAT — Backward Compatibility (the regression net)

### NFR-WEBB-COMPAT-01: Existing acceptance scenarios stay green
- **Requirement**: Every scenario in `foundry-acceptance` that passes before the templating
  work passes after it — including after the htmx-2 upgrade. The suite is the binding
  regression contract.
- **Test**: `cargo test -p foundry-acceptance --release` — the `[Summary]` passing count does
  not drop; no scenario regresses, including after US-B05's htmx bump.
- **Linked stories**: US-B01, US-B02, US-B03, US-B04, US-B05.
- **Job link**: jtbd-outcome-7.

### NFR-WEBB-COMPAT-02: Render contract (asserted substrings + data-* markers) preserved
- **Requirement**: HTML substrings the suite asserts on render byte-identically from the
  templates: column labels ("Backlog", "Todo", "In-Progress", "Done"), issue-key format,
  `hx-swap-oob` targets, `data-*` markers (`data-hx-fragment`, `data-comment-list`,
  `data-column`, `data-issue-key`, `.attachments-empty`), and error copy ("Title is
  required", "You may only edit your own comments.", "This comment has been deleted. Refresh
  to see the latest state.", "Invalid email or password"). The `data-*` markers are scraper
  hooks (NOT htmx directives) and survive even the htmx-2 normalization unchanged.
- **Test**: The unchanged acceptance scenarios that assert these substrings stay green; the
  htmx-normalization slice (US-B05) leaves every `data-*` marker byte-stable.
- **Linked stories**: US-B01, US-B02, US-B03, US-B04, US-B05.

### NFR-WEBB-COMPAT-03: CSRF contract unchanged
- **Requirement**: The double-submit pattern is preserved exactly: non-HttpOnly
  `foundry_csrf` cookie set on GET, `_csrf` hidden field on forms, `HX-CSRF`/`hx-csrf` header
  on htmx mutating calls, `/bootstrap` exempt, 403 on missing/invalid token, constant-time
  compare.
- **Test**: POST without a valid token -> 403 (unchanged); cookie/header names unchanged.
- **Linked stories**: US-B01, US-B03, US-B04.
- **Constraint origin**: backend-mvp NFR-SEC-04 / Feature A NFR-WEB-COMPAT-03.

### NFR-WEBB-COMPAT-04: Session contract unchanged
- **Requirement**: tower-sessions Postgres store unchanged; session cookie attributes
  (HttpOnly, Secure, SameSite=Lax, 30-day TTL) unchanged; no in-memory session state added.
- **Test**: Inspect Set-Cookie after sign-in (matches backend-mvp NFR-SEC-03).
- **Linked stories**: US-B04.

### NFR-WEBB-COMPAT-05: Non-enumerable auth error preserved
- **Requirement**: The sign-in error remains "Invalid email or password" for both unknown
  email and wrong password (no user enumeration), after the template move. The brute-force
  delay (`BRUTE_FORCE_THRESHOLD`/`WINDOW`/`DELAY`) is unchanged and server-side.
- **Test**: Same string for both cases; brute-force delay scenario unchanged.
- **Linked stories**: US-B04.

---

## NFR-WEBB-A11Y — Accessibility & Keyboard (preserved / improved)

### NFR-WEBB-A11Y-01: Keyboard operability preserved
- **Requirement**: All interactive controls on the board, issue page, and sign-in are
  keyboard-reachable; the existing `c`-to-create and j/k navigation (the hidden `#kb-items`
  carrier in `render_board`) continue to work; focus indicators are visible. WCAG 2.2 AA
  operable.
- **Test**: Keyboard-only traversal reaches every control; focus indicator present;
  backend-mvp keyboard scenarios stay green; the `#kb-items` carrier (or its templated
  equivalent) still feeds j/k navigation.
- **Linked stories**: US-B02, US-B03.

### NFR-WEBB-A11Y-02: Semantic, contrast-compliant rendering
- **Requirement**: Templates emit valid semantic HTML; form inputs have associated labels;
  text contrast ≥4.5:1 (3:1 large); interactive targets ≥24×24 px. WCAG 2.2 AA.
- **Test**: Automated a11y lint on the board/issue/sign-in templates; contrast check on the
  vendored stylesheet.
- **Linked stories**: US-B02, US-B03, US-B04.

---

## NFR-WEBB-MAINT — Maintainability (the contributor payoff — jtbd htmx-web-1)

### NFR-WEBB-MAINT-01: Markup lives in templates, not handlers
- **Requirement**: On-screen text and markup for the extracted surfaces live in template
  files, not in handler `format!` literals. Grepping for on-screen text lands in
  `templates/`, not in `projects.rs`/`comments.rs`/`signin.rs`. Full pages extend ONE base
  layout (no duplicated `<head>`/asset boilerplate).
- **Test**: Code inspection — extracted surfaces have no inline HTML `format!` in handlers;
  auth/board templates extend the base layout; 0 duplicated `<head>` blocks.
- **Linked stories**: US-B01, US-B03, US-B04.
- **Job link**: htmx-web-1.

### NFR-WEBB-MAINT-02: One partial per repeated component
- **Requirement**: The issue-card and the comment-card each have ONE template definition,
  consumed by full-page, htmx-fragment, and SSE render paths. No component is rendered by
  more than one definition. (This also FIXES today's `render_comment_card_oob` divergence,
  which omits affordances.)
- **Test**: Code inspection — single issue-card partial, single comment-card partial; the
  live-vs-reloaded card structural-equality scenario (US-B03) stays green.
- **Linked stories**: US-B01, US-B03.
- **Job link**: htmx-web-1.

---

## NFR-WEBB-INFRA — Infrastructure invariants (no new services)

### NFR-WEBB-INFRA-01: No new runtime services or dependencies; htmx vendored + pinned
- **Requirement**: The templating + asset pipeline adds NO new runtime service (no Redis, no
  Node server, no bundler service), NO new container, and NO CDN. htmx, Alpine, and CSS are
  vendored into `static/` and served by the binary; htmx is pinned at a single version (after
  US-B05). Still one binary, one Postgres, `docker compose up`.
- **Test**: `docker compose` topology unchanged (foundry + postgres); no outbound origin on
  page render; exactly one vendored htmx file with a recorded version.
- **Linked stories**: US-B02, US-B05, US-B06.
- **Job link**: htmx-web-2 + htmx-web-3 + brand promise.

### NFR-WEBB-INFRA-02: Any asset build step is build-time only; image identical across deployments
- **Requirement**: (inherits backend-mvp posture) IF DESIGN adopts a build-time asset step,
  it is build-time only and introduces NO runtime secret and NO runtime service; the produced
  image is identical across deployments. (Whether a build-time Node/esbuild step is acceptable
  at all is an OPEN question for DESIGN — see `out-of-scope.md`.)
- **Test**: `docker inspect` shows no new runtime secret/service introduced by the web tier;
  no runtime Node process.
- **Linked stories**: US-B02, US-B06.

---

## NFR-MATRIX — Story-to-NFR Coverage Matrix

| NFR | B01 | B02 | B03 | B04 | B05 | B06 |
|-----|-----|-----|-----|-----|-----|-----|
| BND-01    | x |   | x | x |   | x |
| BND-03    |   |   | x |   |   |   |
| BND-04    |   | x |   |   |   | x |
| PERF-01   | x | x | x | x |   |   |
| PERF-03   |   | x |   |   |   |   |
| COMPAT-01 | x | x | x | x | x |   |
| COMPAT-02 | x | x | x | x | x |   |
| COMPAT-03 | x |   | x | x |   |   |
| COMPAT-04 |   |   |   | x |   |   |
| COMPAT-05 |   |   |   | x |   |   |
| A11Y-01   |   | x | x |   |   |   |
| A11Y-02   |   | x | x | x |   |   |
| MAINT-01  | x |   | x | x |   |   |
| MAINT-02  | x |   | x |   |   |   |
| INFRA-01  |   | x |   |   | x | x |
| INFRA-02  |   | x |   |   |   | x |
