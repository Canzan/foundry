# Foundry Backend MVP — Non-Functional Requirements

Cross-cutting requirements. Each NFR is testable, measurable, and traceable to a JTBD outcome.

> **Source of truth**: `stories.md` for functional behavior; this file for NFRs; `out-of-scope.md` for explicitly deferred items.

---

## NFR-PERF — Performance

### NFR-PERF-01: Page render latency (server-rendered HTML)

- **Requirement**: P95 server-render latency for typical authenticated HTML pages (issue list, issue detail, project board with ≤200 issues) is ≤200 ms measured at the application boundary, on the reference hardware below.
- **Reference hardware**: 2 vCPU, 4 GB RAM, NVMe SSD; Postgres 16 on the same host; ≤1 ms round-trip to DB.
- **Status**: **Aspirational** for the recommendation's "P95 < 50ms server-render" — DIVERGE called for the more conservative 200 ms ceiling to be measurable and not a fragile invariant; the 50 ms number is documented as an internal stretch goal but not a release blocker.
- **Test**: `criterion` benchmark for the render path, plus a synthetic HTTP load test (`oha` or `vegeta`) running 50 RPS for 60 seconds against an instance with 1,000 issues seeded.
- **Linked stories**: US-08 (issue create), US-09 (board view), US-12 (search).
- **JTBD link**: Outcome #4 — Linear-feel speed.

### NFR-PERF-02: File upload cap (bytea ceiling)

- **Requirement**: Per-file upload size is capped by `FILE_UPLOAD_MAX_MB` env var. Default = 10 MB. Recommended max = 50 MB. Hard upper bound = 100 MB (above this, bytea handling in Postgres becomes unreliable and the operator should plan an S3 backend instead — deferred).
- **Test**: Attempting to upload `FILE_UPLOAD_MAX_MB + 1` MB returns HTTP 413; attempting `FILE_UPLOAD_MAX_MB` MB succeeds.
- **Linked stories**: US-11.

### NFR-PERF-03: Realtime fanout latency

- **Requirement**: Median issue-event propagation from writing replica to viewing client ≤1 second; P95 ≤2 seconds.
- **Test**: Multi-client synthetic test: client A writes, client B (on a different replica) receives. Measure timestamp delta. Run with 100 simultaneous SSE subscribers.
- **Linked stories**: US-09, US-10.
- **JTBD link**: Outcome #4 — Linear-feel realtime.

### NFR-PERF-04: Multi-replica connection pool sizing

- **Requirement**: Each replica defaults to 10 Postgres connections. With 5 replicas, total = 50, well below Postgres's default 100. Override via `DATABASE_MAX_CONNECTIONS`.
- **Test**: Postgres `pg_stat_activity` shows ≤10 connections per replica at idle and ≤10 at sustained 50-RPS load.

---

## NFR-OBS — Observability

### NFR-OBS-01: Structured logs

- **Requirement**: All logs are JSON-formatted to stdout. Each log entry contains `timestamp`, `level`, `target`, `message`, and optionally `request_id`, `user_id`, `workspace_id`.
- **No log to file**: Containers must not write logs to disk; let the orchestrator (Docker, K8s) handle log capture.
- **Library**: `tracing` + `tracing-subscriber` with JSON formatter.
- **Test**: `docker compose logs foundry` output parses as JSON line-by-line.
- **JTBD link**: K8s-translatable operational posture.

### NFR-OBS-02: Health endpoints

- **Requirement**: Two unauthenticated endpoints:
  - `GET /healthz` — process is alive (HTTP 200, body `ok`). Returns 200 even if DB is unreachable (this checks the process, not its dependencies).
  - `GET /readyz` — process is ready to serve traffic (HTTP 200 if DB reachable AND migrations applied AND not draining; HTTP 503 otherwise).
- **Test**: Stop Postgres → `/healthz` stays 200, `/readyz` flips to 503 within 10 seconds.
- **Linked stories**: US-02 (multi-replica resilience).

### NFR-OBS-03: Prometheus metrics on a sidecar port

- **Requirement**: A separate HTTP listener on `METRICS_PORT` (default 9090) exposes `GET /metrics` in Prometheus exposition format. Default metrics include: `http_requests_total{path,method,status}`, `http_request_duration_seconds` (histogram), `db_connections_in_use`, `sse_subscribers_total`, `outbox_pending_jobs`, `bootstrap_tokens_unclaimed`.
- **Sidecar port rationale**: Keep metrics off the user-facing port; metrics endpoint is unauthenticated by design and must be firewalled at the operator level.
- **Library**: `prometheus` or `metrics-exporter-prometheus` crate.
- **Test**: `curl localhost:9090/metrics` returns valid exposition; metrics labels match documented schema.

