# DISTILL Driver Design — Slice 1 Acceptance Harness

Owner: acceptance-designer (DISTILL). Companion: `step-skeletons.md` (step signatures), `coverage-matrix.md` (AC→scenario trace). All five `.feature` files in `features/` are driven by the harness specified here.

## 1. Crate location — dedicated `foundry-acceptance` workspace member

**Decision**: a new top-level crate at `crates/foundry-acceptance/` (workspace member; `[lib]` target). NOT an in-crate `tests/` directory inside `foundry-app`.

Justification:

- **Build-graph isolation**: cucumber-rs pulls in async-trait machinery, reqwest, testcontainers, assert_cmd. These are all dev-only dependencies that should never appear in `foundry-app`'s release-build closure. A separate crate makes the boundary structural, not a `[dev-dependencies]` discipline question.
- **Compile time**: `foundry-app` is the largest crate. Putting acceptance tests inside it forces a full `foundry-app` rebuild any time a step definition changes. A separate crate compiles independently.
- **Cargo-deny enforcement**: the dependency-direction rules from `architecture.md` ("`foundry-store` cannot depend on `foundry-auth`") apply to production crates only. A separate test crate is the right place to depend on all of them at once for end-to-end wiring.
- **Mirrors the per-crate `tests/` pattern**: `foundry-core` will have its own `tests/` for property tests (Mandate 9 layer 1-2), `foundry-store` will have its own integration tests against testcontainers Postgres. The acceptance suite is the cross-cutting outer loop; it deserves its own crate.

Layout:

```
crates/foundry-acceptance/
  Cargo.toml                 # depends on: foundry-app, foundry-core, foundry-store,
                             #             cucumber 0.21.x, reqwest, testcontainers,
                             #             assert_cmd (US-01 + US-06 CLI fallback),
                             #             tokio, scraper, serde_json, sqlx
  src/
    lib.rs                   # `pub mod world; pub mod support;` — re-exports for the
                             #  cucumber binary target to consume.
    world.rs                 # FoundryWorld struct (per-scenario state)
    support/
      mod.rs
      spawn_app.rs           # in-process axum server: spawn_app() -> TestApp
      db.rs                  # testcontainers Postgres + per-scenario schema rotation
      fake_smtp.rs           # FakeEmailSender + AppState injection
      fake_clock.rs          # MockClock + recorded sleep durations
      html_assertions.rs     # scraper-based HTML fragment assertions
      cookie_assertions.rs   # parse Set-Cookie header, check HttpOnly/SameSite/Secure
      compose_harness.rs     # docker-compose driver (US-01 only)
  tests/
    acceptance.rs            # cucumber main: `cucumber::run("../../docs/feature/foundry-backend-mvp/distill/features").await`
  steps/                     # per-feature step files (organized below)
    mod.rs
    us_01_install.rs
    us_05_bootstrap.rs
    us_06_signin.rs
    us_07_project_create.rs
    us_08_file_issue.rs
```

The `.feature` files **live in `docs/feature/foundry-backend-mvp/distill/features/`** (single source of truth, version-controlled with the rest of the DISTILL output). The cucumber runner is given that absolute-via-workspace-relative path. Acceptable; cucumber-rs accepts an arbitrary feature-files root.

## 2. cucumber-rs world struct

The `FoundryWorld` carries per-scenario mutable state. Shape:

```rust
// crates/foundry-acceptance/src/world.rs (sketch — DELIVER writes the final form)
#[derive(cucumber::World, Default, Debug)]
pub struct FoundryWorld {
    // Provisioned per-scenario in a Before hook
    pub app: Option<crate::support::TestApp>,            // bound addr + AppState handle
    pub http: reqwest::Client,                            // cookie-jar enabled
    pub db_schema: Option<String>,                        // "test_<uuid>"; dropped in After
    pub fake_smtp: Option<Arc<FakeEmailSender>>,
    pub fake_clock: Option<Arc<MockClock>>,

    // Per-scenario user identity / context
    pub current_user_email: Option<String>,
    pub current_session_cookie: Option<String>,           // last Set-Cookie captured
    pub current_workspace_slug: Option<String>,
    pub current_team_slug: Option<String>,
    pub current_project_slug: Option<String>,
    pub current_project_key: Option<String>,              // e.g. "AUTH"

    // Last response captured by a When step (consumed by Then)
    pub last_response_status: Option<reqwest::StatusCode>,
    pub last_response_body: Option<String>,
    pub last_response_headers: Option<reqwest::header::HeaderMap>,
    pub last_response_elapsed_ms: Option<u128>,

    // Performance scenario aggregation (US-08)
    pub bulk_response_times_ms: Vec<u128>,

    // US-01 only — docker-compose harness state
    pub compose_handle: Option<crate::support::ComposeStack>,
    pub compose_bootstrap_url: Option<String>,
}
```

