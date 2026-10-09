# CONTEXT

## Current Task

`name-db-checks` is delivered and pushed (unreleased): migrations 0019/0020 add triggers that enforce the workspace (24) and project (256) name rules on every new name write. SQLSTATE 23514 names the arm, legacy rows keep working, and the legacy count is 0 on dev and prod. Gates: ndc 57/57, CI 1130/1130, Rust mutation 100%, SQL hand-mutation table complete. Before that: `project-name-rule` shipped in v0.11.0 (`ec17c64`). One project-name rule (trim, non-empty, no control/bidi characters, at most 256, unique within the team) applies on create and rename. A name with no Latin letter gets its lower-cased key prefix as its URL (`jp`, `jp-2`…). Gates: 74/74 scenarios, mutation 62/63, CI 1073/1073. Dev footer `Foundry v0.11.0 · 2026-10-07`, `data-commit="ec17c64"`. Prod runs v0.9.0 (`da9fb91`); v0.10.0 and v0.11.0 await approval.

## Key Decisions

- `foundry_core::{WorkspaceName, ProjectName}` share one refused-character check (`name_chars`). Each error's `Display` is the only copy; check-arch one-source rules enforce it.
- `Services::create_project` is the one place a slug is minted. A lost fallback race retries up to 3 times.
- Prod (`foundry.jeffbailey.us`) is held for approval by design.

## Next Steps

- After dev applies 0019/0020: confirm the boot succeeded and the legacy count stays 0. A release containing them would be v0.12.0 (cut only on request).
- Approve foundry v0.10.0 and v0.11.0, and canzan-lift v0.4.2, for prod; then check the prod footer `data-commit="ec17c64"`.
- Follow-ups: name the projects unique constraints; repair any legacy `slug = ''` projects (user decision).
- Still owed by the user: one manual Keycloak sign-in, and the card-pointer-drag device evidence.
