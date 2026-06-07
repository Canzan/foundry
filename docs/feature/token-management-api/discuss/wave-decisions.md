# Token-Management API — DISCUSS Wave Decisions

> This is the file DESIGN reads FIRST. This feature adds the **machine-facing JSON counterpart**
> to the just-shipped `machine-token-admin-ux` web UI: programmatic **mint / list / revoke** of
> machine tokens under `/api/v1`, JSON in/out, authenticated by a **machine-token bearer**, for
> integrators / CI / automation / agents rather than a human admin clicking a screen. This was the
> deferred fast-follow noted out-of-scope in `machine-token-admin-ux` (Q6 → "WEB ADMIN UI FIRST. A
> JSON token-management API is a deferred fast-follow.").
>
> **The crux of this wave is the authz / privilege-escalation model** — a machine token that can
> mint MORE tokens is a self-replication surface. That decision is framed below with options +
> escalation analysis + a recommendation; the USER must ratify it before DESIGN. Everything else
> reuses primitives that ALREADY SHIPPED and are mutation-hardened (100%).

## Feature Summary

The token-management use-cases already exist and are mutation-hardened in
`crates/foundry-services/src/tokens.rs`:

- `mint_token(store, signer, principal, MintInput) -> MintedToken` — authz (`is_workspace_admin`)
  → TTL validation → scope mapping → claims → sign → persist METADATA ONLY → return the one-time
  value.
- `list_tokens(store, principal) -> Vec<TokenView>` — authz → workspace-scoped, newest-first,
  no `value` field.
- `revoke_token(store, principal, jti) -> ()` — authz → workspace-isolation (non-enumerable
  `NotFound`) → flip `revoked_at`; effective on the next request via the SHIPPED denylist.

The web UI (`/admin/tokens`, Askama, under session+CSRF) is the human call-site for these. **This
feature adds the SECOND call-site: a JSON `/api/v1/.../tokens` adapter** over the same three
use-cases, authenticated by a bearer machine token instead of a browser session — so an
integrator / CI job / agent can mint, list, and revoke programmatically.

The driving adapter to EXTEND is `crates/foundry-api/src/lib.rs`: it already serves JSON under
`/api/v1`, has the `MachinePrincipal` `FromRequestParts` bearer extractor (`token_auth::authenticate`
→ `Principal::Machine`), the `ServiceError → (status, ErrorBody)` JSON envelope (`status_for`), and
`IssueJson`-style serde shapes. The `/api/v1` group is mounted OUTSIDE session+CSRF
(`foundry-app/src/lib.rs` line ~345) — bearer-only, CSRF-exempt by construction. We add token
routes to `routes()` and wire the signer in.

Feature type: **cross-cutting** (JSON API surface + auth/security/privilege-escalation posture).

```
foundry (one binary)
├── foundry-auth      MachineTokenSigner::mint / MachineTokenVerifier        (SHIPPED — reused)
├── foundry-store     machine_tokens repo (incl. created_by)                  (SHIPPED — reused)
├── foundry-services  tokens::{mint_token,list_tokens,revoke_token}           (SHIPPED — mutation-hardened 100% — reused AS-IS)
├── foundry-api       /api/v1 JSON adapter: MachinePrincipal extractor,       (SHIPPED — EXTEND)
│                     status_for envelope, IssueJson shapes
└── NEW (this feature):
    ├── /api/v1/.../tokens routes (GET list, DELETE/POST revoke, POST mint)
    ├── the authz/escalation gate ON TOP of the use-cases (THE CRUX — see below)
    └── the signer reachable from the foundry-api mint handler (today only the
        web /admin handler reads AppState.machine_token_signer directly)
```

## Phase 1 — Discovery & Job Grounding

### No DIVERGE directory (RISK, low impact)
There is no `docs/feature/token-management-api/diverge/`. Jobs in `jobs.yaml` are NEW and
Luna-derived from (a) the brief, (b) a fresh reading of the shipped `foundry-services::tokens`
use-cases + the foundry-api adapter + `machine-token-admin-ux` ratified decisions, and (c) JTBD
method. Importances/satisfactions are Luna estimates pending user/field validation. **Mitigation:**
the use-cases are SHIPPED + mutation-hardened, so the only genuine unknowns are (1) the
authz/escalation model (the CRUX, flagged Q-AUTHZ) and (2) the revoke verb shape (Q-REVOKE-VERB).

### What was grounded by reading the actual code (not assumed)
- `crates/foundry-services/src/tokens.rs`: `mint_token` / `list_tokens` / `revoke_token` EXIST and
  **every one gates on `is_workspace_admin(principal.workspace_id(), principal.user_id())`** (lines
  99-105, 219-226, 276-282). They take a `&Principal`. For a machine token, `Principal::Machine`
  carries `{ user_id, workspace_id, jti, scope_team_id }` and acts AS its bound user. **Therefore
  today, with ZERO code change, a machine token whose bound `user_id` is a workspace admin would
  PASS these gates** — i.e. option (a) is the default behaviour if we wire the routes naively. This
  is the escalation surface, and it is present right now in the shipped code.
- `MintedToken.value` is a `secrecy::SecretString` — the one-time value, never persisted/logged.
  `TokenView` has NO `value` field by construction. `revoke_token` returns a non-enumerable
  `NotFound` for unknown OR cross-workspace `jti`.
- `crates/foundry-api/src/lib.rs`: the `MachinePrincipal` extractor authenticates the bearer
  (fail-closed 401, non-enumerable), `status_for` maps every `ServiceError` to a stable JSON
  envelope (`Validation{code,message}` → 422, `Forbidden` → 403, `NotFound` → 404,
  `Unauthorized` → 401). `routes<S>()` requires `Services: FromRef<S>` + `Arc<MachineTokenVerifier>:
  FromRef<S>`. **It does NOT have access to the signer** — only the web `/admin` handler reads
  `AppState.machine_token_signer` directly via `State<AppState>`.
- `crates/foundry-app/src/lib.rs`: `AppState.machine_token_signer: Option<Arc<MachineTokenSigner>>`
  — `Some` ⇒ issuer binary (mint offered), `None` ⇒ verifier-only. The `/api/v1` group is merged
  OUTSIDE session+CSRF (bearer-only). To let the API mint, the signer must be reachable from the
  foundry-api adapter (DESIGN owns the wiring; the constraint is captured as Q-SIGNER-WIRING).
- `MachineTokenClaims { sub, scope: Option<Uuid>, iat, exp, jti, iss, aud }` — **there is NO
  management-capability claim today.** Option (b) below would add one (a new claim dimension).

## Phase 2 — Scope Assessment (Elephant Carpaccio Gate)

### Scope Assessment: PASS — 6 stories (1 `@infrastructure`), 1 bounded-context surface (the /api/v1 token adapter), estimated ~5-8 days
Oversize signals checked: 6 stories (≤10 OK); the use-cases + envelope + extractor are SHIPPED so
the walking skeleton needs few NEW integration points (one route group + the authz gate + signer
wiring for mint); effort well under 2 weeks; the list/revoke/mint outcomes slice cleanly
safest-authz-first. No oversize signal trips. Right-sized for one DISCUSS→DELIVER pass with a thin
walking skeleton (prove the authz model on GET list — the safest real op — first). No split needed.

---

# ============================================================================
# THE CRUX — Authz / Privilege-Escalation Model (USER MUST RATIFY)
# ============================================================================

## The problem in one sentence

A machine token that can hit these endpoints can manage OTHER machine tokens. If it can MINT, a
single leaked token becomes an **unlimited credential printing press** (self-replication). If it
can REVOKE broadly, a single leaked token becomes a **denial-of-service / admin-lockout switch**.
The web UI did not have this problem: a human session is short-lived, cookie-bound, CSRF-protected,
and tied to a logged-in admin. A machine bearer token is long-lived, copy-pasteable, and lives in
CI config / agent memory. **The same `is_workspace_admin` gate that is correct for the UI is a
materially different risk when the caller is itself a bearer token.**

## Escalation analysis — what a leaked management-capable token can do

Assume an attacker has exfiltrated ONE machine token whose bound `user_id` is a workspace admin
(the realistic worst case: an admin minted a broad token for a CI job and it leaked into a build
log). Under the naive "reuse `is_workspace_admin`" wiring (option a):

| Attack | Possible? | Consequence |
|--------|-----------|-------------|
| **Mint loop / self-replication** | YES | The attacker mints N fresh admin-bound tokens. Revoking the leaked one does nothing — the children are independent credentials with their own `jti`s. The blast radius is now unbounded and outlives the original leak. |
| **Revoke OTHER tokens (incl. the admin's own UI-minted tokens)** | YES | The attacker revokes every other token in the workspace, breaking every legitimate integration (DoS). |
| **Lock the admin out of their own automation** | PARTIAL | Revocation only kills machine tokens, not the admin's browser session — the human admin can still log into `/admin/tokens` and revoke the attacker's children IF they can enumerate them faster than the attacker re-mints. A mint loop wins that race. |
| **Cross-workspace reach** | NO | `revoke_token` already returns non-enumerable `NotFound` across workspaces; `list_tokens`/`mint_token` are workspace-scoped by the principal. The escalation is confined to the leaked token's own workspace. |
| **Escape the workspace boundary entirely** | NO | The bound `user_id`/`workspace_id` are fixed in the token's claims; the attacker cannot mint a token for a DIFFERENT workspace. |

**Conclusion:** the escalation is workspace-confined (good — the shipped isolation holds) but
within the workspace, a leaked management-capable token under option (a) is catastrophic because of
the MINT loop. MINT is the dangerous verb; LIST and REVOKE-SELF are not self-amplifying.

## The four options

### (a) Admin-bound token — reuse `is_workspace_admin` as-is
Any token whose bound user `is_workspace_admin` may mint/list/revoke. **Zero new code in the
use-cases** (this is literally what they do today). Simplest. **But:** a leaked admin-bound token
self-replicates (mint loop) and can DoS the workspace. The capability is implicit and invisible —
the admin who minted a "CI bot" token may not realise it can also mint more tokens.

### (b) Dedicated management capability claim — `tokens:manage`
A token must carry an explicit capability claim (e.g. `tokens:manage`) to reach these endpoints;
the bound-user admin check is no longer sufficient on its own. Explicit, least-privilege, auditable
("which tokens can manage tokens? the ones carrying this claim"). **But:** it adds a NEW dimension
to `MachineTokenClaims` (today `{sub, scope, iat, exp, jti, iss, aud}` — no capability concept), a
migration concern for the claim's representation, and a mint-time UX for granting it. A leaked
`tokens:manage` + admin token STILL self-replicates unless combined with (c).

### (c) Asymmetric — machine tokens may LIST + REVOKE, never MINT
A machine token may LIST the registry and REVOKE (including revoke-self / rotate), but **MINT stays
human-session-only (the `/admin/tokens` UI).** No self-replication is possible from a bearer token —
the printing press is removed entirely. A leaked token can at worst revoke (DoS), which is loud,
reversible by the human admin, and does not outlive the leak (no children). This directly serves
the strongest real automation jobs (rotation scripts revoke-self + an operator audits via LIST;
provisioning the FIRST token is inherently a bootstrap a human does once).

### (d) Human-session-only for ALL management
Then this API does not exist. **Rejected:** it defeats the feature's purpose (programmatic
mint/list/revoke for CI/agents). Stated only to close the option.

## RECOMMENDATION (for the user to ratify)

**Adopt (c) asymmetric as the v1 default, layered with (b)'s explicit-capability principle applied
ONLY to the revoke/list surface, and defer programmatic MINT to a future, separately-ratified slice.**

Concretely for v1:
- **LIST** (`GET /api/v1/.../tokens`) and **REVOKE** (`DELETE`/`POST .../tokens/{jti}/revoke`,
  including revoke-self) are reachable by a machine token, gated by the existing
  `is_workspace_admin` check on the bound user (the use-cases already enforce this — **no use-case
  change**). These are not self-amplifying: the worst case is a loud, reversible DoS, never an
  unbounded credential leak.
- **MINT** (`POST /api/v1/.../tokens`) is **NOT exposed to bearer tokens in v1.** Provisioning a
  NEW credential remains a human-session action (`/admin/tokens`). This removes the mint-loop /
  self-replication surface entirely.
- The walking skeleton proves the model on the SAFEST op: **GET list authorized for a
  management-capable (admin-bound) token, refused (403, non-enumerable) for a non-admin-bound
  token** — before any mutation.

**Why (c)-first over (a):** (a) is one line of wiring but ships the mint-loop footgun on day one to
a credential class designed to be copied into CI. (c) delivers the two jobs that are genuinely
programmatic and frequent (audit-via-LIST, rotate-via-REVOKE-SELF) while keeping the
escalation-sensitive op (MINT) on the channel that already has session+CSRF+human-in-the-loop
protection. Programmatic mint can be added later as its own slice WITH the explicit `tokens:manage`
capability from (b) and mint-rate guardrails — ratified on its own, not smuggled into v1.

**This is the single decision the user must make before DESIGN.** If the user needs programmatic
MINT in v1 (e.g. a provisioning pipeline that bootstraps tokens for ephemeral environments), then
the recommendation becomes **(c)+(b)**: expose MINT but ONLY to a token carrying an explicit
`tokens:manage` capability claim, never to a plain admin-bound token, plus a mint-rate guardrail
(NFR-TMA-SEC-07) and a "management tokens cannot mint management tokens" anti-self-replication rule.

## Key Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| **DA1** | **Reuse the SHIPPED `foundry-services::tokens` use-cases AS-IS.** This feature adds a JSON adapter + an authz gate ON TOP, never new use-case logic. | The use-cases are mutation-hardened (100%) and already enforce admin-only authz, workspace isolation, non-enumerable refusals, one-time value, and metadata-only persistence. Re-spec'ing them would risk regressing tested behaviour. |
| **DA2** | **The authz/escalation model is the CRUX and is USER-RATIFIED before DESIGN.** Recommendation: (c) asymmetric — LIST+REVOKE for bearer tokens, MINT human-session-only in v1. | A bearer token is long-lived and copy-pasteable; the same `is_workspace_admin` gate that is safe for a CSRF-protected human session is a self-replication footgun for a machine credential. See the escalation table above. **Q-AUTHZ.** |
| **DA3** | **The JSON API upholds the SAME security guarantees the web UI ratified.** One-time value in the response body, never re-fetchable / never persisted / never logged; revoke effective on the next request via the SHIPPED denylist; non-enumerable refusals; `created_by` = the calling principal. | The brief's hard constraint. The use-cases already enforce most of this; the adapter must not weaken it (e.g. must not log the response body, must not add a value field to the list shape). See `nfrs.md`. |
| **DA4** | **Extend the EXISTING foundry-api adapter, reuse its envelope + extractor.** New routes go in `routes()`; refusals/validation use the existing `status_for` JSON envelope and the `MachinePrincipal` bearer extractor. | The contract (`ErrorBody`, `status_for`, fail-closed non-enumerable 401) is shipped and tested. Token routes are a peer of the issue/comment routes over the same `Services` seam. |
| **DA5** | **Solution-neutral on mechanism.** The exact revoke verb (`DELETE` vs `POST .../revoke`), the JSON request/response field names, how the signer is wired to the API adapter (if MINT is ever exposed), and the representation of any `tokens:manage` capability are DESIGN. | DISCUSS fixes the requirement + the risk posture + the observable outcomes; DESIGN picks the wire shapes and the wiring. Captured as Q-REVOKE-VERB, Q-SIGNER-WIRING. |
| **DA6** | **`created_by` for an API-minted token (if MINT is ever exposed) = the calling machine principal's bound `user_id`.** The audit row records the SUBJECT the calling token acts as. | Consistent with the use-case: `mint_token` already persists `created_by = principal.user_id()`. For a bearer caller that is the bound user — which is the accountable identity. Surfaced so DESIGN/audit knows API mints are attributable to the bound admin, not "anonymous API". |
| **DA7** | **Output uses the LEGACY per-feature layout** (separate files under `discuss/`), NOT the SSOT/feature-delta model; story IDs use the `US-TMA0x` namespace. | Decided with the user; mirrors `machine-token-admin-ux/discuss/`. `US-TMA0x` distinguishes this feature. |

## Open Questions for the User + DESIGN

| # | Question | Why it matters | Default assumption if unanswered |
|---|----------|----------------|----------------------------------|
| **Q-AUTHZ** | **THE CRUX. Which authz/escalation model?** (a) admin-bound reuse, (b) explicit `tokens:manage` capability, (c) asymmetric (LIST+REVOKE for bearer, MINT human-only), or (c)+(b) if programmatic mint is needed. | A bearer token that can MINT self-replicates; one that can REVOKE broadly is a DoS switch. This sets the entire risk posture and which routes ship. **Highest-priority confirmation.** | **(c) asymmetric** — v1 exposes LIST + REVOKE (incl. revoke-self) to bearer tokens; MINT stays human-session-only. Programmatic MINT deferred to a future slice with (b)'s explicit capability + a mint-rate guardrail. |
| **Q-REVOKE-VERB** | Revoke verb + shape: `DELETE /api/v1/.../tokens/{jti}` (REST-idiomatic) or `POST /api/v1/.../tokens/{jti}/revoke` (mirrors the web UI's `POST .../revoke`)? | Drives the route contract + the AC wording. Both reuse `revoke_token`. | `DELETE /api/v1/.../tokens/{jti}` (idiomatic for a machine API), idempotent (re-DELETE = 204/200, mirrors the idempotent re-stamp). DESIGN picks. |
| **Q-REVOKE-SELF** | Should a token be able to revoke ITSELF (rotation: a script mints-new-then-kills-old, or kills-self on decommission)? | Rotation is a primary automation job (mt-api-job-2). Revoke-self is the safest possible mutation (a token disabling its own future use). | YES — revoke-self is allowed and is the walking-skeleton-adjacent safe mutation. `revoke_token` already handles any `jti` in the caller's workspace; self is a subset. |
| **Q-SIGNER-WIRING** | IF programmatic MINT is ever exposed (not v1 under the recommendation), how does the foundry-api adapter reach `AppState.machine_token_signer`? Today only the web handler reads it via `State<AppState>`; foundry-api sees only `Services` + `Arc<MachineTokenVerifier>` via `FromRef`. | The adapter cannot call `mint_token(store, signer, …)` without the signer. A new `FromRef<AppState> for Option<Arc<MachineTokenSigner>>` (or routing mint through `Services`) is needed. | Out of scope for v1 (MINT deferred). If ratified into v1, DESIGN adds the wiring; the constraint "verifier-only binary offers no API mint, returns 'issuing not enabled', never 500" carries over from `machine-token-admin-ux` NFR-MT-SEC-04. |
| **Q-RATE-LIMIT** | Should programmatic management (esp. any future MINT, but also REVOKE storms) be rate-limited / abuse-throttled? | A programmatic surface invites loops a human UI does not. Even REVOKE storms are a DoS vector. | A guardrail metric + a sane per-principal rate cap on management mutations (NFR-TMA-SEC-07); the exact numbers + mechanism are DESIGN. v1 LIST+REVOKE-self is low-risk; revert-storm protection is a guardrail, not a blocker. |
| **Q-LIST-SHAPE** | What does the LIST JSON expose per token? (`jti`, `label`, `scope_team_id`/name, `expires_at`, `revoked`, `last_used_at`, `created_by`/`minted_by`) — and NEVER a value. | Drives the response contract + the audit job (mt-api-job-3). Must mirror `TokenView` (which has no value field). | Mirror `TokenView` exactly: `jti`, `label`, `scope_team_id` (+ resolved name), `expires_at`, `revoked`, `last_used_at`, `minted_by`. NO `value`/secret field, ever (NFR-TMA-SEC-02). DESIGN picks JSON field names. |

## Constraints Established

- ONE binary, ONE Postgres, no Redis, no Node runtime service, no CDN (carried from the platform).
- **Reuse, don't rebuild:** the JSON adapter calls the SHIPPED `foundry-services::tokens`
  use-cases; the bearer extractor (`MachinePrincipal`/`token_auth`) and the JSON error envelope
  (`status_for`) are reused unchanged.
- The minted token value (if MINT is ever exposed) is in the response body EXACTLY ONCE and is
  NEVER persisted, logged, or re-fetchable; the registry/LIST stores only `jti` + metadata (the
  table has no secret column).
- Revocation is a flag (`revoked_at`), effective on the credential's NEXT `/api/v1` request via the
  SHIPPED per-request denylist.
- Refusals are non-enumerable: a non-management caller and a cross-workspace `jti` both refuse
  without confirming existence (the use-cases already do this — `Forbidden` for authz,
  non-enumerable `NotFound` for cross-workspace revoke, identical 401 for any auth failure).
- `created_by` for any API-minted token = the calling principal's bound `user_id` (attributable).
- **Authz/escalation posture is the gate (DA2/Q-AUTHZ) — user-ratified before DESIGN.**
- Solution-neutral: revoke verb shape, JSON field names, signer wiring, and the `tokens:manage`
  capability representation are DESIGN.

## Risks Surfaced (for DESIGN's risk register)

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|-----------|
| **Mint-loop self-replication** — a leaked management-capable bearer token mints unlimited fresh admin-bound credentials; revoking the leaked one does not stop the children | Medium | **Critical** | DA2/Q-AUTHZ recommends (c): MINT is NOT exposed to bearer tokens in v1. If MINT is ratified in, require (b)'s explicit `tokens:manage` claim + a mint-rate guardrail + "management tokens cannot mint management tokens". |
| **Revoke-storm DoS / admin lockout** — a leaked bearer token revokes every other token in the workspace, breaking all integrations | Medium | High | Workspace-confined (cross-workspace `NotFound`). Q-RATE-LIMIT guardrail on management mutations. The human admin can still revoke the attacker token via the session-only UI (no mint loop to outrace under (c)). |
| One-time value (if MINT exposed) leaks via response logging or an error envelope | Medium | High | The value is a `SecretString` (no Debug/Display); NFR forbids logging the mint response body; the `status_for` envelope already never carries credential material. v1 defers MINT, shrinking this surface to zero. |
| Naive route wiring silently ships option (a) | Medium | High | The use-cases ALREADY behave as (a) (`is_workspace_admin` on the bound user). Wiring GET/DELETE routes without an explicit authz decision = shipping (a) by accident. DA2 makes the decision explicit and the walking skeleton PROVES the chosen model (authorized vs refused) before any mint. |
| Bearer token used for management is broader than intended (admin minted a "CI bot" token not realising it could also manage tokens) | Medium | Medium | Under (c), a bearer token can LIST + REVOKE but not MINT — least surprise for the "kill a stale integration" job. Under (b), management requires an explicit claim the admin must deliberately grant. |
| Cross-workspace enumeration via the API | Low | High | Reused from the use-cases: `revoke_token` returns non-enumerable `NotFound` cross-workspace; LIST is workspace-scoped by the principal; the bearer extractor's 401 is byte-identical for every failure class. |
| No DIVERGE validation of the NEW jobs | Low | Low | The use-cases are shipped/tested; the only real unknowns are the authz model (Q-AUTHZ) and the revoke verb (Q-REVOKE-VERB), both flagged. |

## Open questions — STATUS: AWAITING USER RATIFICATION (2026-06-07)

- **Q-AUTHZ (the crux) → AWAITING RATIFICATION.** Recommended default: **(c) asymmetric** (bearer
  LIST + REVOKE incl. self; MINT human-session-only in v1; programmatic MINT deferred to a future
  slice with explicit `tokens:manage` capability + mint-rate guardrail). The stories + slices below
  are written to this recommendation; if the user picks (a) or (c)+(b), Slice 3 (MINT) is added
  back with the corresponding capability gate + guardrails.
- **Q-REVOKE-VERB → default `DELETE /api/v1/.../tokens/{jti}`** (DESIGN picks vs `POST .../revoke`).
- **Q-REVOKE-SELF → default YES** (revoke-self allowed; safest mutation).
- **Q-SIGNER-WIRING → N/A in v1** (MINT deferred); becomes a DESIGN task if MINT is ratified in.
- **Q-RATE-LIMIT → default a guardrail metric + per-principal cap on management mutations**
  (numbers/mechanism = DESIGN).
- **Q-LIST-SHAPE → default mirror `TokenView`** (no value field, ever).

## Open questions — RATIFIED by user 2026-06-07

- **Q-AUTHZ (the crux) → (c) ASYMMETRIC.** A machine-token bearer may **LIST + REVOKE** machine tokens (including revoke-self / rotation), gated by the existing `is_workspace_admin` check on the bound user — **no use-case change**. **MINT is NOT exposed via the API** (returns 403 / no route); provisioning stays human-session-only via the `/admin/tokens` UI. This removes the mint-loop / self-replication escalation entirely; the worst case is a loud, reversible, workspace-confined DoS (revoke storm), mitigated by the rate guardrail. The capability-claim path (b) is explicitly DEFERRED — if programmatic mint is ever needed, it lands as a follow-up behind a `tokens:manage` claim + mint-rate cap + no-mint-management-token rule.
- **Q-REVOKE-VERB → `DELETE /api/v1/.../tokens/{jti}`** (RESTful).
- **Q-REVOKE-SELF → ALLOWED** (the core of programmatic rotation).
- **Q-RATE-LIMIT → per-principal mutation guardrail + metric** (revoke-storm protection).
- **Q-LIST-SHAPE → JSON array of token metadata, never a value** (same no-value guarantee as the registry/UI).
- All API security guarantees inherit `machine-token-admin-ux`: one-time semantics (N/A here — no mint), never-persisted/logged, non-enumerable cross-workspace, revoke effective on the next request (shipped denylist), `created_by`/actor audit.
