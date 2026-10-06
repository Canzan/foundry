# CONTEXT

## Current Task

`instance-workspace-name-rule` shipped in v0.10.0 (`dc2b7aa`). One workspace-name rule (trim, non-empty, no control/bidi characters, at most 24) applies on dashboard provisioning, bootstrap, the CLI and rename, with identical wording. Gates: 61/61 scenarios, mutation 50/50, CI 999/999. Dev footer `Foundry v0.10.0 · 2026-10-06`, `data-commit="dc2b7aa"`; prod is on v0.9.0, awaiting approval.

## Key Decisions

- `foundry_core::WorkspaceName` is the one rule. Its error's `Display` is the only copy; check-arch `workspace-name-one-source` keeps it there.
- Bootstrap checks the name before the claim. A refusal never uses up the link, and a dead link answers as before.
- Prod (`foundry.jeffbailey.us`) is held for approval by design.

## Next Steps

- Approve foundry v0.9.0 and v0.10.0 and canzan-lift v0.4.2 for prod, then check the prod footer `data-commit="dc2b7aa"`.
- Follow-up D: `CHECK … NOT VALID` on `workspaces.name`. Optionally, the same rule for project names, and one CLI usage message.
- Still owed by the user: one manual Keycloak sign-in, and the card-pointer-drag device evidence.
