<!-- markdownlint-disable MD024 -->
# Web-Tier Extraction — User Stories

> DRIVER-CORRECTED (2026-05-30). The headline outcome is now the **first-class JSON API**
> (reads AND writes, machine-token auth) so external agents/integrations can drive Foundry
> programmatically. The htmx web tier and the JSON API tier are PEER consumers of one
> presentation-neutral core. The JSON-API stories (US-W05a/b/c) lead; the web-tier template
> stories (US-W01..W04) follow as the peer-consumer track. Personas now include integrators /
> automation authors / agent builders alongside the existing member/contributor/operator
> personas. Every story is solution-neutral: this wave does NOT pick the template engine, the
> htmx version, the JSON serialization shape, or the machine-token auth mechanism — those are
> DESIGN.

## Scope Assessment

**Status**: OVERSIZED — split recommended (was PASS under the old read-only-API strawman).

With the JSON API promoted to a first-class read+write surface plus a new machine-token auth
surface, the feature now has **8 stories across two independent shippable outcomes**:

| Oversize signal | Threshold | Now | Trips? |
|-----------------|-----------|-----|--------|
| User-story count | >10 | 8 | Borderline-No |
| Independent shippable user outcomes | "multiple that could ship separately" | **2** (JSON read+write API + machine-token auth) **vs** (web templating + asset build-out + htmx2) | **YES** |
| New auth surface | n/a (qualitative) | machine-token auth is a net-new security surface, separable from templating | **YES** |
| Estimated effort | >2 weeks | ~16-22 days | **YES** |

Three signals trip. **Recommended split (for the user to ratify in DESIGN):**

- **Feature A — "Programmatic Foundry"**: JSON API (read + write) + machine-token auth +
  the web/api/core peer-consumer separation + the boundary guard. Stories US-W05a, US-W05b,
  US-W05c, US-W06. This is the headline; it ships independently and delivers the primary job.
- **Feature B — "Foundry looks like a product"**: htmx web-tier template/asset build-out
  (board, issue+comments, sign-in) + the eventual htmx2 migration. Stories US-W01..US-W04.
  This is the secondary track; it ships independently after (or in parallel with) Feature A.

Both tracks share the SAME core seam, so Feature A's "make core presentation-neutral" work is
a prerequisite the web tier also benefits from — which is why they are sequenced API-first.

**Per the brief, everything stays in THIS `web-tier-extraction/` discuss set for now**; the
split is a recommendation flagged here, in `story-map.md`, and in `wave-decisions.md` (D8) for
the user to ratify before DESIGN. No separate feature directories created yet.

## System Constraints (cross-cutting)

These apply to every story; their measurable forms live in `nfrs.md`.

- **One binary, in-process**: `foundry-web` and `foundry-api` are crates/modules inside the
  single `foundry` binary. They call `foundry-core`/`foundry-store` via in-process function
  calls — **never** over the network. No second service, no Redis, no Node runtime service.
- **Peer consumers of one core**: the web tier (HTML) and the API tier (JSON) are equal
  consumers of a presentation-neutral core. No JSON handler renders HTML; no web handler
  renders JSON; no tier touches Postgres directly (both go through core/store); core knows
  nothing about HTML vs JSON. This is structural, not a guideline.
- **JSON API is first-class (read + write)**: the API covers reads AND writes (create/update
  issues, comments, projects, state changes), not a read-only shadow of the UI.
- **Machine-token auth is a new surface (requirements only)**: programmatic clients
  authenticate with a machine token, NOT the browser cookie/session+CSRF path. This wave
  STATES the requirement and its security constraints; it does NOT design the token mechanism
  (issuance, format, storage, rotation, revocation, scoping) — that is DESIGN.
- **Acceptance suite stays green**: the existing `foundry-acceptance` cucumber suite is the
  regression net. Substrings it asserts on ("Backlog", issue keys, `data-*` markers, error
  copy) are a render contract the templates must honor; the browser auth/CSRF paths it
  exercises must keep working unchanged.
- **CSRF + sessions unchanged for the browser path**: double-submit `foundry_csrf` cookie +
  `_csrf` field / `HX-CSRF` header; `/bootstrap` CSRF-exempt; tower-sessions Postgres store;
  cookie attrs (HttpOnly, Secure, SameSite=Lax, 30-day) unchanged. The machine-token path is
  ADDITIVE — it does not alter the existing browser session/CSRF model.
- **Writes honor the same rules as the UI**: API writes enforce the SAME authorization
  (membership/admin/authorship), validation, and sanitization as the browser path, because
  both go through the same core. The API is not a privileged back door.
- **No CDN**: htmx, Alpine, and CSS are vendored into static assets shipped in the image.
- **htmx 2 deferred**: the `hx-`/`data-hx-` prefix split and any version bump are DESIGN
  decisions. Stories must not assume a version.

---

## Glossary (Ubiquitous Language — additions for this feature)

- **Web tier / `foundry-web`**: the module that renders HTML from templates and serves
  static assets. Consumes core/store; produces HTML.
- **API tier / `foundry-api`**: the module that serves a first-class JSON API (read + write)
  to machine clients. Consumes core/store as a peer of the web tier; produces JSON. Never HTML.
- **Machine token**: a credential a programmatic client (integration, automation, agent)
  presents to the API to authenticate as a machine principal — distinct from the browser
  cookie/session+CSRF path. Mechanism (issuance/format/storage/rotation/revocation/scope) is
  a DESIGN decision; this wave only states the requirement and its security constraints.
- **Integrator / automation author / agent builder**: the persona behind the primary job —
  someone building software (a script, a CI step, an agent) that drives Foundry over the API.
- **Content negotiation**: how a request selects JSON vs HTML (Accept header and/or `/api`
  path prefix). The chosen mechanism is DESIGN; the constraint (no HTML from JSON handlers,
  no JSON from web handlers) is fixed here.
