# DISTILL Driver Design — Slice 3 Acceptance Harness (operator-grade)

Owner: acceptance-designer (DISTILL). Companion: `step-skeletons.md`,
`coverage-matrix.md`, `wave-decisions.md`. This document is an
**additive delta** to the slice-1 and slice-2 driver designs
(`docs/feature/foundry-backend-mvp/distill/driver.md` and
`docs/feature/foundry-realtime-collab/distill/driver.md`). Everything
not mentioned here is inherited unchanged.

## 1. What slices 1+2 already provide (inherited, do not re-build)

From `crates/foundry-acceptance/`:

- `support/harness.rs` — `InProcHarness::spawn(now)`, `ensure_postgres()`,
  `fresh_schema_pool()`, `fresh_schema_pool_with_url()`, `drop_schema()`,
  `signed_in_post()` with full CSRF dance.
- `support/compose_harness.rs` — US-01 docker-compose driver (slice 3
  reuses the *pattern* for `@docker-compose @us-02`; the file itself
  stays untouched and a new sibling appears for the multi-replica
  variant).
- `support/sse_client.rs` — full SSE consumer (slice 3 uses this
  unchanged for US-02's "SSE auto-reconnect" scenario, with one
  additive method — see §2c).
- `support/html_assertions.rs` — scraper helpers (slice 3 uses this
  unchanged for US-11's "attachment is listed on the issue page"
  assertion).
- `support/heartbeat_env.rs` — heartbeat override (unused in slice 3
  but inherited; the multi-replica SSE scenario does not need a fast
  heartbeat).
- `world.rs` — `FoundryWorld` struct with the slice-1 + slice-2 fields.
  Slice 3 appends new optional fields (§3 below); existing scenarios
  unaffected.
- Testcontainers Postgres-16 container (shared), per-scenario schema
  rotation, cucumber-rs runner at `tests/acceptance.rs`, tag-filtering
  CI plumbing.

Slice 3 plugs into the same world struct and the same per-scenario
isolation.

## 2. What slice 3 adds to the harness

Four new support modules in `crates/foundry-acceptance/src/support/`.

### 2a. `round_robin_proxy.rs` — in-process round-robin reverse proxy (US-02 Option A)

A purpose-built axum-based proxy that round-robins reqwest calls to
N replica `SocketAddr`s. Rationale captured in `wave-decisions.md`
§US-02: zero new transitive deps, full control over which replica is
"currently routed to" for the SSE-landing-replica assertion.

Public surface:

```rust
pub struct ProxyHandle {
    pub addr: SocketAddr,
    /// The order the proxy will hit the upstreams. Tests assert
    /// distribution by counting per-replica request observations.
    upstreams: Vec<SocketAddr>,
    health: Arc<HealthMap>,
    _shutdown: oneshot::Sender<()>,
}

pub struct HealthMap {
    /// Per-upstream healthy/unhealthy gate. Updated by the background
    /// /readyz polling task; the proxy skips upstreams marked unhealthy.
    inner: Mutex<HashMap<SocketAddr, bool>>,
}

/// Spawn the proxy in front of N already-booted axum replicas. The
/// proxy binds to 127.0.0.1:0; the bound `SocketAddr` is on the
/// returned handle. A background task polls each upstream's /readyz
/// every 200ms and toggles the HealthMap accordingly.
pub async fn spawn_round_robin_proxy(
    replicas: Vec<SocketAddr>,
) -> ProxyHandle;

impl ProxyHandle {
    /// Force a single upstream out of rotation (test affordance for
    /// the "replica stopped" scenario). The proxy will not route to
    /// this upstream until `restore_replica` is called.
    pub fn fail_replica(&self, addr: SocketAddr);
    pub fn restore_replica(&self, addr: SocketAddr);

    /// Snapshot of per-replica request counts observed by the proxy
    /// since spawn. Used by the "distributed across all 3 replicas"
    /// assertion.
    pub fn request_counts(&self) -> HashMap<SocketAddr, u64>;

    /// Convenience: the public URL the test reqwest::Client should hit.
    pub fn base_url(&self) -> String;
}
```

Implementation sketch (~50 LoC):

- One axum service with a catchall route.
- An `AtomicUsize` counter; `next_upstream()` reads, increments,
  picks the next *healthy* upstream skipping unhealthy ones.
- Forward by building a new `reqwest::Request` mirroring the incoming
  method/path/headers/body, sending it via a `reqwest::Client` keyed
  to the upstream, and copying back status/headers/body to the axum
  response.
