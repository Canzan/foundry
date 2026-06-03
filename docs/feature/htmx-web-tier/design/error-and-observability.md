# htmx Web Tier (Feature B) — Render Errors & Observability

Owner: solution-architect (Morgan). Companion: `architecture.md`, `render-contract.md`, `assets.md`.
Mirrors Feature A's `error-and-observability.md` in shape. Scope is the rendering layer only —
business-error mapping (400/403/410 fragments, 404 pages) is UNCHANGED behavior carried from the
handlers and preserved byte-for-text by the render contract.

## Render-error handling — fail-safe, never a half-page (US-B06 scenario 2)

The hard rule: **a template failure produces a clean 500, never a partially-emitted page or
fragment.** Three layers make this true, earliest-first (Earned Trust):

1. **Compile time (the primary defense).** Askama type-checks every `#[derive(Template)]` at
   `cargo build`. A template referencing a non-existent field, a missing `{% include %}`, or a named
   template that does not exist is a **build error**. The class of "blank/broken page because the
   template was wrong" is eliminated before the binary exists — satisfying US-B06 scenario 3 at the
   compiler.
2. **Render time (the residual).** With templates compiled in and the view-model materialized before
   rendering, the only residual runtime failure is an I/O-free formatting error from
   `Template::render()` returning `Err`. Handlers map it centrally: `match tmpl.render() { Ok(html)
   => Html(html).into_response(), Err(e) => render_500(e) }`. The buffer is only written to the
   response on `Ok`, so a failed render **never emits partial bytes** — the client gets a complete
   500 (full-page request) or a 500 error fragment (htmx request), not a torn page.
3. **Response composition.** Because the engine returns a complete `String` before the handler builds
   the `Response`, there is no streaming-template half-write path. (This is a property of returning
   the rendered string, not streaming the template — keep it that way.)

`render_500` logs at `error` with the template name + the formatting error (no user data), mirroring
the existing `internal_error` helper pattern (`projects.rs:337`, `comments.rs:612`). For htmx
requests it returns a small error fragment so the swap target shows a clean message rather than
injecting a broken DOM.

## Business-error fragments are UNCHANGED (render contract)

The 400/403/410 fragments and the 404 pages are existing behavior; templating preserves their literal
copy and `data-hx-fragment` markers exactly (`render-contract.md` §"surface by surface"):
`"Title is required"`, `"You may only edit your own comments."`, `"This comment has been deleted.
Refresh to see the latest state."`, `"Invalid email or password"`, the `comment-create-error` /
`issue-create-error` / `project-create-error` / `comment-forbidden` / `comment-deleted-notice` /
`comment-not-found` markers. These are decided in the handler/service; the template renders the
string. No change.

## Asset-resolution probe (US-B02 scenario 3 — Earned Trust)

`xtask check-assets` (or a `#[test]` in `foundry-app`) parses the base layout's vendored
`<link href>`/`<script src>` `/static/...` references and asserts each file exists under `static/`
(optionally sha256-matching `VENDOR.md`). A stale/typo'd path **reds CI**; the broken-asset board
never ships. Gold test: rename a blob, assert the check goes red (the probe bites — Principle 12
self-application). Detail in `assets.md`.

## Render-timing metric (NFR-WEBB-PERF-01)

The MVP already has a per-request metrics tower layer
(`metrics_server::request_tracking_layer()`, wired in `lib.rs:277`) emitting
`http_requests_total{path,method,status}` + a duration histogram. **No new metric is required** — the
existing per-request duration histogram already covers the board/issue/sign-in render latency at the
application boundary, which is exactly what NFR-WEBB-PERF-01 measures (P95 ≤200 ms). Feature B's
templating must keep those histograms within budget; the `criterion` bench (`render-contract.md`
§budget) is the offline gate, the existing histogram is the online signal.

- **Optional (non-blocking):** if finer attribution is wanted, a `template_render_duration_seconds`
  histogram labelled by template name could be added behind the same metrics layer convention. DESIGN
  recommends NOT adding it for Feature B — the existing request histogram is sufficient and a new
  metric is unjustified surface (Principle 8). Flagged as optional in `wave-decisions.md`.

## No new external surface

No CDN, no third-party call on render → no new error class from an external dependency, no new
contract-test surface (`architecture.md` §External Integration Note). The only new failure modes are
the two above (render error → 500; missing asset → red CI), both fail-safe.
