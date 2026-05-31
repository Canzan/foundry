# CONTEXT

## Current Task

**GitHub `ci.yml` green end-to-end (first time)** — `main` at `f4cebe3`, in sync. Pinning the test containers to `postgres:16-alpine` (match prod) surfaced + fixed 4 pre-existing CI failures (CI had been red on every push; only `release.yml` was green). Also did a README/AGENTS review. Latest release remains **v0.3.1** (multi-arch, signed, dual SBOMs — all cosign-verified).

## Key Decisions

- **PG16 pin** (4 test containers + admin_cli help): tests now match the production major version (docker-compose/k8s ship `16-alpine`). 117/117 product scenarios validated on PG16.
- **CI binary build**: `acceptance`/`quickstart` jobs (and the README quickstart, cold) failed with `CARGO_BIN_EXE_foundry unset` — `cargo test -p foundry-acceptance` doesn't build the bin the `@real-io` scenarios spawn. Fix: `cargo build --release --bin foundry` before the test (local `cargo xtask ci` masked it via a shared target dir).
- **Backup lane** (US-03, needs `pg_dump`) was wrongly in the default lane → tagged `@needs-pgclient`, excluded from default; `@all` job installs `postgresql-client-16`. Quickstart stays lightweight (Rust + Docker only).
- **`@docker-compose` job** needed `.env` (`cp .env.example .env`) for `docker compose up`.
- **`@all` flake** (`7ff7591`): `ssl_mode(Disable)` + `acquire_timeout` 30s in `harness.rs` (no-TLS testcontainer; SSL-probe failure starved the pool). A/B-proven (reverted 3/5 vs fixed 0/5).
- Renamed `CLAUDE.md` → `AGENTS.md`; fixed README org/URLs (`foundry-project` → `Canzan`) + stale counts.

## Next Steps

- None outstanding — GitHub CI green (5/5 jobs), `cargo xtask ci` green, v0.3.1 shipped + verified.
