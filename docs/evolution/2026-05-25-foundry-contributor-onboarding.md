# Evolution — foundry-contributor-onboarding (Slice 4 / US-13)

**Finalized**: 2026-05-25
**Ship commit**: [bf35a68](../../) — "Slice 4: US-13 contributor onboarding — MVP feature-complete (13/13)"
**Wave coverage**: DISTILL → DELIVER (DISCOVER/DISCUSS/DIVERGE/DESIGN inherited from slices 1–3)

## Feature summary

US-13 closes the Foundry MVP by pinning the contributor onboarding
contract: a Rust developer arriving at the repo for the first time
can run the README's Quickstart from `git clone` to a green
`cargo test -p foundry-acceptance` run in five commands, with no
Redis, no S3, no Node toolchain, and a clear toolchain-version
error if their Rust install is too old.

The feature is contract-shaped, not behaviour-shaped: most of the
value already half-existed in slices 1–3 (testcontainers Postgres,
the cucumber-rs runner, the workspace `Cargo.toml`). Slice 4
finished the contract and pinned it in acceptance tests so it
cannot silently drift.

## Business context

Final MVP slice. Closes JTBD outcome-3 ("Minimize contributor
time-to-meaningful-change") and unblocks the v0.1 release-candidate
cut. Before this slice the README had a working `cargo test` invocation
but no unified Quickstart heading, the `rust-toolchain.toml` pinned a
generic `"stable"` channel (no MSRV contract), and there was no CI
job that exercised the literal contributor-first-command sequence.

## Key decisions

### From DISTILL (`distill/wave-decisions.md`)

- **Strategy C inherited** — every scenario exercises real driving
  adapters (file reads + a `cargo` subprocess). No mocks; the
  contributor's literal first interactions ARE the test surface.
- **Tier A only (Mandate 10)** — 4 automated scenarios are non-chained
  and have a fixed input space; Tier B state-machine PBT is not warranted.
- **Example-only PBT mode (Mandate 9)** — all 4 automated scenarios run
  at layer 3+ (file I/O or subprocess); sad path is one named example.
- **ADR-04 — file-inspection over real-cargo for toolchain error.**
  Chose to assert that `rust-toolchain.toml` pins a specific version
  and README MSRV agrees, rather than spin up a `rust:1.75-slim`
  Docker isolation. ~1 ms vs ~30 s, and pins the upstream contract
  rustup itself obeys.
- **ADR-05 — walking-skeleton in default loop, with `@manual-trigger`
  fallback.** Keeping the literal contributor command in the fast loop
  preserves the contract; if measured cost broke the 60 s top-line,
  the one-tag fallback was pre-authorized.
- **ADR-06/07 — `@manual` drills carry the human-experience contracts.**
  The "≤10 min to green tests" and "visible change after one-line edit"
  promises are process metrics, not software contracts. Drill scripts
  live in `manual-drills.md`; `@manual` step bodies `unreachable!()` so
  a misconfigured CI invocation surfaces loudly.

### From DELIVER (extracted from `bf35a68` commit body)

- **MSRV pin 1.91 (not the DISTILL-suggested 1.85).** Locked dep
  graph (cookie_store, darling, icu_*, serde_with, time) transitively
  requires rustc ≥ 1.88; 1.91 matches what `dtolnay/rust-toolchain@stable`
  was already resolving in CI. Documenting the *resolved* version
  beats documenting the *minimum* version on a project that always
  ships against the current channel.
- **Workspace `rust-version = "1.88"`, toolchain channel = "1.91".**
  Two different numbers, both correct: 1.88 is the true MSRV from
  the lockfile; 1.91 is the development channel everyone's CI was
  already on. Intentional discrepancy; flagged as v0.1 RC open item
  in case dep-audit tools confuse the two.
- **Walking-skeleton uses `cargo build -p foundry-core --release`
  (NOT the DISTILL-planned nested `cargo test -p foundry-acceptance`).**
  Self-recursion risk: a nested `cargo test` of the same package would
  re-invoke the WS scenario indefinitely. The build of a pure crate
  proves "Foundry compiles for the contributor" — the contract under
  test — in 1–3 s with zero recursion exposure.
- **New `quickstart-verify` CI job.** Runs the README's literal 5-command
  sequence on a fresh ubuntu-latest runner on every PR. This is the
  CI reproduction the AC names — independent of the in-repo acceptance
  suite, so README drift is caught even if the helper functions
  evolve.

## Steps completed

No `deliver/execution-log.json` was emitted (DELIVER ran outside the
nWave execute orchestrator). The single ship commit `bf35a68` is the
audit trail; its body enumerates the delivered artifacts:

- `README.md` — Prerequisites + 5-command Quickstart + expected output
  snippet + Run-the-app-locally + Hot-reload subsections
- `rust-toolchain.toml` — `channel = "1.91"`
- `Cargo.toml` — `[workspace.package].rust-version = "1.88"`
- `CONTRIBUTING.md` — Quickstart-on-a-fresh-machine + Manual-onboarding-drills sections
- `.github/workflows/ci.yml` — new `quickstart-verify` job
- `crates/foundry-acceptance/src/support/readme_inspect.rs` — 6 pure helpers
- `crates/foundry-acceptance/src/steps/us_13_contributor_onboarding.rs` — 4 automated scenario bodies + 2 `@manual` `unreachable!()` stubs
- `crates/foundry-acceptance/tests/features/us-13-contributor-onboarding.feature` — copied from `distill/features/`
- Force-link in `tests/acceptance.rs`
- 3 new `FoundryWorld` fields (`us_13_readme_text`, `us_13_rust_toolchain_text`, `us_13_self_test_outcome`)

## All 5 ACs satisfied (verified at `bf35a68`)

- [x] README Quickstart walks clone→green in 5 commands
- [x] No Redis / S3 / Node deps (grep-clean across every `Cargo.toml`)
- [x] `rust-toolchain.toml` pins `channel = "1.91"`; README names "Rust 1.91"
- [x] Hot-reload path documented (`cargo install cargo-watch`, then `cargo watch -x 'run --bin foundry'`)
- [x] CI reproduces the same Quickstart end-to-end on each PR via `quickstart-verify`

## Verification at HEAD

- `cargo xtask ci` → all gates green
- 79 default-loop scenarios pass in 22.6 s (well under the 60 s budget — ADR-05 fallback not triggered)
- 82 scenarios total with `FOUNDRY_ACCEPTANCE_TAGS=all`
- `cargo clippy --all-targets --release -- -D warnings` clean
- `cargo fmt --all -- --check` clean
- `cargo deny check` clean

## Lessons learned

1. **Watch for self-recursion in walking-skeleton subprocess scenarios.**
   The DISTILL plan called for nested `cargo test -p foundry-acceptance
   -- --tags "@walking_skeleton and not @us-13"`. The `not @us-13` filter
   was supposed to prevent recursion, but DELIVER recognized that any
   future tag drift could re-introduce the loop. `cargo build` of a
   different crate is structurally recursion-safe and proves the same
   contract — faster too (1–3 s vs 25–35 s).
2. **MSRV-pin discussions need 3 numbers, not 1.** The DISTILL doc
   talked about "the MSRV"; reality required the development-channel
   pin (toolchain.toml), the true minimum (Cargo workspace), and the
   contributor-facing number (README) to all be chosen separately and
   agree where they should. Future toolchain-pin work should
   pre-distinguish these.
3. **CI quickstart-verify is the load-bearing AC test, not the in-repo
   scenario.** The in-repo `us-13` feature pins the README's
   *structure* (it has a Quickstart, it lists 5 commands). The CI
   `quickstart-verify` job pins the README's *correctness* (those 5
   commands actually run on a clean machine). Both are needed.
