# Evolution — web-tier-extraction (Feature A: "Programmatic Foundry")

**Finalized**: 2026-05-31
**Ship commit**: [23bd492](../../) — "test(web-tier-extraction): kill mutation survivors in foundry-auth verify path" (tip of a 12-commit feature branch off `b3312bc`)
**Wave coverage**: full nWave pipeline — DISCUSS → DESIGN → DISTILL → DELIVER (DISCOVER/DEVOPS not run; legacy per-feature doc layout, `docs/product/` SSOT intentionally not adopted).

## Feature summary

Adds a **first-class JSON read+write API** at `/api/v1`, authenticated by a **machine token (JWT/Ed25519)**, so external agents and integrations can drive Foundry programmatically — without disturbing the existing server-rendered htmx UI or the "one binary, one Postgres, `docker compose up`" promise.

The API and the existing HTML handlers are **peer driving adapters** over a new shared, presentation-neutral service seam. A CI-enforced boundary guard keeps the two tiers honest.

This was **Feature A** of a ratified split. The original request ("build the htmx frontend, cleanly separated from the API backend") was decomposed during DISCUSS/DESIGN into:
- **Feature A — Programmatic Foundry** (this delivery): the JSON API + machine-token auth + the web/api/core separation + boundary guard.
- **Feature B — "Foundry looks like a product"** (deferred): the htmx web-tier templating/asset build-out + htmx 2 migration. To be started later via `/nw:new`, reusing the neutral core seam Feature A proves.

The driver was corrected mid-DISCUSS: the headline outcome is the **JSON API** (not "restyle without touching Rust"), confirmed by the user (DISCUSS D7).

## Architecture delivered

```
foundry (one binary)
├── foundry-api      NEW  — JSON /api/v1 adapter; depends ONLY on foundry-services + foundry-auth
├── foundry-app           — existing HTML/htmx adapter (handlers rewired to the seam)
├── foundry-services NEW  — shared orchestration seam (Services handle wrapping Arc<Store>)
├── foundry-auth          — + MachineToken EdDSA mint/verify (additive to argon2/sessions)
├── foundry-store         — + 0007 machine_tokens jti registry/denylist + probe
└── foundry-core          — unchanged (already presentation-neutral)
```

Dependency direction (CI-enforced, acyclic): `foundry-api`/`foundry-app` → `foundry-services` → `foundry-store` → `foundry-core`. Adapters never touch the store directly.

### Key decisions (ratified with the user)

- **One binary, not two services** (DISCUSS D1): the separation is a crate/module boundary, in-process, no network hop — preserves the brand promise.
- **`foundry-services` crate for the seam** (DESIGN ADR-W07): folding the orchestration into `foundry-core` would force `foundry-core → foundry-store`, a cycle (store already depends on core). A standalone crate above the store is the only acyclic home two adapters can share.
- **JWT / Ed25519, opaque-token rejected** (DESIGN ADR-W02, user override of the architect's opaque+SHA-256 recommendation): standards-based asymmetric bearer credential. Revocation via a `jti` denylist (stateless JWT can't self-revoke). Keys in env like `SESSION_SECRET`; overlapping-key rotation; `alg=[EdDSA]` pinned (`alg:none` + HS256-confusion rejected); `iss`/`aud`/`exp`/`nbf` validated; key-material startup probe refuses boot on bad keys.
- **`/api/v1` path prefix** (ADR-W03): the only contract shape that makes "api handlers never emit HTML" structurally checkable by module path.
- **3-layer boundary guard** (`cargo xtask check-arch`, ADR-W06): (1) AST walk — api≠HTML, api≠ad-hoc-authz, JWT alg pinned; (2) `cargo-deny` crate-graph ban `foundry-api ⊀ foundry-store`; (3) injected-violation gold test proving the guard bites. Wired into `xtask ci` + the CI lint-format job (no DB).
- **One net-new runtime dependency**: `jsonwebtoken = "9"` on the `ring` backend already in `Cargo.lock`.

