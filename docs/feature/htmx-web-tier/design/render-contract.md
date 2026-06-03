# htmx Web Tier (Feature B) — Render Contract

Owner: solution-architect (Morgan). This is DESIGN's Open Decision #2 (byte-identical vs
intentionally-improved markup) plus the binding spec for *how templated output stays acceptance-
green*. Interaction mode: **Propose**. Companion: `architecture.md`, `template-engine.md`,
`wave-decisions.md` (ADR-B02).

## What the acceptance suite actually asserts (grounding)

The DISCUSS framing ("acceptance scenarios assert on HTML substrings; templating changes
whitespace/markup" — risk register, HIGH) is **looser than the code**. Reading the suite:

- **The structural assertions go through `scraper` (a real DOM parser).**
  `crates/foundry-acceptance/src/support/html_assertions.rs` parses with
  `scraper::Html::parse_fragment` and matches **CSS selectors** + **trimmed visible text**:
  - `assert_has(body, css)` — selector must match ≥1 element (`:29`).
  - `assert_comment_has_element_with_text(...)` — finds `.comment[data-author="…"]`, then an inner
    selector whose `text.trim() == expected.trim()` (`:79-101`). **Whitespace-insensitive.**
  - `collect_attributes(body, css, attr)` — e.g. the US-12 keyboard test reads `[data-issue-key]`
    attribute values **in document order** (`:42-48`).
  - `assert_not_has(...)` — the XSS scenario asserts `script` tags are absent (`:52`).
- **The copy/marker assertions are `body.contains("…")` substrings**, e.g. `us_06_signin.rs:273`
  asserts `body.contains("Invalid email or password")`; `us_08_file_issue.rs:386,390` assert the
  issue key and `"Backlog"` are present; `:407` finds `"Backlog"` then asserts the key appears
  **after** it in document order.

**Conclusion:** the contract is **NOT byte-for-byte whitespace identity.** It is:

> **The Render Contract = the set of (a) CSS-selectable elements + attributes, (b) `data-*` scraper
> markers, (c) `hx-*` directives and `hx-swap-oob` targets, and (d) literal text/copy substrings that
> the acceptance suite reads.** Templates must reproduce these faithfully; incidental whitespace,
> indentation, and attribute ORDER within a tag are free to change.

## Decision #2 — byte-identical vs intentionally-improved markup

Two strategies were weighed:

**Option 2a — "selector-and-substring-identical" (move only; defer visual improvement). RECOMMENDED.**
Each surface's template reproduces the *asserted contract* exactly — same elements, classes, `data-*`
markers, `hx-*` directives, and literal copy — while internal whitespace/formatting is whatever the
template naturally emits. **No acceptance scenario is edited** during Slices 1-3 (and Slice 4 keeps
`data-*` byte-stable). Visual/CSS improvement is layered via `static/` CSS and the base-layout
chrome (US-B02/B04), which changes *appearance* without changing the asserted DOM contract.

- Cost/risk: **lowest.** The suite is the regression net and it stays untouched; a green run *is* the
  proof of equivalence. The one intentional behavior change in scope — the comment OOB card gaining
  Edit/Delete affordances (US-B03, fixing `render_comment_card_oob`'s omission) — is covered by a
  NEW scenario (the live-vs-reloaded structural-equality scenario), not by editing an existing one.
- This is what the NFRs point to: NFR-WEBB-COMPAT-01 ("the passing count does not drop") and
  NFR-WEBB-COMPAT-02 ("asserted substrings + `data-*` markers preserved") are satisfied *by
  construction* if no asserted thing moves.

**Option 2b — intentionally-improved markup with acceptance-test updates (rejected for the move).**
Rewrite the markup to a cleaner/semantic shape and update the scenarios to match.

- Cost/risk: **high and self-defeating during the move.** Editing the suite while moving the markup
  destroys the regression net at the exact moment it is most needed — you can no longer tell "the
  template broke behavior" from "we changed the test." The DISCUSS risk register flags this directly.
- Improvement is not forbidden — it is simply **sequenced after** the move, as its own change with
  its own scenario updates, outside Feature B's "keep the suite green" mandate. (Out-of-scope.md
  defers the design-system/responsive/theming work anyway.)

**Recommendation: 2a.** Move with a selector-and-substring-identical contract; defer visual rework to
CSS and to post-feature slices. Visual *appearance* improves immediately (vendored CSS + base
layout); the asserted DOM *contract* does not move.

## The contract, surface by surface (what each template MUST preserve)

### Board (`board.html` + `partials/issue_card.html`, US-B01/B02)
- Column headings literal text: `Backlog`, `Todo`, `In-Progress`, `Done`
  (`projects.rs:44`, asserted `us_08_file_issue.rs:390`).
- `<section class="column" data-column="{slug}">` per column — slug = lowercased, `-`→`_`
  (`projects.rs:521`). `data-column` is a scraper marker; `data-column='backlog'` is also the OOB
  target (`issues.rs:285`).
- Issue card: `<article class="issue-card" data-issue-key="{KEY}">` with a `.key` and `.title` span
  (`issues.rs:272`). `data-issue-key` drives the US-12 ordering assertion.
- Hidden keyboard carrier `<ul id="kb-items" hidden aria-hidden="true">` containing
  `<li data-issue-key="{KEY}">` **sorted ASCENDING by issue number** (`projects.rs:535-547`) —
  NFR-WEBB-A11Y-01 + the `collect_attributes([data-issue-key])` order check. The ASC sort stays in
  the handler/view-model (it is data ordering, not markup); the template just renders the list.
- Empty column body — today `<p class="empty">No issues yet</p>` (`projects.rs:509`). US-B01 scenario
  2 lets this become an *inviting* empty state ("press c to file the first one"); the only asserted
  thing is "guidance explaining how to file" → keep a recognizable empty-state element. This is the
  one place markup text intentionally grows; no existing scenario asserts the old bare string.
- Issue-create OOB fragment: `<div hx-swap-oob="beforeend:[data-column='backlog']" …>` wrapping the
  SAME `issue_card` partial (`issues.rs:283`). Asserted: body contains the key AND `"Backlog"`, key
  after `"Backlog"` in order (`us_08_file_issue.rs:386-414`).
- State-change fragment: `<span class="state" data-state="{state}">{state}</span>`
  (`issues.rs:146`).

### Issue page + comments (`issue.html` + `partials/comment_card.html` + edit-form, US-B03)
- Issue header `<h1>{KEY}</h1>`; comment thread container
  `<section class="comments" data-comment-list>` (`comments.rs:709`) — `data-comment-list` is the OOB
  append target.
- Comment card: `<article id="comment-{id}" class="comment" data-author="{email}"
  data-comment-id="{id}">` with `<header class="comment-author">`, `<div class="comment-body">`
  (embeds `body_html` **verbatim/`|safe`** — already sanitized in core), `.comment-actions`, and the
  `(edited)` `<small class="comment-edited-marker">` when `row.edited` (`comments.rs:812-821`,
  `:781`). The `.comment[data-author=…]` selector is the scraper's entry point (`html_assertions.rs:66`).
- Edit affordance `<button class="comment-edit-button" hx-get hx-target="#comment-{id}"
  hx-swap="outerHTML">Edit</button>` emitted iff `can_edit` (author); Delete
  `<button class="comment-delete-button" hx-delete …>Delete</button>` iff `can_delete`
  (author or admin) (`comments.rs:794-811`). **Flags computed in the handler**; the template only
  renders them (NFR-WEBB-BND-03).
- **OOB-card fix (US-B03):** the live-append OOB wrapper now includes the SAME `comment_card`
  partial with the same flags, so the live card carries the same affordances as the reloaded card —
  eliminating `render_comment_card_oob`'s deliberate omission (`comments.rs:841`). Covered by the new
  live-vs-reloaded structural-equality scenario.
- Edit-form fragment: `<form id="comment-{id}" class="comment-edit-form" hx-patch hx-target
  hx-swap="outerHTML"><textarea name="body_markdown">…</textarea>` + Save + a Cancel button
  (`hx-get` back to the single-comment URL) (`comments.rs:262-266`).
- Error fragments — literal copy preserved exactly:
  - 400: `<div class="error" data-hx-fragment="comment-create-error">{msg}</div>`
    (`comments.rs:623`); `issue-create-error` (`issues.rs:255`); `project-create-error`
    (`projects.rs:488`); `"Title is required"` (`issues.rs:94`).
  - 403: `data-hx-fragment="comment-forbidden"`, `"You may only edit your own comments."`
    (`comments.rs:650`/`:573`).
  - 410: `data-hx-fragment="comment-deleted-notice"`,
    `"This comment has been deleted. Refresh to see the latest state."` (`comments.rs:634`).
  - delete-OK: `data-hx-fragment="comment-deleted"` (`comments.rs:428`).
- Attachments: `.attachments-empty` block when none (`comments.rs:730`), `data-attachment-list`
  (`:755`) — scraper markers; preserved.

### Sign-in + forgot (`signin.html` + `forgot.html`, US-B04)
- `<input type="hidden" name="_csrf" value="{token}">` — the handler passes the token; the template
  emits the field (see §CSRF). Form `method="post" action="/sign-in"`; email + password inputs
  with `required` and **associated labels** (a11y, NFR-WEBB-A11Y-02).
- Error: `<p class="error">Invalid email or password</p>` — the literal `GENERIC_SIGNIN_ERROR`
  (`signin.rs:30`), asserted by `body.contains` (`us_06_signin.rs:273`). Non-enumerable: same string
  for unknown-email and wrong-password (NFR-WEBB-COMPAT-05) — unchanged, it is decided in the handler.
- Forgot page + the "if that email is on file, a reset link has been sent" response copy
  (`signin.rs:225`).

## CSRF emission in templates (NFR-WEBB-COMPAT-03, DB7 — invariant)

The CSRF mechanism is **100% unchanged** — `csrf.rs` middleware, the non-HttpOnly `foundry_csrf`
cookie (`build_csrf_cookie`), the `hx-csrf`/`x-csrf-token` header path, the constant-time compare,
and the `/bootstrap` exemption all stay byte-identical. The ONLY templating responsibility is: **emit
the hidden `_csrf` field carrying the token the handler already computes**, exactly as
`render_signin_form` does today (`signin.rs:305`).

- The handler continues to call `ensure_csrf_cookie(&state, &headers)` (sets the cookie on GET if
  absent) and passes the token into the view-model: `views::SigninPage { csrf, error, ... }`.
- The template renders `<input type="hidden" name="{{ CSRF_FORM_FIELD }}" value="{{ csrf }}">`. The
  field name `_csrf` is the constant `CSRF_FORM_FIELD` (`csrf.rs:27`) — expose it to the template as
  a literal or a view-model field; do not hardcode a divergent name.
- htmx mutating calls keep sending the `hx-csrf` header (an Alpine/htmx hook reads the non-HttpOnly
  cookie) — that is client behavior in the vendored JS (`assets.md`), not a template concern, and is
  unchanged.
- **Escaping:** the token is rendered through Askama's default auto-escape (matches today's
  `html_escape(csrf_token)`, `signin.rs:297`). The one place auto-escape is deliberately bypassed is
  the comment `body_html` (already sanitized by `foundry-core`) — Askama `|safe`.

