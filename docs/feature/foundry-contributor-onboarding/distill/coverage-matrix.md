# Coverage Matrix — Slice 4 / US-13 Contributor Onboarding

## Scenarios at a glance

| # | Scenario | Tier | Layer | Tags | Automated? |
|---|---|---|---|---|---|
| 1 | The contributor's test command succeeds end-to-end with no pre-existing services | A | 5 (subprocess + real I/O) | `@walking_skeleton @real-io @driving_port @us-13` | YES |
| 2 | The README's Quickstart section walks from clone to green tests in five named commands | A | 3 (file inspection) | `@real-io @driving_port @us-13 @readme-contract` | YES |
| 3 | The README states the minimum supported Rust version and the project pins it | A | 3 (file inspection) | `@real-io @driving_port @us-13 @readme-contract` | YES |
| 4 | The README documents how a contributor sees a code change without rebuilding manually | A | 3 (file inspection) | `@real-io @driving_port @us-13 @readme-contract @hot-reload` | YES |
| 5 | A new contributor reaches green tests within ten minutes on a fresh laptop | — | manual drill | `@manual @us-13 @demo @nfr-onboarding-10min` | NO (manual drill) |
| 6 | A contributor sees their one-line change reflected in the running app | — | manual drill | `@manual @us-13 @demo` | NO (manual drill) |

Total: **6 scenarios** (4 automated, 2 `@manual`). Error/edge ratio:
the third scenario (MSRV pin) is a contract-enforcement scenario that
prevents a regression class (rust-toolchain.toml drifting to a
generic `stable` channel). The toolchain-error case from US-13 §
"Domain Examples #3" is covered structurally by the third scenario —
see ADR below.

## AC × scenario trace

US-13's `Acceptance Criteria` list (stories.md lines 1486-1492):

| AC | Pinned by scenario(s) | Notes |
|---|---|---|
| README "Quickstart" section walks from clone to green tests in 5 commands | #2 (Quickstart structural validation) | The "5 commands" count is asserted by counting fenced-block commands in the Quickstart section. |
| No Redis, no S3, no Node toolchain required for the dev loop | #1 (walking skeleton runs with only Docker + cargo on PATH) + #2 (Quickstart prereqs name only Rust + Docker) | Combined: scenario #2 pins the *promise* in the README; #1 pins the *reality* of the test suite. |
| Minimum Rust version pinned in `rust-toolchain.toml` and called out in README | #3 (cross-check README mention against rust-toolchain.toml pin) | This scenario locks the contract; today (slice-3 reality) `rust-toolchain.toml` pins `"stable"`, which fails the third Then. DELIVER changes it to a specific version. |
| Hot-reload path documented (`cargo watch -x run`) | #4 (watch-and-rebuild command + localhost URL documented) | Decoupled from the slice-3 dev-server reality (any watch incantation that recompiles + serves passes). |
| CI pipeline reproduces the same quickstart end-to-end on each PR | #1 (the WS subprocess scenario runs in the default CI loop and exercises the documented test command) | The WS scenario *is* the CI reproduction of the contributor's first command. |

## UAT scenario × DISTILL scenario trace

The 5 UAT scenarios from stories.md lines 1454-1483:

| UAT scenario | Pinned by DISTILL scenario(s) | Treatment |
|---|---|---|
| New contributor reaches green tests in ≤10 minutes | #5 (`@manual` drill A) | manual drill, target ≤10 min, drill script in `manual-drills.md` |
| Quickstart commands are documented and exhaustive | #2 (automated file inspection) | automated, fast (~1ms) |
| Visible change after one-line edit | #6 (`@manual` drill B) | manual drill, target ≤30s recompile-and-see |
| Outdated Rust toolchain produces an actionable error | #3 (automated file inspection of `rust-toolchain.toml` pin) | ADR: file-inspection over real-cargo invocation — see Open Questions |
| Integration tests pass against ephemeral Postgres | #1 (automated subprocess walking skeleton) | automated, ~25-35s |

## Adapter coverage (Mandate 6)

