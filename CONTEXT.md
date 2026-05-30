# CONTEXT

## Current Task

**v0.3.0 released — multi-arch + dual-SBOM, fully verified.** Slice 8 (deferred observability metrics) + the `Store::probe()` schema-scoping fix, 100% mutation coverage on the slice-8 code. Images on `ghcr.io/Canzan/foundry` (`:0.3.0`/`:0.3`/`:latest`, amd64+arm64). cosign-verified: image signature ✓, image SBOM (SPDX, 11 OS pkgs) ✓, Cargo SBOM (CycloneDX, 513 crates) ✓. `main` + `v0.3.0` tag at `e31f865`.

## Key Decisions

- **Release pipeline rebuilt** (`release.yml`, had never succeeded): parallel per-arch build-by-digest + merge/sign/SBOM job. Fixed: lowercase GHCR ref; `metadata-action` drops the leading `v` (image tags are `0.3.0`, not `v0.3.0`).
- **arm64 via cross-compile** (`Dockerfile`): builder on `$BUILDPLATFORM`, `cargo --target $TARGETARCH`; needs cross gcc **+** `libc6-dev-<arch>-cross` (else `ring`'s C fails). ~native speed, no QEMU.
- **Two SBOM attestations**: image SPDX (syft scans the manifest → OS pkgs) and Cargo CycloneDX (syft `file:Cargo.lock` → 513 crates). Distinguished by cosign `--type` (`spdxjson` vs `cyclonedx`). sbom-action: use `file:` not `path:` for a single file.
- **Slice-8 quality**: mutation gaps fixed (test-only) + probe scoped to `current_schema()`.
- **`@all` flake fixed** (`7ff7591`, test-only): the shared testcontainer has no TLS but the URLs set no `sslmode`, so sqlx's default `prefer` SSL probe intermittently failed under the connect-storm (`SSLRequest: 0x00`), starving the harness pool → `PoolTimedOut` on Background seed inserts. Fix: `ssl_mode(Disable)` on all shared-container connects + `acquire_timeout` 5s→30s in `harness.rs`. Validated 5/5 consecutive `@all` sweeps green (123/123).

## Next Steps

- None outstanding. v0.3.0 shipped (multi-arch + dual-SBOM, verified); `@all` lane stable.
- Future versions ship multi-arch + both SBOMs automatically; no release-pipeline work needed.
