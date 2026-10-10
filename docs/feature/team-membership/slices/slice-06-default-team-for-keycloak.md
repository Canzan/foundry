# Slice 06: A newcomer provisioned by a Keycloak sign-in lands on the default team

Story: US-TM-07 | Estimate: 0.5 day | job_id: `job-team-membership`

## Goal

When Keycloak provisioning creates a new account, it also puts the account on
the original workspace's default team as Member. The door the incident came
through then leads straight to a board.

## IN

- The default-team placement inside `provision_federated_member`'s transaction, using slice 05's setting (OQ-5).
- Linking an existing account writes no team row.
- An unset or missing default never refuses or fails the sign-in (D17).

## OUT

- Linking a Keycloak identity to a different existing account (the incident's second follow-up). Provisioning into a workspace other than the original one.

## Learning Hypothesis

- **Disproves if it fails**: that provisioning and team placement belong in one transaction. If the keycloak-sso lane (46) or the OD-10 provision-role checks regress, team placement must move after session establishment as a best-effort step.
- **Confirms if it succeeds**: that the incident cannot recur for any newcomer while a default is set. KPI-1 stays at 0 without manual adds.

## Acceptance Criteria

- [ ] With General as default, Tomás's first Keycloak sign-in → he is a Member of General, and `/team/general/project/sandbox` opens.
- [ ] Priya's existing account links through Keycloak with the same email → she is not added to the default team.
- [ ] Unset or missing default → provisioned, signed in, on no team.
- [ ] The keycloak-sso lane stays at 46/46.
- [ ] Dogfood on dev: with General as default, a fresh Keycloak user holding the provision role signs in and sees Sandbox on the first page.

## Dependencies

- Slice 05 (the setting). OQ-5 and OQ-6 resolved to the recommended answers (2026-10-09).
