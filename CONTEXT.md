# CONTEXT

## Current Task

**v0.17.0** shipped 2026-10-20 (OpenAPI document at `/api/v1/openapi.json`, API page at `/docs/api`, re-revoke keeps first time, rustls-webpki/webpki-roots for RUSTSEC-2026-0382/0381). Tag on `d2b0246`; dev and prod both show v0.17.0 · d2b0246.

## Key Decisions

- `crates/foundry-api/openapi.json` is hand-written; check-arch fails on drift, and `cargo xtask openapi` sets `info.version` on every bump.
- `deny.toml` accepts RUSTSEC-2026-0381 (webpki-roots 0.26 shim over patched 1.0.10) and RUSTSEC-2026-0374 (basic-toml, build-time via askama).
- Tags are annotated, not signed (no GPG key); the local gate needs `CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_STRIP=false`.

## Next Steps

- Dogfood `/docs/api` "Try it out" with a machine token on dev (DoD item 5, owed).
- Migrate askama 0.12 -> 0.13+ to drop basic-toml and its deny ignore.
- `xtask ci` collides with openlore/canzan-lift tests (PortNotExposed, pg shm); rerun failed lanes alone.
