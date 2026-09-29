# CONTEXT

## Current Task

Two changes are on `main` and pushed, with no PR:
- keycloak-sso **D3a** (`9abf827`): role-gated, opt-in provisioning, `FOUNDRY_OIDC_PROVISION_ROLE`.
- **fix-backup-verify-fail-open** (`6b8460e`): row counts run in the binary, and the verifier fails closed with exits 8, 9 and 10.

The final full gate on `6b8460e` passed: 855/855 scenarios. Records are in `docs/evolution/2026-09-27-keycloak-sso.md` and `docs/feature/fix-backup-verify-fail-open/rca.md`.

## Key Decisions

- D3a resolutions: OD-7, a name over 64 characters falls through the chain and is never truncated; OD-8, a member with no password may reset one; OD-9, the role match is exact, case-sensitive and realm-only. Migration 0016 is one-way.
- `backup-verify` prints `status: OK` only after counting at least one Foundry table. pg_restore is its only external tool, and CI no longer installs a host Postgres client.

## Next Steps

- keycloak-sso:
  - OD-10: decide what revoking the provision role does to an existing account.
  - OD-11: un-pend the 23 base `keycloak-sso.feature` scenarios. The shipped SSO flow has no acceptance coverage.
  - Add tests for the `FOUNDRY_OIDC_PROVISION_ROLE` env name (a `from_env` seam) and for the race loser (the `Existing` path) end to end.
  - Change-password copy for a password-less member: point them to forgot-password.
  - A no-usable-name identity is refused; confirm that is intended.
  - Re-ID keycloak OUT-1..6, which collide with the registry, once the outcomes CLI works.
- Ops: verify that RELEASING Pattern 2 (the glibc binary mounted into alpine) actually runs.
- Older, from card-drag: a new issue jumps to the top of its lane on reload; `.lane-drop-indicator` contrast; pin the Chrome image; testcontainers connect flakes; real-browser checks are still owed.
