# Walking Skeleton — board-new-issue

The skeleton is already shipped server-side (modal endpoint + create POST + OOB Backlog card). No new
load-bearing abstraction. This feature is one thin client-wiring slice.

## First failing test (DELIVER entry)

**S1 — "The New issue button is wired to open the modal"** (`@board-new-issue`).

RED → GREEN path:
1. **RED (S1)**: un-@pend S1; author the step glue (fetch board, `scraper`-assert the button `hx-get` + target
   + a `#modal-root` container). Fails — `board.html:5` is `<button data-action="new-issue">` with no htmx.
2. **RED (S2)**: fetch the modal, assert the form `hx-post`. Fails — the modal form is a plain POST.
3. **GREEN**: edit `board.html` (button `hx-get="…/issues/new" hx-target="#modal-root" hx-swap="innerHTML"` +
   add `<div id="modal-root"></div>`) and `partials/new_issue_modal.html` (form gains `hx-post="{{ action }}"
   hx-target="#modal-root" hx-swap="innerHTML"`, keeps `method=post`/`action`/`_csrf`). S1+S2 green.
4. **S3/S4/S5**: un-@pend and confirm the shipped contracts (OOB card, error fragment) + the no-JS fallback
   (S5) — S5 proves the plain-form path still works after adding `hx-post`.
5. **REFACTOR / COMMIT**: fmt + full-workspace release clippy; commit.
6. **DOGFOOD**: on the live board, click New issue, file one, confirm the card lands in Backlog + the modal
   closes with no reload (the interactive layer the HTTP harness can't drive).

## Slice sequence

One slice (`slice-01-new-issue-button`). No ordering concerns.

## Lane safety

All 5 scenarios ship `@pending` (excluded by `acceptance.rs filter_run`), so `@all` stays green until DELIVER.
Full `@all` verification runs at finalize (Docker), per the repo loop.
