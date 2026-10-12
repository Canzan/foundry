# CONTEXT

## Current Task

**team-lifecycle** (create, rename and delete teams) shipped in **v0.14.0** (`9f22b81`; annotated tag, GPG signing failed). It is live on dev AND prod (promoted 2026-10-12; prod verified read-only: migration 24, both `teams_name_rule_*` triggers on, `team_lifecycle_events` present, 0 bad team names). The evolution doc is `docs/evolution/2026-10-12-team-lifecycle.md`.

## Key Decisions

- Delete is refused while the team has any project (team row `FOR UPDATE`, then projects re-read, proven by a 100-run race); a lead may rename; rename and delete are audited; the DB team-name rule caps at 64.
- 0023 keeps its FKs: pre-0023 dumps restore only into a FRESH database, and the upgrading boot warns once. The user said to forget about the old backups: leave them, don't ask again.
- Test connect failures (SSLRequest 0x48, PortNotExposed) come from canzan-lift's acceptance processes holding 55xxx ports, not Docker (see memory). Check `lsof` before restarting Docker.

## Next Steps

- Run the owed final `cargo mutants -j 1` on `crates/foundry-store/src/teams.rs` while canzan-lift is idle. Dogfood create/rename/delete on dev; take the KPI-TL baselines.
- Follow-ups are in the evolution doc's Open/deferred section: delete records use `now()`; the `team_not_found_page` copies in keyboard.rs/comments.rs; OQ-D4; a tracing-capture harness (H8 + W1).