- **Mixed handler**: today's handler that does auth + store + HTML `format!` + htmx
  detection in one function (e.g. `issues::submit_create`). The thing being separated.
- **Partial**: a reusable template fragment (e.g. the issue-card, the comment-card) rendered
  identically across full-page, htmx-swap, and SSE paths.
- **Render contract**: the HTML substrings the acceptance suite asserts on, treated as a
  stable interface during extraction.
- **Boundary guard**: a CI check (lint / crate-graph rule) enforcing that web≠DB and api≠HTML.

---

# ===========================================================================
# PRIMARY TRACK — The first-class JSON API (jtbd-web-4, enabled by jtbd-web-2)
# Slice 1: US-W05a (read). Slice 2: US-W05b (machine-token auth) + US-W05c (writes) + US-W06.
# ===========================================================================

## US-W05a: Read the board's issues as JSON over a presentation-neutral core seam

- **job_id**: jtbd-web-4 (Drive Foundry programmatically through a first-class JSON API)

### Elevator Pitch
- **Before**: Devansh wants his status dashboard to show the Auth v2 board, but Foundry has
  no API — his only option is to scrape the HTML board page and forge a CSRF token.
- **After**: he runs
  `curl -H "Accept: application/json" http://localhost:3000/api/team/backend/project/auth-v2/issues`
  (exact path/negotiation TBD in DESIGN) and gets back a JSON array —
  `[{"key":"AUTH-2","title":"Refresh token rotation","state":"in_progress"}, ...]` —
  served by `foundry-api` reading the SAME core/store the web board reads, with no HTML anywhere.
- **Decision enabled**: an integrator decides Foundry has a real machine-readable door worth
  building on, and a maintainer confirms core is genuinely presentation-neutral (it feeds both
  HTML and JSON from one call) — the foundation the whole API rests on.

