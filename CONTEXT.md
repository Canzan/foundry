# CONTEXT

## Current Task

Slice-8 deferred-metrics mutation testing: closed 3 surviving-mutant gaps (kill rate 76.9% → **100%, 13/13**); committed (`6889099`) and **pushed `main` + the `v0.2.0` tag to origin**. Repo in sync; v0.2.0 release is now public.

## Key Decisions

- **Mutation fix is test-only** (production untouched): pinned the `migration_id` label *value* + added a store-probe-failure scenario that exercises `record_probe_result`. Report at `docs/feature/slice-8-deferred-metrics/deliver/mutation/`.
- **The `@all` "malformed UUID" failure is pre-existing**, not from these changes — proven by a stash baseline that failed identically (`comment-tombstone-gc` Background `PoolTimedOut`, the documented ~1/5 contention flake). Also cleaned 94 leaked testcontainers from the session.
- **cargo-mutants gotcha**: `@real-io` scenarios spawn the `foundry` binary, so runs need `--test-package foundry-app` to rebuild the bin — else mutants falsely survive (15% → true 100%).

## Next Steps

- **Optional prod fix**: `Store::probe()` (`foundry-store/src/lib.rs:148`) omits a `table_schema` filter — counts the 0006 columns across all schemas.
- **Optional**: drive the pre-existing `@all` `PoolTimedOut` flake to zero (separate from this work).
- **v0.3.0** whenever ready — slice-8 already on `main` past the `v0.2.0` tag.
