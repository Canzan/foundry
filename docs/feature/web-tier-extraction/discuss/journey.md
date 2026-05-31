# Web-Tier Extraction — UX Journeys

> DRIVER-CORRECTED (2026-05-30). The PRIMARY journey is now the **integrator/agent driving
> Foundry over the JSON API** (Journey 0, below) — that is the headline outcome. The web-tier
> journeys (board, issue+comments, sign-in) and the contributor's visual-change journey are the
> SECONDARY peer track. Slice references below reflect the corrected order (JSON API = Slices
> 1-2; web tier = Slices 3-5).

Each journey is annotated with: step-by-step flow, emotional arc, UI/CLI mockup per step,
shared artifacts that thread through, failure/recovery paths, and the Gherkin scenarios
from `stories.md` that anchor each step.

Platforms: the PRIMARY journey is a **machine/CLI consumer** of a JSON API (a `curl`/SDK/agent
client — material honesty: it reads like an API, not a GUI). The secondary journeys are **web**
(server-rendered HTML + htmx fragments + Alpine.js). Key Nielsen heuristics on the web side:
#1 visibility of status, #4 consistency, #7 flexibility (keyboard), #9 help-with-errors.
Accessibility target: WCAG 2.2 AA.

---

## Journey 0 — Integrator/agent drives Foundry over the JSON API (PRIMARY; Slices 1-2)

**Persona**: Devansh Rao building an automation (and, by extension, an agent builder). He wants
his software — not a human in a browser — to read and write Foundry's issues/comments.

**Trigger**: Devansh needs his agent to file and update issues as part of a larger workflow.

**Goal**: Authenticate as a machine with a token, then read board data and create/update
issues and comments as JSON — with the same rules the UI enforces — without scraping HTML.

**Success criterion**: A token-authenticated client GETs JSON, POSTs/PATCHes JSON, gets JSON
back (never HTML), API writes are indistinguishable in their effects from UI writes (same
authz/validation/sanitization, same realtime), and revoking the token cuts access.

### Emotional Arc (Confidence Building → Trust in a contract)

| Phase | Emotion | Confidence | Design lever |
|-------|---------|-----------|--------------|
| Discovers there IS an API | "Finally — I don't have to scrape" | 60% | A real `/api` door + stable versioned contract (NFR-WEB-API-CON-01) |
| Gets a machine token | "I can run this unattended" | 75% | Admin-issued token, no browser session needed (US-W05b) |
| First JSON read succeeds | "Predictable" | 82% | JSON in, JSON out; `[]` for empty; no HTML ever |
| First write creates a real issue | "It actually drives Foundry" | 90% | Reuses core write+outbox; the issue appears on a teammate's board live |
| Invalid write rejected like the UI | "Trustworthy — not a back door" | 93% | Rule-parity (NFR-WEB-API-CON-02): same "Title is required" |
| Revokes the token, access stops | In control / safe | 95% | Revocation bites on next call (US-W05b) |

> Material honesty: this consumer is a program. The "best" experience is a *predictable,
> documented contract* — boring, stable JSON — not a clever UI. Surprises here are regressions.

### Step-by-Step Flow

```
[Trigger: agent must file/update issues programmatically]
        |
        v
[Step 0: admin issues a machine token]   Feels: "governed access"   (US-W05b)
        |                                $MACHINE_TOKEN minted, scope-bounded
        v
[Step 1: GET board issues as JSON]        Feels: "predictable"        (US-W05a)
        |   curl -H "Accept: application/json" .../auth-v2/issues
        |   → [{"key":"AUTH-2","title":"…","state":"in_progress"}, …]
        |   Served by foundry-api ← same $CORE_BOARD_QUERY as the HTML board
        v
[Step 2: POST a new issue as JSON]        Feels: "it drives Foundry"  (US-W05c)
        |   -d '{"title":"Refresh token rotation broken on Safari"}'
        |   → 201 {"key":"AUTH-8","state":"backlog", …}
        |   Reuses $CORE_WRITE_PATH + $OUTBOX → appears on Mei's board live
        v
[Step 3: PATCH state, POST a comment]     Feels: "full control"       (US-W05c)
        |   comment body sanitized by core ($SANITIZED_HTML), same as UI
        v
[Step 4: bad write → JSON error]          Feels: "trustworthy"        (US-W05c)
        |   empty title → 422/400 JSON, same rule as UI, no HTML
        v
[Done — Foundry is drivable by software, with UI-equivalent rules]
```

### Step Mockup — Step 1/2 (terminal session; material honesty)

