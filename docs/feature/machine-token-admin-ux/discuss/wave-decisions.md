# Machine-Token Admin UX — DISCUSS Wave Decisions

> This is the file DESIGN reads FIRST. This feature adds the **product surface** for a
> workspace admin to **mint, view, and revoke** machine tokens. The cryptographic and
> storage primitives already SHIPPED in Feature A (`web-tier-extraction`): server-side
> Ed25519 minting (`MachineTokenSigner::mint`), per-request verification
> (`MachineTokenVerifier`), the `machine_tokens` registry + `jti` revocation denylist, and
> the `is_workspace_admin` authz check. Feature A explicitly DEFERRED the admin surface and
> the `created_by` audit column ("no issuer call-site existed"). **This feature is that
> issuer call-site.**

## Feature Summary

Foundry has machine tokens today — external agents/integrations authenticate to `/api/v1`
with a bearer JWT, and revocation works via the `jti` denylist that the per-request
`token_auth` extractor already honors. BUT tokens can only be minted via env/test keys
(`foundry_auth::test_keys`, the boot self-test). There is **no way for a workspace admin to
issue, see, or revoke a token through the product.** This feature adds:

- **Mint** — an admin issues a new token (server-side Ed25519 signing via the existing
  `MachineTokenSigner::mint`); the token value is shown **exactly once** at creation.
- **List** — the admin sees the issued-token registry for their workspace
  (`list_machine_tokens`): label, scope, expiry, who minted it, status (active/revoked).
- **Revoke** — the admin flips `revoked_at` (`revoke_machine_token(jti)`); the
  already-shipped per-request denylist refuses the credential on its very next use.
- **Audit** — re-introduce the deferred `created_by` column so the registry records WHO
  minted each token, surfaced in the list view.

Feature type: **cross-cutting** (auth/security posture change + admin API + admin UI).

Target end state (this feature adds only the admin issuer/registry surface; crypto + store
already shipped):

```
foundry (one binary)
├── foundry-auth     MachineTokenSigner::mint / MachineTokenVerifier   (SHIPPED — reused)
├── foundry-store    machine_tokens repo: insert/list/revoke/find/touch (SHIPPED — reused; + created_by, NEW)
├── is_workspace_admin(workspace, user)                                (SHIPPED — reused for authz)
└── admin surface (NEW: this feature)
    ├── admin UI screens (Askama templates over base.html + views.rs)  ──┐ surface choice
    └── OR/AND  POST/GET/DELETE /api/v1/.../tokens (foundry-services)   ──┘ is an OPEN question
    requires: a MachineTokenSigner LIVE in AppState (NEW — security posture change)
```

## Phase 1 — Discovery & Job Grounding

### No DIVERGE directory (RISK, low impact)
There is no `docs/feature/machine-token-admin-ux/diverge/` (no validated
`recommendation.md`/`job-analysis.md`). The jobs in `jobs.yaml` are NEW and Luna-derived
from (a) the brief, (b) a fresh 2026-06 reading of the shipped Feature-A primitives, and
(c) JTBD method. Importances/satisfactions are Luna estimates pending user/field
validation. Mitigation: the crypto + storage substrate is shipped and tested, so the blast
radius of a mis-ranked job is bounded to the surface (UI vs API) and the scope/expiry model
— both flagged as open questions below.

### What was grounded by reading the actual 2026-06 code (not assumed)
- `crates/foundry-auth/src/lib.rs`: `MachineTokenSigner::mint(&MachineTokenClaims) ->
  Result<SecretString, AuthError>` EXISTS. The minted JWT is wrapped in `SecretString` so
  it can never be `Debug`/`Display`-logged. `MachineTokenClaims { sub, scope: Option<Uuid>,
  iat, exp, jti, iss, aud }` — `iss`/`aud` are pinned to the single-issuer constants on
  mint. `MachineTokenSigner::from_pkcs8_pem(&SecretString)` builds the signer from
  `MACHINE_TOKEN_SIGNING_KEY`.
- `crates/foundry-store/src/lib.rs`: the `machine_tokens` repo is present —
  `insert_machine_token(jti, user_id, workspace_id, scope_team_id, expires_at, label)`,
  `find_machine_token_by_jti(jti)`, `revoke_machine_token(jti)` (`SET revoked_at = now()`),
  `list_machine_tokens(workspace_id)` (newest first), `touch_machine_token_last_used(jti)`.
  `MachineTokenRow { jti, user_id, workspace_id, scope_team_id, expires_at, revoked_at,
  last_used_at, label }`.
