# Slice 04: A team is renamed, and every link to it keeps working

Stories: US-TL-05 | Estimate: 1 day | job_id: `job-team-lifecycle`

## Goal

A workspace admin, or the team's lead, renames a team from its members page.
The name changes everywhere it is read, and the slug, and therefore every URL,
bookmark and board link, stays exactly as it was.

## IN

- On the members page, for `TeamAccess` Manage (a workspace admin, or this team's lead per OQ-1): a **Team name** field prefilled with the current name, and **Rename**.
- `POST /team/{slug}/rename` (proposed, D13) writes the name only, never the slug (D4), and shows "Renamed {old} to {new}."
- The same rule and copy as create, with the team itself excluded from the uniqueness check (D6).
- A byte-equal name is not a write (no record, no notice). A case-only change is a rename (D15).
- (OQ-6) One rename record (actor · old → new · when), committed with the write.
- Uniform 404 for a member, an outsider or a signed-out caller (D5). It works without scripting.

## OUT

- Changing the slug or URL. Redirects. A UI for rename records.

## Learning Hypothesis

- **Disproves if it fails**: that no render path re-derives a team slug from its name. If any page, link or token label breaks or changes after a rename (the defect ADR-PROJECT-RENAME-001 found for projects), "names are labels, slugs are identity" does not yet hold for teams, and a check-arch rule is needed.
- **Confirms if it succeeds**: that `TeamAccess` Manage covers a fourth verb with no new arm (ADR-TEAM-MEMBERSHIP-003).

## Acceptance Criteria

- [ ] Priya renames Backend to "Platform Engineering" → "Renamed Backend to Platform Engineering.". `/teams` lists the new name. `/team/backend/members` and `/team/backend/project/auth` answer 200.
- [ ] Lena (lead, not admin) renames Backend → the same copy (OQ-1).
- [ ] Ops to "general" → "Team name must be unique within the workspace", and Ops keeps its name.
- [ ] Submitting "Backend" unchanged → no notice, and no record.
- [ ] Sam (member), Marco and a signed-out caller → byte-identical uniform 404s, with the name unchanged.
- [ ] Dogfood on dev: rename a team, then open a bookmark taken before the rename.

## Dependencies

- Slices 01 and 03 (the rule). OQ-1, OQ-6.
- Reference class: instance-admin project rename (about 1 day).
