# Slice 02: A workspace admin deletes a team, and a delete never loses a project

Stories: US-TL-02, US-TL-03 | Estimate: 1.5 days | job_id: `job-team-lifecycle`

## Goal

A workspace admin deletes a team from its members page, through a confirm page
that states every consequence. A team that has projects, including one created
while the confirm page was open, is refused with the reason, and nothing is
destroyed.

## Why 1.5 days (taste-test exception)

The projects refusal and its race guarantee are delete's safety net. Shipping
delete without them would make a one-click cascade over every project, issue,
comment and attachment of a team reachable, and an infrastructure-only follow-up
slice is forbidden. So both stories ship together.

## IN

- **Delete team** on the members page, for a workspace admin only.
- `GET /team/{slug}/delete` is the confirm page: "Delete {team}?", the member count, the default-team warning (only when the team is the default), the scoped-token line (only when tokens are scoped to it), and **Delete team** and **Cancel** (D8, D12).
- `POST /team/{slug}/delete` deletes the team, which cascades its memberships and scoped tokens and clears the default. It shows "Deleted {team}." on `/teams` (D10).
- While the team has any project: the confirm page shows "{team} still has {n} project(s): {list}. A team with projects can't be deleted." with no Delete button, and the POST refuses the same way, decided at commit time (OQ-4, D9, INV-TL-1).
- Delete versus project create, and delete versus the membership, default and token writes: never a lost project and never a 500 (INV-TL-1, INV-TL-2).
- (OQ-6) One delete record, which commits with the delete and outlives the team.
- Uniform 404 for everyone except a workspace admin, on both verbs (D2, D5). Both pages work without scripting.

## OUT

- Deleting a team with projects in any form (OQ-4 (b) and (c)). Project delete or move.
- A UI for the delete record. Undo.

## Learning Hypothesis

- **Disproves if it fails**: that a commit-time "has no projects" decision closes the delete-versus-create race. If any of 100 races destroys a project, or answers 500, the house concurrency precedents (lane-002's FK guard, team-membership's row lock) do not transfer to a parent delete, and OQ-4's refusal is not safe as specified.
- **Confirms if it succeeds**: that 0021's cascade exemption and 0022's self-clearing default hold for a real product delete, as ADR-TEAM-MEMBERSHIP-001/002 predicted, with no special casing.

## Acceptance Criteria

- [ ] Deleting "Bakcend" (Sam, no projects) → "Deleted Bakcend.". Sam's dashboard then shows "You're not on any team yet — ask a workspace admin to add you."
- [ ] Ops as the default → the confirm page warns. After the delete, `/teams` shows "No team".
- [ ] Backend with Identity Platform (AUTH) → "Backend still has 1 project(s): Identity Platform (AUTH). A team with projects can't be deleted.", with no button and no change.
- [ ] A stale confirm sent after Lena created Brand Refresh (BRAND) → the refusal naming BRAND, and both still exist.
- [ ] 100 delete-versus-create races on real Postgres → 0 lost projects, 0 responses of 500.
- [ ] Lena (lead), Sam (member), Marco and a signed-out caller get byte-identical uniform 404s on GET and POST, with zero rows deleted.
- [ ] Dogfood on dev: delete a typo team, and see a team with a project refused. Project counts are the same before and after (read-only).

## Dependencies

- Slice 01 (the members page controls). OQ-4 (recommended: refuse). OQ-6 for the record.
- Reference class: the issue-delete GET+POST confirm (ADR-ISSUE-DELETE-002) plus the lane-delete race guard (ADR-BOARD-LANE-002), about 1.5 days.
