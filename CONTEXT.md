# CONTEXT

## Current Task

v0.12.0 (`bff7a80`) is live on dev: `Foundry v0.12.0 · 2026-10-09`. It ships two changes:
- **name-db-checks:** migrations 0019/0020 add triggers enforcing the workspace (24) and project (256) name rules on new name writes; SQLSTATE 23514 names the arm. Verified on dev: migration 20, 4 triggers enabled, 0 legacy violations.
- **fix-hide-unreachable-boards:** the dashboard and the rail Board link list only projects in the user's teams, and a member on no team gets an empty state.

Prod runs v0.11.0 (`ec17c64`); v0.12.0 awaits approval.

## Key Decisions

- The name rules are enforced by triggers on name writes (BEFORE INSERT / UPDATE OF name WHEN changed), not CHECK, so legacy rows keep taking issues. Tests seed legacy rows via a test-support seam that check-arch fences off.
- Dev deploys release tags, not `main`. Prod is held for approval.
- The 404 on dev was a second, Keycloak-provisioned account (jeff@unintelligent-design.us) on no team. The bootstrap account jeffabailey@gmail.com is unchanged (team lead, workspace admin, instance admin).

## Next Steps

- Approve foundry v0.12.0 and canzan-lift v0.4.2 for prod. After prod applies 0019/0020, run the CHANGELOG's legacy-count query.
- Follow-ups:
  - a way to join teams (admin "add to team", or a default-team policy);
  - show the signed-in account, and link a Keycloak identity to an existing account;
  - the fix-hub test gaps (no-team create link, empty-team member, rail Board link off `/`);
  - name the projects unique constraints.
- Still owed by the user: one manual Keycloak sign-in, and the card-pointer-drag device evidence.
