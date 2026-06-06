<!-- markdownlint-disable MD024 -->
# Remaining-Surfaces Templating — User Stories

> MOVE-ONLY refactor reusing Feature B's shipped Askama engine, `base.html`,
> vendored `/static` assets, `views.rs` view-models, and the
> selector-and-substring-identical render contract
> (`docs/feature/htmx-web-tier/design/render-contract.md`). Every story is a
> per-surface instance of the SAME jobs Feature B validated — `htmx-web-1`
> (edit markup in a template, not Rust) and `htmx-web-2` (the screen looks like
> a product, styled from vendored assets, offline). No new jobs, no new infra,
> no new behavior. The `foundry-acceptance` suite is the binding regression net.

## System Constraints (cross-cutting — apply to every story)

- **Selector-and-substring-identical contract** (Feature B render-contract.md):
  templates reproduce the SAME CSS-selectable elements, `data-*` markers,
  `hx-*` directives, and literal copy the suite asserts on. Incidental
  whitespace/attribute-order is free to change. NO existing acceptance scenario
  is edited.
- **Existing acceptance suite stays green**: `cargo test -p foundry-acceptance`
  `[Summary]` passing count must not drop after any slice (NFR-WEBB-COMPAT-01).
- **Fragment vs full page** (Feature B split): htmx fragments (modals, error
  divs, OOB swaps, state `<span>`) keep emitting BARE fragments — they must NOT
  re-wrap in `base.html` or the htmx swap double-wraps. Only FULL pages extend
  `base.html`.
- **No DB in the render path** (NFR-WEBB-BND-01); sanitization/authz stay in
  core/handler (NFR-WEBB-BND-03); web tier gains no DB pool.
- **Browser auth / CSRF / sessions UNCHANGED** (NFR-WEBB-COMPAT-03/04/05): the
  `_csrf` hidden field, `/bootstrap` CSRF exemption, redirect-when-signed-out,
  and all status codes (200/400/401/403/413/SEE_OTHER) move byte-for-behavior.
  Only markup moves.
- **No JS toolchain, no CDN, one binary**: reuse the already-vendored
  `/static` assets; add no new runtime service (NFR-WEBB-INFRA-01).
- **Render budget ≤200 ms P95, no regression** (NFR-WEBB-PERF-01): Askama is
  compiled-in; expected parity with `format!`.
- **Engine is already chosen**: Askama (Feature B ADR-B01). DESIGN here is
  near-trivial — which existing partial/base block each surface reuses.

---

## US-R01: Project-create form and its error fragment render from a template

### Problem
Jamal Okafor is a Foundry contributor who wants to reword the project-create
form ("Key prefix" → "Project key") and make it look like the rest of the now-
styled app. Today the entire form is an inline `format!()` string in
`projects.rs::render_create_form` (a bare `<!doctype><html><head><title>` with
no stylesheet), and the validation error is a second inline `format!()` in
`render_error_fragment`. He must edit Rust and recompile to change a label, and
the page renders unstyled next to Feature B's styled board.

### Who
- Jamal Okafor (contributor) | editing project-create markup/wording | wants markup in `templates/`, not handler `format!()`.
- Mei Chen (self-hoster member) | creating her team's first project | wants the form to look like the styled board, not raw HTML.

### Solution
Move `render_create_form` into a `project_create.html` Askama template that
extends the existing `base.html` (so it links the vendored `/static` stylesheet
+ htmx/Alpine), driven by a `views.rs` view-model carrying `team_name`,
`action`, `csrf_token`, `error`, `raw_name`, `raw_key`. Move
`render_error_fragment` into a small error-fragment template (or shared partial)
keeping the `data-hx-fragment="project-create-error"` marker byte-stable. The
fragment stays a bare fragment; only the full form page extends `base.html`.

