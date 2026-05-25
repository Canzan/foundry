# Wave Decisions — foundry-contributor-onboarding (Slice 4)

DISTILL-wave decisions that gate DELIVER. Established 2026-05-25
during the slice-4 DISTILL pass. Slice 4 is the final MVP slice
(US-13, contributor onboarding); slices 1-3 are all shipped and
reviewer-approved. This file inherits all slice-1/2/3 decisions
verbatim and records only the slice-4 deltas.

## Strategy: C (all real adapters) — inherited

Slice 4 inherits Strategy C from slices 1-3 (per the project's
Architecture of Reference at
`docs/architecture/atdd-infrastructure-policy.md`): every scenario
exercises production driving adapters and real driven adapters. No
mocks, no in-memory fakes. The slice-4 driving surfaces are file reads
(`README.md`, `rust-toolchain.toml`) and one `cargo test` subprocess
— the most "real" surfaces possible because they are the contributor's
literal first interactions with the repository.

The slice-3 project policy table at
`docs/architecture/atdd-infrastructure-policy.md` does NOT need a new
row for slice 4. The new driving surfaces are file reads, which are not
ports in the hexagonal sense — they are the test's own access to the
on-disk contract under test. Recording them in the policy would muddy
the distinction.

## Tier composition: Tier A only — Mandate 10 condition not met

Per Mandate 10 (Two-tier acceptance), Tier B (state-machine PBT) is
added ONLY when the journey is ≥3 chained scenarios AND the input
space is domain-rich. US-13 has:

- 4 automated scenarios, none chained (each is a fresh "given a
  contributor reading the README" — Pillar 2 chaining does not apply
  because each Then assertion is about a different on-disk artifact).
- No domain-rich input space (the inputs are fixed: the README file,
  the rust-toolchain.toml file, the `cargo test` subprocess).
- The only observable state mutation is "did the subprocess exit 0".

Tier B is NOT emitted. Tier A `.feature` + step bodies is the full
acceptance contract.

## PBT input mode: example-only — Mandate 9 layer constraint

All 4 automated scenarios run at layer 3+ (file I/O or subprocess).
Per Mandate 9, layer 3+ tests are example-only. No PBT decorators, no
generated inputs. The sad path (toolchain-error) is a single named
example per Mandate 11.

## ADR-04 — file-inspection over real-cargo invocation for toolchain error

| Option | Status | Rationale |
|---|---|---|
| **File-inspection — assert `rust-toolchain.toml` pins a specific version + README MSRV agrees** | **CHOSEN** for scenario #3 | Fast (~1 ms), no Docker isolation, pins the *contract* that drives rustup's auto-install behaviour. The chain "README claims X → toolchain pins X → rustup auto-installs X for contributors below X" is a transitive contract that fails the most upstream link in a single assertion. |
| Real cargo invocation with `RUSTUP_TOOLCHAIN=1.75 cargo build` | DEFERRED | Either rustup silently resolves to the project-pinned channel (proving nothing — the assertion under test is `rustup` ITSELF, not Foundry) or we Docker-isolate (~30 s + new `@docker-compose` harness + new `rust:1.75-slim` fixture). The cost is disproportionate to the contract. |
| Both | DEFERRED | The structural assertion (file-inspection) gives 100% of the contract value. A docker-compose lane would be defence-in-depth at high cost; the team can add it post-MVP if regression data justifies. |

The file-inspection scenario carries enough specificity to fail loudly
when the contract drifts. Today's reality (`rust-toolchain.toml` pins
`channel = "stable"` — a generic channel, not a specific version) is
precisely what the third Then catches. DELIVER's first GREEN action on
this slice is editing `rust-toolchain.toml` to a specific version. The
test pins that change against future regression.

## ADR-05 — walking-skeleton subprocess in the default fast loop (revisit if budget breaks)

| Option | Status | Rationale |
|---|---|---|
| **Walking-skeleton scenario #1 in default loop** | **CHOSEN** | The contract IS that the contributor's `cargo test` works out of the box. Moving this to an opt-in lane defeats US-13's point. Suite-time delta: ~25-35 s; the slice 1+2+3 default loop sits around 30-32 s, so slice-4 doubles it to ~55-65 s — within the 60 s top-line budget mentioned by slice-3, but tight. |
| Walking-skeleton scenario behind `@manual-trigger` | FALLBACK | If DELIVER measures the slice-4 fast loop exceeding the 60 s top-line OR sees the nested cargo invocation flaking (port collisions, container reuse races), move to `@manual-trigger` and let the manual drill carry the contract. |

