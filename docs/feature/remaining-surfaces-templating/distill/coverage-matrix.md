# DISTILL Coverage Matrix — Remaining-Surfaces Templating (`remaining-surfaces-templating`)

Mirrors `docs/feature/htmx-web-tier/distill/coverage-matrix.md` (Feature B).
Maps every US-R0x surface to a NEW scenario OR records it as
**regression-net-only** (the move is covered by the EXISTING suite staying green
— NFR-WEBB-COMPAT-01 — so no new RED scenario is authored).

This is a **move-only refactor** inheriting Feature B's render contract
(selector-and-substring-identical, ADR-B02). The binding rule from DESIGN
`render-contract.md`: *do NOT re-assert existing green output*. The existing
`us-07-project-create` (project create), `us-12-keyboard-nav` (modal fragment),
`us-11-attachments` (attachment listing + 413 status), `us-05-bootstrap` (claim +
invite), `us-08-file-issue` (issue create + board) suites ARE the regression net
for the unchanged markup. New RED scenarios cover ONLY the genuine user-visible
deltas the DESIGN `render-contract.md` flagged as **GAP** or **PARTIAL**.

## Lane / tag legend

| Tag | Meaning |
|---|---|
| `@remaining-surfaces` | all scenarios in this feature |
| `@walking_skeleton` | the demo-proof scenario for the slice (one per full-page slice) |
| `@real-io` | real in-process axum router + real Postgres (Architecture of Reference: driving + driven-internal real) |
| `@driving_adapter` | entered through the HTTP surface (form/landing/events/upload route) |
| `@error` | error / edge path |
| `@source-tree` | a source-tree contract (no HTTP) — mirrors feature_b's on-disk asset count check |
| `@completion-check` | the north-star KPI guard (0 inline `format!()` full pages) |
| `@slice1..6` | release slice |
| `@acme` | uses the shared Acme workspace fixture |

No `@skip`/`@pending` marker is used — matching the existing harness convention
(Feature A + Feature B). Scenarios run live; RED comes from MISSING_FUNCTIONALITY
(a full page emits a bare `<head>` with no `<link>` today; the completion guard
counts the inline sites that remain).

## Surface classification (from DESIGN render-contract.md §coverage)

| Surface | DESIGN status | DISTILL action |
|---|---|---|
| US-R01 project-create form (full page) | **GAP** (bare `<head>`, no `<link>`) | NEW WS scenario — links `/static` via base |
| US-R01 `project-create-error` fragment | PARTIAL (marker not asserted) | NEW @error scenario — pin marker + bare-fragment guard |
| US-R02 new-issue modal fragment | COVERED (`us_12`) | regression-net-only |
| US-R02 modal full-page (no-JS) fallback | **GAP** (no scenario) | NEW WS scenario — links `/static` via base; same dialog/csrf/autofocus |
| US-R03 `issue-create-error` fragment | PARTIAL | NEW @error scenario — pin marker + literal copy + bare guard |
| US-R03 state-change chip `data-state` | PARTIAL | NEW scenario — pin `data-state` value + bare guard |
| US-R04 dashboard_root `/` landing | **GAP** (signed-in body + signed-out 303) | NEW WS scenario (styled) + NEW regression guard (303 unchanged) |
| US-R04 events 401 page | PARTIAL (body not asserted) | NEW @error scenario — styled body + `/sign-in` link + 401 preserved |
| US-R05 attachment row (OOB) | COVERED (`us_11` listing) | regression-net-only |
| US-R05 413 too-large | COVERED status; PARTIAL body | NEW @error scenario — styled body + copy + 413 preserved |
| US-R05 `attachment-upload-error` fragment | PARTIAL | NEW @error scenario — pin marker + bare guard |
| US-R06 claim form + invite | COVERED (`us_05`) | regression-net-only |
| US-R06 bootstrap dashboard | PARTIAL (copy not asserted) | NEW WS scenario — styled + "Workspace dashboard" copy |
| US-R06 shared `invalid_page` | PARTIAL | NEW @error scenario — shared error-page shape + styled (covers ~17 callers) |
| (completion) 0 inline `format!()` full pages | KPI | NEW @source-tree guard |

## Per-story coverage

### US-R01 — Project-create form + error fragment (Slice 1, Walking Skeleton)

