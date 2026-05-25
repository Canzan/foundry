# Wave Decisions — foundry-operator-grade (Slice 3)

DISTILL-wave decisions that gate DELIVER. Established 2026-05-24
during the slice-3 DISTILL pass. No prior `wave-decisions.md` existed
for `foundry-backend-mvp` or `foundry-operator-grade`; this file is the
first decision-of-record for slice 3. Slice 2's
`docs/feature/foundry-realtime-collab/distill/wave-decisions.md` is the
strategy precedent and is mirrored here.

## Walking Skeleton Strategy: Strategy C — all real adapters (inherited)

For every scenario tagged `@walking_skeleton`, the test exercises
production driving adapters and real driven adapters. There are NO
`@in-memory` fixtures, NO mock substitutes, and NO fake repositories
for the slice-3 walking skeletons. Slice-3 adds new driving adapters
(round-robin proxy, system `pg_dump`/`pg_restore` subprocesses, the
`foundry doctor backup-verify` CLI subcommand, multipart uploads); each
walking skeleton exercises one of them end-to-end.

- **Driving adapters added in slice 3**:
  - Round-robin reverse proxy in `support/round_robin_proxy.rs`
    (in-process axum-based, ~40-50 LoC) routing reqwest calls to
    N replicas. Used by US-02 Option A (default).
  - `std::process::Command` for `pg_dump`, `pg_restore`, and
    `foundry doctor backup-verify` (US-03 driving adapters; the CLI
    subcommand verifies via `assert_cmd::Command::cargo_bin`).
  - `reqwest::multipart::Form` for US-11 attachment uploads.
- **Driven adapters** (Strategy C contract): real testcontainers
  Postgres 16, real `sqlx::PgPool`, real `tower-sessions-sqlx-store`,
  real `PgListener`, real bytea round-trip, real `pg_advisory_lock`.

## US-02 approach decision: Option A (in-process proxy) + 1 × Option B

