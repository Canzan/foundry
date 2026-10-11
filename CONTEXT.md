# CONTEXT

## Current Task

team-membership is delivered and finalized (`e4c8bcd`→`936c4d8`), committed locally and NOT pushed: user said no push or release until told. It adds `/teams`, members pages, add/role/remove for admins and team leads, never-leaderless (409 + migration 0021 trigger), guarded workspace removal, and a default team for invites and first Keycloak sign-ins (migration 0022). Gates: DES 12/12, 100% viable mutants, CI 1219/1219 in one run, review approved. Also unpushed: `e68b7a0`, the board button spacing fix.

## Key Decisions

- Never-leaderless is a team-row `FOR NO KEY UPDATE` lock plus a DB trigger (ADR-TEAM-MEMBERSHIP-001); the default team is a composite-FK column (ADR-002).
- Under machine load, run smoke with `RUST_TEST_THREADS=1` and mutants with `-j 1`; never run hand mutation in parallel with lanes.

## Next Steps

- On release (v0.13.0): first count orphan team rows on dev and prod (OQ-D4); then dogfood on dev (jeff@unintelligent-design.us → General, open Sandbox) and record KPI-1/KPI-3.
- Follow-ups: H8 log-capture test; team create/rename/delete; per-invite team; show signed-in account; name projects constraints.
- The jeffbaileyblog zsh-wasm-playground pytest leaks `zswp-oracle` containers.
