# Evolution — foundry-operator-grade (Slice 3)

**Finalized**: 2026-05-25
**Ship commits**: [b2475f6](../../) (slice 3) + [57b13f9](../../) (polish)
**Wave coverage**: DISTILL → DELIVER (DIVERGE/DISCUSS/DESIGN inherited from slice 1)

## Feature summary

Operator-grade hardening of the Foundry MVP. Four user stories shipped
together (US-02 multi-replica, US-03 backup/restore, US-04 rolling
upgrade, US-11 attachments) — 24 acceptance scenarios green, taking
the suite from 55/55 to 78/78. The slice turns the slice-1 walking
skeleton into something an operator can actually run in production:
fan-out across replicas with shared sessions, byte-identical
backup/restore with a `foundry doctor backup-verify` CLI, zero-
downtime rolling deploy via Postgres advisory-locked migrations, and
file attachments with cascade-delete semantics.

## Business context

Slice 3 is the gap between "Foundry runs" (slice 1) and "Foundry is
operable." US-01 proved a single instance could be installed in under
an hour; slices 2–3 prove the result is fit for production teams:
multi-replica scale-out, restorable backups, in-place upgrade, and
file attachments to make issue threads load-bearing.

## Key decisions

### From DISTILL (`distill/wave-decisions.md`)

- **Strategy C inherited.** Every scenario runs against real
  testcontainers Postgres + real axum binaries + real subprocesses.
  No in-memory fakes; the new driving adapters (round-robin proxy,
  `pg_dump`/`pg_restore` subprocesses, `foundry doctor backup-verify`
  CLI, multipart upload) are exercised end-to-end.
- **US-02 Option A (in-process round-robin proxy) for 6 scenarios +
  one Option B (docker-compose --scale + Caddy) for 1 scenario.**
  In-process proxy keeps the default loop ~3 s; the production-shaped
  compose scenario is gated by `@docker-compose @us-02 @manual-trigger`
  so it stays out of the fast loop but is one tag-filter away on
  demand. Option C (kind/k3d K8s cluster) deferred.
- **US-04 Option B (per-scenario temp-dir migration files) over
  Option A (two cargo-feature-gated binaries).** The real
  `sqlx::migrate::Migrator` + `pg_advisory_lock` code runs unchanged;
  test-only complexity stays in the harness, not the production
  binary.
- **Explicitly not tested: "old replica keeps serving old SQL during
  migration."** Enforced by the expand-only migration discipline
  (NFR-MIG-02) + code-review header comments. Black-box-asserting
  this would require two feature-gated binaries with different schema
  knowledge — rejected as test-only overhead that doesn't earn its
  keep.
- **`pg_dump` + `pg_restore` are contributor prereqs**, probed at
  suite startup with a clear panic on absence. macOS: `brew install
  libpq && brew link --force libpq`. Ubuntu: `apt-get install
  postgresql-client-16`. CI base image already includes them.

### From DISTILL gate-review remediation (2026-05-24)

- **Business-language purity in Gherkin (CM-B).** First pass was
  rejected; implementation-tool names removed from all 4 `.feature`
  files: `pg_dump`/`pg_restore` → "back up"/"restore"; `Postgres` →
  "the database"; `bytea` → "the database"; `multipart` → "upload";
  `sha256` → "byte-identical". Kept on purpose: HTTP status codes in
  `@error` scenarios (the form-submitter SEES a 413/401/403);
  `/readyz` and `SIGTERM` in US-02 (operators literally watch
  these); the `foundry doctor backup-verify` subcommand name (it IS
  the operator-facing contract).
- **US-04 WS scenario reframed.** "Two replicas race to apply a new
  migration; the advisory lock serialises them…" was implementation-
  language; rewritten as "An operator deploys a new version and two
  replicas start in parallel; the schema update applies exactly once
  and both replicas come up healthy." The advisory-lock mechanism is
  still asserted, just not named in the title.

### From DELIVER (extracted from `b2475f6` + `57b13f9` commit bodies)

- **`Box::leak` removed from harness OnceCells.** Static doesn't
  guarantee `Drop` at process exit, so the leak was only partially
  fixed — testcontainers' built-in reaper handles cleanup
  eventually. Operator cleanup one-liner documented in CONTRIBUTING
  alongside a placeholder for a future `cargo xtask docker-prune-leaked`
  subcommand.
- **cargo-test concurrency cap lowered 8 → 6.** US-04's parallel
  spawn (independent advisory-lock racers) saturated 8-way concurrency
  on dev hardware; 6 keeps the race genuine without OOM-ing
  testcontainers.
