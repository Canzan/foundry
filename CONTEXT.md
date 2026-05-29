# CONTEXT

## Current Task

Slice-8 mutation hardening **complete + pushed**. Closed 3 surviving-mutant gaps (76.9% → **100%, 13/13 viable**) and fixed the `Store::probe()` schema-scoping bug found along the way. Latest on `origin/main`: `fb001ba`. Repo in sync; `v0.2.0` tag public.

## Key Decisions

- **Mutation gaps fixed (test-only)**: pinned the `migration_id` label *value* + added a store-probe-failure scenario exercising `record_probe_result`. Report at `docs/feature/slice-8-deferred-metrics/deliver/mutation/`.
- **`Store::probe()` fixed** (`fb001ba`): migration-0006 column check now scoped with `table_schema = current_schema()` (was counting across all schemas; would mask a half-migrated active schema). No prod behaviour change in single-schema deploys.
- **The `@all` Background flake is pre-existing**, not from these changes — proven by a stash baseline that failed identically (`PoolTimedOut` / `SSLRequest` transient on Background inserts, rotating victim; documented ~1/5 contention). Also: cargo-mutants `@real-io` runs need `--test-package foundry-app` to rebuild the bin (else false survivals); cleaned 94 leaked testcontainers.

## Next Steps

- **Optional**: drive the pre-existing `@all` `PoolTimedOut`/`SSLRequest` Background flake to zero (separate infra concern).
- **v0.3.0** whenever ready — slice-8 already on `main` past the `v0.2.0` tag.
