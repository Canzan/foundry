# CONTEXT

## Current Task

All clear; nothing is queued. The release-version-footer build stamp (US-RVF-02) shipped in v0.8.0 (`a610dd9`). The dev footer reads `Foundry v0.8.0 · 2026-10-04` with `data-commit="a610dd9"`; prod is on v0.7.0, awaiting approval. Follow-ups are done: a pure, tested `ref_watch_path` with the climb bounded at `refs` (`c883b6e`), and 4 publish-stamp fixtures (`c61eb9c`). Build-stamp mutation is 66/66. canzan-lift v0.4.2 (the stale-stamp fix, `c30ec4a4`) is live on dev.

## Key Decisions

- `build.rs` stamps from `FOUNDRY_STAMP_*`, then git, then `unknown`. It watches the refs dir when the branch ref is packed, and never climbs above `refs`.
- Both publish workflows stamp images and refuse empty values. check-arch `publish-stamp:` enforces this. The Forgejo runner has git (OQ-D4 resolved).
- Prod (`foundry.jeffbailey.us`) is held for approval by design. A lagging prod footer means "awaiting approval".

## Next Steps

- Approve foundry v0.8.0 and canzan-lift v0.4.2 for prod, then check the prod footer `data-commit` (AC-8).
- OQ-10 (did prod ever set `FOUNDRY_OIDC_PROVISION_ROLE`?) and one manual Keycloak sign-in.
- The user owes card-pointer-drag device evidence. New work starts with `/nw:new`.