```
$ curl -H "Authorization: Bearer fdy_live_•••" \
       -H "Accept: application/json" \
       http://localhost:3000/api/team/backend/project/auth-v2/issues
[{"key":"AUTH-2","title":"OIDC discovery stub","state":"in_progress"},
 {"key":"AUTH-3","title":"Verify magic link expiry","state":"todo"}]

$ curl -X POST -H "Authorization: Bearer fdy_live_•••" \
       -H "Content-Type: application/json" \
       -d '{"title":"Refresh token rotation broken on Safari"}' \
       http://localhost:3000/api/team/backend/project/auth-v2/issues
{"key":"AUTH-8","title":"Refresh token rotation broken on Safari","state":"backlog"}
```

> Exact path, negotiation (Accept vs `/api` prefix), token format, and JSON shape are DESIGN.
> The mockup shows the SHAPE of the experience, not a committed contract.

### Shared Artifacts (Journey 0)

| Artifact | Source of truth | Consumers | Risk | Validation |
|----------|-----------------|-----------|------|-----------|
| `$MACHINE_TOKEN` | admin-issued credential (mechanism = DESIGN) | every API request | CRITICAL — a new credential surface; leak = full programmatic access | revoked token refused next call; never logged/plaintext (NFR-WEB-API-SEC-03) |
| `$CORE_BOARD_QUERY` | `foundry-core`/`foundry-store` board read | JSON board endpoint AND the HTML board | HIGH — proves core is presentation-neutral | both consumers use the one call; same issue set (US-W05a) |
| `$CORE_WRITE_PATH` | `insert_issue_with_outbox` etc. (core/store) | API writes AND UI writes | CRITICAL — rule-parity; API must not be a back door | paired API-vs-UI write scenarios (NFR-WEB-API-CON-02) |
| `$OUTBOX` | existing outbox/SSE path | realtime consumers of both API and UI writes | HIGH — API-created changes must reach the UI live | API-created issue appears on a watching board (US-W05c) |
| `$SANITIZED_HTML` | `foundry_core::render_comment_markdown` (ammonia) | API comment writes AND UI comment writes | CRITICAL — sanitization stays in core for both tiers | API comment with script tag → sanitized identically to UI (US-W05c) |

### Failure / Recovery Modes (Journey 0)

| Step | Failure | API response |
|------|---------|--------------|
| 0/1 | Missing/malformed/revoked token | 401 JSON; no data; no HTML error page |
| 1/2 | Token scope doesn't cover the resource | 403 JSON (same authz core enforces for a human) |
| 2 | Empty/invalid title | 422/400 JSON mirroring the UI's "Title is required"; no HTML |
| 3 | Non-author edits another's comment | 403 JSON (same rule as the UI) |
| any | Client sends `Accept: text/html` to an API route | DESIGN decides negotiation; API route never silently returns HTML from a JSON handler |

### UAT Anchors (Journey 0)

- Step 1 → `The board's issues are available as JSON` (US-W05a)
- Step 0/1 → `A valid machine token authenticates an API request` / `A revoked machine token is refused` (US-W05b)
- Step 2 → `Create an issue through the JSON API` + `An API-created issue is visible to the UI in real time` (US-W05c)
- Step 4 → `An invalid write is rejected with the same rule as the UI, as JSON` (US-W05c)

---

## Journey 1 — Member uses the issue board (SECONDARY; the surface Slice 3 extracts)

**Persona**: Mei Chen, member of "Acme Eng", Backend team. Opens the Auth v2 board to
triage. She is the same Mei from the backend-mvp journeys — this feature must not change
how the board FEELS, only how it is BUILT.

**Trigger**: Mei navigates to `/team/backend/project/auth-v2`.

**Goal**: See a styled, fast board and file/triage issues without friction.

**Success criterion**: The board looks intentional (real CSS, not raw HTML), renders in
the same latency budget as today (≤200ms P95 server-render), and every existing
interaction (file issue with `c`, drag/state-change, realtime updates) still works.

### Emotional Arc (Confidence Building)

| Phase | Emotion | Confidence | Design lever |
|-------|---------|-----------|--------------|
| Board first paint | "This looks like a real product" | 70% | Vendored CSS + coherent layout from a template, not bare HTML |
| Presses `c` | Muscle memory / trust | 85% | Modal opens <200ms; htmx fragment unchanged |
| Files issue | Flow | 90% | Same hot path; card appears in Backlog |
| Sees teammate's edit appear | "It's live, like Linear" | 92% | SSE fragment swap, unchanged |
| Nothing feels different from before | Reassured (the BEST outcome for a refactor) | 95% | The extraction is invisible to her |

