# Driver — machine-token-admin-ux (DISTILL)

How the acceptance scenarios drive the system, the walking skeleton, the harness,
and the test-double policy. Companion to `coverage-matrix.md` + `step-skeletons.md`.

## Architecture of Reference + Project Infrastructure Policy

Per `docs/architecture/atdd-infrastructure-policy.md` — this feature adds NO new
port class; it reuses the established mechanisms:

| Port (this feature) | Class | Mechanism (from the policy) |
|---|---|---|
| Browser admin surface `GET/POST /admin/tokens` | Driving (HTTP, browser) | `reqwest` against `InProcHarness`/`build_router` UNDER the session + CSRF layers (same in-process server + per-scenario schema as the HTML surfaces) |
| `/api/v1` verify path (cross-check) | Driving (HTTP, machine) | `reqwest` with `Authorization: Bearer <jwt>` against the SAME in-process router (the SHIPPED machine-token path — reused, not rebuilt) |
| `machine_tokens` registry + `jti` denylist | Driven internal (real) | shared testcontainers Postgres 16 + per-scenario schema (NEVER faked) |
| `MachineTokenSigner` (Ed25519) | Driven external / non-deterministic (credential secret) | FIXED test keypair via `foundry_auth::test_keys::signer()` — REAL crypto, known key material (mirrors the verifier fixture) |

No policy row is added; every mechanism already exists.

## Walking skeleton

Exactly ONE `@walking_skeleton` scenario:

**us-mt01-mint.feature → "An admin issues a working token and sees its value
once"** (`@walking_skeleton @us-mt00 @real-io`).

It traces the thinnest end-to-end slice of the riskiest, highest-value path:
1. an admin signs in over the REAL cookie path (session + CSRF);
2. POSTs the mint form to `/admin/tokens` through the production router;
3. the one-time value is rendered EXACTLY ONCE (copy affordance + only-time
   warning + jti/label/scope/expiry);
4. the minted token is presented as a bearer to the SHIPPED `/api/v1` read and
   AUTHENTICATES — proving the product minted a REAL, signed, verifiable
   credential (US-MT01 AC), end-to-end through the production composition root.

Litmus test (Dimension 5): the title is a user goal ("issues a working token and
sees its value once"), the Then steps are user observations (sees the value once,
the token works against the API), and a non-technical stakeholder confirms "yes,
that is what Priya needs". It is demo-able.

US-MT00 (signer-in-AppState + the `created_by` migration) is `@infrastructure`,
FOLDED into this skeleton — it never ships standalone; it is the substrate the
mint stands on.

## Issuer vs verifier-only harness

`InProcHarness` gained two constructors (`support/harness.rs`):
- `spawn(now)` — ISSUER: `AppState.machine_token_signer = Some(test signer)`
  (matched to the verifier), so the mint surface is offered. Default for the
  US-MT0x scenarios.
- `spawn_verifier_only(now)` — VERIFIER-ONLY: `machine_token_signer = None`,
  modelling a read-only replica with no `MACHINE_TOKEN_SIGNING_KEY` (US-MT00
  scenario 2 / US-MT01 scenario 3 — "issuing not enabled", graceful, OD1/DD2).

The signed-in Givens (`feature_machine_token_admin.rs`) rebuild the harness in
the required mode and re-seed Acme + admin + the Backend member, so each scenario
is order- and mode-independent.

## Test-double policy

- **Registry / denylist** — REAL Postgres (driven-internal). Tokens are seeded
  via the SHIPPED `insert_machine_token` and revoked via `revoke_machine_token`.
- **Signer** — the FIXED test Ed25519 keypair (`test_keys`); REAL EdDSA crypto,
  known key material so the minted token verifies on `/api/v1`.
- **Browser auth** — REAL session + CSRF (sign-in over `/sign-in`, then the
  `signed_in_post` harness helper carries the `_csrf` + session cookie). NFR-MT-
  SEC-07 (the browser contract unchanged) is exercised, not faked.

## RED scaffold strategy (why 501, not panic)

The `/admin/tokens` handlers RETURN `501 Not Implemented` rather than `panic!`.
A panic aborts the axum connection and surfaces at the `reqwest` client as a
transport error — which trips the step's `.expect()` BEFORE the assertion runs
(a wrong-RED SETUP_FAILURE shape). Returning a real `501` lets the `When` step
capture a real status and the `Then` assertion fail RED on the missing
page/value/list — the correct MISSING_FUNCTIONALITY signal. The route is mounted
on every binary (no 404), because OD1/DD2 graceful degradation differs by the
signer Option, not route presence.

## Single-workspace constraint

The schema permits exactly one workspace per database (`uniq_one_workspace`).
Cross-workspace evil-user paths are modelled with synthetic foreign jti/team
uuids — observably identical to a foreign-workspace target from the acting
admin's side. See `upstream-issues.md` UI-1 for the full rationale and the
promote-when-multi-workspace recommendation.