- **All slice-3 AppState additions feature-gated.** `db_unreachable`,
  `test_migrations_dir`, `applied_migrations`, `test_migration_delay_ms`,
  `replica_id` are all `cfg(any(test, feature = "test-support"))`.
  Release `cargo build --release -p foundry-app` (no features)
  produces a binary with zero test seams.
- **`foundry-core` still I/O-free.** `cargo tree -p foundry-core`
  confirms no sqlx/axum/tokio/reqwest — slice-1's architectural
  promise survived a 23-scenario hardening pass.

### From DELIVER polish (`57b13f9`)

- **`build.rs` for `foundry-store` emits `rerun-if-changed=migrations`.**
  `sqlx::migrate!()` embeds migrations at compile time but doesn't
  emit the directive, so cargo treated the crate as up-to-date when
  new migrations landed — `docker compose up -d` would silently
  re-use a binary with old schema baked in. Real foot-gun caught only
  by end-to-end production walkthrough.
- **URL shape unification: `/issue/{n}` → `/issues/{n}`.** Slice 2
  used the singular form; slice 3 mounted attachments under the
  plural; operators (and frontend formatting code) shouldn't have to
  remember which. Plural wins.
- **Three documented invocation patterns for `foundry doctor
  backup-verify` in distroless.** The `gcr.io/distroless/cc-debian12`
  runtime has no shell and no `pg_restore`; operators following the
  happy-path docs would otherwise hit a clear-but-frustrating error.
  Patterns: run on host with postgres-client-16, mount the binary
  into ephemeral `postgres:16-alpine`, or pair the foundry image
  with a sidecar in a K8s Job.

## Steps completed

No `deliver/execution-log.json` was emitted (DELIVER ran outside the
nWave execute orchestrator). The two ship commits `b2475f6` + `57b13f9`
enumerate the delivered scope.

### 4 user stories (24 scenarios; 23 new + walking-skeleton)

- **US-11 — File attachments** (7 scenarios). Migration 0005 adds
  `issue_attachments` (bytea content, sha256, size_bytes,
  content_type, ON DELETE CASCADE); CSRF-protected multipart upload,
  10 MB default cap via `FILE_UPLOAD_MAX_MB`, sanitized filename,
  upfront Content-Length pre-check; member-only download with
  Content-Disposition + Content-Type round-trip; cascade delete
  verified.
- **US-03 — Backup + restore + `foundry doctor backup-verify` CLI**
  (6 scenarios). `pg_restore --list` parsing, reusable probe schema,
  `exit 0` + `status: OK` on healthy backups, non-zero + clear stderr
  on truncated ones. Attachment integrity verified byte-identically
  across dump+restore. "No state outside the DB" scenario drops all
  Foundry tables, restores, and asserts complete recovery.
- **US-02 — Multi-replica** (7 scenarios). ~250-LOC `round_robin_proxy`
  with SSE-streaming + `Set-Cookie` pass-through + `X-Foundry-Replica`
  injection. `MultiReplicaHarness::spawn(n)` provisions one shared
  PG schema + N axum replicas behind the proxy. Session sharing
  verified across replicas; IssueCreated fan-out verified;
  health-injection via `AppState::db_unreachable: Arc<AtomicBool>`
  flips `/readyz` to 503 without poisoning the shared Postgres;
  SIGTERM-drain via `TestApp::shutdown_graceful()`. One
  `@docker-compose @manual-trigger` scenario covers production-shaped
  Caddy + 3-replica compose stack.
- **US-04 — Rolling upgrade** (4 scenarios).
  `foundry_store::run_migrations_from_dir(pool, path)` is the runtime
  sibling of `sqlx::migrate!`, wrapped in a schema-scoped advisory
  lock (FNV-1a hash of `search_path`; production keeps canonical
  `0xF00DBABEF00DBABE` for `public`).
  `MultiReplicaHarness::spawn_concurrent(n, migrations_dir)` uses
  `join_all` over N independent spawns so the advisory-lock race is
  genuine. Failed-migration scenario verifies rollback (no
  `_sqlx_migrations` row, no schema change); idempotency verifies
  restart applies zero; slow-migration verifies the blocked replica
  waits then observes already-applied.

### Polish (`57b13f9`)

- `crates/foundry-store/build.rs` — emits `rerun-if-changed=migrations`
- URL consistency: `/issue/{n}` → `/issues/{n}` (touched
  `crates/foundry-app/src/lib.rs`, `comments.rs`, `attachments.rs`,
  2 acceptance step files, `discuss/journey.md` URL map)
- `RELEASING.md` documents the 3 distroless `backup-verify`
  invocation patterns

## All slice-3 goals satisfied

