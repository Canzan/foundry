# Evolution — slice-7-gc-counter-race

**Finalized**: 2026-05-27
**Ship commit**: [6ce1337](../../) — "Harden slice-7 tombstone-GC counter scenarios with bounded-poll"
**Wave coverage**: DISTILL only (DISCUSS + DESIGN intentionally skipped — single-feature test-side hardening; third in the slice-6 / us-06 / slice-7 hardening series)

## Feature summary

Replaces the one-shot scrape assertion on `comments_tombstones_purged_total`
with a bounded-poll Then ("the counter eventually reaches N within 15 seconds")
in the three fast-loop slice-7 GC scenarios. Removes the intermittent
`expected 3, got 0` flake that surfaced under `FOUNDRY_ACCEPTANCE_TAGS=all`:
the counter is incremented by a background sweep tick on a 2s cadence, and the
fixed `running for N seconds` + `scrape` could sample it before the tick fired.

Third and last of the three v0.2.0-RC flake gates named in the slice-6 and
us-06 evolution docs (slice-6 `db_connections_in_use` gauge → us-06 sign-in
timing-symmetry → slice-7 GC counter). With this, all known @all-mode flakes
are addressed. Test-side only.

## Business context

The flake was flagged as a follow-up in
`docs/evolution/2026-05-26-comment-tombstone-gc.md` and re-flagged across the
slice-6 and us-06 evolution docs as the third RC gate. It is not a production
defect: `main.rs:307-320` increments the counter correctly (after the DELETE
commits). The test asserted a temporal value at a single instant. Resolving it
clears the last blocker to a clean @all sweep, which gates the v0.2.0 tag.

## Key decisions

### From DISTILL (`docs/feature/slice-7-gc-counter-race/distill/`)

- **D1 — Bounded-poll the counter.** `running N seconds` + `scrapes` +
  `sample has value V` → `the "…purged_total" counter eventually reaches V
  within 15 seconds`. The step owns its own 250ms poll loop and panics with
  full scrape history on timeout. Same temporal-assertion shape as slice-6's
  gauge fix.
- **D2 — Promote `poll_until_sample` to `support::metrics_scrape`.** Fulfils
  slice-6 D2's deferred promotion ("when the second caller appears"). The
  helper + its 250ms interval / 750ms per-scrape-timeout constants moved
  verbatim from `steps/handler_instrumentation.rs` into the support module as
  `pub`. Predicate-driven and metric-shape-agnostic, so the slice-6 gauge
  caller and the slice-7 counter caller share it unchanged.
- **D3 — `>=` ("reaches") semantics.** The counter is monotonic and the seeded
  count is exact, so "reaches N" is robust to an extra tick without masking a
  missing one. `==` would be brittle under future multi-batch seeding.
- **D4 — Row-state asserts move after the counter poll → deterministic.**
  Because production increments the counter only after the DELETE commits,
  ordering the counter poll before the post-recovery `database holds 0 …older
  than 90 days` assertion makes that check race-free too.
- **D5 — Negative assertions + `@slow` cap scenario unchanged.** The
  counter-stays-0-while-locked and rows-survive-the-failure assertions are
  *negative* (verify no progress); bounded-poll-until-0 is meaningless and
  their waits are load-bearing. The `@slow` 11K-row cap scenario isn't the
  reported flake and its DB-state asserts own cap correctness. Both left as-is
  (slice-6 D5 "don't touch the non-flaky companion" principle).
- **D6 — Production code stays unchanged.** Zero changes to `foundry-app` /
  `foundry-store`.

### Why the RED gate didn't apply

Same as slice-6 / us-06: no missing production code, scenarios green in
isolation, change is assertion-shape not implementation.

## Files touched (commit `6ce1337`)

| Path | Change |
|---|---|
| `crates/foundry-acceptance/src/support/metrics_scrape.rs` | Add `pub poll_until_sample` + the two poll constants (promoted per slice-6 D2) |
| `crates/foundry-acceptance/src/steps/handler_instrumentation.rs` | Remove the local helper + constants; import the promoted one |
| `crates/foundry-acceptance/src/steps/us_10_tombstone_gc.rs` | Add the `…counter eventually reaches N within M seconds` Then |
| `crates/foundry-acceptance/tests/features/comment-tombstone-gc.feature` | Reword the 3 fast-loop counter-increment scenarios to bounded-poll |

