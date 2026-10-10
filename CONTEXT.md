# CONTEXT

## Current Task

**team-membership:** DELIVER is in progress; 9 of 12 steps are committed (05-01 `ca34629`: migration 0022, default team, invite placement); 05-02 is next.
- Done: DISCUSS, DESIGN, DISTILL, the roadmap, and slices 01-04: admins list teams and members, add (01-0x), change roles with the last lead protected (02-01 `021cd47`, 02-02 `6118e23`), remove from team and guarded workspace removal (03-01 `1ec818f`, 03-02 `a41fd2c`, hardening `eda1755`), and team leads manage their own team (04-01 `ea653cf`). Mutation is 100% on every step.
- 05-01 (migration 0022, the default team, invite placement, the probe line) passes its store tests; lanes, smoke and the commit are running. Left: 05-02 (default-team setting on /teams), 06-01 (Keycloak newcomers placed), 06-02 (CHANGELOG and burn-down), then finalize.
- Under machine load, smoke needs `RUST_TEST_THREADS=1`; otherwise Docker container start-up flakes (PortNotExposed/PoolTimedOut).

## Key Decisions

- The user resolved every OQ to its recommendation: leads are peers; workspace removal cleans up team rows and refuses removing a last lead; the default team stays in this feature, with no backfill.
- Never leaderless: the store takes `FOR NO KEY UPDATE` on the team row, plus a DB trigger (23514 `team_memberships_team_keeps_a_lead`), per ADR-TEAM-MEMBERSHIP-001. The default team is `workspaces.default_team_id` with a composite FK (ADR-002).
- Unreleased locally: the `e68b7a0` board button-spacing fix. v0.12.0 is live on dev and prod.

## Next Steps

- Finish 02-01, then 02-02 through 06-02 (`docs/feature/team-membership/deliver/roadmap.json`), then finalize (evolution doc, no push).
- Owed after a release: the orphan team-row count on dev and prod before 0021; the dogfood test (add jeff@unintelligent-design.us to General on dev, then open Sandbox).
- Docker: the jeffbaileyblog zsh-wasm-playground pytest leaks `zswp-oracle` containers (`docker run --rm -it … zsh -l` never exits); about 1,500 were stopped with the user's OK.