### Problem
There is no JSON API today; every handler is HTML-only and mixed (auth + store + `format!`
HTML in one function). An integrator, automation author, or agent builder who wants to read
Foundry data programmatically has to scrape rendered HTML and forge browser CSRF tokens —
brittle, and it breaks the moment markup changes. Proving the API starts with the smallest
real read surface (the board's issues) over a core seam that core can feed without knowing
whether the consumer wants HTML or JSON.

### Who
- **Devansh Rao**, an operator/integrator wiring a read-only status dashboard.
- **A maintainer**, validating that core has no presentation coupling.
- **Context**: read-path first; the board/issue data the web board already lists.
- **Motivation**: a JSON endpoint that proves the core seam is real and presentation-neutral.

### Solution
Introduce `foundry-api` serving at least one read endpoint (the board's issues) as JSON,
carved from the data-access logic the mixed handlers already perform. It reuses the same
core/store calls the web tier will use and emits JSON only — never HTML. For US-W05a the
endpoint MAY accept the existing browser session for authentication (the machine-token surface
arrives in US-W05b); the point of this story is to prove core is presentation-neutral.

### Domain Examples
#### 1. Happy path — board issues as JSON
Devansh's status script requests the Auth v2 board's issues and receives a JSON array:
`[{"key":"AUTH-2","title":"...","state":"in_progress"}, {"key":"AUTH-3", ...}]`.
#### 2. Edge case — empty project
The Sandbox project returns `[]` (empty array) with HTTP 200 — not an HTML empty state.
#### 3. Error/boundary case — api handler must not emit HTML
A contributor tries to return an HTML error page from a foundry-api handler. The boundary
guard (US-W06) flags it; the api tier's response type does not permit HTML bodies.

### UAT Scenarios
```gherkin
Scenario: The board's issues are available as JSON
  Given Mei is signed in and the Auth v2 project has issues AUTH-2 and AUTH-3
  When a JSON request is made for the Auth v2 board's issues
  Then the response is a JSON array containing AUTH-2 and AUTH-3 with their titles and states
  And the response contains no HTML

Scenario: An empty project returns an empty JSON array
  Given the Sandbox project has no issues
  When a JSON request is made for the Sandbox board's issues
  Then the response is an empty JSON array with HTTP 200

Scenario: The JSON tier reuses the same core data path as the web tier
  Given the web board and the JSON endpoint both list the Auth v2 issues
  When both are produced
  Then both obtain their data through the same core/store call
  And the set of issues returned matches

Scenario: Unauthorized JSON access is refused
  Given a request with no valid credential
  When it requests the board's issues as JSON
  Then it is refused with an unauthorized status
  And no issue data is returned
```

### Acceptance Criteria
- [ ] At least one read endpoint (the board's issues) is served as JSON by `foundry-api`.
- [ ] The JSON tier reuses the same core/store calls the web tier uses for that data.
- [ ] No `foundry-api` handler emits HTML.
- [ ] The JSON endpoint enforces authorization (membership) equivalent to the web tier.
- [ ] An empty result returns `[]` with HTTP 200, not an error or HTML.

### Outcome KPIs
- **Who**: Integrators and maintainers exercising the machine-readable read surface.
- **Does what**: Read Foundry data programmatically without scraping HTML.
- **By how much**: ≥1 board/issue read endpoint returns valid JSON with 0 HTML bytes; 100% of
  its data comes through the same core call the web board uses.
- **Measured by**: acceptance assertion that the response parses as JSON and contains no HTML
  tags; core-call-reuse verified by inspection.
- **Baseline**: 0 JSON endpoints today (HTML-only handlers); programmatic reads require scraping.

### Technical Notes
- Read-path only in this story. Machine-token auth (US-W05b) and writes (US-W05c) follow.
- Exact route prefix, content negotiation (Accept header vs `/api` prefix), and serialization
  shape are DESIGN.
- This is the walking-skeleton proof that core is presentation-neutral — the prerequisite for
  both the rest of the API and the web-tier peer track.

### Size
**M** (2-3 days, 4 scenarios). Touches: new `foundry-api` module, ≥1 read endpoint, route
wiring, shared core calls.

### Dependencies
None (entry slice for the primary track). The core seam it establishes is reused by US-W05b,
US-W05c, and the web-tier stories US-W01..W04.

---

## US-W05b: Authenticate programmatic clients with a machine token

- **job_id**: jtbd-web-4 (Drive Foundry programmatically through a first-class JSON API)

### Elevator Pitch
- **Before**: Devansh's only way to authenticate to US-W05a's read endpoint is to reuse a
  human's browser session cookie — unusable for an unattended script or an agent.
- **After**: he obtains a machine token once, then calls
  `curl -H "Authorization: Bearer fdy_xxx" http://localhost:3000/api/.../issues`
  and is authenticated as a machine principal — no browser, no cookie, no CSRF forging.
- **Decision enabled**: an automation author/agent builder decides they can run Foundry
  integrations unattended, because there is a real machine credential separate from human login.

### Problem
US-W05a proves data can flow as JSON, but the only authentication that exists is the browser
cookie/session + CSRF double-submit path — designed for an interactive human, not an
unattended script or agent. Without a machine credential, the "first-class API" is still
gated behind a human's browser session. This story establishes the REQUIREMENT for a machine
token (it does NOT design the mechanism).

### Who
- **Devansh Rao**, automation author running an unattended integration.
- **An agent builder**, whose agent must act as a non-human principal.
- **A workspace admin**, who must be able to grant and revoke that machine's access.
- **Context**: a new, additive auth surface that does not change the browser path.
- **Motivation**: a credential a machine can hold and an admin can govern.

### Solution
Establish that the API accepts a machine token (presented out-of-band of the browser session)
as a first-class authentication method. A workspace admin can issue a token bound to a
principal/scope and can revoke it; a request bearing a valid token is authenticated as that
principal and subject to the same authorization as a human with equivalent access; a request
with a missing/invalid/revoked token is refused. The exact token format, storage, hashing,
rotation, and scoping model are DESIGN; the browser session/CSRF path is unchanged.

### Domain Examples
#### 1. Happy path — token-authenticated read
A workspace admin issues Devansh's dashboard a machine token. His script presents it and reads
the Auth v2 board's issues as JSON — no browser session involved.
#### 2. Edge case — revoked token
The admin revokes the dashboard's token after the project ends. The next request with that
token is refused with an unauthorized status; no data is returned.
#### 3. Error/boundary case — token scope does not cover the workspace
A token issued for the "Backend" team is used to read a "Platform" team board it has no access
to. The request is refused with a forbidden status — the same authorization core enforces for
a human without that membership.

### UAT Scenarios
```gherkin
Scenario: A valid machine token authenticates an API request
  Given a workspace admin has issued a machine token for Devansh's dashboard
  When the dashboard requests the Auth v2 board's issues with that token
  Then the request is authenticated as the machine principal
  And the board's issues are returned as JSON

Scenario: A machine-token request needs no browser session or CSRF token
  Given a request carries a valid machine token and no session cookie and no CSRF token
  When it reads the board's issues
  Then it succeeds
  And the existing browser session and CSRF requirements are unchanged for browser requests

Scenario: A revoked machine token is refused
  Given a machine token has been revoked by a workspace admin
  When a request presents that token
  Then it is refused with an unauthorized status
  And no data is returned

Scenario: A machine token is bound to the same authorization as a human principal
  Given a machine token scoped to the Backend team
  When it requests a board belonging to the Platform team
  Then it is refused with a forbidden status

Scenario: A missing or malformed token is refused
  Given a request to a machine-only API endpoint with no token or a malformed token
  When it is processed
  Then it is refused with an unauthorized status
```

### Acceptance Criteria
- [ ] The API accepts a machine token as a first-class authentication method, distinct from
      the browser cookie/session+CSRF path.
- [ ] A workspace admin can issue a machine token and can revoke it; a revoked token is refused.
- [ ] A token-authenticated request is subject to the SAME authorization as a human principal
      with equivalent access (no privilege escalation via the API).
- [ ] A machine-token request requires neither a browser session cookie nor a CSRF token.
- [ ] The existing browser session/CSRF model is unchanged (additive surface only).

### Outcome KPIs
- **Who**: Automation authors and agent builders running unattended Foundry integrations.
- **Does what**: Authenticate to the API as a machine principal without a human browser session.
- **By how much**: 100% of API endpoints reachable with a machine token alone (0 browser
  session/CSRF dependency on the machine path); revoked tokens refused on the next request.
- **Measured by**: acceptance scenarios for issue/revoke/authz; auth-path inspection confirms
  the browser model is untouched.
- **Baseline**: 0 — today the only credential is a human browser session + CSRF.

### Technical Notes
- REQUIREMENTS ONLY: token format, storage/hashing, rotation, expiry, scoping granularity, and
  the issuance UX (CLI? admin screen? bootstrap?) are DESIGN decisions — flagged as open
  questions for DESIGN.
- Security constraints captured as NFRs (NFR-WEB-API-SEC-01..03).
- This is a NEW auth surface; DESIGN must treat it with the same rigor as the password/session
  model (it is where API security will cluster).

### Size
**M** (3 days, 5 scenarios). Touches: `foundry-api` auth extractor, an admin issue/revoke
path, authorization reuse from core. Mechanism deferred to DESIGN.

### Dependencies
US-W05a (a JSON endpoint to authenticate against). Ships in Slice 2 with US-W05c + US-W06.

---

## US-W05c: Create and update issues and comments through the JSON API

- **job_id**: jtbd-web-4 (Drive Foundry programmatically through a first-class JSON API)

### Elevator Pitch
- **Before**: even with read access and a token, Devansh's agent cannot file or update an
  issue programmatically — writes exist only behind the htmx browser forms.
- **After**: the agent runs
  `curl -X POST -H "Authorization: Bearer fdy_xxx" -H "Content-Type: application/json"
  -d '{"title":"Refresh token rotation broken on Safari"}'
  http://localhost:3000/api/team/backend/project/auth-v2/issues`
  and gets back the created issue as JSON (`{"key":"AUTH-8", ...}`); a follow-up PATCH moves
  it to "in_progress" — all through core, honoring the same validation and authorization the
  UI enforces.
- **Decision enabled**: an agent builder decides Foundry is genuinely automatable end-to-end —
  their agent can DO work, not just observe it — which is the headline promise of the feature.

### Problem
Reads (US-W05a) and machine-token auth (US-W05b) make Foundry observable programmatically, but
the primary job is to DRIVE Foundry — create and update issues, comments, projects, and state.
Today every write lives behind an htmx browser form with CSRF and an HTML response. Without a
JSON write surface, agents and integrations can watch but cannot act, and "first-class API"
is only half-true.

### Who
- **An agent builder**, whose agent files and updates issues as part of a workflow.
- **Devansh Rao**, scripting bulk issue creation / state changes.
- **A maintainer**, ensuring API writes honor the same authz/validation/sanitization as the UI.
- **Context**: write-path JSON for the smallest real surface (issues + comments), reusing the
  same core write functions (`insert_issue_with_outbox`, comment create/edit) the UI uses.
- **Motivation**: programmatic create+update that is indistinguishable, in its effects and
  rules, from a human doing the same thing in the UI.

### Solution
Add JSON write endpoints for issues and comments (create + update, and issue state change) to
`foundry-api`, reusing the SAME core write functions the browser handlers call. The API accepts
JSON request bodies, returns the created/updated resource as JSON, and enforces the same
authorization (membership/admin/authorship), the same validation (e.g. "title required"), and
the same markdown sanitization (in core) as the browser path. Writes go through the same
outbox so realtime/SSE consumers see API-created changes exactly as UI-created ones.

### Domain Examples
#### 1. Happy path — create an issue via JSON
Devansh's agent POSTs `{"title":"Refresh token rotation broken on Safari"}` to the Auth v2
issues endpoint with a valid token and receives `201` with `{"key":"AUTH-8","state":"backlog",...}`.
A teammate watching the board sees AUTH-8 appear (same outbox/SSE path as a UI-filed issue).
#### 2. Edge case — update issue state, then add a comment
The agent PATCHes AUTH-8 to `{"state":"in_progress"}` (200 with the updated issue), then POSTs
a comment `{"body":"Picked up — investigating SameSite."}`; the comment body is sanitized by
core exactly as a UI comment would be.
#### 3. Error/boundary case — invalid write is rejected like the UI rejects it
The agent POSTs an issue with an empty title → `422`/`400` with a JSON error mirroring the UI's
"Title is required" rule; a non-author token PATCHing someone else's comment → `403` (same authz
core enforces for the UI); the JSON error body contains no HTML.

### UAT Scenarios
```gherkin
Scenario: Create an issue through the JSON API
  Given Devansh holds a machine token with write access to the Auth v2 project
  When he POSTs a JSON body with title "Refresh token rotation broken on Safari"
  Then a new issue is created with the next sequential key
  And the created issue is returned as JSON including its key and state
  And the response contains no HTML

Scenario: An API-created issue is visible to the UI in real time
  Given Mei is viewing the Auth v2 board
  When Devansh creates an issue through the JSON API
  Then the new issue appears on Mei's board
  And it was persisted through the same core write path as a UI-filed issue

Scenario: Update an issue's state through the JSON API
  Given issue AUTH-8 exists in the Backlog
  When a PATCH sets its state to "in_progress"
  Then the issue's state becomes "in_progress"
  And the updated issue is returned as JSON

Scenario: Create a comment through the JSON API with the same sanitization as the UI
  Given Devansh holds a write-scoped machine token
  When he POSTs a comment body containing a script tag and a "javascript:" link
  Then the comment is created with the dangerous content removed by core
  And the sanitization is identical to a comment posted through the UI

Scenario: An invalid write is rejected with the same rule as the UI, as JSON
  Given a request to create an issue with an empty title
  When the API processes it
  Then it is rejected with the same validation rule the UI enforces ("Title is required")
  And the error is returned as JSON with no HTML

Scenario: A write beyond the token's authorization is refused
  Given a machine token that is not the author of a comment and is not an admin
  When it submits an edit to that comment
  Then it is refused with a forbidden status
  And no change is persisted
```

### Acceptance Criteria
- [ ] JSON write endpoints exist for: create issue, update issue state, create comment, and
      update comment (the smallest real create+update surface).
- [ ] Writes reuse the SAME core write functions the browser handlers call (including the outbox).
- [ ] API writes enforce the SAME authorization (membership/admin/authorship) as the UI.
- [ ] API writes enforce the SAME validation and (for comments) core markdown sanitization as the UI.
- [ ] Successful writes return the created/updated resource as JSON; errors return JSON, never HTML.
- [ ] An API-created change reaches realtime/SSE consumers via the same outbox as a UI change.

### Outcome KPIs
- **Who**: Agent builders and integrators driving Foundry programmatically.
- **Does what**: Create and update issues/comments via JSON without touching the UI.
- **By how much**: ≥4 write operations (issue create, issue state update, comment create,
  comment update) succeed via JSON with 100% rule-parity to the UI (same authz/validation/
  sanitization outcomes); 0 HTML bytes in any API write response.
- **Measured by**: acceptance scenarios asserting parity (same rejection rules, same sanitized
  output) between API and UI writes; outbox/SSE visibility assertion.
- **Baseline**: 0 write endpoints today — programmatic writes are impossible.

### Technical Notes
- REQUIREMENTS ONLY: request/response JSON shapes, status-code conventions, partial-update
  semantics (PATCH vs PUT), and idempotency are DESIGN.
- The rule-parity constraint (API writes == UI writes in authz/validation/sanitization) is the
  load-bearing requirement and is captured as NFR-WEB-API-CON-02.
- Reuses the existing `_with_outbox` write path so realtime stays consistent across tiers.

### Size
**M-L** (3-4 days, 6 scenarios). Touches: `foundry-api` write endpoints, JSON (de)serialization
at the boundary, reuse of core write + outbox + sanitization. Borderline upper bound — if it
grows past 7 scenarios in DESIGN, split issues-writes from comments-writes.

### Dependencies
US-W05a (the JSON read surface + core seam) and US-W05b (machine-token auth, so writes are
authenticated as a machine). Ships in Slice 2 with US-W05b + US-W06.

---

# ===========================================================================
# SECONDARY TRACK — htmx web-tier template/asset extraction
# (jtbd-web-1 restyle, jtbd-web-3 first-screen trust; enabled by jtbd-web-2)
# Slice 3: US-W01 + US-W02. Slice 4: US-W03. Slice 5: US-W04.
# These reuse the SAME presentation-neutral core seam the JSON-API track proved.
# ===========================================================================

## US-W01: Extract the issue board into a web tier over a core seam

- **job_id**: jtbd-web-2 (Web and API are peer consumers of one presentation-neutral core)

### Elevator Pitch
- **Before**: Mei opens `http://localhost:3000/team/backend/project/auth-v2` in her browser.
- **After**: she sees the same board she saw yesterday — same columns, same cards, same `c`-to-create — now served by `foundry-web` rendering from a template and reading the board's data through a core/store seam, with the web code carrying **no** database dependency.
- **Decision enabled**: the maintainer decides the web/api boundary is real and free (no perf or test regression), unlocking the rest of the extraction.

### Problem
Jamal (a contributor) and the maintainer cannot tell, from the code, where presentation
ends and persistence begins: `issues::submit_create` extracts the session, runs
`is_team_member`, calls `find_team_by_slug` / `insert_issue_with_outbox`, AND builds HTML
with `format!` — four concerns in one function. Reviewing a "make the board prettier" PR
means reading SQL. The board is the highest-traffic surface, so it is where proving the
boundary matters most and where a regression would hurt most.

### Who
- **Jamal Okafor**, Rust contributor, AGPLv3-attracted, wants to change UI without fear of
  breaking behavior.
- **Mei Chen**, member viewing the board, must perceive zero behavioral change.
- **Context**: existing `/team/{team}/project/{project}` board route, today rendered by
  `projects::show_board` + `issues::*` via `format!` literals.
- **Motivation**: a boundary that is a compile-time fact, not a reviewer's vigilance.

### Solution
Introduce a `foundry-web` module that owns the board render. The board route renders through
foundry-web; foundry-web obtains board data by calling core/store (the same calls the handler
makes today) and renders the result — it holds no `sqlx`/pool handle of its own. The issue
board, the issue-create fragment, and the state-change fragment all route through foundry-web.
The existing board acceptance scenarios run unchanged and stay green.

### Domain Examples
#### 1. Happy path — board renders via the web tier
Mei opens Auth v2. `foundry-web` renders the board (Backlog/Todo/In-Progress/Done columns,
AUTH-2/3/6/7 cards) from data it got via `store.list_issues_for_board(...)`-style core calls.
The HTML is byte-compatible with the asserted substrings. She presses `c`, files
"Refresh token rotation broken on Safari", and AUTH-8 appears in Backlog via the same
`hx-swap-oob` card fragment — now produced by a foundry-web partial.

#### 2. Edge case — board with zero issues
Devansh opens the brand-new "Sandbox" project. foundry-web renders the four empty columns
with an inviting empty state ("No issues yet — press c to file the first one") instead of a
blank pane. The empty-board render still goes through the same core call (returns an empty list).

#### 3. Error/boundary case — web tier attempts a forbidden DB reach
A contributor tries to add a direct `sqlx::query!` inside the foundry-web board renderer.
The crate does not depend on the pool, so it does not compile (and US-W06 later makes this a
named CI failure). The boundary is enforced structurally, not by review.

### UAT Scenarios
```gherkin
Scenario: Board renders through the web tier with the same content
  Given Mei is signed in as a member of the Backend team
  And the Auth v2 project has issues AUTH-2, AUTH-3, AUTH-6 in Backlog/Todo/In-Progress
  When Mei opens the Auth v2 board
  Then she sees the columns "Backlog", "Todo", "In-Progress", "Done"
  And she sees the cards for AUTH-2, AUTH-3, and AUTH-6 in their respective columns
  And the page is rendered by the web tier from a template

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

Scenario: The existing board acceptance scenarios remain green
  Given the foundry-acceptance suite includes the slice-1 board scenarios
  When the suite runs against the extracted web tier
  Then every previously-passing board scenario still passes

Scenario: The web tier holds no direct database access
  Given the foundry-web module renders the board
  When the project is built
  Then foundry-web has no dependency that exposes the database connection pool
  And board data reaches the renderer only through core/store calls
```

### Acceptance Criteria
- [ ] The board route renders via `foundry-web` from a template (not inline `format!`).
- [ ] Board, issue-create fragment, and state-change fragment share the issue-card partial.
- [ ] The card partial renders identically in full-page, htmx-swap, and SSE paths.
- [ ] All previously-green board acceptance scenarios remain green.
- [ ] `foundry-web` has no direct database/pool dependency; data flows via core/store.
- [ ] Empty board renders an inviting empty state with a call to action.

### Outcome KPIs
- **Who**: Foundry maintainers/contributors evaluating the boundary on the board surface.
- **Does what**: Make a board markup change touching only a template (no handler/SQL edit).
- **By how much**: 100% of board-only visual changes touch zero files under `foundry-store`
  and zero `sqlx` call sites (measured per PR).
- **Measured by**: PR file-path diff inspection on board-visual PRs; CI green on the
  unchanged acceptance suite.
- **Baseline**: 0% today — a board text change touches `issues.rs` (a handler/store file).

### Technical Notes
- Solution-neutral: template engine choice is DESIGN. The store-facing calls already exist
  (`find_team_by_slug`, board listing); reuse them.
- The asserted substrings ("Backlog", issue key format, `hx-swap-oob` target) are a render
  contract — keep them stable.
- Mixed `hx-`/`data-hx-` prefixes are preserved as-is in this slice (Slice 3); normalization is DESIGN.

### Size
**M** (3 days, 5 scenarios). Touches: new `foundry-web` module, board template + card
partial, rewiring `projects::show_board` and `issues::*` render paths.

### Dependencies
US-W05a (reuses the presentation-neutral core seam the JSON-API track established). Paired with
US-W02 in **Slice 3** (the entry slice of the secondary web-tier track).

---

## US-W02: Render the board from vendored assets so it looks like a product

- **job_id**: jtbd-web-3 (Minimize the chance a self-hoster's first screen feels unstyled)

### Elevator Pitch
- **Before**: Mei opens the board in her browser.
- **After**: the board loads with real CSS, a coherent header, and htmx + Alpine behavior — all from assets the binary already ships (no CDN, no external fetch) — so it reads as a finished product, not raw HTML.
- **Decision enabled**: a self-hosting team decides Foundry looks credible enough to keep evaluating instead of bouncing on an unstyled screen.

### Problem
`crates/foundry-app/static/` and `templates/` are empty. Today's htmx behavior is wired
through inline string attributes with no stylesheet and no vendored JS. A first-time
self-hoster who opens the board sees unstyled HTML, which reads as "unfinished prototype"
and undermines the "Linear-style ergonomics" promise from the README before the team ever
files an issue.

### Who
- **Mei Chen** / her teammates, seeing Foundry's UI for the first time.
- **Devansh**, the operator who will screenshot it for his team.
- **Context**: the Slice-1 board, now template-rendered (US-W01), still visually bare.
- **Motivation**: a first screen that earns trust.

### Solution
Establish a static-asset pipeline in `foundry-web`: vendor htmx and Alpine.js and a Foundry
stylesheet into `static/`, served by the binary. The base layout template links them. The
board uses the stylesheet for columns, cards, header, and the create affordance. No CDN, no
Node runtime service; assets are part of the image.

### Domain Examples
#### 1. Happy path — styled board offline
Devansh runs Foundry on an air-gapped VM with no internet egress. Mei opens the board and it
is fully styled and interactive — htmx and Alpine load from `localhost:3000/static/...`,
never from a CDN.
#### 2. Edge case — keyboard-only user
Hiroshi navigates the board with Tab/Enter only. Focus indicators are visible; the `c`
shortcut and all interactive controls are keyboard-reachable (WCAG 2.2 AA operable).
#### 3. Error/boundary case — a stale/incorrect asset path
A typo points the layout at `/static/htmx.js` when the file is `/static/htmx.min.js`. The
acceptance check for "vendored assets resolve" fails in CI, so the broken-asset board never
ships.

### UAT Scenarios
```gherkin
Scenario: The board loads with vendored styles and scripts, no external CDN
  Given Foundry is running on a host with no outbound internet access
  When Mei opens the Auth v2 board
  Then the board is visually styled (columns and cards are laid out, not raw HTML)
  And htmx and Alpine are loaded from the application's own static path
  And no request is made to an external CDN

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
  When the asset-resolution acceptance check runs
  Then the check fails and the board is not released in that state
```

### Acceptance Criteria
- [ ] htmx, Alpine, and a Foundry stylesheet are vendored under the web tier's static path
      and served by the binary (HTTP 200, correct content type).
- [ ] The board renders styled (columns, cards, header) using the vendored stylesheet.
- [ ] No external CDN request is made to render or operate the board.
- [ ] The board is fully keyboard-operable with visible focus indicators (WCAG 2.2 AA).
- [ ] A referenced-but-missing static asset is caught by an acceptance/build check.

### Outcome KPIs
- **Who**: First-time self-hosting teams opening Foundry's board.
- **Does what**: Continue evaluating past the first screen (don't bounce on "looks broken").
- **By how much**: 0 external CDN requests on the board; styled-board acceptance check green
  on a no-egress host.
- **Measured by**: Network-request assertion in the acceptance harness (count of external
  origins = 0) + visual/asset-resolution checks.
- **Baseline**: today the board is unstyled (empty `static/`), 0% styled.

### Technical Notes
- Solution-neutral on the exact CSS approach and whether a build-time (non-runtime) asset
  step is used — that is DESIGN. Constraint: no new *runtime* service, no CDN dependency.
- The vendored htmx version is NOT chosen here (htmx 2 deferred to DESIGN).

### Size
**M** (2-3 days, 4 scenarios). Touches: static pipeline, base layout, board stylesheet,
vendored assets, asset-resolution check.

### Dependencies
US-W01 (the board must render from a template before it can be styled coherently). Ships
together with US-W01 as **Slice 3**.

---

## US-W03: Move the issue detail and comment thread to templates

- **job_id**: jtbd-web-1 (Minimize effort to restyle a screen without touching Rust logic)

### Elevator Pitch
- **Before**: Mei opens `/team/backend/project/auth-v2/issues/3` and reads the comment thread.
- **After**: the issue page and every comment card render from a single comment-card partial in `foundry-web` — full-page load, htmx post-append, inline edit, and cancel all show the identical card — while Edit/Delete affordances and markdown sanitization stay decided in core.
- **Decision enabled**: a contributor decides they can restyle the comment thread by editing one partial, confident the four old render paths can no longer drift apart.

### Problem
The comment surface is the most tangled: `comments.rs` has `render_issue_page`,
`render_comment_card`, and `render_comment_card_oob` — at least three `format!` render
sites, plus the edit-form fragment built inline. The OOB (live) card deliberately *omits*
the Edit/Delete buttons "for simplicity", so a live-posted card already looks different from
a reloaded one. Restyling means editing Rust in four places and risking divergence; and the
authz/sanitization logic is interleaved with the markup.

### Who
- **Jamal Okafor**, restyling the comment thread.
- **Mei Chen**, posting/editing/deleting comments; must see no behavioral change.
- **Context**: `comments::show_issue`, `submit_comment`, `submit_edit_comment`,
  `submit_delete_comment`, `show_edit_form`, `show_single_comment`.
- **Motivation**: one place to change comment markup, with the live card matching the reloaded card.

### Solution
foundry-web owns an issue-page template and a single comment-card partial used by every
render path. Authorization affordances (can-edit = author; can-delete = author or admin) are
*decided* in core/store and passed to the partial as booleans; the partial only *renders*
them. Markdown sanitization stays in `foundry_core::render_comment_markdown`. The 400/403/410
error fragments keep their exact copy.

### Domain Examples
#### 1. Happy path — post a comment, live card matches reloaded card
Mei posts "Looked into this — SameSite default change." Hiroshi (viewing) sees the new card
appended via htmx; it shows the same author/body/affordance layout as it does after a full
page reload, because both render the same partial.
#### 2. Edge case — author edits, "(edited)" marker
Mei edits her comment. The inline edit-form fragment and the re-rendered card both come from
templates; the "(edited)" marker appears; Hiroshi sees the update. Non-authors still see no
Edit button (affordance decided in core).
#### 3. Error/boundary case — non-author PATCH and deleted-comment 410
Hiroshi POSTs an edit to Mei's comment endpoint → 403 fragment "You may only edit your own
comments." Editing an already-soft-deleted comment → 410 fragment "This comment has been
deleted. Refresh to see the latest state." Both strings are unchanged.

### UAT Scenarios
```gherkin
Scenario: Issue page and comment thread render from templates
  Given issue AUTH-3 has comments by Mei and Hiroshi
  When Mei opens the AUTH-3 issue page
  Then the issue header and both comment cards render from the web tier templates
  And each comment card shows its author and rendered markdown body

Scenario: A live-posted comment card matches a reloaded one
  Given Hiroshi is viewing AUTH-3 while Mei posts a new comment
  When Mei's comment is appended via htmx
  Then the appended card has the same structure as the same card after a full page reload

Scenario: Edit and delete affordances are gated in core, rendered in the template
  Given Mei is the author of a comment and Devansh is a workspace admin
  When the comment thread renders for each of them
  Then Mei sees Edit and Delete on her own comment
  And Devansh sees Delete (admin moderation) but not Edit on Mei's comment
  And Hiroshi (neither author nor admin) sees neither on Mei's comment

Scenario: Non-author edit is refused with the unchanged message
  Given Hiroshi is not the author of Mei's comment
  When Hiroshi submits an edit to that comment
  Then he receives a 403 with the message "You may only edit your own comments."

Scenario: Editing a deleted comment returns the unchanged gone message
  Given a comment has been soft-deleted
  When an edit is submitted for it
  Then the response is 410 with copy stating the comment has been deleted and to refresh

Scenario: Markdown sanitization remains in core
  Given Mei submits a comment containing a "javascript:" link and a script tag
  When the comment renders
  Then the dangerous URL and script are removed
  And the sanitization is performed by core before the template renders the body
```

### Acceptance Criteria
- [ ] Issue page and all comment render paths use one comment-card partial in foundry-web.
- [ ] The live (htmx-appended) card and the reloaded card are structurally identical.
- [ ] Edit/Delete affordances are decided in core/store and passed to the partial as flags;
      the partial contains no authorization logic.
- [ ] Markdown sanitization stays in `foundry_core`; the web tier never sanitizes.
- [ ] 400/403/410 error fragments keep their exact existing copy.
- [ ] All previously-green comment/issue acceptance scenarios stay green.

### Outcome KPIs
- **Who**: Contributors changing the comment-thread presentation.
- **Does what**: Change comment markup in one partial instead of multiple Rust sites.
- **By how much**: comment-render `format!` sites reduced from ≥3 to 1 partial; 0 authz logic
  in the web tier.
- **Measured by**: code inspection (count of comment-render sites; grep for membership/admin
  checks under foundry-web = 0) + acceptance suite green.
- **Baseline**: ≥3 `format!` comment-render sites today; authz interleaved with markup.

### Technical Notes
- The OOB-card-omits-buttons quirk is resolved by sharing the partial (affordances come from
  the same flags), removing the live-vs-reloaded divergence.
- Sanitization staying in core is also an NFR (NFR-WEB-BND-03).

### Size
**M** (3 days, 6 scenarios). Touches: issue-page template, comment-card partial, edit-form
fragment, rewiring six handlers' render paths.

### Dependencies
US-W01 (the seam, base layout, and card-partial pattern). **Slice 4.**

---

## US-W04: Move sign-in and forgot-password to templates

- **job_id**: jtbd-web-3 (Minimize the chance a self-hoster's first screen feels unstyled)

### Elevator Pitch
- **Before**: Mei visits `/sign-in` to come back to Foundry.
- **After**: she sees a styled, full-page sign-in rendered from the shared base layout — labels above inputs, a clear primary button, a "Forgot your password?" link — that posts to the same endpoint and sets the same 30-day session cookie.
- **Decision enabled**: a returning user (and a first-time evaluator) decides Foundry's auth screens look as trustworthy as the rest of the product.

### Problem
`signin.rs` renders the sign-in, sign-out, and forgot-password screens as full-page `format!`
HTML with no shared layout. As the only full-page (non-fragment) surfaces, they are where an
evaluator first lands; an unstyled login reads as insecure/unfinished. They also duplicate
head/asset boilerplate the board template now has, risking visual inconsistency (Nielsen #4).

### Who
- **Mei Chen**, returning member signing in.
- **A first-time evaluator** landing on `/sign-in`.
- **Context**: `signin::show_form`, `show_forgot_form`, plus the dashboard/landing renders.
- **Motivation**: a consistent, trustworthy auth screen reusing the base layout.

### Solution
Move sign-in and forgot-password to templates that extend the shared base layout (same head,
vendored assets, header). The POST handlers, CSRF contract (hidden `_csrf` field, cookie set
on GET), session-cookie attributes, and the non-enumerable "Invalid email or password" error
are all unchanged — only the markup moves.

### Domain Examples
#### 1. Happy path — styled sign-in, same cookie
Mei visits `/sign-in`, sees a styled card (labels above inputs, one-column form per
web-patterns), enters her credentials, and lands on the dashboard with the same HttpOnly
Secure SameSite=Lax 30-day cookie as before.
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
  Then the sign-in page rendered from the web tier base layout
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
  Then the page rendered from the web tier base layout
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
- **By how much**: 100% of full-page auth screens extend the one shared layout (0 duplicated
  head/asset boilerplate).
- **Measured by**: code inspection (auth templates extend base layout; 0 inline `<head>`
  duplication) + acceptance suite green.
- **Baseline**: sign-in is standalone `format!` HTML with no shared layout today.

### Technical Notes
- Sign-in/forgot are full-page surfaces (no htmx fragment swap), so this is the
  lowest-fragment-risk extraction — hence sequenced after the fragment-heavy surfaces.
- The brute-force artificial delay (NFR-SEC-02) is server-side and untouched.

### Size
**S-M** (2 days, 4 scenarios). Touches: base-layout extension, sign-in + forgot templates,
rewiring `signin::show_form`/`show_forgot_form`.

### Dependencies
US-W01 (base layout + static pipeline). **Slice 5.**

---

## US-W05: (SUPERSEDED by the driver correction — see US-W05a / US-W05b / US-W05c)

The original US-W05 was a single read-only JSON endpoint sequenced LAST, treating the API as a
boundary-proof afterthought. The 2026-05-30 driver correction makes the JSON API the headline
outcome (read + write, machine-token auth) and sequences it FIRST. US-W05 is therefore split
and promoted into the primary track:

- **US-W05a** — Read the board's issues as JSON over a presentation-neutral core seam (Slice 1).
- **US-W05b** — Authenticate programmatic clients with a machine token (Slice 2).
- **US-W05c** — Create and update issues and comments through the JSON API (Slice 2).

See the PRIMARY TRACK section above for the full stories.

---

## US-W06: Lock the web/api boundary with a structural guard `@infrastructure`

- **job_id**: infrastructure-only
- **infrastructure_rationale**: This story produces no new user-observable behavior on its
  own — it enforces the peer-consumer boundary the value stories (US-W05a/b/c and US-W01/03/04)
  establish. It exists so the boundary *cannot erode* (jtbd-web-2's durability) — which matters
  most once the JSON API is a real write surface, because an eroded boundary would let the API
  tier start emitting HTML or the web tier start bypassing core. Per the slice-level rule it is
  NOT a standalone slice: it ships folded into **Slice 2** (the JSON-API write slice) alongside
  US-W05c (a user-visible write story), so the released slice carries value.

### Problem
A boundary enforced only by code review erodes: a future PR adds a pool dependency to a web
handler, or returns an HTML error from an api handler, and nobody notices until the tiers are
tangled again — re-creating exactly the mixed-handler problem this feature set out to fix.

### Who
- **Maintainers and contributors**, who inherit the boundary as a CI fact.
- **Context**: CI pipeline; crate/module dependency graph and a lint pass.
- **Motivation**: make the boundary a compile-/CI-time guarantee, not a tribal rule.

### Solution
A CI check (crate-graph assertion and/or lint) that fails the build when (a) `foundry-web`
gains a dependency exposing the DB pool, or (b) `foundry-api` produces an HTML body. Wired
into the existing CI lane so a violating PR goes red.

### Domain Examples
#### 1. Happy path — clean PR passes
Jamal's template-only PR has no pool dependency in foundry-web and no HTML in foundry-api;
the guard passes silently.
#### 2. Edge case — web tier adds a pool dependency
A PR adds `sqlx` pool access to foundry-web; the crate-graph check fails with a message
naming the forbidden dependency.
#### 3. Error/boundary case — api returns HTML
A PR makes a foundry-api handler return an HTML error page; the guard (response-type and/or
content-type lint) fails the build.

### UAT Scenarios
```gherkin
Scenario: A clean boundary passes the guard
  Given foundry-web has no database pool dependency and foundry-api emits only JSON
  When the boundary guard runs in CI
  Then it passes

Scenario: A web-tier database dependency fails the guard
  Given a change makes foundry-web depend on the database connection pool
  When the boundary guard runs in CI
  Then it fails and names the forbidden dependency

Scenario: An HTML response from the API tier fails the guard
  Given a change makes a foundry-api handler return an HTML body
  When the boundary guard runs in CI
  Then it fails and identifies the offending handler or response type
```

### Acceptance Criteria
- [ ] CI fails when `foundry-web` depends on anything exposing the DB pool.
- [ ] CI fails when a `foundry-api` handler returns an HTML body.
- [ ] The guard runs in the existing CI lane; a clean boundary passes without manual steps.

### Outcome KPIs
- **Who**: Maintainers reviewing PRs that touch the tiers.
- **Does what**: Catch boundary violations automatically instead of by manual review.
- **By how much**: 100% of boundary-violating PRs fail CI before merge.
- **Measured by**: CI guard pass/fail history; injected-violation test in CI proves it bites.
- **Baseline**: 0 — no automated boundary enforcement exists.

### Technical Notes
- Exact mechanism (cargo-deny/cargo-modules/clippy lint/custom xtask) is DESIGN.
- This guard is what makes jtbd-web-2's "structural guarantee" claim true rather than aspirational.

### Size
**S** (1 day, 3 scenarios). Touches: CI config + a crate-graph/lint rule.

### Dependencies
US-W05a + US-W05c (a JSON read+write tier to guard against HTML leakage) and US-W01 (a web
tier to guard against DB reach). Ships folded into **Slice 2** (the write slice), NOT as a
standalone slice. The web≠DB half of the guard begins biting once US-W01 lands in Slice 3;
the api≠HTML half bites from Slice 1 onward.
