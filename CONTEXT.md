# CONTEXT

## Current Task

**team-lifecycle** (create, rename and delete teams) shipped in **v0.14.0** (`9f22b81`, annotated tag; GPG signing failed). It is live on dev: migration 24, both `teams_name_rule_*` triggers on, the `team_lifecycle_events` table present, 0 bad team names. Prod is still v0.13.0 (`730b573`) and awaits approval. Release gates: full CI stages each green on 2026-10-12 (workspace tests in run 1, acceptance 1314/1314, deny after accepting test-only RUSTSEC-2024-0449). Later connect failures were canzan-lift's acceptance processes holding the 55xxx ports Docker publishes on (see memory).

## Key Decisions

- Delete is refused while the team has any project (team row `FOR UPDATE`, then projects re-read, proven by a 100-run race); a lead may rename; rename and delete are audited; the DB name rule caps at 64.
- 0023 keeps its FKs, so pre-0023 dumps restore only into a FRESH database. The upgrading boot warns once; old backups are to be deleted (which ones is confirmed with the user at release).

## Next Steps

- Approve v0.14.0 for prod, then verify migration 24 there read-only. Final `cargo mutants` on store teams.rs is still owed (run it when canzan-lift is idle).
- The user chose to KEEP the pre-0023 backups for now; ask again after prod is on v0.14.0. Dogfood slices 01/02/04 on dev; take the KPI-TL baselines.
- Follow-ups (see the evolution doc): delete records use `now()`, not `clock_timestamp()`; the `team_not_found_page` copies in keyboard.rs/comments.rs; OQ-D4; a tracing-capture harness (team-membership H8 plus W1).
