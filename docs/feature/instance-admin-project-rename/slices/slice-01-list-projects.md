# Slice 01 — List every project on the instance dashboard

Story: US-IAPR-01 | Estimate: 0.5 day | job_id: `job-instance-project-rename`

## Goal

The instance dashboard (`GET /admin/instance/workspaces`) shows, under each
workspace it already lists, that workspace's projects — name, key prefix, and
owning team — so Priya can find a stale-named project without `psql`.

## IN

- Per-workspace project listing nested under the existing `data-workspace-row`
  entries (`data-project-row` markers; ordered by name within a workspace).
- Explicit "No projects yet." empty state per project-less workspace.
- Listing read that includes the project id (needed by slice 02's rename form)
  and the team display name — extend or supplement `list_projects_for_workspace`
  (DESIGN decides shape; instance-wide iteration over `list_workspaces` is fine
  at homelab scale, D3).
- Unchanged authz: `require_instance_admin`, uniform non-enumerable 404.

## OUT

- Any mutation (slice 02), validation (slice 03), pagination/search (D3),
  links from rows into workspace-scoped boards (cross-tenant navigation is a
  separate concern).

## Learning Hypothesis

The grouped read view alone already replaces the operator's `SELECT` habit —
and confirms the row markup slice 02 will hang the rename form on.

## Acceptance Criteria

- [ ] Every project in every workspace appears under its workspace, ordered by
      name, showing display name, key prefix, and team name.
- [ ] A workspace with zero projects renders the explicit empty state.
- [ ] Non-admin and signed-out requests still get the byte-identical uniform
      404 (no new oracle introduced by the richer page).

## Dependencies

None — extends the shipped dashboard (`instance_admin.rs::show_dashboard`,
`InstanceDashboardPage`).
