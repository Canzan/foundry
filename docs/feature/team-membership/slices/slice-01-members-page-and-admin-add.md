# Slice 01: A workspace admin sees a team's members and adds a workspace member to it

Stories: US-TM-01, US-TM-02 | Estimate: 1 day | job_id: `job-team-membership` | Walking skeleton

## Goal

A workspace admin follows **Teams** from the dashboard, opens a team's members
page, and adds a workspace member to the team. That member can then open the
team's boards. This resolves the 2026-10-09 incident in the product.

## IN

- A "Teams" quick action on `/`, and `GET /teams`: every team of the acting workspace for a workspace admin, and only the caller's own teams otherwise, each with a member count. The no-team copy is shown for a member on no team (D12).
- `GET /team/{team_slug}/members`: rows of display name · email · role, leads first (D8, D11). A plain member sees it read-only.
- `POST /team/{team_slug}/members`: a workspace admin adds a current workspace member as Member (D2, D7). Re-adding is idempotent (D13). The D15 copy is used.
- The Add list is (acting-workspace members) minus (team members). When it is empty, the D15 copy replaces it.
- Uniform 404 for every refusal, on both verbs (D5): signed out, unknown or foreign team, a caller without the right, a target outside the workspace.
- It works with scripting disabled.

## OUT

- Role change (slice 02), remove (slice 03), lead powers (slice 04), default team (slices 05-06).
- Any board-authz change (D9). Instance-admin access (OQ-1).

## Learning Hypothesis

- **Disproves if it fails**: that a single `team_memberships` row is all the SSO account lacks. If, after the add, `jeff@unintelligent-design.us` still cannot see Sandbox on the dashboard or still gets 404 on the board (a cached session workspace, a nav resolution, or a second gate), the incident's diagnosis is incomplete and slices 02-06 rest on a wrong model.
- **Confirms if it succeeds**: that a workspace admin can act on a team they are not on, through a team-scoped route, without opening an existence oracle. The uniform 404 stays byte-identical across the authz matrix. Every later slice reuses this surface.

## Acceptance Criteria

- [ ] On dev, signed in as `jeffabailey@gmail.com`: **Teams** → `/teams` shows "General · 1 member". `/team/general/members` shows "Jeff Bailey · jeffabailey@gmail.com · Lead".
- [ ] Add "Jeff Bailey — jeff@unintelligent-design.us" → "Added Jeff Bailey (jeff@unintelligent-design.us) to General as a member."
- [ ] Signed in with Keycloak as `jeff@unintelligent-design.us`: the dashboard lists "SBX Sandbox", and `/team/general/project/sandbox` answers 200 with the board (D4).
- [ ] Re-add from a stale page → "… is already on General.", with one row.
- [ ] Marco, an unknown slug, a foreign team, and a forged target id get byte-identical uniform 404s, with zero rows written.
- [ ] A plain team member sees the list with no controls. A workspace admin sees any team's list, but still gets 404 on its boards when not on the team.
- [ ] Dogfood (read-only queries, dev and prod): count workspace members on no team (KPI-1 baseline) and teams with no lead (KPI-3 baseline).

## Dependencies

- OQ-1 (recommended: instance admins not covered). This does not block, because the stories already assume it.
- No schema change. Reuses `is_workspace_admin`, `is_team_member`, `find_team_by_slug`, `resource_not_found_page`.
- Reference class: `/workspace/invites` (admin-gated GET+POST form on the shared layer, about 1 day including scenarios).
