# CONTEXT

## Current Task

`project-name-rule` shipped in v0.11.0 (`ec17c64`). One project-name rule (trim, non-empty, no control/bidi characters, at most 256, unique within the team) applies on create and rename. A name with no Latin letter gets its lower-cased key prefix as its URL (`jp`, `jp-2`…). Gates: 74/74 scenarios, mutation 62/63, CI 1073/1073. Dev footer `Foundry v0.11.0 · 2026-10-07`, `data-commit="ec17c64"`. Prod runs v0.9.0 (`da9fb91`); v0.10.0 and v0.11.0 await approval.

## Key Decisions

- `foundry_core::{WorkspaceName, ProjectName}` share one refused-character check (`name_chars`). Each error's `Display` is the only copy; check-arch one-source rules enforce it.
- `Services::create_project` is the one place a slug is minted. A lost fallback race retries up to 3 times.
- Prod (`foundry.jeffbailey.us`) is held for approval by design.

## Next Steps

- **In progress, paused by the user (machine busy): `name-db-checks` DELIVER.** Done: step 01-01 (check-arch `name-rule-legacy-seam`, `f2176bd`). Next is 01-02 (migration 0019 + legacy seam), then 01-03…03-02 per `docs/feature/name-db-checks/deliver/roadmap.json` (step contexts are regenerated from it). Local commits are NOT pushed; do not push until finalize, because 0019/0020 freeze once dev applies them. Resume with `/nw:continue`.
- Approve foundry v0.10.0 and v0.11.0, and canzan-lift v0.4.2, for prod; then check the prod footer `data-commit="ec17c64"`.
- Follow-ups: DB CHECKs on workspace and project names; name the projects unique constraints; repair any legacy `slug = ''` projects (user decision).
- Still owed by the user: one manual Keycloak sign-in, and the card-pointer-drag device evidence.