> Refactor rule: for an end user, the *best* emotional outcome is **no perceptible change**.
> Any new "jank" is a regression. The only positive delta we allow ourselves is "the
> screen now looks styled instead of unstyled."

### Step-by-Step Flow

```
[Trigger: navigate to /team/backend/project/auth-v2]
        |
        v
[Step 1: Board page loads]        Feels: "looks real now"
        |                         Served by: foundry-web (template) ← foundry-core/store
        |                         Shared artifact: $BOARD_COLUMNS, $ISSUE_CARDS
        v
[Step 2: Press 'c']               Feels: muscle memory
        |                         htmx GET /…/issues/new → modal fragment (foundry-web)
        v
[Step 3: Type title, Cmd-Enter]   Feels: flow
        |                         htmx POST /…/issues → card fragment (foundry-web)
        |                         Shared artifact: $ISSUE_KEY, $CSRF_TOKEN
        v
[Step 4: Card appears in Backlog] Feels: confirmed
        |                         hx-swap-oob into [data-column='backlog']
        v
[Step 5: Teammate moves a card]   Feels: "it's live"
        |                         SSE fragment swap (foundry-web renders the fragment)
        v
[Done — board feels identical to before, but styled]
```

### Step Mockups

#### Step 1 — Styled board (post-extraction; the visible win)

```
+-- Acme Eng · Backend · Auth v2 --------------------------- [Mei ▾] -+
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
```

The asserted-on substrings from today's acceptance suite ("Backlog", the issue key, the
title) are preserved verbatim inside the new template — they become the render contract.

#### Step 3 — Issue-create modal (htmx fragment, now template-rendered)

```
+-- New issue in Auth v2 ------------------------------- [Esc] -+
| Title  [ Refresh token rotation broken on Safari_         ]  |
| Desc   [                                                  ]  |
| State [Backlog ▾]   Priority [Med ▾]   Assignee [None ▾]    |
|                              [Cancel]   [Create  ⌘↵]        |
+--------------------------------------------------------------+
```

### Shared Artifacts (Journey 1)

| Artifact | Source of truth | Consumers | Risk | Validation |
|----------|-----------------|-----------|------|-----------|
| `$BOARD_COLUMNS` | `foundry-core` (fixed state list: Backlog/Todo/In-Progress/Done/Cancelled) | board template, issue-create modal `state` select, state-change fragment | HIGH — column labels are asserted in acceptance tests | Same labels render in every consumer; one core constant |
| `$ISSUE_CARD_MARKUP` | `foundry-web` issue-card partial template | full board render, htmx create fragment (`hx-swap-oob`), SSE fragment | HIGH — the SAME card partial must render in all three paths or live cards differ from reloaded cards | One partial, three call sites; snapshot test |
| `$ISSUE_KEY` | Postgres sequence `projects.next_issue_number` (core/store) | toast, card, URL | HIGH — sequential per project, no gaps | unchanged from backend-mvp |
| `$CSRF_TOKEN` | `foundry_csrf` cookie + `_csrf` field / `HX-CSRF` header (csrf.rs) | every mutating form/htmx call | CRITICAL — extraction must keep the same cookie + header contract | POST without token → 403 (unchanged) |
| `$STATIC_ASSET_PATHS` | `foundry-web` static pipeline (`static/`), vendored htmx + Alpine + CSS | `<script>`/`<link>` tags in the layout template | HIGH — wrong path = unstyled/broken JS | assets served by the binary, no CDN; integrity check |

### Failure / Recovery Modes (Journey 1)

