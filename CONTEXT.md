# CONTEXT

## Current Task

**`invite-accept-flow` shipped + finalized** (`main`, not pushed). The public `/invites/accept` credential-establishment vertical: a provisioned workspace's first-admin verifies a signed `InviteToken`, sets a password (min-12, NIST length-first), is consumed single-use, and is auto-signed-in onto their workspace. GET (non-committal, mint CSRF, render set-password form) + POST under session+CSRF (re-verify → `check_password_policy` → ONE atomic guarded-UPDATE consume + argon2id write → session → 303). Reuses shipped `invites.used_at`/`used_by` (NO migration, NO new crate); non-enumerable byte-identical 200-OK refusal across expired/used/tampered/unknown. **CLOSES the dead `/invites/accept` URL** flagged by `multi-workspace-provisioning` + `web-provisioning-flow` — the provisioning arc (tenancy → provisioning → web-provisioning → invite-accept) is COMPLETE. 18 DES TDD steps (`770626d`→`bfb7e79`) + remediation `b8b9b06`; `@all` 303/303 scenarios, review APPROVED, mutation 100% (4/4) on the policy. Evolution: `docs/evolution/2026-06-14-invite-accept-flow.md`.

## Key Decisions

- **D1–D7 all IMPLEMENTED** (wave-decisions.md + adr-001..005 marked SHIPPED): D1 no-migration guarded-UPDATE consume; D2 one-TX password+consume; D3 uniform non-enumerable 200-OK refusal; D4 public-POST CSRF cookie-on-GET; D5 `foundry_auth::check_password_policy` min-12; D6 GET advisory vs POST TOCTOU-safe consume; D7 NO new LAYER-1e line (uses `resolve_active_workspace` like signin).
- **Ten features through the full nWave pipeline, all on trunk** (each in `docs/evolution/`): …multi-workspace-provisioning, web-provisioning-flow, **invite-accept-flow**. All feature workspaces PRESERVED.
- **Trunk-based**: commit to `main`, no PRs; verify full-workspace `cargo fmt --all --check` + `cargo clippy --all-targets --release -D warnings`; DES hook requires the 5-phase contract.

## Next Steps

- Optional: `git push` (orchestrator confirms push separately — this finalize did NOT push).
- **Security follow-up**: close the bootstrap claim flow enumeration oracle (`bootstrap.rs:124-139` leaks distinct expired/used/not-found — bootstrap NOT modified here); add general workspace-member invite-accept (v1 = first-admins only).
- **Deferred**: Prometheus `foundry_token_mutations_total` exporter; per-workspace backup (OD-5); key-rotation UX; nightly scoped mutation pass on the web adapter.
</content>