| AC | Scenario | RED reason / status |
|---|---|---|
| `render_create_form` markup lives in a template extending `base.html`; links `/static` | **US-R01 WS "opens a styled, templated project-create form"** | RED — today `projects.rs::render_create_form` (:466) emits a bare `<head>` with no `<link>`. |
| Error fragment renders from a template; `data-hx-fragment="project-create-error"` byte-stable | **US-R01 @error "byte-stable project-create error fragment"** | Marker pinned (PARTIAL → asserted) + bare-fragment guard. Passes GREEN today (the move must keep it). |
| `_csrf`, name/key inputs selector-identical | folded into the WS scenario (`input[name=name]`, `[name=key_prefix]`, hidden `_csrf`) | Asserted on the form. |
| Existing project-create scenarios stay green; no scenario edited | (regression-net-only) | `us-07-project-create` — the binding regression net. |
| No inline HTML `format!()` remains | (completion-check / code-inspection) | Covered by US-R07. |

### US-R02 — New-issue modal (fragment + full-page fallback) (Slice 2)

| AC | Scenario | RED reason / status |
|---|---|---|
| Modal markup lives in ONE partial; both paths include it | (regression-net-only / code-inspection) | `us_12` asserts the fragment; the one-partial rule is a DELIVER code-inspection AC. |
| Fragment path bare; full-page path extends `base.html` | **US-R02 WS "no-script new-issue page is a styled full page"** | RED — today `render_modal_full_page` (:124) emits a bare `<head>` with no `<link>`. |
| `data-modal`, role/aria, `_csrf`, `action`, autofocus title input selector-identical | folded into the WS scenario | Asserted on the full page. |
| Modal fragment stays green | (regression-net-only) | `us-12-keyboard-nav` (`input[name=title][autofocus]`). |

### US-R03 — issue-create-error + state-change fragments (Slice 3)

| AC | Scenario | RED reason / status |
|---|---|---|
| `bad_request_fragment` renders from a template; `issue-create-error` + "Title is required" byte-stable | **US-R03 @error "byte-stable issue-create error fragment"** | Marker + copy pinned (PARTIAL → asserted); bare guard. GREEN today (move must keep it). |
| State-change `<span>` byte-stable (`class="state" data-state`) | **US-R03 "byte-stable state chip"** | `data-state="in_progress"` pinned; bare guard. GREEN today (move must keep it). |
| Both stay bare fragments | the bare-fragment guard step on both | Enforced. |
| Existing issue-create scenarios stay green | (regression-net-only) | `us-08-file-issue`. |

### US-R04 — Dashboard landing + events sign-in page (Slice 4)

| AC | Scenario | RED reason / status |
|---|---|---|
| `dashboard_root` signed-in body extends `base.html`; signed-out redirect unchanged | **US-R04 WS "styled dashboard landing"** + **"signed-out request still redirects"** | RED — `signin.rs::dashboard_root` (:243) emits bare `<head>`, no `<link>`. The 303 guard is GREEN today (behaviour unchanged). |
| Events page extends `base.html`; 401 + copy + `/sign-in` link preserved | **US-R04 @error "styled sign-in-required events page"** | RED (styling) — `events.rs::unauthorized_response` (:138) bare `<head>`; the 401 + `/sign-in` link are GREEN guards. |
| Existing scenarios stay green | (regression-net-only) | `us-06-signin` / `us-09-realtime-sse`. |

### US-R05 — Attachment surfaces (Slice 5)

| AC | Scenario | RED reason / status |
|---|---|---|
| Attachment row in ONE partial; OOB target + `.attachment`/`data-filename` byte-stable | (regression-net-only) | `us-11-attachments` asserts the listed row. |
| Upload-error fragment from template; `attachment-upload-error` byte-stable | **US-R05 @error "byte-stable attachment upload-error fragment"** | Marker pinned (PARTIAL → asserted); bare guard. GREEN today (move must keep it). |
| `payload_too_large` (413) extends `base.html`; status + copy preserved | **US-R05 @error "styled too-large page with the unchanged status"** | RED (styling) — `attachments.rs::payload_too_large` (:353) bare `<head>`; 413 + "Upload too large" are GREEN guards. |
| `not_found_page` via shared `invalid_page` | (covered by US-R06 invalid_page) | Same shared template. |
| Existing attachment scenarios stay green | (regression-net-only) | `us-11-attachments`. |

### US-R06 — Bootstrap + claim + invite + shared invalid_page (Slice 6)

| AC | Scenario | RED reason / status |
|---|---|---|
| `bootstrap.rs::dashboard` extends `base.html` | **US-R06 WS "styled bootstrap dashboard"** | RED — `bootstrap.rs::dashboard` (:205) bare `<head>`; "Workspace dashboard" copy is a GREEN guard. |
| Shared `invalid_page` extends `base.html` (heading + message); all callers restyled | **US-R06 @error "shared styled not-found page"** | RED (styling) — `bootstrap.rs::invalid_page` (:356) bare `<head>`; the `<h1>`/`<p>` shape is a GREEN guard. One assert covers ~17 callers. |
| Claim form + invite URL + `/bootstrap` CSRF exemption preserved | (regression-net-only) | `us-05-bootstrap` (claim flow + signed invite URL). |
| No bare-`<head>` `format!()` page remains | **US-R07 completion-check** | RED until the whole cut lands. |

