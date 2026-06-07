# Machine-Token Admin UX — DESIGN Wave Decisions

Owner: solution-architect (Morgan). Scope: the **admin product surface** for machine-token
issuance + lifecycle (mint / one-time display / list / revoke / scope+TTL choice / authz /
audit) over the SHIPPED Feature-A primitives. Interaction mode: **Propose**. This file is the
central wave record: decisions table, ADRs (MADR-style, kept here — mirroring how Feature A and
backend-mvp keep ADRs close to the wave), the Reuse Analysis tally, the technology stack with
rationale, constraints, and the **Open decisions awaiting user ratification** list.

Output uses the LEGACY per-feature layout under `docs/feature/machine-token-admin-ux/design/`
(NOT `docs/product/` SSOT), per DISCUSS DM8. Companion docs: `architecture.md` (C4 +
component decomposition + Reuse table), `signer.md`, `admin-routes.md`, `data-and-migration.md`,
`token-admin-services.md`, `error-and-observability.md`.

> **DISCUSS authority.** The four open DISCUSS questions were **RATIFIED by the user 2026-06-07**
> (discuss/wave-decisions.md §"Open questions — RATIFIED"): Q1 signer in-process; Q3 workspace OR
> team scope (`scope_team_id`) + REQUIRED TTL with a max cap, no read/write split v1; Q6 web admin
> UI first; Q2/Q4/Q5 confirmed. DESIGN does NOT re-open those. What DESIGN owns and proposes here:
> the **at-rest key mechanism + signer-absent behaviour** (within the ratified "signer in-process"),
> the **migration nullability shape**, the **route path/placement**, the **scope-UI shape**, and the
> **exact TTL numbers** — surfaced as the four "Open decisions awaiting user ratification" below,
> each with a recommendation.

## Architecture summary

This feature turns the running binary from a token **verifier** into a token **issuer**, and adds
a **browser admin surface** to drive it — entirely by EXTENDING shipped primitives. A
`MachineTokenSigner` (Ed25519 private key) is loaded into `AppState` at boot **mirroring the
existing verifier load** (env → `SecretString` → `from_pkcs8_pem`), reusing the *already-present*
boot self-test probe (`MachineTokenVerifier::self_test`, foundry-auth) as the wire-then-probe-then-use
gate. A new **`foundry_services::tokens`** module holds the three use-cases (`mint` / `list` /
`revoke`) — the same seam Feature A established (`foundry-services` owns the only `Arc<Store>`),
so authz (`is_workspace_admin`), claims construction, scope mapping, and TTL validation live in
ONE neutral place. New **Askama admin screens** (`/admin/tokens` under the existing browser
session + CSRF layers — NOT the CSRF-exempt `/api/v1` path) render mint → one-time display → list
→ revoke, following the shipped `projects.rs` handler + `views.rs` view-model pattern verbatim.
A single **forward-only migration `0008_machine_tokens_created_by.sql`** adds the deferred
`created_by` audit column. **No new crates. No new runtime services. One binary, one Postgres.**

The riskiest thing here is not code volume — it is the **security posture change** (a live signing
key) and the **one-time-secret display** (the only place a token value ever crosses the wire). Both
are addressed by EXTENDING existing hardened paths (`SecretString` end-to-end, the existing boot
probe, the existing `force_board_render_failure` all-or-nothing render seam) rather than inventing
new ones. Full detail in `architecture.md`, `signer.md`, `admin-routes.md`,
`data-and-migration.md`, `token-admin-services.md`, `error-and-observability.md`.

## Decisions table (DDD-numbered, this wave)

