# CONTEXT

## Current Task

**`multi-workspace-tenancy` ISOLATION CORE (slices 1-4) shipped + finalized as a milestone** (`main` at `384fa5f`, not pushed). Foundry is no longer single-workspace: real per-tenant isolation + cross-tenant **non-enumerability** across web htmx, JSON `/api/v1`, machine-token, and sessions. Built slice-by-slice as 18 DES-monitored TDD steps (4 phases). Slice 4's adversarial matrix found + closed **4 real enumeration oracles** (slug-echoing not-found pages). `cargo xtask ci` ALL GREEN (`@all` 273/2279). Zero new crates. Evolution doc: `docs/evolution/2026-06-11-multi-workspace-tenancy.md`.

## Key Decisions

- **Seven features through the full nWave pipeline, all on trunk** (each in `docs/evolution/`): web-tier-extraction, htmx-web-tier, remaining-surfaces-templating, keyboard-fragments-templating, machine-token-admin-ux, token-management-api `a2502f8`, **multi-workspace-tenancy (slices 1-4) `384fa5f`**.
- **Isolation mechanism**: drop `uniq_one_workspace` (`0009`) + `0010` active_workspace; request→workspace RESOLUTION seam (session `resolve_active_workspace` replacing buggy `first_workspace()`; token.workspace_id for API; fail-closed); `ActingWorkspace` newtype + a build-time check-arch **LAYER-1e tenant-scoping guard** (un-scoped query → build failure); uniform non-enumerable 404 everywhere; membership-guarded `/workspace/switch`. Ratified: OD-1 shared-schema, OD-2 multi-membership, OD-3 instance super-admin (role itself deferred). Synthetic-uuid cross-workspace residual CLOSED with real fixtures.
- **Trunk-based** (AGENTS.md + memory): commit to `main`, no PRs, no CI commit-gate; verify full-workspace `cargo fmt --all --check` + `cargo clippy --all-targets --release -D warnings`; DES hook requires the 5-phase legacy contract; roadmap migrations now run to `0010`.

## Next Steps

- Optional: `git push` (origin behind; the whole arc since `7391eec`... incl. multi-workspace is local).
- **Follow-up feature `multi-workspace-provisioning`** (slices 5-6, seed = the 6 ADRs + `slices/slice-05/06-*.md`): slice 5 existing-install migration GUARANTEE (upgrade-safety proof; `0009` guard-drop already shipped); slice 6 instance super-admin PROVISIONING surface + close the rate-bucket-eviction residual.
- Other deferred: Prometheus exporter for `foundry_token_mutations_total` (DEVOPS); per-workspace backup (OD-5); key-rotation UX.