## How it was built (DELIVER)

8 DES-monitored TDD steps across 4 phases, each a `@real-io` cucumber scenario driven to green:

| Step | Outcome |
|------|---------|
| 01-01 | `foundry_services::board` read seam; HTML board handler rewired (byte-identical) |
| 01-02 | `GET /api/v1/.../issues` → JSON; mounted in `build_router` (walking skeleton) |
| 02-01 | `0007_machine_tokens` migration + jti denylist repo + probe |
| 02-02 | `foundry_auth::MachineToken` EdDSA mint/verify + AppState verifier + key probe |
| 02-03 | `token_auth` bearer extractor (verify + denylist + scope), CSRF-exempt mount |
| 03-01 | issue/comment write use-cases extracted to the seam; HTML handlers rewired |
| 03-02 | `foundry-api` POST/PATCH routes + JSON error envelope (write rule-parity) |
| 04-01 | `cargo xtask check-arch` guard; foundry-api made store-free; cargo-deny bans |

Then: rustfmt fixup; Phase-4 adversarial review (Sonnet) → 0 blockers, 4 high fixed test-first (iss/aud + nbf validation, trimmed-title contract, deferred `created_by` documented); Phase-5 mutation (foundry-auth security core **81.8%**, gate ≥80% met, 2 survivor-killing tests added).

## Quality at ship

- **Acceptance**: 135/135 default-lane scenarios green (24 Feature-A: us-w05a read ×4, us-w05b auth ×10, us-w05c writes ×6, us-w06 guard ×4). Browser session/CSRF path byte-for-byte unchanged.
- **Guard**: `cargo xtask check-arch` passes clean; the injected-violation test confirms it bites.
- **Build/lint**: `cargo build --workspace --tests`, `cargo fmt --all --check`, `cargo clippy --workspace --all-targets`, `cargo deny check` all clean.
- **Mutation**: foundry-auth 81.8% (PASS); foundry-api pure auth fns 4/4 viable killed. Postgres-backed surfaces (store repo, services use-cases, api handlers) excluded from the mutation pass for runtime cost — covered by the `@real-io` acceptance suite, logged in `deliver/mutation/mutation-report.md`.
- **Security review strengths**: triple-layer alg-confusion defense, non-enumerable 401 refusal catalogue, correct revocation (flag-not-delete), structural write rule-parity (API reuses the same core+outbox as the UI), test signing key gated behind `#[cfg(feature="test-support")]` (never ships).

## Residuals / follow-ups

- **`foundry-app → foundry-store` ban not yet active**: foundry-app is the binary root and still holds the HTML handlers (30 `Store` refs) until **Feature B** extracts `foundry-web`. The `foundry-api ⊀ foundry-store` ban — the Feature-A payoff — is fully real and enforced. Enforcing the app-side ban now would require either weakening the rule (rejected) or the out-of-scope Feature B extraction.
- **`created_by` audit column** deferred to the future token-issuance feature (no issuer call-site exists in Feature A; documented in `design/auth.md` §Storage rather than shipping a `NOT NULL` column nothing populates).
- **Token issuance UX** (admin mints/revokes tokens) is out of Feature-A scope — tokens are minted via env/test keys today. A natural next feature.
- **6 US-03 `@needs-pgclient` backup scenarios** fail locally on a `pg_dump` v14-vs-server-v16 mismatch (homebrew client); they pass in CI (pg16 client). Pre-existing, untouched.

## Pointers

- Spec: `docs/feature/web-tier-extraction/discuss/`, `design/`, `distill/`
- DES roadmap + execution log: `docs/feature/web-tier-extraction/deliver/`
- Mutation report: `docs/feature/web-tier-extraction/deliver/mutation/mutation-report.md`
- Boundary guard: `xtask/src/check_arch.rs`, `deny.toml`