### Elevator Pitch
- **Before**: a contributor edits a Rust `format!()` literal in `projects.rs` and recompiles to change the project-create form, which renders unstyled with a bare `<head>`.
- **After**: visiting `GET /team/{slug}/projects/new` returns the project-create form rendered from `project_create.html` extending `base.html` — the page links the vendored `/static` stylesheet and shows the same name + key-prefix inputs, the `_csrf` hidden field, and (on a bad submit) the `data-hx-fragment="project-create-error"` error div — selector-identical to today.
- **Decision enabled**: a contributor can reword or restyle the project-create form by editing one template file, and a self-hoster sees a styled, consistent create form — they decide the form is trustworthy and submit it.

### Domain Examples
#### 1: Happy path — Jamal restyles, Mei creates
Jamal changes the "Key prefix" label to "Project key" by editing
`project_create.html` only. Mei (signed into team "Platform") opens the create
form, sees it styled like the board, types name "Billing" key "BILL", submits,
and lands on the new project board.
#### 2: Edge case — validation error fragment
Mei submits name "Billing" with an empty key. The handler returns the
`project-create-error` fragment ("Key prefix is required") rendered from the
template; htmx swaps it in; the `data-hx-fragment="project-create-error"` marker
is unchanged so any scraper/Alpine hook still finds it.
#### 3: Boundary — duplicate key conflict
Mei submits key "BILL" which already exists; the handler returns the 409 form
re-render carrying `raw_name`/`raw_key` pre-filled and the inline error — the
template re-renders the same fields with the same values, no behavior change.

### UAT Scenarios (BDD)
#### Scenario: Project-create form renders styled from the template
Given Mei Chen is signed into team "Platform"
When she opens the project-create form
Then the page links the vendored `/static` stylesheet via the base layout
And it shows the project-name and key-prefix inputs and the `_csrf` hidden field
And the acceptance suite's project-create scenarios stay green

#### Scenario: A contributor rewords the form by editing only a template
Given Jamal wants to change the "Key prefix" label to "Project key"
When he edits `project_create.html` and rebuilds
Then the new label appears with no change to `projects.rs` handler logic

#### Scenario: Invalid submission returns the byte-stable error fragment
Given Mei submits the form with an empty key prefix
When the handler rejects it
Then she sees an error fragment carrying `data-hx-fragment="project-create-error"`
And the marker is identical to the previous `format!()` output

### Acceptance Criteria
- [ ] `render_create_form` markup lives in `project_create.html` extending `base.html`; the page links `/static` assets.
- [ ] The error fragment renders from a template keeping `data-hx-fragment="project-create-error"` byte-stable.
- [ ] `_csrf` field, `method=post`, form `action`, and the name/key inputs are selector-identical.
- [ ] `cargo test -p foundry-acceptance` passing count does not drop; no existing scenario edited.
- [ ] No inline HTML `format!()` remains in `render_create_form`/`render_error_fragment`.

### Outcome KPIs
- **Who**: contributors editing the project-create surface
- **Does what**: change the form's markup/wording by editing a template, not handler Rust
- **By how much**: 100% of project-create on-screen text greppable in `templates/`, 0 in `projects.rs`
- **Measured by**: code inspection / grep for on-screen strings
- **Baseline**: today 0% (all in `projects.rs::render_create_form`)

### Technical Notes
- job_id: htmx-web-1. Slice 1 (Walking Skeleton).
- Reuses Feature B Askama + `base.html` + `views.rs` pattern; near-trivial DESIGN.
- Dependency: Feature B shipped (engine, base, assets) — resolved.

---

## US-R02: New-issue modal renders from a template/partial

### Problem
Jamal wants to restyle the `c`-to-create new-issue modal. Today the modal markup
is an inline `format!()` in `keyboard.rs::render_modal_fragment`, and the no-JS
full-page fallback is a second inline `format!()` (`render_modal_full_page`) with
a bare `<head>`. Editing the modal means editing Rust twice and the full-page
fallback is unstyled.

### Who
- Jamal Okafor (contributor) | restyling the new-issue modal | wants the modal markup in one template/partial.
- Mei Chen (self-hoster member) | pressing `c` to file an issue, or submitting without JS | wants a styled modal and a styled fallback page.

