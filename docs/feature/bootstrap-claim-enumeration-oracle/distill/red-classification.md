# RED Classification — bootstrap-claim-enumeration-oracle

Pre-DELIVER fail-for-the-right-reason gate. Each new scenario was run against the
current (un-rewired) production code and classified MISSING_FUNCTIONALITY (correct
RED) vs BROKEN/SETUP (wrong RED).

Command:
```
FOUNDRY_ACCEPTANCE_TAGS=bootstrap-enum-oracle cargo test -p foundry-acceptance --test acceptance
```
Result: `4 scenarios (1 passed, 3 failed)` — every failure lands on the `Then`
assertion (all `Given`/`When` setup steps pass), so every RED is genuine.

| Scenario | Tags | Outcome | Classification | Evidence |
|---|---|---|---|---|
| A fresh-email claim still seeds the workspace and the first instance admin | `@us-03 @regression @real-io` | PASS (GREEN) | Regression guard — correctly green today | 303 redirect + workspace + `instance_admins` row all present under current handler |
| Colliding email, expired token, and unknown token are refused indistinguishably | `@us-01 @error @nfr-sec-01 @security-regression @real-io` | FAIL | **MISSING_FUNCTIONALITY** | Byte-identity Then fires: collision arm status `500 Internal Server Error` vs the `200 OK` token refusals. Today the 23505 collision surfaces as a 500 (the oracle). |
| A colliding submit leaves the bootstrap token unconsumed | `@us-02 @error @real-io` | FAIL | **MISSING_FUNCTIONALITY** | `bootstrap_tokens.used_at` query returns `0` unconsumed rows (expected `1`). Today the token is claimed BEFORE the create runs, so the collision burns it. |
| After a collision the token is reusable with a corrected email | `@us-02 @error @real-io` | FAIL | **MISSING_FUNCTIONALITY** | Retry of the (burned) token returns `200` refusal instead of the `303` dashboard redirect. Recovery is impossible until the claim+create is atomic. |

No BROKEN / IMPORT_ERROR / FIXTURE / SETUP failures. No WRONG_ASSERTION /
OBSERVABLE_NOT_AT_PORT: every assertion reads a port-exposed observable — HTTP
status + full body (byte-identity), the 303 + `workspaces`/`instance_admins` seed at
the Postgres boundary, and `bootstrap_tokens.used_at`. Gate: PASS — RED is genuine,
handoff to DELIVER is unblocked.

## us-05 regression net — still green

```
FOUNDRY_ACCEPTANCE_TAGS=us-05 cargo test -p foundry-acceptance --test acceptance
→ 13 scenarios (13 passed), 89 steps (89 passed)
```
The shipped token enumeration-oracle scenario and the happy-path claim are untouched.

## Placement / STOP decision — recorded for DELIVER

**Handler was NOT rewired in DISTILL (deliberate).** Design D1 calls for rewiring
`POST /bootstrap` to call the new atomic `claim_bootstrap_and_create_workspace`. But
that endpoint is the SAME driving port us-05's happy-path and `@security-regression`
scenarios exercise. Rewiring it to a `panic!` scaffold would drive those shipped
scenarios into a 500 and break the regression net — a wrong RED. Per the DISTILL
mandate (never break the shipped net; RED must be MISSING_FUNCTIONALITY, not a
self-inflicted break) and the task's explicit escape hatch ("keep
`create_initial_workspace` intact and add the new method as pure scaffold … rather
than implementing the feature"), DISTILL:

- adds `BootstrapClaimOutcome` + the panicking `claim_bootstrap_and_create_workspace`
  scaffold (compiles; `pub` so no dead-code lint; `__SCAFFOLD__` marker in doc), and
- leaves `bootstrap.rs` on its current two-call path, so the new scenarios are RED
  against the current WRONG behavior (500 on collision, burned token) rather than
  against a panic.

**DELIVER's two steps** (both required to flip these RED green while keeping us-05
green): (1) implement the transaction body of `claim_bootstrap_and_create_workspace`
(guarded UPDATE → seed → 23505-specific catch → commit), then (2) rewire the handler
to call it and match `Consumed → 303 / Refused|EmailCollision → bootstrap_refusal_page()
/ Err → 500`. After (2), `create_initial_workspace` becomes dead — DELIVER deletes it
per the repo's remove-dead-code policy (verify no other callers first).

**NFR-3 narrow-catch (non-23505 → 500) placement.** Forcing a non-23505 DB error
mid-transaction is impractical at the HTTP acceptance layer (it needs a broken schema
or injected fault the driving port cannot express). Per architecture.md ("Store-scope
mutation testing should target the new method … hold the [100% store-scope kill] bar")
this arm is DELIVER's responsibility: a store-scope unit test asserting a non-23505
error propagates as `Err(StoreError)` (not `EmailCollision`/`Refused`), pinned by
store-scope mutation testing on the 23505 code check. No HTTP acceptance scenario is
written for it (it would be a fixture-theater 500 rather than a port-observable
behavior).