- SSE pass-through: detect `Accept: text/event-stream` and stream the
  body without buffering (axum's `Body::from_stream`).
- The proxy adds an `X-Foundry-Replica` response header naming the
  upstream that served the request — tests use this to assert which
  replica served which request (the SSE-landing-replica assertion).
- The background `/readyz` poller is one tokio task per upstream that
  polls every 200ms with a 2s timeout.

### 2b. `multi_replica_harness.rs` — N-replica spawn (US-02 Option A)

A builder that wraps the slice-1 `InProcHarness` to produce N
`TestApp`s sharing **one per-scenario Postgres schema** (so sessions
cross replicas without per-replica schema bleed) plus a round-robin
proxy in front.

Public surface:

```rust
pub struct MultiReplicaHarness {
    /// The N replicas, in spawn order. Tests can introspect each one
    /// individually (e.g. for /readyz checks).
    pub replicas: Vec<TestApp>,
    /// The shared per-scenario schema (every replica's `spawn_app`
    /// receives the same schema-pinned PgPool).
    pub schema: String,
    /// The round-robin proxy in front of all replicas.
    pub proxy: ProxyHandle,
    /// The fake clock + fake email shared across all replicas (so
    /// the slice-1 fake-port contract holds at the cluster level).
    pub fake_clock: Arc<MockClock>,
    pub fake_email: Arc<FakeEmailSender>,
}

impl MultiReplicaHarness {
    pub async fn spawn(n: usize, now: OffsetDateTime) -> Self;

    /// Stop a replica (for "replica goes down" scenarios). The
    /// proxy's health poller observes the drop within ~200ms.
    pub async fn stop_replica(&mut self, idx: usize);

    /// Manually flip a replica's /readyz to 503 (simulates DB
    /// outage from that replica's perspective).
    pub async fn mark_replica_db_unreachable(&mut self, idx: usize);

    pub fn base_url(&self) -> String { self.proxy.base_url() }
}
```

Implementation notes:

- The shared per-scenario schema is the linchpin: per-scenario schema
  rotation in slice 1 keyed each scenario to one schema; slice 3
  reuses that schema *across the N replicas of a single scenario*.
  This matches production (one Postgres, N replicas).
- The fake `MockClock` is shared via `Arc` so all replicas observe
  the same time — without this, two replicas would disagree on
  `expires_at` checks.
- Replica spawn order matters for the US-04 "race for migration lock"
  scenario; that scenario uses a different harness entry point —
  `MultiReplicaHarness::spawn_concurrent(n, migrations_dir)` — that
  boots all replicas in parallel via `join_all` so the advisory-lock
  race is the actual production race.

### 2c. `pg_backup.rs` — system pg_dump / pg_restore shell-out (US-03)

A thin wrapper over `std::process::Command` exposing the two
operations the US-03 scenarios need. Probes for binary presence at
suite startup; missing binaries produce a clear panic (F-004 anti-flake).

Public surface:

```rust
/// Probe `pg_dump --version` and `pg_restore --version` at suite
/// startup. Panics with a contributor-friendly message if either is
/// missing.
pub fn probe_pg_tools_on_path();

/// Run `pg_dump -Fc -d foundry` against `pg_url`, writing the dump
/// to `out_path`. `pg_url` includes credentials. Returns the dump
/// file size in bytes.
pub async fn dump_to_file(pg_url: &str, out_path: &Path) -> u64;

/// Boot a fresh testcontainers Postgres and return its connection
/// URL. Each US-03 scenario gets its own restore target — restore is
/// destructive and would poison sibling scenarios sharing one Postgres.
pub async fn spawn_restore_target() -> (RestoreTarget, String);

/// RAII wrapper that drops its underlying testcontainers container
/// when it goes out of scope.
pub struct RestoreTarget { /* ContainerAsync<Postgres>, ... */ }

/// Run `pg_restore --clean --if-exists -d foundry <dump_file>` against
/// `pg_url`.
pub async fn restore_from_file(pg_url: &str, dump_file: &Path);

/// Truncate a dump file to its first N bytes (for the
/// `@us-03-cli @error` scenario that expects backup-verify to fail).
pub fn truncate_dump(dump_file: &Path, keep_bytes: usize);
```

Implementation notes:

- Each restore target is a fresh testcontainers Postgres because
  restore is destructive; the slice-1 shared container cannot be
  reused (would break parallel scenarios).
- Cost amortisation: the WS scenario is the only one that needs a
  full second container; the sibling scenarios (round-trip, key
  continuity, NFR-DATA-01, the two CLI scenarios) can share a single
  restore target per file via a `lazy_static`-style helper if the
  US-03 lane becomes a budget hotspot. Default: one per scenario;
  revisit if pg_restore startup dominates the slice-3 budget per
  `wave-decisions.md` open decisions.
- `pg_dump -Fc` produces the custom-format that `backup-verify` and
  `pg_restore` both accept.

### 2d. `test_migration.rs` — per-scenario migration staging (US-04)

A helper that writes a test-only `.sql` file into a per-scenario
temp directory and yields a `MigrationsDir` handle that
`AppState`'s spawn path can point `sqlx::migrate!` at.

Public surface:

```rust
pub struct MigrationsDir {
    pub path: PathBuf,
    /// The temp dir backing this path; dropped at scenario teardown.
    _temp: tempfile::TempDir,
}

/// Materialise the canonical slice-1/2 migrations into a temp dir,
/// then append `extra_sql` as a new migration named
/// `<version>_<descriptor>.sql`. Returns the populated dir.
pub fn stage_test_migration(
    version: u32,           // e.g. 99
    descriptor: &str,       // e.g. "add_dummy_column"
    extra_sql: &str,
) -> MigrationsDir;
```

The trick: `sqlx::migrate!` is a compile-time macro keyed to a
literal path, so slice 3 cannot point it at a runtime path via the
macro. DELIVER will add a `foundry_store::run_migrations_from_dir(pool,
path)` companion to the existing `run_migrations(pool)`; the test
path uses the runtime variant. The production binary continues to use
the compile-time macro version. Both code paths share the same
advisory-lock wrapper.

## 2e. Additive: `SseSubscription::landing_replica()` (slice 2 extension)

US-02's "SSE auto-reconnects to a healthy replica" scenario requires
knowing which replica the SSE stream landed on. The proxy adds an
`X-Foundry-Replica` response header; the SSE client captures it on the
open response and exposes:

```rust
impl SseSubscription {
    /// The `SocketAddr` of the replica that served this subscription.
    /// `None` if the response was refused (open_status != 200).
    pub fn landing_replica(&self) -> Option<SocketAddr>;
}
```

This is a tiny additive method on the slice-2 type, not a new module.

## 3. World struct additions (`FoundryWorld`)

Slice 3 adds the following fields. All default to `None` / empty; the
existing slice-1 + slice-2 scenarios are unaffected.

```rust
pub struct FoundryWorld {
    // ... existing slice-1 + slice-2 fields ...

    // ---- US-02 multi-replica ----
    pub us_02_multi: Option<MultiReplicaHarness>,
    /// Last SocketAddr served by the proxy on the most recent request
    /// (captured from X-Foundry-Replica response header).
    pub us_02_last_request_replica: Option<SocketAddr>,
    /// Counts of per-replica observations across a multi-request
    /// scenario (e.g. the round-robin distribution assertion).
    pub us_02_replica_observations: HashMap<SocketAddr, u64>,

    // ---- US-03 backup-restore ----
    pub us_03_backup_file: Option<PathBuf>,
    pub us_03_restore_target: Option<RestoreTarget>,
    pub us_03_restore_url: Option<String>,
    /// Captured attachment sha256s (filename -> hex digest) at upload
    /// time; the post-restore Then step recomputes and compares.
    pub us_03_attachment_sha256: HashMap<String, String>,
    /// Captured doctor-CLI output for the @us-03-cli scenarios.
    pub us_03_cli_output: Option<assert_cmd::Output>,

    // ---- US-04 rolling-upgrade ----
    pub us_04_migrations_dir: Option<MigrationsDir>,
    /// Per-replica migration outcomes captured at boot.
    pub us_04_migration_outcomes: Vec<MigrationOutcome>,
    /// Boot-start instants for the racing-replicas timing assertion.
    pub us_04_boot_start_instants: Vec<Instant>,

    // ---- US-11 attachments ----
    pub us_11_last_upload_response: Option<reqwest::Response>,
    pub us_11_last_upload_status: Option<reqwest::StatusCode>,
    pub us_11_uploaded_sha256: HashMap<String, String>,
    pub us_11_last_download_bytes: Option<Vec<u8>>,
    pub us_11_last_download_headers: Option<reqwest::header::HeaderMap>,
}

#[derive(Debug, Clone)]
pub enum MigrationOutcome {
    Executed { versions: Vec<String> },
    AlreadyApplied,
    Failed { error_summary: String },
}
```

## 4. Per-scenario isolation — extended to multi-replica

The slice-1 invariant (one container, fresh schema per scenario) extends:

- For US-02: one per-scenario schema is shared across all N replicas of
  that scenario. When the scenario ends, the schema drops and all N
  replicas tear down (their tokio runtimes drop).
- For US-03: each scenario gets a SECOND testcontainers Postgres (the
  restore target) that lives for the scenario only.
- For US-04: each scenario gets its own temp migrations dir; the
  per-scenario schema lives in the shared testcontainers Postgres as
  usual.
- For US-11: standard slice-1 isolation; no special handling.

The `@docker-compose @us-02 @manual-trigger` scenario uses
`support/compose_harness.rs` (slice 1 pattern) with a slice-3-specific
docker-compose.test.yml fixture under
`crates/foundry-acceptance/tests/fixtures/slice3/`.

## 5. Real-I/O budget — slice 3 adds ~14.5s on top of slice 1+2

Per `wave-decisions.md` §Suite-time budget. The line-item breakdown
mirrors the table there; key hotspots:

- US-03 dominated by pg_dump (~300ms) + pg_restore (~1500ms) per
  scenario. The WS is the worst (~2s); siblings amortise via shared
  restore target if needed.
- US-04 dominated by per-scenario 2-replica spawn (~800ms) + the one
  "slow" scenario's intentional 2s migration.
- US-02 dominated by per-scenario 3-replica spawn (~750ms); the SSE
  auto-reconnect scenario adds a 10s ceiling but typically resolves
  in 200-500ms.
- US-11 cheapest; mostly slice-1 multipart with bytea round-trip.
- The `@docker-compose @us-02` scenario costs ~35s and is opt-in only.

Total slice-3 default loop: ~14.5s. Combined slice-1+2+3 suite: ~30-32s,
well within the 60s top-line budget.

## 6. CI invocation (delta only)

Per `wave-decisions.md`. The slice-3 default scenarios pick up
automatically under the same feature-files root. The
`@docker-compose @us-02 @manual-trigger` scenario is opt-in:

```
# Default fast loop (CI default + local):
cargo test -p foundry-acceptance --test acceptance -- \
  --tags "not @docker-compose and not @manual and not @manual-trigger"

# Slice-3 only:
cargo test -p foundry-acceptance --test acceptance -- \
  --tags "@slice3 and not @docker-compose and not @manual-trigger"

# Docker-compose lane (CI matrix opt-in):
cargo test -p foundry-acceptance --test acceptance -- \
  --tags "@docker-compose"
```

## 7. Standing rules carried into DELIVER (additions)

- Every US-02 multi-replica scenario captures the `X-Foundry-Replica`
  header on every response and asserts distribution via
  `us_02_replica_observations`. Tests must NOT assume which replica
  served which request — the round-robin counter is the proxy's
  internal state.
- US-03 scenarios MUST `panic!` at suite startup if `pg_dump` or
  `pg_restore` is not on PATH (`probe_pg_tools_on_path()`). Silent
  skip is forbidden (F-004 anti-flake).
- US-04 scenarios MUST use `MultiReplicaHarness::spawn_concurrent`
  (parallel boot) when asserting the advisory-lock race. Sequential
  boot would not produce the race and the test would be vacuously
  green.
- US-11 multipart scenarios MUST verify the round-trip sha256 against
  the bytes uploaded (not against a precomputed digest). This catches
  client-side encoding bugs that a fixed-digest assertion would miss.
- The proxy adds an `X-Foundry-Replica` response header on every
  response it serves. Production replicas do NOT add this header
  (it's a test affordance); DELIVER must wire it in the proxy only.
- The `@docker-compose @us-02 @manual-trigger` scenario MUST NOT run
  by default; CI matrix selects it via `--tags @docker-compose`.

## 8. Cross-references

- Slice 1 driver: `docs/feature/foundry-backend-mvp/distill/driver.md`
- Slice 2 driver: `docs/feature/foundry-realtime-collab/distill/driver.md`
- Slice 1 + 2 harness code: `crates/foundry-acceptance/src/support/`
- ATDD policy: `docs/architecture/atdd-infrastructure-policy.md`
  (slice 3 will append rows for `pg_dump`, `pg_restore`, the
  multi-replica harness, and multipart upload mechanism per the
  apply-if-exists / write-if-absent contract).
