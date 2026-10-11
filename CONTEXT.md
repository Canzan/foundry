# CONTEXT

## Current Task

team-membership shipped in **v0.13.0** (`730b573`), live on dev (migration 22, both lead-guard triggers on, default_team_id + composite FK present, 0 leaderless teams); prod is still v0.12.0, awaiting approval. It adds `/teams`, members pages, add/role/remove for admins and team leads, never-leaderless (409 + migration 0021 trigger), guarded workspace removal, and a default team for invites and first Keycloak sign-ins (migration 0022). Gates: DES 12/12, 100% viable mutants, CI 1219/1219 in one run, review approved.

## Key Decisions

- Never-leaderless is a team-row `FOR NO KEY UPDATE` lock plus a DB trigger (ADR-TEAM-MEMBERSHIP-001); the default team is a composite-FK column (ADR-002).
- Under machine load, run smoke with `RUST_TEST_THREADS=1` and mutants with `-j 1`; never run hand mutation in parallel with lanes.

## Next Steps

- Done before release: 0 orphan team rows on dev and prod (OQ-D4); KPI-1 baseline dev 1 (the SSO account) / prod 0; KPI-3 0/0. Owed: the dogfood on dev (as jeffabailey, add jeff@unintelligent-design.us to General, then open Sandbox), and approving prod.
- Follow-ups: H8 log-capture test; team create/rename/delete; per-invite team; show signed-in account; name projects constraints.
- The jeffbaileyblog zsh-wasm-playground pytest leaks `zswp-oracle` containers.
