# Slice 01: The database refuses a workspace name the app would refuse

Story: US-NDC-01 | Estimate: 1 day | job_id: `job-name-rule-below-the-app`

## Goal

A `psql` INSERT or UPDATE that would give a workspace a name `WorkspaceName`
refuses (empty, padded, a D4 character, more than 24 characters) fails with
SQLSTATE 23514 and a constraint name that states the reason. Rows that already
break the rule stay as they are and keep working. No app door changes.

## IN

- The workspace rule at the database, added without scanning existing rows (D1, D2, D4). Parity with `WorkspaceName::try_new` (D3).
- One named refusal per arm, so the `psql` error names the reason (D5).
- The one test-support legacy seam for workspaces (D8). F1 (the iawr 53-character name) and F2 (iwnr "Canzan\tLabs") move onto it. F7 (compose `pre-claimed-…`, 31 characters) gets a valid name. F8 is already staged and needs no change.
- Parity property test: Rust verdict equals database verdict over generated names, plus the shared example table (KPI-2).
- Legacy guard: a legacy over-length name and a legacy tab name boot, list, render, give a quiet no-op on resubmission, and rename to a valid name (D6).
- Migration test: the constraints exist, are not validated, and apply cleanly to a database that already holds legacy rows.

## OUT

- Projects (slice 02). Restore and rollback proof (slice 03). VALIDATE. Rewriting legacy rows. Uniqueness.

## Learning Hypothesis

- **Disproves if it fails**: that the database can express the app rule exactly. Any generated name where Rust and Postgres disagree (for example a trim or whitespace class, `char_length` versus scalar count, or a regex range) disproves D3. Since every door would then turn that name into a 500, the slice stops.
- **Confirms if it succeeds**: that the legacy seam keeps every shipped legacy-premise scenario green (the iawr and iwnr lanes stay at their counts) with only fixture changes, and that the only production UPDATE of `workspaces` (`lib.rs:1438`) writes the name.

## Acceptance Criteria

- [ ] `UPDATE workspaces SET name = E'House\tHold'` fails with 23514. The constraint name says "control characters". The stored name is unchanged.
- [ ] `''`, `' Globex'`, a 25-character name and `'Ops'||U&'\202E'||'x'` each fail with the constraint for their arm. A 24-character name, "👨‍👩‍👧 Bailey" and "Ångström Øresund Société" are accepted.
- [ ] A database already holding "Canzan Labs Platform Engineering Group" (38) and "Canzan\tLabs" migrates without error. Both rows still list, render, resubmit as a quiet no-op, and rename to "Canzan Labs".
- [ ] Parity: 0 disagreements between the app rule and the database rule.
- [ ] Every app door's shipped workspace lanes stay green (iawr, iwnr, iapr, web-provisioning, mwt-slice-06, us-05 bootstrap).
- [ ] Dogfood (read-only, via the documented ssh path): count of existing workspace names that break the rule, recorded as the KPI-1 baseline.

## Dependencies

- OQ-2 (the test-fixture seam shape) is a DESIGN choice inside D8. It does not block.
- Reference class: migration 0018 plus its shape and CHECK tests (instance-admin-workspace-rename DDD-5), about half a day. Fixture repair adds the rest (see the inventory).