4. **Manual drills earn their tag.** Automating Drill A (time-to-green)
   would measure CI cache state, not the contributor's pause-to-read
   tempo. Automating Drill B would need a browser harness for a property
   a human verifies in a glance. The `@manual` classification preserved
   the signal these drills actually carry.

## Issues encountered

- **DELIVER ran outside the nWave execute orchestrator.** No
  `deliver/roadmap.json` or `deliver/execution-log.json` was created.
  Audit trail recoverable from git (`bf35a68`) only because the commit
  body is unusually thorough. Future slices should run through
  `/nw:deliver` for a machine-readable execution record.
- **DISTILL plan vs DELIVER reality drift on subprocess approach.** Not
  a quality issue (the DELIVER choice is better), but a process note:
  DISTILL's `step-skeletons.md` should be treated as a starting point,
  not a contract. DELIVER's freedom to choose a structurally safer
  subprocess won this slice; capture that freedom explicitly in future
  DISTILL handoffs.

## Permanent artifact locations

All artifacts produced by this feature stayed in their delivery
locations rather than being migrated:

- **Acceptance feature & step bodies** — `crates/foundry-acceptance/tests/features/us-13-contributor-onboarding.feature` + `crates/foundry-acceptance/src/steps/us_13_contributor_onboarding.rs`
- **README helper module** — `crates/foundry-acceptance/src/support/readme_inspect.rs`
- **Manual drill scripts** — `docs/feature/foundry-contributor-onboarding/distill/manual-drills.md` (linked from `CONTRIBUTING.md` and referenced by the `@manual` `unreachable!()` messages in the step source; relocating would silently break both)
- **CI quickstart-verify job** — `.github/workflows/ci.yml`

The DISTILL workspace at `docs/feature/foundry-contributor-onboarding/`
is preserved as historical record — DISTILL artifacts (driver.md,
coverage-matrix.md, step-skeletons.md, wave-decisions.md) remain
where they were authored.

## Open items for v0.1 RC

1. `Cargo.toml` `rust-version = "1.88"` vs `rust-toolchain.toml`
   `channel = "1.91"` discrepancy is intentional but could confuse
   dep-audit tooling. Surface in RELEASING.md.
2. The `@docker-compose` lane stays excluded from `quickstart-verify`.
   Deeper operator scenarios live in RELEASING.md, not the contributor
   contract.