| # | Decision | Rationale |
|---|---|---|
| DD1 | **`MachineTokenSigner` lives in `AppState` as `Option<Arc<MachineTokenSigner>>`, loaded at boot from `MACHINE_TOKEN_SIGNING_KEY` exactly mirroring the verifier load.** Present ⇒ issuer binary; `None` ⇒ verifier-only. (ADR-MT01, signer.md) | The ratified posture (Q1) is "signer in-process". `Option` makes "issuer vs verifier-only" a TYPE-LEVEL fact the mint surface keys off — a verifier-only binary structurally cannot mint and offers no mint surface (NFR-MT-SEC-04, US-MT00 scenario 2). Reuses `from_pkcs8_pem` + the SHIPPED `\n`-normalization the boot path already does for the public keys. |
| DD2 | **Signer-absent behaviour = mint disabled + UI hidden / 403 (graceful), NOT a hard-require at boot.** The boot probe still runs WHEN a signing key is present. (ADR-MT01, signer.md, Open Decision OD1) | A self-hosted verifier-only replica is a legitimate, ratified configuration (US-MT00 scenario 2: "the server does not error"). Hard-requiring the key would break that deployment. The existing boot self-test (main.rs ~199-216) already runs only `if let Ok(signing_key) = env::var(...)` — DD2 keeps that exact shape. |
| DD3 | **The three token-admin use-cases live in a new `foundry_services::tokens` module**, reusing `MachineTokenSigner` (passed in, NOT stored in `Services`), the `machine_tokens` repo, and `is_workspace_admin`. (ADR-MT02, token-admin-services.md) | This is the seam Feature A built (ADR-W04/W07). Putting authz + claims construction + scope mapping + TTL validation in ONE neutral function per use-case makes the admin-only invariant (US-MT05) and the bounds (US-MT04) structural and testable without a running server — and keeps a future JSON token API (deferred fast-follow) a thin second adapter over the SAME functions. |
| DD4 | **The signer is passed to `mint` as a parameter, not held inside `Services`.** `Services` stays exactly as Feature A shaped it (owns `Arc<Store>` only). (ADR-MT02, token-admin-services.md) | `Services` is reconstructed per request via `FromRef` from `Arc<Store>` alone (lib.rs:153). Threading the `Option<Arc<MachineTokenSigner>>` through `FromRef` would change that contract and the signer would leak into every use-case. Instead the admin handler reads `state.machine_token_signer` and passes it to `services.mint_token(signer, …)` — the signer is confined to the mint call path. |
| DD5 | **New Askama admin screens under `/admin/tokens`, mounted INSIDE the existing session + CSRF layers** (NOT the `/api/v1` CSRF-exempt mount). (ADR-MT03, admin-routes.md, Open Decision OD3) | The admin is a HUMAN with a browser cookie (NFR-MT-SEC-07). Feature A's CSRF exemption is ONLY for `/api/v1` (bearer, no cookie). Mounting under the same layers as `projects.rs` means the double-submit `_csrf` + tower-sessions contract applies unchanged and the existing browser-auth acceptance suite stays green. |
| DD6 | **Forward-only migration `0008_machine_tokens_created_by.sql` adds `created_by UUID NULL REFERENCES users(id) ON DELETE SET NULL`.** Nullable, no backfill. `insert_machine_token` gains a `created_by` parameter. (ADR-MT04, data-and-migration.md, Open Decision OD2) | Ratified Q5. Nullable is simplest for the 0 existing rows (DISCUSS confirms back-fill NULL) and matches the 0006 three-nullable-columns precedent. `ON DELETE SET NULL` (NOT CASCADE) so deleting an admin user does NOT vaporize the token registry rows — audit history survives as "minted by —" (US-MT06 edge path). Forward-only per ADR-003. |
| DD7 | **The minted token value is a `SecretString` from `AppState` boot to the one-time render, and is dropped immediately after rendering. It is NEVER stored, logged, or returned by any read path.** (NFR-MT-SEC-01/02, error-and-observability.md) | The mint primitive already returns `SecretString` (no `Debug`/`Display`). DESIGN's job is to keep it secret end-to-end: expose it exactly once into the one-time-display view-model, render all-or-nothing (reuse the `force_board_render_failure` seam pattern, NFR-MT-REL-01), and let it drop. No `token` field on any list/detail view-model. |
| DD8 | **Required TTL with a server max cap of 365 days and a default of 90 days.** Validated in the service. (token-admin-services.md, Open Decision OD4-numbers) | Ratified Q3 fixed "REQUIRED TTL with a max cap (e.g. 1y)". DESIGN picks: cap = 365d, default = 90d (both constants in `foundry_services::tokens`, overridable by a future env if needed — not v1). An expiry over the cap is a `Validation` refusal (US-MT04 scenario 2); the cap is stated in the message. |
| DD9 | **Scope UI = a radio "Whole workspace / A specific team" + a team picker; maps to `scope_team_id` (None = workspace).** No read/write split. (admin-routes.md, token-admin-services.md, Open Decision OD4-shape) | Ratified Q3 (workspace OR team via existing `scope_team_id`, no read/write split v1). The service validates the chosen team belongs to the acting workspace before minting (US-MT04 scenario 3 evil-user path). The claim `scope` carries the team id; the SHIPPED `board::list_board_issues` scope-narrowing already enforces it per request. |
| DD10 | **Revoke reuses the SHIPPED `revoke_machine_token(jti)` + the SHIPPED per-request `jti` denylist; the admin path adds ONLY workspace-isolation + admin authz + a confirm.** No new refusal code. (token-admin-services.md, admin-routes.md) | DM-level: revocation effectiveness is already true (the `resolve_active_token` denylist check, foundry-services::auth). The service revokes only after confirming the `jti` belongs to the acting workspace (US-MT03 scenario 3, NFR-MT-REL-03) — a cross-workspace `jti` is a non-enumerable not-found. Revoke is idempotent because the UPDATE just re-stamps `revoked_at` (NFR-MT-REL-02). |
| DD11 | **Architecture enforcement: the existing boundary guard (`xtask check-arch` + `cargo-deny bans`) already bans `foundry-app ⊀ foundry-store` and the like; this feature adds NO new crate edge, so it inherits enforcement for free.** The Earned-Trust signer probe is enforced by the existing boot self-test path. | No new enforcement tooling needed (principle 11). The one NEW structural rule worth a guard assertion: "no `machine_tokens` migration adds a token/secret/hash column" (NFR-MT-DATA-02) — recommend a one-line static check in the existing migration-review gate. |

