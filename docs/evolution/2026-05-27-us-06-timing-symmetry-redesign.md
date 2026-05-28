# Evolution — us-06-timing-symmetry-redesign

**Finalized**: 2026-05-27
**Ship commit**: [3c7a952](../../) — "Redesign US-06 timing-symmetry scenario with interleaved median compare"
**Wave coverage**: DISTILL only (DISCUSS + DESIGN intentionally skipped — single-scenario test-side hardening, mirrors slice-6-scenario-hardening)

## Feature summary

Replaces US-06's single-sample sign-in timing comparison with an interleaved
7-pair sample + per-arm median compare (150ms budget), removing the
`Δ~1250ms` flake that the old `|unknown - wrong| < 500ms` shape produced under
`FOUNDRY_ACCEPTANCE_TAGS=all`. The scenario guards the username-enumeration
side-channel property: an attacker timing sign-in responses must not be able to
distinguish a registered email (wrong password) from an unregistered one.

Second of the v0.2.0-RC-gate flake fixes named in
`docs/evolution/2026-05-27-slice-6-scenario-hardening.md` (the first being
slice-6 itself; the third is the slice-7 GC counter race). Test-side only.

## Business context

The timing-symmetry property is a real security NFR — `signin.rs:92-117` runs
exactly one argon2id verify on both the real-user path (`verify_password`
against the stored hash) and the unknown-email path (against `known_bad_hash()`),
specifically so response timing doesn't leak email registration. The flake was
never a production defect; it was the test asserting a statistical property with
a single sample, which became visible once the argon2 `spawn_blocking` migration
(`d9db0b3`) let scenarios run concurrently enough to contend the shared blocking
pool. The flake blocked a clean @all sweep, which blocks the v0.2.0 tag.

## Key decisions

### From DISTILL (`docs/feature/us-06-timing-symmetry-redesign/distill/`)

- **D1 — Split the conflated scenario.** The old scenario asserted both error
  *content* (deterministic) and error *timing* (statistical) in one place. Split
  into an `@error` content scenario (non-enumerable body + no cookie) and a new
  `@nfr-sec-03` timing scenario. Each property now reads in its truthful shape
  (the slice-6 D1 principle: Gherkin words should predict the step-impl shape).
- **D2 — Interleaved sampling + median compare.** 7 strictly-alternating pairs
  (unknown, wrong, unknown, wrong, …) preceded by 1 discarded warm-up pair;
  compare per-arm medians within 150ms. Knobs: N=7 (odd → clean median, enough
  to dilute a single spike), warm-up absorbs the once-per-process
  `known_bad_hash()` argon2 lazy-init, strict alternation makes both arms sample
  the same contention distribution, 150ms budget = generous headroom over
  interleaved-median noise but far below the ~1250ms single-sample spike. Timed
  region is the POST only (CSRF GET fetched untimed).
- **D3 — World state.** Replaced the two scalar `Option<u64>` timing fields with
  two `Vec<u64>` sample vectors; removed the now-dead baseline-capture branch in
  `visitor_submit_signin` and the timing write in the shared
  `submit_signin_inner`. Updated the resets in `us_06_signin.rs` and
  `us_07_project_create.rs`.
- **D4 — Production code stays unchanged.** Zero changes to `foundry-app` /
  `foundry-auth`. The symmetry is already in `signin.rs:103-117`.
- **D5 — Reuse `@nfr-sec-03`.** The timing-symmetry property is the same
  sign-in-confidentiality NFR family as the secure-session walking skeleton; no
  new tag minted (slice-7 D8 "reuse unless genuinely new" principle).

### Why the RED gate didn't apply

Same as slice-6: no missing production code, scenario green in isolation, change
is measurement-shape not implementation. The intermediate state is a refactored
test awaiting re-verification, not a RED test awaiting GREEN.

## Files touched (commit `3c7a952`)

| Path | Change |
|---|---|
| `crates/foundry-acceptance/tests/features/us-06-signin.feature` | Drop timing line from the unknown-email scenario; add the `@nfr-sec-03` timing-symmetry scenario |
| `crates/foundry-acceptance/src/steps/us_06_signin.rs` | Remove old single-sample step + dead baseline-capture + the `submit_signin_inner` timing write; add `timed_signin_post_ms`, `median`, the interleaved `When`, and the median-compare `Then` |
| `crates/foundry-acceptance/src/world.rs` | Two scalar timing fields → two `Vec<u64>` sample vectors |
| `crates/foundry-acceptance/src/steps/us_07_project_create.rs` | Reset clears the new vectors |

