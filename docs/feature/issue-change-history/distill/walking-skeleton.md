# Walking Skeleton — issue-change-history

The NET-NEW machinery is a durable, append-only `issue_change_events` table (migration `0013`) written in-tx at
each mutation via a shared `record_issue_change` helper, and three thin read surfaces over it. Slice 01 ships the
model + one capture point (status) + the human surface (the issue-detail page timeline) end-to-end; later slices
widen capture and add the program feed and the report.

## First failing test (DELIVER entry) — slice 01
**S1 — "A status change records one change event in the same transaction"**.
RED→GREEN:
1. Migration `0013`: `issue_change_events (id, workspace_id, project_id, issue_id, actor_id, field CHECK(status,
   title, description, rank), old_value NULL, new_value NOT NULL, created_at)` + indexes `(issue_id, created_at)`
   and `(project_id, created_at)`.
2. Store: a `record_issue_change(&mut tx, ws, project, issue, actor, field, old, new)` helper; call it inside
   `reposition_issue_with_outbox` (`lib.rs:1364`, which already reads `old_state` in-tx) to record
   `field=status` when the state changes (no-op → no record). A `list_issue_changes(issue_id)` read
   (newest-first) + a `list_project_changes(project_id)` read (for slice 04).
3. Web: a NEW issue-detail route + handler + template `/team/{t}/project/{p}/issues/{n}` rendering the issue +
   the timeline (newest-first, attributed, plain-language); the board card gains a link to it (quick-edit modal
   preserved). Resolve `update_issue_state_with_outbox` liveness (capture or delete per dead-code policy).

Then S2 (timeline render), S3 (empty for unchanged — UC-1), S4 (append-only), S5 (no-op), S6 (foreign refusal),
S7 (modal + card-link regression) green off the same model.

## Slice 02 — every editable field
Extend capture: restructure `update_issue_details` (`lib.rs:1524`) into a transaction that reads old title/desc,
UPDATEs, and records `field=title`/`description` for each changed field (it emits nothing today); record
`field=rank` in `reposition_issue_with_outbox` on a position change. S8/S9/S10 off the slice-01 model.

## Slice 03 — program feed
`GET /api/v1/teams/{t}/projects/{p}/issues/{n}/history` in foundry-api, mirroring the comments GET route (same
`Principal` auth + non-enumerable 404 JSON): serialize `list_issue_changes` oldest-first as `[{actor,field,old,
new,at}]`. S11/S12.

## Slice 04 — project report + CSV
A NEW project change-report web page over `list_project_changes` (list + status-flow + per-actor summaries) and a
CSV export (`text/csv` + `Content-Disposition: attachment`, mirroring `attachments.rs`), workspace-scoped.
S13/S14/S15. Then DOGFOOD the three surfaces on the sandbox board.

## Lane safety
All `@pending` (excluded by `filter_run`); `@all` stays green until DELIVER un-@pends per slice. Full `@all`
(incl. card-ranking 11/11 + issue-status-move 6/6 regressions) at finalize.
