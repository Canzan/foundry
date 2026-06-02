# htmx Web Tier (Feature B) — UX Journeys

> Feature B of the web-tier-extraction split — "Foundry looks like a product."
> Feature A (the JSON API + the presentation-neutral `foundry_services` seam) has
> SHIPPED. These journeys are the SECONDARY web track from that split, now the
> PRIMARY work of this feature. Each journey: step-by-step flow, emotional arc,
> UI mockup per step, shared artifacts that thread through, failure/recovery
> paths, and the UAT anchors from `stories.md`.

Platform: **web** (server-rendered HTML + htmx fragments + Alpine.js). Key Nielsen
heuristics: #1 visibility of status, #4 consistency, #7 flexibility (keyboard), #9
help-with-errors. Accessibility target: **WCAG 2.2 AA**. Material honesty: this is a
server-rendered htmx app, NOT a SPA — the "best" UI is fast, styled, keyboard-driven
HTML, not a client framework.

> **Refactor rule (applies to every journey here).** For an existing user (Mei,
> Jamal) the *best* emotional outcome of templating is **no perceptible behavioral
> change** — the only allowed positive delta is "the screen now looks styled
> instead of unstyled." Any new jank is a regression. The acceptance suite's
> asserted substrings are the render contract; templates honor them byte-for-byte.

---

## Journey 1 — Contributor restyles/re-words a screen (PRIMARY; jtbd htmx-web-1)

**Persona**: Jamal Okafor, the contributor. A maintainer asks "can you make the
empty board friendlier and tighten the card spacing?" — a pure visual change.

**Trigger**: A visual/wording change request lands on a Foundry surface.

**Goal**: Ship a visual change touching ONLY a template + stylesheet, confident he
changed no behavior, in minutes.

**Success criterion**: Grepping the on-screen text lands in `templates/`, not in
`projects.rs`/`comments.rs`/`signin.rs`; the change is a template/CSS diff; the
acceptance suite stays green; no handler or SQL file is touched.

### Emotional Arc (Problem Relief)

| Phase | Emotion | Confidence | Design lever |
|-------|---------|-----------|--------------|
| Before (today) | Dread — "the 'No issues yet' text is a string literal in `render_board` in projects.rs, right next to `column_label_to_state` and a board query" | 25% | (the problem we fix) |
| Opens `templates/board.html` | "Oh — it's just here" | 78% | Markup lives in a predictable templates/ location |
| Edits template + CSS, reloads | Flow | 88% | Edit -> refresh -> see it (README hot-reload loop) |
| Opens PR; reviewer sees an HTML/CSS diff | Pride | 92% | Diff is markup, not escaped Rust strings |

### Step-by-Step Flow

```
[Trigger: "make the empty board friendlier"]
        |
        v
[Step 1: grep the on-screen text]   Before: hits projects.rs (render_board, mixed).
        |                           After: hits templates/board.html.   $TEMPLATE_FILES
        v
[Step 2: edit templates/board.html + the stylesheet]   Feels: "only touching markup"
        |   Guarantee (Feature A's boundary guard, carried as constraint): the web
        |   tier holds no DB pool, so a template edit cannot break persistence.
        v
[Step 3: reload (cargo watch / refresh)]   Feels: flow
        v
[Step 4: open PR — HTML/CSS diff]   Feels: confident; reviewer sees markup as markup
        |   Acceptance suite green => behavior provably unchanged.
        v
[Done — visual change shipped without reading handler logic]
```

### Step Mockup — the empty-board state, before vs after

Today (`render_board`, projects.rs) emits, per empty column:
`<p class="empty">No issues yet</p>` with NO stylesheet.

After (templated + styled):

```
+-- Acme Eng · Backend · Sandbox --------------------------- [Mei v] -+
| [Backlog 0]       [Todo 0]        [In-Progress 0]      [Done 0]     |
| +----------------------------------------------------------------+  |
| |                                                                |  |
| |              No issues yet — press  c  to file the first one.   |  |
| |                                                                |  |
| +----------------------------------------------------------------+  |
|                                                          [ + c ]    |
+--------------------------------------------------------------------+
```

> The empty-state wording is the editable bit. The column labels ("Backlog",
> "Todo", "In-Progress", "Done") and `data-column` markers are the render contract
> and stay verbatim.

### Shared Artifacts (Journey 1)

