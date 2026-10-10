# CONTEXT

## Current Task

**team-membership:** DELIVER is in progress; 4 of 12 steps are done, and 02-02 is next. Everything is committed locally and NOT pushed (the user says no pushes until told).
- Done: DISCUSS, DESIGN, DISTILL, the roadmap, slice 01 (01-01 `11785f0`, 01-02 `2e9a5f8`, 01-03 `3643a45`; an admin can see teams and members and add a member to a team), and 02-01 `021cd47` (migration 0021 never-leaderless trigger plus `set_team_role`; mutation 100%).
- Under machine load, smoke needs `RUST_TEST_THREADS=1`; otherwise Docker container start-up flakes (PortNotExposed/PoolTimedOut).

## Key Decisions

- The user resolved every OQ to its recommendation: leads are peers; workspace removal cleans up team rows and refuses removing a last lead; the default team stays in this feature, with no backfill.
- Never leaderless: the store takes `FOR NO KEY UPDATE` on the team row, plus a DB trigger (23514 `team_memberships_team_keeps_a_lead`), per ADR-TEAM-MEMBERSHIP-001. The default team is `workspaces.default_team_id` with a composite FK (ADR-002).
- Unreleased locally: the `e68b7a0` board button-spacing fix. v0.12.0 is live on dev and prod.

## Next Steps

- Finish 02-01, then 02-02 through 06-02 (`docs/feature/team-membership/deliver/roadmap.json`), then finalize (evolution doc, no push).
- Owed after a release: the orphan team-row count on dev and prod before 0021; the dogfood test (add jeff@unintelligent-design.us to General on dev, then open Sandbox).
- Docker: the jeffbaileyblog zsh-wasm-playground pytest leaks `zswp-oracle` containers (`docker run --rm -it … zsh -l` never exits); about 1,500 were stopped with the user's OK.
