# CONTEXT

## Current Task

`instance-admin-workspace-rename` shipped in v0.9.0 (`da9fb91`). Instance admins rename a workspace from `/admin/instance/workspaces`; each rename is audited in `workspace_rename_events` (migration 0018), and the sidebar brand is one line with an ellipsis. Gates: 27/27 scenarios, mutation 14/14, CI 937/938 (sqlx flake, rerun green). Dev footer `Foundry v0.9.0 · 2026-10-05`, `data-commit="da9fb91"`; prod is still on v0.8.0, awaiting approval.

## Key Decisions

- The rename and its audit row are one transaction under a row lock; a same-name submission writes nothing. Rename records stay out of per-workspace exports (TENANT_TABLES stays at ten).
- The 24-character cap applies to the rename path only; there is no CHECK on `workspaces.name` (D10).
- Prod (`foundry.jeffbailey.us`) is held for approval by design. A lagging prod footer means "awaiting approval".

## Next Steps

- Approve foundry v0.9.0 and canzan-lift v0.4.2 for prod, then check the prod footer `data-commit="da9fb91"`.
- Follow-up B, `instance-workspace-name-rule`: the 24-character rule for provisioning, bootstrap and the CLI, and possibly a rule against control characters. Then follow-up D, `CHECK … NOT VALID`.
- Still owed by the user: one manual Keycloak sign-in, and the card-pointer-drag device evidence.
