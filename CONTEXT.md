# CONTEXT

## Current Task

All clear. No nWave work is queued or in progress: every roadmap step is committed and all 58 features have an evolution record. This session shipped card-pointer-drag (pushed), v0.6.0 / v0.6.1 / v0.6.2, keycloak-sso OD-11 and OD-10, and v0.7.0 (live on dev and prod), then finished OQ-8 and the docs housekeeping.

## Key Decisions

- keycloak-sso D3b: an account created by role provisioning must hold the role at every Keycloak sign-in. The password door stays open (OD-12). Unsetting the role reopens the Keycloak door (OD-13).
- Foundry runs at dev https://foundry.unintelligent-design.us/ and prod https://foundry.jeffbailey.us/. Prod skipped the v0.6.1 and v0.6.2 patches and took v0.7.0 after about 45 minutes.
- `.nwave/des-config.json` (untracked) uses `tdd_phases` [RED, GREEN, COMMIT]. The DES Agent hook still wants PREPARE, RED_ACCEPTANCE and RED_UNIT listed as numbered items in prompts.

## Next Steps

- OQ-10: confirm whether prod ever set `FOUNDRY_OIDC_PROVISION_ROLE`; if it did, check the accounts migration 0017 marked.
- Do one manual Keycloak sign-in on v0.7.0, and check why prod skips patch releases.
- The user owes card-pointer-drag device evidence: slice-02 tallies and the slice-03 checklist. New work starts with `/nw:new`.