## ADRs (MADR-style)

### ADR-MT01 — Signer in AppState as `Option`, graceful when absent
- **Status**: Accepted (within ratified Q1).
- **Context**: Minting requires the Ed25519 private key live in-process (ratified posture change).
  A verifier-only binary is a legitimate deployment (US-MT00 scenario 2). The verifier is already
  loaded at boot from `MACHINE_TOKEN_PUBLIC_KEYS`; a transient signing-key self-test already exists
  (main.rs ~199-216) but the signer is NOT retained.
- **Decision**: Add `machine_token_signer: Option<Arc<MachineTokenSigner>>` to `AppState`. Load it
  in main.rs in the SAME `if let Ok(signing_key) = env::var("MACHINE_TOKEN_SIGNING_KEY")` block that
  already runs the self-test — on success, retain the signer; the self-test stays as the
  wire-then-probe-then-use gate. Absent ⇒ `None` ⇒ no mint surface, graceful "issuing not enabled".
- **Alternatives considered**:
  - *Hard-require the key at boot* — rejected: breaks the ratified verifier-only deployment; turns
    US-MT00 scenario 2 ("does not error") into a refuse-to-start.
  - *Separate issuer binary / build feature* — rejected for v1: more operational surface (two
    artifacts) for a self-hosted one-binary product; `Option` in one binary achieves the same
    "verifier-only cannot mint" guarantee with zero new build/deploy complexity. Revisit if a
    deployment wants the private key physically absent from the verifier artifact.
  - *Issuer-mode env flag separate from the key* — rejected: the presence of the signing key IS the
    mode; a second flag can only disagree with reality.
- **Consequences**: + verifier-only stays a first-class config; + reuses the existing probe; +
  type-level "can this binary mint?". − the private key is in the issuer process's memory (the
  ratified, accepted posture; hardening in signer.md). − a deployment that wants the key absent from
  the verifier artifact must wait for the deferred separate-binary option.

