# Pre-DELIVER RED classification — slice-6-scenario-hardening

**Date**: 2026-05-27
**Status**: n/a — test-hardening DISTILL with no RED-then-GREEN cycle

## Why the standard RED-classification gate doesn't apply

The skill's pre-DELIVER fail-for-the-right-reason gate exists to confirm that
scenarios fail because the implementation is missing (✅ MISSING_FUNCTIONALITY)
rather than because of import errors, fixture bugs, or wrong-shape assertions
(❌). DELIVER then turns RED → GREEN by writing the missing production code.

This DISTILL doesn't fit that shape:

1. **No production code change is being asked for.** Per the brief and the
   `@nw-troubleshooter` RCA from this session: the `db_connections_in_use`
   gauge IS being updated by the production code's 1-second poll task
   (`crates/foundry-app/src/main.rs:208-219`). Production is correct.
2. **The scenario was already green in isolation.** The flake only manifests
   under `FOUNDRY_ACCEPTANCE_TAGS=all` contention (cap=6 parallel scenarios),
   not when run alone. The single-scenario green state means there is no RED
   to classify pre-DELIVER.
3. **The change is shape, not implementation.** We're replacing a one-shot
   assertion with a bounded-poll assertion of the same observable. The
   intermediate state (after rewrite, before re-verify) is NOT a RED test
   awaiting GREEN — it's a refactored test awaiting confirmation it still
   passes.

## What replaces the RED gate here

A single verification: run the scenario in isolation post-change and confirm
it passes. This is the brief's "verification protocol":

```
cargo test -p foundry-acceptance --test acceptance -- --name "Postgres connection pool"
```

Outcome of that run is captured in this DISTILL's handoff message back to the
user, not in this file (the run happens after this file is written, in the
verification step).

## What the user verifies (out of DISTILL scope)

The acceptance criterion in the brief — "passes deterministically across N≥5
consecutive `FOUNDRY_ACCEPTANCE_TAGS=all` runs" — requires a ~8.5min @all
sweep per run, ~45min for N=5. Per the brief this is the user's turn after
DISTILL hands off, not DISTILL's responsibility. The single-scenario isolation
pass DISTILL produces is necessary but not sufficient evidence; the @all sweep
is the sufficient evidence.

## Classification of the change

| Dimension | Classification |
|---|---|
| Change scope | test-side only (steps file + .feature file) |
| Production code touched | none |
| RED → GREEN cycle | n/a (no missing implementation) |
| Failure mode being addressed | wrong-shape assertion of a temporal property (Universe-shape issue, not Universe-content) |
| Pre-DELIVER classification | n/a — see "Why this gate doesn't apply" above |
| Post-change verification | single-scenario isolation pass + user's @all sweep |
