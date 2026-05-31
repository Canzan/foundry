# RED Classification — Feature A "Programmatic Foundry" (pre-DELIVER gate)

The pre-DELIVER fail-for-the-right-reason gate (nw-distill). Each scenario was RUN
against the RED scaffolds and classified: `MISSING_FUNCTIONALITY` (✅ correct RED — the
assertion fires because production behaviour is unimplemented) vs `IMPORT_ERROR` /
`FIXTURE_BROKEN` / `SETUP_FAILURE` / `WRONG_ASSERTION` (❌ wrong RED — block handoff).

## How to reproduce

```bash
# Requires a running Docker daemon (testcontainers Postgres).
cargo test -p foundry-acceptance --test acceptance
```

The default lane runs all 134 scenarios. Filter Feature A by `@feature-a` once DELIVER
adds a fast subset, or grep the run for `us-w05`/`us-w06`.

## Run result (2026-05-31, this machine)

```
134 scenarios (111 passed, 23 failed)
1145 steps   (1122 passed, 23 failed)
```

- **111 passed** = the ENTIRE pre-existing acceptance suite (110 scenarios) + the one
  Feature-A browser-path regression scenario that is GREEN by design (see below).
- **23 failed** = the 23 not-yet-implemented Feature-A scenarios. ALL classify as
  `MISSING_FUNCTIONALITY`. **Zero pre-existing scenarios regressed** (NFR-WEB-COMPAT-01).
- In EVERY failing scenario the Background + Given + When steps PASS (real preconditions,
  real HTTP / real subprocess); only the behavioural `Then` fires. No `IMPORT_ERROR`, no
  fixture crash, no setup failure.

## Per-scenario classification

### us-w05a-api-read-issues.feature (4 scenarios)

| Scenario | Fails at | Reason | Class |
|---|---|---|---|
| An integrator reads the board's issues as data (`@walking_skeleton`) | `the answer is a data list containing AUTH-2 and AUTH-3` | `GET /api/v1/...` → 404 (route not merged into build_router); body empty, not a JSON array | ✅ MISSING_FUNCTIONALITY |
| An empty project answers with an empty list | `the answer is an empty data list` | 404 → not a `[]` JSON answer | ✅ MISSING_FUNCTIONALITY |
| The data answer and the browser board come from the same core path | `both list exactly the same set of issues` | JSON side 404s → no issues to compare | ✅ MISSING_FUNCTIONALITY |
| A request with no valid credential is refused | `the request is refused as unauthenticated` | 404 (route absent), expected 401 | ✅ MISSING_FUNCTIONALITY |

### us-w05b-machine-token-auth.feature (10 scenarios)

| Scenario | Fails at | Reason | Class |
|---|---|---|---|
| A machine reads with its granted credential (`@walking_skeleton`) | `the machine requests ... with that credential` → `authenticated as the machine` | 404 (route + token_auth absent) | ✅ MISSING_FUNCTIONALITY |
| A machine credential needs no browser session and no anti-forgery token | `the request succeeds` | 404 | ✅ MISSING_FUNCTIONALITY |
| A request with no credential is refused | `refused as unauthenticated` | 404, expected 401 | ✅ MISSING_FUNCTIONALITY |
| A malformed credential is refused | `refused as unauthenticated` | 404, expected 401 | ✅ MISSING_FUNCTIONALITY |
| A forged credential the registry never issued is refused | `refused as unauthenticated` | 404, expected 401 (no `jti` denylist) | ✅ MISSING_FUNCTIONALITY |
| An expired credential is refused | `refused as unauthenticated` | 404, expected 401 (no `exp` check) | ✅ MISSING_FUNCTIONALITY |
| A revoked credential is refused on its next use | `refused as unauthenticated` | 404, expected 401 (no revoke path) | ✅ MISSING_FUNCTIONALITY |
| A credential signed with a disallowed algorithm is refused | `refused as unauthenticated` | 404, expected 401 (no alg pin) | ✅ MISSING_FUNCTIONALITY |
| A credential cannot reach beyond the team it was scoped to | `the machine requests the "Billing" board's issues` → assertion | 404 (no scope check) | ✅ MISSING_FUNCTIONALITY |
| The browser sign-in path is unchanged by the machine credential surface | — | **PASSES (GREEN)** — real browser sign-in works today; this is the live regression guard proving the additive surface does not touch the session/CSRF path. Must STAY green through DELIVER. | ✅ GREEN BY DESIGN |

