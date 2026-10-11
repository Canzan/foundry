# CONTEXT

## Current Task

**team-membership is delivered and finalized, committed locally and NOT pushed** (the user says no pushes or releases until told). It adds:
- `/teams` and per-team members pages;
- add, make lead/member, and remove, for workspace admins and a team's own leads;
- never leaderless: 409, plus migration 0021's DB trigger;
- workspace removal clears team rows and refuses removing a last lead;
- a default team for new members (migration 0022), used by invites and first Keycloak sign-ins.

Commits run `e4c8bcd` → `16608ee`, plus finalize. Evolution doc: `docs/evolution/2026-10-10-team-membership.md`.

Gates:
- all 12 DES steps complete;
- 100% of viable mutants killed;
- full CI 1219/1219 in one run;
- adversarial review approved.

Also unpushed: `e68b7a0`, the board button spacing fix.

## Key Decisions

- Leads are peers. Never-leaderless is enforced by a team-row `FOR NO KEY UPDATE` lock in the store plus a DB trigger (23514 `team_memberships_team_keeps_a_lead`), per ADR-TEAM-MEMBERSHIP-001.
- The default team is `workspaces.default_team_id` with a composite FK (same workspace) and `ON DELETE SET NULL` (ADR-002). Placement is in a savepoint and never fails sign-up.
- Under machine load, run smoke with `RUST_TEST_THREADS=1` and cargo-mutants with `-j 1`. Don't run hand mutation in parallel with lanes.

## Next Steps

- On "push"/release (v0.13.0): run the read-only orphan team-row count on dev and prod before 0021 (OQ-D4). Then dogfood on dev: add jeff@unintelligent-design.us to General and open Sandbox. Record the KPI-1 and KPI-3 baselines.
- Follow-ups:
  - H8: a log-capturing test for spurious placement warnings;
  - team create/rename/delete;
  - per-invite team choice;
  - show the signed-in account and link the Keycloak identity;
  - name the projects constraints.
- The jeffbaileyblog zsh-wasm-playground pytest leaks `zswp-oracle` containers; fix it in that repo.
