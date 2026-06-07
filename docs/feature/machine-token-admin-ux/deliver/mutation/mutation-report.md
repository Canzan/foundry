# Mutation Report — machine-token-admin-ux (DELIVER Phase 5)

**Tool:** `cargo-mutants` 25.3.1
**Target (security core):** `crates/foundry-services/src/tokens.rs`
**Test command:** `cargo test -p foundry-services` (real Postgres testcontainer, in-process — NOT the subprocess-spawned `foundry` bin, so the known @real-io false-survivor rebuild caveat does not apply)
**Run command:** `cargo mutants --package foundry-services --file crates/foundry-services/src/tokens.rs --test-tool=cargo -- -p foundry-services`
**Date:** 2026-06-07

## Result — gate MET

| File | Mutants found | Unviable | Viable | Caught | Survived | Kill rate (viable) |
|------|--------------:|---------:|-------:|-------:|---------:|-------------------:|
| `crates/foundry-services/src/tokens.rs` | 18 | 3 | 15 | 15 | 0 | **100%** |

**≥80% gate on `tokens.rs`: MET (100%).**

Baseline (before this pass): 18 found, 3 unviable, 15 viable, 10 caught, **5 survived → 66.7%**.
After adding 3 tests: 15/15 viable caught → **100%**.

## Survivors found in baseline → all now killed (real gaps, not equivalent)

All 5 baseline survivors were whole-function / helper replacement mutants in the three
use-cases that had NO in-process test (`revoke_token`, `list_tokens`, `resolve_team_name`).
Each represented a genuine authz/correctness gap, not an equivalent mutant.

| # | Mutant | Why it is a real bug | Killing test |
|---|--------|----------------------|--------------|
| 1 | `tokens.rs:276 revoke_token -> Ok(())` | Skips the store write entirely — a "successful" revoke that never flips `revoked_at`; the kill-switch silently does nothing. | `revoke_flips_revoked_at_and_foreign_jti_is_non_enumerable_notfound` asserts `revoked_at` transitions NULL→set after revoke. |
| 2 | `tokens.rs:220 list_tokens -> Ok(vec![])` | Returns an empty list regardless of registry contents — the admin never sees issued credentials (and the status/`minted_by` derivation is dead). | `list_returns_workspace_tokens_with_status_and_minted_by` asserts exactly the two minted tokens appear with derived `revoked` status and `minted_by` resolved from `created_by`. |
| 3 | `tokens.rs:201 resolve_team_name -> Ok(None)` | A team-scoped grant loses its team name on the display label (DD9). | `scope_team_name_resolves_...` asserts a `Team(team_id)` grant resolves `scope_team_name == Some("Backend")`. |
| 4 | `tokens.rs:201 resolve_team_name -> Ok(Some("xyzzy"))` | Renders a wrong/garbage team name. | same test — asserts the SPECIFIC real name `"Backend"`, not just "some name". |
| 5 | `tokens.rs:201 resolve_team_name -> Ok(Some(""))` | Renders a blank team name. | same test — asserts non-empty `"Backend"`. |

## Caught in baseline (security/correctness logic already covered — no action needed)

The load-bearing security gates were already killed by the existing
`mint_token_use_case.rs` suite:

- `103:8 delete ! in mint_token` — admin authz gate (US-MT05).
- `108:23 replace <= with > in mint_token` — `ttl_required` lower bound.
- `114:23 replace > with ==` / `>=` / `<` in mint_token — `ttl_over_cap` upper bound + the `== MAX` at-cap boundary.
- `130:16 delete ! in mint_token` — team-belongs-to-workspace scope check.
- `144:26 replace + with - in mint_token` — `expires_at = now + ttl` computation.
- `224:8 delete ! in list_tokens` — list admin authz gate.
- `280:8 delete ! in revoke_token` — revoke admin authz gate.
- `293:8 delete ! in revoke_token` — workspace-isolation `!belongs` (non-enumerable NotFound).