| Artifact | Source of truth | Consumers | Risk | Validation |
|----------|-----------------|-----------|------|-----------|
| `$TEMPLATE_FILES` | `crates/foundry-app/templates/*` (NEW) | every rendered web surface | MEDIUM — one place for markup | grep on-screen text lands in templates/, not handlers (NFR-WEBB-MAINT-01) |
| `$RENDER_CONTRACT` | the acceptance suite's asserted substrings | board/issue/sign-in templates | HIGH — a whitespace/markup change can red the suite | unchanged acceptance scenarios stay green (NFR-WEBB-COMPAT-02) |
| `$WEB_NO_POOL` | Feature A's boundary guard (CI) | the web tier | HIGH — what makes Jamal confident | web crate gains no DB-pool dependency (carried constraint, NFR-WEBB-BND-01) |

### Failure / Recovery Modes (Journey 1)

| Step | Failure | Recovery |
|------|---------|----------|
| 2 | Jamal accidentally edits handler logic | The acceptance suite catches behavior changes in CI; Feature A's boundary guard catches a DB reach |
| 3 | Template syntax error | foundry-web fails to render the affected page with a clear error; other pages unaffected |
| 4 | A whitespace change reds an asserted substring | The render-contract test names the changed substring; Jamal restores the asserted text |

### UAT Anchors (Journey 1)

- Step 1-2 -> `A board wording change touches only a template` (US-B01)
- Step 1-2 -> `Comment markup changes in one partial` (US-B03)
- Guarantee -> the web tier holds no DB pool (carried from Feature A; NFR-WEBB-BND-01)

---

## Journey 2 — Self-hoster's first styled screen, offline (PRIMARY; jtbd htmx-web-2)

**Persona**: Mei Chen (and her teammates / Devansh the operator) opening Foundry's
board in a browser for the first time, possibly on an air-gapped VM.

**Trigger**: Mei navigates to `/team/backend/project/auth-v2` on a fresh install.

**Goal**: See a styled, fast, interactive board that reads as a finished product —
served entirely from assets the binary ships, with no external fetch.

**Success criterion**: The board is visually styled (columns/cards laid out, header
chrome), htmx + Alpine + CSS load from `localhost:3000/static/...` (zero external
origins), the board is keyboard-operable with visible focus, and render latency stays
in the existing budget.

### Emotional Arc (Confidence Building / First-impression trust)

| Phase | Emotion | Confidence | Design lever |
|-------|---------|-----------|--------------|
| Board first paint (today) | "...is this broken? It's raw HTML" | 30% | (the problem we fix — `static/` is empty) |
| Board first paint (after) | "This looks like a real product" | 72% | Vendored CSS + coherent layout from a template |
| Presses `c` | Muscle memory / trust | 85% | Create affordance works; htmx fragment unchanged |
| Files an issue | Flow | 90% | Same hot path (`issue_service::create_issue`); card appears in Backlog |
| Sees a teammate's change appear | "It's live, like Linear" | 92% | SSE fragment swap, unchanged from backend-mvp |
| Realizes it worked with no internet | Trust + relief | 95% | All assets vendored; 0 external origins (air-gap friendly) |

### Step-by-Step Flow

```
[Trigger: open /team/backend/project/auth-v2 on a fresh / air-gapped install]
        |
        v
[Step 1: Board page loads, STYLED]   Feels: "looks real now"
        |   Served by: foundry-web template <- data via foundry_services (Feature A seam)
        |   Assets: $STATIC_ASSET_PATHS (vendored htmx + Alpine + CSS), 0 external origins
        v
[Step 2: Press 'c']   Feels: muscle memory
        |   htmx GET issue-create fragment (template-rendered)
        v
[Step 3: Type title, submit]   Feels: flow
        |   htmx POST -> issue_service::create_issue -> card fragment ($ISSUE_CARD_MARKUP)
        |   Shared artifact: $CSRF_TOKEN (HX-CSRF header + foundry_csrf cookie)
        v
[Step 4: Card appears in Backlog]   Feels: confirmed
        |   hx-swap-oob into [data-column='backlog']  (render contract preserved)
        v
[Step 5: Teammate's change streams in]   Feels: "it's live"
        |   SSE fragment swap; same card partial as full render (no divergence)
        v
[Done — board feels identical to before, but now styled, fully offline]
```

### Step Mockup — Step 1 (styled board, the visible win)

