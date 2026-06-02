<!-- markdownlint-disable MD024 -->
# htmx Web Tier (Feature B) — User Stories

> Feature B of the web-tier-extraction split — "Foundry looks like a product." Feature A
> (the JSON API + the presentation-neutral `foundry_services` seam + the CI boundary
> guard) has SHIPPED. These stories refine the strawman US-W01..W04 from
> `web-tier-extraction/discuss/stories.md` against a fresh 2026-06 code-surface reading,
> renumbered into the `US-B0x` namespace. Every story is solution-neutral: this wave does
> NOT pick the template engine, the htmx 2 version, or the CSS approach — those are DESIGN.

## What changed from the strawman (validation notes)

- **US-W01's job changed.** The strawman traced US-W01 to jtbd-web-2 ("web is a peer
  consumer of one core"). Reading the code, the browser handlers ALREADY consume the
  `foundry_services` seam (`projects::show_board` -> `list_board_issues`;
  `issues::submit_create` -> `issue_service::create_issue`), so that boundary is DONE
  (Feature A). US-B01 is re-traced to **htmx-web-1** (the contributor's restyle job) and
  the boundary becomes a CONSTRAINT (NFR-WEBB-BND-01), not the story's purpose.
- **US-W05a/b/c and US-W06 are dropped** — they are Feature A (shipped).
- **A new htmx-2-migration story (US-B05)** carries the deferred htmx 1->2 normalization,
  reframed: the active directives are bare `hx-*` (small surface), and the `data-hx-*`/
  `data-*` attributes are passive scraper markers, NOT htmx directives — they must be left
  untouched.
- **US-W02's air-gap framing is kept and sharpened** — `static/` is genuinely empty, so
  "no CDN / served by the binary" is verifiable from a no-egress host.

## System Constraints (cross-cutting)

Apply to every story; measurable forms live in `nfrs.md`.

- **Reuse, don't rebuild, the seam.** Feature A's `foundry_services` seam is the source of
  data and the home of authz/sanitization. Templates render data already fetched; they
  make no DB calls, no authz decisions, no sanitization. (Carried from Feature A as
  NFR-WEBB-BND-01/03; enforced by Feature A's existing CI boundary guard.)
- **One binary, no new runtime services.** No Node runtime, no bundler service, no Redis,
  no CDN. Assets are vendored into `static/` and served by the binary. (A *build-time*
  asset step is an OPEN question for DESIGN — see `out-of-scope.md`.)
- **Acceptance suite is the regression net.** Every `foundry-acceptance` scenario green
  before this feature stays green after. Asserted HTML substrings (column labels, issue
  keys, `data-*` markers, error copy) are a render contract templates honor byte-for-byte.
- **Browser auth/CSRF/sessions unchanged.** Double-submit `foundry_csrf` cookie + `_csrf`
  field / `HX-CSRF` header; tower-sessions Postgres store; 30-day cookie attrs; argon2id;
  brute-force delay; non-enumerable sign-in error — all untouched.
- **Keyboard-first / WCAG 2.2 AA.** Rendered HTML stays keyboard-operable with visible
  focus; the existing `c`-to-create and j/k navigation keep working.
- **Solution-neutral.** Template engine, htmx 2 version, and CSS strategy are DESIGN.

## Glossary (additions for this feature)

- **Template / partial**: a `.html` template file (full page) or reusable fragment (e.g.
  the issue-card, the comment-card) rendered identically across full-page, htmx-swap, and
  SSE paths. NEW for this feature; `templates/` is empty today.
- **Vendored asset**: htmx, Alpine, or CSS shipped inside the binary's `static/` dir
  (empty today), served locally with no CDN.
- **Base layout**: the one template (head, vendored `<link>`/`<script>` tags, title) that
  full pages extend, for consistency (Nielsen #4).
- **Render contract**: HTML substrings + `data-*` markers the acceptance suite asserts on;
  a stable interface templates must preserve.
- **htmx directive vs scraper marker**: `hx-*` attributes are active htmx behavior;
  `data-hx-fragment` / `data-column` / `data-comment-list` / `data-issue-key` are passive
  markers for tests/scrapers — NOT htmx, and NOT part of the htmx-2 migration.

---

# ==========================================================================
# Slice 1 — Walking Skeleton: the board is a styled, templated product surface
# US-B01 (board -> template) + US-B02 (vendored assets) + US-B06 (pipeline, @infra)
# ==========================================================================

## US-B01: Render the issue board from a template

- **job_id**: htmx-web-1 (Minimize effort to restyle/re-word a screen without touching Rust)

### Elevator Pitch
- **Before**: Mei opens `http://localhost:3000/team/backend/project/auth-v2`; the board's
  on-screen text ("No issues yet", "New issue", the column headings) lives inside
  `render_board` in `projects.rs` as `format!()` string literals next to a board query and
  `column_label_to_state`.
- **After**: she opens the same URL and sees the same board — same columns, same cards,
  same `c`-to-create — now rendered from `templates/board.html`, with the on-screen text
  living in that template; a contributor changing the empty-state wording edits the
  template, not `projects.rs`.
- **Decision enabled**: a contributor decides they can change board wording/layout in one
  template file, confident (acceptance suite green) they changed no behavior.

### Problem
Jamal (contributor) cannot change board wording or layout without editing
`projects.rs::render_board` — a `format!()` block interleaved with the column-to-state
mapping and the issue-card rendering. Reviewing a "make the board prettier" PR means
reading Rust string literals. The board is the highest-traffic surface, so it is where
templating matters most and where a regression would hurt most.

### Who
- **Jamal Okafor**, Rust contributor, wants to change board UI without fear of breaking behavior.
- **Mei Chen**, member viewing the board, must perceive zero behavioral change.
- **Context**: existing `/team/{team}/project/{project}` board route, rendered today by
  `projects::show_board` -> `render_board` via `format!`, with data already fetched through
  `foundry_services::board::list_board_issues`.
- **Motivation**: board markup that is reviewable as markup, in a predictable location.

### Solution
Render the board route from a template (`templates/board.html`) plus a reusable issue-card
partial. The handler keeps fetching data through the existing `foundry_services` seam (no
change to data access) and passes the result to the template. The issue board, the
issue-create fragment (`render_issue_card_with_column_marker`), and the state-change
fragment all render the issue through the SAME card partial. The existing board acceptance
scenarios run unchanged and stay green.

### Domain Examples
#### 1. Happy path — board renders via a template
Mei opens Auth v2. The board renders from `templates/board.html`
(Backlog/Todo/In-Progress/Done columns; AUTH-2/3/6/7 cards) using data from
`list_board_issues`. The HTML preserves the asserted substrings ("Backlog", the issue key,
`data-column`, `data-issue-key`). She presses `c`, files "Refresh token rotation broken on
Safari", and AUTH-8 appears in Backlog via the same `hx-swap-oob` card fragment — now
produced by the shared card partial.
#### 2. Edge case — board with zero issues
Devansh opens the brand-new "Sandbox" project. The template renders the four empty columns
with an inviting empty state ("No issues yet — press c to file the first one") instead of
the bare `<p class="empty">No issues yet</p>` today. The empty render still goes through
`list_board_issues` (returns an empty list).
#### 3. Error/boundary case — a wording change must not touch handler logic
A maintainer asks Jamal to reword the empty state. Jamal edits only
`templates/board.html`; `projects.rs` is untouched; the acceptance suite stays green,
proving behavior is unchanged.

### UAT Scenarios
```gherkin
Scenario: Board renders through a template with the same content
  Given Mei is signed in as a member of the Backend team
  And the Auth v2 project has issues AUTH-2, AUTH-3, AUTH-6 in Backlog/Todo/In-Progress
  When Mei opens the Auth v2 board
  Then she sees the columns "Backlog", "Todo", "In-Progress", "Done"
  And she sees the cards for AUTH-2, AUTH-3, and AUTH-6 in their respective columns
  And the page is rendered from a template file rather than an inline format! string

Scenario: Filing an issue still returns the same card fragment
  Given Mei is viewing the Auth v2 board
  When Mei files an issue titled "Refresh token rotation broken on Safari"
  Then a new card with the next sequential key appears in the Backlog column
  And the returned fragment marks the Backlog column as its swap target

Scenario: Empty board shows an inviting empty state
  Given the Sandbox project has no issues
  When Mei opens the Sandbox board
  Then she sees the four state columns
  And she sees guidance explaining how to file the first issue

Scenario: A board wording change touches only a template
  Given a contributor changes the empty-board guidance text
  When the change is made
  Then only a template file is edited and no handler or store file changes
  And the existing board acceptance scenarios still pass

Scenario: The existing board acceptance scenarios remain green
  Given the foundry-acceptance suite includes the board scenarios
  When the suite runs against the templated board
  Then every previously-passing board scenario still passes
```

### Acceptance Criteria
- [ ] The board route renders from a template file, not an inline `format!` block in `projects.rs`.
- [ ] Board, issue-create fragment, and state-change fragment share one issue-card partial.
- [ ] The card partial renders identically in full-page, htmx-swap, and SSE paths.
- [ ] The asserted substrings ("Backlog"/"Todo"/"In-Progress"/"Done", issue key,
      `data-column`, `data-issue-key`) render byte-identically.
- [ ] Data still reaches the template through the existing `foundry_services` seam (no new
      DB access in the render path).
- [ ] Empty board renders an inviting empty state with a call to action.
- [ ] All previously-green board acceptance scenarios remain green.

### Outcome KPIs
- **Who**: Foundry contributors changing the board surface.
- **Does what**: Make a board markup/wording change touching only a template (no handler/SQL edit).
- **By how much**: 100% of board-only visual changes touch zero files under
  `foundry-store` and zero handler `format!` HTML sites (measured per PR).
- **Measured by**: PR file-path diff inspection on board-visual PRs; CI green on the
  unchanged acceptance suite.
- **Baseline**: 0% today — a board text change touches `projects.rs` (`render_board`).

### Technical Notes
- Solution-neutral: template engine choice is DESIGN. The store-facing path already exists
  (`foundry_services::board::list_board_issues`); reuse it unchanged.
- Existing htmx attributes (`hx-swap-oob`) are MOVED into the template/partial as-is — no
  version bump, no directive rename in this slice (that is US-B05).
- The `data-*` markers are render-contract scraper hooks; keep them stable.

### Size
**M** (2-3 days, 5 scenarios). Touches: board template + issue-card partial, rewiring
`projects::show_board` and the `issues::*` render paths to render via templates.

### Dependencies
US-B06 (the static-asset/template pipeline scaffolding it renders into). Paired with US-B02
in **Slice 1**.

---

## US-B02: Render the board from vendored assets so it looks like a product

- **job_id**: htmx-web-2 (Minimize the chance a self-hoster's first screen feels unstyled)

### Elevator Pitch
- **Before**: Mei opens the board and sees unstyled HTML — `render_board` emits
  `<html><head><title>` with no `<link>` stylesheet and no `<script>`; `static/` is empty.
- **After**: the board loads with real CSS, coherent header chrome, and htmx + Alpine
  behavior — all from assets the binary already ships (`/static/...`, no CDN, no external
  fetch) — so it reads as a finished product even on an air-gapped VM.
- **Decision enabled**: a self-hosting team decides Foundry looks credible enough to keep
  evaluating instead of bouncing on an unstyled screen.

### Problem
`crates/foundry-app/static/` and `templates/` are EMPTY. Today's board renders with no
stylesheet and no vendored JS. A first-time self-hoster who opens the board sees unstyled
HTML, which reads as "unfinished prototype" and undermines the README's "Linear-style
ergonomics" promise before the team ever files an issue.

### Who
- **Mei Chen** / her teammates, seeing Foundry's UI for the first time.
- **Devansh**, the operator who screenshots it for his team and may run it air-gapped.
- **Context**: the Slice-1 board, now template-rendered (US-B01), still visually bare
  without an asset pipeline.
- **Motivation**: a first screen that earns trust, offline.

### Solution
Establish a static-asset pipeline: vendor htmx and Alpine.js and a Foundry stylesheet into
`static/`, served by the binary. The base layout template links them. The board uses the
stylesheet for columns, cards, header, and the create affordance. No CDN, no Node runtime
service; assets are part of the image. (Whether a build-time asset step is used is a DESIGN
open question; a runtime service is a hard non-goal.)

### Domain Examples
#### 1. Happy path — styled board offline
Devansh runs Foundry on an air-gapped VM with no internet egress. Mei opens the board and
it is fully styled and interactive — htmx and Alpine load from `localhost:3000/static/...`,
never from a CDN.
#### 2. Edge case — keyboard-only user
Hiroshi navigates the board with Tab/Enter only. Focus indicators are visible; the `c`
shortcut and all interactive controls are keyboard-reachable (WCAG 2.2 AA operable).
#### 3. Error/boundary case — a stale/incorrect asset path
A typo points the layout at `/static/htmx.js` when the file is `/static/htmx.min.js`. The
asset-resolution check fails in CI, so the broken-asset board never ships.

### UAT Scenarios
```gherkin
Scenario: The board loads with vendored styles and scripts, no external CDN
  Given Foundry is running on a host with no outbound internet access
  When Mei opens the Auth v2 board
  Then the board is visually styled (columns and cards are laid out, not raw HTML)
  And htmx and Alpine are loaded from the application's own static path
  And no request is made to an external origin

Scenario: Static assets are served by the binary
  Given the foundry binary is running
  When a browser requests the vendored stylesheet and scripts under the static path
  Then each returns HTTP 200 with the correct content type

Scenario: The board is keyboard operable with visible focus
  Given Mei navigates the board using only the keyboard
  When she tabs through the interactive controls
  Then every interactive control is reachable
  And the currently focused control shows a visible focus indicator

Scenario: A missing vendored asset fails the build check
  Given a layout template references a static asset path that does not exist
  When the asset-resolution check runs
  Then the check fails and the board is not released in that state
```

### Acceptance Criteria
- [ ] htmx, Alpine, and a Foundry stylesheet are vendored under the static path and served
      by the binary (HTTP 200, correct content type).
- [ ] The board renders styled (columns, cards, header) using the vendored stylesheet.
- [ ] No external-origin request is made to render or operate the board.
- [ ] The board is fully keyboard-operable with visible focus indicators (WCAG 2.2 AA).
- [ ] A referenced-but-missing static asset is caught by an asset-resolution/build check.

### Outcome KPIs
- **Who**: First-time self-hosting teams opening Foundry's board.
- **Does what**: Continue evaluating past the first screen (don't bounce on "looks broken").
- **By how much**: 0 external-origin requests on the board; styled-board check green on a
  no-egress host.
- **Measured by**: Network-request assertion in the acceptance harness (external origin
  count = 0) + visual/asset-resolution checks.
- **Baseline**: today the board is unstyled (empty `static/`), 0% styled, 0 vendored assets.

### Technical Notes
- Solution-neutral on the exact CSS approach and whether a build-time (non-runtime) asset
  step is used — DESIGN. Constraint: no new *runtime* service, no CDN.
- The vendored htmx version is NOT chosen here; htmx 2 version pin is DESIGN (the bump is US-B05).

### Size
**M** (2-3 days, 4 scenarios). Touches: static pipeline, base layout, board stylesheet,
vendored assets, asset-resolution check.

### Dependencies
US-B01 (the board must render from a template before it can be styled coherently) and
US-B06 (the pipeline). Ships together as **Slice 1**.

---

## US-B06: Stand up the template + static-asset pipeline `@infrastructure`

- **job_id**: infrastructure-only
- **infrastructure_rationale**: This story produces no user-observable behavior on its own
  — it is the templating engine wiring + the `static/` serving route that US-B01 and US-B02
  render into. It exists so the value stories have a pipeline to target. Per the slice-level
  rule it is NOT a standalone slice: it ships folded into **Slice 1** alongside US-B01 and
  US-B02 (both user-visible), so the released slice carries value. Without it the board
  cannot be templated or styled; with it alone, nothing the user sees changes.

### Problem
There is no template engine wired into `foundry-app` and no route serving `static/`. Every
value story in this feature needs both. Building the pipeline once, cleanly, prevents each
surface story from re-inventing wiring and keeps the engine choice in one place.

### Who
- **Contributors**, who inherit one templating + asset convention.
- **Context**: `foundry-app`'s router; the empty `templates/` and `static/` dirs.
- **Motivation**: a single, documented place where templates are loaded and assets are served.

### Solution
Wire a template engine (choice = DESIGN) into the app and add a static-file serving route
for `static/` (no CDN, served by the binary). Provide the base layout template that
US-B01/B02/B04 extend. Nothing user-facing changes until a surface story renders through it.

### Domain Examples
#### 1. Happy path — pipeline available
The board template (US-B01) loads through the engine; the stylesheet (US-B02) serves from
`/static/`. Both work because the pipeline exists.
#### 2. Edge case — template not found
A surface references a template that does not exist; the engine fails fast at load with a
clear error rather than rendering a blank page.
#### 3. Error/boundary case — static route does not escape its dir
A request for `/static/../secret` is refused; the static route serves only files under `static/`.

### UAT Scenarios
```gherkin
Scenario: A template renders through the engine
  Given the template pipeline is wired into the app
  When a surface renders a registered template
  Then the rendered HTML is returned with HTTP 200

Scenario: Static files are served only from the static directory
  Given the static-asset route is mounted
  When a request attempts to traverse outside the static directory
  Then the request is refused and no file outside the directory is served

Scenario: A referenced missing template fails fast with a clear error
  Given a surface references a template name that does not exist
  When the page is requested
  Then the failure is reported clearly rather than rendering a blank page
```

### Acceptance Criteria
- [ ] A template engine is wired into `foundry-app`; templates load from `templates/`.
- [ ] A static-asset route serves files from `static/` only (no path traversal), no CDN.
- [ ] A base layout template exists for full pages to extend.
- [ ] The pipeline adds no new runtime service and no DB dependency.

### Outcome KPIs
- **Who**: Contributors building/maintaining web surfaces (enabling metric, not a user outcome).
- **Does what**: Render any surface through one shared pipeline.
- **By how much**: 1 template engine wiring + 1 static route; 0 new runtime services.
- **Measured by**: code inspection; US-B01/B02 render successfully through it.
- **Baseline**: 0 — no engine, no static route today.

### Technical Notes
- Engine choice, asset directory layout, and any build-time step are DESIGN.
- Must satisfy NFR-WEBB-INFRA-01 (no new runtime service) and NFR-WEBB-BND-01 (no DB in web tier).

### Size
**S** (1 day, 3 scenarios). Touches: engine wiring, static route, base layout skeleton.

### Dependencies
None. Folds into **Slice 1** (never standalone). Required by US-B01, US-B02, US-B04.

---

# ==========================================================================
# Slice 2 — Issue & comments read like a product (one card partial)
# US-B03
# ==========================================================================

## US-B03: Move the issue detail and comment thread to templates

- **job_id**: htmx-web-1 (Minimize effort to restyle a screen without touching Rust)

### Elevator Pitch
- **Before**: Mei opens `/team/backend/project/auth-v2/issues/3`; the page and every
  comment card are built by four `format!` sites in `comments.rs` (`render_issue_page`,
  `render_comment_card`, `render_comment_card_oob`, the inline edit-form), and the OOB
  (live) card deliberately OMITS Edit/Delete — so a live-posted card already looks
  different from a reloaded one.
- **After**: the issue page and every comment card render from ONE comment-card partial —
  full-page load, htmx post-append, inline edit, and cancel all show the identical card —
  while Edit/Delete affordances and markdown sanitization stay decided in the handler/core.
- **Decision enabled**: a contributor decides they can restyle the comment thread by
  editing one partial, confident the four old render paths can no longer drift apart.

### Problem
The comment surface is the most tangled in the codebase: `comments.rs` has at least three
`format!` render sites plus an inline edit-form. The OOB card omits the Edit/Delete buttons
"for simplicity" (comments.rs ~line 840), so the live card visibly differs from a reloaded
one. Restyling means editing Rust in four places and risking divergence; authz/sanitization
are interleaved with the markup.

### Who
- **Jamal Okafor**, restyling the comment thread.
- **Mei Chen**, posting/editing/deleting comments; must see no behavioral change.
- **Context**: `comments::show_issue`, `submit_comment`, `submit_edit_comment`,
  `submit_delete_comment`, `show_edit_form`, `show_single_comment`.
- **Motivation**: one place to change comment markup, with the live card matching the reloaded card.

### Solution
A templated issue-page + a single comment-card partial used by every render path.
Authorization affordances (`can_edit` = author; `can_delete` = author or admin) are DECIDED
in the handler/core (`is_workspace_admin` + author check, as today) and passed to the
partial as booleans; the partial only RENDERS them. Markdown sanitization stays in
`foundry_core::render_comment_markdown`. The 400/403/410 error fragments keep their exact
copy. The fix: the OOB (live) card now uses the same partial, so it shows the same
affordances as a reloaded card.

### Domain Examples
#### 1. Happy path — post a comment, live card matches reloaded card
Mei posts "Looked into this — SameSite default change." Hiroshi (viewing) sees the new card
appended via htmx; it shows the same author/body/affordance layout as it does after a full
page reload, because both render the same partial.
#### 2. Edge case — author edits, "(edited)" marker
Mei edits her comment. The inline edit-form fragment and the re-rendered card both come from
templates; the "(edited)" marker appears; Hiroshi sees the update. Non-authors still see no
Edit button (affordance decided in the handler).
#### 3. Error/boundary case — non-author PATCH and deleted-comment 410
Hiroshi POSTs an edit to Mei's comment endpoint -> 403 fragment "You may only edit your own
comments." Editing an already-soft-deleted comment -> 410 fragment "This comment has been
deleted. Refresh to see the latest state." Both strings unchanged.

### UAT Scenarios
```gherkin
Scenario: Issue page and comment thread render from templates
  Given issue AUTH-3 has comments by Mei and Hiroshi
  When Mei opens the AUTH-3 issue page
  Then the issue header and both comment cards render from templates
  And each comment card shows its author and rendered markdown body

Scenario: A live-posted comment card matches a reloaded one
  Given Hiroshi is viewing AUTH-3 while Mei posts a new comment
  When Mei's comment is appended via htmx
  Then the appended card has the same structure and affordances as the same card after a full page reload

Scenario: Edit and delete affordances are gated in the handler, rendered in the template
  Given Mei is the author of a comment and Devansh is a workspace admin
  When the comment thread renders for each of them
  Then Mei sees Edit and Delete on her own comment
  And Devansh sees Delete but not Edit on Mei's comment
  And Hiroshi (neither author nor admin) sees neither on Mei's comment

Scenario: Non-author edit is refused with the unchanged message
  Given Hiroshi is not the author of Mei's comment
  When Hiroshi submits an edit to that comment
  Then he receives a 403 with the message "You may only edit your own comments."

Scenario: Editing a deleted comment returns the unchanged gone message
  Given a comment has been soft-deleted
  When an edit is submitted for it
  Then the response is 410 stating the comment has been deleted and to refresh

Scenario: Markdown sanitization remains in core
  Given Mei submits a comment containing a "javascript:" link and a script tag
  When the comment renders
  Then the dangerous URL and script are removed
  And the sanitization is performed by core before the template renders the body
```

### Acceptance Criteria
- [ ] Issue page and all comment render paths use one comment-card partial.
- [ ] The live (htmx-appended) card and the reloaded card are structurally identical,
      including affordances (fixing today's OOB-omits-buttons divergence).
- [ ] Edit/Delete affordances are decided in the handler/core and passed to the partial as
      flags; the partial contains no authorization logic.
- [ ] Markdown sanitization stays in `foundry_core::render_comment_markdown`; the template
      never sanitizes.
- [ ] 400/403/410 error fragments keep their exact existing copy.
- [ ] All previously-green comment/issue acceptance scenarios stay green.

### Outcome KPIs
- **Who**: Contributors changing the comment-thread presentation.
- **Does what**: Change comment markup in one partial instead of multiple Rust sites.
- **By how much**: comment-render `format!` sites reduced from ≥3 to 1 partial; 0 authz
  logic in the template; live-vs-reloaded card divergence eliminated.
- **Measured by**: code inspection (count of comment-render sites; the live-vs-reloaded
  structural-equality scenario) + acceptance suite green.
- **Baseline**: ≥3 `format!` comment-render sites today; OOB card omits affordances.

### Technical Notes
- The OOB-card-omits-buttons quirk is RESOLVED by sharing the partial (affordances come
  from the same flags), removing the live-vs-reloaded divergence — a real UX improvement.
- Sanitization staying in core is also a carried NFR (NFR-WEBB-BND-03).
- Existing htmx directives (`hx-patch`/`hx-get`/`hx-target`/`hx-swap`/`hx-delete`,
  `hx-swap-oob`) move into the partial as-is; the version bump is US-B05.

### Size
**M** (3 days, 6 scenarios). Touches: issue-page template, comment-card partial, edit-form
fragment, rewiring six handlers' render paths.

### Dependencies
US-B01 (the template pipeline, base layout, and card-partial pattern). **Slice 2.**

---

# ==========================================================================
# Slice 3 — First impression: sign-in looks trustworthy
# US-B04
# ==========================================================================

## US-B04: Move sign-in and forgot-password to templates

- **job_id**: htmx-web-2 (Minimize the chance a self-hoster's first screen feels unstyled)

### Elevator Pitch
- **Before**: Mei visits `/sign-in`; `signin.rs::render_signin_form` emits a bare
  `<html><head><title>Sign in to Foundry</title></head>` page with no stylesheet — an
  unstyled login that reads as unfinished/insecure.
- **After**: she sees a styled, full-page sign-in rendered from the shared base layout —
  labels above inputs, a clear primary button, a "Forgot your password?" link — that posts
  to the same endpoint and sets the same 30-day session cookie.
- **Decision enabled**: a returning user (and a first-time evaluator) decides Foundry's
  auth screens look as trustworthy as the rest of the product.

### Problem
`signin.rs` renders sign-in and forgot-password as full-page `format!` HTML with no shared
layout and no CSS. As the only full-page (non-fragment) surfaces, they are where an
evaluator first lands; an unstyled login reads as insecure/unfinished. They also duplicate
head boilerplate the board template now has, risking visual inconsistency (Nielsen #4).

### Who
- **Mei Chen**, returning member signing in.
- **A first-time evaluator** landing on `/sign-in`.
- **Context**: `signin::show_form`, `show_forgot_form`, `dashboard_root`.
- **Motivation**: a consistent, trustworthy auth screen reusing the base layout.

### Solution
Move sign-in and forgot-password to templates that extend the shared base layout (same
head, vendored assets, header). The POST handlers, CSRF contract (hidden `_csrf` field,
cookie set on GET via `ensure_csrf_cookie`), session-cookie attributes, brute-force delay,
and the non-enumerable `GENERIC_SIGNIN_ERROR` ("Invalid email or password") are all
unchanged — only the markup moves.

### Domain Examples
#### 1. Happy path — styled sign-in, same cookie
Mei visits `/sign-in`, sees a styled card (labels above inputs, one-column form), enters
her credentials, and lands on the dashboard with the same HttpOnly Secure SameSite=Lax
30-day cookie as before.
#### 2. Edge case — wrong password, non-enumerable
Hiroshi mistypes. The template shows "Invalid email or password" — the same message whether
the email exists or not. The error renders inline in the styled form, not as bare text.
#### 3. Error/boundary case — CSRF cookie absent on GET
A fresh browser hits `/sign-in` with no `foundry_csrf` cookie. The GET sets the cookie and
the template renders the matching hidden `_csrf` field; the subsequent POST validates.

### UAT Scenarios
```gherkin
Scenario: Sign-in renders from the shared layout and signs the user in
  Given Mei has a member account and no active session
  When Mei opens the sign-in page and submits valid credentials
  Then the sign-in page is rendered from the shared base layout
  And Mei lands on the dashboard
  And her browser holds an HttpOnly Secure SameSite=Lax session cookie valid for 30 days

Scenario: Invalid credentials show the unchanged non-enumerable error in the styled form
  Given Hiroshi submits an email that is not registered
  When the sign-in form re-renders
  Then it displays "Invalid email or password"
  And the same message is shown for a registered email with a wrong password

Scenario: Forgot-password page renders from the shared layout
  Given SMTP is configured
  When Mei opens the forgot-password page and submits her email
  Then the page is rendered from the shared base layout
  And the response states a reset link has been sent if the email is on file

Scenario: CSRF token contract is preserved on the templated form
  Given a browser with no CSRF cookie opens the sign-in page
  When the page renders
  Then a CSRF cookie is set
  And the form carries a matching hidden CSRF field
  And a POST without a valid CSRF token is rejected
```

### Acceptance Criteria
- [ ] Sign-in and forgot-password render from templates extending the shared base layout.
- [ ] Session-cookie attributes (HttpOnly, Secure, SameSite=Lax, 30-day) are unchanged.
- [ ] The non-enumerable "Invalid email or password" copy is unchanged and non-enumerable.
- [ ] The CSRF contract (cookie set on GET, hidden `_csrf` field, 403 on missing/invalid)
      is unchanged.
- [ ] All previously-green sign-in/forgot-password acceptance scenarios stay green.

### Outcome KPIs
- **Who**: Returning users and first-time evaluators on the auth screens.
- **Does what**: Encounter a styled, consistent auth screen (same look as the board).
- **By how much**: 100% of full-page auth screens extend the one shared layout (0
  duplicated head/asset boilerplate).
- **Measured by**: code inspection (auth templates extend base layout; 0 inline `<head>`
  duplication) + acceptance suite green.
- **Baseline**: sign-in is standalone `format!` HTML with no shared layout, no CSS today.

### Technical Notes
- Sign-in/forgot are full-page surfaces (no htmx fragment swap), so this is the
  lowest-fragment-risk extraction — hence sequenced after the fragment-heavy surfaces.
- The brute-force artificial delay (backend-mvp NFR-SEC-02, `BRUTE_FORCE_*`) is server-side
  and untouched.

### Size
**S-M** (2 days, 4 scenarios). Touches: base-layout extension, sign-in + forgot templates,
rewiring `signin::show_form`/`show_forgot_form` (and optionally `dashboard_root`).

### Dependencies
US-B01/US-B06 (base layout + static pipeline). **Slice 3.**

---

# ==========================================================================
# Slice 4 — htmx is consistent and ready to upgrade to 2
# US-B05
# ==========================================================================

## US-B05: Normalize htmx directives and upgrade to a pinned htmx 2

- **job_id**: htmx-web-3 (Normalize htmx attributes so an htmx-1->2 upgrade is low-risk)

### Elevator Pitch
- **Before**: htmx behavior is wired through bare `hx-*` attributes emitted ad-hoc per
  handler (`hx-patch`/`hx-get`/`hx-target`/`hx-swap`/`hx-delete` in the comment edit-form;
  `hx-swap-oob` in the create card and comment OOB), and htmx is not vendored or pinned at
  all (`static/` was empty before Slice 1).
- **After**: every htmx directive uses one consistent convention across the templates, htmx
  is vendored at a single pinned 2.x file in `static/`, and every existing hx-driven
  interaction (create-card OOB swap, comment edit/delete/cancel, SSE fragment) still works —
  with the passive `data-*` scraper markers left byte-untouched.
- **Decision enabled**: a maintainer decides a future htmx upgrade is "swap the vendored
  file, run the suite" rather than an archaeology project.

### Problem
The deferred htmx-1->2 migration (web-tier-extraction D3) needs a home. Today the active
directives are scattered as per-handler strings and the version is unpinned. An upgrade now
would mean hunting directives and choosing a version blind. After Slices 1-3 the directives
live in a few partials — the right moment to normalize them and pin a 2.x version in one
controlled change. CRITICAL nuance validated from code: `data-hx-fragment`/`data-column`/
`data-comment-list`/`data-issue-key` are SCRAPER markers the suite asserts on, NOT htmx
directives — they must NOT be renamed during normalization.

### Who
- **Jamal/maintainer**, performing the normalization + version bump.
- **Mei/Hiroshi**, whose hx-driven interactions must keep working identically.
- **Context**: the templated partials from Slices 1-3; the vendored htmx file in `static/`.
- **Motivation**: a consistent, version-pinned htmx that upgrades cleanly.

### Solution
Normalize the active htmx directives to one consistent convention across all templates;
vendor a single pinned htmx 2.x file (version pin = DESIGN) in `static/`; add/keep a
regression scenario for every hx-driven interaction so the bump is provably non-regressing.
Leave the `data-*` render-contract markers exactly as they are.

### Domain Examples
#### 1. Happy path — consistent directives, one vendored version
After normalization, all templates use the same htmx attribute convention; `static/` holds
exactly one pinned htmx 2.x file linked from the base layout.
#### 2. Edge case — create-card OOB swap after the bump
Mei files an issue; the `hx-swap-oob` create-card swap into the Backlog column still works
under htmx 2; the card appears exactly as before.
#### 3. Error/boundary case — a data-* marker is NOT an htmx directive
A normalization pass must not rename `data-hx-fragment` to an htmx directive; the
render-contract test reds if a `data-*` scraper marker is altered.

### UAT Scenarios
```gherkin
Scenario: htmx directives use one consistent convention across templates
  Given all web surfaces render from templates
  When the htmx directives are reviewed across the templates
  Then they use a single consistent attribute convention

Scenario: htmx is vendored at a single pinned version
  Given the static asset directory
  When the vendored htmx file is inspected
  Then there is exactly one htmx file and its version is recorded

Scenario: Every existing htmx-driven interaction still works after the upgrade
  Given htmx has been upgraded to the pinned 2.x version
  When a user files an issue, posts a comment, edits a comment, deletes a comment, and cancels an edit
  Then each htmx-driven swap behaves exactly as before the upgrade

Scenario: The render-contract data markers are left untouched
  Given the templates carry data-* scraper markers used by the acceptance suite
  When the htmx normalization is applied
  Then every data-* marker (data-hx-fragment, data-column, data-comment-list, data-issue-key) is unchanged
```

### Acceptance Criteria
- [ ] All active htmx directives across templates use one consistent convention.
- [ ] htmx is vendored at exactly one pinned 2.x version under the static path.
- [ ] Every existing hx-driven interaction (create-card OOB, comment edit/delete/cancel,
      SSE fragment) has a green regression scenario after the upgrade.
- [ ] The `data-*` render-contract markers are byte-unchanged.
- [ ] The full acceptance suite stays green after the upgrade.

### Outcome KPIs
- **Who**: Maintainers performing/maintaining the htmx version.
- **Does what**: Upgrade htmx by swapping one vendored file, with consistent directives.
- **By how much**: 1 pinned vendored htmx version (was 0/unpinned); 1 consistent directive
  convention (was per-handler ad-hoc); 100% of hx-driven interactions regression-covered.
- **Measured by**: code inspection (one htmx file, version recorded; consistent directives)
  + acceptance suite green after the bump.
- **Baseline**: htmx unvendored/unpinned, directives emitted ad-hoc per handler today.

### Technical Notes
- The specific htmx 2.x version pin is DESIGN (carried from web-tier-extraction D3).
- Slices 1-3 move existing directives into templates AS-IS; this slice is the ONLY one that
  changes directive convention or htmx version (keeps the bump atomic and regression-tested).
- `data-*` markers are render-contract, not htmx; normalization must not touch them.

### Size
**S-M** (2 days, 4 scenarios). Touches: directive normalization across the partials,
vendoring + pinning one htmx 2.x file, regression scenarios per interaction.

### Dependencies
US-B01, US-B03, US-B04 (all surfaces templated so directives are centralized). **Slice 4.**
