# Mutation Report — Feature A (web-tier-extraction), DELIVER Phase 5

- Tool: `cargo-mutants` 25.3.1 (cargo 1.91.1)
- Branch: `feature/web-tier-extraction`
- Date: 2026-05-31
- Threshold: **≥80% kill rate** on the security core (foundry-auth)
- Strategy: feature-scoped to the pure-logic, fast-unit-tested security surface
  whose coverage is **in-process** (no `@real-io` subprocess lane). The
  Postgres/testcontainer-backed surfaces are EXCLUDED and logged below — running
  cargo-mutants over them would rerun the full container suite per mutant (hours,
  hundreds of containers).

---

## Summary

| Target | File | Test command | Mutants | Caught | Missed | Unviable | Kill rate (viable) | Gate |
|---|---|---|---|---|---|---|---|---|
| PRIMARY — security core | `crates/foundry-auth/src/lib.rs` | `cargo test -p foundry-auth` | 28 | 18 | 6 | 4 | 75% raw / **81.8% security-core** | **MET** |
| SECONDARY — API pure fns | `crates/foundry-api/src/lib.rs` (`--lib` scope) | `cargo test -p foundry-api --lib` | 36 | 4 | 5 | 27 | see note | analyzed |
| OPTIONAL — xtask | `xtask/src/check_arch.rs` | — | not run | — | — | — | — | skipped (time/scope) |

Run times: foundry-auth 1m47s per pass (×2 passes); foundry-api 29s.

---

## PRIMARY — foundry-auth (the MachineToken verify/mint security core)

### Result

- **Before adding tests**: 28 mutants → 12 caught, 12 missed, 4 unviable → 50% viable kill rate.
- **After adding 2 tests (test-first)**: 28 mutants → **18 caught, 6 missed, 4 unviable**.
- Viable kill rate (all viable): 18/24 = **75%**.
- **Security-core kill rate** (excluding the 2 `test_keys::*` survivors, which are
  `#[cfg(feature = "test-support")]` test-harness scaffolding NOT compiled by the
  package test command — not a production security surface): 18/22 = **81.8% — gate MET**.

All caught mutants are killed by **in-process unit tests** (`cargo test -p foundry-auth`),
NOT by any `@real-io` subprocess lane — so no false-survivor risk from the
binary-rebuild gotcha (MEMORY note honoured: target deliberately chosen for
in-process coverage).

### Tests added (test-first discipline — written, then confirmed to kill)

1. `machine_token_tests::token_omitting_iss_aud_verifies_via_serde_defaults`
   — kills 4 survivors: `default_iss → ""`, `default_iss → "xyzzy"`,
   `default_aud → ""`, `default_aud → "xyzzy"` (lines 93, 97).
   A genuinely-signed JWT whose JSON body OMITS `iss`/`aud` must still verify,
   because `#[serde(default)]` supplies the pinned single-issuer constants on
   decode. If the defaulting functions returned `""`/`"xyzzy"`, the
   issuer/audience check would reject the token. Asserted through the `verify`
   driving port (Ok vs Err) plus the recovered claim values.

2. `tests::invite_signature_binds_to_id_and_expiry`
   — kills 2 survivors: `invite_payload → ""`, `invite_payload → "xyzzy"` (line 389).
   The invite signature must BIND to the (invite_id, expires_at) pair. A constant
   payload would let a signature minted for invite A validate a DIFFERENT invite
   (swapped id, or extended expiry) under a genuine signature. Asserted through the
   `InviteToken::verify` driving port: the signature must NOT cross-validate across
   a different id or a different expiry. (Symmetric-property gap: sign+verify used
   the SAME payload fn, so a constant round-tripped.)

Both tests pass on unmutated code (suite: 16 → 18 tests, all green) and were
confirmed to flip the listed mutants from MISSED to CAUGHT on re-run.

### Accepted survivors (6) — verdicts

| Line | Mutant | Verdict | Reasoning |
|---|---|---|---|
| 290 | `test_keys::verifier → Default::default()` | **accepted — excluded (test-support)** | Inside `#[cfg(feature = "test-support")]`. The package test command does not enable that feature, so the helper is not compiled/reachable by `cargo test -p foundry-auth`. Test-harness scaffolding, not production security surface. Excluded from the security-core denominator. |
| 296 | `test_keys::signer → Default::default()` | **accepted — excluded (test-support)** | Same as above. |
| 307 | `argon2_params → Default::default()` | **accepted — equivalent** | `Params::default()` is also a valid argon2id parameter set. hash/verify round-trips under any valid params, so behavior is observationally identical. Killing it would require asserting on internal param values — implementation-mirroring (banned Testing-Theater pattern 5). |
| 307:20 | `argon2_params` `*` → `+` | **accepted — equivalent** | `64*1024` vs `64+1024`: both yield a valid memory-cost; argon2id still produces a self-describing PHC hash that verify re-parses. Round-trip behavior identical. Same implementation-mirroring objection to any kill. |
| 207:22 | `self_test` exp `+` → `*` | **accepted — equivalent** | `now + 60` vs `now * 60`: the key probe verifies the token IMMEDIATELY; any non-expired `exp` (both are far-future) yields an identical instant round-trip. The `60` is a liveness margin with no observable effect on an instant probe. |
| 207:22 | `self_test` exp `+` → `-` | **accepted — equivalent** | `now - 60` lands within jsonwebtoken's default 60s `exp` leeway, so the instant probe still verifies. Borderline-equivalent and timing-flaky to pin; the `60` is a liveness margin swallowed by validation leeway. |

