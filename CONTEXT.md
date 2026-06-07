# CONTEXT

## Current Task

**`machine-token-admin-ux` shipped to trunk** (`main` at `433fe69`). Full nWave pipeline (DISCUSS→DESIGN→DISTILL→DELIVER, wave-by-wave). Workspace admins can now **mint** (server-side Ed25519, value shown once), **list**, and **revoke** machine tokens at `/admin/tokens` — making the app a token issuer (was verifier-only). 215/215 acceptance green; `foundry_services::tokens` mutation 100%; zero new crates.

## Key Decisions

- **Signer in `AppState`** as `Option<Arc<MachineTokenSigner>>`, retained only after the boot self-test; SecretString + Debug-omitted + never logged; **graceful-absent** (no key → mint disabled/UI hidden/403, verify-only still boots). One-time secret never persisted/logged. Revoke reuses the **shipped jti denylist** (refused next `/api/v1`). `0008` adds nullable `created_by` (ON DELETE SET NULL). Web-UI-first; JSON token API deferred.
- **Security review (Sonnet)**: 0 blockers, 3 high fixed test-first — headline: `created_by` was written but never read back, so the audit list resolved "minted by" from the token *subject* not the *issuer*; fixed + proven with a subject≠issuer seed. Mutation pinned authz/TTL/scope/isolation/status (100% on tokens.rs).
- **Trunk-based** (AGENTS.md + memory): all commits direct to `main`, no PRs; verify with full-workspace `cargo fmt --all --check` + `cargo clippy --all-targets --release -D warnings` (per-crate misses the acceptance crate).

## Next Steps

- None outstanding for this feature. Deferred (in `discuss/out-of-scope.md` + `distill/upstream-issues.md`): JSON token-management API; real cross-workspace fixtures (await multi-workspace support); key-rotation UX.
- Five features now shipped this arc: web-tier-extraction, htmx-web-tier, remaining-surfaces-templating, keyboard-fragments-templating, machine-token-admin-ux.
