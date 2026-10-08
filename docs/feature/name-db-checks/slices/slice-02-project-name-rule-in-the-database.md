# Slice 02: The database refuses a project name the app would refuse, and legacy projects keep taking issues

Story: US-NDC-02 | Estimate: 1 day | job_id: `job-name-rule-below-the-app`

**Blocked on OQ-1** (the enforcement shape for `projects`). The ACs below hold for
every OQ-1 option except (c), which drops this slice.

## Goal

A `psql` INSERT or UPDATE that would give a project a name `ProjectName` refuses
(empty, padded, a D4 character, more than 256 characters) fails with SQLSTATE
23514 and a constraint name stating the reason. A project whose name broke the
rule before this change still gets new issues (OPS-8 after OPS-7). Its other
columns can still be written.

## IN

- The project rule at the database, enforced on new names only (D1, D2, D4, D6, and OQ-1's answer). Parity with the pure arms of `ProjectName::try_new` (D3). Uniqueness stays in the app (D12).
- One named refusal per arm (D5).
- The project half of the legacy seam (D8). F3-F6 (the project-name-rule legacy outline rows and the LNG row) move onto it. The `slug = ''` "Ωμέγα" rows have valid names and need no change.
- Legacy guard: a legacy project "Homelab\tOps" (OPS) still files OPS-8 through the new-issue dialog. A byte-equal rename is a quiet no-op, and it renames to "Homelab Ops".
- Parity property test for the 256 rule.

## OUT

- Workspaces (slice 01). Restore and rollback (slice 03). VALIDATE. Slug or key-prefix rules. Uniqueness at the database.

## Learning Hypothesis

- **Disproves if it fails**: that the new-name rule can be enforced without touching legacy rows. The new-issue scenario on "Homelab\tOps" is the probe. Today's `UPDATE projects SET next_issue_number` (`lib.rs:1746`) writes the whole row. A row-level CHECK would refuse it with 23514, and the user would see a 500 on every new issue for that project.
- **Confirms if it succeeds**: that the chosen OQ-1 shape refuses a bad name on the name write and on nothing else.

## Acceptance Criteria

- [ ] `UPDATE projects SET name = E'Sand\tbox'` and `INSERT … name = repeat('x', 257)` each fail with 23514 and the constraint for their arm. Nothing changes.
- [ ] A 256-character name of "日" and "Café Roadmap 👨‍👩‍👧" are accepted.
- [ ] Legacy "Homelab\tOps" (OPS): Priya files "Replace UPS battery" and gets OPS-8. The board, the report and the dashboard row render as before.
- [ ] Legacy no-op and real rename on "Homelab\tOps" behave as shipped (project-name-rule D8).
- [ ] Parity: 0 disagreements. The iapr, project-name-rule, us-07 and us-08 lanes stay green.
- [ ] Dogfood: count of existing project names that break the rule, recorded as the KPI-1 baseline (OQ-3).

## Dependencies

- **OQ-1 must be answered first.** OQ-3 (the dogfood count) informs it.
- Slice 01's legacy seam and parity harness are reused. Only the table changes.
