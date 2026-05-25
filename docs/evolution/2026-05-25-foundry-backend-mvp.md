# Evolution — foundry-backend-mvp (Slice 1)

**Finalized**: 2026-05-25
**Ship commit**: [33e5f6f](../../) — "Initial commit: Foundry MVP — Slices 1 + 2"
**Wave coverage**: DIVERGE → DISCUSS → DESIGN → DISTILL → DELIVER

## Feature summary

Slice 1 of the Foundry MVP — the foundational issue-tracker backend.
Five user stories shipped end-to-end via Outside-In TDD: operator
install, admin bootstrap, user sign-in, project create, issue file.
Together these prove the walking-skeleton claim that Foundry can be
`docker compose up`'d on a fresh machine, an admin can claim it, and
a user can sign in and file an issue — all in under an hour.

This is the slice that made every later slice possible: the 5-crate
workspace boundary, the cucumber-rs harness with testcontainers
Postgres + per-scenario schema rotation, the xtask architecture-
enforcement compiler, and the in-repo nWave artefact tree that the
remaining slices inherited verbatim.

## Business context

The foundational JTBD: "Help me operate my team's planning loop on
infrastructure I control, with the speed and feel my team already
trusts, so I can extend it (agentic workflows, custom fields,
integrations) without vendor permission."

Five JTBD outcomes drive the slice — the load-bearing one is
**outcome-1: minimize time to stand up a working issue tracker
(importance 9, satisfaction today 3, opportunity 15, under-served)**.
US-01 directly closes that outcome at the operator level; US-05–08
prove the resulting system is actually usable.

## Key decisions

### From DIVERGE (`diverge/recommendation.md`)

- **D1 "Boring Monolith" chosen at weighted total 4.90.** Five
  directions evaluated (D1 Boring Monolith, D2 Two-Mode Binary, D3
  Web Components, D4 CP/DP Split, D5 OIDC-Delegated). D1 won on T1
  Subtraction and T4 Speed-as-Trust; D5's enterprise advantage was
  preserved as a v0.3 hook, D2's mode-switching as a v0.5 hook, and
  doltgresql evaluation reserved for v1.0.
- **AGPLv3 license.** Aligns with the JTBD "no extraction tax"
  outcome; constrains downstream SaaS forks; cargo-deny is configured
  to refuse AGPL-incompatible deps.

### From DESIGN (`design/architecture.md` + ADRs)

- **ADR-001 — Five-crate Cargo workspace.** `foundry-core` (I/O-free
  domain) + `foundry-store` (sqlx) + `foundry-auth` (argon2/HMAC) +
  `foundry-realtime` (LISTEN + broadcast) + `foundry-app` (axum
  binary). Compile-time architecture enforcement via the `xtask
  check-arch` job: `foundry-core` cannot depend on anything I/O-bound.
- **ADR-002 — askama for templating** (compile-time HTML, type-safe
  template bindings, vs. tera/handlebars which are runtime-checked).
