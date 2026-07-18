# RCA — htmx form validation errors are invisible in the browser (app-wide)

**Origin**: escalated from `/nw-bugfix` (2026-07-18) on a defect surfaced while shipping
`new-issue-dialog-description`. Investigation by nw-troubleshooter (Toyota 5 Whys, read-only).
**Verdict**: **design-level flaw** in the app's error-display contract — user-approved **design-first**.
This document is the design-input problem statement for the DESIGN wave.

## Defect

An htmx-submitting form that fails validation returns a bare error fragment with a **4xx** status, but the
browser shows **nothing** — the form appears to do nothing. Input is preserved; the error message is discarded.

## Root cause chain (evidence)

- **WHY 1** — the 4xx error fragment is never inserted into the DOM. Forms declare
  `hx-target="#modal-root" hx-swap="innerHTML"` (`new_issue_modal.html:4`, `issue_edit_modal.html:4`) /
  `#comment-{id}` (`comment_edit_form.html:1`); on a 4xx the target is untouched.
- **WHY 2** — htmx 2.0.4 does not swap 4xx by default. `vendor/htmx.min.js`:
  `responseHandling:[{code:"204",swap:false},{code:"[23]..",swap:true},{code:"[45]..",swap:false,error:true}]`.
  The `[45]..` rule is `swap:false,error:true` — htmx fires `htmx:responseError` and swaps nothing.
- **WHY 3** — nothing in the app changes that: no `responseHandling` override, no `htmx.config`, no
  `htmx:beforeSwap`/`responseError` handler, no `response-targets` extension vendored. `base.html` loads
  htmx + board-dnd.js + csrf-upload.js + keyboard.js; none register a global error-swap.
- **WHY 4** — the error contract was authored assuming htmx swaps on 4xx. `board-new-issue`
  `wave-decisions.md` D3: *"the error swaps INTO the modal … visible, board untouched."* False under the
  shipped htmx config; the handlers implement D3 faithfully (`issues.rs` `bad_request_fragment` →
  `(StatusCode::BAD_REQUEST, Html(fragment))`).
- **WHY 5 / ROOT CAUSE A** — the swap-on-4xx behavior D3 depends on was never implemented; there is no
  error-display contract for the htmx path.
- **ROOT CAUSE B (why it survived)** — the acceptance oracle asserts the HTTP **response body** (status +
  `div.error` substring), never the post-swap **DOM** (`feature_board_new_issue.rs:288-308` and the us-r01 /
  us-r03 / issue-edit-dialog / new-issue-dialog-description equivalents). Green over nothing.
- **ROOT CAUSE C (systemic)** — three divergent error-display idioms coexist with no rule for which applies
  where (see blast radius).

## Blast radius (verified)

**Invisible (the bug) — htmx form, bare 4xx fragment, no bespoke JS:**
1. Issue create (`new_issue_modal.html` → `#modal-root`; `issues.rs` → 400)
2. Issue edit / details (`issue_edit_modal.html` → `#modal-root`; `issues.rs` → 400)
3. Issue edit-dialog state change (`issues.rs:393` → 400)
4. Comment edit (`comment_edit_form.html` → `#comment-{id}`; `comments.rs` → 4xx)
5. Comment delete (`comment_card.html` hx-delete; `comments.rs` → 4xx)

**Partial** — board drag state change: `board-dnd.js:136-142` reverts the card on `!ok` but discards the
error body (user sees a snap-back with no reason).

**Lesser** — comment create: plain `method="post"` but `comments.rs:207` returns a bare fragment at 400 → on
full navigation the browser renders just `<div class="error">…</div>` as the whole page (visible but
shell-less; guarded in practice by `<textarea required>`).

**Already correct (the intended patterns — reuse these):**
- **Attachment upload** — `csrf-upload.js:52-58,82` reads the 4xx body and injects it into a
  `[data-upload-error]` slot. A working bespoke handler doing exactly what the htmx forms fail to do.
- **Full-page / no-JS POST forms** (sign-in, forgot-password, project-create, invites, token mint/revoke,
  bootstrap-claim, invite-accept) re-render the whole form with the error at 4xx — **visible**.

## Fix options (for DESIGN to ratify)

| Option | Mechanism | Keeps 400/422 + byte-stable oracles | Risk |
|--------|-----------|:---:|------|
| (a) `response-targets` extension | vendor it, `hx-target-4xx`/`hx-target-error` + error slot per form | ✅ | Low–Med (new vendored dep, hash-pin per VENDOR.md) |
| (b) global `responseHandling`/`beforeSwap` override | one script makes all 4xx swap | ✅ | **HIGH** — app-wide; collides with board-dnd.js revert + OOB flows + not-found/forbidden fragments |
| (c) return 200 + fragment | change handlers to 200 | ❌ breaks 400/422 oracles; wrong HTTP; erodes NFR-WEB-API-CON-02 | **HIGH** |
| **(d) small custom `htmx:beforeSwap` handler (house idiom)** | ~15 lines vanilla JS mirroring csrf-upload.js: on 4xx for an element with an error slot, `shouldSwap=true` + retarget to the slot | ✅ | **Low** — scoped listener, no new dep |

**RCA recommendation**: **(d)** as the implementation (with **(a)** as an acceptable declarative alternative).
It reuses the shipped csrf-upload.js idiom + vanilla-JS house style (board-dnd.js, keyboard.js), adds no
dependency, keeps every byte-stable fragment + status assertion, and is CSRF-safer than a full-modal replace
(swapping only the error slot leaves the form + its `_csrf` hidden field intact for retry).

## Non-negotiable for the fix (whichever mechanism)

- **Add a DOM-level acceptance oracle** — assert the error is present in the rendered target after the swap,
  not only in the response body. Otherwise Root Cause B persists and the next regression is again invisible.
- **Reconcile the edges** — the drag-revert message loss (surface #6) and the comment-create bare-page edge
  should join the same contract, not stay one-offs.
- **Correct the false contract** — `board-new-issue` D3's "error swaps in, visible" claim.

## Risk notes

- The 262144-char description path: `description_too_long` returns its message at 400; (d) swaps it into the
  slot. Body > the per-route `DefaultBodyLimit` is 413'd before the handler (separate, already-correct path).
- CSRF: (d) is client-side; swapping only the error slot preserves the form's `_csrf` (safer than replacing
  `#modal-root`'s innerHTML, which risked dropping the token on the retry).
- Scope the (d) listener to elements that declare an error slot so it does not hijack board-dnd.js's revert,
  the OOB comment/attachment/state flows, or not-found/forbidden fragments.

## Central files (for DESIGN/DELIVER)

`base.html`; `partials/new_issue_modal.html`, `issue_edit_modal.html`, `comment_edit_form.html`,
`comment_card.html`; `static/js/csrf-upload.js` (reference), `board-dnd.js` (revert path to reconcile);
`src/issues.rs`, `src/comments.rs`; contract to revise: `board-new-issue/discuss/wave-decisions.md` (D3);
oracle to strengthen: `foundry-acceptance/src/steps/feature_board_new_issue.rs`.