```
+-- Acme Eng · Backend · Auth v2 --------------------------- [Mei v] -+
| [Backlog 2]        [Todo 1]          [In-Progress 1]      [Done 0]  |
| +----------------+ +----------------+ +-----------------+           |
| | AUTH-7         | | AUTH-3         | | AUTH-2          |           |
| | Refresh token  | | Verify magic   | | OIDC discovery  |           |
| | broken on Saf… | | link expiry    | | stub            |           |
| +----------------+ +----------------+ +-----------------+           |
| | AUTH-6         |                                                  |
| +----------------+                                                  |
|                                                          [ + c ]    |
+--------------------------------------------------------------------+
   ^ assets: /static/htmx.min.js  /static/alpine.min.js  /static/foundry.css
     all served by the binary — 0 requests to any external origin.
```

The asserted substrings from today's suite ("Backlog", the issue key, `data-column`,
`data-issue-key`) are preserved verbatim inside the new template — the render contract.

### Step Mockup — Step 3 (issue-create fragment, now template-rendered)

```
+-- New issue in Auth v2 ------------------------------- [Esc] -+
| Title  [ Refresh token rotation broken on Safari_         ]  |
|                              [Cancel]   [Create]            |
+--------------------------------------------------------------+
```

> Note: today's `render_issue_card` is intentionally minimal (key + title only). The
> create fragment chrome above is the *styled* target; the card markup contract
> (`data-issue-key`, the key text) is preserved.

### Shared Artifacts (Journey 2)

| Artifact | Source of truth | Consumers | Risk | Validation |
|----------|-----------------|-----------|------|-----------|
| `$STATIC_ASSET_PATHS` | NEW `foundry-web` static pipeline (`static/`): vendored htmx + Alpine + CSS | `<script>`/`<link>` tags in the base layout | HIGH — wrong path = unstyled/broken JS; external path = breaks air-gap | assets served by the binary, HTTP 200, 0 external origins (NFR-WEBB-PERF-03) |
| `$BOARD_COLUMNS` | `foundry-core`/`projects.rs` fixed list (Backlog/Todo/In-Progress/Done) | board template, create fragment, state-change fragment | HIGH — labels are asserted in the suite | same labels in every consumer (NFR-WEBB-COMPAT-02) |
| `$ISSUE_CARD_MARKUP` | NEW `foundry-web` issue-card partial | full board render, htmx create fragment (`hx-swap-oob`), SSE fragment | HIGH — ONE partial across three paths or live cards differ from reloaded cards | one partial, three call sites; snapshot test (NFR-WEBB-MAINT-02) |
| `$CSRF_TOKEN` | `csrf.rs` (`foundry_csrf` cookie + `_csrf` field / `HX-CSRF` header) | every mutating form/htmx call | CRITICAL — templating must keep the cookie + header contract | POST without token -> 403 (unchanged, NFR-WEBB-COMPAT-03) |
| `$ISSUE_KEY` | Postgres sequence (core/store) via `foundry_services` | card, board, URL | HIGH — sequential per project | unchanged from backend-mvp |

### Failure / Recovery Modes (Journey 2)

