# Evolution — slice-6-scenario-hardening

**Finalized**: 2026-05-27
**Ship commit**: [9dab570](../../) — "Harden slice-6 db_connections_in_use scenario with bounded-poll"
**Wave coverage**: DISTILL only (DISCUSS + DESIGN intentionally skipped — single-scenario test-side hardening, not a new feature)

## Feature summary

Replaces a one-shot Prometheus scrape assertion in the slice-6
`db_connections_in_use` scenario with a bounded-poll assertion of the same
observable, restoring deterministic passes under `FOUNDRY_ACCEPTANCE_TAGS=all`
contention. The gauge is updated by a 1-second poll task in
`crates/foundry-app/src/main.rs:208-219`; a single-instant scrape could sample
a transient idle window even while traffic was in-flight. Test-side change
only — production code, the slice-6 ADR-012 poll-task contract, and the
Grafana dashboard all stay as written.

This is the first feature in the project that is **not** a full nWave
traversal: only DISTILL was dispatched. The brief explicitly excluded
DISCUSS/DESIGN because the contract (a sampled gauge updated at fixed
cadence) was already settled by slice-6's ADR-012 — the work was to align the
test's assertion shape with the temporal nature of that contract, not to
re-decide what the contract should be.

## Business context

Surfaced as a follow-up flag in `docs/evolution/2026-05-26-comment-tombstone-gc.md`
("Slice-6 `db_connections_in_use` flake under heavy `@all`-tag contention…
worth a troubleshooter pass before v0.2.0 RC; might be a contended-pool
test-design issue or a real race"). The v0.2.0 RC bundles slices 5+6+7 and
needs the @all sweep clean before the tag cuts; this hardening removes one of
the two test-side blockers (the other — US-06 timing-symmetry — is now the
remaining gate).

## Key decisions

### From DISTILL (`docs/feature/slice-6-scenario-hardening/distill/`)

- **D1 — Re-word the Gherkin** to express the temporal property
  (`is eventually greater than 0 within 5 seconds`) instead of patching a
  one-shot assertion silently. Picked Option A over Option B because Gherkin
  should let a reader predict the step impl's shape from the words. The
  preceding `When the operator scrapes the metrics endpoint` step was
  REMOVED from this scenario — the new Then owns its own scrape loop.
- **D2 — Helper signature** `poll_until_sample<P>(addr, metric_name,
  predicate, timeout) -> MetricSample`. Lives next to existing step bodies
  in `crates/foundry-acceptance/src/steps/handler_instrumentation.rs`. Not
  promoted to `support/metrics_scrape.rs` yet — single caller, single
  scenario. Promote when the second caller appears (YAGNI).
- **D3 — Timing knobs.** Outer deadline 10s (initial 5s proved insufficient
  under @all contention because the shared `scrape_metrics_raw`'s 10s reqwest
  timeout meant a single slow scrape could monopolise the deadline). Inner
  poll interval 250ms. Per-scrape timeout 750ms via a helper-owned reqwest
  client. Worst-case scenario wall-clock grows from ~8s to ~16s — acceptable,
  and that worst case indicates the production code legitimately isn't
  holding a pool conn (a real bug we WANT to surface).
- **D4 — Panic-on-timeout carries the full sample history.** The helper's
  panic message dumps every scrape outcome it observed during the deadline
  window (timestamp + samples or error). The original one-shot assertion gave
  "expected > 0, got 0" with no history; the new shape captures the temporal
  shape of what the subprocess actually emitted, in the test output. Future
  flake investigations get the data in-band rather than needing a logging
  re-run.
- **D5 — Companion `register-at-0` scenario stays unchanged.** It asserts a
  different invariant (the register-at-0 guarantee from slice-6 D4) at a
  structurally non-flaky moment (immediately after spawn, no in-flight
  traffic). Already passes deterministically; the brief's "no change to the
  companion scenario" constraint held.
- **D6 — Production code stays unchanged.** Zero changes to `foundry-app/`,
  `foundry-store/`, `foundry-realtime/`. The 1s poll task is correct; the
  metric definition is correct; the dashboard is correct. The fix is in the
  test layer only.

### Why the RED gate didn't apply (red-classification.md)

The pre-DELIVER fail-for-the-right-reason gate exists for RED→GREEN cycles
where DELIVER writes missing production code. This DISTILL didn't fit that
shape: no production code change was being asked for, the scenario was
already green in isolation (flake only manifested under @all contention), and
the change was assertion-shape, not implementation. The intermediate state
was NOT a RED test awaiting GREEN — it was a refactored test awaiting
re-verification. Replacement gate: single-scenario isolation pass +
user-driven @all sweep for N≥5 deterministic-pass evidence.

## Surrounding perf-flake triage (same session, different scope)

While the slice-6 scenario hardening was narrowly the work of this DISTILL,
the @all sweep that exposed it also surfaced three sibling flakes that were
fixed in the same session under separate commits. Recorded here for the v0.2
RC narrative; **these are NOT part of this feature's DISTILL** and have no
artefact directory:

1. **`1f0cb86` — Cap @all-mode cucumber concurrency at 6** (test harness).
   The unbounded @all run let N scenarios each open a 10-connection sqlx
   pool, exceeding the shared Postgres container's 100-connection ceiling;
   remote acquires blocked, the 32 in-flight /readyz pounders hit their 2s
   timeout, and the local `in_use` gauge never rose above 0 across the scrape
   window. Cap restores symmetry with the default lane.
2. **`d9db0b3` — Move argon2 password hash/verify onto `spawn_blocking`**
   (production fix in `foundry-auth`). OWASP-grade argon2id pins a CPU for
   80-300ms per call; running it directly on a tokio worker thread blocked
   every other future scheduled on that worker. Caused a bimodal P95 on
   US-08's 100-POST burst (~70 fast samples 2-12ms, ~30 slow samples in a
   tight 1238-1282ms plateau) and the US-09 SSE-arrival flake. Patch makes
   `hash_password` and `verify_password` `async fn` and moves the CPU work
   into `tokio::task::spawn_blocking`; `known_bad_hash()` switches from
   `std::sync::OnceLock` to `tokio::sync::OnceCell` so the lazy init can
   `.await`. US-08 P95 drops from 1259ms → 2ms (matches the slice-1
   baseline); US-09 stops flaking.
