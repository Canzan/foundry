# Pre-DELIVER RED classification — gc-transient-state-hardening

**Date**: 2026-05-27
**Status**: n/a — test-hardening DISTILL with no RED-then-GREEN cycle

## Why the standard RED-classification gate doesn't apply

Same rationale as the three prior hardenings (slice-6, us-06, slice-7):

1. **No production code change is being asked for.** `main.rs:307-340` increments
   `purged_total` after the DELETE commits and sets `comments_tombstones_pending` to
   the live pending count each tick. The cap, the gauge, and register-at-0 are all
   correct.
2. **The scenarios pass in isolation.** The flakes only manifest under release-mode
   `FOUNDRY_ACCEPTANCE_TAGS=all` contention — and intermittently (3 failures one run,
   2 the next).
3. **The change is assertion shape, not implementation.** Fixed-wait single scrapes
   of transient/non-monotonic values become an ordered-subsequence trajectory poll.

## What replaces the RED gate here

Isolation pass of both reworded scenarios + **N≥3 consecutive release-mode @all
sweeps green** (the contention condition that exposed the flakes). Captured in this
DISTILL's wave-decisions verification section.

## Classification of the change

| Dimension | Classification |
|---|---|
| Change scope | test-side only (support helper + 1 step + 2 scenario rewrites) |
| Production code touched | none |
| RED → GREEN cycle | n/a (no missing implementation) |
| Failure mode being addressed | fixed-wait single scrape of transient / non-monotonic metric values |
| Pre-DELIVER classification | n/a — see above |
| Post-change verification | isolation pass + N≥3 release-mode @all sweeps |
