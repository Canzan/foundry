# Slice 04: A team lead manages their own team, and only their own team

Story: US-TM-05 | Estimate: 0.5-1 day | job_id: `job-team-membership`

## Goal

A team lead who is not a workspace admin can add workspace members to their own
team, change roles there, and remove people from it, under the same rules as a
workspace admin. On every other team they get the uniform 404.

## IN

- The authz predicate for the members-page controls and the three POSTs widens from "workspace admin" to "workspace admin, or lead of this team" (D2, OQ-2).
- Authority is evaluated at request time (INV-6). A demoted lead's stale page gets a 404.
- Self-demotion and self-removal are allowed when another lead remains (INV-4).

## OUT

- New routes. Leads setting the workspace default team (slice 05, admin only). Leads of one team acting on another.

## Learning Hypothesis

- **Disproves if it fails**: that one predicate ("admin, or lead of this team") cleanly covers all three verbs. If any verb needs a special case (for example, a lead removing a workspace admin), the role model needs a user decision.
- **Confirms if it succeeds**: that the `lead` role carries real, bounded power for the first time, with no new existence oracle (a lead who is a plain member of another team still gets 404s on that team's writes).

## Acceptance Criteria

- [ ] Lena (lead of Backend, not admin) adds "Tomás Rivera — tomas.rivera@canzan.dev" → "Added Tomás Rivera (tomas.rivera@canzan.dev) to Backend as a member."
- [ ] Lena makes Sam lead, then makes herself a member. Sam is the only lead, and Lena's controls are gone.
- [ ] Lena's add, role and remove on Ops (where she is a Member) and on General (not a member) → uniform 404, and nothing changes. On Ops she sees the list read-only.
- [ ] Demoted Lena submits from a page opened earlier → uniform 404.
- [ ] Dogfood on dev: promote the SSO account to Lead of General, sign in as it, and add a test account to General.

## Dependencies

- Slices 01-03. OQ-2's recommended answer is assumed.
