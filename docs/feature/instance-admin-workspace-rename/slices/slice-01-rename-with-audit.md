# Slice 01: Rename a workspace from the dashboard, with the change on record

Story: US-IAWR-01 | Estimate: 1-1.5 days | job_id: `job-instance-workspace-rename`

## Goal

An instance admin renames a workspace from its dashboard row. Invalid names
are refused inline, every effective rename leaves one append-only audit entry,
and the sidebar brand never breaks its line.

## IN

- Rename form plus `[data-error-slot]` (with the `data-error-target` child-selector) on each workspace row of `/admin/instance/workspaces`.
- `POST /admin/instance/workspaces/{workspace_id}/rename` under the CSRF middleware and session layer. Responses are 200 row, 422 fragment, or uniform 404.
- Validation: trim, non-empty, at most 24 Unicode scalars. Copy: "Workspace name must not be empty" / "Workspace name must be at most 24 characters".
- Same-name submission is a quiet 200 with no write and no audit entry (D4, user-confirmed).
- New append-only audit record (actor, workspace, old, new, timestamp), written atomically with the rename. One new migration.
- Sidebar `.sidebar__workspace` single-line ellipsis plus `title` (desktop and narrow breakpoint).

## OUT

- DB CHECK on `workspaces.name` (D10, option A). Follow-ups: B (same rule on provisioning, bootstrap and admin CLI), then D (`CHECK ... NOT VALID`).
- Uniqueness rule (D7); audit viewer or retention; project-rename auditing.
- Same-swap refresh of the renaming admin's own sidebar (next page load is enough).

## Learning Hypothesis

- **Disproves if it fails**: the hypothesis that the shipped per-row rename pattern (project rename) carries over to workspace rows with an atomic side-write and no new interaction idiom. If the audit append forces a different error-handling or transaction shape at the handler, the pattern is less reusable than assumed.
- **Confirms if it succeeds**: the hypothesis that a 24-character cap plus ellipsis keeps every real workspace name legible in the 164px brand box, checked against the operator's actual workspace names on her instance.

## Acceptance Criteria

- [ ] "Bailey Family" renamed to "Household": the row swaps without a reload, the member sidebar shows "H  Household" on the next load, and exactly 1 audit entry exists.
- [ ] "Canzan Labs Platform Ops" (24 characters) is accepted. "Canzan Labs Platform Team" (25) and empty or whitespace names get 422 inside the row (repeatable), with no write and no audit entry.
- [ ] " Household " on "Household" returns 200 with no audit entry.
- [ ] Marco, signed-out, malformed-id and unknown-id requests get the byte-identical uniform 404 with no write. A missing `_csrf` gets 403 from the middleware.
- [ ] A 52-character legacy name renders on one sidebar line with an ellipsis and full-name `title` (browser lane).
- [ ] Production-data dogfood: Priya renames a real workspace on her instance and confirms the sidebar and audit entry the same day.

## Taste Test

The slice touches about 5 components (migration, store write, use-case,
handler with row template, and sidebar CSS). That is over the "4+ components
means not thin" line. It is kept as one slice by the user's choice ("ideally one
slice"), because each piece is a small copy of a shipped idiom. If DESIGN
estimates more than 1 day, split the sidebar safety net (CSS plus `title`,
independently shippable) out as slice-00 to land first.

## Dependencies

None outstanding. Reference class: `instance-admin-project-rename` slices 02
and 03 (1 day each).
