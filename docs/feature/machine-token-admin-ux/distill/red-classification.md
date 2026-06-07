# RED Classification — machine-token-admin-ux (DISTILL)

Pre-DELIVER fail-for-the-right-reason gate. Every machine-token scenario must
fail because the behaviour is UNIMPLEMENTED (MISSING_FUNCTIONALITY), not because
of a setup/import/fixture error (BROKEN).

## How to reproduce

```
cargo test -p foundry-acceptance --test acceptance
```

(default lane; the machine-token scenarios are in-process `@real-io` — Postgres
testcontainer + real axum router via `InProcHarness`/`build_router`. No
`@docker-compose`, no extra env.)

## Result (2026-06-07)

```
39 features
202 scenarios (174 passed, 28 failed)
1626 steps (1598 passed, 28 failed)
```

- **174 passed** = the entire pre-existing suite, UNAFFECTED (0 regressions —
  verified: every failed step is inside a machine-token feature).
- **28 failed** = ALL 28 machine-token-admin-ux scenarios, every one RED.

## Per-scenario classification — all `MISSING_FUNCTIONALITY` (correct RED)

The `/admin/tokens` GET/POST/revoke routes are MOUNTED (so no 404 — verifier-only
must differ by the signer Option, not route presence), but the handlers are RED
scaffolds returning `501 Not Implemented` (`admin_tokens.rs`). Each scenario's
behaviour assertion fires cleanly against that 501:

| Failure shape (panic message) | Count | Classification |
|---|---|---|
| `the token list must have rendered (200) … got 501` | 10 | MISSING_FUNCTIONALITY |
| `the token surface must have rendered (200) … got 501` | 4 | MISSING_FUNCTIONALITY |
| `a non-admin must get a non-enumerable 404; got 501` | 3 | MISSING_FUNCTIONALITY |
| `missing the 'issuing not enabled' notice; status 501` | 2 | MISSING_FUNCTIONALITY |
| `mint must succeed to show the one-time value; got 501` | 2 | MISSING_FUNCTIONALITY |
| `expected 422 validation refusal, got 501` | 2 | MISSING_FUNCTIONALITY |
| `the one-time display must show the chosen team scope; body "…501…"` | 1 | MISSING_FUNCTIONALITY |
| `revoke should succeed (idempotent); got 501` | 1 | MISSING_FUNCTIONALITY |
| `expected 422 over-cap refusal, got 501` | 1 | MISSING_FUNCTIONALITY |
| `an admin must see the token surface (200); got 501` | 1 | MISSING_FUNCTIONALITY |
| `a cross-workspace revoke must be a non-enumerable 404; got 501` | 1 | MISSING_FUNCTIONALITY |

**Zero** `IMPORT_ERROR` / `FIXTURE_BROKEN` / `SETUP_FAILURE` / transport-error
panics. (An earlier iteration had two such wrong-RED shapes — a fixture
`ON CONFLICT (name)` bug against the single-workspace schema, and `panic!`-based
scaffold handlers that aborted the axum connection and surfaced at the client as
a transport error. Both were fixed: the seeding is now single-workspace-safe,
and the scaffold handlers RETURN `501` so the assertion fires. See `driver.md`.)

## Fixture-Theater guard (Critical Rule 7)

Negative + structural assertions ("no token value is shown", "token surface is
shown", "lists only the acting workspace's tokens") cannot pass on the scaffold
501: they call `require_rendered(...)` which asserts a 200 BEFORE the
absence/structure check is meaningful. So none of these can produce a false GREEN
while the handler is a scaffold — they will flip GREEN only when DELIVER renders
the real surface.

## DELIVER entry gate

DELIVER reads this file at the RED phase entry (ADR-025 D2). All 28 scenarios are
genuine RED → unskip one at a time, implement the handler + `tokens` use-case +
the signer-in-AppState wiring (`step-skeletons.md`), flip GREEN, commit.
