# Evolution — comment-add-csrf (the issue-detail page's write forms were CSRF-broken)

**Finalized**: 2026-07-09
**Commits**: DELIVER (nw-bugfix flow — no DISCOVER/DISCUSS/DESIGN/DISTILL waves; a root-cause roadmap straight to
two TDD fix steps) — `9bca990` (01-01 add-comment CSRF issuance) → `4ad6e6e` (01-02 attachment-upload CSRF hook).
Trunk-based, committed to `main`. Feature dir PRESERVED. These two fixes landed before the
`navigation-bar-linear-ui` work and were left DELIVER-complete-but-unfinalized until now.
**Wave coverage**: bug fix only — `deliver/roadmap.json` + `execution-log.json` written; both steps show all
phases EXECUTED/PASS through the trailing COMMIT record. `RED_UNIT` was `SKIPPED (NOT_APPLICABLE)` on both steps
by decision: the fix is an HTTP/acceptance-level double-submit contract (port-boundary glue), not pure-domain
logic, so it is pinned end-to-end by the `@csrf` acceptance scenarios.
**Scope**: presentation/web tier only, inside the `foundry-app` adapter. No new crate, no DB migration, no new
dependency.

## Milestone — every write form on the issue page now works

Before this, the issue-detail page (`comments::show_issue`) was the one authed write surface that omitted the CSRF
double-submit **issuance** seam every other write-form page already used. Both forms on the page were dead on
arrival for a real browser: the add-comment POST was rejected by `csrf_middleware` ("CSRF token missing or
mismatched"), and the attachment upload had no way to send the token a multipart request requires. This closes
that gap so commenting and file upload actually function.

## Root cause

`comments::show_issue` never minted the non-HttpOnly `foundry_csrf` cookie and never rendered a hidden `_csrf`
field, so the double-submit check had nothing to compare. The existing us-10 acceptance test masked the bug: it
**hand-minted** a token from `GET /sign-in` and injected it into the form, exercising a path a browser never
takes. The real issue page issued no token at all.

## What shipped

### Step 01-01 — the add-comment form carries a usable CSRF token (`9bca990`)
- **Shared-helper dedupe (AGENTS.md: no duplication):** factored `ensure_csrf_cookie` +
  `response_with_optional_cookie` into `crate::csrf` as `pub(crate)`, and deleted the duplicated private copies in
  both `issues.rs` and `keyboard.rs`, repointing both to the shared helpers.
- Added `pub csrf: String` to `IssuePage` (`views.rs`) and rendered a hidden `<input type=hidden name=_csrf>` as
  the first child of the add-comment `<form>` in `templates/issue.html` (view-model + template landed together —
  Askama won't compile `{{ csrf }}` without the field).
- Threaded `HeaderMap` into `show_issue`, minted/reused the cookie via `ensure_csrf_cookie`, and returned through
  `response_with_optional_cookie` so the `Set-Cookie` rides along with the page.
- **Regression close:** the new `@csrf` us-10 scenario acquires the token from the REAL issue page (capture
  `foundry_csrf` from the GET's `Set-Cookie`, scrape the hidden `_csrf` field via the `scraper` crate) and POSTs
  with only those — replacing the `/sign-in` hand-mint that hid the bug. It failed pre-fix; all 17 us-10 comment
  scenarios are green post-fix.

### Step 01-02 — CSRF-protect the attachment upload form (`4ad6e6e`)
- **The multipart wrinkle:** `csrf_middleware`'s `is_multipart` branch requires the token in the `x-csrf-token`
  (or `hx-csrf`) **header**, never the urlencoded `_csrf` field. A plain `<form enctype="multipart/form-data">`
  cannot set a request header, so a browser upload always 403'd — even after 01-01 made the server side
  satisfiable. What was missing was the CLIENT sending the cookie value as a header.
- **JS-hook fix (mirrors `board-dnd.js`'s cookie→header idiom):** new CSP-safe static `static/js/csrf-upload.js`
  — on submit of a `[data-csrf-upload]` form it reads the non-HttpOnly `foundry_csrf` cookie and `fetch`-POSTs the
  multipart body with the `x-csrf-token` header, appending the OOB row on success and surfacing the server error
  fragment on non-2xx. Served with the existing `no-cache` revalidating header (`lib.rs` `static_cache_control_value`
  already matches `/js/*.js`, so edits reach browsers).
- Tagged the upload form `data-csrf-upload` (`templates/issue.html`) and loaded the hook in `templates/base.html`
  (defer).
- **Why the JS hook over htmx `hx-headers`:** the upload handler returns error fragments (400/413) that htmx
  swallows on non-2xx without the response-targets extension; the `fetch` hook surfaces those errors and matches
  the dogfooded `board-dnd.js` split. A cheap acceptance assertion pins that the script is served (mirroring
  card-ranking's "drag-and-drop script is served" scenario).

## Decisions realized
| Decision | Status |
|----------|--------|
| Reuse the existing `issues.rs` edit-dialog CSRF issuance seam rather than invent a new one | IMPLEMENTED |
| Factor `ensure_csrf_cookie` + `response_with_optional_cookie` into `crate::csrf`, dedupe issues.rs + keyboard.rs | IMPLEMENTED |
| Multipart CSRF handled with a CSP-safe static JS cookie→header hook, not htmx `hx-headers` | IMPLEMENTED |
| No pure-domain unit tests — the seam is an HTTP/acceptance-level contract, pinned end-to-end | IMPLEMENTED (RED_UNIT NOT_APPLICABLE) |

## Test coverage
- **us-10 (`@csrf`):** the add-comment form posts with the CSRF token the issue page itself mints — token acquired
  from the real page (Set-Cookie + scraped hidden field), not hand-minted from `/sign-in`. RED pre-fix, green
  post-fix; all 17 us-10 comment scenarios green.
- **us-11 (attachments):** GET issue page sets `foundry_csrf` → a multipart upload carrying `x-csrf-token` = that
  cookie value succeeds (pins the server contract; the browser gesture is dogfooded, same split as `board-dnd.js`).
  Plus a served-asset assertion for `csrf-upload.js`.
- **Static-asset cache header:** already covered by the existing `static_cache_control_value` unit test in
  `lib.rs` — no new unit needed.
- **Green gate:** `cargo fmt --all --check` + `cargo clippy --all-targets --release -D warnings` clean at each
  step; a full `cargo xtask ci` at the end.

## Follow-ups / deferred
- **Mutation testing: DEFERRED** — consistent with `navigation-bar-linear-ui`, the touched surface is web-adapter
  HTTP glue with no pure-domain unit, so it falls under the carried "nightly scoped mutation pass on the web
  adapter" item rather than an inline `cargo-mutants` run. The regression scenarios are behavior-pinned end-to-end.
- **The `keyboard.rs` copy** was deduped opportunistically in 01-01; no further callers of the old private helpers
  remain.