3. **`906ceab` — Bump harness sqlx pool to 10** (test harness). After the
   argon2 migration, scenarios stopped queueing behind pinned tokio workers
   and started making concurrent progress fast enough to exhaust the
   4-conn per-scenario harness pool, surfacing as `PoolTimedOut` on
   workspace-seed inserts. Mirrors `foundry-store/src/lib.rs:85`'s
   `.max_connections(10)` in both `fresh_schema_pool_with_url` and
   `fresh_schema_pool_no_migrations`. US-02's pool-ceiling assertion (≤ 10
   per NFR-PERF-04) still holds.

## Files touched (DISTILL deliverable, commit `9dab570`)

| Path | Change |
|---|---|
| `crates/foundry-acceptance/tests/features/handler-instrumentation.feature` | Re-word one Then; remove the standalone `When` from that scenario |
| `crates/foundry-acceptance/src/steps/handler_instrumentation.rs` | Add `poll_until_sample` helper; replace one `#[then]` handler |

Production code: untouched. DESIGN docs: untouched. DEVOPS / CI: untouched.

## Verification at HEAD (`9dab570`)

- Single-scenario isolation pass: `cargo test -p foundry-acceptance --test acceptance -- --name "Postgres connection pool"` — green.
- `FOUNDRY_ACCEPTANCE_TAGS=all` run: 108/110 scenarios pass. Slice-6
  scenarios both green. The two remaining @all failures are pre-existing
  test-design issues, NOT regressions from this DISTILL:
  - **US-06 timing-symmetry** — Δ~1250ms vs 500ms budget. Root cause is
    argon2 contention on the `spawn_blocking` CPU pool from sibling scenarios'
    Background hashes. Right fix is scenario redesign (interleaved N pairs
    with statistical compare), not a budget tweak. Tracked in CONTEXT.md.
  - **Slice-7 `comments_tombstones_purged_total` counter race** — GC timing
    race surfacing as `expected 3, got 0`. Needs bounded-poll treatment
    analogous to this DISTILL's. Tracked in CONTEXT.md.
- Full `@all`-mode flake-resistance check (N≥5 consecutive runs) remains the
  user's responsibility per the acceptance criterion. Run 7 = 1-of-1 pass;
  CI will accumulate the larger sample.

## Lessons learned

1. **Single-instant assertion of a sampled gauge is a structural flake.**
   The metric was being updated correctly; the test was reading it at a
   moment that may or may not have captured `in_use > 0`. Bounded-poll
   ("eventually within Δt") is the right shape for any temporal property —
   counters under cadenced emission, gauges under polling, async-effect
   observables. Future scenarios that assert on metric values should default
   to this shape unless the metric is genuinely synchronous-on-request.
2. **Gherkin words should let a reader predict the step impl's shape.**
   Option B (silent step-impl fix preserving the existing English) was
   tempting because it minimizes diff. It also lies to the reader: the
   English would have said "is greater than 0" while the impl polled. Option
   A made the diff bigger and the meaning clearer. Worth the cost.
