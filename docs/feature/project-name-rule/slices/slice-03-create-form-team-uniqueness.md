# Slice 03: Create and rename mean the same thing by "unique within the team"

Story: US-PNR-03 | Estimate: 0.5 day | job_id: `job-project-naming`

## Goal

Create refuses a name that case-insensitively equals a sibling's name **or**
whose non-empty slug equals a sibling's stored slug, within the same team.
Rename already applies this definition. Both doors now call one uniqueness
check, which closes the project-rename D7 residual (OQ-2, confirmed by the user
on 2026-10-06).

## IN

- Create calls the shared uniqueness check against the team's siblings, after the pure arms (D5, D6).
- The slug arm is skipped when `slugify(name)` is empty, on both doors (D6). Slice 04 relies on this.
- The `UNIQUE (team_id, slug)` race fallback stays.
- A parity outline: the same names through create and rename give the same verdict and copy (KPI-2).

## OUT

- The fallback slug mint (slice 04). Serializing concurrent creates (the ADR-PROJECT-RENAME-002 race stays accepted). Confusable detection. A DB unique index on `lower(name)`.

## Learning Hypothesis

- **Disproves if it fails**: that the name arm refuses only real lookalikes. If the dogfood duplicate query (`GROUP BY team_id, lower(name)`) or the operator's own report shows teams that intentionally hold case-variant names, the name arm is too strict for create.
- **Confirms if it succeeds**: that one uniqueness function serves both doors with no store change, and that D7 closes without new machinery.

## Acceptance Criteria

- [ ] With "Identity Platform" (slug `auth-v2`) in Backend, creating "identity platform" (key IDP) gets 422 with "Project name must be unique within the team", and nothing is written.
- [ ] "Auth V2!" is still refused (slug arm).
- [ ] "Identity Platform" in team Frontend is accepted.
- [ ] "sandbox" through create and as a rename of "Identity Platform" gives the same 422 and copy at both doors.
- [ ] A 300-scalar case-duplicate gets the length copy.
- [ ] An empty derived slug never triggers the slug arm: renaming to "🚀" next to a legacy `""` sibling is not refused for its slug.
- [ ] Dogfood: the same-team case-insensitive duplicate count on the operator's instance is recorded as the KPI-4 baseline.

## Dependencies

Slice 02 (create on the shared rule). Reference class: precedent project-rename slice 03 (uniqueness arms, about half a day).