### Solution
Move the modal markup into one `partials/new_issue_modal.html` partial. The htmx
fragment path renders the bare partial (no `base.html` — it is swapped into the
page). The no-JS full-page path renders a page that extends `base.html` and
`{% include %}`s the SAME partial (one-partial rule, NFR-WEBB-MAINT-02).
Preserve `data-modal="new-issue"`, `role="dialog"`/`aria-modal`, the `_csrf`
field, the form `action`, and the title `autofocus` input.

### Elevator Pitch
- **Before**: a contributor edits two Rust `format!()` blocks in `keyboard.rs` to restyle the new-issue modal, and the no-JS fallback renders with a bare `<head>`.
- **After**: pressing `c` (or `GET …/issues/new`) returns the new-issue modal from `partials/new_issue_modal.html` — the fragment swaps in selector-identical (`data-modal="new-issue"`, role/aria, `_csrf`, title input), and the full-page fallback wraps the SAME partial in `base.html`.
- **Decision enabled**: a contributor restyles the modal by editing one partial; a self-hoster sees a styled modal and decides to file the issue.

### Domain Examples
#### 1: Happy path — Mei files via the modal
Mei presses `c` on the Platform board; htmx loads the modal fragment; she types
"Login button misaligned", submits, and the new issue card appears.
#### 2: Edge case — no-JS full-page fallback
Mei has JS disabled; she navigates to the new-issue URL directly; the full page
extends `base.html` (styled) and includes the same modal form; submitting posts
to the identical `action`.
#### 3: Boundary — one partial, two paths
Jamal adds a placeholder to the title input in `new_issue_modal.html`; it appears
in BOTH the htmx modal and the full-page fallback, because both include the one
partial.

### UAT Scenarios (BDD)
#### Scenario: The new-issue modal swaps in selector-identical
Given Mei is on the Platform board
When she opens the new-issue modal via htmx
Then the swapped fragment carries `data-modal="new-issue"`, `role="dialog"`, the `_csrf` field, and the title input
And the keyboard/create acceptance scenarios stay green

#### Scenario: The no-JS fallback page is styled and shares the modal partial
Given Mei has JavaScript disabled
When she opens the new-issue URL directly
Then the full page extends `base.html` and links `/static`
And it includes the same modal form posting to the same `action`

#### Scenario: One partial drives both paths
Given Jamal edits `partials/new_issue_modal.html`
When he rebuilds
Then the change appears in both the htmx modal and the full-page fallback

### Acceptance Criteria
- [ ] Modal markup lives in ONE `partials/new_issue_modal.html`; both paths include it.
- [ ] Fragment path emits a bare fragment (no `base.html`); full-page path extends `base.html`.
- [ ] `data-modal`, role/aria, `_csrf`, `action`, and the `autofocus` title input are selector-identical.
- [ ] Acceptance suite passing count does not drop; no existing scenario edited.
- [ ] No inline HTML `format!()` remains in `render_modal_fragment`/`render_modal_full_page`.

### Outcome KPIs
- **Who**: contributors editing the new-issue modal
- **Does what**: restyle the modal by editing one partial (not two Rust blocks)
- **By how much**: 1 partial definition for both render paths; 0 inline modal `format!()`
- **Measured by**: code inspection
- **Baseline**: today 2 inline `format!()` blocks in `keyboard.rs`

### Technical Notes
- job_id: htmx-web-1. Slice 2.
- One-partial rule (NFR-WEBB-MAINT-02) generalized to the modal.
- Optional fold-in: `render_search_fragment` + `show_keyboard_help` overlay (same module, same pattern, lowest risk) — fold here if cheap, else defer.

---

## US-R03: Issue-create-error and state-change fragments render from templates

### Problem
The issue-create validation error ("Title is required") is an inline `format!()`
in `issues.rs::bad_request_fragment`, and the state-change response is an inline
`<span class="state" data-state="…">` `format!()`. To reword the error or adjust
the state chip, Jamal edits Rust. These are tiny, but they are the last inline
fragments on the issue-create hot path.

