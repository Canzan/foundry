# Slice 04: A team whose projects are all deleted can be deleted

Stories: US-PL-05 | Estimate: 1 day (0.5 day if OQ-1 is answered (a)) | job_id: `job-project-lifecycle`

Written to OQ-1's recommended answer, (b).

## Goal

A team delete is refused only by **live** projects. A team whose projects are
all deleted can be deleted, after a confirm page that says its deleted projects
are kept but can no longer be restored. Every row of those projects survives.

## IN

- The team delete's projects check (page and POST, commit time) counts live projects only (amends team-lifecycle INV-TL-1, OUT-25).
- The confirm page's extra line: "{team} also has {k} deleted project(s): {list}. They are kept, but can't be restored after {team} is deleted, and their keys stay taken in {workspace}." (D14).
- The deleted projects' rows survive the team row (INV-PL-6). `restore-project` on such a project exits 4: "its team {slug} was deleted".
- Restore versus team delete: never a live project lost.

## OUT

- Restoring a detached project into another team (follow-up, needs move). Purge.

## Learning Hypothesis

- **Disproves if it fails**: that team-lifecycle's `FOR UPDATE` delete transaction can keep deleted projects without loosening its guarantee. If a restore can slip a live project past the commit-time check (100 races), the detachment needs a different lock order.
- **Confirms if it succeeds**: that TL-KPI-5's demand (teams blocked by projects) is met by project delete alone, without move.

## Acceptance Criteria

- [ ] Labs with only PROTO (deleted, 12 issues) → the confirm page warns; "Deleted Labs."; PROTO's 12 issues still in the store.
- [ ] Ops with Homelab Operations (live) and Reading List (deleted) → refused, naming only Homelab Operations (OPS).
- [ ] `restore-project` on PROTO after Labs is gone → exit 4 with the reason, nothing changes.
- [ ] 100 restore-versus-team-delete races on real Postgres → 0 lost projects, 0 lost issues, 0 unmapped errors.
- [ ] Dogfood on dev: delete a scratch team whose only project is deleted; read-only counts of that project's rows unchanged.

## Dependencies

- Slices 02 and 03. OQ-1 (sequencing: not started until OQ-1 is answered). team-lifecycle's ADR-TEAM-LIFECYCLE-002 is amended in DESIGN.
- Reference class: team-lifecycle slice 02's race work, about 1 day.
