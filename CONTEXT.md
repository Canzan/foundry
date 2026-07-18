# CONTEXT

## Current Task

**`fix-comment-delete-csrf` FIXED + finalized** (`main`, `59c10be` + docs `673f185`, **2 ahead of BOTH
remotes, not pushed**). Follow-up from form-error-display-contract's archive: the comment Delete button was a
bare `hx-delete` with no CSRF token → 403'd in a real browser (broken for users), masked because HTTP-lane
tests inject the token. Fixed with the `hx-headers` cookie→`x-csrf-token` echo (correct for a body-less
DELETE); a `@needs-browser` regression proves a real delete removes the card + soft-deletes the row; HTTP-lane
comment tests 10/10 green. Swept all mutating htmx triggers — **none lacks CSRF now**. Archive:
`docs/evolution/2026-07-18-fix-comment-delete-csrf.md`. See `[[htmx-4xx-errors-invisible-form-errors-js]]`.

**Prior this session** — everything below `59c10be` was PUSHED to both remotes (`github` Canzan + `origin`
Forgejo) at `9ae65db`. The full `cargo xtask ci` was red only on a confirmed browser-lane load flake (leaked
chromedrivers → WaitTimeout on untouched board render; 6/6 pass in isolation); pushed on that evidence.

**`form-error-display-contract` SHIPPED** (bugfix→design-first→DESIGN→DISTILL→DELIVER), on `main`, 5 code
commits `8b08487`→`80754a8` + finalize `c5795cb`, **not pushed**. htmx 2.0.4 doesn't swap 4xx bodies, so form
validation errors (correct `400/422 + fragment`) were invisible in-browser app-wide. Shipped `form-errors.js`
(an `htmx:beforeSwap` handler routing 4xx into opt-in `[data-error-slot]`s) + slots on 3 forms; 6
`@needs-browser` DOM-oracle scenarios (S1-S6) prove it; server error path unchanged; no migration (stays
`0014`). The browser lane caught a **real latent defect**: comment-edit shipped with NO CSRF token (403'd in a
real browser) — fixed with a body `_csrf` like every form. Slice 03 (drag/comment-create edges, S7-S8)
deferred. Archive: `docs/evolution/2026-07-18-form-error-display-contract.md`. See
`[[htmx-4xx-errors-invisible-form-errors-js]]`. (Fourteen features through the pipeline.)

**Prior**: `new-issue-dialog-description` SHIPPED (`ec52a7a`→`3c91c57`+`621ec4c`) — Description field on
new-issue create, threaded through all layers, `validate_description` bound (262144 = DB CHECK) on create+edit;
16 scenarios green. Archive: `docs/evolution/2026-07-18-new-issue-dialog-description.md`.

## Key Decisions

- **Two DISCUSS premises corrected by reading code**: description was NOT unbounded (DB `CHECK ≤ 262144`,
  over-long surfaced as a 500); and the modal is NOT destroyed on a validation error — htmx 2.0.4 doesn't swap
  the 4xx, so input is preserved but the error message is **invisible in-browser** (app-wide pre-existing
  defect, deferred to its own `/nw:root-why`). Error scenarios assert HTTP, never the DOM.
- **CSRF 64KB form-body cap blocked the 262144 bound over the web** (`csrf.rs`) — user-approved raise to 2 MiB +
  per-route `DefaultBodyLimit` on issue create/edit POSTs. Security-sensitive; flagged for security review. See
  `[[csrf-form-body-64kb-cap]]`. A slice-03 crafter correctly BLOCKED rather than modify the CSRF control
  unscoped.
- **Mutation found a real gap = the `@real-io` trap**: the bound was acceptance-only-covered so mutants falsely
  survived (5/6 missed); fixed by extracting the pure `validate_description` + fast unit tests (also the
  reviewer's dedup) → 4/4 caught.

## Next Steps

- Optional: `git push` (finalize did NOT push, per trunk pattern). Pre-push gate is full `cargo xtask ci`.
- **Deferred bugfix**: app-wide invisible in-browser htmx validation errors (`board-new-issue` D3 is false under
  htmx 2.0.4). When fixed, this feature's `description_too_long`/`title_required` copy becomes visible for free.
- **Security review** of the CSRF body-cap raise; a `csrf.rs` comment overstates "scoped" (buffer ceiling is
  global, matches axum default — no new DoS surface).
- **Carried from keyboard-shortcut-bindings**: UI-5 IME scenario retarget; ADR-008 trap-B inversion;
  `#kb-search-panel`/`#kb-overlay-root` have no CSS; lane flakes (PoolTimedOut, no-JS ~2-3/10). Prometheus
  `foundry_token_mutations_total` exporter; per-workspace backup (OD-5); key-rotation UX.
