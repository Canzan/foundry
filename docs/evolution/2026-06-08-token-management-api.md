# Evolution — token-management-api

**Finalized**: 2026-06-08
**Ship commit**: `a23cc2b` (tip; 7 DES steps + Phase-4 security fixes + rate-bucket mutation hardening) off `e21db26` (pre-feature) — feature range `0e87f90..a23cc2b`.
**Wave coverage**: full nWave pipeline — DISCUSS → DESIGN → DISTILL → DELIVER (wave-by-wave with checkpoints; legacy per-feature layout; trunk-based, committed directly to `main`).

## Feature summary

The **JSON token-management API** — the machine-facing counterpart to `machine-token-admin-ux`'s web UI. It adds `GET .../tokens` (list) and `DELETE .../tokens/{jti}` (revoke) under `/api/v1`, authenticated by the **machine-token bearer** (the shipped `MachinePrincipal` extractor), so an audit pipeline or rotation job can see and kill credentials with no browser and no DB access. It is overwhelmingly inheritance: two thin route handlers over the already-shipped, 100%-mutation-hardened `foundry_services::tokens` use-cases, reached through the existing `State<Services>` seam in the `foundry-api` adapter. The central security decision is **(c) asymmetric authz** (ratified DISCUSS Q-AUTHZ): a bearer may **list + revoke** (including revoke-self / rotate) via the existing `is_workspace_admin` gate, but may **NEVER mint via the API**. Mint stays human-session-only (`/admin/tokens`, session + CSRF) — the privilege-escalation surface (a leaked token as a credential printing press) is kept structurally closed.

## What shipped (security-bearing)

- **Value-free `TokenJson` list**: `GET .../tokens` → 200 with a newest-first JSON array (empty registry → `[]`, never 404/error); each object mirrors `TokenView` verbatim (snake_case) with **no value / token / secret / hash key** by construction — asserted value-free at the contract layer (NFR-TMA-SEC-02).
- **`DELETE .../tokens/{jti}` → 204, idempotent**: a re-revoke of an already-revoked credential is also a harmless 204 (NFR-TMA-REL-01); `{jti}` is extracted as `uuid::Uuid`, so a malformed id fails axum extraction before the handler, leaking no existence.
- **Kill-switch via the shipped jti denylist**: a revoked token's very next `/api/v1` call is refused 401 by the SHIPPED per-request denylist — no new enforcement mechanism.
- **Non-enumerable cross-workspace 404**: a foreign-workspace or missing jti returns an identical NotFound; the API reveals nothing about credentials it does not own.
- **Byte-identical 401 authn class**: refusals ride the SHIPPED `status_for` / `ErrorBody { error: { code, message } }` envelope with stable machine-readable codes (`unauthorized`/`forbidden`/`not_found`/`rate_limited`) — no new error shape for the token routes; an integrator can branch on `error.code` without parsing prose.
- **No-mint boundary as a build-time invariant**: enforced by **structural absence (no `POST .../tokens` route)** PLUS a NEW `check_api_no_mint_route` **LAYER-1d `check-arch` rule** — `Services::mint_token` must never appear in `foundry-api`, and no `post(` may be registered on a `.../tokens"` collection literal. A planted-violation **gold test** drives the guard binary against a copy of the tree and asserts it names the offending file+line and exits non-zero (proving the guard bites, not just claims to). Remediation reworked the POST detector from line-scoped to a per-`.route(..)`-block two-pass, making it robust to the **multi-line axum route evasion** (the headline review finding F3).
- **Per-principal revoke-storm rate guardrail (429)**: an in-process token bucket in `AppState`, **keyed by bound `user_id`** (the accountable identity and correct blast-radius unit), checked on the DELETE route AFTER auth and BEFORE the use-case. A burst beyond capacity → **429 `rate_limited`** in the shipped envelope. It is **adapter-local — NOT a `ServiceError` variant** (rate-limiting is a transport concern; the web UI has no rate concept), deterministic via the SHIPPED `state.clock` / `MockClock` (no wall-clock sleeps), adds **no new crate dependency**, and emits `foundry_token_mutations_total{principal, outcome}` so the per-principal mutation rate is observable (NFR-TMA-SEC-07). It guards only the self-DoS-capable verb — LIST stays unguarded.
- `foundry-api` gained **NO new crate dependency**, stayed **HTML-free**, never named `foundry_store::Store`, and did **no ad-hoc authz** (the boundary guard `api≠HTML` / `api≠Store` / `api≠ad-hoc-authz` lanes stay green; the rate guardrail reads shared state through the existing `FromRef` seam).

## How it was built (DELIVER)

7 DES-monitored TDD steps across 4 slices, each driven by `@real-io` cucumber scenario(s) over the real `/api/v1` router (real HTTP, real EdDSA bearer, real Postgres registry):