- **ADR-003 — sqlx for SQL** (compile-time SQL verification against a
  real Postgres, vs. diesel's macro DSL).
- **ADR-004 — Sessions stored in Postgres** via `tower-sessions`.
  Multi-replica-ready from day one — no sticky-session requirement,
  no Redis sidecar (see ADR-101).
- **ADR-005 — `thiserror` per crate + `IntoResponse` adapter** at the
  axum boundary. Domain errors never know about HTTP; the adapter
  layer maps them to status codes and HTML fragments.

### From DESIGN — system-level (`design/system/adrs/`)

- **ADR-101 — Postgres-for-everything (no Redis).** Sessions, the
  outbox, pg_notify/LISTEN for realtime — all in Postgres. Eliminates
  an operational dependency for the "under an hour" install goal.
- **ADR-102 — `docker-compose` as primary deploy artefact** (vs.
  helm-from-day-one). Helm chart deferred to v0.x once docker-compose
  has proven its operator UX.
- **ADR-103 — Caddy as default LB + TLS terminator.** Automatic
  Let's Encrypt, zero config for the happy path.
- **ADR-104 — Minimal observability by default.** Structured stdout
  logs + Prometheus `/metrics` only. Tracing + Loki + Tempo are
  opt-in via the observability overlay (`docker-compose.observability.yml`).
- **ADR-105 — Single-Postgres SPOF accepted for v0.1.** The HA
  Postgres story (Patroni / Stolon) is a v0.x track. The walking-
  skeleton's value proposition is "an hour to install"; HA Postgres
  is incompatible with that bar.

### From DISTILL (`distill/driver.md` + `coverage-matrix.md`)

- **Strategy C — all real adapters.** Every scenario exercises
  production driving adapters (HTTP through axum, real Postgres
  through sqlx) and real driven adapters (testcontainers Postgres,
  not in-memory fakes). Established the precedent slices 2–4 all
  inherited via `docs/architecture/atdd-infrastructure-policy.md`.
- **Per-scenario schema rotation.** Each cucumber scenario gets a
  fresh Postgres schema in the same container. Cheap (~10 ms) and
  fully isolated.
- **MockClock for time-dependent assertions.** US-06's brute-force
  delay (250 ms after wrong password) is asserted via a fakable
  clock injected into `foundry-auth` — no `tokio::time::sleep` in
  tests.
- **Sequential AUTH-N issue numbering must be gap-free under rollback.**
  US-08's invariant was load-bearing for the slice — a transactional
  outbox + a Postgres sequence reserved at insert time, with the
  rollback path asserted explicitly.

## Steps completed

No `deliver/execution-log.json` was emitted (slice 1 predates the
nWave execute orchestrator). The single ship commit `33e5f6f`
enumerates the delivered scope:

### 5 user stories (37 scenarios)

- **US-01 install** — `docker compose up` → live foundry, bootstrap URL in logs
- **US-05 bootstrap** — single-use HMAC token, atomic admin claim, invite link
- **US-06 sign-in** — argon2id verify, MockClock brute-force delay, Postgres sessions, CSRF double-submit + middleware integration test
- **US-07 project create** — ProjectKey regex invariant, duplicate 409, htmx fragment errors
- **US-08 file issue** — sequential AUTH-N numbering (gap-free under rollback), outbox event, P95 = 4 ms vs 200 ms budget

### 5-crate workspace + harness

- `foundry-core` — I/O-free domain (entities, value objects, errors)
- `foundry-store` — sqlx adapters
- `foundry-auth` — argon2id + HMAC + clock-injectable rate limiting
- `foundry-realtime` — pg_notify LISTEN + tokio broadcast (used by slice 2)
- `foundry-app` — axum binary
- `crates/foundry-acceptance` — cucumber-rs harness with testcontainers Postgres + per-scenario schema rotation
- `xtask` — compile-time architecture enforcement (`check-arch`, `check_probes`)

## Verification at HEAD (`33e5f6f`)

- `cargo build --release --all` green
- `cargo test --workspace` (52 default-tag scenarios — includes slice 2)
- `FOUNDRY_ACCEPTANCE_TAGS=docker-compose cargo test -p foundry-acceptance` (3 additional `@docker-compose` scenarios)
- `cargo clippy --all-targets -- -D warnings` clean
- `cargo fmt --all -- --check` clean
- `cargo deny check` clean (AGPLv3-compatible deps only)
- Multi-replica ready (sessions in Postgres, no sticky-session requirement, advisory-lock-safe migrations)

## Lessons learned

1. **Compile-time architecture enforcement is worth the xtask
   investment.** `xtask check-arch` runs on every CI build and refuses
   to let `foundry-core` take an I/O-bound dep. Slices 2–4 all
   benefited; the rule never had to be re-litigated in code review.
2. **Per-scenario schema rotation beats per-scenario container.**
   Initial sketch was "one testcontainer per scenario." Rotation
   inside a single container is ~100× faster and gives the same
   isolation guarantee for everything except DDL contention (which
   no slice has hit).
3. **Postgres-for-everything is a real ergonomic win.** ADR-101
   eliminated Redis from the contributor onboarding contract. Slice 4
   (US-13) cashed in on this directly — "no Redis, no S3, no Node
   toolchain" was assertable without exception.
4. **The 5-crate boundary needed all 5 from the start.** The temptation
   to merge `foundry-auth` into `foundry-core` was strong (auth feels
   "domain-y"); resisting it paid off in slice 2 when realtime
   filtering needed a clock-injectable rate limit without dragging
   the domain into a clock-aware shape.
5. **HTML-fragment errors via htmx, not JSON.** The MVP renders
   server-side; treating errors as HTML fragments (instead of JSON
   with client-side rendering) kept the entire error path within the
   existing askama template system. No client-side error renderer to
   maintain.

## Issues encountered

- **DELIVER ran outside the nWave execute orchestrator.** No
  `deliver/roadmap.json` or `deliver/execution-log.json` was created.
  Slice 1 + 2 were both squashed into the initial commit, so even
  per-story commits are unavailable. The `33e5f6f` commit body is
  the single audit-trail substitute for both slices.
- **No `discover/` wave artefact.** Discovery happened informally
  pre-bootstrap; only DIVERGE captured the JTBD-and-direction work.
  Future projects starting fresh should run `/nw:discover` so
  outcome scoring has a documented provenance.

## Permanent artefact locations

All artefacts stay in their delivery locations — `docs/feature/foundry-backend-mvp/`
is referenced from 8+ external sites (README, CONTRIBUTING, RELEASING,
`docker-compose.observability.yml`, `crates/foundry-app/src/admin_cli.rs`,
`deploy/k8s/README.md`, and the sibling features `foundry-realtime-collab`,
`foundry-operator-grade`, `foundry-contributor-onboarding`). Relocating
would silently break those references for zero benefit, since the workspace
already serves the lasting-reference role from where it is.

Notable references that callers depend on:

- `README.md:20` — `docs/feature/foundry-backend-mvp/distill/features/`
- `README.md:116` — `docs/feature/foundry-backend-mvp/design/`
- `CONTRIBUTING.md:182` — full nWave-artefact overview block
- `RELEASING.md:237` — `design/system/migrations.md`
- `docker-compose.observability.yml:13` — `design/system/observability-infra.md`
- `crates/foundry-app/src/admin_cli.rs:5` — Rust doc comment pointing at `design/system/backup-restore.md`
- `deploy/k8s/README.md:93,135` — `design/system/migrations.md` + `backup-restore.md`

## Open items for v0.1 RC

1. Single-Postgres SPOF is documented and accepted (ADR-105); the HA
   Postgres track is a v0.x deliverable. Surface this in RELEASING.md.
2. The OIDC v0.3 hook (D5) and the two-mode-binary v0.5 hook (D2)
   are referenced in `diverge/recommendation.md` and should appear
   in the roadmap.
3. The doltgresql v1.0 evaluation is a research deliverable, not an
   engineering deliverable — owners should be assigned before v0.5.
