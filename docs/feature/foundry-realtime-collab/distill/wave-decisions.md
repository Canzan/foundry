# Wave Decisions — foundry-realtime-collab

DISTILL-wave decisions that gate DELIVER. Reviewed and approved
during the DISTILL gate (2026-05-24).

## Walking Skeleton Strategy: Strategy C — all real adapters

For every scenario tagged `@walking_skeleton`, the test exercises
production driving adapters and real driven adapters. There are NO
`@in-memory` fixtures, NO mock substitutes, and NO fake repositories
for the slice-2 walking skeletons:

- **Driving adapter**: real `reqwest` HTTP client → real `axum`
  binary spawned by `crates/foundry-acceptance/src/support/harness.rs::spawn_app`
  (per-scenario, in-process, listening on an ephemeral port).
- **Driven adapter**: real `sqlx::PgPool` → real Postgres provided by
  `testcontainers-rs` (one shared container; per-scenario schema
  rotation via `search_path` keeps scenarios isolated).
- **Realtime substrate**: real `LISTEN`/`NOTIFY` on a dedicated
  per-replica Postgres connection (set up by `spawn_app`); real
  `tokio::sync::broadcast` fan-out; real SSE response stream parsed
  by `crates/foundry-acceptance/src/support/sse_client.rs`.
- **Markdown rendering**: real `pulldown-cmark` + `ammonia`
  sanitization through the production composition root; assertions
  parse the returned HTML with `scraper`.

### Why Strategy C (vs A: all mocks, or B: mocks at the driven side)

- **JTBD outcome 4** ("Foundry feels as fast as Linear") only manifests
  end-to-end. A mocked driven adapter cannot prove that `pg_notify`
  + `LISTEN` + broadcast actually delivers within the 2-second NFR.
- The slice-1 harness already pays the testcontainers + spawn_app
  cost (~80ms per scenario). Reusing it costs nothing extra and
  guarantees slice-2 walking skeletons surface the same real-adapter
  failures the operator would see in production.
- The cost of substrate honesty is ~7.7s additional wall-clock for
  the slice-2 suite (one cross-project negative dominates with a
  ~2.5s zero-event wait). Total suite stays well under the 60s
  per-developer-laptop budget per `distill/driver.md`.

### What this strategy excludes

- `@in-memory` fixtures for any adapter the production code uses.
  If a future scenario needs `@in-memory`, it does NOT count as a
  walking skeleton (the walking-skeleton litmus test is that real
  production code paths run end-to-end).
- Browser-side JavaScript execution. The `@manual` browser drill in
  `features/us-12-keyboard-nav.feature` (the user-keyboard-to-issue-
  opens journey) is THE walking skeleton for US-12; the automated
  US-12 scenarios are server-contract tests that pin the data the
  alpine.js handler depends on.

## Open Decisions Resolved During Review

| Decision | Resolution | Locked in |
|----------|------------|-----------|
| SSE heartbeat env var name | `SSE_HEARTBEAT_MS` + per-scenario override via `override_heartbeat_ms(ms)` | `distill/driver.md` §2c |
| Comment-event payload shape | Extend with `author_email` (no JOIN at fan-out time) | DELIVER must implement |
| SSE event format compatibility | Single `event:` type per channel (`issue_events`); payload JSON carries `event_type` + `schema_version: 1` to allow forward-compatible additions | `design/realtime-roadmap.md` (inherited) |
| `eventsource-client` crate adoption | Rejected — roll a ~80-line custom SSE line parser to avoid pulling in a second TLS stack | `distill/driver.md` §custom-parser |
| Cross-project negative wait optimization | Defer — 2.5s wait is the only way to assert zero events without false-negative risk | revisit only if CI suite exceeds budget |

## DELIVER Pre-flight Checklist

DELIVER must satisfy these before merging:

- [ ] `crates/foundry-acceptance/src/support/sse_client.rs` exists and exposes
      `open_sse_subscription(...)`, `read_event_with_timeout(...)`, and
      `override_heartbeat_ms(ms)`.
- [ ] All 3 walking skeletons (US-09 line 44, US-10 line 44, US-12 `@manual`
      drill) execute against real Postgres + real axum.
- [ ] Comment-event payload includes `author_email`.
- [ ] `@nfr-perf-03` scenario measures wall-clock and asserts ≤ 2s p99 per event.
- [ ] `@nfr-sec-05` scenario sends a real `<script>alert('xss')</script>` body
      and asserts the rendered HTML contains zero `<script>` elements.
- [ ] No scenario regresses slice-1's 37/37 green state.
