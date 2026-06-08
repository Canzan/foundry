# CONTEXT

## Current Task

**`token-management-api` shipped + finalized on trunk** (`main` at `a2502f8`, not yet pushed). The deferred JSON fast-follow to machine-token-admin-ux: `GET`/`DELETE /api/v1/.../tokens` (bearer-authed list + revoke over the shipped use-cases), **no mint via the API** (now a build-time `api≠mint` check-arch invariant), per-principal token-bucket rate guardrail (429). Reviewed (Sonnet, findings fixed test-first) + mutation-hardened (`rate_limit.rs` 100% viable kill). Gates green: `@all` 235/1879, fmt/clippy `-D warnings`, check-arch, xtask tests.

## Key Decisions

- **Six features delivered through the full nWave pipeline, all on trunk** (each in `docs/evolution/`): web-tier-extraction `ba791ee`, htmx-web-tier `36c0fd3`, remaining-surfaces-templating `71c9c72`, keyboard-fragments-templating `7f63c8b`, machine-token-admin-ux `1e0f4dd`, **token-management-api `a2502f8`**.
- **token-management-api authz = (c) asymmetric**: bearer may list+revoke (incl. revoke-self) via `is_workspace_admin`; mint stays human-session-only (`/admin/tokens`). Guardrail is adapter-local (not a domain ServiceError), foundry-api gained no new crate dep.
- **Trunk-based** (AGENTS.md + memory): commit to `main`, no PRs, no CI commit-gate; verify full-workspace `cargo fmt --all --check` + `cargo clippy --all-targets --release -D warnings`; this project's DES hook requires the 5-phase legacy contract.

## Next Steps

- Optional: `cargo xtask ci` full-gate confirmation; `git push` (a2502f8 is local, not pushed).
- Deferred (accepted residuals): rate-bucket map eviction + real cross-workspace fixtures (await multi-workspace); Prometheus exporter wiring for `foundry_token_mutations_total` (DEVOPS); key-rotation UX.
