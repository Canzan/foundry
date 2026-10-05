# CONTEXT

## Current Task

The release-version-footer build stamp (US-RVF-02) shipped in v0.8.0 (`a610dd9`). The dev footer reads `Foundry v0.8.0 · 2026-10-04` with `data-commit="a610dd9"`; prod verified on v0.8.0 with the same stamp (AC-8 met). Follow-ups are done: a pure, tested `ref_watch_path` with the climb bounded at `refs` (`c883b6e`), and 4 publish-stamp fixtures (`c61eb9c`). Build-stamp mutation is 66/66. canzan-lift v0.4.2 (the stale-stamp fix, `c30ec4a4`) is live on dev.

## Key Decisions

- `build.rs` stamps from `FOUNDRY_STAMP_*`, then git, then `unknown`. It watches the refs dir when the branch ref is packed, and never climbs above `refs`.
- Both publish workflows stamp images and refuse empty values. check-arch `publish-stamp:` enforces this. The Forgejo runner has git (OQ-D4 resolved).
- Prod (`foundry.jeffbailey.us`) is held for approval by design. A lagging prod footer means "awaiting approval".

## Next Steps

- In progress, uncommitted, not started by this session: `instance-admin-workspace-rename`. DISCUSS and DESIGN are written (2026-10-05, migration 0018, ADR-workspace-rename-001/002, plus brief, jobs and persona edits). Next is DISTILL; resume with `/nw:continue`.
- Approve canzan-lift v0.4.2 for prod.
- OQ-10 closed: prod sets `foundry-member` but never provisioned an account (1 user, created 2026-08-22). Still owed: one manual Keycloak sign-in.
- The user owes card-pointer-drag device evidence. New work starts with `/nw:new`.