3. **The shared `scrape_metrics_raw` 10s timeout is pathological inside a
   poll loop.** One slow scrape consumes the whole outer deadline. Helpers
   that scrape inside loops should build their own short-timeout reqwest
   clients, not reuse the shared one-shot scraper. Future poll-loop helpers
   that emerge should follow the same convention.
4. **Panic-on-timeout should carry the history.** A single "expected > 0,
   got 0" is unhelpful when the failure is intermittent. The helper now
   dumps every observed scrape outcome in the panic message. Future
   temporal-assertion helpers should default to this — flake-debuggability
   is a property of the panic message, not the logging configuration.
5. **DISCUSS + DESIGN are skippable for narrow test-side hardening when the
   contract is already in an evolution doc.** The slice-6 contract was
   settled by ADR-012 (in `docs/evolution/2026-05-26-handler-instrumentation.md`).
   Re-running DISCUSS/DESIGN would have re-litigated a closed question. The
   wave decision (DISTILL-only) was the right size for the work. Future
   test-side hardenings that don't change a contract should default to this
   shape: DISTILL-only, 1-2 day cycle, no DELIVER dispatch (direct TDD by
   the agent inside DISTILL).
6. **@all-mode flake triage compounds.** The four session commits
   (`1f0cb86`, `d9db0b3`, `906ceab`, `9dab570`) fix four distinct contention
   issues that the unbounded @all run surfaced simultaneously. Fixing the
   first (concurrency cap) made the next visible (argon2 worker pinning);
   fixing that one made the next visible (harness pool exhaustion); fixing
   that one made the slice-6 assertion shape the last remaining flake. The
   triage was sequential, not parallel — each fix unmasked the next. Future
   perf-flake triage should expect this layering and budget time for ≥3
   rounds.

## Issues encountered

- **None blocking this DISTILL.** The wave-decisions doc captured an iteration
  (run 6 with 5s deadline + shared scrape timeout) that revealed the inner
  deadline / per-scrape-timeout interaction; the fix landed in run 7.
- **Two remaining @all failures are non-regressions.** US-06 and slice-7 GC
  count are pre-existing flakes that this DISTILL surfaced more loudly under
  faster runtime (post-argon2), not caused. Tracked in CONTEXT.md as the
  next deferred items.

## Permanent artefact locations

All artefacts stay in their delivery locations.
`docs/feature/slice-6-scenario-hardening/` has no inbound external
references. The DISTILL context flows downward into the test code at
`crates/foundry-acceptance/src/steps/handler_instrumentation.rs`
(`poll_until_sample` helper + rewritten step) +
`crates/foundry-acceptance/tests/features/handler-instrumentation.feature`
(re-worded Then, removed `When`).

Slice-6's ADR-012 (`docs/evolution/2026-05-26-handler-instrumentation.md`)
remains the source of truth for the gauge contract; this evolution doc
records only the test-side assertion-shape fix and the surrounding session's
sibling perf-flake commits.

## Open items for v0.2 RC

1. **US-06 timing-symmetry redesign.** The remaining @all flake. Argon2
   spawn_blocking CPU-pool contention from sibling scenarios' Background
   hashes causes Δ~1250ms vs the 500ms budget. Right fix is scenario
   redesign (interleaved N pairs with statistical compare), not a knob
   tweak. Owns the v0.2.0 cut gate alongside item 2.
2. **Slice-7 `comments_tombstones_purged_total` counter race.** Intermittent
   `expected 3, got 0`. Needs bounded-poll treatment analogous to this
   DISTILL — same shape, different metric. Could share infra: if a second
   caller materialises for `poll_until_sample`, promote it to
   `support/metrics_scrape.rs` per D2's deferred promotion plan.
3. **5 deferred metrics from handler-instrumentation D0** still need
   dashboard consumers + emission: `outbox_pending_jobs`,
   `bootstrap_tokens_unclaimed`, `migration_apply_duration_seconds`,
   `realtime_listen_disconnects_total`, `probe_failures_total`. Tracked in
   `docs/evolution/2026-05-26-handler-instrumentation.md` and
   `docs/evolution/2026-05-26-comment-tombstone-gc.md`.
4. **CHANGELOG.md + v0.2.0 tag.** Bundles slices 5+6+7 + this hardening +
   the three sibling perf-flake commits into one release-note section.
   Blocked by items 1 and 2.

## Workflow note

Per project convention, the 4-reviewer parallel gate is deferred to PR time
rather than invoked here.