Driven adapters NEW in slice 4: none. The slice reuses existing
infrastructure (testcontainers + the cucumber runner). The only new
driving surfaces are `README.md` + `rust-toolchain.toml` reads — see
the inventory below.

| Adapter | `@real-io` scenario | Covered by |
|---|---|---|
| `std::fs::read_to_string(README.md)` (new — `support/readme_inspect.rs::read_readme`) | YES | #2, #3, #4 (all three file-inspection scenarios) |
| `std::fs::read_to_string(rust-toolchain.toml)` (new — `support/readme_inspect.rs::read_rust_toolchain`) | YES | #3 |
| `std::process::Command::new("cargo")` for nested test invocation (new — inlined in step body, no new helper module) | YES | #1 (walking skeleton) |
| testcontainers `Postgres` container | YES — INHERITED | #1's nested cargo invocation runs the slice 1+2+3 walking skeletons against testcontainers |
| `assert_cmd::Command::cargo_bin("foundry")` CLI subprocess | n/a | not exercised in slice 4 |

Zero "NO — MISSING" rows.

## Driving adapter coverage (RCA-fix P1)

| Driving adapter | Mapped scenario | Protocol |
|---|---|---|
| `README.md` (the contributor's literal text source) | #2, #3, #4 | file read + structural search |
| `rust-toolchain.toml` (the toolchain manager's input) | #3 | file read + TOML parse |
| `cargo test -p foundry-acceptance` (the contributor's literal first command) | #1 | subprocess invocation with stripped env |

Each surface has at least one scenario that exercises it via its
production protocol. Mandate satisfied.

## Mandate compliance preview

| ID | Mandate | Status |
|---|---|---|
| CM-A | Hexagonal boundary — tests enter through driving ports only | ✅ — file reads + subprocess; no internal-component imports |
| CM-B | Business language purity in Gherkin | ✅ — "contributor", "Quickstart section", "test command exits successfully"; `cargo test` survives as a contract-naming token (matches operator-grade slice's `pg_dump` precedent) |
| CM-C | User journey completeness | ✅ — every scenario has user trigger + observable outcome; the manual drills cover the full demo journey |
| CM-D | Pure function extraction | ✅ — `readme_inspect` helpers are pure; only `std::fs::read_to_string` is impure and isolated |
| CM-E | Universe-bound assertion at layers 1-3 | n/a — all automated scenarios are at layer 3+ (file I/O or subprocess); per Mandate 8 layer 4+ may use traditional assertions; per Mandate 11 sad paths are example-based |
| CM-F | Layer-dependent PBT mode | ✅ — no PBT (layer 3+ → example-only per Mandate 9) |
| CM-G | Tier B state machine | n/a — feature is config-shaped; no journey of ≥3 chained scenarios; per Mandate 10 Tier B skipped |
| CM-H | Sad paths example-based | ✅ — toolchain-error pinned via one example (scenario #3) |

## Open questions for DELIVER

1. **Toolchain-error scenario approach** — file-inspection vs real-cargo
   invocation. DISTILL chose file-inspection (see `wave-decisions.md` §
   "ADR-04 — file-inspection over real-cargo invocation"). If DELIVER
   later wants a real-cargo lane, the right home is a new
   `@docker-compose @us-13 @manual-trigger` scenario that runs the
   build in a `rust:1.75-slim` container — costs ~30 s and adds a new
   docker-compose harness. DISTILL deferred this; the contributor
   experience is adequately pinned by the structural contract.

2. **Walking-skeleton scenario placement in the fast loop vs `@manual-trigger`**
   — depends on the measured suite-time delta. DELIVER should profile
   one run and decide; default is keep it in the fast loop (it is the
   only proof that the contributor's first command works out of the
   box).

3. **`http://localhost:3000` vs the actual production bind** — slice 1's
   `foundry-app` binds to whatever `FOUNDRY_PORT` resolves to (or a
   default). The fourth scenario's `find_local_app_url` helper accepts
   any `http://localhost:*` URL; DELIVER pins the specific port in the
   README to match the production default.