### us-w05c-api-write-issues.feature (6 scenarios)

| Scenario | Fails at | Reason | Class |
|---|---|---|---|
| A machine files an issue through the API (`@walking_skeleton`) | `a new issue is created with the next sequential key` | `POST /api/v1/...` falls through to the CSRF layer → 403 "CSRF token missing"; expected 201. The `/api/v1` group is not yet mounted CSRF-exempt. | ✅ MISSING_FUNCTIONALITY |
| An issue filed through the API appears to a member watching the board | `the new issue appears on Mei's board` | create 403s → never reaches the outbox/SSE | ✅ MISSING_FUNCTIONALITY |
| A machine moves an issue to a new state through the API | `the updated issue is returned as data` | PATCH → 403 CSRF; expected 200 + JSON | ✅ MISSING_FUNCTIONALITY |
| A comment posted through the API is sanitized exactly as a browser comment | `the comment is stored with the dangerous content removed` | POST comment → 403 CSRF; expected 201 | ✅ MISSING_FUNCTIONALITY |
| An issue with an empty title is rejected by the same rule the browser enforces | `the write is rejected for a missing title` | 403 CSRF; expected 422 `title_required` | ✅ MISSING_FUNCTIONALITY |
| A write beyond the credential's authorization is refused | `the write is refused as not-allowed` | PATCH comment → 403 CSRF, but the assertion REQUIRES a JSON `forbidden` envelope (not the CSRF 403), so it correctly fails RED — the authorization use-case is never reached | ✅ MISSING_FUNCTIONALITY |

> NOTE — false-GREEN guard applied (Critical Rule 7): the catch-all CSRF 403 happens to
> share the HTTP status (403) with a real authorization refusal. Without a guard the
> `not-allowed` Then would have PASSED for the wrong reason (the authz code never runs).
> `assert_authorization_forbidden` rejects any 403 whose body contains "CSRF" or is not
> the JSON `{"error":{"code":"forbidden"}}` envelope — so this scenario fails RED today
> and flips GREEN only once DELIVER mounts `/api/v1` CSRF-exempt AND the service returns
> `Forbidden`. This was caught and fixed during this DISTILL run.

### us-w06-boundary-guard.feature (4 scenarios)

| Scenario | Fails at | Reason | Class |
|---|---|---|---|
| A clean tree passes the boundary check | `the check passes` | `cargo xtask check-arch` exits 2 "unknown subcommand"; expected 0 | ✅ MISSING_FUNCTIONALITY |
| A page constructed in the data-API tier fails the check | `the check fails` (hardened) | guard never ran ("unknown subcommand"); the hardened assertion rejects the spurious exit-2 | ✅ MISSING_FUNCTIONALITY |
| An adapter reaching the database directly fails the check | `the check fails` (hardened) | guard never ran | ✅ MISSING_FUNCTIONALITY |
| A credential verifier that would accept a disallowed algorithm fails the check | `the check fails` (hardened) | guard never ran | ✅ MISSING_FUNCTIONALITY |

> NOTE — false-GREEN guard applied (Critical Rule 7): `cargo xtask check-arch` currently
> exits non-zero simply because the subcommand is unrecognised, which would spuriously
> satisfy a bare `exit != 0` on the violation scenarios. The hardened `the check fails`
> step asserts the output does NOT contain "unknown subcommand" — i.e. the guard
> actually RAN — before treating non-zero as "caught the violation". Caught and fixed
> during this DISTILL run.

## Gate verdict

**PASS.** All 23 not-yet-implemented Feature-A scenarios fail as
`MISSING_FUNCTIONALITY`; the browser-path regression scenario is GREEN by design; zero
pre-existing scenarios regressed; the workspace compiles with no warnings. No
`IMPORT_ERROR` / `FIXTURE_BROKEN` / `SETUP_FAILURE` / spurious-GREEN remains. Two
false-GREEN risks (the CSRF-403 collision on the authorization scenario, and the
unknown-subcommand exit on the guard-violation scenarios) were detected by running the
suite and hardened so they fail RED for the right reason. DELIVER may proceed to the
RED→GREEN cycle (unskip-free: Foundry uses RED scaffolds, not skip markers).
