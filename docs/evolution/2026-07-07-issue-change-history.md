# Evolution — issue-change-history (a durable, attributable change record for issues, read three ways)

**Finalized**: 2026-07-07
**Commits**: DISCUSS+DESIGN+DISTILL `1487de8` → DELIVER slice 01 `1e4740a` → slice 02 `0dc414a` → slice 03
`c8172b9` → slice 04 `3fa119d` → finalize (this). Trunk-based; repo legacy multi-file convention; DES exempt.
Feature dir PRESERVED.
**Wave coverage**: full pipeline WITH a real DESIGN — DISCUSS (4 stories → 4 slices, 3 personas → 3 surfaces) →
DESIGN (ADR-001 dedicated change-event model + in-tx capture; ADR-002 three read surfaces; ODD-1..6 user-ratified)
→ DISTILL (15-scenario SSOT) → DELIVER (4 slices).
**Scope**: every change to a tracked issue field (status, title, description, rank) becomes a durable, immutable,
attributable record — `actor · field · old → new · when` — written in the SAME transaction as the mutation, and
read three ways: a human timeline on the issue page, a program JSON feed, and a project report + CSV.

## The one model, three surfaces

A single append-only table feeds all three consumers — no second source of truth. Adding a future surface (e.g.
webhooks) is another reader; adding priority/assignee capture (once those become editable) is a CHECK addition +
a helper call — no model change.

## What shipped

### Slice 01 — status history + timeline (`1e4740a`)
- **Migration `0013`** — `issue_change_events (id, workspace_id, project_id, issue_id, actor_id, field CHECK(status
  |title|description|rank), old_value NULL, new_value, created_at)` + indexes `(issue_id, created_at)` and
  `(project_id, created_at)`, mirroring the `comments` precedent; append-only, cascade-deleted.
- **`record_issue_change(&mut tx, …)`** store helper — INSERTs in the SAME transaction as the mutation (no
  phantom / no drop). Called inside `reposition_issue_with_outbox`'s existing `if new_state != old_state` block →
  records `field=status` (a no-op state save records nothing).
- **`list_issue_changes`** (newest-first) → the **issue page** now renders an attributed, plain-language change
  timeline. Genesis = start empty (ODD-5): an unchanged issue's timeline is empty (no created event).
- The board card's key became a **link to the issue page** (quick-edit modal preserved).
- **Dead code removed**: `update_issue_state_with_outbox` (superseded by `reposition_issue_with_outbox` in
  card-ranking, zero callers) deleted per the pre-stable policy.

### Slice 02 — every editable field (`0dc414a`)
- **`update_issue_details`** — which had NO transaction and ignored the actor — restructured into one tx that
  reads the old title/description, UPDATEs, and records a `title`/`description` change **per actually-changed
  field**. `reposition_issue_with_outbox` additionally records `field=rank` on a position change (alongside the
  status event on a cross-status drop). One row per changed field; a same-value save records nothing.

### Slice 03 — program JSON feed (`c8172b9`)
- **`GET /api/v1/teams/{t}/projects/{p}/issues/{n}/history`** → `[{actor, field, old, new, at}]`, **oldest→newest**
  (stable audit order; the human timeline is newest-first). Auth + uniform non-enumerable 404 mirror the sibling
  `/api/v1` issue routes. `actor` = email (matching `CommentJson.author_email`); the shared `list_issue_changes`
  gained an `actor_email` projection so the feed and the timeline read the SAME rows (one source of truth).

### Slice 04 — project report + CSV (`3fa119d`)
- **`GET /team/{t}/project/{p}/report`** — a change report across the project's issues (issue key, actor, field,
  old→new, when), newest-first, with **status-flow transition** + **per-actor** summaries; `?format=csv` returns a
  `text/csv` attachment (hand-escaped RFC-4180) with the stable header `issue,actor,field,old,new,at`, mirroring
  the attachments download. `list_project_changes` joins issues/projects for the issue key; workspace-scoped. The
  board gained a link to the report.

## Decisions realized (ADR-001/002, ODD-1..6, ratified)
| # | Decision | Status |
|---|----------|--------|
| ODD-1 | Dedicated append-only `issue_change_events` table (NOT the outbox) | IMPLEMENTED |
| ODD-2 | In-tx capture via a shared `record_issue_change` helper at every write path | IMPLEMENTED |
| ODD-3 | Human timeline on the issue page (the route already existed → EXTENDED, not a duplicate) | IMPLEMENTED |
| ODD-4 | Project report page + CSV export (status-flow + per-actor summaries) | IMPLEMENTED |
| ODD-5 | Genesis = start empty (no backfill, no created event) | IMPLEMENTED |
| ODD-6 | Program envelope: bare JSON array, oldest→newest, non-enumerable 404 | IMPLEMENTED |

**DESIGN deviation (justified)**: ADR-002 assumed "no issue-detail page today," but `comments::show_issue`
already rendered one — so the timeline was ADDED to that page (Axum panics on a duplicate route), which is
strictly better (timeline lives alongside comments).

## Verification
- **DELIVER**: issue-change-history 15/15 (93 steps) — 7 `@us-01`, 3 `@us-02`, 2 `@us-03`, 3 `@us-04`. Per-slice
  regressions green throughout: card-ranking 11/11, issue-status-move 6/6, issue-edit-dialog 6/6, us-w05a 4/4,
  us-w05c 7/7, us-b01 4/4. `cargo xtask smoke` green at each slice.
- **Finalize**: `cargo xtask ci` full `@all` lane (recorded at finalize).

## Deferred (out of scope)
Adding priority/assignee EDITING (this feature records changes; the field-agnostic model absorbs them for free
once a separate feature makes them editable); comments/attachments in the timeline (own surfaces); editing/
deleting history entries (append-only, immutable); live push of new entries to open viewers (converge on reload);
cross-project / org-wide reporting; retention/pruning (append-only in v1); a same-index cross-column move
recording a rank change (records status only — the position index is unchanged).
