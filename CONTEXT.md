# CONTEXT

## Current Task

The empty-nonce hardening shipped in v0.6.2 (`51b62cd`, release `b6e17ed`, no PR): `foundry-oidc` now refuses an empty or absent nonce, state or verifier. Full `cargo xtask ci` passed 903/903. v0.6.2 is live on dev; prod is stuck on v0.6.0 and has not taken v0.6.1 or v0.6.2. Next is keycloak-sso OD-10 DISCUSS (lightweight).

## Key Decisions

- OD-10 (user, 2026-10-04): an account created by role provisioning must still hold the realm role at every Keycloak sign-in, or it gets the generic refusal. Other accounts are unaffected.
- In scenario 19, each door lands on its own page: `/` for password and SSO, `/dashboard` for the bootstrap claim. The empty-nonce fix is scope (a)+(b)+(c) only.
- Foundry runs at dev https://foundry.unintelligent-design.us/ and prod https://foundry.jeffbailey.us/. Check both footers after every release.

## Next Steps

- Find out why prod has not rolled past v0.6.0 (the image updater or sync on the prod cluster; there is no kube context for it here). Then do one manual Keycloak sign-in on v0.6.2.
- Take keycloak-sso OD-10 through DISCUSS, DESIGN, DISTILL and DELIVER. It needs a record of which accounts were provisioned, and it changes provisioning scenario 11.
- The user owes card-pointer-drag device evidence. Small cleanups remain: stale feature-file comments, and evolution docs for three shipped fixes.