### Who
- Jamal Okafor (contributor) | rewording the create error or restyling the state chip | wants the markup in templates.
- Mei Chen (self-hoster member) | filing an issue with a missing title, or changing an issue's state | sees consistent, styled fragments.

### Solution
Move `bad_request_fragment` into an error-fragment template (or the shared error
partial from US-R01) keeping `data-hx-fragment="issue-create-error"` and the
"Title is required" copy byte-stable. Move the state-change `<span>` into a tiny
state-fragment template keeping `class="state" data-state="{state}"` byte-stable.
Both stay bare fragments.

### Elevator Pitch
- **Before**: a contributor edits inline `format!()` in `issues.rs` to reword "Title is required" or restyle the state chip.
- **After**: submitting an issue with no title returns the `data-hx-fragment="issue-create-error"` fragment from a template carrying "Title is required" byte-stable; changing state returns a `<span class="state" data-state="{state}">` from a template.
- **Decision enabled**: a contributor rewords/restyles these fragments in templates; a self-hoster reads the styled error and decides to add a title and resubmit.

### Domain Examples
#### 1: Happy path — state change renders the chip
Mei drags issue BILL-3 to "In-Progress"; the response is a `<span class="state" data-state="in-progress">In-Progress</span>` rendered from the template; the chip updates in place.
#### 2: Edge case — missing title error
Mei submits a new issue with an empty title; she gets the
`data-hx-fragment="issue-create-error"` fragment with "Title is required",
byte-stable, swapped in by htmx.
#### 3: Boundary — invalid state value
Mei's client posts an unknown state; the handler returns "Invalid issue state"
through the same error-fragment template; behavior unchanged.

### UAT Scenarios (BDD)
#### Scenario: Missing-title error renders from a template byte-stable
Given Mei submits a new issue with an empty title
When the handler rejects it
Then she sees the `issue-create-error` fragment with the literal "Title is required"
And the marker and copy match the previous `format!()` output

#### Scenario: State change renders the state chip from a template
Given Mei changes issue BILL-3's state to "In-Progress"
When the handler responds
Then she receives `<span class="state" data-state="in-progress">` rendered from a template
And the issue-state acceptance scenarios stay green

### Acceptance Criteria
- [ ] `bad_request_fragment` renders from a template; `data-hx-fragment="issue-create-error"` + "Title is required" byte-stable.
- [ ] State-change `<span>` renders from a template; `class="state" data-state` byte-stable.
- [ ] Both remain bare fragments (no `base.html`).
- [ ] Acceptance suite passing count does not drop; no existing scenario edited.
- [ ] No inline HTML `format!()` remains in these two sites.

### Outcome KPIs
- **Who**: contributors editing issue-create/state fragments
- **Does what**: reword/restyle these fragments in templates, not Rust
- **By how much**: 0 inline fragment `format!()` left in `issues.rs` render sites
- **Measured by**: code inspection
- **Baseline**: today 2 inline `format!()` fragments

### Technical Notes
- job_id: htmx-web-1. Slice 3.
- May reuse the shared error-fragment partial introduced in US-R01.

---

## US-R04: Dashboard landing and the events sign-in-required page extend base.html

### Problem
The signed-in landing page (`signin.rs::dashboard_root`, `GET /`) is an inline
`format!()` bare-`<head>` "Foundry / You are signed in" page — the FIRST thing a
self-hoster sees after sign-in, and it is unstyled. The events endpoint's
sign-in-required page (`events.rs::unauthorized_response`) is likewise a bare
inline `<!doctype>` string. Both clash with the now-styled board.

### Who
- Mei Chen (self-hoster member) | landing on `/` after sign-in, or hitting the events endpoint signed-out | wants a styled, coherent page.
- Jamal Okafor (contributor) | editing the landing/events copy | wants it in a template.

