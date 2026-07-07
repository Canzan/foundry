# ADR-002 — Three read surfaces: issue-detail page, program feed, project report

**Status**: Accepted (user-ratified 2026-07-07) · **Feature**: issue-change-history · **Slices**: 01, 03, 04

## Context

The one `issue_change_events` model (ADR-001) feeds three consumers (human / program / report). Each needs a
home, and none may become a second source of truth.

## Decision

### 1. Human timeline → a new issue-detail page (ODD-3)
A new route `GET /team/{t}/project/{p}/issues/{n}` renders an issue-detail page (title, status, description) with
the **change timeline** below it — `list_issue_changes(issue_id)` ordered newest-first, each entry in
plain-language (`Mei moved status Todo → In Progress · 2h ago`). Rationale: the timeline needs room and grows over
time; the modal edit dialog is small and mixing edit with history reads poorly; and there is **no issue-detail
page today** — this fills a real gap and gives the project report (below) a sibling surface.

- **Navigation**: the board card gains a link to the detail page (DELIVER picks the affordance — likely the issue
  KEY as a link — so it does not collide with the drag gesture or the quick-edit modal). The existing
  click→quick-edit modal is **preserved** (regression-guarded).
- Progressive enhancement: the detail page + timeline are fully server-rendered (no JS required); any live refresh
  is out of scope (converge on reload, mirroring card-ranking UC-1).

### 2. Program feed → `/api/v1` (ODD-6)
`GET /api/v1/teams/{t}/projects/{p}/issues/{n}/history` returns `[{actor, field, old, new, at}]` (ISO-8601 UTC),
**oldest→newest** (stable audit order), mirroring the shipped comments GET route: the same `Principal` auth, the
same uniform non-enumerable `404` JSON envelope for a foreign/absent issue (never a 500). A reserved `cursor`
field documents pagination-readiness; v1 returns the full list in stable order. The JSON reads the SAME table the
timeline renders (AC-03.4 — no second source of truth).

### 3. Project report → a report page + CSV (ODD-4)
`GET /team/{t}/project/{p}/report` (or a project-scoped path DELIVER pins) renders a change report across the
project's issues via `list_project_changes(project_id)` on the `(project_id, created_at)` index — a table (issue
key, actor, field, old→new, when, newest-first) plus at least **status-flow transition counts** and **per-actor
change counts**. An **Export → CSV** action returns `text/csv` with `Content-Disposition: attachment` (mirroring
`attachments.rs`) and a stable column contract, generated from the same events. Workspace-scoped (no cross-tenant
leakage, AC-04.4).

## Why

- One page-based home for the timeline (not the modal) matches the growth of history and fills the missing
  issue-detail surface; the report is its natural sibling.
- Mirroring the comments `/api/v1` route reuses the whole auth + non-enumerability + JSON-envelope contract — no
  new security surface.
- CSV via the attachment pattern reuses a shipped response idiom.

## Consequences

- Two new web pages (issue-detail, project report) + one new API route + one CSV response.
- The board card's markup gains a detail-page link (a small, additive change; the modal path is unchanged).
- All three surfaces are thin reads over ADR-001's table; adding a surface later (e.g. webhooks) is another reader,
  not another writer.
- `GET history` orders oldest→newest (audit/stream order for programs); the human timeline orders newest-first
  (reading order) — same data, surface-appropriate ordering.
