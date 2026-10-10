# Slice 03: A workspace admin takes someone off a team, without leaving it leaderless

Story: US-TM-04 | Estimate: 0.5-1 day | job_id: `job-team-membership`

## Goal

A workspace admin removes a person from a team. The person loses access to the
team's boards on their next request. Removing the last lead is refused.

## IN

- `POST /team/{team_slug}/members/{user_id}/remove`: a "Remove" control for a workspace admin (D11). The D15 copy is used, with no confirm dialog (D13).
- The last-lead refusal, using slice 02's invariant (INV-1, INV-3, INV-4).
- A stale remove (the person is no longer on the team) changes nothing and re-renders the page.
- *If OQ-9 is accepted:* `POST /workspace/members/remove` also removes the person's team rows in that workspace, and is refused when they are any team's last lead.

## OUT

- Team-lead powers (slice 04). Notifying the removed person. Workspace role changes.

## Learning Hypothesis

- **Disproves if it fails**: that slice 02's invariant seam generalizes to a second verb. If remove needs a separate guard, INV-8 (one store-level guarantee) is in doubt.
- **Confirms if it succeeds**: that access follows membership immediately. The removed person's next board request is the uniform 404, with no session refresh needed.

## Acceptance Criteria

- [ ] Remove Tomás Rivera (Member) from Backend → "Removed Tomás Rivera (tomas.rivera@canzan.dev) from Backend." His dashboard drops AUTH, and the AUTH board answers the uniform 404.
- [ ] Removing Sam, Backend's only lead → the last-lead copy, and nothing changes.
- [ ] A concurrent removal of Lena and demotion of Sam (the only two leads) → exactly one succeeds, over at least 100 runs.
- [ ] Marco → uniform 404, and Backend is unchanged.
- [ ] Removing someone from their only team → their dashboard shows the no-team copy.
- [ ] Dogfood on dev: remove and re-add `jeff@unintelligent-design.us` on General, and confirm the board flips 404 → 200.

## Dependencies

- Slice 02 (invariant). OQ-9 (scope widening into a shipped route) must be answered before DESIGN.