### Solution
Move `dashboard_root`'s signed-in body into a `dashboard_root.html` template
extending `base.html`; keep the signed-out `SEE_OTHER`→`/sign-in` redirect
unchanged in the handler. Move the events sign-in-required body into a template
extending `base.html`, preserving the copy and the `/sign-in` link and the 401
status. Both are full pages → extend `base.html`.

### Elevator Pitch
- **Before**: after signing in, a self-hoster lands on `/` showing an unstyled bare-`<head>` "Foundry / signed in" page; the events sign-in page is likewise unstyled.
- **After**: `GET /` (signed in) returns `dashboard_root.html` extending `base.html` — styled, linking `/static`; the events sign-in-required page renders from a template extending `base.html` with the same copy and `/sign-in` link and 401 status.
- **Decision enabled**: a self-hoster's first post-sign-in impression is a styled, trustworthy landing — they decide Foundry is a real product and keep using it.

### Domain Examples
#### 1: Happy path — styled landing after sign-in
Mei signs in and is taken to `/`; she sees a styled "Foundry" landing consistent with the board, not raw HTML.
#### 2: Edge case — signed-out redirect unchanged
An unauthenticated request to `/` still gets `303 SEE_OTHER` to `/sign-in` (no body change, handler logic untouched).
#### 3: Boundary — events page signed-out
Mei (session expired) hits the events endpoint; she gets the 401 sign-in-required page rendered from the template, styled, with the working `/sign-in` link.

### UAT Scenarios (BDD)
#### Scenario: Signed-in landing is styled
Given Mei is signed in
When she opens `/`
Then the landing renders from `dashboard_root.html` extending `base.html` and links `/static`

#### Scenario: Signed-out landing still redirects
Given a request to `/` with no session
When the handler runs
Then it returns `303 SEE_OTHER` to `/sign-in` with no body change

#### Scenario: Events sign-in-required page is styled with the right status
Given Mei's session has expired
When she requests the events endpoint
Then she gets a 401 page rendered from a template extending `base.html`
And it contains the "sign-in required" copy and a `/sign-in` link

### Acceptance Criteria
- [ ] `dashboard_root` signed-in body renders from `dashboard_root.html` extending `base.html`; signed-out redirect unchanged.
- [ ] Events sign-in-required page renders from a template extending `base.html`; 401 status and copy + `/sign-in` link preserved.
- [ ] Acceptance suite passing count does not drop; no existing scenario edited.
- [ ] No inline HTML `format!()` remains in `dashboard_root`/`unauthorized_response`.

### Outcome KPIs
- **Who**: self-hosters landing on `/` or the events page
- **Does what**: see a styled page instead of unstyled raw HTML
- **By how much**: 2 highest-visibility bare-`<head>` pages now extend `base.html`; 0 unstyled landing pages remain
- **Measured by**: code inspection + visual check (page links `/static`)
- **Baseline**: today both bare `<!doctype>` inline strings, no stylesheet

### Technical Notes
- job_id: htmx-web-2. Slice 4.
- Handler control flow (redirect, 401) is untouched; only the rendered bodies move.

---

## US-R05: Attachment surfaces render from templates

### Problem
The attachment surfaces are all inline `format!()` in `attachments.rs`: the
upload-error fragment (`data-hx-fragment="attachment-upload-error"`), the OOB
attachment-row swap (`hx-swap-oob="beforeend:[data-attachment-list]"` wrapping a
`<li class="attachment">`), the "Upload too large" 413 page (bare `<head>`), and
the not-found page. Restyling an attachment row or the too-large page means
editing Rust, and the full pages are unstyled.

### Who
- Jamal Okafor (contributor) | restyling the attachment row or upload pages | wants them in templates/partials.
- Mei Chen (self-hoster member) | uploading a file, or hitting the size limit | sees styled rows and styled error pages.

### Solution
Move the attachment-row markup into `partials/attachment_row.html`; the OOB path
wraps the SAME partial in a `<div hx-swap-oob="beforeend:[data-attachment-list]">`
keeping the target byte-stable. Move the upload-error fragment into the shared
error-fragment template keeping `data-hx-fragment="attachment-upload-error"`
byte-stable. Move `payload_too_large` (413) and `not_found_page` into full-page
templates extending `base.html`, preserving status codes and copy.