Rules:

- **State NEVER crosses scenarios.** cucumber-rs reuses the World struct between scenarios; the Before hook resets all fields (and drops DB schema, compose stack, etc.). Per-feature Background steps reseed the per-scenario state in a known order.
- **Cookie jar**: `reqwest::Client::builder().cookie_store(true).build()` so session cookies persist across the multi-step Given/When/Then within a scenario. A new client is created per scenario.
- **No business logic in steps.** Steps call `self.app.unwrap().services.create_issue(...)` (production composition root) or perform a `reqwest::post(...)` against the bound port — never reach into the database or domain types to fabricate state.

## 3. Per-scenario isolation — shared container, per-scenario schema

**Decision**: One Postgres container per `cargo test` invocation, **fresh schema per scenario**.

Comparison:

| Strategy | Setup cost | Isolation | Rejected because |
|---|---|---|---|
| Fresh DB per scenario | ~200ms/scenario (CREATE DATABASE + migrate) | Total | At 25 scenarios = 5s pure overhead; testcontainers `Postgres::default()` is a 2-3s one-shot anyway |
| Truncate tables between scenarios | ~10ms | Fragile with FK CASCADE + outbox + tower_sessions table ordering; easy to miss a new table | FK + cascade order is bug-prone |
| Transactional rollback (BEGIN/ROLLBACK) | ~5ms | None for tower-sessions (manages its own txn); fights sqlx pool semantics | Defeated by tower-sessions's internal commits |
| **Shared container + per-scenario schema** | **~30ms (CREATE SCHEMA + migrate-into-schema + SET search_path)** | **Total — every scenario sees a fresh universe** | **CHOSEN** |

Implementation sketch (see `support/db.rs`):

```rust
// One-shot per process
pub static POSTGRES: tokio::sync::OnceCell<testcontainers::ContainerAsync<...>> = OnceCell::const_new();

// Per scenario
pub async fn fresh_schema() -> (String, sqlx::PgPool) {
    let schema = format!("test_{}", uuid::Uuid::new_v4().simple());
    let pool = build_pool_with_search_path(&schema).await;
    sqlx::query(&format!("CREATE SCHEMA {}", schema)).execute(&pool).await?;
    sqlx::migrate!("../foundry-store/migrations").run(&pool).await?;
    // tower-sessions's own migrator also runs here
    (schema, pool)
}

pub async fn drop_schema(schema: &str, pool: &sqlx::PgPool) {
    sqlx::query(&format!("DROP SCHEMA {} CASCADE", schema)).execute(pool).await.ok();
}
```

The schema name lives on `FoundryWorld.db_schema`; the After hook drops it.

**Caveat acknowledged**: `tower_sessions_sqlx_store` writes to a `tower_sessions` table. We run its migrator into the per-scenario schema as well, so the table is created inside the schema (not in `public`). Verified compatible with tower-sessions 0.13.x via its `Schema` configuration knob.

## 4. Postgres provisioning — `testcontainers-rs` (rejected `docker compose --profile test` and external PG)

**Decision**: `testcontainers-rs` (0.20+) with the `postgres` module. One shared `postgres:16-alpine` container per `cargo test`.

Alternatives considered and rejected:

- **`docker compose --profile test`**: requires Docker AND a docker-compose file AND a separate `cargo test` invocation order. Forces every contributor to remember to start the stack manually OR forces `cargo test` to shell out to compose. testcontainers-rs gives the same result with zero ceremony for the contributor.
- **Assume external PG (CI provides DATABASE_URL)**: tempting for CI speed but breaks NFR-DEV-01 (cold-start dev env ≤10 minutes). A contributor running `cargo test` for the first time would get a confusing connection error instead of an automatic Postgres.
- **`sqlx-cli` against a single shared DB**: no isolation; non-starter.

testcontainers-rs cost: ~3s container startup, amortised across the entire suite. Per-scenario cost: ~30ms (schema rotation, above).

## 5. Step definition organisation — one file per feature, shared types in `support/`

**Decision**: one `steps_us_XX_<topic>.rs` file per feature file. Reuse via shared support modules, not via cross-feature step pools.

Rationale:

- cucumber-rs treats step phrases as globally unique; if two features wanted "Given Mei is signed in" with different parameters, they would collide. One-file-per-feature avoids the collision.
- Vocabulary that IS shared (login the test user, create a workspace, parse Set-Cookie) lives as plain Rust functions in `support/` — invoked from any step body. This is the cucumber-rs idiom; it preserves the "step text is the contract" property without pooling steps.

