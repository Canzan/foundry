# CONTEXT

## Current Task

**`workspace-member-invites` shipped + finalized** (`main`, not pushed). GENERALIZES the shipped first-admin `/invites/accept` to general workspace members (member role only, v1). Two surfaces: (1) ISSUANCE — an admin invites a member at `/workspace/invites` (`is_workspace_admin`-gated, CSRF, non-enumerable 404 for non-admins, reuses `insert_invite`, emits a signed link + best-effort email); (2) ACCEPTANCE — the invitee (no prior account) sets a password and the accept POST runs the NEW one-tx `Store::create_member_and_consume` (atomic: guarded-UPDATE consume + CREATE user + ADD member membership + argon2id; SQLSTATE-23505 email-collision → uniform non-enumerable refusal, NEVER a 500, invite UNCONSUMED), then auto-signs-in. ONE `/invites/accept` route serves BOTH kinds via data-derived dispatch (`is_first_admin_invite`); the shipped first-admin path is unchanged (regression-guarded). NO migration, NO new crate. 14 DES TDD steps / 30 scenarios (`3fa73a7`→`a1953d7`) + remediation `392a9e4` + mutation-hardening `c24e73a`; `@all` 334/334 scenarios / 2623/2623 steps; review APPROVED; store-scope mutation 100%; check-arch PASSED (no new LAYER-1e line). Evolution: `docs/evolution/2026-06-16-workspace-member-invites.md`.

## Key Decisions

- **D1–D8 all IMPLEMENTED** (wave-decisions.md + adr-001..004 marked IMPLEMENTED): D3 data-derived kind dispatch (no `kind` column); D4 one-tx consume+create-user+membership+password; D5 23505 UNIQUE-catch → uniform refusal (no 500, no migration); D7 NO new LAYER-1e line (issuance resolves workspace from session); D8 NO migration (reuse `used_at`/`used_by`, `email_lower UNIQUE`, `role CHECK`). Review findings D1+D4 fixed (`392a9e4`); the 3 store survivors killed (`c24e73a`).
- **Eleven features through the full nWave pipeline, all on trunk** (each in `docs/evolution/`): …multi-workspace-provisioning, web-provisioning-flow, invite-accept-flow, **workspace-member-invites**. All feature workspaces PRESERVED.
- **Trunk-based**: commit to `main`, no PRs; verify full-workspace `cargo fmt --all --check` + `cargo clippy --all-targets --release -D warnings`; DES hook requires the 5-phase contract.

## Next Steps

- Optional: `git push` (orchestrator confirms push separately — this finalize did NOT push).
- **Direct invite increments**: admin-role member invites; bulk invites; invite revocation/resend; multi-workspace-membership-via-invite (the email-already-a-user case is currently refused non-enumerably).
- **Carried**: close the bootstrap claim-flow enumeration oracle (`bootstrap.rs:124-139`); Prometheus `foundry_token_mutations_total` exporter; per-workspace backup (OD-5); key-rotation UX; nightly scoped mutation pass on the web adapter.