### Elevator Pitch
- **Before**: a contributor edits inline `format!()` in `attachments.rs` to restyle the attachment row, the upload-error, or the unstyled "Upload too large" page.
- **After**: uploading a file appends a `<li class="attachment" data-filename="…">` from `partials/attachment_row.html` via the byte-stable `hx-swap-oob="beforeend:[data-attachment-list]"` target; the upload-error fragment and the 413/404 pages render from templates (full pages extend `base.html`).
- **Decision enabled**: a contributor restyles attachment surfaces in templates; a self-hoster sees styled rows + error pages and decides whether to retry a too-large upload.

### Domain Examples
#### 1: Happy path — upload appends a styled row
Mei uploads "spec.pdf" to issue BILL-3; the OOB swap appends a `<li class="attachment" data-filename="spec.pdf">` from the partial into `[data-attachment-list]`, byte-stable target.
#### 2: Edge case — too-large upload
Mei uploads a 50 MB file over a 25 MB limit; she gets the 413 "Upload too large" page rendered from a template extending `base.html`, styled, with the same copy and status.
#### 3: Boundary — upload-error fragment
Mei uploads an empty file; she gets the `attachment-upload-error` fragment from the shared error template, marker byte-stable.

### UAT Scenarios (BDD)
#### Scenario: Uploaded attachment appends a styled row via the byte-stable OOB target
Given Mei uploads "spec.pdf" to an issue
When the upload succeeds
Then a `<li class="attachment" data-filename="spec.pdf">` is appended via `hx-swap-oob="beforeend:[data-attachment-list]"`
And the attachment acceptance scenarios stay green

#### Scenario: Too-large upload shows a styled 413 page
Given Mei uploads a file over the configured size limit
When the handler rejects it
Then she gets a 413 page rendered from a template extending `base.html`
And the "Upload too large" copy and the 413 status are unchanged

#### Scenario: Upload error fragment renders byte-stable
Given Mei's upload is rejected as a bad request
When the handler responds
Then she sees the `data-hx-fragment="attachment-upload-error"` fragment from a template

### Acceptance Criteria
- [ ] Attachment row lives in `partials/attachment_row.html`; OOB wrapper includes it; `hx-swap-oob` target + `.attachment`/`data-filename` byte-stable.
- [ ] Upload-error fragment renders from a template; `data-hx-fragment="attachment-upload-error"` byte-stable.
- [ ] `payload_too_large` (413) and `not_found_page` render from full-page templates extending `base.html`; status codes + copy preserved.
- [ ] Acceptance suite passing count does not drop; no existing scenario edited.
- [ ] No inline HTML `format!()` remains in the attachment render sites.

### Outcome KPIs
- **Who**: contributors editing attachment surfaces
- **Does what**: restyle attachment row + error/limit pages in templates
- **By how much**: 0 inline attachment `format!()` HTML; 1 attachment-row partial reused by full + OOB paths
- **Measured by**: code inspection
- **Baseline**: today ~4 inline `format!()` sites in `attachments.rs`

