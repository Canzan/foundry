# Token-Management API — DISTILL Wave Decisions

> Sentinel (nw-acceptance-designer), DISTILL wave. Fast-forward mode. Trunk-based
> (no branch, no PR). This feature is a thin JSON adapter over SHIPPED, mutation-
> hardened use-cases; the scenarios cover the NEW `/api/v1/.../tokens` adapter
> surface + the security boundaries (authz, non-enumerability, no-mint, rate).
> LEGACY per-feature layout (DA7); story namespace `US-TMA0x`.

## Wave-Decision Reconciliation HARD GATE — RESULT: PASSED (0 contradictions)

Read and cross-checked DISCUSS (`discuss/wave-decisions.md`) against DESIGN
(`design/{wave-decisions,api-contract,no-mint-boundary,rate-guardrail}.md`).
DEVOPS directory is EMPTY → WARN, default environment matrix applied (no
contradiction possible). Every ratified DISCUSS decision is directly
implementable as DESIGN specifies them — DESIGN's own "Upstream changes: None"
is confirmed independently here:

| DISCUSS ratified (Q-) | DESIGN realization | Verdict |
|---|---|---|
| Q-AUTHZ → (c) asymmetric: bearer may LIST + REVOKE (incl. self), NO API mint | DD-TMA-01 (two routes only), DD-TMA-04 + `no-mint-boundary.md` (no `POST` route + check-arch rule), DD-TMA-07 (authz stays in use-case `is_workspace_admin`) | CONSISTENT |
| Q-REVOKE-VERB → `DELETE /api/v1/.../tokens/{jti}` | `api-contract.md §1` exact path/verb | CONSISTENT |
| Q-REVOKE-SELF → ALLOWED | `api-contract.md §3` revoke-self (caller's own jti, denylist bites next call) | CONSISTENT |
| DELETE success shape (DISCUSS unpinned) | DD-TMA-03 / OD-TMA-3 → 204 No Content, idempotent | CONSISTENT (DISCUSS left it to DESIGN) |
| Q-RATE-LIMIT → per-principal mutation guardrail + metric | DD-TMA-05 / OD-TMA-1 → in-process token bucket keyed by bound `user_id`, adapter-local 429, SHIPPED `state.clock` for determinism | CONSISTENT (mechanism still OPEN — see upstream-issues) |
| Q-LIST-SHAPE → metadata array, NEVER a value | DD-TMA-02 / OD-TMA-4 → `TokenJson` mirrors `TokenView`, no value/token/secret/hash key | CONSISTENT |
| All security guarantees inherit `machine-token-admin-ux` (non-enumerable, never-persisted/logged, revoke-on-next-request denylist) | `api-contract.md §4-5` reuse `status_for`/`ErrorBody`; non-enumerability §4 | CONSISTENT |

No contradiction found. Proceeded to scenario writing.

## Architecture of Reference + Project Infrastructure Policy (applied, not renegotiated)

- **Driving port**: HTTP `/api/v1` → real adapter, in-process `InProcHarness`
  (`foundry_app` router via `spawn`), real `reqwest` client, real EdDSA bearer
  JWT minted with the FIXED test signing key. Already recorded in
  `docs/architecture/atdd-infrastructure-policy.md` (the "JSON API `/api/v1/...`"
  Driving row + the "Ed25519 machine-token signing keypair" Driven-external row).
- **Driven internal**: Postgres (`machine_tokens` registry + `jti` denylist,
  workspaces/users/teams/projects/memberships) → REAL, testcontainers postgres:16
  + per-scenario schema (existing harness). The `machine_tokens` registry row is
  already in the policy (Feature A US-W05b).
- **Driven external / non-deterministic**: the Ed25519 key MATERIAL is a fixture
  (crypto path is real); the `state.clock` / `MockClock` is the deterministic
  clock seam the rate-guardrail will read.
- **No NEW infrastructure** introduced. Policy file unchanged (all ports in scope
  already present). Lang: Rust (`Cargo.toml`) → `[lang-mode] rust`. State-delta
  port: layer-3 real-adapter scenarios use traditional assertions (Mandate 8
  applies to layers 1-3 with state-delta; at layer 3 the universe-guard is
  optional — assertions here are example-based and traditional, which is correct
  for real-I/O acceptance per Mandate 11). No `tests/common/state_delta.rs`
  bootstrap needed for these scenarios.

## Layer + Tier classification

- **Layer 3 (subprocess/real-adapter acceptance)** for every scenario: real HTTP
  + real Postgres + real EdDSA. Per Mandate 9 + 11 → **example-based**, sad paths
  enumerated explicitly, **no PBT machinery** (`@given`/state-machine). Correct
  for a thin adapter/boundary feature.
- **Tier A only** (Gojko-style, production composition root via `InProcHarness`).
  **Tier B NOT added**: although the revoke→rotate journey is ≥3 chained
  scenarios, the input space is NOT domain-rich (the variables are jti/label/
  workspace-membership, a small discrete set — not emails/dates/free-text/large
  payloads). The escalation surface is covered by enumerated adversarial
  examples, which is the right instrument for a security boundary. Mandate 10
  "skip Tier B" condition met (journey is example-coverable; the only state
  machine is the SHIPPED, already-hardened denylist).

## Scenario list with tags

`.feature` file: `crates/foundry-acceptance/tests/features/us-tma-token-management-api.feature`
(co-located with all other foundry-acceptance features — precedent: every
`us-*`/`us-w05*`/`us-mt*` feature lives here; matches exactly).

| # | Scenario | Tags | Story | Type |
|---|---|---|---|---|
| 1 | An audit pipeline lists the workspace's tokens as data | `@walking_skeleton @us-tma00 @us-tma01 @real-io @driving_adapter` | US-TMA00/01 | happy (WS) |
| 2 | An empty registry answers with an empty list, not an error | `@us-tma01 @error` | US-TMA01 | edge |
| 3 | A non-management caller is refused without leaking the registry | `@us-tma01 @error` | US-TMA01 | error (authz) |
| 4 | A request with no credential is refused before any token logic runs | `@us-tma01 @us-tma00 @error` | US-TMA00/01 | error (authn) |
| 5 | The list never exposes a token value | `@us-tma01 @error` | US-TMA01 | boundary (SEC-02) |
| 6 | A rotation job revokes a credential and it is dead on its next call | `@us-tma02` | US-TMA02 | happy (kill-switch) |
| 7 | Revoking an already-revoked credential is a harmless success | `@us-tma02 @error` | US-TMA02 | edge (idempotent) |
| 8 | Revoking a credential from another workspace reveals nothing | `@us-tma02 @error` | US-TMA02 | error (non-enum 404) |
| 9 | A non-management caller cannot revoke | `@us-tma02 @error` | US-TMA02 | error (authz) |
| 10 | A rotation job retires its own credential after promoting a new one | `@us-tma03` | US-TMA03 | happy (revoke-self) |
| 11 | Re-running rotation against an already-retired credential is harmless | `@us-tma03 @error` | US-TMA03 | edge (idempotent self) |
| 12 | A listed token reflects its revocation on the next read | `@us-tma04` | US-TMA04 | happy (read-after-write) |
| 13 | Every token-route refusal carries a stable machine-readable code | `@us-tma04 @error` | US-TMA04 | contract |
| 14 | Cross-workspace and unknown ids are indistinguishable | `@us-tma05 @error` | US-TMA05 | error (non-enum) |
| 15 | An invalid or revoked credential is refused identically | `@us-tma05 @error` | US-TMA05 | error (authn) |
| 16 | A credential signed with a disallowed algorithm is refused | `@us-tma05 @error` | US-TMA05 | error (alg-pin) |
| 17 | There is no programmatic mint surface to escalate through | `@us-tma05 @error` | US-TMA05 | boundary (no-mint) |
| 18 | A burst of revocations beyond the guardrail is throttled | `@us-tma05 @rate-guardrail @pending` | US-TMA05 | error (rate) — SCAFFOLD |

**Counts**: 18 scenarios. Error/edge/boundary: 13 (#2,3,4,5,7,8,9,11,13,14,15,16,17,18)
= **72%** error ratio (well above the 40% Mandate; appropriate for a security
boundary feature). Happy: 5 (#1,6,10,12 + #1 is the WS).

## Walking Skeleton strategy

ONE WS scenario (#1), tagged `@walking_skeleton @real-io @driving_adapter`. It is
the SAFEST real op (read-only LIST) and proves the ratified authz model on the
safest surface before any mutation: a management-capable bearer lists the
workspace's tokens as value-free JSON. Litmus test: a non-technical stakeholder
confirms "yes — an audit pipeline can see what tokens exist, without a browser or
DB." User goal framing (title = user goal, Then = observable JSON outcome). Real
adapters end-to-end (real HTTP, real EdDSA bearer, real Postgres registry). The
authorized-vs-refused proof is split across #1 (authorized) + #3 (refused) per
the story-map "prove the authz model on the safest op" sequencing.

## Adapter coverage table (every endpoint → ≥1 @real-io scenario)

| Driving endpoint | Use-case (SHIPPED) | @real-io scenario(s) | Covered |
|---|---|---|---|
| `GET /api/v1/.../tokens` | `Services::list_tokens` | #1 (lists two), #2 (empty []), #3 (403 non-mgmt), #4 (401 no-cred), #5 (no value), #12/#13 (read-after-write + stable code), #15/#16 (authn refusals) | YES |
| `DELETE /api/v1/.../tokens/{jti}` | `Services::revoke_token` | #6 (revoke→401-next), #7 (idempotent), #8 (cross-ws 404), #9 (403 non-mgmt), #10 (revoke-self→401-next), #11 (idempotent self), #14 (non-enum) | YES |
| `POST /api/v1/.../tokens` (the NEGATIVE — must NOT exist) | — (`mint_token` NOT routed) | #17 (no mint route → 404/405; no value returned) | YES (negative) |
| Rate guardrail on DELETE | adapter-local 429 | #18 (`@pending` — mechanism open) | SCAFFOLD |

Driven-adapter real-I/O: the `machine_tokens` registry + the SHIPPED `jti`
denylist are exercised for real by the revoke→next-call-401 cross-check (#6,#10)
— the kill-switch reuses the SHIPPED denylist assertion path (re-mint the revoked
jti, hit a SHIPPED `/api/v1` route, assert 401).

## Driving-adapter coverage (GET + DELETE + the no-mint negative)

- **GET** list: #1 (WS) invokes the real HTTP GET, asserts 200 + JSON array shape
  + field set + no-value. Status, body shape, and argument handling (team/project
  path + bearer header) all verified.
- **DELETE** revoke: #6 invokes the real HTTP DELETE, asserts 204 (no body) + the
  denylist kill-switch.
- **No-mint NEGATIVE**: #17 invokes a real HTTP POST to the tokens collection,
  asserts 404/405 (no route) and that no value is ever returned — the v1
  expression of the Q-AUTHZ ratification.

## Scaffold inventory (Mandate 7 — RED-ready)

NO production scaffold stubs were created: every production symbol the steps
touch ALREADY EXISTS and is SHIPPED — `Services::list_tokens`/`revoke_token`
(`foundry-services/src/lib.rs:156,165`), `Store::insert_machine_token`/
`revoke_machine_token`/`pool` (`foundry-store/src/lib.rs:1401,1473,230`),
`TokenView` (`foundry-services/src/tokens.rs:61`), the `MachinePrincipal`
extractor + `status_for`/`ErrorBody`, `foundry_auth::test_keys::signer`/
`TEST_PUBLIC_KEY_PEM`/`MachineTokenClaims`. The ONLY missing thing is the
`/api/v1/.../tokens` ROUTE WIRING in `foundry-api` (the routes are not yet
`.merge()`-d into `build_router`). So the scenarios are RED-by-route-absence: the
real HTTP request 404s, the assertion fires (e.g. `expected HTTP 200, got 404`),
which is **MISSING_FUNCTIONALITY**, not BROKEN (the test crate compiles; the
harness, bearer minting, and seeding all succeed). This is the cleanest possible
RED — no fake stubs to remove.

- `tests/features/us-tma-token-management-api.feature` — 18 scenarios (1 WS,
  16 active RED, 1 `@pending`).
- `src/steps/feature_token_management_api.rs` — NEW step module (registered in
  `src/lib.rs`, force-linked in `tests/acceptance.rs`).
- `src/world.rs` — 4 new per-scenario fields (`tma_first_refusal`,
  `tma_first_refusal_status`, `tma_revoke_status`, `tma_burst_statuses`).
- The `@pending` rate-guardrail scenario (#18) carries a deterministic-by-design
  SCAFFOLD body (drives the SHIPPED `state.clock`, NO wall-clock sleep) but is
  held `@pending` until OD-TMA-1/OD-TMA-5 ratify the bucket mechanism — see
  `upstream-issues.md`.

## NEW vs reused-from-existing steps

The bearer/JSON/seeding PATTERNS are reused from `feature_a_programmatic.rs`
(real `mint_credential`, real HTTP, `capture`→`world.last_*`) and
`feature_machine_token_admin.rs` (admin/member role seeding, `seed_token_row`),
but cucumber-rs requires globally-unique step TEXT, so every token-management
step phrase is NEW (token-management-domain wording, distinct from the issue-board
and `/admin/tokens` phrasings). The Background lines `a workspace "..." exists
with admin "..."` (us_06) and `a member "..." belongs to the team "..."` (us_07)
ARE reused verbatim (globally-registered). Net: ~46 NEW step definitions
(token-management-specific) + 2 reused Background phrases. The IMPLEMENTATIONS
mirror the SHIPPED Feature-A helpers (no new harness, no new infra).

## Rate-guardrail determinism decision

DESIGN (`rate-guardrail.md`) specifies the guardrail reads the SHIPPED
`state.clock` / `MockClock`, so a deterministic burst test IS achievable (advance
the mock clock to prove refill — no real sleep, no wall-clock flake). HOWEVER the
guardrail MECHANISM itself is an OPEN decision: OD-TMA-1 (token bucket vs
alternatives, capacity `C` / refill `R`), OD-TMA-1b (key = `user_id`), OD-TMA-5
(adapter-local 429 vs new `ServiceError` variant), and the test-only clock-advance
affordance the bucket must expose are all awaiting ratification at the
post-roadmap checkpoint. Per the DISTILL contract (when the mechanism is still an
open decision, author the burst as a SCAFFOLD/`@pending` with a clear marker
rather than a flaky timing test), scenario #18 is:

- authored **deterministic-by-design** (the step body drives a clock-advanced
  burst, never `sleep`), AND
- tagged `@pending` (excluded from the default + `@all` lanes), AND
- flagged as the single open item in `upstream-issues.md`.

DELIVER unskips #18 once OD-TMA-1/1b/5 are ratified and the bucket + clock-advance
affordance are wired.

## Pre-requisites (DESIGN driving ports + env matrix the scenarios depend on)

- DESIGN driving ports: the two `/api/v1/.../tokens` routes merged into
  `build_router` (DD-TMA-01); `TokenJson` serde shape (DD-TMA-02); 204 on DELETE
  (DD-TMA-03); the no-mint structural absence + check-arch rule (DD-TMA-04 /
  `no-mint-boundary.md`); the rate guardrail (DD-TMA-05, mechanism OPEN).
- DEVOPS: directory EMPTY → default environment matrix; the per-principal
  mutation metric sink (DD-TMA-05) is a DEVOPS-owned exporter decision (emitting
  the metric is in DELIVER scope; wiring Prometheus is DEVOPS).
- Infra: existing testcontainers postgres:16 + per-scenario schema + the fixed
  EdDSA test keypair (all SHIPPED, all in the policy).

## Pre-DELIVER fail-for-the-right-reason gate

`red-classification.md` (sibling file) records the per-scenario RED
classification. Expectation: all 16 active scenarios = MISSING_FUNCTIONALITY
(route 404 → assertion fires); #18 = `@pending` (not executed in default/all
lanes). The test crate compiles clean (`cargo check -p foundry-acceptance
--tests` green), so there is no IMPORT_ERROR / BROKEN class.

## Self-review (Dimension 9 + Mandate 7)

- [x] WS strategy declared (real-io, driving adapter, in-process composition root)
- [x] WS tagged `@walking_skeleton @real-io @driving_adapter`
- [x] Every driving endpoint has ≥1 `@real-io` scenario (GET, DELETE, POST-negative)
- [x] No InMemory doubles (all real adapters per the policy)
- [x] Mandate 7: no scaffold stubs needed (all production symbols SHIPPED); RED by route absence, test crate compiles (RED not BROKEN)
- [x] Driving adapter exercised via its protocol (real HTTP GET/DELETE/POST), not a direct service call
- [x] Business language (Pillar 1): scenario titles + steps use domain words (token, credential, revoke, list, refused) — no HTTP/JSON/SQL in titles or step text
- [x] Chained narrative (Pillar 2): the revoke/rotate journey reuses the list + bearer Givens
- [x] Layer 3 example-based, sad paths enumerated (Mandate 9 + 11); no PBT machinery
- [x] Error ratio 72% (≥40%)