### NFR-OBS-04: Request IDs

- **Requirement**: Every incoming HTTP request is tagged with a `request_id` (UUIDv7). Logs and error responses carry it. Clients receive it via `X-Request-Id` response header.
- **Test**: Compare `X-Request-Id` header on a request to the matching log line; they correlate.

---

## NFR-SEC — Security

### NFR-SEC-01: Password hashing

- **Requirement**: argon2id with parameters m=64 MiB, t=3, p=1. Revisit annually (track OWASP guidance).
- **Test**: Hashes have form `$argon2id$v=19$m=65536,t=3,p=1$...`.
- **Linked stories**: US-05, US-06.

### NFR-SEC-02: Brute-force protection (artificial delay, not lockout)

- **Requirement**: After 5 failed sign-in attempts within 15 minutes for the same email, subsequent attempts are delayed server-side by 5 seconds before responding.
- **Rationale**: Avoid lockouts (which enable denial-of-service attacks against legitimate users). Delays cap brute-force throughput while preserving recoverability.
- **Test**: Time the 6th failed attempt; should be ≥4.5 seconds.
- **Linked stories**: US-06.

### NFR-SEC-03: Session cookies

- **Requirement**: HttpOnly, Secure (in production behind HTTPS), SameSite=Lax, 30-day TTL.
- **Secure flag toggle**: env var `SESSION_COOKIE_SECURE` (default `true`); operators behind a localhost-only test may set to `false`.
- **Test**: Inspect Set-Cookie response header after sign-in.

### NFR-SEC-04: CSRF protection

- **Requirement**: All POST/PUT/DELETE form-driven endpoints require a CSRF token (double-submit cookie or hidden form field). htmx-driven calls include the token in the `HX-CSRF` header (set by an alpine.js hook at page load from a `meta` tag).
- **Test**: POST without a valid CSRF token returns HTTP 403.

### NFR-SEC-05: HTML sanitization

- **Requirement**: Markdown rendering sanitizes the output HTML. Disallowed: `<script>`, `<iframe>`, `<object>`, `<embed>`, event-handler attributes (`onclick`, etc.), and `javascript:` URLs.
- **Library**: `ammonia`.
- **Test**: Render markdown `[evil](javascript:alert(1))` and verify the resulting anchor's href is empty or removed.
- **Linked stories**: US-08 (issue description), US-10 (comment body).

### NFR-SEC-06: Authorization checks at every endpoint

- **Requirement**: Every protected endpoint checks that the requesting user is a member of the relevant workspace (and team, for team-scoped resources). Default-deny: no endpoint relies on "if no check is written, request passes."
- **Test**: A signed-in user from a different workspace receives HTTP 403 when accessing another workspace's resources.

### NFR-SEC-07: Secret management (MVP scope)

- **Requirement**: Secrets (`DATABASE_URL`, `SESSION_SECRET`, `SMTP_PASS`) are injected via env vars, sourced from a `.env` file via docker-compose for MVP. No secrets in the container image. Rotation: documented as "restart with new env" — sessions persist across SESSION_SECRET rotation because tower-sessions stores session data server-side, not in the cookie; rotating SESSION_SECRET only invalidates the bootstrap-token signature and similar HMACs.
- **Rotation story**: Document the rotation procedure in `docs/operations/secret-rotation.md`. K8s migration path swaps `.env` for `Secret` resources.
- **Test**: `docker inspect` on a running container shows no plaintext secret in image config, only in env at runtime.

### NFR-SEC-08: Dependency license audit

- **Requirement**: All Rust dependencies (transitive included) are MIT, Apache-2.0, BSD-2/3-clause, MPL-2.0, ISC, or unlicensed-public-domain (CC0). No GPL/LGPL/AGPL transitive deps that would impose obligations beyond the Foundry codebase itself (which is AGPLv3 by choice).
- **Test**: `cargo deny check licenses` passes in CI with the allow-list above.

---

## NFR-MIG — Database Migrations

### NFR-MIG-01: Forward-only, advisory-locked, idempotent