- [x] US-02 multi-replica acceptance contract pinned (7 scenarios incl. 1 `@docker-compose @manual-trigger`)
- [x] US-03 backup/restore + `foundry doctor backup-verify` CLI (6 scenarios; pg_dump/pg_restore prereq documented + suite-startup-probed)
- [x] US-04 rolling upgrade via Postgres advisory-locked migration (4 scenarios; failed migration + idempotency + slow migration)
- [x] US-11 attachments with CSRF + member-auth + cascade delete (7 scenarios)
- [x] Suite-time budget held: slice-3 default loop ~14.5 s (within "≤+20 s on top of slice 1+2" target); total ~30–32 s (well under 60 s top-line)
- [x] Reviewer (`nw-software-crafter-reviewer`) APPROVED with zero critical issues
- [x] Production foot-guns caught and fixed in polish commit

## Verification at HEAD (`57b13f9`)

- `cargo xtask ci` → 78 scenarios green (slice 1 = 37 incl. 3 `@docker-compose` + slice 2 = 19 + slice 3 = 22)
- `cargo build --release --all` green
- `cargo clippy --all-targets --release -- -D warnings` clean
- `cargo fmt --all -- --check` clean
- `cargo deny check` clean
- `cargo build --release -p foundry-app` (no features) — release binary contains no test-support seams
- End-to-end production walkthrough on `docker compose down -v && docker compose build --no-cache && docker compose up -d --wait` reproduced cleanly: admin claim → project create → issue file → attachment upload (byte-identical sha256 round-trip) → CLI backup verification

## Lessons learned

1. **End-to-end production walkthroughs catch what acceptance tests
   can't.** All three polish fixes (`build.rs`, URL consistency,
   distroless docs) were invisible to a green 78/78 acceptance suite
   — they only surfaced when an operator actually clicked through a
   fresh deploy. The acceptance suite is necessary; the walkthrough
   is also necessary.
2. **`sqlx::migrate!()` does not emit `rerun-if-changed`.** This is
   exactly the class of foot-gun macro-embedded build inputs cause:
   the macro hides what the build system needs to know. Future
   macro-embedded inputs should ship with a `build.rs` that
   reconstructs the cargo dependency edge.
3. **Test-support feature gating must be uniform across AppState.**
   Five new fields all behind the same `cfg(any(test, feature =
   "test-support"))` makes the no-test-seams-in-release proof a
   one-liner: `cargo build --release -p foundry-app` and inspect.
4. **Hand-rolling a ~250-LOC round-robin proxy beat both an in-process
   Caddy and `hyper-reverse-proxy`.** Zero new transitive deps in
   the test crate; full control over which replica is "currently
   routed to" for the SSE-landing-replica assertion. The "build vs
   buy" call paid for itself in test debuggability.
5. **DISTILL's gate-review caught real issues.** First pass was
   rejected on business-language purity + WS framing; the
   remediation produced a Gherkin set that reads as a stakeholder
   document, not a test script. Reviewer effort earns its keep when
   the rejection criteria are concrete.

## Issues encountered

- **DELIVER ran outside the nWave execute orchestrator.** No
  `deliver/roadmap.json` or `deliver/execution-log.json` was emitted;
  audit trail is reconstructed from `b2475f6` + `57b13f9` commit
  bodies.
- **DISTILL gate review rejected on first pass.** Two blockers
  (Gherkin business-language purity, US-04 walking-skeleton framing)
  required a second DISTILL pass. Both are now resolved and the
  remediation is captured in `wave-decisions.md` § "Gate-review
  remediation (2026-05-24)".
- **`Box::leak` only partially removed.** Rust statics don't
  guarantee `Drop` at process exit; testcontainers' built-in reaper
  handles cleanup eventually. Documented in CONTRIBUTING with a
  placeholder for a future `cargo xtask docker-prune-leaked`.

## Permanent artefact locations

All artefacts stay in their delivery locations.
`docs/feature/foundry-operator-grade/` has one inbound cross-reference
from a sibling feature workspace (`docs/feature/foundry-contributor-onboarding/distill/driver.md`),
and the slice's design context is inherited from the slice-1 design
artefacts already preserved at `docs/feature/foundry-backend-mvp/design/`.

## Open items for v0.1 RC

1. `cargo xtask docker-prune-leaked` subcommand — placeholder noted
   in CONTRIBUTING; ship as a one-off operator convenience before v0.1.
2. The `@docker-compose @us-02 @manual-trigger` scenario stays out of
   the default loop; add to the v0.1 CI matrix opt-in lane.
3. `Box::leak` removal could be revisited if a Drop-guaranteed
   pattern emerges in the testcontainers ecosystem.
