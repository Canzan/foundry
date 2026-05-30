# CONTEXT

## Current Task

**v0.3.0 cut + published.** Slice 8 (deferred observability metrics) + the `Store::probe()` schema-scoping fix, with 100% mutation coverage (13/13 viable) on the slice-8 code. Release commit `05e8b10`, annotated tag `v0.3.0` pushed → `release.yml` building/signing multi-arch images to `ghcr.io/Canzan/foundry` (`:v0.3.0`/`:v0.3`/`:latest`). Repo in sync.

## Key Decisions

- **Mutation gaps fixed (test-only)**: pinned the `migration_id` label *value* + added a store-probe-failure scenario exercising `record_probe_result`. Report at `docs/feature/slice-8-deferred-metrics/deliver/mutation/`.
- **`Store::probe()` fixed** (`fb001ba`): migration-0006 column check scoped with `table_schema = current_schema()` (was counting across all schemas; would mask a half-migrated active schema). No prod behaviour change in single-schema deploys.
- **The `@all` Background flake is pre-existing**, not from these changes — proven by a stash baseline that failed identically (`PoolTimedOut` / `SSLRequest` transient on Background inserts, rotating victim; documented ~1/5 contention). Also: cargo-mutants `@real-io` runs need `--test-package foundry-app` to rebuild the bin (else false survivals); cleaned 94 leaked testcontainers.

## Next Steps

- **Confirm the v0.3.0 release run went green** (multi-arch build + cosign + SBOM); verify the `ghcr.io` tags published.
- **Optional**: drive the pre-existing `@all` `PoolTimedOut`/`SSLRequest` Background flake to zero (separate infra concern).
