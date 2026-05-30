# CONTEXT

## Current Task

**v0.3.0 released — multi-arch + dual-SBOM, fully verified.** Slice 8 (deferred observability metrics) + the `Store::probe()` schema-scoping fix, 100% mutation coverage on the slice-8 code. Images on `ghcr.io/Canzan/foundry` (`:0.3.0`/`:0.3`/`:latest`, amd64+arm64). cosign-verified: image signature ✓, image SBOM (SPDX, 11 OS pkgs) ✓, Cargo SBOM (CycloneDX, 513 crates) ✓. `main` + `v0.3.0` tag at `e31f865`.

## Key Decisions

- **Release pipeline rebuilt** (`release.yml`, had never succeeded): parallel per-arch build-by-digest + merge/sign/SBOM job. Fixed: lowercase GHCR ref; `metadata-action` drops the leading `v` (image tags are `0.3.0`, not `v0.3.0`).
- **arm64 via cross-compile** (`Dockerfile`): builder on `$BUILDPLATFORM`, `cargo --target $TARGETARCH`; needs cross gcc **+** `libc6-dev-<arch>-cross` (else `ring`'s C fails). ~native speed, no QEMU.
- **Two SBOM attestations**: image SPDX (syft scans the manifest → OS pkgs) and Cargo CycloneDX (syft `file:Cargo.lock` → 513 crates). Distinguished by cosign `--type` (`spdxjson` vs `cyclonedx`). sbom-action: use `file:` not `path:` for a single file.
- **Slice-8 quality**: mutation gaps fixed (test-only) + probe scoped to `current_schema()`. `@all` Background flake (`PoolTimedOut`/`SSLRequest`) is pre-existing, proven change-independent via a stash baseline.

## Next Steps

- **Optional**: drive the pre-existing `@all` Postgres-contention flake to zero (separate infra concern).
- Future versions ship multi-arch + both SBOMs automatically; no release-pipeline work needed.
