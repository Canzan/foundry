# Walking Skeleton — form-error-display-contract

The load-bearing unknown is not the ~20-line handler — it is **whether the browser lane can prove the error is
visible**. The defect exists because no test could see the DOM (RCA Root Cause B). So the skeleton is the
instrument + the mechanism + one form, end to end in a real browser: **submit an invalid create → the error is
visible in the dialog.** If that can't be made to run and to fail-without-the-fix, nothing else here matters.

## First failing test (DELIVER entry)

**S1 — "The browser lane can observe a rejected submit end to end"** (the `@lane-probe @walking_skeleton`),
immediately followed by **S2** (invalid create shows the reason, creates nothing).

RED → GREEN (slice 01):
1. **RED (the defect, reproduced in a browser)**: with no `form-errors.js` and no `[data-error-slot]`, drive
   the browser: press `c` → Create with an empty title → assert "Title is required" is visible in the dialog.
   It is NOT (htmx discards the 400) → S1/S2 red. This is the first time the defect is caught by any test.
2. **GREEN, minimal**:
   - Add `static/js/form-errors.js`: one vanilla IIFE, `htmx:beforeSwap` listener; on `xhr.status` 400–499,
     resolve the triggering element's error slot (`data-error-target` selector, else `closest('form')`
     `[data-error-slot]`); if found, `shouldSwap = true; target = slot; isError = false`; else return.
   - Load it `<script defer>` from `base.html` (line 10, beside the other app JS). Set a readiness marker
     (e.g. `document.documentElement.dataset.formErrorsReady = "1"`) so the lane can wait on it, mirroring
     keyboard.js's `data-kb-ready`.
   - Add `<div data-error-slot></div>` inside `partials/new_issue_modal.html`'s form (inside `.modal-dialog`
     so it shows while the modal is open).
3. **GREEN** S3 (fix + resubmit succeeds — proves the slot-only swap preserved `_csrf`) and S4 (a valid create
   shows no error — blast-radius guard).
4. `cargo xtask smoke` (fmt/clippy/check-arch/workspace tests) + the `@needs-browser` lane green for slice 01;
   commit. Then DOGFOOD the real press-`c` → empty Create → visible error in a browser.

## Slice sequence

1. **Slice 01** (skeleton above) — `form-errors.js` + the browser DOM oracle + issue-create's slot. S1–S4.
   Ships the mechanism AND the instrument that proves it.
2. **Slice 02** — apply the contract: `[data-error-slot]` in `issue_edit_modal.html` + `comment_edit_form.html`;
   browser DOM scenarios S5 (issue edit), S6 (comment edit). Pure application of the proven contract; the
   handler is unchanged (it already routes any 4xx with a slot).
3. **Slice 03** — the edges: drag-revert message via the OOB `#toast` (board-dnd.js) + comment-create htmx +
   slot. S7, S8. DELIVER may defer.

## Lane safety

All scenarios `@pending` → excluded by `filter_run` from every lane (default, `all`, `@needs-browser`), so
`@all` stays green until DELIVER un-@pends each slice. `fail_on_skipped()` stays ON — an un-@pended scenario
with no step definition FAILS the lane rather than passing silently (the UI-4 lesson). Full `@all` at finalize.

## Why the oracle is the deliverable (not just the handler)

The handler is small and obvious. The durable value is the **DOM-level browser oracle**: it is the only thing
that can distinguish "the server returned the right error" (already true, already green) from "the user saw
it" (the actual bug). Every covered form gets one, so this class of defect — a correct response the browser
silently drops — cannot ship green again for a form under the contract. That is the point of the feature.