The Pillar 2 chained-narrative rule still applies WITHIN each feature file (the `Given` of scenario N reuses the step-method bodies from N-1's `Given+When`). Cross-file reuse goes through `support/`.

## 6. CI + local invocation

**Local developer loop**:
```bash
cargo test -p foundry-acceptance --test acceptance        # fast path: skips @docker-compose and @manual
cargo test -p foundry-acceptance --test acceptance -- --tags "@us-05 or @us-06 or @us-07 or @us-08"
cargo test -p foundry-acceptance --test acceptance -- --tags "@us-01"  # the slow docker-compose set
```

Tag-filter is forwarded to cucumber-rs's built-in tag expression parser. Default test invocation excludes `@manual` and `@docker-compose` to keep the inner loop fast.

**CI pipeline** (suggested):

1. Stage A — `cargo test --workspace --exclude foundry-acceptance` (unit + integration). ~30s.
2. Stage B — `cargo test -p foundry-acceptance --test acceptance` with `@docker-compose` and `@manual` excluded. ~60s target (see §7).
3. Stage C — `cargo test -p foundry-acceptance --test acceptance -- --tags "@us-01 and not @manual"` runs the docker-compose scenarios. ~180s. Sharded separately so it can run in parallel with Stage B.
4. Stage D (post-merge or on-tag) — manual scenarios surface to a human reviewer via the GitHub Action artifact `manual-uat-checklist.md`.

Stages A+B form the fast loop. Stage C is the slow but bounded compose loop. Stage D is the human-in-the-loop track for `@manual`.

## 7. Time budget — Slice 1 suite ≤60s on a developer laptop

Target: `cargo test -p foundry-acceptance --test acceptance -- --tags "not @docker-compose and not @manual"` completes in ≤60 seconds on:

- MacBook Pro M2/M3 (or equivalent), Docker Desktop running, `postgres:16-alpine` already pulled.
- Tests run with default cucumber-rs concurrency (uses tokio runtime).

Budget breakdown (assumptions):

| Phase | Cost | Assumption |
|---|---|---|
| `testcontainers` Postgres startup (one-shot) | 3.0s | `postgres:16-alpine` pre-pulled |
| sqlx migration into `public` (one-shot baseline; per-schema repeats incremental) | 0.2s + 0.1s × N schemas | 9 tables + tower-sessions = 10 statements |
| Per scenario fixed cost (schema create + migrate + AppState init + bind port) | ~80ms | ~30ms schema + 30ms migrate + 20ms app spawn |
| Per scenario logic (HTTP roundtrips, assertions) | 20–100ms | Mostly DB roundtrip + reqwest call |
| Scenario count (excluding @docker-compose and @manual) | ~22 scenarios | See coverage-matrix.md |
| Performance scenario (US-08, 100 sequential creates) | ~3-5s | 100 × ~30ms |
| Sub-total | 3.0 + 22 × 0.15 + 5 = **~11.3s sequential** | |
| With cucumber-rs scenario concurrency = 4 | **~5-8s plus startup** | If we don't enable concurrency, ~12s |

**Conservative budget**: 30s sequential, 15s with concurrency. The 60s target is generous; the real risk is testcontainers cold-pull (≥10s on first run) which the developer pays once.

Risks to the budget:

1. Per-scenario schema rotation is 30ms only if the testcontainers DB is already warm. If we re-create the container per scenario, blow the budget — DO NOT.
2. `@nfr-perf-01` runs 100 sequential creates (~3-5s); cannot parallelise within the scenario. Acceptable.
3. Cucumber-rs scenario concurrency requires `FoundryWorld` to be `Send + Sync` and the AppState to be independently bound per scenario. We pre-allocated `127.0.0.1:0` per scenario specifically for this; should hold.

If we discover a real slowdown in DELIVER, the escape hatches (in order):

1. Enable cucumber-rs `--concurrency N` flag.
2. Pre-build the migration cache via `sqlx prepare`.
3. Share the migrated `public` schema and per-scenario use `SET search_path` to a copy-schema (Postgres 16 supports `CREATE SCHEMA ... LIKE` via templating).
4. Demote the perf scenario to `@nfr @manual` if 100-sample P95 is too slow for the inner loop (run it nightly).

## 8. Standing rules carried into DELIVER

- Every step body uses production composition (call `foundry_app::test_support::spawn_app(pool, fake_smtp, fake_clock)` then `reqwest::post(...)`). No step body should construct `Issue`, `User`, etc. directly — Pillar 3.
- The cookie jar enforces that "logged in as Mei" is a fact of the World, not re-asserted each scenario — Pillar 2.
- No Gherkin step mentions `sqlx`, `axum`, `tower-sessions`, HTTP status codes by number except where they ARE the observable contract (the few `4xx` / `5xx` lines in `.feature` files) — Pillar 1. The 400/403/409/410 numbers in the `.feature` files are user-facing contracts (URL bookmarkable error pages, HTTP-spec-compliant). They are not implementation details.