- **Requirement**: Migrations are SQL files in `migrations/` numbered sequentially (`0001_init.sql`, `0002_*.sql`, ...). At startup, the binary calls `sqlx migrate run` wrapped in a Postgres advisory lock (`pg_advisory_lock(MIGRATION_LOCK_ID)`) so concurrent replicas serialize. Migrations are forward-only — no down migrations.
- **Idempotency**: Re-running migrations on an already-migrated database is a no-op (sqlx tracks applied migrations in `_sqlx_migrations` table).
- **Test**: Start 3 replicas simultaneously against a DB needing one new migration; observe that the migration is applied exactly once and no replica errors.
- **Linked stories**: US-04.
- **JTBD link**: Outcome #6 — multi-replica operability.

### NFR-MIG-02: Migration failure rollback

- **Requirement**: Each migration runs in a transaction where Postgres permits. On failure, the transaction rolls back and the replica exits with non-zero status. The migrations table is not advanced.
- **Exception**: Operations that cannot be transactional in Postgres (e.g., `CREATE INDEX CONCURRENTLY`) are run outside transactions and the migration file header comment notes this. Such migrations require a documented manual recovery path.
- **Test**: A deliberately-broken migration in CI causes the replica to exit non-zero and the schema is unchanged.

### NFR-MIG-03: Migration impact in release notes

- **Requirement**: Every release that includes a migration documents:
  - Tables added/modified.
  - Expected runtime on 100k-issue / 10k-user databases.
  - Whether sessions / cookies / invites are invalidated.
- **Test**: Release notes review checklist.

---

## NFR-AVAIL — Availability and Resilience

### NFR-AVAIL-01: Multi-replica capability

- **Requirement**: Foundry must run N=1..10 replicas behind a round-robin LB with no sticky-session requirement and no in-process state required for correctness.
- **Test**: Run 3-replica test in CI; verify session-cookie + SSE survive replica restarts.
- **Linked stories**: US-02.

### NFR-AVAIL-02: Graceful shutdown

- **Requirement**: On SIGTERM, the replica:
  1. Sets `/readyz` to 503.
  2. Waits up to `SHUTDOWN_GRACE_SECONDS` (default 15) for in-flight requests to finish.
  3. Closes SSE connections cleanly (client-side EventSource reconnects to another replica).
  4. Exits.
- **Test**: Send SIGTERM during a load test; observe LB drains traffic and no requests are dropped mid-flight.

### NFR-AVAIL-03: SSE reconnect tolerance

- **Requirement**: Browsers (EventSource default) reconnect automatically. Server endpoint serves the `Last-Event-Id` request header and ignores it in MVP (no event replay); v0.4 implements replay.
- **Test**: Drop SSE connection; observe browser reconnect.

---

## NFR-DATA — Data Sovereignty and Backup

### NFR-DATA-01: All-state-in-Postgres

- **Requirement**: No Foundry data lives outside the Postgres database during MVP. Attachments are bytea. Sessions are rows. The outbox is a table. The only files in the container are the binary and static assets shipped in the image.
- **Test**: `docker inspect` shows no `Mounts` entries pointing to `/data` or similar paths owned by Foundry.
- **Linked stories**: US-03.

### NFR-DATA-02: pg_dump completeness

- **Requirement**: A single `pg_dump -Fc` produces a backup that, when restored on a fresh Postgres of the same major version, reproduces a functionally identical Foundry.
- **Test**: CI job runs full backup-restore-verify on each PR.

---

## NFR-PORT — K8s Portability

### NFR-PORT-01: No host-only assumptions

- **Requirement**: docker-compose files MUST NOT rely on features that don't translate to K8s. Specifically:
  - No `host` networking mode.
  - No path-dependent `bind` volumes for the app (`./logs:/var/log` etc. are forbidden).
  - All persistent volumes are named volumes or external (the Postgres volume is the only such; in K8s it becomes a PVC).
  - No use of `extra_hosts` for service discovery — services find each other via service names only.
- **Test**: A K8s manifest skeleton is provided alongside docker-compose; the same image runs in both.
- **Linked stories**: cross-cutting.
- **JTBD link**: Outcome #6 + future-proofing without locking-in containerization details.

### NFR-PORT-02: 12-Factor configuration

- **Requirement**: All configuration via env vars. No hot-reload of config at runtime (restart to change). No config files baked into the image.
- **Test**: All configuration knobs are documented in `.env.example`.

---

## NFR-LIC — Licensing and Compliance

### NFR-LIC-01: AGPLv3 hygiene

