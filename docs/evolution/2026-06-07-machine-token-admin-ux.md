# Evolution — machine-token-admin-ux

**Finalized**: 2026-06-07
**Ship commit**: `8940604` (tip; 6 DES steps + Phase-4 security fixes + mutation hardening) off `5d9d613` (roadmap)
**Wave coverage**: full nWave pipeline — DISCUSS → DESIGN → DISTILL → DELIVER (wave-by-wave with checkpoints; legacy per-feature layout; trunk-based, committed directly to `main`).

## Feature summary

The admin UX for **machine-token issuance + lifecycle**. Feature A (`web-tier-extraction`) shipped JWT/Ed25519 machine tokens for `/api/v1` but tokens could only be minted via env/test keys — no product surface. This feature adds the workspace-admin web UI at `/admin/tokens` to **mint** (server-side Ed25519 signing, value shown ONCE), **list** (the issued-token registry), and **revoke** (the jti-denylist kill-switch). It makes the running app a token **issuer** (previously verifier-only) — the central security decision.

## What shipped (security-bearing)

- **Signer in `AppState`**: `Option<Arc<MachineTokenSigner>>`, loaded from `MACHINE_TOKEN_SIGNING_KEY` and retained ONLY after the existing key-material self-test passes. `SecretString` end-to-end; omitted from `AppState` `Debug`; never logged on any path. **No key → graceful**: mint disabled, UI hidden / 403, app still serves (verify-only deployments boot fine).
- **Mint** (`foundry_services::tokens::mint_token`): admin-gated (`is_workspace_admin`, fail-closed) → TTL validation (required; default 90d; >365d cap rejected; ==365 accepted) → scope mapping (workspace or team via `scope_team_id`, team-belongs-to-workspace enforced) → claims → **sign before persist** → persist **metadata only** (`created_by` recorded; never the token value) → return the one-time `SecretString`, rendered once via `expose_secret()` then dropped.
- **List** (`/admin/tokens`): workspace-scoped, newest-first; rows show label/scope(team name)/created/expiry/last-used/status (active|revoked|expired) + **minted-by (resolved from `created_by`, the issuer)**. No token value on any surface (the registry has no value/secret column).
- **Revoke**: admin-gated, CSRF-protected, idempotent, **non-enumerable** (missing or foreign-workspace jti → identical NotFound). Reuses the **shipped per-request jti denylist** (Feature A `token_auth`) — the revoked token is refused 401 on its very next `/api/v1` call (no new enforcement code).
- **`0008` migration**: nullable `created_by UUID REFERENCES users(id) ON DELETE SET NULL` (forward-only; preserves audit history when an admin is deleted).
- Admin POSTs are inside the session + CSRF layer (browser surface — `_csrf` applies; NOT the machine-API CSRF-exempt path). **Zero new crates.**

## How it was built (DELIVER)

6 DES-monitored TDD steps across 4 slices, each a `@real-io` cucumber scenario driven green:

| Step | Outcome |
|------|---------|
| 01-01 | signer retained-in-AppState (never logged) + `created_by` wired + `mint_token` (authz/TTL/scope/sign-then-persist) |
| 01-02 | `/admin/tokens` mint route + one-time-display screen (walking skeleton — minted token authenticates against real `/api/v1`) |
| 02-01 | `list_tokens` + the index list (rows/status/empty/workspace-scoped/no-value) |
| 03-01 | `revoke_token` kill-switch (revoke → next `/api/v1` 401 via the shipped denylist; non-enumerable, idempotent) |
| 04-01 | admin-only non-enumerable authz + browser CSRF + scope/TTL surfacing + signer-absent graceful |
| 04-02 | `created_by` attribution + last-used in the list |

Then: scaffold-comment cleanup; **security-focused adversarial review** (Sonnet) → 0 blockers, 3 high + 2 low, all fixed test-first (the headline: `created_by` was written but never read back — `list_tokens` resolved "minted by" from the token's *subject*, not the *issuer*; fixed + proven with a subject≠issuer seed); **mutation** (`tokens.rs` **100% kill**, 15/15 viable, 3 tests added killing real authz/correctness survivors).

## Quality at ship

- **Acceptance** (`@all`): 215 scenarios / 1752 steps green — all us-mt0* plus the entire existing suite.
- **Build/lint**: `cargo build --workspace --tests`, full-workspace `cargo fmt --all --check`, `cargo clippy --all-targets --release -- -D warnings` all clean.
- **Mutation**: `foundry_services::tokens` 100% (the security core); admin authz, TTL bounds, workspace isolation, status derivation all pinned.
- **Security review strengths**: key SecretString + Debug-omitted + retained-only-after-self-test; one-time secret never persisted/logged; defense-in-depth fail-closed non-enumerable authz; cross-workspace non-enumerable; CSRF correct; server-side TTL; SQL params; real EdDSA round-trip; sign-before-persist.

## Residuals / follow-ups

- **JSON token-management API** — deferred (web-UI-first per OD3/Q6); a natural fast-follow. **SHIPPED 2026-06-08** as the `token-management-api` feature (list + revoke under `/api/v1`, no mint) — see `docs/evolution/2026-06-08-token-management-api.md`.
- **Single-workspace schema** (`uniq_one_workspace`) means cross-workspace evil-user paths are tested with synthetic uuids; the CODE enforces workspace scoping + non-enumerability, but real two-workspace fixtures await multi-workspace support (UI-1, `distill/upstream-issues.md`).
- **Signing-key heap residual**: the transient `env::var` String + `.replace()` allocation aren't zeroized (accepted residual, documented in `design/signer.md`; no logging/persistence).
- **Key rotation UX** — out of scope (env-var overlapping-key rotation from Feature A still applies operationally).

## Pointers
- Spec: `docs/feature/machine-token-admin-ux/{discuss,design,distill}/`
- DES roadmap + log + mutation report: `docs/feature/machine-token-admin-ux/deliver/`
- Core: `crates/foundry-services/src/tokens.rs`, `crates/foundry-app/src/admin_tokens.rs`, `templates/token_*.html`, `crates/foundry-store/migrations/0008_machine_tokens_created_by.sql`
