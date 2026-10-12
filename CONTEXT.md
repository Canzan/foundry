# CONTEXT

## Current Task

**team-lifecycle** (create, rename and delete teams): DELIVER is in progress; 9 of 13 steps are done: slices 01-03 are complete (create; delete, refused while projects exist; the team-name rule in app and DB, migration 0024; last commit `c30622a`). Slice 04 (rename) is in progress: 04-01 `30e043b` (store rename plus audit; clock_timestamp ordering fix) is done, 04-02 `cd52dd5` adds the rename UI for admins and leads, and 04-03 (rename refusals) is next. 11 of 13 steps are done. Committed locally and NOT pushed. Done:
- slice 01: admins create teams and lead them (`f14e979`, `23d2174`, `25857a7`, `e44a2da`);
- 02-01: migration 0023 (the `team_lifecycle_events` audit table), a delete that never loses a project (store-proven, 100-run race), and requests that lose their team answer 404/422 instead of 500.

**Resume:** `/nw:continue team-lifecycle` picks up at the first step without a COMMIT/PASS. The step contexts are in `docs/feature/team-lifecycle/deliver/roadmap.json`; owed items are in `deliver/finalize-owed.md`. Earlier: team-membership shipped in v0.13.0 (live on dev; prod awaiting approval).

## Key Decisions

- The user decided: refuse delete while the team has projects; a lead may rename; audit rename and delete; a DB team-name rule at cap 64.
- 0023 keeps its foreign keys, so a pre-0023 dump can no longer be restored OVER an upgraded database. The upgrading boot logs one warning (not on a fresh install), and the CHANGELOG says to restore old dumps into a fresh database.

## Next Steps

- 02-02 to 04-04, then finalize.
- At release, ask the user which pre-0023 backups to delete (per-workspace backup files and/or the homelab CNPG backups). The user said to delete the old backups; nothing has been deleted yet.
- Follow-ups are in `docs/feature/team-lifecycle/deliver/finalize-owed.md`.