### Technical Notes
- job_id: htmx-web-1. Slice 5.
- Second OOB-fragment move (after Feature B's create-card / comment-card); same one-partial discipline.

---

## US-R06: Bootstrap, claim, invite, and the shared invalid_page extend base.html

### Problem
The first-run/bootstrap surfaces are all inline `format!()` bare-`<head>` pages:
`bootstrap.rs::dashboard` ("Workspace dashboard"), `render_claim_form` (the
workspace-claim form), the `create_invite` invite-link page, and the shared
`signin.rs::invalid_page` helper that every not-found/error path across handlers
renders. A self-hoster's very FIRST screens (claim, dashboard) are unstyled, and
every not-found page in the app is unstyled because they all funnel through one
inline helper.

### Who
- Mei Chen (self-hoster member) | first-run claim, bootstrap dashboard, inviting a teammate, or hitting a not-found page | wants styled first-run + error pages.
- Jamal Okafor (contributor) | editing first-run copy or the shared error page | wants it in templates.

### Solution
Move `bootstrap.rs::dashboard`, `render_claim_form`, and the `create_invite`
invite-link page into full-page templates extending `base.html`, preserving the
`_csrf` field, the `/bootstrap?token=…` action, the `/bootstrap` CSRF exemption,
and the signed invite URL. Move `signin.rs::invalid_page` into a shared
`invalid_page.html` template extending `base.html` parameterized by heading +
message — restyling EVERY not-found/error path at once (high leverage). All full
pages → extend `base.html`.

### Elevator Pitch
- **Before**: a self-hoster's first-run claim and dashboard pages, the invite-link page, and every not-found page render as unstyled bare-`<head>` `format!()` HTML.
- **After**: `GET /bootstrap` renders the claim form from a template extending `base.html` (styled, `_csrf` + `/bootstrap` action preserved); the bootstrap dashboard, the invite-link page, and the shared `invalid_page` (heading + message) all extend `base.html`.
- **Decision enabled**: a self-hoster's first-run experience looks like a real product, so they decide to claim the workspace and invite teammates; a contributor edits first-run/error copy in templates.

### Domain Examples
#### 1: Happy path — styled claim then invite
Devansh runs `docker compose up`, opens `/bootstrap`, sees a styled claim form, claims the workspace; later he invites Mei and the invite-link page renders styled with the signed URL.
#### 2: Edge case — shared not-found page restyled everywhere
Mei mistypes a team slug; the not-found page (rendered via the shared
`invalid_page` template) is now styled — and so is EVERY other not-found path,
because they all use the one shared template.
#### 3: Boundary — CSRF exemption preserved
The `/bootstrap` POST stays CSRF-exempt and the `_csrf` field still renders in
the claim form template; auth behavior unchanged.

### UAT Scenarios (BDD)
#### Scenario: Claim form renders styled with auth contract preserved
Given Devansh opens `/bootstrap` on a fresh install
When the claim form renders
Then it extends `base.html` and links `/static`
And it carries the `_csrf` field and posts to `/bootstrap?token=…` exactly as before
And the bootstrap acceptance scenarios stay green

#### Scenario: The shared not-found page is styled everywhere at once
Given Mei requests a non-existent team slug
When the not-found page renders
Then it renders from the shared `invalid_page.html` extending `base.html`
And every handler that uses `invalid_page` now renders styled

#### Scenario: Invite-link page renders styled with the signed URL intact
Given Devansh creates an invite
When the invite-link page renders
Then it extends `base.html` and shows the signed invite URL unchanged

### Acceptance Criteria
- [ ] `bootstrap.rs::dashboard`, `render_claim_form`, and the invite-link page render from full-page templates extending `base.html`.
- [ ] Shared `invalid_page` renders from `invalid_page.html` extending `base.html`, parameterized by heading + message; all callers restyled.
- [ ] Claim form `_csrf`, `/bootstrap?token=…` action, `/bootstrap` CSRF exemption, and the signed invite URL preserved.
- [ ] Acceptance suite passing count does not drop; no existing scenario edited.
- [ ] No inline HTML `format!()` bare-`<head>` page remains anywhere in foundry-app after this slice.

### Outcome KPIs
- **Who**: self-hosters during first-run + anyone hitting a not-found page
- **Does what**: see styled first-run/bootstrap/error pages instead of raw HTML
- **By how much**: 0 bare-`<head>` `format!()` full pages remaining in foundry-app (feature-complete cut)
- **Measured by**: code inspection / grep for `<!doctype` in `format!` strings → 0
- **Baseline**: today ~4 inline bare-`<head>` first-run/error sites

### Technical Notes
- job_id: htmx-web-2. Slice 6 (finishes the cut).
- The shared `invalid_page` move is high-leverage: one template restyles all not-found paths.
- Dependency: none beyond Feature B (resolved). Closes out the deferred-surfaces list.
