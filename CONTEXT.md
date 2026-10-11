# CONTEXT

## Current Task

**team-lifecycle** (create, rename and delete teams): DELIVER is in progress; slice 01 is done (01-01 `f14e979`, 01-02 `23d2174`, 01-03 `25857a7`+`e44a2da`), and 02-01 (migration 0023 and a delete that never loses a project) is next. It is committed locally and NOT pushed. The user decided: refuse delete while the team has projects; a lead may rename; audit rename and delete; a DB name rule at cap 64. Earlier: team-membership shipped in v0.13.0 (dev live; prod awaiting approval).

## Key Decisions

- Never-leaderless is a team-row `FOR NO KEY UPDATE` lock plus a DB trigger (ADR-TEAM-MEMBERSHIP-001); the default team is a composite-FK column (ADR-002).
- Under machine load, run smoke with `RUST_TEST_THREADS=1` and mutants with `-j 1`; never run hand mutation in parallel with lanes.

## Next Steps

- Done before release: 0 orphan team rows on dev and prod (OQ-D4); KPI-1 baseline dev 1 (the SSO account) / prod 0; KPI-3 0/0. Owed: the dogfood on dev (as jeffabailey, add jeff@unintelligent-design.us to General, then open Sandbox), and approving prod.
- Follow-ups: H8 log-capture test; team create/rename/delete; per-invite team; show signed-in account; name projects constraints.
- The jeffbaileyblog zsh-wasm-playground pytest leaks `zswp-oracle` containers.