- `crates/foundry-store/migrations/0007_machine_tokens.sql`: the table has columns
  `jti, user_id, workspace_id, scope_team_id, expires_at, revoked_at, last_used_at, label,
  created_at`. **There is DELIBERATELY NO token/hash/secret column** ("Persisting the token
  would defeat the point of a self-contained signed credential and create an exfiltration
  target"). **There is NO `created_by` column** — confirmed; the migration comment ties the
  whole table to US-W05b and `insert_machine_token` takes no `created_by` parameter.
- `is_workspace_admin(workspace_id, user_id)` EXISTS (checks
  `workspace_memberships.role = 'admin'`).
- `crates/foundry-app/src/lib.rs`: **`AppState` holds `machine_token_verifier:
  Arc<MachineTokenVerifier>` ONLY — there is NO signer in AppState.** The signing key is
  read transiently at boot for the self-test, not retained. So minting-through-the-product
  requires placing a `MachineTokenSigner` (the Ed25519 PRIVATE key) into the running
  process — a real security-posture change (see DM1).
- Web tier renders via Askama templates extending `base.html`, with the browser
  auth/CSRF/session machinery intact (double-submit CSRF cookie + `HX-CSRF`, tower-sessions
  Postgres store, 30-day cookie, argon2id, brute-force delay, non-enumerable sign-in error).

## Phase 2 — Scope Assessment (Elephant Carpaccio Gate)

### Scope Assessment: PASS — 7 stories (1 `@infrastructure`), 1 bounded-context surface (admin), estimated ~8-12 days
Oversize signals checked: 7 stories (≤10 OK); the substrate (crypto + store + authz) is
SHIPPED so the walking skeleton needs few NEW integration points (signer-in-AppState +
one mint handler + one template/endpoint); effort estimate is well under 2 weeks; the
mint/list/revoke/audit outcomes are sliceable but ship coherently as one admin surface.
No oversize signal trips. The feature is right-sized for one DISCUSS→DELIVER pass with a
thin walking skeleton (mint ONE token end-to-end) first. No split needed.

## Key Decisions

| # | Decision | Rationale |
|---|----------|-----------|
| **DM1** | **The app must hold a `MachineTokenSigner` (Ed25519 PRIVATE key) live in `AppState` to mint through the product.** This wave captures it as a REQUIREMENT + RISK; the mechanism is DESIGN. | Today `AppState` is verifier-only and the signing key is read transiently at boot. Minting-on-demand means the running process becomes an ISSUER, expanding the key-exposure surface. **OPEN QUESTION — user must accept this posture change before DESIGN** (see Open Questions Q1). |
| **DM2** | **The minted token value is shown EXACTLY ONCE at creation and is never retrievable again.** Only the `jti` + metadata persist (the table already has no secret column). | Matches the shipped table design (no secret/hash column — deliberate, anti-exfiltration). The UX must make "copy it now or lose it" unmistakable. Re-issuance = mint a NEW token, not recover the old one. |
| **DM3** | **Minting and revoking are workspace-admin-only**, reusing `is_workspace_admin(workspace_id, user_id)`. A non-admin gets a non-enumerable refusal. | The check already exists and is the established authz primitive. Surfaced as Open Question Q4 to confirm admin-only (vs a future finer-grained "token-manager" role). |
| **DM4** | **Re-introduce the deferred `created_by` audit column** on `machine_tokens` (forward-only migration per ADR-003), populated at mint with the acting admin's `user_id`, surfaced in the list view. | Feature A deferred it because "no issuer call-site existed". This feature IS the issuer call-site, so the registry can finally record WHO minted each token. `insert_machine_token` gains a `created_by` parameter. |
| **DM5** | **Scope + expiry are admin-chosen within server-enforced bounds.** Admin picks a label, a scope (workspace-wide or a specific team via `scope_team_id`), and an expiry/TTL; the server enforces a maximum cap and a sane default. | The claim set already supports `scope: Option<Uuid>` and `exp`. The exact scope vocabulary (read/write? team-only?) and the TTL default/cap are OPEN (Q3). DISCUSS fixes that bounds EXIST and are server-enforced; DESIGN picks the numbers. |
| **DM6** | **Surface (web admin UI vs JSON API vs both) is an OPEN question (Q6).** The user-facing stories are written surface-neutral where possible; each story's Elevator Pitch names a CONCRETE entry point (an admin UI action AND/OR `POST /api/v1/.../tokens`) so DESIGN can pick without re-discovering the need. | The brief says DISCUSS frames the need and DESIGN picks. Default assumption pending Q6: a **web admin UI** (consistent with the just-shipped Askama web tier) is the primary surface; a JSON API is a strong candidate since the consumers are themselves programmatic. |
| **DM7** | **Solution-neutral.** Where the signing key lives at rest, how it is loaded into AppState, the template engine/route shapes, the exact TTL numbers, and the scope vocabulary are DESIGN. | DISCUSS fixes the constraints (one-time display, no secret persisted, admin-only, revocation effective next request, audit trail, signing-key posture) and the observable outcomes; DESIGN picks the mechanism. |
| **DM8** | **Output uses the LEGACY per-feature layout** (separate files under `discuss/`), NOT the SSOT/feature-delta model; story IDs use the `US-MT0x` namespace. | Decided with the user; `docs/product/` does not exist and we are intentionally not migrating. Mirrors `foundry-backend-mvp/discuss/` and the recent features. `US-MT0x` distinguishes this feature. |

## Open Questions for the User + DESIGN

| # | Question | Why it matters | Default assumption if unanswered |
|---|----------|----------------|----------------------------------|
| **Q1** | **Signing-key-in-the-running-process.** Do we accept that minting-through-the-product puts the Ed25519 PRIVATE key live in `AppState` (the app becomes an issuer, not just a verifier)? | This is a genuine security-posture change. Today only `MACHINE_TOKEN_PUBLIC_KEYS` is retained at runtime; the private key is read transiently at boot. Minting on demand expands the key-exposure surface (memory, crash dumps, an attacker who reaches the process can mint). **Highest-priority confirmation.** | Accept the posture for issuer binaries only; DESIGN specifies how the key is loaded/guarded and whether issuer capability is a separate, explicitly-enabled binary/config mode. |
| **Q2** | **One-time display semantics.** Confirm the token value is shown once at creation and never retrievable; the registry stores only `jti` + metadata (matches the shipped table). | Drives the mint UX ("copy it now or lose it"), the revoke-and-reissue flow, and the AC that NO endpoint/screen ever re-displays a token value. | Confirmed (matches shipped table design); shown once, never re-shown; lose-it = mint a new one. |
| **Q3** | **Scope + expiry model.** What scope can an admin grant — workspace-wide vs a specific team (`scope_team_id`)? Read-vs-write? What is the TTL default and the maximum cap? | The claim set supports `scope: Option<Uuid>` and `exp`. The vocabulary and the numbers shape the mint form and the AC. | Scope = workspace-wide OR one team (via existing `scope_team_id`); no read/write split in v1 (the principal `sub`'s membership already bounds authorization); admin-chosen TTL with a server max cap (e.g. 90 days) and a sensible default (e.g. 30 days) — **DESIGN picks the numbers.** |
| **Q4** | **Issue/revoke authz.** Workspace-admin only (reuse `is_workspace_admin`), or a future finer-grained role? | Determines who sees the admin surface and who gets refused. | Workspace-admin only for v1, reusing `is_workspace_admin`; non-admins get a non-enumerable refusal. |
| **Q5** | **`created_by` audit reach.** Confirm re-introducing the deferred `created_by` column and surfacing "minted by {admin}" in the list view. | A forward-only migration + a new `insert_machine_token` parameter + a list-view column. | Confirmed; add `created_by UUID REFERENCES users(id)`, populate at mint, surface in list. |
| **Q6** | **Surface.** Web admin UI (Askama), JSON API (`/api/v1/.../tokens`), or both? | The consumers of these tokens are programmatic, so an API is natural; but the admin doing the issuing is a human, so a UI is natural. The brief says DISCUSS frames, DESIGN picks. | Web admin UI as the primary human surface (matches shipped web tier); a JSON API is a strong fast-follow. Stories' Elevator Pitches name both concrete entry points so DESIGN can choose. |

## Requirements Summary

- 7 user stories, 1 explicitly `@infrastructure` (US-MT00, signer-in-AppState + `created_by`
  migration; the substrate the user-visible stories stand on — folded into the walking
  skeleton, never shipped standalone).
- **Walking skeleton (Slice 1) — mint ONE token end-to-end, value shown once:**
  - US-MT00 — signer live in AppState + `created_by` migration (`@infrastructure`, folded).
  - US-MT01 — admin mints a token and sees its value exactly once.
- **Slice 2 — see what exists (the registry):**
  - US-MT02 — admin lists the workspace's issued tokens with status + who minted each.
- **Slice 3 — take it back (revocation):**
  - US-MT03 — admin revokes a token; it is refused on its next API use.
- **Slice 4 — make the issuance trustworthy (guardrails + audit polish):**
  - US-MT04 — admin chooses scope + expiry within server-enforced bounds at mint time.
  - US-MT05 — non-admins cannot mint, list, or revoke (authz boundary).
  - US-MT06 — the list view shows "minted by {admin}" and last-used, for audit.
- NFRs (SECURITY-heavy): signing-key handling, one-time display never re-shown, no token
  value ever persisted, revocation effective on the next request, admin-only authz, audit
  trail; plus the invariants (one binary, browser auth/CSRF/sessions for the admin UI).

## Constraints Established

- ONE binary, ONE Postgres, no Redis, no Node runtime service, no CDN.
- The token value is shown EXACTLY ONCE at mint and is NEVER persisted or re-displayed; the
  registry stores only `jti` + metadata (matches the shipped table — no secret column).
- Revocation is a FLAG (`revoked_at`), honored by the already-shipped per-request denylist;
  the row survives revocation so the check keeps refusing until expiry/GC.
- Minting and revoking are workspace-admin-only (reuse `is_workspace_admin`).
- Reuse (don't rebuild) the shipped primitives: `MachineTokenSigner::mint`, the
  `machine_tokens` repo, `MachineTokenVerifier`, `is_workspace_admin`.
- Admin UI (if chosen) preserves the existing browser auth/CSRF/session contract unchanged.
- Solution-neutral: signing-key-at-rest mechanism, surface (UI/API), TTL numbers, and scope
  vocabulary are DESIGN.

## Risks Surfaced (for DESIGN's risk register)

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|-----------|
| Signing key live in AppState widens the key-exposure surface (memory, crash dumps, in-process attacker can mint) | Medium | High | DM1 / Q1: explicit user acceptance; DESIGN scopes issuer capability (separate binary/config mode), guards the key, and documents the threat delta vs verifier-only today. |
| One-time token leaks via logs, error messages, or a half-rendered page | Medium | High | The minted JWT is already a `SecretString` (no Debug/Display); NFR forbids logging it and forbids any re-display path; mint render must be all-or-nothing. |
| Admin loses the token before copying (no recovery path by design) | High | Low | Mint UX makes "copy now or lose it" unmistakable; recovery = mint a new token + revoke the lost one; AC covers the reissue flow. |
| Revocation believed effective but a cached/long-lived token still works | Low | High | Revocation is per-request via the SHIPPED denylist (no token cache); AC asserts refusal on the NEXT API call after revoke; guardrail metric on revoke-to-refusal latency. |
| `created_by` migration is forward-only but back-fills NULL for any pre-existing rows | Low | Low | New column is NULLABLE; pre-feature rows (if any) show "minted by —/unknown"; only new mints record the issuer. |
| Scope/expiry vocabulary mis-modeled (e.g. read/write needed but not built) | Medium | Medium | Q3 flags it; v1 reuses existing `scope_team_id` + `exp`; DESIGN confirms before building the mint form. |
| Non-admin can reach the mint/revoke surface (authz gap) | Low | High | US-MT05 asserts admin-only via `is_workspace_admin`; non-enumerable refusal; covered by an explicit "evil user" scenario. |
| No DIVERGE validation of the NEW jobs | Medium | Low | Substrate is shipped/tested; surface + scope/expiry are the only real unknowns and are flagged as Q3/Q6; confirm job ranking before DESIGN. |

## Open questions — RATIFIED by user 2026-06-07

- **Q1 (signing-key posture) → ACCEPTED: signer in-process.** `AppState` gains a `MachineTokenSigner` loaded from `MACHINE_TOKEN_SIGNING_KEY`; mint signs in-process. The app becomes a token issuer; the private key lives in the running process (acceptable for a self-hosted one-binary product). DESIGN owns hardening (no-log, secrecy/zeroize; the key-material startup probe already exists).
- **Q3 (scope + expiry) → workspace OR team scope (existing `scope_team_id`) + REQUIRED TTL with a max cap (e.g. 1y); NO read/write split in v1.**
- **Q6 (surface) → WEB ADMIN UI FIRST** (Askama `/admin/.../tokens` screens: mint form → one-time display, list, revoke). A JSON token-management API is a deferred fast-follow.
- **Q2 → CONFIRMED**: the minted token value is shown ONCE at creation and never retrievable/persisted (only the jti + metadata persist).
- **Q4 → CONFIRMED**: mint/revoke are workspace-admin-only (reuse `is_workspace_admin`).
- **Q5 → CONFIRMED**: re-introduce the `created_by` audit column (who minted), surfaced in the list view (new forward-only migration; Feature A deferred it).
