# Slice 01: A workspace admin creates a team, leads it, and starts a project on it

Stories: US-TL-01 | Estimate: 1 day | job_id: `job-team-lifecycle` | Walking skeleton

## Goal

In a workspace that has no teams, a workspace admin creates one on `/teams`,
lands on its members page as its Lead, follows **New project**, and opens the
new board. This ends the zero-team dead end of provisioned workspaces.

## IN

- On `/teams`, for a workspace admin: a **Team name** field and a **Create team** button. When the workspace has no teams, the copy "This workspace has no teams yet. Create one to start a project." is shown too. Non-admins see no form.
- `POST /teams` (proposed, D13): it writes the team and the creator's Lead row together (D3, INV-TL-7), then lands on `/team/{slug}/members` with "Created {team}. You're its lead."
- Name refusals, as a minimum: empty, and not unique within the workspace (a case-insensitive name, or a colliding derived slug). The D12 copy is used, the input is kept, and nothing is written.
- The slug is `slugify(name)`, minted once (D7).
- **New project** on the members page, for anyone on the team, linking to the shipped `/team/{slug}/projects/new` (D14).
- The dashboard's empty-state "create your first project" link targets a team the caller is on (the first by name), not the hardcoded `general` (D14).
- Uniform 404 for a non-admin, a signed-out caller, or an instance admin who is not a workspace admin (D2, D5).
- It works with scripting disabled.

## OUT

- Control-character and length arms, the non-Latin fallback slug, the concurrent-create race and the DB rule (slice 03). If DESIGN lands the whole value object here, slice 03 still owns proving it.
- Rename (slice 04) and delete (slice 02).
- An automatic first team on provisioning (OQ-5).

## Learning Hypothesis

- **Disproves if it fails**: that a team row plus its creator's Lead row is all a provisioned workspace lacks. If, after create, Priya still cannot create a project or open its board (a second gate, a nav or dashboard path that assumes `general`, or a workspace-switch subtlety), the zero-team diagnosis is incomplete, and slices 02-04 rest on a wrong model.
- **Confirms if it succeeds**: that creator-as-lead (D3) makes a new team immediately usable, with no membership step and no change to board authz.

## Acceptance Criteria

- [ ] On dev, in a provisioned workspace with zero teams, signed in as its workspace admin: `/teams` shows the empty-state copy. Create "Ops" → "Created Ops. You're its lead." and the creator listed as Lead.
- [ ] **New project** → create "Homelab Ops" (OPS) → `/team/ops/project/homelab-ops` answers 200, and the dashboard lists "OPS Homelab Ops".
- [ ] "backend", where Backend exists → "Team name must be unique within the workspace", with no write.
- [ ] Lena (lead, not admin), Marco and a signed-out caller get byte-identical uniform 404s on `POST /teams`, with zero rows written.
- [ ] The dashboard's empty-state project link never points at a team the caller is not on.
- [ ] Dogfood (read-only, dev and prod): count workspaces with zero teams (the KPI-TL-1 baseline).

## Dependencies

- OQ-3 only if DESIGN lands the fallback slug here. Otherwise an empty slugify result is refused in this slice and handled in slice 03.
- Reuses `is_workspace_admin`, `slugify`, `is_refused_name_char`, the members page, and project create.
- Reference class: team-membership slice 01 (an admin-gated GET+POST on `/teams` plus a members-page change, about 1 day with scenarios).
