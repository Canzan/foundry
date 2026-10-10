# CONTEXT

## Current Task

**team-membership:** DELIVER is in progress, at step 02-01 of 12. Everything is committed locally and NOT pushed (the user says no pushes until told).
- Done: DISCUSS, DESIGN, DISTILL, the roadmap, and slice 01 (01-01 `11785f0`, 01-02 `2e9a5f8`, 01-03 `3643a45`). An admin can see teams and members and add a member to a team.
- 02-01 (migration 0021, the never-leaderless trigger, plus `set_team_role`) is uncommitted in the working tree. It is waiting on a test fix: one new test uses `DISABLE TRIGGER`, which check-arch bans; switch it to `DROP TRIGGER`. Then rerun smoke and mutation testing and des-commit.

## Key Decisions

- The user resolved every OQ to its recommendation: leads are peers; workspace removal cleans up team rows and refuses removing a last lead; the default team stays in this feature, with no backfill.
- Never leaderless: the store takes `FOR NO KEY UPDATE` on the team row, plus a DB trigger (23514 `team_memberships_team_keeps_a_lead`), per ADR-TEAM-MEMBERSHIP-001. The default team is `workspaces.default_team_id` with a composite FK (ADR-002).
- Unreleased locally: the `e68b7a0` board button-spacing fix. v0.12.0 is live on dev and prod.

## Next Steps

- Finish 02-01, then 02-02 through 06-02 (`docs/feature/team-membership/deliver/roadmap.json`), then finalize (evolution doc, no push).
- Owed after a release: the orphan team-row count on dev and prod before 0021; the dogfood test (add jeff@unintelligent-design.us to General on dev, then open Sandbox).
- Docker: the jeffbaileyblog zsh-wasm-playground pytest leaks `zswp-oracle` containers (`docker run --rm -it … zsh -l` never exits); about 1,500 were stopped with the user's OK.
