# Token-Management API — DESIGN Wave Decisions

> Morgan (nw-solution-architect), DESIGN wave. Fast-forward mode: options + recommendation per open
> decision; the user ratifies at the post-roadmap checkpoint. Builds DIRECTLY on the RATIFIED
> DISCUSS decisions (Q-AUTHZ → option c asymmetric; `DELETE /api/v1/.../tokens/{jti}`; revoke-self
> allowed; per-principal rate guardrail; list = value-free metadata array). No DISCUSS assumption
> was challenged — see "Upstream changes" (none).

## Architecture summary

A **pure adapter feature**. Two routes added to the EXISTING `foundry-api` `/api/v1` driving adapter,
over the SHIPPED, mutation-hardened `list_tokens` / `revoke_token` use-cases (reached through the
existing `State<Services>` seam — foundry-api never names `foundry_store::Store`). No new crate, no
migration, no use-case change. Mint stays human-session-only (`/admin/tokens`); the bearer surface
exposes LIST + REVOKE only. Default architecture style (modular monolith + ports-and-adapters) is
already in force — this feature is a new driving-adapter route group, nothing more.

### Route → use-case map

| Route | Handler (NEW) | Use-case (SHIPPED, reused as-is) | Authz | Success |
|---|---|---|---|---|
| `GET /api/v1/teams/{team}/projects/{project}/tokens` | `list_tokens_handler` | `Services::list_tokens(&principal)` | `is_workspace_admin` (in use-case) | 200 `[TokenJson]` / `[]` |
| `DELETE /api/v1/teams/{team}/projects/{project}/tokens/{jti}` | `revoke_token_handler` (rate-guarded) | `Services::revoke_token(&principal, jti)` | `is_workspace_admin` (in use-case) | 204 |
| `POST /api/v1/.../tokens` (mint) | **none — no route** | — (`mint_token` NOT routed) | — | 404/405 + check-arch guard |

## Reuse Analysis (verdict counts)

**REUSE/EXTEND = 9** (8 reuse-as-is: both use-cases, both `Services` wrappers, the `MachinePrincipal`
extractor, `status_for`/`ErrorBody`, the `is_workspace_admin` gate, the per-request denylist; + 1
EXTEND: the `routes<S>()` router). **CREATE NEW = 5** (the 2 route handlers, the `TokenJson` serde
shape, the rate guardrail + metric, the proposed check-arch no-mint rule). Full table in
`architecture.md §3`. This feature is overwhelmingly inheritance.

## DDD-numbered decisions

| # | Decision | Rationale |
|---|---|---|
| **DD-TMA-01** | Two routes (`GET .../tokens`, `DELETE .../tokens/{jti}`) added to the existing `routes<S>()`; reach the SHIPPED use-cases via `State<Services>`. No new Services method, no use-case change. | `Services` already exposes `list_tokens`/`revoke_token`; mirror the issue/comment handler pattern. Keeps foundry-api off `foundry_store::Store` (boundary guard). |
| **DD-TMA-02** | `TokenJson` serde response mirrors `TokenView` verbatim (snake_case), **no value/token/secret/hash key**. RFC3339 timestamps; `minted_by` (resolved issuer email). | One obvious mapping, zero translation risk; NFR-TMA-SEC-02 enforceable by a response-key contract assertion. |
| **DD-TMA-03** | `DELETE` success → **204 No Content** (idempotent re-DELETE also 204). The LIST is the canonical read-after-write source. | `revoke_token` returns `()`; 204 is idiomatic and avoids inventing a new success body shape. |
| **DD-TMA-04** | MINT kept off the bearer surface by **structural absence (no route)** PLUS a **proposed `check-arch` LAYER-1d no-mint AST rule** (`mint_token` must not appear in `foundry-api`; no `post(.../tokens)`). | Absence is correct today but fragile to future edits (`Services::mint_token` is reachable); the guard turns the ratified no-mint decision into a build-time invariant (Principle 11/12). |
| **DD-TMA-05** | Rate guardrail = **in-process per-principal token bucket** in `AppState` (keyed by bound `user_id`), checked on DELETE before the use-case, emitting a per-principal mutation metric; throttle → adapter-local **429 `rate_limited`** in the SHIPPED envelope. Uses the SHIPPED `state.clock` for deterministic tests. | Single binary, no Redis, no migration, O(1) hot path; bounds the only v1 abuse vector (revoke storm). The DB-backed sign-in throttle solves a different threat and would add schema. |
| **DD-TMA-06** | The 429 rides as an **adapter-local response**, NOT a new `ServiceError` variant (the web UI has no rate concept; rate-limiting is a transport concern). | Keeps the cross-adapter `ServiceError` contract unchanged; revisit only if a second surface needs it. |
| **DD-TMA-07** | Authz stays in the use-cases (`is_workspace_admin`); the adapter performs NO authz (boundary guard `api≠ad-hoc-authz`). The "authz-gate seam" of US-TMA00 is documented as: in v1 the use-case gate IS the seam; a future `tokens:manage` capability check would live in foundry-services, not the adapter. | NFR-WEB-API-SEC-02; the SHIPPED guard already forbids `is_workspace_admin(` in foundry-api source. |

