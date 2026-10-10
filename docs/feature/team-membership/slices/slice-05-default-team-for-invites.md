# Slice 05: A workspace admin picks a default team, and an invited newcomer lands on it

Story: US-TM-06 | Estimate: 1 day | job_id: `job-team-membership`

## Goal

A workspace admin chooses a default team on `/teams`. Anyone who accepts an
invite to that workspace afterwards is on that team as Member and sees its
boards on their first dashboard.

## IN

- One per-workspace default-team setting, unset on upgrade (OQ-4). DESIGN owns the shape. It must always point at a team of the same workspace or nowhere (INV-7, OQ-6).
- On `/teams`, the "Default team for new members" select (the workspace's teams plus "No team") and `POST /teams/default`, for a workspace admin only (OQ-3). The D15 copy is used.
- Invite accept adds the newcomer to the default team as Member, atomically with the workspace membership (OQ-5).
- An unset or missing default never refuses or fails an accept (D17).

## OUT

- Keycloak provisioning (slice 06). A per-invite team choice (OQ-7). Backfilling existing members (OQ-8).

## Learning Hypothesis

- **Disproves if it fails**: that the default can be applied inside the shipped accept transaction without changing the invite's single-use or expiry guarantees. If the member-invites lanes (30 + 18) go red, the default needs its own step.
- **Confirms if it succeeds**: that a newcomer's first page is a board. KPI-2 moves from 0% to 100% for invites.

## Acceptance Criteria

- [ ] Priya sets General → "New members will join General." Setting "No team" → "New members join no team."
- [ ] Tomás accepts an invite to Canzan Labs → he is a Member of General, and his first dashboard lists Sandbox.
- [ ] With no default → today's no-team copy. With a missing default → the account is created, signed in, and on no team.
- [ ] Lena (not admin), or a Globex team id → uniform 404, and the default is unchanged.
- [ ] The migration is additive and rolling-deploy safe (an older replica ignores it), with a CHANGELOG migration note.
- [ ] Dogfood on dev: set General as default, invite a real secondary address, accept, and the Sandbox board opens.

## Dependencies

- Slice 01. OQ-3 to OQ-8 resolved to the recommended answers, and OQ-10 keeps it in this feature (2026-10-09).
