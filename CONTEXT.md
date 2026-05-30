# CONTEXT

## Current Task

**v0.3.0 released as multi-arch (amd64 + arm64).** Slice 8 (deferred observability metrics) + the `Store::probe()` schema-scoping fix, 100% mutation coverage on the slice-8 code. The release pipeline was rebuilt (it had never once succeeded) and arm64 was added via cross-compile. `main` and the `v0.3.0` tag both at `9873b83`; images (`:v0.3.0`/`:v0.3`/`:latest`, amd64+arm64, cosign-signed + SBOM) on `ghcr.io/Canzan/foundry`.

## Key Decisions

- **Release pipeline rebuilt** (`release.yml`): the old single job built both arches sequentially and arm64-under-QEMU exceeded the 30m limit — every run was cancelled and nothing had ever published. Now: parallel per-arch build-by-digest + merge/sign/SBOM job. Also fixed a lowercase-GHCR-ref bug.
- **arm64 via cross-compile** (`Dockerfile`): builder pinned to `$BUILDPLATFORM`, cross-compiles to `$TARGETARCH` (`cargo --target`) — arm64 builds in ~tens of seconds, no QEMU. Needs the cross gcc **and** `libc6-dev-<arch>-cross` (without it, `ring`'s C fails on `bits/libc-header-start.h`). Validated locally both directions; green in CI.
- **Slice-8 quality**: mutation gaps fixed (test-only) + `Store::probe()` scoped to `current_schema()`. The `@all` Background flake (`PoolTimedOut`/`SSLRequest`) is pre-existing, proven change-independent via a stash baseline.

## Next Steps

- **Optional**: drive the pre-existing `@all` Postgres-contention flake to zero (separate infra concern).
- Next version (v0.3.1 / v0.4.0) ships multi-arch automatically; no release-pipeline work needed.
