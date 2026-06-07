# Slice 3 — Trust the contract: stable codes + non-enumerable, escalation-bounded refusals

> Hardens what Slices 1-2 established so the surface is provably safe to publish and script against.
> Pulls the confirmation-bias defense (evil caller, hostile input) into explicit, tested scenarios,
> and makes the v1 escalation boundary (no bearer mint route) + the abuse guardrail first-class.

## Learning hypothesis (one line)
If every token-route outcome is the stable shipped envelope, refusals are non-enumerable, there is NO
bearer-reachable mint route, and management mutations are rate-bounded, then an integrator can build
an SDK against a predictable contract and a security reviewer can certify the surface leaks no
existence oracle and cannot be turned into a runaway.

## Stories
- **US-TMA04** (mt-api-job-3) — stable error contract across the token routes (codes/statuses from
  `status_for`; read-after-write equality on the LIST shape).
- **US-TMA05** (mt-api-job-1) — the refusal boundary: non-management → 403; cross-workspace/unknown →
  identical 404; revoked/expired/forged/`alg:none`/wrong-alg → identical 401; **no bearer mint route**
  (route-surface assertion, NFR-TMA-SEC-08); revoke-storm throttled (NFR-TMA-SEC-07).

## Backbone coverage
Trust-the-contract across all of Authenticate / Inventory / Revoke-Rotate.

## Reused primitives (verified in code)
- `status_for` + `ErrorBody`/`ErrorDetail` — SHIPPED, reused unchanged for every refusal.
- Non-enumerable `NotFound` from `revoke_token`; identical `Unauthorized` from `token_auth`
  (the SHIPPED refusal catalogue: missing/malformed/bad-sig/wrong-alg/`alg:none`/expired/forged all
  collapse to one 401 — covered by the existing `token_auth_tests`).
- The v1 no-mint-route rule is the expression of the ratified Q-AUTHZ (option c).

## Done when
- Every non-2xx token-route response is the shipped envelope + conventional status (NFR-TMA-CON-01);
  LIST is read-after-write consistent (NFR-TMA-CON-02).
- A non-management bearer is refused (403) non-enumerably on every token route.
- Cross-workspace + unknown jti are indistinguishable (identical 404).
- All invalid/revoked bearer classes return the identical 401.
- No bearer-reachable mint route exists in v1 (route-surface assertion).
- A management-mutation burst beyond the guardrail is throttled; per-principal mutation rate is a
  guardrail metric (mechanism/numbers = DESIGN, Q-RATE-LIMIT).

## Risk addressed
Existence-oracle leaks, privilege escalation (the crux), and abuse loops — made explicit + tested so
the surface is safe to publish.

## Depends on
Slices 1-2; ratified Q-AUTHZ + the Q-RATE-LIMIT default (guardrail + per-principal cap).

> NOTE: if the user ratifies programmatic MINT into v1 (option c+b), US-TMA05's "no bearer mint route"
> AC is replaced by the explicit `tokens:manage` capability gate + the "management tokens cannot mint
> management tokens" anti-self-replication AC + the mint-rate guardrail, and a new MINT slice
> (US-TMA06) is appended.
