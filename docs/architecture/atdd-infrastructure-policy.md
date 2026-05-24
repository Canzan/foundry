# ATDD Infrastructure Policy

Per `nw-distill` § Project Infrastructure Policy. One file per project. Apply-if-exists; write-if-absent; rewrite with `--policy=fresh`. Git history is the audit trail.

Bootstrapped: 2026-05-23 (first DISTILL — Slice 1 of `foundry-backend-mvp`).

The Architecture of Reference (port class → treatment) is unchanged here. This file specializes the concrete MECHANISM for each port in this codebase.

## Driving

| Port | Mechanism | Note |
|---|---|---|
| HTTP API (axum routes) | `reqwest::Client` against a `foundry_app::test_support::spawn_app()` helper that binds the real axum router on `127.0.0.1:0` and returns the bound `SocketAddr` | One in-process server per acceptance suite; per-scenario isolation via DB schema rotation (see Driven internal). No `WebApplicationFactory`-equivalent in Rust; bespoke `spawn_app()` is the idiom. |
| CLI (`foundry admin ...` subcommands) | `assert_cmd::Command::cargo_bin("foundry")` invoked as a subprocess from each scenario | Only used by US-06's "CLI-fallback password reset" scenario in Slice 1; expanded in later slices. |
| Docker Compose harness (US-01 only) | `std::process::Command` spawning real `docker compose -f tests/acceptance/fixtures/docker-compose.test.yml up -d` against a published-or-locally-built image | One Compose stack per `@us-01` scenario (slow: 30-60s startup). Tagged `@docker-compose` so CI can shard. See `driver.md`. |

## Driven internal (real)

| Port | Mechanism | Note |
|---|---|---|
| `PgPool` (Postgres) — Slice 1 schema (workspaces, users, teams, projects, issues, bootstrap_tokens, invites, outbox, signin_attempts, tower_sessions) | **One shared `testcontainers-rs` Postgres 16 container per `cargo test` invocation**, plus **per-scenario schema** — each scenario gets `CREATE SCHEMA test_<uuid>` + `SET search_path` + run migrations. Schema dropped at scenario teardown. | Decision: shared container + per-scenario schema is the **speed-vs-isolation pivot** for Slice 1. Truncating tables is too entangled with FK cascades + outbox; transactional rollback fights tower-sessions's own transaction; fresh-DB-per-scenario costs 200ms/scenario × 25 scenarios = 5s overhead → ruled out. Per-schema rotation costs ~30ms/scenario. |
| `tower_sessions_sqlx_store::PostgresStore` | Same Postgres as above; tower-sessions's own migrator runs once per test schema | Sessions are first-class observable state in US-06; the store is real, never mocked. |
| `bootstrap_tokens`, `invites`, `signin_attempts` tables | Same Postgres | These are state surfaces the assertions read directly via a thin `tests/acceptance/support/db_introspect.rs` helper (read-only SELECTs); never used to bypass the driving port for writes. |

## Driven external / non-deterministic (fake)

| Port | Fake | Note |
|---|---|---|
| `lettre::SmtpTransport` (SMTP for email invites in US-05, password reset in US-06) | `FakeEmailSender` — in-memory `Vec<SentEmail>` exposed via `AppState::test_outbox()`; tests assert message count + headers + body fragments | Real SMTP (GreenMail/MailHog container) is on the slice-3 roadmap per `auth.md`'s contract-test note. Slice 1 uses the in-memory fake; the `FakeEmailSender` enforces the same input validation as `lettre::SmtpTransport` (assert `to`, `from`, `subject` non-empty) per the "test doubles must validate inputs" rule. |
| `tokio::time::sleep` (used by NFR-SEC-02 brute-force delay) | `FakeClock` exposed via `AppState::time` injection — production wires real `tokio::time::sleep`; tests wire a `MockClock` that records sleep durations and returns immediately | Decision: assert that the 6th attempt's recorded delay is ≥4500ms WITHOUT actually waiting 5s in CI. The latency assertion is on the recorded duration, not wall-clock. Justification: NFR-SEC-02 is a CORRECTNESS contract ("delay was scheduled"), not a wall-clock contract; wall-clock test would add 5s × N scenarios to the suite. |
| Bootstrap token generator (`rand::random` over 32 bytes) | Production randomness in tests; the assertion is on token *shape* (length, URL-safety, single-use), not on a specific value | The HMAC and SHA-256 paths are deterministic given the input; no fake needed. |
| Clock for `expires_at` (HMAC tokens, sessions, signin_attempts windowing) | Same `FakeClock` as above; production uses `std::time::SystemTime`; tests override via injection | Required so US-05's "expired token rejected" scenario can advance time without sleeping. |
