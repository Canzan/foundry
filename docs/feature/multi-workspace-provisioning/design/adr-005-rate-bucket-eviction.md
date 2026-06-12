# ADR-005 — Rate-bucket map eviction (residual F2)

## Status
**IMPLEMENTED** (2026-06-12; ratified 2026-06-11). Shipped in DELIVER phase 05 (step 05-01):
std-only idle (`W=ceil(C/R)s`) + LRU size-cap eviction on `RevokeRateLimiter`, keyed off the
shipped clock; mutation-hardened to 100%. **Closes the F2 residual** carried by
`token-management-api` / `machine-token-admin-ux` and named in the parent `multi-workspace-tenancy`
DM8. See the evolution doc.

## Context
`RevokeRateLimiter` (`rate_limit.rs:103`) holds `buckets: Mutex<HashMap<Uuid, BucketState>>`,
keyed by the bound `user_id` of `Principal::Machine`. It has NO eviction: an entry is created on a
principal's first revoke and lives for the process lifetime. Under the single-workspace model the
keyspace was bounded by the count of workspace admins (O(dozens)) — an accepted residual. With
multi-workspace now shipped, the principal population grows with tenants, so the map must be
bounded WITHOUT weakening throttle correctness for active principals (NFR-MWT-PERF-01).

Grounding (read the code, including the module's own residual note, `rate_limit.rs:30-47`):
- The module ITSELF documents the behaviour-preserving eviction rule: *"a bucket idle longer than
  `C / R` seconds has fully refilled to `C` and is indistinguishable from a fresh one, so eviction
  is behaviour-preserving."* A freshly-`or_insert`ed bucket starts at full capacity
  (`tokens: capacity`, `:152`), so re-creating an evicted-because-idle bucket yields the identical
  state it would have had.
- `consume` (`:147`) already takes `now: time::OffsetDateTime` from the shipped clock seam
  (`ClockedRevokeGuard`, `:178`) — deterministic under `MockClock`. Eviction can read the SAME
  `now` with no new time source.
- The module is 100%-mutation-hardened with a tight behaviour budget; the eviction must preserve
  that discipline (a behaviour-preserving change, plus a bounded-map property test).

## Options considered
- **(a) Idle eviction keyed off the shipped clock (std-only).** On each `consume`, opportunistically
  drop buckets whose `last_refill_unix_nanos` is older than an idle window `W = ceil(C / R)` seconds
  (the refill-to-full horizon). Because an idle-evicted bucket re-creates at full `C`, throttle
  correctness for any principal that returns is identical to never having evicted. Pure `std`
  (`HashMap::retain` over the existing `Mutex`). Zero new crate.
- **(b) LRU size-cap (std-only).** Cap the map at `N` entries; evict the least-recently-used on
  insert. Bounds the map by a hard size rather than by idleness. Needs an LRU ordering — doable in
  `std` (e.g. a small intrusive order or periodic prune), but more moving parts, and evicting a
  NON-idle bucket (under cap pressure) is NOT behaviour-preserving (a throttled principal whose
  bucket is evicted resets to full `C`, briefly relaxing its cap).
- **(c) A new crate (`lru`, `dashmap`, `moka`).** REJECTED: violates the zero-new-crate constraint
  inherited from the parent; the policy is simple enough for `std`; adds a dependency to a hardened
  module for no benefit over (a).

## Decision
**(a) Idle eviction keyed off the shipped clock, std-only, with an (b) LRU size-cap as a bounded
fallback.** Concretely:
- The PRIMARY policy is idle eviction: a bucket idle longer than `W = ceil(C / R)` seconds
  (default `ceil(20/1) = 20s`, derived from the existing `DEFAULT_REVOKE_BUCKET_*` constants, not a
  new magic number) is evicted opportunistically during `consume` via `buckets.retain(|_, b|
  now - b.last_refill <= W)`. This is behaviour-preserving by the module's own documented rule.
- A SECONDARY hard size-cap `N` (a named, DESIGN-tunable constant, e.g. 10_000) bounds the map even
  in the pathological "many distinct principals all active within `W`" case: if the map exceeds `N`
  after the idle sweep, evict the entries with the oldest `last_refill` down to `N`. This fallback
  is rare and, when it fires, the worst case is a returning principal resetting to full `C` (a
  relaxation, never a tightening — it can never wrongly throttle a legitimate principal).
- NO new crate: `std::collections::HashMap::retain` + the SHIPPED clock. The `user_id` key,
  the token-bucket math, the metric, and the `RevokeRateGuard` port are UNCHANGED.

## Consequences
- **Positive**: the map is bounded by ACTIVE principals (idle window) with a hard ceiling (size
  cap); throttle correctness for active principals is provably unchanged (idle eviction is
  behaviour-preserving; the cap fallback only ever relaxes); zero new crate; the change is local to
  one module and preserves its 100%-mutation discipline.
- **Negative**: the size-cap fallback's eviction of a still-throttled principal (only under
  pathological load) briefly relaxes that principal's throttle — an explicitly-accepted, bounded,
  one-directional (never over-throttle) trade-off; documented so it is not a surprise.
- **Verification (Earned Trust)**: a property/unit test (mirroring the shipped `rate_limit` tests,
  driven by `MockClock`) asserts (i) the map size stays bounded under a workload spanning many
  idle + active principals, and (ii) throttle correctness for an ACTIVE principal is byte-identical
  with and without eviction (the behaviour-preserving claim is probed, not assumed).

## Relationship to prior decisions
Closes residual F2 exactly as the parent DM8 / NFR-MWT-PERF-01 specified (LRU/idle eviction,
`user_id`-keyed, bounded under many tenants). Honours the rate-guardrail design's clock seam and
the zero-new-crate constraint.
</content>
