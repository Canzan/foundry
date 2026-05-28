# Pre-DELIVER RED classification — us-06-timing-symmetry-redesign

**Date**: 2026-05-27
**Status**: n/a — test-hardening DISTILL with no RED-then-GREEN cycle

## Why the standard RED-classification gate doesn't apply

Identical rationale to slice-6-scenario-hardening:

1. **No production code change is being asked for.** `crates/foundry-app/src/signin.rs:103-117`
   already runs exactly one argon2id verify on both the real-user and unknown-email
   paths (the latter against `known_bad_hash()`), so the timing-symmetry property is
   genuinely in production. Production is correct.
2. **The scenario was already green in isolation.** The flake only manifests under
   `FOUNDRY_ACCEPTANCE_TAGS=all` contention (cap=6 concurrent scenarios sharing one
   `spawn_blocking` pool), not when run alone.
3. **The change is measurement shape, not implementation.** A single-sample
   `|unknown - wrong| < 500ms` comparison becomes an interleaved-N-pairs median
   compare. The intermediate state is a refactored test awaiting re-verification, not
   a RED test awaiting GREEN.

## What replaces the RED gate here

A single verification: run the two US-06 timing scenarios in isolation post-change
and confirm they pass. Captured in this DISTILL's wave-decisions verification
section after the run.

## What the user verifies (out of DISTILL scope)

The acceptance criterion — "passes deterministically across N≥5 consecutive
`FOUNDRY_ACCEPTANCE_TAGS=all` runs" — requires a multi-minute @all sweep per run.
Per the slice-6 precedent this is the user's turn after DISTILL hands off. The
single-scenario isolation pass is necessary but not sufficient evidence.

## Classification of the change

| Dimension | Classification |
|---|---|
| Change scope | test-side only (steps + feature + world + one reset) |
| Production code touched | none |
| RED → GREEN cycle | n/a (no missing implementation) |
| Failure mode being addressed | single-sample assertion of a contention-sensitive measurement (wrong-shape assertion of a statistical property) |
| Pre-DELIVER classification | n/a — see "Why this gate doesn't apply" above |
| Post-change verification | two-scenario isolation pass + user's @all sweep |
