# CONTEXT

## Current Task

**`new-issue-dialog-description` SHIPPED via full nWave pipeline** (DISCUSS→DESIGN→DISTILL→DELIVER), on `main`,
8 commits `ec52a7a`→`3c91c57` + finalize `621ec4c`, **not pushed**. The ask was "show the Description field in
the new-issue dialog like edit"; execution found it was full-stack (absent at store/service/form/view/template
+ JSON API), threaded an optional `description` through all layers keeping web/API rule-parity, and added a
shared `validate_description` bound (262144 = the DB CHECK) on both create and edit — converting the edit
path's DB-CHECK 500 into a clean 422/fragment. All 16 acceptance scenarios green; DES integrity exit 0;
`validate_description` mutation 4/4 caught. ZERO new routes/endpoints/migrations (latest stays `0014`). Archive:
`docs/evolution/2026-07-18-new-issue-dialog-description.md`. (Thirteen features now through the pipeline.)

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
