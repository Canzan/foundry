# Remaining-Surfaces Templating — Render Contract

Owner: solution-architect (Morgan). This feature **inherits Feature B's render contract verbatim**:
the **selector-and-substring-identical** contract (ADR-B02,
`docs/feature/htmx-web-tier/design/render-contract.md`). Templates must reproduce the **asserted set**
— (a) CSS-selectable elements + attributes, (b) `data-*` scraper markers, (c) `hx-*` directives and
`hx-swap-oob` targets, (d) literal copy/substrings — while incidental whitespace and in-tag attribute
order are free to change. **No existing acceptance scenario is edited.** A green
`cargo test -p foundry-acceptance` (passing count not dropping, NFR-WEBB-COMPAT-01) IS the proof of
equivalence.

## The fragment-vs-full-page rule (the only render-shape rule)

Inherited from Feature B (nfrs.md §"Fragment vs full-page"):

- **Full pages** `{% extends "base.html" %}`, link the vendored `/static` stylesheet + htmx/Alpine,
  and own the `<head>`. The base layout is the single source of head/asset boilerplate.
- **Bare fragments** (htmx-swapped: modals, error divs, OOB rows, the state `<span>`) emit BARE markup
  and **MUST NOT** extend `base.html` — extending it double-wraps the swap and breaks the page.

Per-surface classification is in `architecture.md` §"Surface → template / view-model map".

## Per-surface contract (what each template MUST preserve, byte-stable where marked)

Verified against current code. The byte-stable markers/copy below are the
selector-and-substring-identical contract for these surfaces (nfrs.md §"byte-stable set").

### US-R01 — project-create (full page + error fragment)
- Full page: `method="post"`, form `action="/team/{slug}/projects"`, hidden `_csrf` field, the
  `name` and `key_prefix` text inputs with `required`, `raw_name`/`raw_key` repopulated values
  (`projects.rs:466-497`). Full page → extends `base.html` (adds the stylesheet the bare `<head>`
  lacked today).
- Error fragment: `<div class="error" data-hx-fragment="project-create-error">` byte-stable
  (`projects.rs:499-504`). Bare fragment.

### US-R02 — new-issue modal (fragment + full-page fallback)
- Fragment: `<div class="modal" role="dialog" aria-modal="true" data-modal="new-issue">`, the
  `_csrf` field, form `action`, and `input[name="title"][autofocus]` (`keyboard.rs:108-122`). Bare
  fragment, swapped in.
- Full-page fallback: extends `base.html`, `{% include %}`s the SAME modal partial
  (`keyboard.rs:124-141`). One-partial rule (NFR-WEBB-MAINT-02).
- Optional fold-ins (same module, same pattern): `render_search_fragment`
  (`ul.search-results`, `data-empty`, `li.search-result[data-issue-key]`, `keyboard.rs:226-242`) and
  `show_keyboard_help` (`section.keyboard-help[role=dialog]`, `dt[data-shortcut]`,
  `keyboard.rs:248-267`). Both bare fragments.

### US-R03 — issue-create error + state chip (both bare fragments)
- Error: `<div class="error" data-hx-fragment="issue-create-error">` + literal copy
  `"Title is required"` byte-stable (`issues.rs:253`, copy at `:94`).
- State chip: `<span class="state" data-state="{normalized}">{normalized}</span>` byte-stable
  (`issues.rs:147`).

### US-R04 — dashboard landing + events sign-in page (both full pages)
- `dashboard_root` signed-in body: heading `Foundry`, copy `"You are signed in. Welcome back."`
  (`signin.rs:243-247`) → `dashboard_root.html` extends `base.html`. The signed-out branch keeps
  `303 SEE_OTHER`→`/sign-in` with empty body — **handler control flow unchanged**
  (`signin.rs:249-253`).
- Events: 401 status + copy `"Sign-in required to subscribe to events."` + the `/sign-in` link
  (`events.rs:138-146`) → template extends `base.html`; **status code preserved**.

### US-R05 — attachment surfaces
- OOB row: `<li class="attachment" data-filename="{filename}">` with `.filename`/`.size` spans,
  wrapped in `<div hx-swap-oob="beforeend:[data-attachment-list]">` — the OOB target byte-stable
  (`attachments.rs:385-392`). Bare fragment; OOB wrapper `{% include %}`s the row partial.
- Upload error: `<div class="error" data-hx-fragment="attachment-upload-error">` byte-stable
  (`attachments.rs:369-375`). Bare fragment.
- 413 page: heading `"Upload too large"` + the limit copy, **413 status preserved**
  (`attachments.rs:353-362`) → extends `base.html`.
- Not-found: already delegates to the shared `invalid_page` (`attachments.rs:349-351`) → uses the
  shared `invalid_page.html`. Full page.

### US-R06 — bootstrap / claim / invite + shared invalid_page (all full pages)
- Claim form: `method="post"`, `action="/bootstrap?token={token}"`, the four required inputs
  (email/password/display_name/workspace_name), and the **`/bootstrap` CSRF exemption preserved**
  (`bootstrap.rs:338-353`). Note: the claim form has no `_csrf` field today because `/bootstrap` is
  CSRF-exempt — the template must NOT add one; reproduce the form as-is.
