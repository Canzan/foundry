# Acceptance Review — issue-change-history (DISTILL self-review)

| Criterion | Verdict | Note |
|-----------|---------|------|
| Every AC covered | ✅ | S1–S15 cover AC-01.1–.6 + AC-02.1–.3 + AC-03.1/.2/.4 + AC-04.1–.4 |
| Port-driven | ✅ | edit-dialog/state + drop (capture); issue-detail page GET (human); `/api/v1/.../history` (program); report page + CSV (report) |
| Honest harness boundary | ✅ | record contract + renders + JSON + CSV all HTTP/store-testable; phrasing polish + live refresh = dogfood |
| Negative paths | ✅ | S6 (web foreign), S12 (API foreign), S15 (report tenancy), S5 (no-op no event) |
| One model, three surfaces | ✅ | S11 asserts JSON == stored events; report + timeline read the same table |
| Genesis = start empty (UC-1) | ✅ | S3 asserts empty timeline for an unchanged issue; no scenario asserts a created event |
| Lane safety | ✅ | all `@pending` |

## Watch-items for DELIVER
- **R1 in-tx capture**: the record INSERT must be in the SAME tx as the mutation. A rolled-back mutation records
  NOTHING; a committed one ALWAYS records. Capture inside `reposition_issue_with_outbox`'s existing tx and inside
  the RESTRUCTURED `update_issue_details` tx (S1/S4/S8).
- **R2 no-op = no event (S5)**: record only when the value actually changes (state unchanged → no row; a
  same-title save → no title row).
- **R3 one row per changed field (S9)**: a multi-field save writes one row per CHANGED field; unchanged fields
  write nothing.
- **R4 update_issue_details has no tx + ignores actor today** (`lib.rs:1524`): it must be restructured to read
  old → tx → UPDATE → record, and its `_actor_id` threaded through (S8). Also confirm `update_issue_state_with_
  outbox` liveness — capture if the API PATCH still uses it, else delete (dead-code policy).
- **R5 empty timeline (S3, UC-1)**: no backfill, no created event — an unchanged issue's timeline AND its
  `/history` JSON are empty.
- **R6 detail page + modal (S7, UC-2)**: the card must gain a detail-page link AND keep its quick-edit
  hx-get/modal (regression); the detail-page nav affordance must not collide with the drag gesture.
- **R7 ordering**: human timeline newest-first (S2); `/api/v1/.../history` JSON oldest-first (S11) — same data,
  surface-appropriate order.
- **R8 report CSV contract (S14)**: stable columns `issue,actor,field,old,new,at`; `text/csv` +
  `Content-Disposition: attachment` (mirror `attachments.rs`).
- **R9 tenancy on all three surfaces (S6/S12/S15)**: workspace-scoped; foreign/absent → uniform non-enumerable
  refusal, never a 500.

## Verdict
READY for DELIVER. Slice 01 (RED **S1** → model + in-tx capture + issue-detail timeline) ships the machinery;
02 widens capture, 03 adds the JSON feed, 04 adds the report + CSV. All `@pending` until wired.
