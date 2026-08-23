# Slice 02 — New projects start with Backlog, In-Progress, Done

Story: US-BLM-02 | Estimate: 0.5 day | job_id: `job-board-lane-shaping`

## Goal

Project creation seeds exactly three lanes — Backlog, In-Progress, Done, in
that order (D4) — and the new-issue landing rule becomes "leftmost lane"
instead of the hardcoded `'backlog'` default (D6).

## IN

- Default-lane seeding in the create-project write: three lanes, fixed order.
- New-issue landing: issues are filed into the project's leftmost lane —
  replaces reliance on the `issues.state DEFAULT 'backlog'` column default as
  an observable rule (Driving Port 5).
- Dialog options and API validation on a new project reflect exactly its
  three lanes (falls out of slice 01's data-driven surfaces; pinned here).

## OUT

- Any change to existing projects' lanes (grandfathered in slice 01, D5).
- Configurable default templates (feature OUT).

## Learning Hypothesis

With slice 01's surfaces data-driven, "different defaults" is seeding-only:
no render/dialog/API code changes should be needed. If any surface still
shows Todo on a new project, a static-list consumer survived slice 01.

## Acceptance Criteria

- [ ] Creating "Reading List" (READ) renders exactly Backlog, In-Progress,
      Done, in order.
- [ ] Filing READ-1 "Dune" lands it in Backlog (leftmost lane, D6).
- [ ] READ-1's edit dialog offers exactly the three lanes.
- [ ] API PATCH READ-1 → "in_progress" succeeds; → "todo" is a 422 with the
      card still In-Progress.

## Dependencies

Slice 01 (lane data + data-driven surfaces).
