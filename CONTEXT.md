# CONTEXT

## Current Task

**Session complete — the full web-tier + machine-token arc is shipped, finalized, and gate-green** (`main` at `1e0f4dd`, in sync with origin, tree clean). `cargo xtask ci` passes all stages end-to-end (fmt, clippy, check-arch, build --release, workspace tests, deny, `@all` acceptance **215 scenarios / 1752 steps**). Foundry now has a clean two-tier architecture (JSON `/api/v1` + fully-templated htmx web tier over a presentation-neutral core) with the complete machine-token lifecycle.

## Key Decisions

- **Five features delivered through the full nWave pipeline this arc, all on trunk** (each finalized in `docs/evolution/`): `web-tier-extraction` (JSON API + JWT, `ba791ee`), `htmx-web-tier` (Askama + vendored htmx2, `36c0fd3`), `remaining-surfaces-templating` (inline full-pages 9→0, `71c9c72`), `keyboard-fragments-templating` (0 inline HTML in foundry-app, `7f63c8b`), `machine-token-admin-ux` (admin mint/list/revoke, signer in-AppState, `1e0f4dd`).
- **Infra fixes** that made the gate green: US-03 restore deadlock (`6407946`, FoundryWorld drop-order) + matching postgresql@16 client on PATH; two CI-enforced inline-HTML guards.
- **Trunk-based** (AGENTS.md + memory): commit to `main`, no PRs, no CI commit-gate; verify with full-workspace `cargo fmt --all --check` + `cargo clippy --all-targets --release -D warnings` (per-crate misses the acceptance crate).

## Next Steps

- None outstanding — arc delivered, `cargo xtask ci` all-gates-green, one branch (`main`), nothing in progress.
- Deferred (documented in the features' `out-of-scope.md` / `upstream-issues.md`): JSON token-management API, real cross-workspace fixtures (await multi-workspace), key-rotation UX.
