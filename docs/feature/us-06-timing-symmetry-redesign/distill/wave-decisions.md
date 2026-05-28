# DISTILL wave-decisions — us-06-timing-symmetry-redesign

**Date**: 2026-05-27
**Wave coverage**: DISTILL only (DISCUSS + DESIGN intentionally skipped — single-scenario
test-side hardening, not a new feature; mirrors the slice-6-scenario-hardening shape)
**Inheritance**:
- `docs/evolution/2026-05-27-slice-6-scenario-hardening.md` — the precedent for
  DISTILL-only test-side hardening of a flaky temporal assertion, and the
  open-item that named this redesign as a v0.2.0 RC gate.
- `crates/foundry-app/src/signin.rs:92-117` — the unchanged production handler:
  both the real-user and unknown-email paths run exactly one `argon2id` verify
  (`verify_password` against the user's hash, or against `known_bad_hash()`), so
  the timing-symmetry property is genuinely in production.
- `crates/foundry-auth/src/lib.rs:103-130` — `hash_password`/`verify_password`
  already moved onto `spawn_blocking` (commit `d9db0b3`). The CPU work is off the
  async workers, but the blocking pool is shared across all concurrent scenarios.

## Problem in one paragraph

US-06's "Unknown email produces the same error as wrong password" scenario asserts
the timing-symmetry security property (no username enumeration via response-time
side channel) with a **single-sample comparison**:
`|unknown_email_ms - wrong_password_ms| < 500ms`
(`us_06_signin.rs:312-324`; the Gherkin says "within 50ms" but the step impl was
already relaxed to 500ms — a knob-twiddle that papered over the real issue). Under
`FOUNDRY_ACCEPTANCE_TAGS=all` the in-process harness runs up to 6 scenarios
concurrently, all sharing one `spawn_blocking` pool. Each single timing sample is
hit by whatever argon2 jobs sibling scenarios happen to be running at that instant.
The production code does identical argon2 work on both arms, so the true symmetry
holds — but a single sample of each arm has Δ that spikes to ~1250ms when the two
measurements land in different contention windows. Single-sample comparison of a
contention-sensitive measurement = flake. The fix is statistical, not a budget bump.

## Decisions

### D1 — Split the timing-symmetry property into its own scenario

The old scenario conflated two contracts: (a) unknown email returns the same
**error content** as a wrong password (non-enumerable error body + no cookie), and
(b) unknown email returns in the same **time** as a wrong password (no timing side
channel). Content is deterministic; timing is statistical. Splitting them lets each
scenario assert its property in its truthful shape (the slice-6 D1 principle: a
reader should predict the step-impl shape from the Gherkin words).

**Before** (one scenario, `us-06-signin.feature:43-47`):
```gherkin
Scenario: Unknown email produces the same error as wrong password
  When a visitor submits the sign-in form with email "ghost@acme.com" and password "anything"
  Then the response body contains "Invalid email or password"
  And no session cookie is set
  And the response time is within 50ms of the wrong-password response time
```

**After** (content scenario keeps the deterministic assertions; new scenario owns
the statistical timing property):
```gherkin
Scenario: Unknown email produces the same error as wrong password
  When a visitor submits the sign-in form with email "ghost@acme.com" and password "anything"
  Then the response body contains "Invalid email or password"
  And no session cookie is set

@nfr-sec-03
Scenario: Sign-in timing does not reveal whether an email is registered
  When sign-in latency is sampled over 7 interleaved unknown-email and wrong-password attempts
  Then the median unknown-email latency is within 150ms of the median wrong-password latency
```

### D2 — Interleaved sampling + median compare (the core redesign)

The `When` step performs **7 strictly-alternating pairs** (unknown, wrong, unknown,
wrong, …), preceded by **1 discarded warm-up pair**. The `Then` step computes the
median of each arm's 7 samples and asserts the absolute difference of the medians is
within budget.

| Knob | Value | Reason |
|---|---|---|
| pairs (N) | 7 | Odd → unambiguous median (4th of 7 sorted). Enough samples that a single contention spike is one of 7, not the whole measurement, while keeping the scenario's argon2 cost bounded (14 measured verifies + 2 warm-up). |
| warm-up pairs | 1 (discarded) | `known_bad_hash()` pays the argon2 lazy-init cost once per process (`tokio::sync::OnceCell`); the first unknown-email call in the process would otherwise be a guaranteed outlier. One warm-up pair stabilises both arms before measurement. |
| interleaving | strict alternation u,w,u,w,… | Both arms sample the same time-varying contention distribution. Block sampling (all u then all w) would let a contention burst land entirely on one arm; alternation cancels systematic contention in the median difference. |
| budget | 150ms median Δ | Production runs identical argon2id verify on both paths, so the noise-free median Δ is bounded by the DB-lookup difference (`find_user_by_email` → row vs `None`, sub-millisecond). 150ms is generous headroom above interleaved-median noise yet far below the ~1250ms single-sample spike that the old test produced and well below any Δ that would signal a real leak. Tighter bounds (≤50ms) are plausible under median smoothing but reserved for a dedicated bench; 150ms is the CI-safe budget. |
| timed region | POST only | CSRF GET is fetched untimed before the `Instant::now()` (matching the old `submit_signin_inner`); the measurement isolates the argon2-dominated POST. The cheap GET would add equal noise to both arms anyway. |

### D3 — World state: replace the two scalar fields with sample vectors

`us_06_last_response_ms: Option<u64>` and `us_06_wrong_pw_response_ms: Option<u64>`
existed only to carry one sample of each arm between the `When` and the old `Then`.
They are replaced by `us_06_unknown_latencies_ms: Vec<u64>` and
`us_06_wrong_pw_latencies_ms: Vec<u64>`. The `submit_signin_inner` timing write and
the `visitor_submit_signin` baseline-capture branch (both feeding the old fields)
are removed — dead after the split (delete-unused per project convention). Resets in
`us_06_signin.rs` and `us_07_project_create.rs` updated to clear the vectors.

### D4 — Production code stays unchanged

Zero changes to `crates/foundry-app/src/signin.rs`, `crates/foundry-auth/`, or any
production crate. `signin.rs:103-117` already runs one argon2 verify on both arms;
the symmetry is real. The fix is in the test's measurement methodology only.

### D5 — Tag: reuse `@nfr-sec-03`

The walking-skeleton scenario already carries `@nfr-sec-03` (secure-session
property family). Timing-symmetry / non-enumeration is the same security NFR family
(sign-in confidentiality). No new tag is minted — consistent with slice-7 D8's
"reuse the existing NFR tag unless the property is genuinely new" principle. The old
single-sample line lived in an `@error`-tagged scenario; the new dedicated scenario
takes `@nfr-sec-03` because the property under test is a security NFR, not an error
path.

## Why the RED gate doesn't apply

Same rationale as slice-6: no production code is missing, the scenario already
passes in isolation (flake only under @all contention), and the change is
measurement-shape, not implementation. The intermediate state is a refactored test
awaiting re-verification, not a RED test awaiting GREEN. Replacement gate:
single-scenario isolation pass + the user's @all sweep for N≥5 deterministic passes.

## Files touched

| Path | Change |
|---|---|
| `crates/foundry-acceptance/tests/features/us-06-signin.feature` | Drop the timing line from the unknown-email scenario; add the dedicated `@nfr-sec-03` timing-symmetry scenario |
| `crates/foundry-acceptance/src/steps/us_06_signin.rs` | Remove old single-sample step + dead baseline-capture; add interleaved-sampling `When` + median-compare `Then` |
| `crates/foundry-acceptance/src/world.rs` | Replace two scalar timing fields with two `Vec<u64>` sample vectors |
| `crates/foundry-acceptance/src/steps/us_07_project_create.rs` | Update the world-reset to clear the new vectors |

Production code: untouched. DESIGN docs: untouched. DEVOPS / CI: untouched.

## Verification protocol

Run the two US-06 timing scenarios in isolation post-change:
```
cargo test -p foundry-acceptance --test acceptance -- --name "timing"
cargo test -p foundry-acceptance --test acceptance -- --name "Unknown email"
```
Full @all flake-resistance (N≥5) remains the user's responsibility per the
acceptance criterion. Results appended below after the run.

### Isolation run (post-change) — PASSED

- `--name "timing"` → "Sign-in timing does not reveal whether an email is
  registered": 1 scenario / 4 steps passed.
- `--name "Unknown email"` → content scenario after the split: 1 scenario /
  5 steps passed.
- `--name "sign"` (bootstrap + sign-in walking skeleton): 3 scenarios passed
  — confirms removing the timing write from the shared `submit_signin_inner`
  broke nothing.
- `--name "sixth failed|Sign-out|Password-reset"`: 3 scenarios passed — the
  remaining US-06 scenarios are intact.
- `cargo clippy -p foundry-acceptance --tests -- -D warnings`: clean.
- `cargo fmt -p foundry-acceptance -- --check`: clean.

The interleaved-median scenario passed on the first attempt — no deadline or
budget iteration was needed (contrast slice-6, which needed a deadline bump).
The 150ms median budget held comfortably in isolation; the @all contention
sweep (run alongside the slice-7 GC fix) is the sufficient-evidence check.
