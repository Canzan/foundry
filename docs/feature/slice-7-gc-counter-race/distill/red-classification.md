# Pre-DELIVER RED classification — slice-7-gc-counter-race

**Date**: 2026-05-27
**Status**: n/a — test-hardening DISTILL with no RED-then-GREEN cycle

## Why the standard RED-classification gate doesn't apply

Identical rationale to slice-6-scenario-hardening and us-06-timing-symmetry-redesign:

1. **No production code change is being asked for.** `crates/foundry-app/src/main.rs:307-320`
   increments `comments_tombstones_purged_total` only after `gc_tombstoned_comments`
   returns `Ok(deleted)` — the counter and the GC task are correct.
2. **The scenarios were already green in isolation.** The `expected 3, got 0` flake
   only manifests under `FOUNDRY_ACCEPTANCE_TAGS=all`, when the subprocess competes for
   CPU/IO with up to 6 sibling scenarios and the sweep tick lands after the fixed wait.
3. **The change is assertion shape, not implementation.** A one-shot
   `running N seconds` + `scrape` + `sample has value V` becomes a bounded-poll
   "eventually reaches V". The intermediate state is a refactored test awaiting
   re-verification, not a RED test awaiting GREEN.

## What replaces the RED gate here

A single verification: run the GC scenarios (and the slice-6 regression) in isolation
post-change and confirm they pass. Captured in this DISTILL's wave-decisions
verification section.

## What the user verifies (out of DISTILL scope)

The acceptance criterion — "passes deterministically across N≥5 consecutive
`FOUNDRY_ACCEPTANCE_TAGS=all` runs" — is the user's turn after DISTILL hands off, per
the slice-6 precedent.

## Classification of the change

| Dimension | Classification |
|---|---|
| Change scope | test-side only (steps + feature + support helper relocation) |
| Production code touched | none |
| RED → GREEN cycle | n/a (no missing implementation) |
| Failure mode being addressed | one-shot scrape of an asynchronously-incremented counter (wrong-shape assertion of a temporal property) |
| Pre-DELIVER classification | n/a — see "Why this gate doesn't apply" above |
| Post-change verification | GC + slice-6 isolation pass + user's @all sweep |