| Step | Failure | UX response (Nielsen #9) |
|------|---------|--------------------------|
| 1 | Static asset 404 (bad vendored path) | Page still renders HTML (graceful degradation); but this is a release blocker caught by US-W02 acceptance |
| 1 | Template render error | foundry-web returns 500 with request_id; never a half-rendered page |
| 3 | Empty title | Inline "Title is required"; modal stays open; focus in title (unchanged contract) |
| 3 | Missing/blank CSRF token | 403 before the handler; htmx surfaces a generic "Please refresh and retry" |
| 5 | SSE drops | EventSource auto-reconnect; "Reconnected — refresh for the latest" toast (unchanged) |

### UAT Anchors (Journey 1)

- Step 1 → `Board renders from a template with styled assets` (US-W02)
- Step 1 → `Existing board acceptance scenarios stay green after extraction` (US-W01)
- Step 3-4 → `Filing an issue still returns the same card fragment` (US-W01)
- Step 5 → realtime fragment unchanged (covered by backend-mvp US-09 regression)

---

## Journey 2 — Member reads an issue and comments (SECONDARY; Slice 4 surface)

**Persona**: Mei Chen, viewing AUTH-3 with 2 existing comments.

**Goal**: Read the thread, post a markdown comment, edit/delete her own — all unchanged
in feel, now rendered from templates instead of `render_issue_page`/`render_comment_card`
`format!` literals.

### Emotional Arc

| Phase | Emotion | Confidence | Design lever |
|-------|---------|-----------|--------------|
| Issue page loads | "Readable, scannable" | 75% | Template layout; comment cards visually distinct |
| Posts comment | Satisfied | 88% | htmx fragment append, unchanged contract |
| Edits her comment | In control (Nielsen #3) | 90% | Inline edit form fragment; "(edited)" marker preserved |
| Sees a 410/403 on a deleted/foreign comment | Informed, not blamed | 85% | Same terse, helpful copy as today ("This comment has been deleted. Refresh…") |

### Step Mockup — Issue detail (template-rendered)

```
+-- AUTH-3 · Verify magic link expiry ----------------------- [Mei ▾] -+
| Attachments: none                                                    |
| --------------------------------------------------------------       |
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

### Shared Artifacts (Journey 2)

| Artifact | Source of truth | Consumers | Risk | Validation |
|----------|-----------------|-----------|------|-----------|
| `$COMMENT_CARD_MARKUP` | `foundry-web` comment-card partial | full issue render, POST-comment fragment, PATCH-edit fragment, GET single-card (cancel) | HIGH — four render paths today (`render_comment_card`, `render_comment_card_oob`) must collapse to one partial or diverge | One partial across all paths; snapshot test |
| `$SANITIZED_HTML` | `foundry_core::render_comment_markdown` (ammonia) | comment body in every card | CRITICAL — sanitization MUST remain in core, NOT move to the template tier | Render `[x](javascript:…)` → href removed (unchanged NFR) |
| `$AUTHZ_AFFORDANCES` | core/store (`is_workspace_admin`, author check) | which of Edit/Delete buttons the card partial emits | HIGH — affordance gating is a *decision* (core), button rendering is *presentation* (web) | Non-author sees no Edit; admin sees Delete (server-side gated, unchanged) |
| `$CSRF_TOKEN` | csrf.rs (`_csrf` field for PATCH, `HX-CSRF` header for DELETE) | edit form, delete button | CRITICAL | unchanged contract |

### Failure / Recovery Modes (Journey 2)

| Step | Failure | UX response |
|------|---------|-------------|
| Post | Empty/over-long body | 400 inline fragment ("Comment cannot be empty" / "too long") — unchanged |
| Edit | Non-author attempts edit | 403 fragment "You may only edit your own comments." — unchanged |
| Edit/Delete | Comment already soft-deleted | 410 fragment "This comment has been deleted. Refresh…" — unchanged |
| Any | Markdown contains script/js: URL | Sanitized by core before template ever sees it — defense stays in core |

### UAT Anchors (Journey 2)

- Issue render → `Issue detail and comment thread render from templates` (US-W03)
- Post/edit/delete → `Comment edit/delete authorization unchanged after extraction` (US-W03)
- Sanitization → preserved by NFR-WEB-BND-03 (sanitization stays in core)

---

## Journey 3 — Self-hoster signs in (SECONDARY; Slice 5 surface)

**Persona**: Mei returning the next day; full-page (non-fragment) HTML.

**Goal**: A styled sign-in page that looks trustworthy, posts to the same endpoint, sets
the same session cookie.

### Emotional Arc (First-impression trust)

| Phase | Emotion | Confidence | Design lever |
|-------|---------|-----------|--------------|
| Sign-in page loads | "Looks legit" | 70% | Template + CSS; centered card, clear labels above inputs (web-patterns form rule) |
| Wrong password | Not blamed | 75% | Same non-enumerable "Invalid email or password" |
| Signed in | Relief | 95% | Same 30-day cookie, lands on dashboard |

### Step Mockup — Sign-in (template-rendered, full page)

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

### Shared Artifacts (Journey 3)

| Artifact | Source of truth | Consumers | Risk | Validation |
|----------|-----------------|-----------|------|-----------|
| `$CSRF_TOKEN` | csrf.rs (`_csrf` hidden field, cookie set on GET) | sign-in form, forgot-password form | CRITICAL | unchanged |
| `$SESSION_COOKIE` | tower-sessions Postgres store | set on POST success | CRITICAL — extraction must not change cookie attrs (HttpOnly, Secure, SameSite=Lax, 30d) | inspect Set-Cookie (unchanged NFR) |
| `$GENERIC_SIGNIN_ERROR` | signin.rs constant "Invalid email or password" | error render on both wrong-email and wrong-password | HIGH — non-enumeration must survive the template move | same string both cases |
| `$LAYOUT_TEMPLATE` | foundry-web base layout (head, vendored assets, title) | sign-in, board, issue, dashboard | MEDIUM — one base layout = consistency (Nielsen #4) | all full pages extend it |

### Failure / Recovery Modes (Journey 3)

| Step | Failure | UX response |
|------|---------|-------------|
| Submit | Wrong credentials | "Invalid email or password" (non-enumerable) — unchanged |
| Submit | 6th failure within window | 5s artificial delay (NFR-SEC-02) — unchanged, server-side |
| GET | CSRF cookie absent | Set on render; form carries matching `_csrf` — unchanged |

### UAT Anchors (Journey 3)

- Render → `Sign-in and forgot-password pages render from templates` (US-W04)
- Cookie/error → `Session cookie and error contract unchanged after extraction` (US-W04)

---

## Journey 4 — Contributor changes a screen (the jtbd-web-1 payoff)

**Persona**: Jamal Okafor, the contributor from backend-mvp US-13. He wants to change the
Backlog column heading and tweak card spacing — a pure visual change.

**Trigger**: A maintainer asks "can you make the empty-board state friendlier?"

**Goal**: Ship a visual change touching ONLY a template + CSS, with confidence he changed
no behavior.

### Emotional Arc (Problem Relief)

| Phase | Emotion | Confidence | Design lever |
|-------|---------|-----------|--------------|
| Before (today) | Dread — "the text is buried in a format! in issues.rs next to SQL" | 25% | (the problem we're fixing) |
| After: opens `templates/board.html` | "Oh — it's just here" | 80% | Markup lives in templates/, predictable location |
| Edits template, `cargo watch` reload | Flow | 88% | Edit → refresh → see it (README hot-reload loop) |
| Opens PR; reviewer sees real markup diff | Pride | 92% | Diff is HTML, not escaped Rust strings |

### Step-by-Step Flow

```
[Trigger: "make the empty board friendlier"]
        |
        v
[Step 1: grep the on-screen text]   Before: hits issues.rs (mixed). After: hits templates/.
        v
[Step 2: edit templates/board.html] Feels: "I'm only touching markup"
        |                           Guarantee (US-W06): web tier can't reach the DB,
        |                           so a template edit cannot break persistence.
        v
[Step 3: cargo watch reload]        Feels: flow
        v
[Step 4: open PR — HTML diff]       Feels: confident; reviewer sees markup as markup
```

### Shared Artifacts (Journey 4)

| Artifact | Source of truth | Consumers | Risk | Validation |
|----------|-----------------|-----------|------|-----------|
| `$TEMPLATE_FILES` | `crates/foundry-web/templates/*` | all rendered surfaces | MEDIUM — one place for markup | grep on-screen text lands in templates/, not handlers |
| `$BOUNDARY_GUARD` | US-W06 lint/crate-graph rule | CI | HIGH — the guarantee that makes Jamal confident | web crate has no `sqlx`/pool dependency; api crate emits no HTML |

### Failure / Recovery Modes (Journey 4)

| Step | Failure | Recovery |
|------|---------|----------|
| 2 | Jamal accidentally edits handler logic | US-W06 boundary guard + existing acceptance suite catch behavior changes in CI |
| 3 | Template syntax error | foundry-web fails to render the affected page with a clear error; other pages unaffected |

### UAT Anchors (Journey 4)

- Step 1-2 → `A visual change touches only a template, not handler logic` (US-W01 / US-W03)
- Step 4 / guarantee → `Web tier cannot access the database directly` (US-W06)

---

## Cross-Journey Vocabulary & Boundary Map

The extraction must keep URL/label vocabulary identical to backend-mvp (the operator-to-user
handoff depends on it) while introducing a clean internal boundary:

| Concern | Lives in (after extraction) | Must NOT do |
|---------|------------------------------|-------------|
| HTML render (templates, partials, static assets) | `foundry-web` | touch Postgres directly; serialize JSON for API clients |
| JSON responses (machine clients) | `foundry-api` | render HTML |
| Auth decisions, membership, sequences, sanitization | `foundry-core` / `foundry-store` / `foundry-auth` | render presentation |
| Realtime fanout | `foundry-realtime` (existing) | unchanged |
| CSRF / sessions | shared middleware | change cookie/header contract |

The single most important integration invariant: **the issue-card partial and the
comment-card partial each have ONE definition** in `foundry-web`, consumed by the
full-page render, the htmx fragment swap, AND the SSE fragment — so a live-updated card is
byte-identical to a reloaded card. Today these are three separate `format!` sites; the
extraction's value is collapsing them.
