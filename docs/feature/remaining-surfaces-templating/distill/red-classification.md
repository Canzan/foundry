# DISTILL RED Classification — Remaining-Surfaces Templating

Owner: acceptance-designer (Sentinel). The pre-DELIVER fail-for-the-right-reason
gate: every NEW scenario was run once and classified. DELIVER reads this file at
its RED-phase entry (ADR-025) to confirm RED is genuine
(MISSING_FUNCTIONALITY), not a setup/import/fixture error.

## How to reproduce

```
FOUNDRY_ACCEPTANCE_TAGS=all cargo test -p foundry-acceptance --test acceptance
```

(The `@remaining-surfaces` scenarios run in the default lane too; `@all` is used
here only to run the full regression net in the same pass and confirm it stays
green. Requires Docker for the shared testcontainers Postgres.)

## Result — last run

```
183 scenarios (175 passed, 8 failed)
8 steps failed
```

All **8 failing scenarios are in `feature_remaining_surfaces`** (verified: no
failing step matched any other module). The **175 passing** include the entire
pre-existing regression net — `us-05`/`us-07`/`us-08`/`us-11`/`us-12` stayed green,
confirming the MOVE's regression net is intact and no existing scenario was edited
(NFR-WEBB-COMPAT-01).

## Per-scenario classification

| # | Scenario | Classification | Evidence |
|---|---|---|---|
| 1 | US-R01 WS "styled, templated project-create form" | ✅ MISSING_FUNCTIONALITY | `links were []; body: <!doctype…><head>…` — no `<link>` today (`render_create_form` bare head) |
| 2 | US-R02 WS "no-script new-issue page is a styled full page" | ✅ MISSING_FUNCTIONALITY | `links were []` — `render_modal_full_page` bare head |
| 3 | US-R04 WS "styled dashboard landing" | ✅ MISSING_FUNCTIONALITY | body captured: `<!doctype html>…<h1>Foundry</h1>…` no `<link>` (`dashboard_root`) |
| 4 | US-R04 @error "styled sign-in-required events page" | ✅ MISSING_FUNCTIONALITY | body: `<!doctype…><p>Sign-in required…</p>` no `<link>`; 401 + `/sign-in` link GREEN guards passed |
| 5 | US-R05 @error "styled too-large page" | ✅ MISSING_FUNCTIONALITY | 413 + "Upload too large" copy GREEN; `links were []` — `payload_too_large` bare head |
| 6 | US-R06 WS "styled bootstrap dashboard" | ✅ MISSING_FUNCTIONALITY | "Workspace dashboard" copy GREEN; `links were []` — `bootstrap::dashboard` bare head |
| 7 | US-R06 @error "shared styled not-found page" | ✅ MISSING_FUNCTIONALITY | `<h1>`/`<p>` shape GREEN; `links were []` — shared `invalid_page` bare head |
| 8 | US-R07 @source-tree "no bare-head inline document" | ✅ MISSING_FUNCTIONALITY | "9 handler site(s) still emit a bare-`<head>` inline full-page": events:142, signin:243, bootstrap:213/285/341/358, keyboard:132, projects:479, attachments:355 |

## Scenarios that PASS today (GREEN guards — the move must keep them green)

These NEW scenarios pin byte-stable markers/copy/behaviour that the current
`format!()` output already satisfies. They are RED-ready in the sense that a
DELIVER move which drops a marker would turn them RED — they guard the
selector-and-substring-identical contract during the move:

| Scenario | What it pins |
|---|---|
| US-R01 @error project-create error fragment | `data-hx-fragment="project-create-error"` + bare-fragment guard |
| US-R03 @error issue-create error fragment | `data-hx-fragment="issue-create-error"` + "Title is required" + bare guard |
| US-R03 state chip | `<span class="state" data-state="in_progress">` + bare guard |
| US-R04 signed-out landing redirect | `303 SEE_OTHER → /sign-in`, empty body (handler control flow UNCHANGED) |
| US-R05 @error attachment upload-error fragment | `data-hx-fragment="attachment-upload-error"` + bare guard |

## Wrong-reason RED fixed during classification (test bugs, not behaviour)

The gate caught and fixed three wrong-reason failures BEFORE handoff:
1. **Ambiguous step** — `changes the state of "X" to "Y"` collided with
   `us_09_realtime_sse`. Renamed to `moves "X" to the "Y" state from the board`.
2. **Multipart CSRF** — the attachment upload posted `_csrf` as a form field; the
   CSRF middleware reads the `x-csrf-token` HEADER for multipart bodies (per
   `us_11::perform_upload`). Fixed to send the header. (Symptom: 403 "CSRF token
   missing or mismatched" instead of the upload-error fragment / 413.)
3. **Wrong normalized value** — the state chip asserts `in_progress` (underscore),
   not `in-progress` (hyphen); the handler normalizes with an underscore.

After these fixes, every failing scenario is category-1 (MISSING_FUNCTIONALITY) or
a GREEN guard — **zero category-2/3 (setup/fixture/wrong-shape) failures remain**.
The gate PASSES: handoff to DELIVER is unblocked.

## Gate verdict

PASS — all 8 RED scenarios fail for MISSING_FUNCTIONALITY; the regression net
(175 scenarios) is green; no existing scenario edited.