## The one-partial rule (US-B03, NFR-WEBB-MAINT-02)

`issue_card.html` and `comment_card.html` each have **exactly one definition**. Every render path
includes it:
- full-page board/issue render → `{% include %}` the partial in a `{% for %}`;
- htmx-append (OOB) → an `oob/*_oob.html` wrapper that includes the SAME partial inside
  `<div hx-swap-oob="…">`;
- edit-rerender + cancel (single-comment) → render the partial directly.
This makes the live-vs-reloaded structural-equality scenario green by construction and is verifiable
by code inspection (one partial file per component).

## Render budget (≤200 ms P95, NFR-WEBB-PERF-01)

- Askama compiles templates into the binary → render is buffer-writing Rust, no runtime parse, no
  file I/O. Expected parity with the `format!` baseline.
- **No DB in the render path** (NFR-WEBB-BND-01): the view-model is fully materialized from
  `foundry-services` before rendering; the template touches no pool.
- **Gate:** a `criterion` bench on the board render (template view-model → HTML) vs the `format!`
  baseline, plus the synthetic HTTP load profile (50 RPS, 1,000 issues seeded) reusing the
  backend-mvp NFR-PERF-01 harness. **Slice 1 (the board, the hottest surface) is where this is
  proven first** (DB3 walking-skeleton). An engine choice that misses the budget would be rejected —
  Askama is not expected to.

## How this stays green (the discipline)

1. Move one surface's markup into a template + view-model; compute the same flags in the handler.
2. Run `cargo test -p foundry-acceptance` — the `[Summary]` passing count must not drop
   (NFR-WEBB-COMPAT-01). A drop means the template moved an asserted element/marker/copy — fix the
   template, not the test.
3. Do NOT edit existing scenarios during the move (Option 2a). New behavior (the OOB-card affordance
   fix) gets a NEW scenario.
4. Slice 4 leaves every `data-*` marker byte-stable while normalizing `hx-*` and bumping htmx
   (`htmx2-migration.md`).