## Technology choices

**No new crates expected.** Reuses `axum` (routing/extractors), `serde` (the new `TokenJson`),
`uuid`, `time` — all already in `foundry-api`. The guardrail uses `std::sync::Mutex<HashMap>` (or
`dashmap` only if it is ALREADY a workspace dependency — prefer std to add nothing) + the SHIPPED
`state.clock`. The proposed guard rule is a function in the existing `xtask` crate. All
Rust/OSS, no proprietary, no license question. If a metrics facade is not already wired, the metric
degrades to a `tracing` counter field (confirm sink with platform-architect — DEVOPS owns the
exporter).

## Constraints (inherited, all honored)

ONE binary · ONE Postgres · NO Redis · NO new crate · NO migration (NFR-TMA-DATA-01: zero) · foundry-api
stays HTML-free + off `foundry_store::Store` (boundary guard LAYER 1+2 green) · `/api/v1` mounts
OUTSIDE session+CSRF (bearer-only) · browser `/admin/tokens` byte-for-byte unchanged · the full
`foundry-acceptance` suite green-before stays green-after (NFR-TMA-REL-03).

## ============================================================
## Open decisions awaiting user ratification
## ============================================================

| # | Open decision | Options | RECOMMENDED | One-line why |
|---|---|---|---|---|
| **OD-TMA-1** | Rate-guardrail mechanism (Q-RATE-LIMIT) | (a) in-process per-principal token bucket; (b) DB-backed throttle (reuse sign-in pattern); (c) `last_used`/timestamp check | **(a) token bucket** | Single-binary-native, no Redis/migration, O(1), deterministically testable via the SHIPPED clock; (b) adds schema + wrong response, (c) cannot bound a burst. |
| **OD-TMA-1b** | Guardrail key | bound `user_id` vs `jti` | **`user_id`** | The accountable identity and the correct blast-radius unit; an attacker can't dodge the cap by switching tokens. |
| **OD-TMA-2** | No-mint enforcement (the proposed guard rule) | (a) no route only; (b) no route **+ a `check-arch` LAYER-1d no-mint AST rule** | **(b) add the guard rule** | Absence is correct today but fragile; `Services::mint_token` is reachable, so a guard makes the ratified no-mint decision a build-time invariant against the highest-impact regression (mint loop). Cheap (one detector + gold test, mirrors the existing authz detector). |
| **OD-TMA-3** | DELETE success response (Q-REVOKE-VERB shape) | (a) **204 No Content**; (b) 200 + `{"jti","revoked":true}` | **(a) 204** | `revoke_token` returns nothing; 204 is idiomatic; LIST is the canonical read-after-write; avoids a new success body shape. |
| **OD-TMA-4** | LIST response field names (Q-LIST-SHAPE) | confirm the exact serde fields | **`jti, label, scope_team_id, scope_team_name, expires_at, revoked, last_used_at, minted_by`** (no value) | Verbatim `TokenView` mirror = zero translation risk; matches the web `TokenRow`; NFR-TMA-SEC-02 holds by construction. |
| **OD-TMA-5** | 429 representation | adapter-local response vs new `ServiceError::TooManyRequests` | **adapter-local** | Rate-limiting is a transport concern; the web UI has no rate concept — keep `ServiceError` unchanged. |

## Upstream changes (DISCUSS assumptions challenged)

**None.** Every DISCUSS ratified decision is directly implementable as specified; the SHIPPED
use-cases and adapter primitives are exactly as DISCUSS described (verified by reading
`tokens.rs`, `foundry-api/src/lib.rs`, `foundry-services/src/lib.rs`, `check_arch.rs`,
`admin_tokens.rs`). No `upstream-changes.md` is written.

## Handoff to DISTILL / DELIVER

- **Acceptance-designer**: the AC in `stories.md` are observable and implementation-neutral; the
  status-code table (`api-contract.md §4`) + the no-mint route-surface assertion + the burst
  scenario are the new contract assertions to formalize. Read-after-write (US-TMA04) lands cleanly
  given the 204 + canonical-LIST decision.
- **Platform-architect (DEVOPS)**: NO external integrations / no consumer-driven contract tests owed.
  ONE new observability item: the per-principal mutation metric (DD-TMA-05) — confirm the metrics
  sink. The proposed `check-arch` no-mint rule (OD-TMA-2) joins the existing boundary-guard CI lane.
</content>