### ADR-MT02 — Token-admin use-cases in `foundry_services::tokens`
- **Status**: Accepted.
- **Context**: Mint/list/revoke need authz (`is_workspace_admin`), claims construction, scope
  mapping, TTL validation, and the one-time-secret return — orchestration that must be identical
  whether driven by the web UI now or a JSON API later, and must not let the adapter name
  `foundry_store::Store` (boundary guard).
- **Decision**: A new `foundry_services::tokens` module with `mint_token` / `list_tokens` /
  `revoke_token` free functions (and thin `Services` methods delegating to them, matching
  `board`/`issues`/`comments`). `mint_token` takes `&MachineTokenSigner` as a parameter.
- **Alternatives considered**:
  - *Orchestrate in the `foundry-app` handler* — rejected: re-inlines use-case logic in an adapter
    (the exact anti-pattern Feature A's ADR-W04 removed) and would make a future JSON token API
    duplicate authz/validation.
  - *Hold the signer inside `Services`* — rejected (DD4): changes the `FromRef`-from-`Arc<Store>`
    contract and leaks the key into every use-case.
- **Consequences**: + one neutral home for authz + bounds (testable without a server); + a future
  JSON token adapter is thin; + boundary guard satisfied for free. − `mint_token`'s signature
  carries the signer parameter (confined, intentional).

### ADR-MT03 — Admin screens under `/admin/tokens`, inside session + CSRF
- **Status**: Accepted (within ratified Q6).
- **Context**: The admin is a browser human; the existing browser auth/CSRF/session contract must
  be preserved unchanged (NFR-MT-SEC-07). Feature A's CSRF exemption is ONLY for `/api/v1`.
- **Decision**: New routes mounted in `build_router` ALONGSIDE the existing HTML routes (so they
  sit under `csrf::csrf_middleware` + `session_layer`), NOT merged via `foundry_api::routes`.
  Path `/admin/tokens` (workspace-implicit from the session). Handlers follow `projects.rs`:
  `signed_in_user` → resolve workspace from session → `is_workspace_admin` gate → Askama view-model.
- **Alternatives considered**:
  - *Workspace-scoped path `/workspaces/{ws}/admin/tokens`* — rejected for the web UI v1: the
    session already binds exactly one workspace (slice-1 single-workspace model; `SessionUser`
    carries `workspace_id`), so the path segment is redundant and invites a cross-workspace
    mismatch bug. A future JSON API SHOULD use the explicit `/api/v1/workspaces/{ws}/tokens` form
    (the consumer is programmatic and may act across workspaces) — noted for the fast-follow.
  - *Reuse the `/api/v1` mount* — rejected: that path is CSRF-exempt and session-free by
    construction; a browser POST there would have no CSRF protection (NFR-MT-SEC-07 violation).
- **Consequences**: + CSRF + session apply unchanged; + mirrors `projects.rs` exactly; + browser
  suite stays green. − the web path is workspace-implicit (correct for the single-workspace session
  model; the JSON fast-follow carries the explicit `{ws}`).

### ADR-MT04 — `created_by` forward-only, nullable, `ON DELETE SET NULL`
- **Status**: Accepted (ratified Q5).
- **Context**: Re-introduce the deferred audit column; 0 existing rows; FK to `users(id)`.
- **Decision**: `0008_machine_tokens_created_by.sql`: `ALTER TABLE machine_tokens ADD COLUMN
  created_by UUID NULL REFERENCES users(id) ON DELETE SET NULL;`. `insert_machine_token` gains a
  `created_by: uuid::Uuid` parameter, persisted on every new mint.
- **Alternatives considered**:
  - *NOT NULL + backfill sentinel* — rejected: there are 0 existing rows, so NOT NULL would still
    need a sentinel `users` row to satisfy the FK and complicate the migration for zero benefit;
    nullable + "minted by —" for the (non-existent) legacy rows is simpler and matches US-MT00
    scenario 3 / NFR-MT-DATA-01 verbatim.
  - *`ON DELETE CASCADE`* — rejected: deleting an admin would DELETE their issued-token registry
    rows, destroying audit history and silently un-revoking nothing but losing the record; SET NULL
    preserves the row and degrades to "minted by —".
  - *`ON DELETE RESTRICT`* — rejected: would block deleting any admin who ever minted a token.
- **Consequences**: + non-destructive, forward-only; + audit survives admin deletion as "—"; +
  matches the 0006 nullable-columns precedent. − rows for a deleted admin lose attribution (the
  honest, acceptable outcome).

## Reuse Analysis (MANDATORY) — verdict: 7 EXTEND/REUSE, 5 CREATE NEW

| Capability | Verdict | Evidence (file) |
|---|---|---|
| Ed25519 mint primitive (`MachineTokenSigner::{from_pkcs8_pem, mint}`) | **REUSE** | `crates/foundry-auth/src/lib.rs:113,124` |
| Claim set (`MachineTokenClaims {sub,scope,iat,exp,jti,iss,aud}`) | **REUSE** | `crates/foundry-auth/src/lib.rs:66-90` |
| Boot keypair self-test probe (`MachineTokenVerifier::self_test`) | **EXTEND** (retain signer after it passes) | `crates/foundry-auth/src/lib.rs:201`; `crates/foundry-app/src/main.rs:199-216` |
| `machine_tokens` repo (`insert/list/revoke/find/touch`) | **EXTEND** (`insert` gains `created_by`) | `crates/foundry-store/src/lib.rs:1380,1410,1430,1441,1461` |
| Per-request `jti` denylist / revocation effectiveness | **REUSE** (unchanged) | `crates/foundry-services/src/lib.rs:256` (`resolve_active_token`) |
| `is_workspace_admin` authz | **REUSE** | `crates/foundry-store/src/lib.rs:1031`; call sites in `comments.rs:107` |
| Web tier (Askama view-models, session, CSRF double-submit, `base.html`, error fragment, render-failure seam) | **EXTEND** (new screens, same pattern + layers) | `crates/foundry-app/src/projects.rs`, `views.rs:29,53`, `csrf.rs`, `lib.rs:196-330` |
| `foundry_services::tokens` (mint/list/revoke use-cases) | **CREATE NEW** | new module beside `board`/`issues`/`comments` (`foundry-services/src/lib.rs:283,372`) |
| Askama admin screens + view-models (mint form, one-time display, list, revoke confirm) | **CREATE NEW** | new templates + `views.rs` structs (pattern: `projects.rs`/`views.rs`) |
| Admin route handlers (`/admin/tokens` GET/POST/revoke) | **CREATE NEW** | new `admin_tokens.rs` (pattern: `projects.rs`) |
| `0008_machine_tokens_created_by.sql` migration | **CREATE NEW** | new file (pattern: `0006_comments_edit_delete.sql`) |
| `AppState.machine_token_signer: Option<Arc<MachineTokenSigner>>` field | **CREATE NEW** | new field beside `machine_token_verifier` (`foundry-app/src/lib.rs:70`) |

**Tally: 7 EXTEND/REUSE, 5 CREATE NEW.** Every NEW item is the minimal issuer/registry surface
DISCUSS identified; the crypto, store, verify, authz, and web substrate are all reused. **Zero new
crates.**

## Technology stack with rationale

No new dependencies expected. Everything is already in the tree:

| Concern | Tool | License | Already present |
|---|---|---|---|
| Ed25519 sign/verify, JWT | `jsonwebtoken` (via foundry-auth) | MIT | yes (Feature A) |
| Secret handling | `secrecy::SecretString` | MIT/Apache-2.0 | yes |
| Templating | `askama` | MIT/Apache-2.0 | yes (web tier) |
| HTTP / routing | `axum`, `tower`, `tower-http` | MIT | yes |
| Sessions / CSRF | `tower-sessions` + project `csrf.rs` | MIT | yes |
| Persistence | `sqlx` (Postgres) | MIT/Apache-2.0 | yes |
| UUIDs / time | `uuid`, `time` | MIT/Apache-2.0 | yes |

## Constraints (carried from DISCUSS / NFRs)

- ONE binary, ONE Postgres, no Redis, no Node runtime service, no CDN.
- Token value shown EXACTLY ONCE, never persisted/logged/re-displayed; registry stores only
  `jti` + metadata (no secret column — NFR-MT-DATA-02). Signing key never logged.
- Mint/list/revoke are workspace-admin-only (`is_workspace_admin`), non-enumerable refusal.
- The admin UI is a BROWSER surface → CSRF + session APPLY (NOT the `/api/v1` exemption).
- Revocation effective on the next request via the SHIPPED denylist (reuse, don't rebuild).
- Required TTL + server max cap; workspace or team scope via existing `scope_team_id`.
- Reuse the shipped primitives; the `foundry-acceptance` suite green before stays green after.

## Open decisions awaiting user ratification (Propose mode)

Each is a DESIGN-owned mechanism choice WITHIN the already-ratified DISCUSS envelope. Recommendation
is the default if unanswered.

- **OD1 — Signer-absent behaviour.** Options: (a) **mint disabled + UI hidden / 403, graceful**
  [RECOMMENDED — DD2]; (b) hard-require the key at boot (refuse to start); (c) a separate
  issuer-mode env flag distinct from the key. *Why (a):* it is the only option that preserves the
  ratified verifier-only deployment (US-MT00 scenario 2 "does not error") and reuses the existing
  conditional boot probe unchanged.
- **OD2 — `created_by` migration nullability + FK delete behaviour.** Options: (a) **nullable, no
  backfill, `ON DELETE SET NULL`** [RECOMMENDED — DD6/ADR-MT04]; (b) NOT NULL + backfill sentinel;
  (c) nullable but `ON DELETE CASCADE`. *Why (a):* simplest for 0 existing rows, preserves audit
  history when an admin is deleted, matches the 0006 precedent and NFR-MT-DATA-01 verbatim.
- **OD3 — Route placement.** Options: (a) **`/admin/tokens`, workspace-implicit from session,
  inside session+CSRF** [RECOMMENDED — DD5/ADR-MT03]; (b) `/workspaces/{ws}/admin/tokens` explicit.
  *Why (a):* the slice-1 session binds exactly one workspace (`SessionUser.workspace_id`), so the
  path segment is redundant for the web UI and risks a cross-workspace mismatch; the explicit
  `{ws}` form is reserved for the deferred JSON API (programmatic, cross-workspace).
- **OD4 — Scope-UI shape + TTL numbers.** Options for shape: (a) **radio "Whole workspace / one
  team" + team picker → `scope_team_id`** [RECOMMENDED — DD9]; (b) a single nullable team dropdown.
  Numbers: **cap = 365 days, default = 90 days** [RECOMMENDED — DD8] vs other caps/defaults. *Why:*
  (a) makes "workspace-wide" an explicit, unmistakable choice (least-privilege framing) rather than
  an empty-dropdown default; 90d default / 365d cap matches the ratified "e.g. 1y" cap and a sane
  rotation cadence. **These numbers are the one place the user may most want to override.**

No DISCUSS assumption was challenged; `upstream-changes.md` is therefore NOT written for this wave.

## Open decisions — RATIFIED by user 2026-06-07 (all as recommended)

- **OD1 (signer-absent) → GRACEFUL**: if `MACHINE_TOKEN_SIGNING_KEY` is unset, mint is disabled and the mint UI is hidden / returns 403; the app stays verifier-only and boots fine (NOT a hard boot requirement). Signer retained in AppState only after the existing self-test probe passes.
- **OD2 (created_by migration) → NULLABLE, no backfill**: `0008` forward-only `ALTER TABLE machine_tokens ADD COLUMN created_by UUID NULL REFERENCES users(id) ON DELETE SET NULL`.
- **OD3 (routes) → `/admin/tokens`**, workspace-implicit (session binds the workspace), mounted INSIDE the session + CSRF layers (browser admin UI — `_csrf` applies; NOT the machine-API CSRF-exempt path). Gated by `is_workspace_admin`.
- **OD4 (TTL policy) → required expiry, DEFAULT 90 days, MAX CAP 365 days**; no never-expires option in v1. Scope = workspace or team via `scope_team_id`.
