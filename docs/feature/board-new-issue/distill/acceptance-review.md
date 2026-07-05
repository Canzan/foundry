# Acceptance Review — board-new-issue (DISTILL self-review)

## Checklist

| Criterion | Verdict | Note |
|-----------|---------|------|
| Every AC has coverage | ✅ | S1–S5 cover AC-01.1/.2/.3/.4/.5/.6; AC-01.7 (zero backend) verified by git diff at finalize. |
| Port-driven | ✅ | All drive GET/POST on the board + issue endpoints. |
| Honest about the harness boundary | ✅ | Interactive htmx (click→swap→close) is NOT automatable in the HTTP suite; covered by wiring assertions + browser dogfood, documented in `test-scenarios.md` + `walking-skeleton.md`. Mirrors us-12's split. |
| Negative path | ✅ | S4 (empty title → error fragment, no card). |
| Regression pin | ✅ | S5 proves the no-JS plain-form fallback survives adding `hx-post` (D4). |
| Lane safety | ✅ | All `@pending`; excluded by `filter_run`. |
| No dependency on unbuilt seams | ✅ | Every port maps to a shipped seam (`requirements.md` table). |

## Risks / watch-items for DELIVER

- **R1 — htmx double-submit**: keeping BOTH `method="post" action=…` and `hx-post=…` on the form is
  intentional (D4 no-JS fallback) and htmx suppresses the native submit when it handles it — confirm no
  double POST fires when htmx is active (the harness can't see this; check in browser dogfood).
- **R2 — modal close**: D2 relies on the successful create response's PRIMARY body being empty/OOB-only so the
  `#modal-root` target empties. Confirm `submit_create`'s htmx success body is the OOB `<div>` only (it is —
  `issues.rs:88`), so swapping it into `#modal-root` leaves no visible modal. If a stray primary card renders
  in the modal, add an explicit empty primary or `hx-swap="none"` + rely on OOB.
- **R3 — scraper attribute assertions**: S1/S2 assert `hx-get`/`hx-post` presence via `scraper`; keep the
  selectors resilient (assert the attribute + its value substring, not exact whole-tag equality).

## Verdict

**READY for DELIVER.** One slice; un-@pend S1→S5, wire the two templates, dogfood the live interaction.
