# CONTEXT

## Current Task

**v0.3.0 released** — Slice 8 (deferred observability metrics) + the `Store::probe()` schema-scoping fix, 100% mutation coverage on the slice-8 code. Container images (`:v0.3.0`/`:v0.3`/`:latest`, **amd64**, cosign-signed + SBOM) are live on `ghcr.io/Canzan/foundry`. `main` and the `v0.3.0` tag both at `87854f2`; repo in sync.

## Key Decisions

- **Release pipeline was broken, now fixed** (`release.yml`): the single build job compiled both arches sequentially and arm64-under-QEMU blew past the 30m job limit — every release run was cancelled and *nothing* had ever published. Rewrote to parallel per-arch build-by-digest + a merge/sign/SBOM job. Also fixed a lowercase-GHCR-ref bug (`Canzan` → must be lowercase).
- **arm64 deferred**: the full Rust workspace under QEMU exceeds even 45m, and this PRIVATE repo has no free native arm64 runner. Shipped amd64-only; `v0.3.0` tag force-moved onto the working workflow (its first release published nothing, so the rewrite was safe).
- **Slice-8 quality**: mutation gaps fixed (test-only) + `Store::probe()` scoped to `current_schema()`. The `@all` Background flake (`PoolTimedOut`/`SSLRequest`) is pre-existing, proven change-independent via a stash baseline.

## Next Steps

- **Add arm64 back** via a cross-compile Dockerfile (build on the amd64 host targeting `aarch64` with `--platform=$BUILDPLATFORM` + `cargo --target`), then re-add `linux/arm64` to the release matrix — avoids QEMU, builds at native speed.
- **Optional**: drive the pre-existing `@all` Postgres-contention flake to zero (separate infra concern).