- **Requirement**: The Foundry codebase is AGPLv3-licensed. A `LICENSE` file is at the repo root. Every source file has an SPDX-License-Identifier header.
- **Test**: `reuse lint` (or equivalent) passes in CI.

### NFR-LIC-02: SPDX in build artifacts

- **Requirement**: The Docker image contains a `/licenses/` directory enumerating Foundry's license + the licenses of all linked Rust crates. Generated by `cargo about` or `cargo deny` at build time.
- **Test**: Image inspection finds the directory.

---

## NFR-DEV — Developer Experience (Contributors)

### NFR-DEV-01: Cold-start dev environment ≤10 minutes

- **Requirement**: From `git clone` to green `cargo test` on a fresh laptop in ≤10 minutes assuming Rust + Docker prerequisites met. No Redis, no S3, no Node toolchain.
- **Test**: Periodic timed onboarding session by a developer who has never touched Foundry.
- **Linked stories**: US-13.
- **JTBD link**: Outcome #3 — contributor productivity day 1.

### NFR-DEV-02: CI pipeline mirrors local dev

- **Requirement**: CI runs the same `cargo test` against the same Postgres-in-container that local dev uses. No CI-only flags or environments.
- **Test**: Compare CI scripts with README quickstart commands; they should be byte-identical or trivially equivalent.

---

## NFR-MATRIX — Story-to-NFR Coverage Matrix

| NFR | US-01 | US-02 | US-03 | US-04 | US-05 | US-06 | US-07 | US-08 | US-09 | US-10 | US-11 | US-12 | US-13 |
|-----|-------|-------|-------|-------|-------|-------|-------|-------|-------|-------|-------|-------|-------|
| PERF-01 |       |       |       |       |       |       | x     | x     |       |       |       | x     |       |
| PERF-02 |       |       |       |       |       |       |       |       |       |       | x     |       |       |
| PERF-03 |       |       |       |       |       |       |       |       | x     | x     |       |       |       |
| PERF-04 |       | x     |       |       |       |       |       |       |       |       |       |       |       |
| OBS-01  | x     | x     |       |       |       |       |       |       |       |       |       |       | x     |
| OBS-02  | x     | x     |       |       |       |       |       |       |       |       |       |       |       |
| OBS-03  |       | x     |       |       |       |       |       |       |       |       |       |       |       |
| OBS-04  | x     |       |       |       |       |       |       |       |       |       |       |       |       |
| SEC-01  |       |       |       |       | x     | x     |       |       |       |       |       |       |       |
| SEC-02  |       |       |       |       |       | x     |       |       |       |       |       |       |       |
| SEC-03  |       |       |       |       | x     | x     |       |       |       |       |       |       |       |
| SEC-04  |       |       |       |       | x     | x     | x     | x     |       | x     | x     |       |       |
| SEC-05  |       |       |       |       |       |       |       | x     |       | x     |       |       |       |
| SEC-06  |       |       |       |       | x     | x     | x     | x     | x     | x     | x     | x     |       |
| SEC-07  | x     |       |       |       |       |       |       |       |       |       |       |       |       |
| SEC-08  |       |       |       |       |       |       |       |       |       |       |       |       | x     |
| MIG-01  | x     | x     |       | x     |       |       |       |       |       |       |       |       |       |
| MIG-02  |       |       |       | x     |       |       |       |       |       |       |       |       |       |
| MIG-03  |       |       |       | x     |       |       |       |       |       |       |       |       |       |
| AVAIL-01|       | x     |       | x     |       |       |       |       |       |       |       |       |       |
| AVAIL-02|       | x     |       | x     |       |       |       |       |       |       |       |       |       |
| AVAIL-03|       | x     |       |       |       |       |       |       | x     |       |       |       |       |
| DATA-01 |       |       | x     |       |       |       |       |       |       |       | x     |       |       |
| DATA-02 |       |       | x     |       |       |       |       |       |       |       |       |       |       |
| PORT-01 | x     | x     |       |       |       |       |       |       |       |       |       |       |       |
| PORT-02 | x     |       |       |       |       |       |       |       |       |       |       |       |       |
| LIC-01  |       |       |       |       |       |       |       |       |       |       |       |       | x     |
| LIC-02  |       |       |       |       |       |       |       |       |       |       |       |       | x     |
| DEV-01  |       |       |       |       |       |       |       |       |       |       |       |       | x     |
| DEV-02  |       |       |       |       |       |       |       |       |       |       |       |       | x     |
