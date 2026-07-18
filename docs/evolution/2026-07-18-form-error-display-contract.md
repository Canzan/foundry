# Evolution — form-error-display-contract (the browser finally shows why a form was rejected)

**Finalized**: 2026-07-18
**Commits**: DELIVER `8b08487` → `80754a8` (5 code commits; `e43c278` is the pre-DELIVER baseline) — 4
DES-monitored 5-phase TDD steps across 2 slices, plus one review-driven revision (`80754a8`, the CSRF
consolidation). Full pipeline: escalated from `/nw-bugfix` → **design-first** → DESIGN → DISTILL → DELIVER.
Trunk-based; legacy multi-file convention; DES-monitored (exempt at finalize). Feature dir PRESERVED. **Not
pushed.**
**Scope**: htmx 2.0.4 does not swap 4xx bodies, and the app shipped no override/extension/handler, so form
validation errors — returned correctly as `400/422 + fragment` — were **discarded by the browser**: the form
silently did nothing. Five htmx forms were affected. This ships the client-side contract that displays them,
plus the browser-level oracle that proves it. ZERO server behavior change to the error path, ZERO new
route/endpoint/migration (latest stays `0014`); ONE new runtime artifact (`static/js/form-errors.js`, 75
lines), ONE `<script defer>`, and a `[data-error-slot]` in three form partials.

## Milestone — the error is visible. And the test can finally see it too.

The defect had two causes, and the second is why it shipped: (A) the contract assumed htmx swaps 4xx bodies —
`board-new-issue` D3 literally says *"the error swaps into the modal — visible"* — a behavior htmx 2.0.4 never
provides; and (B) **every acceptance oracle asserted the HTTP response body, never the rendered DOM**, so a
correct `400 + fragment` passed while the browser showed nothing. Green over an invisible defect.

The fix for (A) is small and reuses what already shipped: `form-errors.js` — one vanilla document-delegated
`htmx:beforeSwap` listener that, on a 4xx whose triggering element declares a `[data-error-slot]`, routes the
fragment into that slot (`shouldSwap=true; target=slot; isError=false`) and otherwise does nothing (so 2xx/OOB
success, `board-dnd.js`, and not-found/forbidden are untouched). It mirrors `csrf-upload.js`'s slot idiom and
the `keyboard.js`/`board-dnd.js` house style. The server was not touched; the shipped HTTP-lane oracles still
assert the `400/422 + fragment` and stay green — this feature **adds** the DOM assertion they never had.

The fix for (B) is the durable part (ADR-002): **the oracle moved to the browser.** Every scenario is
`@needs-browser` (the fantoccini lane keyboard-shortcut-bindings shipped) and asserts the error is *visible in
the rendered DOM* — the only layer that can see a client-side swap. After this, "green suite, invisible
browser" is no longer possible for a form under the contract.

## The browser lane earned its keep — it caught a real latent defect the HTTP lane masked

Writing S6 (comment-edit error visibility) surfaced that **`comment_edit_form.html` shipped with no CSRF token
at all** — the only write form in the app missing `<input name="_csrf">`. In a real browser the PATCH `403`'d
on CSRF *before* validation ever ran; comment editing was broken for real users. It stayed hidden because the
HTTP-lane comment tests inject the token manually — **precisely the HTTP-body blindness this feature exists to
close, demonstrated on the feature's own first browser test.** Fixed properly (user-approved): comment-edit now
mints and carries a body `_csrf` field exactly like every other form, mirroring the sibling issue-edit handler
(`ensure_csrf_cookie` + `response_with_optional_cookie`). The interim cookie→header echo that first unblocked
the test was consolidated away.

## What shipped

- **`static/js/form-errors.js`** — the `htmx:beforeSwap` handler + a `formErrorsReady` marker; loaded
  `<script defer>` from `base.html`. Guarded to 400–499; slot resolved via `data-error-target` or
  `closest('form') [data-error-slot]`; no-op when none resolves (blast radius bounded by opt-in).
- **`[data-error-slot]`** in `new_issue_modal.html`, `issue_edit_modal.html`, `comment_edit_form.html`
  (byte-additive; the 2xx success swaps are untouched).
- **comment-edit CSRF** now via a body `_csrf` field (consistency + a genuine latent-defect fix).
- **6 `@needs-browser` DOM-oracle scenarios** (S1–S6): the lane-probe/skeleton, invalid create shows the
  reason + no row, fix-and-resubmit succeeds (proves the slot-only swap preserved `_csrf`), a valid create
  shows no error (blast-radius guard), invalid issue edit, invalid comment edit. Real browser + real Postgres.

## Falsification was demonstrated, not asserted

Each green was shown red first: S1/S2 red against a build without `form-errors.js`/the slot (the direct defect
reproduction — the first test that could *see* it); S3 red against a full-`#modal-root` replace (drops `_csrf`
→ resubmit fails); S4 red against a handler firing on 2xx (over-broad); S5/S6 red without their per-form slot.
Mutation testing has ~no Rust target here (client-side JS + templates + handler wiring that mirrors an existing
pattern, all acceptance-covered) — RED_UNIT is `SKIPPED`/`NOT_APPLICABLE` on all four steps, the
keyboard-shortcut-bindings precedent for a client-layer feature.

## Deferred / open

- **Slice 03 (the edges) — deferred by design**: S7 (a rejected drag reverts *and says why* via an OOB toast)
  and S8 (invalid comment-create shows the reason instead of a bare page) remain `@pending`; they need
  deterministic server-refusal fixtures. A follow-up.
- **Audit comment-delete for the same CSRF gap** — comment-edit lacked a token; the delete trigger
  (`comment_card.html`) should be checked for the same, and covered by a browser scenario when slice 03 lands.
- **Browser-lane flake** — intermittent sign-in/nav `WaitTimeout` (leaked testcontainers / timing), carried
  from `[[foundry-browser-lane-fantoccini]]`; re-runs clear it. Bounded `wait().for_element`, never sleeps.
- **`board-new-issue` D3 corrected** (the false "errors are visible" premise); recorded in
  `design/upstream-changes.md`, shipped as behavior + the DOM oracle.
