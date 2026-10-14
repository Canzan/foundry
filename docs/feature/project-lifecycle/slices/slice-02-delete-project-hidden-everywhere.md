# Slice 02: A team's lead deletes a project, and it answers nowhere

Stories: US-PL-02, US-PL-03 | Estimate: 2 days | job_id: `job-project-lifecycle`

## Goal

The team's lead, or a workspace admin, deletes a project through a confirm page
that says nothing is erased. Afterwards the project is gone from every list, and
every web and API address under it answers exactly as an address that never
existed, for everyone. No row under it changes.

## Why 2 days (taste-test exception)

The delete and the closing of every door are one guarantee. A delete that ships
while `/api/v1/…/projects/{p}/…`, the report or the live-update stream still
serve the project is a "deleted" project that answers, which is a leak. An
infrastructure-only "close the remaining doors" slice is forbidden. So both
stories ship together.

## Release rule

Slice 02 is not tagged for release without slice 03 (restore). Until then, a
delete is irreversible for anyone without psql.

## IN

- **Delete project** per row on the members page, for `Manage` (D-B, D8).
- `GET /team/{t}/project/{p}/delete`: "Delete {project}?", the issue count, the two consequence lines, **Delete project** and **Cancel** (D7, D14).
- `POST /team/{t}/project/{p}/delete`: marks the project deleted (who, when), "Deleted {project}." on the members page (D-A).
- Every web route under `/team/{t}/project/{p}/…` (attachment upload and download included) and every API route under `/api/v1/teams/{t}/projects/{p}/…` answers a deleted project exactly as the same route with a never-existed slug: the uniform not-found (the live updates: their shipped 403 text/plain); the dashboard, nav, Projects list and instance dashboard omit it (D5, INV-PL-2; DISTILL: there is no new-issue project picker, see Upstream notes).
- Races with issue, comment, attachment and lane writes: never a 500 (INV-PL-3).
- (OQ-3) One delete record.

## OUT

- Restore (slice 03). Purge (D-C). The team delete change (slice 04).

## Learning Hypothesis

- **Disproves if it fails**: that one deleted marker, applied at one resolution seam, closes every project address. If the inventory outline finds a door that still answers (a store reader the seam does not cover), soft delete needs a structural guard (a view or a check-arch rule) before it is safe.
- **Confirms if it succeeds**: that `comments.deleted_at`'s soft-delete pattern scales from one row kind to a whole aggregate.

## Acceptance Criteria

- [ ] Lena deletes Auth v2 Spike (SPIKE: 3 issues, 2 comments, 1 attachment, 4 lanes) after reading the confirm page → "Deleted Auth v2 Spike."; the store still holds exactly those rows, unchanged.
- [ ] After the delete, every address of the project (the board, SPIKE-2, the report, the CSV, the new-issue form, comments, attachments, lanes and the live updates) answers Lena, Sam and Priya exactly as the same address of a never-existed project: the uniform 404, and for the live updates their shipped 403 text/plain (Changed Assumption 1; edited at DISTILL 2026-10-13); the API answers its not-found for SPIKE's issues, history, comments and tokens.
- [ ] Sam's dashboard and Board link, Backend's Projects section, and the instance dashboard no longer list it (DISTILL: the new-issue dialog is per project and has no project picker; its address is in the inventory above).
- [ ] Sam's stale edit dialog save → the uniform 404, SPIKE-2 unchanged.
- [ ] 100 delete-versus-issue-create races on real Postgres → 0 responses of 500, 0 lost issues.
- [ ] Sam, Marco, Hugo, Lena on an Ops project and a signed-out caller get byte-identical 404s on both verbs, zero writes.
- [ ] Dogfood on dev: delete a scratch project; check its board, an issue link and an API call answer not-found; read-only counts unchanged.

## Dependencies

- Slice 01 (the Projects section). OQ-3, OQ-5.
- Reference class: team-lifecycle slice 02 (confirm page plus commit-time guard, 1.5 days) plus the address sweep.
