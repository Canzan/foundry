# Pre-DELIVER RED classification — acceptance-all-lane-stabilization

**Date**: 2026-05-28
**Status**: n/a — test-infrastructure hardening with no RED-then-GREEN cycle

## Why the standard RED-classification gate doesn't apply

1. **No production code change.** The fixes are in the test harness
   (`support/harness.rs` testcontainer `max_connections`) and a scenario tag
   (`@serial`). Production GC/pool/metrics code is correct and untouched.
2. **The scenarios pass in isolation.** The slice-6 `db_connections_in_use`
   scenario passes alone; it flaked only under `@all` 6-way parallelism where its
   32 `tokio::spawn` load-generator tasks were CPU-starved.
3. **The change is test-infrastructure (scheduling + resource ceiling), not
   implementation.** Raising an ephemeral container's connection ceiling and
   de-contending a load-generating scenario; no RED test awaiting GREEN.

## What replaces the RED gate here

A baseline measurement (3 @all sweeps to characterise the residual after
gc-transient-state-hardening) followed by N=5 consecutive release-mode `@all`
sweeps green post-fix. Captured in wave-decisions § Findings + Result.

## Classification of the change

| Dimension | Classification |
|---|---|
| Change scope | test-infrastructure (harness container config + 1 scenario tag) |
| Production code touched | none |
| RED → GREEN cycle | n/a |
| Failure mode addressed | test-runtime starvation of a load generator + shared-container connection-ceiling contention under @all |
| Post-change verification | 5 consecutive release-mode @all sweeps (111/111) |
