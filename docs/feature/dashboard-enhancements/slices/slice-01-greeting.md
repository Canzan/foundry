# Slice 01 — Personalized greeting

**Goal**: the dashboard greets the signed-in user by name and names the acting workspace.

**Stories**: US-01 (+ its store-query test, US-05 AC-05.4).

**IN scope**
- New store query (tenant-scoped): `(display_name, workspace_name)` for the session `user_id`/`workspace_id`.
- `dashboard_root` loads it; `DashboardRoot` view gains `display_name` + `workspace_name` fields.
- Template renders "Welcome back, {display_name}" + "Workspace: {workspace_name}"; keeps `<h1>Foundry</h1>`.

**OUT of scope**
- Sign-out, admin link, style promotion (later slices). Email/avatar. Editing profile.

**Learning hypothesis**: disproves "one session-scoped query cleanly yields name + workspace" if it needs
multiple round-trips or a join the schema lacks. (Confidence high: `users.display_name` + `workspaces.name`
are single-table reads keyed by the session ids.)

**Acceptance**: `acceptance-criteria.md` US-01 (3 scenarios) + store test AC-05.4.

**Seams**: `signin.rs:252 dashboard_root`; `bootstrap.rs:23 SessionUser`; `users.display_name`,
`workspaces.name`; escaping via Askama.

**Dependencies**: none. **Effort**: ~0.5 day. **Reference class**: identical to the shipped
`list_projects_for_workspace` add (`51ba981`).