| Option | Status | Rationale |
|---|---|---|
| **A — in-process round-robin proxy + N spawned axum replicas** | **CHOSEN as default** for 6 of 7 US-02 scenarios | ~50ms overhead per scenario; reuses `spawn_app` from slice 1 multiplied N times; exercises the real Postgres session-shared invariant; cheap enough to keep in the fast loop. |
| **B — docker-compose `--scale foundry=3` + real Caddy** | **CHOSEN for 1 scenario** tagged `@docker-compose @us-02 @manual-trigger` | Production-shaped sanity check that the Caddy round-robin + replica scaling actually behaves as the in-process proxy claims. Costs ~30s extra; runs in CI only on tag selection (precedent: US-01's `@docker-compose` scenario). |
| C — kind/k3d K8s cluster | **DEFERRED** to a future `@k8s` slice | Adds ~60s startup + a kind/k3d toolchain prereq for every contributor; the K8s YAMLs are reviewed-by-eye and routed via the docker-compose contract. Revisit if/when K8s ships as a supported deploy variant. |

The in-process proxy is purpose-built (not `caddy` in-process or
`hyper-reverse-proxy`): we want zero new transitive deps in the test
crate and full control over which replica is "currently routed to" for
the SSE-landing-replica assertion. The proxy is ~50 lines of axum +
`AtomicUsize` round-robin counter + health-check polling of `/readyz`.

## US-04 approach decision: Option B (test-only migration into per-scenario temp dir)

We do NOT cargo-feature-gate two binaries (Option A). Instead, the
test harness writes a per-scenario migration file
`0099_us04_test_<scenario>.sql` into a `tmp_path` migrations directory
and the per-scenario AppState's migrator points at that directory. The
real `sqlx::migrate::Migrator` + `pg_advisory_lock` code runs without
modification.

**Property explicitly NOT tested**: "old replica keeps serving old SQL
during migration." That property is enforced by the expand-only
migration discipline (per-migration header comments + code review per
NFR-MIG-02). Black-box testing would require two cargo-feature-gated
binaries with different schema knowledge, which the team rejected as
test-only complexity that does not earn its keep. See
`features/us-04-rolling-upgrade.feature` § "Out of scope".

## US-03 system-tool prereq

`pg_dump` and `pg_restore` must be on PATH for `@us-03 @real-io`
scenarios. The harness probes for both at suite startup; missing
binaries produce a clear `panic!("contributor missing pg_dump; install
postgresql-client")` rather than silent skip (F-004 anti-flake). This
prereq is documented in `CONTRIBUTING.md` (DELIVER must update). On
macOS via Homebrew: `brew install libpq && brew link --force libpq`
(the Postgres client binaries package). On Ubuntu: `apt-get install
postgresql-client-16`. CI base image already includes them.

## US-11 multipart prereq

`reqwest` is already a workspace dep but the slice-1 entry does not
enable the `multipart` feature. DELIVER must add it in
`crates/foundry-acceptance/Cargo.toml` (and any other crate that needs
client-side multipart). The server-side multipart extraction is real
`axum::extract::Multipart` from the production crate.

## Scenarios per file

| File | Scenarios | WS scenarios | Error / NFR scenarios | Approach |
|---|---:|---:|---:|---|
| `us-02-multi-replica.feature` | 7 | 1 | 3 (NFR-AVAIL-02, NFR-OBS-02, NFR-PERF-04) | A + 1×B |
| `us-03-backup-restore.feature` | 6 | 1 | 1 (`@error` CLI failure) | C (testcontainers + system pg_dump) |
| `us-04-rolling-upgrade.feature` | 4 | 1 | 1 (`@error` failed migration) | B (test-only migration file) |
| `us-11-attachments.feature` | 7 | 1 | 4 (oversize, non-member ×2, anon) | C (multipart) |
| **TOTAL** | **24** | **4** | **9** | — |

Error / NFR ratio: 9 / 24 = 38%. Within the 40% target band; the
walking-skeletons are intentionally happy-path so the overall ratio is
slightly below the band. Slice 2 ran at ~33%; this is comparable.

## Tag conventions (additions only)

Inherited from slice 1 + slice 2: `@slice1`, `@slice2`, `@walking_skeleton`,
`@real-io`, `@driving_port`, `@driving_adapter`, `@error`, `@in-memory`,
`@manual`, `@docker-compose`, `@nfr-perf-*`, `@nfr-sec-*`, `@nfr-avail-*`,
`@nfr-obs-*`, `@us-NN`.

Added in slice 3:

- `@slice3` — every scenario in this slice.
- `@multi-replica`, `@backup-restore`, `@rolling-upgrade`, `@attachments`
  — per-story tags for selective runs.
- `@nfr-mig-01`, `@nfr-mig-02`, `@nfr-data-01`, `@nfr-perf-02`,
  `@nfr-perf-04`, `@nfr-avail-02`, `@nfr-avail-03` — NFR mappings.
- `@us-03-cli` — narrows the US-03 CLI driving-adapter scenarios.
- `@manual-trigger` — opt-in tag for the docker-compose multi-replica
  scenario; not in the default fast loop, runs on explicit selection
  or CI matrix opt-in.

## CI invocation (delta only)

Slice-3 fast loop:
```
cargo test -p foundry-acceptance --test acceptance -- \
  --tags "@slice3 and not @docker-compose and not @manual-trigger"
```
Slice-3 docker-compose lane (CI matrix opt-in):
```
cargo test -p foundry-acceptance --test acceptance -- \
  --tags "@docker-compose and @us-02"
```

## Suite-time budget

| Bucket | Estimate | Notes |
|---|---:|---|
| US-02 default loop (6 scenarios × 3-replica spawn) | ~3.0s | ~250ms × 6 + ~250ms × 3 for replica boot overhead amortised; proxy adds ~5ms/req |
| US-03 default loop (6 scenarios incl. pg_dump/restore) | ~6.0s | pg_dump on tiny DB ~300ms; pg_restore on fresh container ~1500ms × 1 walking skeleton + cached dumps for siblings |
| US-04 default loop (4 scenarios) | ~4.0s | 2-replica concurrent boot ~800ms × 4; the "slow" scenario adds 2s deliberately |
| US-11 default loop (7 scenarios) | ~1.5s | multipart + bytea round-trip ~200ms each |
| **Subtotal — slice 3 default loop** | **~14.5s** | within the "≤+20s on top of current ~30s" budget |
| **Suite total (slice 1 + slice 2 + slice 3 default)** | **~30-32s** | comfortably within the 60s top-line budget |
| @docker-compose @us-02 (opt-in) | ~35s | docker compose up of 5 containers + 6 round-trip requests |

## Open Decisions for DELIVER

| Decision | Status | Owner |
|----------|--------|-------|
| Per-replica spawn signature: `spawn_replica(state)` vs `spawn_app_with_listener` reused N times | RESOLVED in this DISTILL — extend the existing `spawn_app_with_listener` (already takes state) and let `support/round_robin_proxy.rs` keep N `TestApp`s in a `Vec` | DELIVER (no API change to `spawn_app_with_listener`) |
| Round-robin proxy crate vs hand-roll | RESOLVED — hand-roll ~50 LoC axum binary; zero new deps | DELIVER |
| US-03 second testcontainers Postgres reuse strategy (one per scenario vs one per file) | OPEN — propose "one per scenario" for safety; revisit if pg_restore startup dominates the slice-3 budget | DELIVER |
| US-11 content-type sniffing crate (`infer` vs `mime_guess`) | OPEN — `infer` reads magic bytes, `mime_guess` looks at filename; production likely wants both. Acceptance asserts the recorded content-type matches what the upload sent, not the sniff per se | DELIVER |
| `foundry doctor backup-verify` impl scope | OPEN — slice-3 acceptance assumes the subcommand exists; if not present in slice 1+2, DELIVER must scaffold it. The acceptance scenarios pin the contract (`exit 0`, structured `row-counts:` output, `status: OK`) | DELIVER |
| US-02 "replica is stopped" mechanism in Option A | OPEN — propose drop the `TestApp` from the proxy's pool to simulate; alternative: signal the spawned axum task. DELIVER picks whichever yields the cleaner shutdown semantics. | DELIVER |

## DELIVER Pre-flight Checklist

DELIVER must satisfy these before merging:

- [ ] `crates/foundry-acceptance/src/support/round_robin_proxy.rs` exists
      and exposes a `spawn_round_robin_proxy(replicas: Vec<TestApp>) ->
      ProxyHandle` API.
- [ ] `crates/foundry-acceptance/src/support/multi_replica_harness.rs`
      exists and exposes a `MultiReplicaHarness::spawn(n: usize) -> Self`
      builder that returns N `TestApp`s sharing one per-scenario Postgres
      schema + a round-robin proxy in front.
- [ ] `crates/foundry-acceptance/src/support/pg_backup.rs` exists and
      exposes `dump_to_file(pg_url, path)` and `restore_from_file(pg_url,
      path)` helpers shelling out to system `pg_dump` / `pg_restore`.
- [ ] `crates/foundry-acceptance/src/support/test_migration.rs` exists
      and exposes a `stage_test_migration(temp_dir, version, sql) ->
      MigrationsDir` helper used by US-04 scenarios.
- [ ] `crates/foundry-acceptance/Cargo.toml` enables reqwest's `multipart`
      feature.
- [ ] All 4 walking skeletons (`us-02` line 53, `us-03` line 47, `us-04`
      line 48, `us-11` line 54) execute against real Postgres + real axum
      + real I/O.
- [ ] The `foundry doctor backup-verify` subcommand exists with the
      output contract pinned by US-03's `@us-03-cli` scenarios.
- [ ] No scenario regresses slice 1+2's 55/55 green state.
- [ ] System `pg_dump` / `pg_restore` prereq documented in
      `CONTRIBUTING.md` and probed at suite startup with a clear panic
      message on absence.
- [ ] `@docker-compose @us-02` scenario is wired through `compose_harness`
      pattern from US-01 + a slice-3 Caddyfile fixture under
      `crates/foundry-acceptance/tests/fixtures/`.
- [ ] Slice-3 default fast loop runs in ≤+20s on top of slice 1+2.

## Gate-review remediation (2026-05-24)

The first DISTILL pass was REJECTED_PENDING_REVISIONS by the
acceptance-designer reviewer with two blockers; both are now resolved.

### Blocker 1: CM-B (business-language purity in Gherkin) — RESOLVED

All four `.feature` files refactored to remove implementation-tool
names (`pg_dump`, `pg_restore`, `Postgres`, `bytea`, `multipart`,
`sha256`) from Given/When/Then steps. Substitutions:

- `pg_dump` / `pg_restore` → "back up" / "restore" (the verbs the
  operator actually thinks in)
- `Postgres` → "the database" (the operator's mental model; the
  specific DB engine is implementation choice)
- `bytea` → "the database" (storage is implementation; the user-
  observable property is "attachments survive backup")
- `multipart` → "upload" (multipart is HOW; upload is WHAT)
- `sha256` → "byte-identical" (the user-observable property)

What we kept on purpose:

- HTTP status numbers in `@error` scenarios where the rejection
  status IS the user-facing contract (the form-submitter sees a
  413/401/403; the test asserts on that observable).
- `/readyz`, `SIGTERM` in US-02 — the OPERATOR is the user for that
  story, and operators literally watch `/readyz` and send SIGTERM.
  Removing those would obscure the contract under test.
- The `foundry doctor backup-verify` subcommand name + its CLI
  output contract in US-03 — the CLI subcommand IS the
  operator-facing contract under test.

Implementation tools remain documented in each feature's leading
comment block (for contributor context) and in `driver.md` +
`step-skeletons.md` (where step bodies invoke them). The Gherkin
itself reads like a stakeholder document.

### Blocker 2: US-04 walking-skeleton framing — RESOLVED

The original WS scenario title — "Two replicas race to apply a new
migration; the advisory lock serialises them and the migration is
applied exactly once" — described the implementation mechanism
(advisory-lock race), not the user goal. Rewritten as:

> An operator deploys a new version and two replicas start in
> parallel; the schema update applies exactly once and both replicas
> come up healthy

The new title pins what the operator cares about (one schema update,
all replicas healthy after deploy). The advisory-lock mechanism is
still tested via the Then assertions and the per-scenario Background,
but it's no longer named in the scenario title.

### Open Decisions reviewer affirmed

All three OPEN items in the table above remain OPEN for DELIVER:

1. `foundry doctor backup-verify` scaffolded in DELIVER per US-03's
   pinned contract. Reviewer: "Scaffold now if not present; contract
   is well-specified by acceptance tests."
2. "DB unreachable" simulation: proceed with the `cfg(test, feature
   = "test-hooks")` health-injection flag from the project ATDD
   policy. Reviewer: "Health-injection preserves production purity."
3. pg_restore target reuse: default to fresh-per-scenario; profile
   in DELIVER and amortise per-file via `lazy_static` if pg_restore
   startup dominates the slice-3 budget.