- Dashboard: heading `"Workspace dashboard"` + `"Signed in: {bool}"` copy (`bootstrap.rs:205-220`).
- Invite-link page: the signed invite URL `…/invites/accept?id=…&sig=…` rendered as a link, copy
  `"Share this URL to invite a teammate (valid for 7 days):"` (`bootstrap.rs:286-289`).
- Shared `invalid_page.html`: `<h1>{heading}</h1><p>{message}</p>`, parameterized by heading +
  message, status passed by the handler (`bootstrap.rs:356-363`). **High-leverage:** restyles every
  not-found/error path (7 modules, ~17 call sites) at once.

## CSRF in templates (NFR-WEBB-COMPAT-03 — invariant, inherited)

`csrf.rs` middleware, the `foundry_csrf` cookie, the `hx-csrf` header, the constant-time compare, and
the `/bootstrap` exemption are **100% unchanged**. The only template responsibility is to emit the
hidden `_csrf` field carrying the handler-supplied token on the CSRF-protected forms (US-R01
project-create, US-R02 new-issue). The claim form (US-R06) is **`/bootstrap`-exempt and has no `_csrf`
field today** — reproduce it without one. Token rendered through Askama default auto-escape.

## Existing acceptance coverage (per surface) + gaps to flag for DISTILL

Verified in `crates/foundry-acceptance`. The move stays selector-identical against these; surfaces
**without** existing coverage need NEW DISTILL scenarios (a move with no regression net is the one
real risk this feature carries).

| Surface | Existing coverage (evidence) | Status |
|---|---|---|
| US-R01 project-create form + 409 re-render | `us_07_project_create.rs` (create → 303 redirect, board columns, conflict path) | **COVERED** (page + redirect) |
| US-R01 `project-create-error` fragment | `us_07_project_create.rs` exercises invalid submits, but no explicit `data-hx-fragment="project-create-error"` assertion found | **PARTIAL — flag for DISTILL** (add a selector assert on the error fragment marker) |
| US-R02 new-issue modal fragment | `us_12_keyboard_nav.rs:231-253` (GET as htmx) + `:384-390` asserts `input[name="title"][autofocus]` | **COVERED** (fragment) |
| US-R02 modal full-page (no-JS) fallback | no scenario found exercising the non-htmx full-page path | **GAP — flag for DISTILL** (assert the full page extends base + includes the same form) |
| US-R02 search / keyboard-help overlays | `us_12_keyboard_nav.rs` search scenarios; help overlay coverage minimal | **PARTIAL** — only relevant if these optional fold-ins are taken |
| US-R03 issue-create error + state chip | `us_08_file_issue.rs` (issue create + board), `us_12` reads `[data-issue-key]` order | **PARTIAL — flag for DISTILL** (confirm/add an assert on `data-state` chip and `issue-create-error` marker) |
| US-R04 dashboard_root `/` landing | no scenario asserts the signed-in `/` body or the signed-out 303 | **GAP — flag for DISTILL** (assert styled landing + signed-out 303) |
| US-R04 events sign-in-required (401) | `us_09_realtime_sse.rs` likely exercises the events endpoint; 401-page body assertion not confirmed | **PARTIAL — flag for DISTILL** (assert 401 + `/sign-in` link copy) |
| US-R05 attachment row (OOB) | `us_11_attachments.rs:494-537` asserts the row is listed (`class="attachment"`, filename, size) | **COVERED** (listing); OOB-swap-specific assert not confirmed → **verify in DISTILL** |
| US-R05 413 too-large | `us_11_attachments.rs:388-394` asserts 413 status | **COVERED** (status); page body copy not asserted → minor |
| US-R05 upload-error fragment | not explicitly asserted on the `attachment-upload-error` marker | **PARTIAL — flag for DISTILL** |
| US-R06 claim form + invite | `us_05_bootstrap.rs` (claim happy path → redirect; invite link `…/invites/accept?id=`, signed token) | **COVERED** (claim flow + invite URL) |
| US-R06 bootstrap dashboard | `"Workspace dashboard"` body assertion not confirmed | **PARTIAL — flag for DISTILL** |
| US-R06 shared `invalid_page` | reused across ~17 call sites; the not-found paths have scattered coverage but no dedicated `invalid_page` structural assertion | **PARTIAL — flag for DISTILL** (one structural assert on `invalid_page.html` shape covers all callers) |

**DISTILL handoff — surfaces needing NEW scenarios (no/weak regression net):**
`dashboard_root` `/` landing (GAP), the new-issue modal full-page fallback (GAP), the
`project-create-error` / `issue-create-error` / `attachment-upload-error` fragment markers (PARTIAL),
the state-change chip `data-state` (PARTIAL), the events 401 page body (PARTIAL), the bootstrap
dashboard copy (PARTIAL), and one structural assertion on the shared `invalid_page.html` (PARTIAL).
For COVERED surfaces, the existing suite is the regression net — move only, do not edit those
scenarios.

## How this stays green (inherited discipline)

1. Move one surface's markup into a template + view-model; compute the same flags/values in the
   handler.
2. Run `cargo test -p foundry-acceptance` — the `[Summary]` passing count must not drop. A drop means
   the template moved an asserted element/marker/copy — fix the template, not the test.
3. Do NOT edit existing scenarios. For GAP/PARTIAL surfaces above, DISTILL adds NEW scenarios BEFORE
   or alongside the move so the move is not blind.
4. Fragments stay bare; full pages extend `base.html`. No htmx version bump (done in Feature B).
