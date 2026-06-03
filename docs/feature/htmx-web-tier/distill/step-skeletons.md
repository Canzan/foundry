# DISTILL Step Skeletons + DELIVER Wiring List — Feature B "htmx Web Tier"

Mirrors `docs/feature/web-tier-extraction/distill/step-skeletons.md` (Feature A).
The step definitions live in
`crates/foundry-acceptance/src/steps/feature_b_web_tier.rs` (already written,
compiling, RED). This doc is the **precise list of what DELIVER must wire to
flip each RED scenario GREEN** — the contract between DISTILL and DELIVER.

## What DISTILL scaffolded vs deferred

**Scaffolded (this wave):**
- `crates/foundry-acceptance/src/steps/feature_b_web_tier.rs` — full step module
  (Given/When/Then), driving the real in-process router over reqwest. Compiles;
  9 scenarios fail RED for MISSING_FUNCTIONALITY (see `red-classification.md`).
- `world.rs` — the `b_*` per-scenario state fields.
- `lib.rs` + `tests/acceptance.rs` — module registration + force-link.
- 5 `.feature` files under `tests/features/us-b0*.feature` (+ copies under
  `distill/features/`).

**Deliberately NOT scaffolded (and why):**
- **No Askama template files, no `views.rs`, no `#[derive(Template)]` structs.**
  Askama derives are compile-time: an empty/missing referenced template or a
  half-wired `views` struct would BREAK the workspace build (askama is declared
  in `Cargo.toml:38` but absent from `Cargo.lock` — wiring it is a DELIVER task).
  The acceptance steps drive HTTP only and reference NO Askama type, so the RED
  comes entirely from missing functionality (no `/static` route, empty
  `static/`, the OOB bug, unvendored htmx) WITHOUT needing any Rust scaffold.
  This keeps the workspace compiling and the existing suite green — the
  surgical choice the brief asked for.
- **No `static/` blobs, no `/static` route.** Same reason — these are the
  MISSING functionality the RED scenarios prove is absent.
- **No render-failure test seam.** The US-B01 clean-500 scenario records the
  intent (`b_force_template_failure`); DELIVER wires the seam (below).

## DELIVER wiring list (RED → GREEN, per slice)

### Slice 1 — US-B06 pipeline + US-B01 board + US-B02 assets

1. **Wire Askama** (`askama` + `askama_axum`) into `foundry-app`; add `askama.toml`
   with `dirs = ["templates"]`. Re-run `cargo deny check` (MIT/Apache-2.0).
   Adds `askama`/`askama_axum` to `Cargo.lock`. (DD1 / ADR-B01.)
2. **Add `crates/foundry-app/src/views.rs`** — the typed `#[derive(Template)]`
   view-models: `BaseLayout`, `BoardPage`, `IssuePage`, `SigninPage`,
   `ForgotPage`, + `IssueCard`/`CommentCard` partials. (DD3 consequence.)
3. **`templates/base.html`** — the ONE base layout: `<head>` with the vendored
   `<link rel="stylesheet" href="/static/css/foundry.css">` and
   `<script src="/static/vendor/htmx.min.js">` + `.../alpine.min.js`, header
   chrome, `{% block content %}`. **This is what flips the asset-reference RED
   (US-B01 WS / US-B04) green.** The asset paths the steps probe are
   `/static/vendor/htmx.min.js`, `/static/vendor/alpine.min.js`,
   `/static/css/foundry.css` — use EXACTLY these names (or update the step
   constants in `feature_b_web_tier.rs` to match the chosen content-hashed
   names; see `assets.md` §cache-busting — the version-pinned vendor names and a
   hashed css name are both acceptable as long as the layout references and the
   served paths agree).
