# Acceptance Review — issue-edit-dialog (DISTILL self-review)

| Criterion | Verdict | Note |
|-----------|---------|------|
| Every AC covered | ✅ | S1–S6 + 2 store scenarios cover AC-01.1..08 |
| Port-driven | ✅ | board + edit endpoints + store |
| Honest harness boundary | ✅ | live click→save interaction is dogfood, per board-new-issue precedent |
| Negative + security paths | ✅ | S4 empty-title, S5 foreign-non-enumerable |
| Regression pin | ✅ | S6 no-JS fallback; store isolation |
| Lane safety | ✅ | all @pending |
| Reconciliation | ✅ | ODD-1..4 (ADR-001/002) reflected in S3/S6; 0 contradictions |

## Watch-items for DELIVER
- **R1 card `number`**: the board card-view currently exposes only key+title; DELIVER must surface the issue
  `number` to build the edit URL (a small card-view field, analogous to board-new-issue's board slugs).
- **R2 OOB-replace selector**: the save response replaces `[data-issue-key='GEN-1']` via `outerHTML`; keep the
  replaced card ITSELF clickable (re-emit its `hx-get`).
- **R3 pre-fill read**: reuse an existing gated issue-fetch if one exists behind `show_issue`; only add a read
  method if none fits.

## Verdict
READY for DELIVER. One slice; RED S3 first.
