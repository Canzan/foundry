# Slice 04 — Project change report + CSV export

**Goal**: a lead sees aggregated change activity across a project and can export it.
**Story**: US-04.

**IN scope**
- A project-level **change report** — change events across the project's issues (issue key, actor, field,
  old → new, when), most recent first, workspace-scoped, from the SAME stored events.
- Summaries: at least **status-flow** transition counts and **per-actor** change counts (ODD-4 fixes the exact
  dimensions).
- **CSV export** with a stable column contract, generated from the same events.
- Tenancy — only the acting workspace's data; no cross-tenant leakage.
- Acceptance: the aggregated listing + the two summaries + a CSV with the stable columns + tenancy isolation.

**OUT of scope**: cross-project / org-wide reporting; charts/visualization polish; scheduled/emailed reports; the
program feed (03, its own slice).

**Learning hypothesis**: disproves "the same events aggregate into a useful, exportable project report" if
status-flow / per-actor rollups or CSV export need a separate store or a different event grain.

**Seams**: the slice-01 stored events; the project/board web surface for the report page; a CSV response
(`Content-Disposition: attachment`, mirroring the attachments download seam); tenancy `resolve_member_project`.
**Dependencies**: slice 01 (stored events); benefits from 02 (more field kinds to report). **Effort**: ~1 day
(an aggregate read + export; the report page is the only new surface).
