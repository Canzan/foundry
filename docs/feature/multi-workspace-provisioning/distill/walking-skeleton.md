# DISTILL — Walking Skeleton (multi-workspace-provisioning)

> Quinn (nw-acceptance-designer), DISTILL wave. Identifies the thinnest end-to-end cut that delivers
> observable user value and proves the driving-port wiring, demo-able to a stakeholder.

## The headline walking skeleton — slice-06 scenario 1

```gherkin
@walking_skeleton @wiring_e2e @us-mwt07
Scenario: A super-admin provisions a new isolated workspace with a first admin
  Given an instance claimed by super-admin "ops@acme.com" with workspace "Acme"
  When the super-admin provisions workspace "Globex" with first admin "priya@globex.com"
  Then the new workspace "Globex" exists and is isolated from all others
  And the command reports the new workspace and a first-admin invite link
  And "priya@globex.com" signs in and acts on "Globex"
```

**Why this is the thinnest end-to-end cut.** This single scenario exercises the entire new vertical
slice through the production composition root:

1. **The driving port** — the operator CLI subprocess `foundry doctor provision-workspace --name
   Globex --admin-email priya@globex.com`, invoked exactly as the operator invokes it (real
   `assert_cmd::Command::cargo_bin("foundry")`, real `DATABASE_URL`). This is the user's actual
   invocation path per RCA-fix P1 — not a direct `create_workspace` use-case call.
2. **The new authz gate** — `is_instance_admin(ops@acme.com)` must pass (the bootstrap-claiming
   operator is the first super-admin, D1).
3. **The new use-case + tx** — `create_workspace` → `provision_workspace` atomic tx (mirrors the
   shipped `create_initial_workspace` shape), inserting the new workspace + first admin + seeded
   team/project, plus the `0011 instance_admins` migration that makes the authz table exist.
4. **The observable outcome** — the CLI reports the new workspace id + a first-admin invite link
   (port-exposed stdout), and the new admin SIGNS IN and ACTS on the new tenant (the SHIPPED sign-in
   + `resolve_active_workspace` seam), proving the provisioned workspace is real, reachable, and
   isolated.

A non-technical stakeholder confirms: "yes — an operator can stand up a new isolated client
workspace from the shell, and the new admin can immediately log in and use it." That is the headline
user value of the entire feature (US-MWT07, mwt-job-4: "create and provision a new tenant on a
running instance"). It touches the new CLI port, the new authz, the new tx, the new migration, and
the shipped sign-in/resolution seam — the whole slice, end to end, in one demo-able journey.

## The second walking skeleton — slice-05 scenario 1 (per-feature convention)

```gherkin
@walking_skeleton @wiring_e2e @us-mwt06
Scenario: Upgrading a single-workspace install keeps it working as workspace 1
  Given a pre-feature single-workspace install of "Acme" with admin "ops@acme.com"
  And "Acme" has members, teams, projects, issues, and invites
  And "Acme" has a live signed-in session and a valid machine token
  When the install is upgraded to multi-workspace support
  Then the existing workspace becomes the first workspace with its identity unchanged
  And all of its tenant data is present and unchanged
  And "ops@acme.com" signs in and works exactly as before
```

The repo convention (and the slice-03 exemplar) is ONE `@walking_skeleton @wiring_e2e` first
scenario PER `.feature` file. Slice 05 is a structurally distinct deliverable (migration safety, a
different driving surface — the migration runner) from slice 06 (provisioning, the CLI port), so it
carries its own walking skeleton: the thinnest cut that proves the riskiest migration question (does
the upgrade lose or change any data, or break sign-in?) end-to-end against a REAL pre-feature
snapshot. It is the demo for mwt-job-3 ("bring my existing install across without losing data").

## Why two walking skeletons, not one

The feature spans two genuinely different driving surfaces with two different riskiest assumptions:

- **Slice 5** drives the **migration runner** and asks: *does forward-only upgrade touch any data?*
  (data-safety risk). Demo: a real pre-feature DB upgrades losslessly.
- **Slice 6** drives the **operator CLI** and asks: *can an operator provision a real isolated
  tenant?* (provisioning + isolation risk). Demo: provision Globex, Priya logs in isolated.

Each surface needs its own thinnest-wiring proof. If a single skeleton were forced, it would either
omit the migration-safety proof or the provisioning proof — neither is reducible to the other. The
OVERALL feature demo for stakeholders leads with slice-06 sc 1 (the headline user value); slice-05
sc 1 is the upgrade-safety reassurance shown alongside it.

## Strategy / infrastructure (inherited, per the project Infrastructure Policy)

Per `docs/architecture/atdd-infrastructure-policy.md` (inherited, four rows appended this wave):

- **CLI driving port** → real `foundry` subprocess via `assert_cmd`, `DATABASE_URL` → per-scenario
  testcontainers schema (reuses the allow-listed `run_restore_comment` scaffold).
- **Migration runner** → `support/test_migration.rs` `TestMigrationsDir` precedent: stage the
  pre-feature migration history in a tempdir, apply the canonical `0009/0010/0011` via the SAME
  `run_migrations_from_dir` the production boot path uses.
- **Driven-internal (Postgres, `instance_admins`, sessions, tokens)** → REAL, never faked
  (shared testcontainers PG16 + per-scenario schema).
- **Driven-external (clock)** → `MockClock` only where time must advance (rate-bucket eviction,
  layer 1-2 — not in these acceptance scenarios).

Both walking skeletons are `@real-io @wiring_e2e` — they use real adapters end-to-end. No
`@in-memory` on any walking skeleton (Dimension 9e clean).