| Step | Outcome | Drove green |
|------|---------|-------------|
| 01-01 | `GET .../tokens` route + `TokenJson` serde shape (value-free list end-to-end) | walking-skeleton list: admin bearer → 200 `[TokenJson]`, empty → `[]`, no value key |
| 01-02 | bearer-auth refusals on the list route | inherited 401 (extractor) + 403 (`is_workspace_admin`) — non-enumerable, asserted at acceptance |
| 02-01 | `DELETE .../tokens/{jti}` revoke route (204, idempotent, kill-switch) | rotation job revokes → dead-on-next-call 401; re-revoke 204; non-admin 403 |
| 02-02 | revoke-self / rotate + cross-workspace non-enumerable 404 | revoke-self rotation; foreign/missing jti → identical 404 (green by inheritance from 02-01) |
| 03-01 | read-after-write list consistency + stable refusal codes | post-revoke LIST shows `revoked:true`, every other field byte-identical; every refusal a stable code |
| 03-02 | no-mint boundary guard — `check-arch` LAYER-1d + planted-violation gold test | `POST .../tokens` → 404/405; guard bites a planted `mint_token` / `post(.../tokens)` violation |
| 04-01 | per-principal revoke-storm guardrail (429) + mutation metric | DELETE burst beyond `C` → 429 `rate_limited`; refill via MockClock; metric reflects per-principal rate |

Most slices were thin adapter work over use-cases already shipped **and 100%-mutation-hardened** by `machine-token-admin-ux`, so several steps were **green-by-inheritance** — verified port-to-port over real HTTP (not fixture theater), with `RED_UNIT` deliberately `SKIPPED` and the rationale recorded in the DES log (re-testing a shipped use-case is redundant; mocking inside the hexagon is forbidden). Then: a **security-focused adversarial review** (Sonnet) → **CHANGES-REQUESTED**, all actionable findings fixed test-first — **F3** (multi-line route-guard evasion; the headline) closed by the per-route-block two-pass detector; **F4/F5/F7** tightened the read-after-write field-by-field comparison, the empty-list self-exclusion guard, and the burst test's exact `25 ok / 5 throttled` assertions; **F1** (rate-guard runs before authz: 429 before 403 for a throttled non-admin) and **F2** (unbounded per-principal bucket map) documented as **accepted residuals** with rationale (adapter-side authz would violate the boundary guard; the map is O(dozens) under the single-workspace model). Finally a **mutation pass** on the new pure logic lifted `rate_limit.rs` from 10/17 (59%) to **17/17 viable kill (100%)**, killing the masked `*`→`/` refill mutant that an `R=1.0` test had hidden, plus the decision-predicate and `check_revoke` constant mutants.

## Quality at ship

- **Acceptance** (`@all`): **235 scenarios / 1879 steps** green — all us-tma* plus the entire existing suite (NFR-TMA-REL-03: green-before stays green-after).
- **Build/lint**: full-workspace `cargo fmt --all --check` (0) and `cargo clippy --all-targets --release -- -D warnings` (0); `xtask` tests green.
- **`check-arch`**: green — now includes the new LAYER-1d `api≠mint` rule alongside `api≠HTML` / `api≠Store` / `api≠ad-hoc-authz` / JWT-alg-pin.
- **Mutation**: `crates/foundry-app/src/rate_limit.rs` **100% viable kill (17/17, 2 unviable)** — the bucket arithmetic, decision predicates, and `check_revoke` are pinned. (The reused `tokens.rs` use-cases were already 100%-hardened by the prior feature.)

## Residuals / follow-ups

- **F1 — rate-guard ordering** (429 before 403 for a throttled non-admin): **accepted**. The guard deliberately runs before authz so a revoke-storm cannot exhaust the authz lookup, and 429/403 leak nothing about jti existence; an adapter-side authz pre-check would break `check_api_no_adhoc_authz`. Documented at the call site.
- **F2 — bucket-map eviction**: **accepted** under the single-workspace model (the map is O(dozens of admins)). LRU / idle-eviction is the tracked mitigation for multi-workspace; recorded in the `rate_limit` module doc.
- **Cross-workspace fixtures**: the evil-user cross-workspace paths are still exercised with synthetic uuids (the CODE enforces workspace scoping + non-enumerability); real two-workspace fixtures await multi-workspace support (`distill/upstream-issues.md`).
- **Prometheus exporter wiring** for `foundry_token_mutations_total` is a DEVOPS decision — emitting the metric is in scope; wiring the exporter sink is platform/operations.

## Pointers

- Spec: `docs/feature/token-management-api/{discuss,design,distill,slices}/` — notably `design/no-mint-boundary.md`, `design/rate-guardrail.md`, `design/api-contract.md`, `design/wave-decisions.md`.
- DES roadmap + execution log (the audit trail, preserved): `docs/feature/token-management-api/deliver/roadmap.json` + `execution-log.json` (7 steps × PREPARE/RED_ACCEPTANCE/RED_UNIT/GREEN/COMMIT = 35 events; `des-verify-integrity` clean).
- Core: `crates/foundry-api/src/lib.rs` (the two route handlers + `TokenJson` + DELETE rate-guard call site), `crates/foundry-app/src/rate_limit.rs` (the per-principal token bucket + 100% mutation kill), `xtask/src/check_arch.rs` (the LAYER-1d `check_api_no_mint_route` rule + gold test), `crates/foundry-acceptance/tests/features/us-tma-token-management-api.feature` + `crates/foundry-acceptance/src/steps/feature_token_management_api.rs`.
- Predecessor: `docs/evolution/2026-06-07-machine-token-admin-ux.md` — this feature is its deferred JSON-API fast-follow.