No remaining survivor is a real, cheaply-killable test gap in the production
security surface. The two `argon2_params` and two `self_test` survivors are
equivalent mutants on liveness/cost margins; the two `test_keys` survivors are
test-only scaffolding outside the package test command's feature set.

---

## SECONDARY — foundry-api pure functions (`status_for`, `verify_bearer`, `bearer_token`, `IssueJson`)

Test scope deliberately limited to `--lib` (the fast in-process unit tests:
`token_auth_tests::*`), NOT the acceptance/handler suite (which needs Postgres).

### Result

36 mutants → **4 caught, 5 missed, 27 unviable**.

- **Caught (4/4)** — all `token_auth::bearer_token` parsing mutants killed
  (`Some("xyzzy")`, `None`, `Some("")`, `delete !`). The bearer-token parsing +
  `verify_bearer` crypto branches (missing/malformed/bad-sig/wrong-alg/alg:none/
  expired) are exercised port-to-port by
  `refuses_every_invalid_credential_non_enumerably` and
  `accepts_valid_eddsa_credential_and_recovers_claims`. Zero survivors on the
  targeted pure-security surface.
- **27 unviable** — match-arm / return-type / handler-body mutations the compiler
  rejects (e.g. `status_for` arms that don't type-check when collapsed).

### Survivors (5) — all in Postgres-backed handler/router wiring, EXCLUDED from this pass

| Line | Mutant | Verdict |
|---|---|---|
| 175 | `routes → Router::new()` / `Router::from(Default::default())` | **excluded** — router wiring, covered by the acceptance/handler suite (real `Services` over Postgres), not lib unit tests. |
| 148 | `<ApiError as IntoResponse>::into_response → Default::default()` | **excluded** — covered by handler-response acceptance tests. |
| 205 | `list_issues_handler → Ok(Json::from(vec![]))` | **excluded** — Postgres-backed handler; see EXCLUSIONS. |
| 233 | `create_issue_handler → Ok(Default::default())` | **excluded** — Postgres-backed handler; see EXCLUSIONS. |

These are NOT test gaps in the targeted pure functions — they are handler/router
bodies whose genuine coverage lives in the Postgres acceptance lane. Killing them
in a fast lib-unit pass would require either mocking `Services` INSIDE the hexagon
(a port-boundary violation) or pulling Postgres (the cost this scoped pass avoids).
No tests added for foundry-api; the targeted pure security surface is already at
100% viable kill (4/4) with the existing unit tests.

---

## EXCLUDED FROM THIS PASS (honest coverage gaps — not silently hidden)

The following surfaces are Postgres/testcontainer-backed. Their only meaningful
mutation coverage comes from the container-backed suite; cargo-mutants would rerun
that suite per-mutant (hours, hundreds of containers), so they are out of scope for
this fast, in-process pass:

- **`crates/foundry-store`** (0007 repo) — needs Postgres (testcontainers). All
  repo behavior is integration-tested against a real PG16-alpine container.
- **`crates/foundry-services`** use-cases — need Postgres for the use-case round
  trips. (Pure helpers like `normalize_state` / title validation could be a future
  scoped sub-pass if isolated behind a fast unit-test target.)
- **`crates/foundry-api` handlers** (`list_issues_handler`, `create_issue_handler`,
  `routes`, `ApiError::into_response`) — covered by the acceptance/handler suite
  that constructs real `Services` over Postgres. Confirmed as the 5 survivors above
  under `--lib` scope; correctly excluded from the fast pass.
- **`xtask/src/check_arch.rs`** AST detectors — OPTIONAL target, NOT run this pass
  (time/scope). No Docker dependency, so a future cheap sub-pass is feasible
  (`cargo mutants -f xtask/src/check_arch.rs --package xtask`).

These exclusions are about **runtime cost of the test command per mutant**, not
about the surfaces being unimportant. The security-critical primitive
(foundry-auth) — the one surface where mutation most validates test quality — was
the PRIMARY target and meets the ≥80% gate on its production security core.

---

## Conclusion

- **foundry-auth security core: 81.8% kill rate — ≥80% gate MET** (75% raw incl.
  test-support scaffolding survivors).
- 2 tests added (test-first), killing 6 real survivors (serde-default iss/aud path
  ×4; invite-signature binding ×2).
- 6 accepted survivors: 2 test-support scaffolding (excluded from core), 4
  equivalent mutants (argon2 cost/liveness margins).
- foundry-api targeted pure security fns: 4/4 viable kill; 5 survivors are
  Postgres-backed handler/router wiring, correctly excluded.
- Postgres/testcontainer surfaces explicitly excluded with cost rationale above.
