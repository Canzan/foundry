# Slice 03: Every team name follows one rule, and a name in any script gets a working URL

Stories: US-TL-04 | Estimate: 1 day | job_id: `job-team-lifecycle`

## Goal

Team names obey the same rule as workspace and project names: the shared
refused-character predicate, a cap, and one definition of "unique". A name that
slugifies to nothing still gets a reachable team page.

## IN

- Every D6 arm on create, with the D12 copy: "Team name must not be empty", "Team name must not contain control characters", "Team name must be at most 64 characters" (OQ-2), and "Team name must be unique within the workspace".
- One shared predicate. A core property test shows the same control-character verdict as `WorkspaceName` and `ProjectName`.
- Fallback slug (OQ-3): when `slugify` is empty, the slug is `team`, then `team-N` with the lowest free N from 2. Never `""`.
- Concurrency (INV-TL-4): concurrent same-name creates give one team, and the loser gets the uniqueness copy. Concurrent fallback creates both succeed, with a bounded retry.
- (OQ-7) A database rule on `teams.name` over the shared `foundry_name_rule_violation` at cap 64. It fires on insert and on a name change only, so legacy rows are untouched.
- A check-arch literal rule for `"Team name must`, if DESIGN follows the `workspace-name-one-source` precedent.

## OUT

- The rename door (slice 04 reuses this rule unchanged).
- Rewriting existing team names or slugs.

## Learning Hypothesis

- **Disproves if it fails**: that the project fallback mechanism transfers to teams, which have no key prefix. If `team`/`team-N` collides with a real team called "Team", confuses people in the URL, or the bounded retry exhausts under realistic use, the fallback needs a different base (a user decision).
- **Confirms if it succeeds**: that three name rules (workspace, project, team) share one predicate and one database verdict function, so a fourth (lanes, someday) is mechanical.

## Acceptance Criteria

- [ ] "インフラ" → `/team/team/members`. Then "運用" → `/team/team-2/members`. Both open.
- [ ] A U+202E name and a 65-character name are refused with their copy. A 64-character name is created.
- [ ] "Back-End", where Backend exists → the uniqueness copy.
- [ ] 100 concurrent same-name races give 1 team and 1 refusal each, with 0 responses of 500. Concurrent "インフラ" creates both succeed.
- [ ] (OQ-7) A psql INSERT with a NUL in `teams.name` is refused with SQLSTATE 23514.

## Dependencies

- Slice 01. OQ-2, OQ-3, OQ-7.
- Reference class: project-name-rule slices plus name-db-checks 0020 (about 1 day).