Production code: untouched. DESIGN docs: untouched. DEVOPS / CI: untouched.

## Verification at HEAD (`6ce1337`)

- GC feature (`--name "sweep|pending-tombstones"`): 6 scenarios / 75 steps
  passed — the 3 reworded bounded-poll scenarios plus the unchanged
  gc-threshold, the `@slow` gc-cap (10000/11000), and the gc-metrics gauge.
- Slice-6 regression (`--name "Postgres connection pool"`): 1 scenario / 9
  steps passed — relocating `poll_until_sample` left the original caller
  working.
- `cargo clippy -p foundry-acceptance --tests -- -D warnings`: clean.
- `cargo fmt -p foundry-acceptance -- --check`: clean.
- **@all N≥5 flake-resistance sweep**: validated in the combined pre-tag sweep
  with the US-06 fix (the contention condition requires sibling scenarios).

## Lessons learned

1. **Counters under scheduled emission are temporal too.** Slice-6 taught this
   for a polled gauge; slice-7 confirms it for a cadence-incremented counter.
   Any metric updated by a background task — gauge or counter — wants
   "eventually reaches/within" semantics, never a one-shot scrape after a fixed
   sleep. The fixed-sleep idiom is a flake waiting for enough contention.
2. **YAGNI-deferred promotion paid off cleanly.** slice-6 D2 deliberately kept
   `poll_until_sample` local with a written trigger condition ("when the second
   caller appears"). When slice-7 became that caller, the promotion was a
   verbatim move + a generalised doc comment — no redesign, because the helper
   was already predicate-driven. Writing down the promotion trigger at defer
   time made the later decision mechanical.
3. **Order assertions so the cheap one gates the racy one.** Because the counter
   is incremented only after the DELETE commits, polling the counter first turns
   the subsequent row-count assertion from racy to deterministic. Sequencing a
   convergence check ahead of a state check is a general tactic for async
   effects, not a one-off.
4. **Don't bounded-poll a negative.** "The counter stays 0 while the lock is
   held" cannot be expressed as poll-until — there is nothing to converge to.
   These assertions keep their load-bearing fixed wait (which proves a tick was
   *attempted*). Knowing which assertions are positive (wait for an effect) vs
   negative (prove an effect's absence) is what decides whether bounded-poll
   applies.
5. **The series is now closed.** slice-6, us-06, and slice-7 were three faces of
   one root cause: single-instant assertions of values that the (correct)
   production code produces asynchronously, exposed once the argon2
   `spawn_blocking` migration raised real concurrency. The fix pattern
   (bounded-poll / interleaved-median) is now established and the shared helper
   lives in `support`. Future metric/timing scenarios should reach for it by
   default.

## Issues encountered

- **None blocking.** All reworded scenarios passed on the first attempt; the
  relocated helper needed no changes to satisfy both callers.

## Permanent artefact locations

All artefacts stay in their delivery locations.
`docs/feature/slice-7-gc-counter-race/` has no inbound external references. The
DISTILL context flows into the test code at
`crates/foundry-acceptance/src/support/metrics_scrape.rs` (the now-shared
`poll_until_sample`), `crates/foundry-acceptance/src/steps/us_10_tombstone_gc.rs`
(the counter-poll Then), and
`crates/foundry-acceptance/tests/features/comment-tombstone-gc.feature`. The GC
task contract remains owned by `docs/evolution/2026-05-26-comment-tombstone-gc.md`
(ADR-015..017) and the production code at `crates/foundry-app/src/main.rs`.

## Open items for v0.2 RC

1. **Combined @all N≥5 sweep** — validate slice-6 + us-06 + slice-7 fixes
   together under contention before cutting the tag.
2. **5 deferred metrics** — `outbox_pending_jobs`, `bootstrap_tokens_unclaimed`,
   `migration_apply_duration_seconds`, `realtime_listen_disconnects_total`,
   `probe_failures_total` — still need dashboard consumers + emission.
3. **CHANGELOG.md + v0.2.0 tag** — bundles slices 5+6+7 + the three hardening
   fixes. With all flake gates now cleared, this is unblocked.

## Workflow note

Per project convention, the 4-reviewer parallel gate is deferred to PR time
rather than invoked here.
