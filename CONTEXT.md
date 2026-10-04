# CONTEXT

## Current Task

The release-version-footer build stamp (US-RVF-02) shipped in v0.8.0 (`a610dd9`, no PR). The dev footer reads `Foundry v0.8.0 · 2026-10-04` with `data-commit="a610dd9"`; prod is still on v0.7.0, awaiting approval. CI 911/911, mutation 55/60 (91.7%), review APPROVED. The canzan-lift stale-stamp fix shipped as canzan-lift v0.4.2 (`c30ec4a4`): live on dev, prod awaiting approval.

## Key Decisions

- The footer matches canzan-lift. `build.rs` stamps from `FOUNDRY_STAMP_*` build-args, then git, then `unknown`. When the loose branch ref is absent it watches the ref's parent dir, so the stamp re-stamps after `pack-refs`.
- Both publish workflows pass a separate 7-char stamp and refuse to publish on an empty value. check-arch `publish-stamp:` enforces this. OQ-D4 is resolved: the Forgejo runner has git, since dev shows a real SHA.
- Prod (`foundry.jeffbailey.us`) is held for approval by design. A lagging prod footer means "awaiting approval".

## Next Steps

- Approve foundry v0.8.0 and canzan-lift v0.4.2 for prod, then check the prod footer (AC-8). Then OQ-10, and one manual Keycloak sign-in.
- Mutation follow-ups: build.rs path choice done (`c883b6e`, pure `ref_watch_path`, climb now bounded at `refs`). 4 xtask publish-stamp fixtures remain.
- The user owes card-pointer-drag device evidence.