The fallback is cheap to apply (one tag edit) and well-precedented
(slice-3 `@docker-compose @us-02 @manual-trigger` lane). DELIVER owns
the call after one CI measurement.

## ADR-06 — `@manual` drills carry the human-experience contracts

UAT scenarios #1 ("≤10 minutes to green tests") and #3 ("visible
change after one-line edit") are process metrics, not software
contracts. Per slice-1 precedent (`us-01-install.feature`'s `@manual
@demo` scenario for the operator-side "30 min to admin claim" promise)
and slice-1 US-12's `@manual` browser drill, these stay manual.

Drill scripts live at
`docs/feature/foundry-contributor-onboarding/distill/manual-drills.md`
(committed in this slice). They name the start condition, the timer,
the stopwatch checkpoints, and the pass/fail criterion. A contributor
or maintainer runs them at release-candidate cuts and pre-1.0; the
results inform the JTBD outcome-3 onboarding survey baseline.

## ADR-07 — `@manual` step bodies fail loudly if invoked

`acceptance.rs` already filters `@manual` by default (slice-1
precedent). The two `@manual` scenario step bodies in slice 4 nonetheless
use `unreachable!("@manual scenario invoked; run the drill instead")`
in their step bodies so that a misconfigured CI invocation
(`FOUNDRY_ACCEPTANCE_TAGS=all` would still exclude `@manual`, but a
future ad-hoc invocation might not) surfaces immediately rather than
silently passing.

## Scenarios per file

| File | Scenarios | WS scenarios | `@manual` scenarios | `@error` / sad-path | Approach |
|---|---:|---:|---:|---:|---|
| `us-13-contributor-onboarding.feature` | 6 | 1 | 2 | 1 (toolchain-error pinned via file-inspection contract) | C (file-inspection + nested subprocess) |
| **TOTAL slice 4** | **6** | **1** | **2** | **1** | — |

Error / sad-path ratio across automated scenarios: 1/4 = 25%. Below
the 40% target band. Justification: US-13 is contract-shaped, not
behaviour-shaped. The "errors" a contributor encounters
(out-of-date Rust toolchain, missing Docker daemon, incomplete
README) are precisely the structural contracts the three file-
inspection scenarios pin. Adding synthetic `@error` scenarios (e.g.
"the README is empty", "the Quickstart heading is misspelled") would
test the test helpers, not the contributor experience. Reviewer can
override.

## Tag conventions (additions only)

Inherited from slices 1-3: `@slice1`, `@slice2`, `@slice3`,
`@walking_skeleton`, `@real-io`, `@driving_port`, `@error`,
`@in-memory`, `@manual`, `@docker-compose`, `@us-NN`.

Added in slice 4:

- `@slice4` — every scenario in this slice.
- `@contributor-onboarding` — feature-level marker for selective runs.
- `@readme-contract` — narrows the three file-inspection scenarios.
- `@hot-reload` — narrows the watch-mode scenario.
- `@nfr-onboarding-10min` — the ≤10-min drill's NFR marker (parallel to
  slice-3's `@nfr-perf-*` / `@nfr-sec-*` style).

## CI invocation (delta only)

Slice-4 fast loop (no change to the default invocation):
```
cargo test -p foundry-acceptance
# excludes @manual + @manual-trigger + @docker-compose by default
# (see crates/foundry-acceptance/tests/acceptance.rs)
```

Slice-4 selective run:
```
cargo test -p foundry-acceptance --test acceptance -- \
  --tags "@slice4"
```

## Suite-time budget

| Bucket | Estimate | Notes |
|---|---:|---|
| 3 × file-inspection scenarios | ~5 ms | pure file read + string scan |
| 1 × walking-skeleton subprocess | ~25-35 s | nested `cargo test --tags "@walking_skeleton and not @us-13"` |
| 2 × `@manual` (skipped by default) | 0 s | filtered out |
| **Subtotal — slice 4 default loop** | **~25-35 s** | dominated by the nested cargo invocation |
| **Suite total (slice 1 + 2 + 3 + 4)** | **~55-65 s** | within the 60-s top-line target, edge-case tight |

If the slice-4 budget exceeds the 60-s top-line, ADR-05 fallback
applies: move the walking-skeleton scenario behind `@manual-trigger`.
The other three scenarios add negligible cost regardless.

## Open Decisions for DELIVER

| Decision | Status | Owner |
|----------|--------|-------|
| `rust-toolchain.toml` pinned channel — bump from `"stable"` to `"1.85"` (or whichever specific version) | OPEN — needs a quick "what is the actual MSRV?" check against `cargo msrv find` or the workspace's `rust-version` field | DELIVER |
| README Quickstart section — unify to 5 commands matching US-13 § Elevator Pitch (`cargo install sqlx-cli`, `cargo build`, `docker compose up postgres`, `sqlx migrate run`, `cargo test`) | OPEN — current README has 4 development commands but no unified Quickstart heading | DELIVER |
| Hot-reload command + local URL documented in README | OPEN — `cargo watch -x run` mentioned only in stories.md; add to README | DELIVER |
| Move walking-skeleton scenario to `@manual-trigger` if fast-loop budget breaks 60s | CONDITIONAL on CI measurement | DELIVER |
| `tests/common/state_delta.rs` polyglot port bootstrap | DEFERRED — US-13 scenarios run at layer 3+ (example-only per Mandate 9, traditional assertions per Mandate 8); no layer 1-2 PBT introduced in slice 4. The Rust state-delta port can be bootstrapped lazily by a future feature that introduces in-memory acceptance | DELIVER may skip; future feature triggers |
| Whether to add a `@docker-compose @us-13 @manual-trigger` real-cargo-old-toolchain lane | DEFERRED — see ADR-04 | post-MVP |

## DELIVER Pre-flight Checklist

DELIVER must satisfy these before merging:

- [ ] `crates/foundry-acceptance/src/support/readme_inspect.rs` exists with
      the six pure helpers (workspace_root, read_readme, read_rust_toolchain,
      find_quickstart, extract_pinned_msrv, find_readme_msrv_mention,
      find_watch_command, find_local_app_url — see step-skeletons.md).
- [ ] `crates/foundry-acceptance/src/steps/us_13_contributor_onboarding.rs`
      exists and is force-linked from `tests/acceptance.rs`.
- [ ] `crates/foundry-acceptance/tests/features/us-13-contributor-onboarding.feature`
      exists (copy from `distill/features/`).
- [ ] `FoundryWorld` carries the three new optional US-13 fields.
- [ ] `README.md`'s Quickstart section is unified to 5 contributor commands
      and names Rust + Docker as prereqs.
- [ ] `rust-toolchain.toml`'s `[toolchain].channel` is pinned to a
      specific version (NOT `"stable"`).
- [ ] README documents a watch-and-rebuild command + a local URL.
- [ ] All 4 automated scenarios are GREEN.
- [ ] The 2 `@manual` scenarios remain filtered out by the default runner.
- [ ] No scenario regresses slice 1+2+3's green state.
- [ ] `manual-drills.md` linked from `CONTRIBUTING.md` (one-line addition).
- [ ] Slice-4 default fast loop runs in ≤+35s on top of slice 1+2+3
      (combined ≤65s); if it breaks 60s, apply ADR-05 fallback and
      record the measurement here.

## Final Wave Review Gate

Per `nw-distill` § "Final Wave Review Gate", after appending DISTILL
sections to a unified `feature-delta.md` the agent dispatches four
reviewers (Eclipse / Architect / Forge / Sentinel) in parallel. Slice 4
does NOT have a unified `feature-delta.md` (the project predates the
unified-feature-delta refactor and uses the legacy per-wave file
layout — same as slices 1-3, all reviewer-approved under the legacy
layout). The slice-4 deliverables are the five files in this `distill/`
directory plus the executable `.feature` file under
`crates/foundry-acceptance/tests/features/` once DELIVER copies it.
Final review is invoked separately at PR time, against this directory.
