# Mutation Testing Report — multi-workspace-provisioning

**Phase 5 quality gate** (mutation testing on the feature's NEW production code).
**Date:** 2026-06-12
**Tool:** cargo-mutants 25.3.1
**Diff scope:** `949e8f1..HEAD` (feature production lines only)
**Gate:** ≥80% kill rate on the mutated scope.

---

## Verdict

| Scope | Mutants (viable) | Caught | Survived | Unviable | Kill rate | Gate |
|---|---|---|---|---|---|---|
| **TIER 1 — `foundry-app/src/rate_limit.rs`** | 20 | 20 | 0 | 1 | **100%** | PASS |
| **TIER 2 — `foundry-store/src/lib.rs`** (new fns) | 7 | 7 | 0 | 0 | **100%** | PASS |
| **Services use-case — `foundry-services/src/lib.rs`** | 1 | 1 | 0 | 2 | **100%** | PASS |
| **TIER 3 — `foundry-app/src/{admin_cli,main}.rs`** | — | — | — | — | EXCLUDED (documented) | n/a |
| **OVERALL (mutated scope)** | **28** | **28** | **0** | **3** | **100%** | **PASS (≥80%)** |

**Overall: 28/28 viable mutants killed = 100% kill rate. Gate PASS (≥80%).**

Five survivors found during the initial passes were all killed by adding focused
behavioral tests (no production code changed). After those tests, zero survivors
remain across every measured tier.

---

## Scope, tiering, and exclusions

cargo-mutants only mutates Rust source. The feature diff `949e8f1..HEAD` was
restricted to production `.rs` files (NOT tests, `.md`, `.sql`) via
`git diff 949e8f1..HEAD -- <prod files>`, then split per cost-tier so each pass
runs the cheapest test command that genuinely covers the mutated lines.

### TIER 1 — `crates/foundry-app/src/rate_limit.rs` (MUTATED)
The genuinely-new eviction algorithm (idle + LRU, behaviour-preservation,
ADR-005) plus the token-bucket arithmetic. Covered by fast in-crate unit +
proptest tests (no testcontainers). Highest mutation value.

### TIER 2 — `crates/foundry-store/src/lib.rs` new functions (MUTATED)
`is_instance_admin`, `user_id_by_email`, `provision_workspace`,
`grant_instance_admin`, and the `create_initial_workspace` `instance_admins`
seed. Covered DIRECTLY by the store integration tests (testcontainers, no
acceptance lane).

### Services use-case — `crates/foundry-services/src/lib.rs` (MUTATED)
`provisioning::provision_workspace` + the `Services::provision_workspace`
delegating wrapper. The security-critical fail-closed authz gate
(`if !is_admin { Err(Forbidden) }`) lives here. Was acceptance-only before this
gate run; a focused services integration test was added so it is now covered by
a fast (testcontainer, no bin) test — so it was mutated rather than excluded.

### TIER 3 — `crates/foundry-app/src/admin_cli.rs` + `main.rs` (EXCLUDED, documented)
**Excluded functions:** `run_provision_workspace`, `run_grant_super_admin`,
`generate_provisioning_password` (admin_cli.rs); the `provision-workspace` /
`grant-super-admin` subcommand dispatch arms (main.rs).

**Why excluded:** these are CLI plumbing covered ONLY by the bin-driven
acceptance lane, which exercises the `foundry` BINARY via
`assert_cmd::cargo_bin`. Per project memory, cargo-mutants must REBUILD the
`foundry-app` binary (release LTO) for each mutant or mutants falsely SURVIVE.
Bin-rebuild (minutes, release LTO `codegen-units=1`) × testcontainer spin-up ×
the cucumber acceptance lane per mutant × ~15+ CLI mutants = multiple hours —
intractable for a timed quality gate. They also consist largely of env-var
reads, `std::thread` + tokio-runtime scaffolding (copied verbatim from the
already-shipped `run_restore_comment`), and arg parsing.

**Their behavior IS exercised** by the slice-06 acceptance scenarios
(`us-mwt-slice-06-provision-and-prove.feature`), which drive the real `foundry`
binary end-to-end:
- "A super-admin provisions a new isolated workspace with a first admin" (happy path → exit 0, prints workspace id + invite link)
- "A non-super-admin cannot provision a workspace" (authz refusal → distinct non-zero exit)
- "An unauthorized provisioning attempt does not reveal whether the target exists" (non-enumerability)
- "The bootstrap-claiming operator is the first super-admin and can provision"
- "An upgraded install grants its first super-admin and can then provision" (grant-super-admin path)
- "Provisioning is unreachable from the bearer API surface"

The CORE provisioning logic the CLI delegates to (`provisioning::provision_workspace`,
the authz gate, and all store functions) IS mutated above at 100% — the CLI
layer is a thin adapter over already-mutation-covered seams.

---

## cargo-mutants invocations

All runs used `--in-diff <prod-only diff>` so only feature-changed production
lines are mutated.

**Tier 1 (debug profile — fast, deterministic logic tests):**
```
cargo mutants --in-diff /tmp/mwp-rate_limit-prod.diff \
  --package foundry-app --test-package foundry-app
```
(`--test-package foundry-app` rebuilds the foundry-app crate so in-crate
`rate_limit` unit + proptest tests run. The release profile was tried first but
each LTO rebuild was ~2 min/mutant; debug yields identical verdicts for the
deterministic bucket/eviction logic at a fraction of the cost.)

**Tier 2 (testcontainers — serialized to avoid Postgres-pool exhaustion):**
```
RUST_TEST_THREADS=1 cargo mutants --in-diff /tmp/mwp-store.diff \
  --package foundry-store --test-package foundry-store --jobs 1 --timeout 240
```
(`RUST_TEST_THREADS=1` + `--jobs 1` serialize container startup; an early
unserialized baseline hit `PoolTimedOut` from 8 concurrent Postgres containers.)

**Services use-case (testcontainers, serialized):**
```
RUST_TEST_THREADS=1 cargo mutants --in-diff /tmp/mwp-services.diff \
  --package foundry-services --test-package foundry-services --jobs 1 --timeout 240
```

---

## Per-tier results

### TIER 1 — rate_limit.rs: 20 caught / 0 survived / 1 unviable → 100%

21 mutants generated, 20 viable. All 20 caught after fixing 2 initial survivors.

**Unviable (excluded from kill rate):**
- `consume -> RateDecision with Default::default()` — `RateDecision` has no `Default`; does not compile.

**Initial survivors → fixed (2):**

1. **`rate_limit.rs:212: replace + with -` in `consume`** (LRU overflow count).
   The overflow count is `buckets.len() + 1 - self.max_principals`. The `+→-`
   mutant makes it the `usize` expression `len() - 1 - max_principals`, which
   underflows at the cap boundary (`len() == N`) and wraps to a huge value,
   evicting the WHOLE map down to ~1 each time the cap is hit. The existing
   `lru_size_cap_bounds_the_map_and_only_relaxes` proptest only asserts the
   UPPER bound (`bucket_count() <= cap`), which an over-eviction still satisfies.
   **Reveals:** no lower-bound coverage on the steady-state map size — eviction
   could collapse the map far below `N` and the suite wouldn't notice.
   **Killed by added test** `lru_eviction_is_minimal_map_settles_exactly_at_the_cap`:
   drives `>> N` distinct principals at one instant and asserts the map settles
   at EXACTLY `N` (minimal eviction), not below it.

2. **`rate_limit.rs:163: replace / with *` (and `/ with %`) in `idle_window_secs`**
   The idle window is `W = ceil(C / R)`. Every existing eviction test uses
   `R = 1.0`, where `C/R == C*R` (and the ceil collapses `%` too), so the
   arithmetic-operator mutants on `C / R` are indistinguishable there.
   **Reveals:** the window formula was only ever exercised at `R = 1`, so a
   wrong operator on the `C/R` expression would ship silently for any other rate.
   **Killed by added test** `idle_window_is_ceil_capacity_over_refill_rate`:
   pins `W` with `R != 1` and a non-integer ratio (C=10, R=4 → `ceil(2.5)=3`,
   where `*`→40 and `%`→2 diverge), the shipped default (C=20, R=1 → 20), and the
   `R <= 0` "never idle-evict" guard (→ `u64::MAX`).

### TIER 2 — store new functions: 7 caught / 0 survived / 0 unviable → 100%

7 mutants generated, all viable. All 7 caught after fixing 3 initial survivors.

**Initial survivors → fixed (3):**

1. **`lib.rs:1194: user_id_by_email -> Ok(None)`**
2. **`lib.rs:1194: user_id_by_email -> Ok(Some(Default::default()))`**
3. **`lib.rs:1224: provision_workspace -> Ok(())`** (skips the whole transaction)

   **Reveals:** `user_id_by_email` and `provision_workspace` had NO direct
   foundry-store test — they were exercised only indirectly by foundry-services
   and the bin-driven acceptance lane, so the store-scoped suite couldn't see a
   regression in either.
   **Killed by added test file** `crates/foundry-store/tests/provision_workspace_store.rs`:
   - `user_id_by_email_resolves_a_stored_user_to_their_exact_id` — asserts the
     lookup returns the EXACT stored id (not None, not the nil/default UUID) and
     `None` for an absent email; kills both `user_id_by_email` mutants.
   - `provision_workspace_atomically_creates_workspace_admin_membership_and_invite`
     — asserts all four provisioned rows (workspace, admin user, admin
     membership, invite) exist under the passed ids and that exactly one
     additional workspace was created; kills the `Ok(())` no-op mutant.

**Caught from the start (4):** `is_instance_admin -> Ok(true)`,
`is_instance_admin -> Ok(false)` (both arms pinned by `is_instance_admin_authz`),
`grant_instance_admin -> Ok(())` (pinned by `grant_super_admin_idempotent`),
`create_initial_workspace -> Ok(())` (the whole claim tx is depended on by many
existing tests).

### Services use-case: 1 caught / 0 survived / 2 unviable → 100%

3 mutants generated, 1 viable.

**Unviable (excluded from kill rate):**
- `Services::provision_workspace -> Ok(Default::default())`
- `provisioning::provision_workspace -> Ok(Default::default())`
  Both require `Provisioned: Default`, which is not implemented → do not compile.

**Caught (1) — the security-critical one:**
- **`lib.rs:235: delete ! in provisioning::provision_workspace`** — inverts the
  fail-closed authz gate (`if !is_admin` → `if is_admin`), which would let a
  NON-super-admin provision and refuse a real super-admin.
  **Killed by added test file** `crates/foundry-services/tests/provision_workspace_use_case.rs`:
  - `non_super_admin_is_refused_fail_closed_and_no_workspace_is_created` — a
    non-super-admin actor gets `ServiceError::Forbidden` and creates no
    workspace; the gate inversion would return `Ok` and create one.
  - `super_admin_provisions_a_new_workspace_and_its_ids_resolve_to_real_rows` —
    the authorized path commits a real workspace whose ids resolve to real rows.

---

## Tests added to kill survivors (no production code changed)

| File | Test | Kills |
|---|---|---|
| `crates/foundry-app/src/rate_limit.rs` (test module) | `lru_eviction_is_minimal_map_settles_exactly_at_the_cap` | `consume` `+→-` (LRU over-eviction) |
| `crates/foundry-app/src/rate_limit.rs` (test module) | `idle_window_is_ceil_capacity_over_refill_rate` | `idle_window_secs` `/→*`, `/→%` |
| `crates/foundry-store/tests/provision_workspace_store.rs` (new) | `user_id_by_email_resolves_a_stored_user_to_their_exact_id` | `user_id_by_email` `Ok(None)`, `Ok(Some(Default))` |
| `crates/foundry-store/tests/provision_workspace_store.rs` (new) | `provision_workspace_atomically_creates_workspace_admin_membership_and_invite` | `provision_workspace` `Ok(())` |
| `crates/foundry-services/tests/provision_workspace_use_case.rs` (new) | `non_super_admin_is_refused_fail_closed_and_no_workspace_is_created` | `provision_workspace` `delete !` (authz gate inversion) |
| `crates/foundry-services/tests/provision_workspace_use_case.rs` (new) | `super_admin_provisions_a_new_workspace_and_its_ids_resolve_to_real_rows` | re-confirms the authorized path |

All added tests are port-to-port (driving-port signatures / `Services` driving
port → driven-port observable rows), assert observable outcomes (never internal
structure), and pass against the unmutated production code. No production source
was modified — the only production-file edit is additions inside the
`#[cfg(test)] mod tests` block of `rate_limit.rs`.

## Quality

- `cargo fmt --all --check` — clean.
- `cargo clippy --all-targets --release -p foundry-app -p foundry-store -p foundry-services -- -D warnings` — clean.
- Working tree after the run: only the report + the three test changes; all
  production logic byte-identical to `HEAD`; no source left mutated.
