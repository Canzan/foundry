# Slice 01: A team's lead renames a project from the team page

Stories: US-PL-01 | Estimate: 1 day | job_id: `job-project-lifecycle`

## Goal

The team's members page lists the team's projects. The team's lead, or a
workspace admin, renames one there. Only the name changes: the slug, the key,
every URL and every issue id stay.

## IN

- A **Projects** section on `/team/{slug}/members`: "{project} · {KEY}" rows for anyone who can view the team, each linking to its board only when the viewer is on the team (OQ-U1 = (b), DESIGN Changed Assumption 2); "{team} has no projects yet." when empty (D8).
- Per row, for `Manage` (the team's lead or a workspace admin): a prefilled **Project name** field and **Rename** (D-B).
- `POST /team/{team_slug}/project/{project_slug}/rename` (proposed, D15): the shipped `ProjectName` rule and copy, the project excluded from uniqueness; a byte-equal name writes nothing; "Renamed {old} to {new}." (D2, D3).
- (OQ-3) One rename record per effective rename, on this door and on the shipped instance-admin door.
- The uniform 404 for everyone else, on both verbs (D4). Works without scripting.

## OUT

- Delete, restore (slices 02, 03). Changing a slug, URL or key (OQ-2).
- A UI for the rename record.

## Learning Hypothesis

- **Disproves if it fails**: that the instance-admin rename use-case (ADR-PROJECT-RENAME-002) can serve a second, team-scoped door unchanged. If the team door needs a different rule or a different uniqueness meaning, "one rule on both doors" (job-project-naming) breaks.
- **Confirms if it succeeds**: that `TeamAccess::can_manage` is the right gate for project management, as D-B asserts, with no new arm.

## Acceptance Criteria

- [ ] Lena renames Identity Platform (AUTH) to "Identity & Access" → "Renamed Identity Platform to Identity & Access."; `/team/backend/project/auth/issues/7` still opens AUTH-7.
- [ ] Priya (admin, not on Ops) renames Homelab Ops to "Homelab Operations"; the address stays `/team/ops/project/homelab-ops`.
- [ ] Reading List → "homelab operations" is refused with "Project name must be unique within the team", text kept.
- [ ] A byte-equal name writes nothing and records nothing.
- [ ] Sam (member), Marco, Hugo (Globex admin), Lena on an Ops project and a signed-out caller get byte-identical uniform 404s with zero writes; Sam sees no Rename.
- [ ] Dogfood on dev: rename a real project and reopen a bookmark taken before.

## Dependencies

- team-lifecycle (shipped): the members page and `TeamAccess`. OQ-2 (recommended: slug and key fixed), OQ-3 (recommended: records).
- Reference class: team-lifecycle slice 04 (team rename), about 1 day.
