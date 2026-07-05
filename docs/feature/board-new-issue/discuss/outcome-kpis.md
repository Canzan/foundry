# Outcome KPIs — board-new-issue

| KPI | Target | Measurement |
|-----|--------|-------------|
| Button works | Clicking "New issue" opens the modal and filing a title creates the issue | Browser dogfood + acceptance scenario |
| Card lands in Backlog without reload | New card appended to `[data-column='backlog']` via OOB, no full navigation | Acceptance scenario (AC-01.3) |
| No-JS fallback intact | Plain-form POST still creates + shows the issue | Acceptance scenario (AC-01.6) |
| Zero backend change | 0 changes under `src/`; only `board.html` + `new_issue_modal.html` (+ test glue) | git diff at finalize |
| No regressions | `@all` lane + `cargo xtask ci` green | full CI |

**North-star**: a member can capture an issue from the board in one click + a title, and see it immediately.