Production code: untouched. DESIGN docs: untouched. DEVOPS / CI: untouched.

## Verification at HEAD (`3c7a952`)

- `--name "timing"` → 1 scenario / 4 steps passed (first attempt — no budget or
  deadline iteration needed, unlike slice-6).
- `--name "Unknown email"` → content scenario after the split: passed.
- `--name "sign"` → bootstrap + sign-in walking skeleton: 3 scenarios passed
  (confirms the shared-helper edit broke nothing).
- `--name "sixth failed|Sign-out|Password-reset"` → 3 scenarios passed.
- `cargo clippy -p foundry-acceptance --tests -- -D warnings`: clean.
- `cargo fmt -p foundry-acceptance -- --check`: clean.
- **@all N≥5 flake-resistance sweep**: runs alongside the slice-7 GC fix before
  the v0.2.0 tag cut (the contention condition requires sibling scenarios, so it
  is validated in the combined sweep, not in isolation).

## Lessons learned

1. **A relaxed budget is a smell, not a fix.** The Gherkin said 50ms; the impl
   had quietly been bumped to 500ms to stop the flake — and it still flaked at
   ~1250ms. Widening a single-sample budget chases the tail of a contention
   distribution forever. The structural fix (more samples + a robust statistic)
   let the budget come *back down* to 150ms while being more reliable. When a
   timing budget keeps growing, change the measurement, not the number.
2. **Interleave, don't block, when comparing two timed arms under shared
   contention.** Measuring all of arm A then all of arm B lets a contention
   burst land entirely on one arm. Strict alternation exposes both arms to the
   same time-varying load; the median *difference* then cancels systematic
   contention. This generalizes to any A-vs-B latency comparison on a shared
   runtime.
3. **Warm up once-per-process lazy state before measuring.** `known_bad_hash()`
   pays its argon2 cost once via `OnceCell`; the first unmeasured-arm call would
   be a guaranteed outlier. A discarded warm-up pair is cheaper than widening the
   budget to swallow the cold-start sample.
4. **Splitting a conflated scenario improves both halves.** Content (deterministic)
   and timing (statistical) wanted different assertion shapes and different tags
   (`@error` vs `@nfr-sec-03`). Forcing them into one scenario meant one of them
   was always expressed dishonestly. Two scenarios, two truthful shapes.
5. **The argon2 `spawn_blocking` migration's second-order effects keep
   surfacing.** `d9db0b3` fixed US-08/US-09 by unpinning the async workers, but
   the freed concurrency then exposed harness pool exhaustion (`906ceab`) and now
   this timing-comparison flake. Each perf fix that increases real concurrency
   re-tests every latent single-sample/single-resource assumption in the suite.

## Issues encountered

- **None blocking.** The interleaved-median scenario passed on the first attempt
  — no iteration was needed (contrast slice-6, which needed a 5s→10s deadline
  bump and a per-scrape-timeout fix mid-DISTILL).

## Permanent artefact locations

All artefacts stay in their delivery locations.
`docs/feature/us-06-timing-symmetry-redesign/` has no inbound external
references. The DISTILL context flows into the test code at
`crates/foundry-acceptance/src/steps/us_06_signin.rs` (the `timed_signin_post_ms`
+ `median` helpers and the two new steps) +
`crates/foundry-acceptance/tests/features/us-06-signin.feature`. The production
timing-symmetry contract remains owned by `crates/foundry-app/src/signin.rs`.

## Open items for v0.2 RC

1. **Slice-7 `comments_tombstones_purged_total` counter race** — the last
   remaining flake gate. Bounded-poll treatment analogous to slice-6.
2. **Combined @all N≥5 sweep** — validate US-06 + slice-7 fixes together under
   contention before cutting the tag.
3. **5 deferred metrics + v0.2.0 tag** — unchanged from the slice-6 / handler-
   instrumentation open items.

## Workflow note

Per project convention, the 4-reviewer parallel gate is deferred to PR time
rather than invoked here.
