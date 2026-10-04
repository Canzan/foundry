# CONTEXT

## Current Task

keycloak-sso OD-10 (US-07) shipped in v0.7.0 (`40a91e3`, no PR). Withdrawing the Keycloak provision role now closes Keycloak sign-in to the accounts that role provisioning created. Migration 0017 adds `users.provisioned_at` and backfills password-less accounts. CI 911/911, cargo-mutants 12/12, review APPROVED. v0.7.0 is live on dev and prod. Prod skipped v0.6.1 and v0.6.2 and jumped from v0.6.0 straight to v0.7.0.

## Key Decisions

- D3b with OD-12 / OD-13 / OD-14: the password door stays open, so full revocation also means removing the workspace membership. Unsetting the role reopens the Keycloak door. The backfill marks password-less accounts.
- v0.6.2 hardening: `foundry-oidc` refuses an empty nonce, state or verifier. A user still owes one manual Keycloak sign-in to confirm it.
- Foundry runs at dev https://foundry.unintelligent-design.us/ and prod https://foundry.jeffbailey.us/. Check both after a release.

## Next Steps

- OQ-10: confirm whether prod ever set `FOUNDRY_OIDC_PROVISION_ROLE`; if it did, check the accounts 0017 marked. Do the manual Keycloak sign-in, and check why prod skips patch releases.
- The user owes card-pointer-drag device evidence: slice-02 tallies and the slice-03 checklist.
- Done 2026-10-04: OQ-8 (`check-arch` `provisioned-marker` rule, `87d1ee8`); evolution docs for the three shipped fixes; stale feature-file comments fixed (`d38864b`). No feature has open DELIVER work.
