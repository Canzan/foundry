# CONTEXT

## Current Task

**team-lifecycle** (create, rename and delete teams) is delivered and finalized (`0b961ea`→HEAD; evolution doc `docs/evolution/2026-10-12-team-lifecycle.md`), committed locally and NOT pushed: the user says no push or release until told. All 13 DES steps are complete, the adversarial review APPROVED it, and all 54 scenarios / 95 rows are green. Migrations 0023 (`team_lifecycle_events` audit) and 0024 (DB team-name rule). Earlier: team-membership shipped in v0.13.0, live on dev AND prod (`730b573`; prod verified read-only 2026-10-12: migration 22, both lead-guard triggers on, default-team column and FK present, 0 leaderless teams).

## Key Decisions

- Delete is refused while the team has any project (team row `FOR UPDATE`, then projects re-read, proven by a 100-run race); a lead may rename; rename and delete are audited; the DB name rule caps at 64.
- 0023 keeps its FKs, so pre-0023 dumps restore only into a FRESH database. The upgrading boot warns once; old backups are to be deleted (which ones is confirmed with the user at release).

## Next Steps

- Before release (v0.14.0): one uninterrupted `FOUNDRY_XTASK_INCLUDE_DOCKER=1 cargo xtask ci` plus a final `cargo mutants` on store teams.rs. Blocked on 2026-10-12 by Docker Desktop connect flakes (PortNotExposed/PoolTimedOut) under heavy load.
- At release: confirm which pre-0023 backups to delete; dogfood slices 01/02/04 on dev; take the KPI-TL baselines.
- Follow-ups (see the evolution doc): delete records use `now()`, not `clock_timestamp()`; the `team_not_found_page` copies in keyboard.rs/comments.rs; OQ-D4; a tracing-capture harness (team-membership H8 plus W1).
