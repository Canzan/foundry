# CONTEXT

## Current Task

**Web-tier templating arc COMPLETE and the full local gate is green** (`main` at `0f9d307`). `cargo xtask ci` passes ALL stages end-to-end — fmt, clippy, `xtask check-arch`, build --release, workspace tests, `cargo deny`, and the `@all` acceptance lane (**187 scenarios / 1572 steps, 0 failures**). Foundry now has zero inline `format!()` HTML anywhere in `foundry-app/src`; every web surface renders from an Askama template.

## Key Decisions

- Four features delivered through the full nWave pipeline, all on trunk: `web-tier-extraction` (JSON `/api/v1` + JWT, `ba791ee`) → `htmx-web-tier` (Askama + vendored htmx2, `36c0fd3`) → `remaining-surfaces-templating` (full pages 9→0, `71c9c72`) → `keyboard-fragments-templating` (last 2 fragments, `7f63c8b`). Two CI-enforced guards prevent inline-HTML regression.
- Infra fixes that made the gate green: US-03 restore **deadlock** fixed (`FoundryWorld` field-drop-order race, `6407946`) + matching **postgresql@16** client installed (replacing libpq-18) so `pg_dump`/`pg_restore` match the pg16 testcontainer.
- Trunk-based (AGENTS.md + memory): commit to `main`, no PRs, no CI commit-gate; `cargo xtask ci` is the local gate.

## Next Steps

- None outstanding — arc delivered, gate green, one branch (`main`), tree clean.
- Lesson logged: run FULL-workspace `cargo fmt --all --check` + `cargo clippy --all-targets --release -- -D warnings` per step (per-crate checks missed nits in the acceptance crate).