The 3 newly-added tests additionally re-exercise the revoke admin gate, the
`!belongs` branch (via the unknown-jti → `None` → NotFound path), and the
list/revoke happy paths, hardening these beyond the baseline.

## Unviable mutants (do not compile — correctly ignored by cargo-mutants)

| Mutant | Why unviable |
|--------|--------------|
| `99:5 mint_token -> Ok(Default::default())` | `MintedToken` has no `Default` impl (it holds a `SecretString`, `Uuid`, `OffsetDateTime`). |
| `220:5 list_tokens -> Ok(vec![Default::default()])` | `TokenView` has no `Default` impl. |
| `144:26 replace + with * in mint_token` | `OffsetDateTime * Duration` does not type-check. |

These are compile failures, not test gaps. No accepted-equivalent survivors remain.

## Tests added (3 — test-first, all green on real code, all kill their target survivor)

New sibling file (follows the per-use-case convention of `board_use_case.rs` /
`write_use_cases.rs` / `mint_token_use_case.rs`):
`crates/foundry-services/tests/revoke_and_list_use_cases.rs`

1. `revoke_flips_revoked_at_and_foreign_jti_is_non_enumerable_notfound` — revoke stamps
   `revoked_at` (observable kill-switch); unknown jti → NotFound (non-enumerable,
   exercises `!belongs`); a principal with a foreign/stale `workspace_id` is refused and
   does not mutate the real token.
2. `list_returns_workspace_tokens_with_status_and_minted_by` — list returns the minted
   tokens with derived `revoked` status and `minted_by` resolved from `created_by` (the
   ISSUER, not the subject).
3. `scope_team_name_resolves_to_the_real_team_for_a_team_grant_and_none_for_workspace` —
   team grant resolves `scope_team_name` to the real team name; workspace grant resolves
   to None; the list view resolves the same name.

Test budget: 3 distinct behaviours (revoke effect+isolation, list projection, scope-name
resolution). Within budget (2×3 = 6 max).

## EXCLUDED FROM THIS PASS

| Surface | Why excluded |
|---------|--------------|
| `crates/foundry-web/.../admin_tokens.rs` (handler/UI) | Not pure logic — HTTP/HTML adapter. Covered by the @real-io acceptance lane (the `/admin/tokens` browser scenarios), not the in-process services tests. Mutating it here would require driving the web stack, exceeding this pass's services-scope and runtime budget. |
| `crates/foundry-store/migrations/000*_*.sql` + `0008` `created_by` | Schema migrations are not Rust functions — `cargo-mutants` does not mutate SQL. Correctness is asserted by the real-Postgres harness running the migrations (any broken DDL fails the testcontainer setup). |
| Workspace-scoping filter of `list_tokens` (the `WHERE workspace_id = $1` row filter) | The deployment is single-tenant (`uniq_one_workspace ON ((true))` — only ONE workspace can ever exist), so a second-workspace row cannot be seeded in-process. The cross-workspace exclusion is asserted by the @real-io acceptance lane, not these services tests. The `tokens.rs` mutant for this (`list_tokens -> Ok(vec![Default::default()])`) is unviable anyway. |
| Cross-workspace REVOKE of an existing foreign-owned row (`r.workspace_id != principal.workspace_id()` with a real foreign row) | Same single-tenant constraint — a token owned by another workspace cannot exist as a row. The realisable non-enumerability path (unknown jti → `None` → NotFound) IS exercised and kills the relevant mutant. |

## Post-run safety

- Tree clean: only the new test file + this report added.
- `cargo build -p foundry-services` — green.
- `cargo test -p foundry-services` — green (13 tests across 4 files + doc/unit).
- `cargo fmt --all -- --check` — clean.
- `cargo clippy --all-targets --release -- -D warnings` — clean.
- Leaked `postgres:16-alpine` containers removed after each run.