### Completion — north-star KPI (0 inline `format!()` full pages)

| AC | Scenario | RED reason / status |
|---|---|---|
| 0 bare-`<head>` `format!()` full pages remain in foundry-app | **US-R07 @source-tree "no handler emits a bare-head inline HTML document"** | RED — 9 inline sites today (events:142, signin:243, bootstrap:213/285/341/358, keyboard:132, projects:479, attachments:355). Flips GREEN only when DELIVER finishes the cut. |

## Adapter coverage (Mandate 6)

No NEW driven adapter is introduced by this feature (DESIGN: zero new infra). All
surfaces reuse Feature B's already-covered adapters; their `@real-io` coverage is
inherited.

| Driven adapter | `@real-io` scenario | Covered by |
|---|---|---|
| Static-asset serving (`tower_http::ServeDir` over `static/`) | YES (inherited) | Feature B US-B02 `@adapter-integration`; this feature's full-page scenarios link the same content-hashed CSS. |
| Postgres (data via `foundry_services`) | YES | every `@real-io` scenario here (real per-scenario schema). |
| Template engine (Askama, compiled-in) | n/a (not a runtime driven port) | compile-time presence probe + the green render scenarios. |
| Multipart upload (real `axum::extract::Multipart`) | YES (inherited) | `us-11-attachments` + this feature's US-R05 error scenarios. |

No `NO — MISSING` rows: this is a move-only feature with zero new adapters.

## Scenario counts

| File | Total | Happy/WS | Error/edge | RED today |
|---|---|---|---|---|
| us-r01-project-create | 2 | 1 | 1 | 1 |
| us-r02-new-issue-modal | 1 | 1 | 0 | 1 |
| us-r03-issue-fragments | 2 | 1 | 1 | 0 |
| us-r04-landing-events | 3 | 1 | 1 (+1 regression guard) | 2 |
| us-r05-attachments | 2 | 0 | 2 | 1 |
| us-r06-bootstrap | 2 | 1 | 1 | 2 |
| us-r07-completion-check | 1 | 0 | 1 (KPI guard) | 1 |
| **Total** | **13** | **5** | **8** | **8** |

Error/edge ratio: **8/13 ≈ 62%** (well above the 40% target). This is unusual for
a move-only refactor and is JUSTIFIED by scope discipline: the happy-path render
of every COVERED surface is already exhaustively covered by the existing
`us-05`/`us-07`/`us-08`/`us-11`/`us-12` suites (the binding regression net,
NFR-WEBB-COMPAT-01) which MUST stay green. Authoring duplicate happy-path
scenarios here would violate the render-contract discipline ("do NOT re-assert
existing green output", ADR-B02). The NEW scenarios therefore concentrate on the
genuine deltas — the GAP full pages (styling) and the PARTIAL markers/error pages
(byte-stability + styling) — which skew error/edge by construction.

## Walking skeleton

Exactly ONE `@walking_skeleton` for the whole feature (DISCUSS DR4 + story-map
Slice 1): **US-R01 "A member opens a styled, templated project-create form"**. It
is the cheapest surface that exercises every mechanic the later slices repeat —
a full page extending `base.html`, linking `/static`, emitting `_csrf`, plus a
`data-hx-fragment` error fragment. Green here proves the move pattern; every later
slice is a mechanical repeat (story-map §"Walking Skeleton").

The other full-page slices (US-R02 no-JS page, US-R04 landing, US-R06 bootstrap
dashboard) are demo-able `@driving_adapter` scenarios but are NOT tagged
`@walking_skeleton` — there is one skeleton per FEATURE, not per slice (the
surface list is the backbone, not an end-to-end flow, so one skeleton establishes
the pattern for all). US-R03 (fragments) and US-R05 (attachment error/limit) carry
no headline scenario — their value is byte-stability + styling guards.

## Regression-net-only surfaces (COVERED — no new scenarios)

The MOVE of these surfaces is proven by the existing suite staying green:
- project-create flow + 409 re-render → `us-07-project-create` (`us_07`).
- new-issue modal fragment + autofocus → `us-12-keyboard-nav` (`us_12`).
- attachment listing + 413 status → `us-11-attachments` (`us_11`).
- claim flow + signed invite URL → `us-05-bootstrap` (`us_05`).
- issue create + board → `us-08-file-issue` (`us_08`).