| Step | Failure | UX response (Nielsen #9) |
|------|---------|--------------------------|
| 1 | Static asset 404 (bad vendored path) | Page still renders HTML (graceful degradation), but this is a release blocker caught by the asset-resolution check (US-B02/US-B06) |
| 1 | An asset path points at an external origin | Air-gap test fails in CI; the broken board never ships |
| 1 | Template render error | foundry-web returns 500 with request_id; never a half-rendered page |
| 3 | Empty title | Inline "Title is required"; fragment swap; focus returns to title (unchanged contract from issues.rs) |
| 3 | Missing/blank CSRF token | 403 before the handler; htmx surfaces a generic "Please refresh and retry" |
| 5 | SSE drops | EventSource auto-reconnect (unchanged backend-mvp behavior) |

### UAT Anchors (Journey 2)

- Step 1 -> `The board loads with vendored styles and scripts, no external CDN` (US-B02)
- Step 1 -> `Static assets are served by the binary` (US-B02)
- Step 1 -> `The board is keyboard operable with visible focus` (US-B02)
- Step 1 -> `Board renders through a template with the same content` (US-B01)
- Step 3-4 -> `Filing an issue still returns the same card fragment` (US-B01)

---

## Journey 3 — Member reads an issue and comments (jtbd htmx-web-1; Slice 2 surface)

**Persona**: Mei Chen, viewing AUTH-3 with 2 existing comments by her and Hiroshi.

**Goal**: Read the thread, post a markdown comment, edit/delete her own — all
unchanged in feel, now rendered from ONE templated comment-card partial instead of the
four `format!` render sites in `comments.rs` (`render_issue_page`,
`render_comment_card`, `render_comment_card_oob`, plus the inline edit-form fragment).

### Emotional Arc (Confidence Building)

| Phase | Emotion | Confidence | Design lever |
|-------|---------|-----------|--------------|
| Issue page loads | "Readable, scannable" | 75% | Template layout; comment cards visually distinct |
| Posts comment | Satisfied | 88% | htmx fragment append, unchanged contract |
| Edits her comment | In control (Nielsen #3) | 90% | Inline edit form fragment; "(edited)" marker preserved |
| The live-appended card looks identical to a reloaded one | Reassured | 92% | ONE partial fixes the current OOB-omits-buttons divergence |
| Sees a 410/403 on a deleted/foreign comment | Informed, not blamed | 85% | Same terse copy as today |

> Validated quirk worth fixing: today `render_comment_card_oob` (comments.rs line ~828)
> deliberately OMITS the Edit/Delete buttons "for simplicity", so a live-posted card
> ALREADY looks different from a reloaded one. Collapsing to one partial removes that
> divergence — a genuine (small) UX improvement, not just a refactor.

### Step Mockup — Issue detail (template-rendered)

```
+-- AUTH-3 · Verify magic link expiry ----------------------- [Mei v] -+
| Attachments: none                                                    |
| -------------------------------------------------------------------- |
| Comments                                                             |
| +----------------------------------------------------------------+   |
| | Mei  (edited)                                                  |   |
| |  Looked into this — root cause is the SameSite default change. |   |
| |  [Edit] [Delete]                                               |   |
| +----------------------------------------------------------------+   |
| | Hiroshi                                                        |   |
| |  Confirmed on Safari 17.                                       |   |
| +----------------------------------------------------------------+   |
| Add a comment                                                        |
| [ **markdown** supported …                                       ]   |
|                                                       [ Post ]       |
+----------------------------------------------------------------------+
```

### Shared Artifacts (Journey 3)

| Artifact | Source of truth | Consumers | Risk | Validation |
|----------|-----------------|-----------|------|-----------|
| `$COMMENT_CARD_MARKUP` | NEW `foundry-web` comment-card partial | full issue render (`render_issue_page`), POST-comment OOB fragment, PATCH-edit re-render, GET single-card (cancel) | HIGH — four render paths today must collapse to one partial or diverge | one partial across all paths; the live-vs-reloaded structural-equality scenario (US-B03) |
| `$SANITIZED_HTML` | `foundry_core::render_comment_markdown` (ammonia) — already in core | comment body in every card | CRITICAL — sanitization MUST stay in core, NOT move to the template | render `[x](javascript:…)` -> href removed (carried NFR, NFR-WEBB-BND-03) |
| `$AUTHZ_AFFORDANCES` | core/store: `is_workspace_admin` + author check (decided in comments.rs handler, passed as flags) | which of Edit/Delete the card partial emits | HIGH — affordance gating is a DECISION (handler/core), button rendering is PRESENTATION (template) | non-author sees no Edit; admin sees Delete; gated server-side, unchanged (NFR-WEBB-BND-03) |
| `$CSRF_TOKEN` | `csrf.rs` (`_csrf` field for PATCH, `HX-CSRF` header for DELETE) | edit form, delete button | CRITICAL | unchanged contract (NFR-WEBB-COMPAT-03) |
| `$ERROR_FRAGMENTS` | comments.rs constants: 400 "Title/comment" copy, 403 "You may only edit your own comments.", 410 "This comment has been deleted. Refresh to see the latest state." | error swaps | HIGH — exact copy is asserted | substring match stays green (NFR-WEBB-COMPAT-02) |

### Failure / Recovery Modes (Journey 3)

| Step | Failure | UX response |
|------|---------|-------------|
| Post | Empty/whitespace body | 400 inline fragment ("Title is required"-style copy from `bad_request_fragment`) — unchanged |
| Edit | Non-author attempts edit | 403 fragment "You may only edit your own comments." — unchanged (comments.rs `forbidden_fragment`) |
| Edit/Delete | Comment already soft-deleted | 410 fragment "This comment has been deleted. Refresh to see the latest state." — unchanged (`gone_fragment`) |
| Any | Markdown contains script/js: URL | Sanitized by `foundry_core::render_comment_markdown` before the template sees it — defense stays in core |

### UAT Anchors (Journey 3)

- Issue render -> `Issue page and comment thread render from templates` (US-B03)
- Live vs reload -> `A live-posted comment card matches a reloaded one` (US-B03)
- Affordances -> `Edit and delete affordances are gated in core, rendered in the template` (US-B03)
- Errors -> `Non-author edit is refused with the unchanged message` / `Editing a deleted comment returns the unchanged gone message` (US-B03)

---

## Journey 4 — Self-hoster signs in (jtbd htmx-web-2; Slice 3 surface)

**Persona**: Mei returning the next day; full-page (non-fragment) HTML. Also a
first-time evaluator who lands on `/sign-in`.

**Goal**: A styled sign-in page that looks trustworthy, posts to the same endpoint,
sets the same 30-day session cookie, preserves the non-enumerable error.

### Emotional Arc (First-impression trust)

| Phase | Emotion | Confidence | Design lever |
|-------|---------|-----------|--------------|
| Sign-in page loads (today) | "...looks unfinished / unsafe" | 35% | (problem — bare `render_signin_form`, no CSS) |
| Sign-in page loads (after) | "Looks legit" | 72% | Template + CSS; centered card, labels above inputs |
| Wrong password | Not blamed | 75% | Same non-enumerable "Invalid email or password" (`GENERIC_SIGNIN_ERROR`) |
| Signed in | Relief | 95% | Same 30-day HttpOnly Secure SameSite=Lax cookie; lands on dashboard |

### Step Mockup — Sign-in (template-rendered, full page, extends base layout)

```
+----------------------------------------------------+
|                     FOUNDRY                        |
|            Sign in to Acme Eng                     |
|                                                    |
|   Email     [ mei@acme.com                     ]   |
|   Password  [ ••••••••••••                     ]   |
|                                                    |
|                           [   Sign in   ]          |
|   Forgot your password?                            |
+----------------------------------------------------+
```

### Shared Artifacts (Journey 4)

| Artifact | Source of truth | Consumers | Risk | Validation |
|----------|-----------------|-----------|------|-----------|
| `$LAYOUT_TEMPLATE` | NEW `foundry-web` base layout (head, vendored asset links, title) | sign-in, forgot-password, board, issue, dashboard | MEDIUM — one base layout = consistency (Nielsen #4) | all full pages extend it; 0 duplicated `<head>` (NFR-WEBB-MAINT-01) |
| `$CSRF_TOKEN` | `csrf.rs` (`_csrf` hidden field, cookie set on GET via `ensure_csrf_cookie`) | sign-in form, forgot-password form | CRITICAL | unchanged (NFR-WEBB-COMPAT-03) |
| `$SESSION_COOKIE` | tower-sessions Postgres store | set on POST success (signin.rs `submit_signin`) | CRITICAL — templating must not change cookie attrs (HttpOnly, Secure, SameSite=Lax, 30d) | inspect Set-Cookie (NFR-WEBB-COMPAT-04) |
| `$GENERIC_SIGNIN_ERROR` | signin.rs constant "Invalid email or password" | error render on both wrong-email and wrong-password | HIGH — non-enumeration must survive the template move | same string both cases (NFR-WEBB-COMPAT-05) |

### Failure / Recovery Modes (Journey 4)

| Step | Failure | UX response |
|------|---------|-------------|
| Submit | Wrong credentials | "Invalid email or password" (non-enumerable) — unchanged |
| Submit | 6th failure within 15-min window | 5s artificial delay (backend-mvp NFR-SEC-02, `BRUTE_FORCE_*`) — unchanged, server-side |
| GET | CSRF cookie absent | Set on render (`ensure_csrf_cookie`); form carries matching `_csrf` — unchanged |

### UAT Anchors (Journey 4)

- Render -> `Sign-in renders from the shared layout and signs the user in` (US-B04)
- Error -> `Invalid credentials show the unchanged non-enumerable error in the styled form` (US-B04)
- CSRF/cookie -> `CSRF token contract is preserved on the templated form` (US-B04)

---

## Journey 5 — Maintainer upgrades htmx 1 -> 2 (jtbd htmx-web-3; Slice 4)

**Persona**: Jamal/maintainer performing the deferred htmx-1->2 normalization + version bump.

**Trigger**: DESIGN picks an htmx 2 version; the directives must already be consistent
and the version vendored/pinned.

**Goal**: Swap the vendored htmx file and run the suite — no archaeology, no regressions.

### Emotional Arc (Problem Relief)

| Phase | Emotion | Confidence | Design lever |
|-------|---------|-----------|--------------|
| Before normalization | "which prefix did each handler use? is the version even pinned?" | 40% | (the fragility) |
| After: directives consistent + version pinned in `static/` | "one file to swap" | 85% | One vendored, pinned htmx; one attribute convention |
| Run suite after bump | Calm | 92% | Every hx-driven interaction has a green regression scenario |

### Step-by-Step Flow

```
[Trigger: DESIGN selects an htmx 2 version]
        |
        v
[Step 1: directives already normalized in templates]   $HTMX_DIRECTIVES consistent
        |   (note: data-* render-contract markers are NOT touched — they are
        |    scraper hooks, not htmx directives)
        v
[Step 2: swap the single vendored htmx file in static/]   $HTMX_VENDORED_VERSION
        v
[Step 3: run foundry-acceptance]   every hx-driven swap (create OOB, comment
        |                          edit/delete/cancel, SSE) has a green scenario
        v
[Done — htmx 2 in, no regression]
```

### Shared Artifacts (Journey 5)

| Artifact | Source of truth | Consumers | Risk | Validation |
|----------|-----------------|-----------|------|-----------|
| `$HTMX_DIRECTIVES` | the normalized templates | every htmx-driven swap (create card OOB, comment edit/delete/cancel, SSE fragment) | MEDIUM — a normalization typo silently breaks a swap | each hx-driven interaction has a green acceptance scenario (NFR-WEBB-COMPAT-01) |
| `$HTMX_VENDORED_VERSION` | a single pinned file in `static/` | the layout `<script>` tag | MEDIUM — version drift / dual copies | exactly one vendored htmx file; version recorded (NFR-WEBB-INFRA-01) |
| `$DATA_MARKERS` | the render-contract `data-*` attributes (data-hx-fragment, data-comment-list, data-column, data-issue-key) | the acceptance suite (NOT htmx) | HIGH — must NOT be confused with htmx directives during normalization | normalization leaves data-* markers byte-stable (NFR-WEBB-COMPAT-02) |

### Failure / Recovery Modes (Journey 5)

| Step | Failure | Recovery |
|------|---------|----------|
| 1 | Normalization touches a `data-hx-fragment` scraper marker thinking it is an htmx directive | render-contract test reds; restore the marker |
| 2/3 | htmx 2 changes a default that breaks an existing swap | the per-interaction regression scenario reds before release; pin held until fixed |

### UAT Anchors (Journey 5)

- Step 1 -> `htmx directives use one consistent convention across templates` (US-B05)
- Step 2 -> `htmx is vendored at a single pinned version` (US-B05)
- Step 3 -> `Every existing htmx-driven interaction still works after the upgrade` (US-B05)

---

## Cross-Journey Vocabulary & Boundary Map (carried from Feature A as constraints)

Feature B keeps URL/label vocabulary identical and renders within the boundary Feature
A already established (this is a CONSTRAINT, not a Feature-B job — see jobs.yaml
retired_jobs):

| Concern | Lives in | Must NOT do |
|---------|----------|-------------|
| HTML render (templates, partials, static assets) | `foundry-web` (the templating this feature adds) | touch Postgres directly; sanitize; make authz decisions |
| Data access, auth decisions, sequences, sanitization | `foundry_services` / `foundry-core` / `foundry-store` (Feature A seam, shipped) | render presentation |
| JSON responses (machine clients) | `foundry-api` (Feature A, shipped) | render HTML |
| CSRF / sessions | shared middleware (`csrf.rs`, tower-sessions) | change cookie/header contract |

The single most important integration invariant for Feature B: **the issue-card
partial and the comment-card partial each have ONE definition**, consumed by the
full-page render, the htmx fragment swap, AND the SSE fragment — so a live-updated card
is structurally identical to a reloaded card. Today these are separate `format!` sites
(`render_issue_card` / `render_issue_card_with_column_marker` in issues.rs;
`render_comment_card` / `render_comment_card_oob` in comments.rs). Collapsing them is
the core value of this feature, and it fixes the live-vs-reloaded comment-card
divergence the OOB renderer currently has.