4. **`templates/board.html`** (extends base; renders columns + `{% include
   partials/issue_card.html %}`; keep the `#kb-items` hidden carrier ASC-sorted,
   `data-column`, `data-issue-key`; grow the empty-state to "press c to file the
   first one" — flips the US-B01 empty-state RED). Rewire `projects::show_board`
   → build `views::BoardPage` → render. Delete `render_board`'s `format!`.
5. **Vendor the blobs** into `crates/foundry-app/static/`:
   `vendor/htmx.min.js` (htmx 1.x for Slice 1 — bumped to 2.x in Slice 4;
   US-B02 only needs it served + non-empty), `vendor/alpine.min.js`,
   `css/foundry.css` + `VENDOR.md` (provenance + sha256). **Flips US-B02 served-
   asset RED green** (non-empty, correct content type, cache header).
6. **Mount `/static`** in `build_router` (`lib.rs:175`-ish):
   `.nest_service("/static", ServeDir::new("static"))` + a `SetResponseHeader`
   layer for `Cache-Control: public, max-age=31536000, immutable`. **Flips the
   404 + cache-header + traversal RED green** (ServeDir refuses `..` by
   construction — flips the US-B02 traversal `@error` green).
7. **Render-failure → clean 500 seam** (US-B01 @error): add a `cfg(any(test,
   feature="test-support"))` `AppState` flag (parallel to `db_unreachable`) that
   makes the board view render return `Err`; the handler maps render errors to a
   clean `500` (never a half-page). Wire the `board_template_fails` Given to set
   it (DELIVER replaces the `b_force_template_failure` placeholder with the real
   flag). (error-and-observability.md.)
8. **Asset-resolution probe** (US-B02 missing-asset, build-time): an `xtask
   check-assets` subcommand (or a `#[test]`) parsing the base layout's
   `/static` refs and asserting each file exists. (The runtime 404 scenario is
   already covered; this is the CI guard against a forgotten rename.)

### Slice 2 — US-B03 comment partial + the OOB-affordance bug fix

9. **`templates/partials/comment_card.html`** — the ONE comment-card definition:
   `<article id="comment-{id}" class="comment" data-author=… data-comment-id=…>`
   + `.comment-author` (+ `(edited)` marker) + `.comment-body` (`{{ body_html
   |safe }}` — already sanitized in core) + `.comment-actions` rendering the
   Edit (`comment-edit-button`, iff `can_edit`) and Delete
   (`comment-delete-button`, iff `can_delete`) buttons with their `hx-*`.
10. **`templates/partials/oob/comment_card_oob.html`** — wraps the SAME partial
    in `<div hx-swap-oob="beforeend:[data-comment-list]">`, passing the SAME
    `can_edit`/`can_delete` flags. **THIS is the bug fix** — it removes
    `comments.rs::render_comment_card_oob`'s deliberate affordance omission
    (`comments.rs:841`). **Flips the US-B03 WS live-vs-reloaded RED green.**
    The handler must compute `can_edit`/`can_delete` for the OOB path too (the
    actor is the author by construction on a fresh post, so `can_edit=true`).
11. **`templates/issue.html`** + `comment_edit_form.html` + `errors/*` fragments
    (400/403/410 with EXACT copy preserved). Rewire `comments::show_issue` /
    `submit_comment` / `submit_edit_comment` / `show_edit_form` /
    `show_single_comment` to render the partial. Delete the four `format!` sites.
    Affordance flags + sanitization stay in the handler/core (NFR-WEBB-BND-03).

### Slice 3 — US-B04 sign-in / forgot

12. **`templates/signin.html`** + **`forgot.html`** (extend base; emit the
    hidden `_csrf` field with the handler-supplied token; the error slot shows
    `GENERIC_SIGNIN_ERROR`). Rewire `signin::show_form` / `show_forgot_form`.
    **Flips the US-B04 stylesheet-reference + base-layout RED green.** The CSRF
    middleware, cookie attrs, brute-force delay, non-enumerable error are
    UNCHANGED (DB7) — the regression-guard scenarios stay green by not touching
    them. NOTE: the forgot page is served at `GET /forgot-password` (the step
    GETs that path); confirm the route path matches `signin.rs` (adjust the step
    or the route to agree).

### Slice 4 — US-B05 htmx 2

13. **Swap the vendored htmx blob** to the pinned latest-stable **htmx 2.0.x**
    `.min.js` (DD7); update `VENDOR.md` + the base-layout reference. **Flips the
    US-B05 "version 2" RED green** — the step asserts the served bytes report a
    2.x version (`2.0.`/`@2.`/`"2.`). Normalize the active `hx-*` directives to
    one consistent convention across the partials; leave every `data-*` marker
    byte-stable (the US-B05 marker scenario guards this).
14. Confirm the hx-driven regression scenarios (issue-file OOB, comment
    post/edit) stay green under htmx 2 (US-B05 regression net, DB4).

## Step-method → wiring cross-reference (the 9 RED steps)

| RED step (feature:line) | Flips green when DELIVER ships |
|---|---|
| us-b01:47 board links stylesheet/scripts | step 3 (base.html asset refs) + step 6 (/static route) |
| us-b01:57 empty-state guidance | step 4 (board.html grown empty state) |
| us-b01:73 clean 500 | step 7 (render-failure seam + clean-500 mapping) |
| us-b02:35 asset served (htmx/Alpine/CSS) | step 5 (vendor blobs) + step 6 (ServeDir) |
| us-b02:46 non-empty htmx script | step 5 (real blob, not stub) |
| us-b03:46 live==reloaded affordances | step 10 (OOB wrapper includes shared partial w/ flags) |
| us-b04:40 sign-in links stylesheet | step 12 (signin.html extends base) |
| us-b04:65 forgot links stylesheet | step 12 (forgot.html extends base) |
| us-b05:38 served htmx is v2 | step 13 (vendor htmx 2.0.x blob) |

## Asset path contract (must agree between layout, ServeDir, and steps)

The steps probe these exact paths:
- `/static/vendor/htmx.min.js`
- `/static/vendor/alpine.min.js`
- `/static/css/foundry.css`

`assets.md` recommends version/content-hash in the committed filename
(`htmx-2.0.4.min.js`, `foundry.<sha>.css`). If DELIVER uses hashed names, EITHER
(a) keep a stable `htmx.min.js`/`foundry.css` symlink/copy ServeDir resolves,
OR (b) update the four path constants at the top of `feature_b_web_tier.rs`
(`request_htmx_asset` / `request_alpine_asset` / `request_css_asset`) to the
chosen names. The render-contract requirement is only that the base-layout
reference and the served path AGREE — the asset-resolution probe (step 8) is the
CI guard for that agreement.
