# Evolution — all-lane-ssl-flake-fix

**Finalized**: 2026-05-30
**Ship commit**: [7ff7591](../../) — "fix(test): eliminate @all PoolTimedOut/SSLRequest flake (disable TLS probe)"
**Wave coverage**: test-infrastructure hardening (no formal wave; follow-on to
`acceptance-all-lane-stabilization`, which reduced this flake but did not
eliminate it).

## Feature summary

Eliminates the residual intermittent `@all` acceptance-lane flake — `PoolTimedOut`
(and, rarely, `unexpected response from SSLRequest: 0x00`) on the Background seed
inserts (e.g. `insert admin user`). The prior stabilization (`fc3aa94`) raised the
container connection ceiling and `@serial`-tagged the load generator, taking the
flake from ~1/3 to ~1/5 but never to zero. This change takes it to **0** and proves
causality with a controlled A/B.

## Root cause

The shared `Postgres::default()` testcontainer serves **no TLS**, but the harness
connection strings set no `sslmode`. sqlx therefore defaulted to `sslmode=prefer`
and performed an `SSLRequest` probe on **every** connection. Under the `@all`
connect-storm (6 concurrent scenarios each establishing per-scenario pool
connections at Background start), that probe intermittently read a garbage byte
(`unexpected response from SSLRequest: 0x00`), failing connection establishment.
The starved in-process harness pool (`fresh_schema_pool_with_url`, `max=10`,
`acquire_timeout=5s`) then surfaced the failure downstream as `PoolTimedOut` on the
first seed insert. Both observed flake signatures traced to this one pool — the SSL
probe was the establishment-time root; `PoolTimedOut` was its acquire-time shadow.

## Fix

Test-infrastructure only (`crates/foundry-acceptance/src/support/harness.rs`);
**production code untouched**:

- New `pg_options(base)` helper sets `ssl_mode(PgSslMode::Disable)` on the shared
  container connections — removes the wasted probe and its garbage-byte failure
  mode (correct: the local container has no TLS). Applied to both per-scenario pools
  and the admin/`drop_schema` connects.
- Pool `acquire_timeout` raised `5s → 30s` to absorb transient connect-storm spikes.

## Verification

- **5 consecutive release-mode `@all` sweeps green** (123/123 each) with the fix,
  cleaning leaked testcontainers between sweeps to remove that confound.
- **Controlled A/B (the causal proof)** — same harness, same between-sweep cleanup,
  fix toggled:

  | Harness | `@all` sweeps | Failures |
  |---|---|---|
  | **Fixed** (`ssl_mode=Disable`, `acquire_timeout=30s`) | 5 | **0/5** |
  | **Reverted** (pre-fix `prefer` + 5s) | 5 | **3/5** — all `PoolTimedOut` on the same `malformed UUID … invalid-argument exit code` Background |

  Reverting the two lines reintroduced the exact pre-fix failure at a ~60% rate;
  restoring them returned it to 0. The fix is load-bearing.

## Lessons learned

1. **A flake fix cannot be mutation-tested.** The changed surface
   (`ssl_mode(Disable)` argument, `from_secs(30)` literal) is invisible to
   cargo-mutants' operators (it mutates function returns + binary ops, not call
   args or literals), and the harness is test apparatus, not code-under-test.
   Moreover the effect is *probabilistic* — a single-shot mutant run cannot detect
   "reintroduces a 1-in-5 flake." The correct analog of killing a mutant is a
   **differential A/B**: revert the fix and confirm the failure returns.
2. **`sslmode` defaults bite local test containers.** sqlx's `prefer` does a real
   TLS probe even against a plaintext server; under connection storms the probe is
   both pure latency and an intermittent failure surface. Pin `sslmode=disable` for
   no-TLS test databases.
3. **`PoolTimedOut` is often a connection-establishment symptom, not a pool-size
   one.** The ceiling was already 300; the timeouts came from probe-time failures
   starving the pool, not from exhausting connections.
