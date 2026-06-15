# Evolution — invite-accept-flow (the public `/invites/accept` credential-establishment vertical)

**Finalized**: 2026-06-14
**DELIVER commits**: `770626d` (01-01) → `bfb7e79` (03-04) — the 18 DES-monitored TDD steps committed directly to `main` (trunk-based, no PRs) — plus the review-remediation `b8b9b06` (findings D1–D3).
**Wave coverage**: requirements COMPLETE (DISCUSS DoR 27/27, SSOT); DESIGN ratified here (D1–D7, 5 ADRs, OD-1..5 ratified by user 2026-06-14); DISTILL authored the 18 invite-accept scenarios; DELIVER shipped 18 steps across 3 phases (integrity exit 0). Legacy per-feature layout (`docs/feature/invite-accept-flow/`).
**Scope**: this is the deferred `/invites/accept` vertical that `web-provisioning-flow` ADR-005 / D5 ratified OUT of its v1 (the emitted first-admin invite link was a DEAD URL on BOTH the CLI and web surfaces). This feature makes the link LIVE. The feature directory is PRESERVED (same policy as the parents).

## Milestone — the gating gap is CLOSED

This feature **closes the gating gap flagged across the two prior features**. The CLI-first `multi-workspace-provisioning` made tenants provisionable and `web-provisioning-flow` gave super-admins a browser path to provision — but on BOTH surfaces the first-admin invite link was informational/dead: a provisioned or invited first-admin could not actually sign in. `/invites/accept` was a dead URL.

It is no longer. A provisioned workspace's first-admin can now verify their signed invite token, set a password, be consumed single-use, and land auto-signed-in on their workspace. **This completes the multi-workspace provisioning arc**: tenancy (isolation core) → provisioning (CLI surface) → web-provisioning (browser surface) → **invite-accept (credential establishment)**. The link the prior two features emitted now resolves to a working sign-in.

First-admins only in v1; general workspace-member invite-accept is deferred (see below).

## What shipped

- **`GET /invites/accept`** — public (signed-out-accessible) route: verifies the `InviteToken` signature + expiry (HMAC, no DB) plus an advisory DB-liveness check, mints a CSRF cookie, and renders a set-password form naming the target workspace. Strictly non-committal — NO mutation.
- **`POST /invites/accept`** — public route under the shipped session + double-submit CSRF layers: re-verifies the token (defense-in-depth, rejects tampered URLs before any DB hit), runs `check_password_policy` (min-12), then ONE atomic transaction — the guarded-UPDATE consume `set_first_admin_password_and_consume` (reusing the SHIPPED `invites.used_at` / `used_by` columns + an argon2id password write onto the first-admin user), establishes a session, and `303 SEE_OTHER` → the workspace.
- **`crates/foundry-auth::check_password_policy`** — net-new, tiny, length-first (min-12, NIST 800-63B, no composition rule), co-located beside `hash_password` so a future app-wide rollout (sign-up, reset, bootstrap) imports the SAME check. **Foundry had no password-strength enforcement before this** — this is its first.
- **`crates/foundry-store`** — `invite_accept_view` (the GET read) + `set_first_admin_password_and_consume` (the atomic guarded consume + password write, TOCTOU-safe, mirroring the shipped `claim_bootstrap_token` idiom).
- Routes wired on the PUBLIC layer (under session + CSRF, NOT behind the instance-admin gate); landing resolved via the shipped `resolve_active_workspace` seam, exactly as `signin.rs` does.
- **ZERO migration. ZERO new crate. No new check-arch LAYER-1e line** — the handler uses the resolution seam (like `signin`, already allow-listed), so D7's provisional "no line" verdict held against the real `cargo xtask check-arch` run.

## Security — the crux

- **Non-enumerable refusal**: byte-identical 200-OK refusal body AND status across expired / used / tampered / unknown. The refusal upgraded the shipped 404 page to a uniform `invite_refusal_page()` carrying the journey's "no longer valid… ask your instance administrator to re-issue" copy. Reasons differ ONLY in `tracing` keyed on `invite_id`, never in the response. This deliberately DIVERGES from `bootstrap.rs:124-139`, which leaks distinct Used/Expired/Unknown messages (recorded as a security follow-up, see below).
- **Single-use under concurrency**: enforced by the atomic guarded UPDATE (`SET used_at = now(), used_by = $user WHERE id = $1 AND used_at IS NULL AND expires_at > now() RETURNING …`) — proven exactly-once under concurrent POSTs (step 02-07) and TOCTOU-safe between GET and POST (step 02-10); the GET liveness check is advisory, the TX guard is the sole authority.
- **CSRF** on the public POST via the shipped double-submit middleware (a token-less POST is refused before any consume or password write — step 02-08).
- **Token signature bound to the DB-stored expiry** — URL-tamper cannot extend an invite's lifetime.
- **No leakage**: no token signature and no password ever appears in logs or responses (step 02-09).
- **Recoverability**: password + confirm checks run BEFORE the consume opens — a weak or mismatched password re-renders inline with the invite STILL LIVE for retry (steps 03-01..03-03).

## Decisions realized (D1–D7)

| # | Decision | Status |
|---|---|---|
| **D1** | NO migration. Reuse shipped `invites.used_at` / `used_by` as the single-use marker; new guarded-UPDATE consume mirroring `claim_bootstrap_token`. | **IMPLEMENTED** |
| **D2** | ONE TX = password write + consume (`set_first_admin_password_and_consume`); neither effect without the other; first-admin user_id == `invites.created_by` (the consume's `RETURNING` names the row). | **IMPLEMENTED** |
| **D3** | Non-enumerable refusal = all four reasons collapse to ONE byte-identical `invite_refusal_page()` at a single fixed status (200 OK); reasons differ only in `tracing` on `invite_id`. | **IMPLEMENTED** |
| **D4** | Public-POST CSRF = mint a CSRF cookie on the GET accept page (reuse `ensure_csrf_cookie`); POST mounts under the shipped `csrf_middleware`; no new middleware, no exemption. | **IMPLEMENTED** |
| **D5** | `check_password_policy` (min-12, length-first NIST) in `foundry-auth` beside `hash_password`; applied BEFORE the consume; a violation re-renders the form with the invite UNTOUCHED. | **IMPLEMENTED** |
| **D6** | GET token-verify (non-committal, advisory liveness) vs POST re-verify + policy + TOCTOU-safe consume TX; expiry HMAC-bound and re-checked inside the guarded UPDATE. | **IMPLEMENTED** |
| **D7** | LAYER-1e allow-list = NO new line (uses the `resolve_active_workspace` seam like `signin`). Confirmed against the real `check-arch` run. | **IMPLEMENTED** (no line needed) |

OD-1..OD-5 were ratified by the user 2026-06-14 before DISTILL (no migration; min-12; 200-OK refusal; no allow-list line; OD-4 doc-promote deferred).

## How it was built (DELIVER) — the 18-step TDD arc

**18 DES-monitored TDD steps across 3 phases**, each driven by `@real-io` cucumber scenarios over the real surfaces (real axum router, real session + CSRF layers, real testcontainers PG16), every step running all 5 DES phases (integrity exit 0).

| Phase | Steps | What it proved |
|---|---|---|
| **01 — accept + auto-sign-in walking skeleton (US-01, D1/D2/D6)** | 01-01.. | first-admin verifies a signed invite, sets a password, is consumed, and lands signed in on their workspace end-to-end through the real session + CSRF layers + the atomic consume TX; the landed workspace == the invite's workspace via `resolve_active_workspace`. |
| **02 — refuse-invalid-safely + non-enumerability + concurrency (US-02, security)** | 02-04..02-10 | expired / used / tampered / unknown all refuse byte-identically (consolidated invariant + revert-reds-it litmus); a consumed invite is never reusable; concurrent accepts of one invite succeed exactly once; token-less POST refused by the shipped CSRF middleware; no sig/password in logs; a link consumed in the GET→POST window is refused by the TX guard (TOCTOU). |
| **03 — password-mistake recovery (US-03, D5)** | 03-01..03-04 | a weak password corrected inline with the invite left LIVE; a mismatched confirmation corrected inline, invite stays live; a valid retry on the SAME invite completes; a password exactly at the minimum length is accepted (boundary). |

## Quality at ship

`cargo xtask ci` — **ALL GATES GREEN**:

- **fmt**: `cargo fmt --all --check` clean.
- **clippy**: `cargo clippy --all-targets --release -- -D warnings` clean.
- **`cargo xtask check-arch`**: PASSED (D7 confirmed — no new LAYER-1e allow-list line).
- **`@all` acceptance**: **303 scenarios / 2460 steps** green (parent suites plus this feature's 18 new invite-accept scenarios; green-before stays green-after).
- **Adversarial review**: **APPROVED** — the reviewer called it "the strongest acceptance suite I have reviewed"; **Testing Theater: none found** (the per-step falsifiability litmus proofs were verified to bind to production code).
- **Scoped mutation testing**: **100% (4/4)** on `check_password_policy` (the net-new pure logic). The consume guard + handlers are covered by the acceptance suite + per-step falsifiability proofs + the detailed adversarial review.
- **Review findings D1–D3**: ALREADY FIXED in `b8b9b06` — done, not outstanding:
  - **D1** — documented the no-leak scenario's form-body sig-scan exclusion (scope clarification).
  - **D2** — extracted `submit_accept` into 3 named security helpers (behavior-preserving).
  - **D3** — deleted the dead `consume_invite` store method (per the AGENTS.md pre-stable dead-code policy).
- **Zero new crates. Zero migration.**

One known test-infra flake (NOT a feature defect): concurrent testcontainers can hit a transient sqlx `unknown message type` warm-up error; passes on a clean re-run.

## Deferred / follow-ups

**General workspace-member invite-accept** — v1 is first-admins only. Extending the flow to general member invites is the natural next increment.

**Security follow-up (carried from DESIGN `upstream-changes.md`)** — the shipped **bootstrap claim flow** (`bootstrap.rs:124-139`) returns DISTINCT expired / used / not-found messages — an enumeration oracle that this feature's invite-accept flow deliberately does NOT replicate. Worth closing in the bootstrap flow (OUT of scope here; bootstrap NOT modified).

**Carried from prior features:**
- The deferred `web-provisioning-flow` follow-ups.
- Prometheus exporter for `foundry_token_mutations_total`.
- Per-workspace backup/restore (OD-5) — whole-instance backup unchanged.
- Key-rotation UX.
- A nightly/follow-up scoped mutation pass on the web adapter.

## Pointers

- Spec (preserved): `docs/feature/invite-accept-flow/{discuss,design,distill,deliver}/` — notably `design/wave-decisions.md` (D1–D7 + OD-1..5 ratification), the 5 ADRs (`adr-001..005`), `design/upstream-changes.md` (the `used_at`-already-shipped correction + the bootstrap enumeration-oracle finding), and the DISTILL scenarios.
- DES roadmap + execution log (the audit trail, preserved): `docs/feature/invite-accept-flow/deliver/roadmap.json` (3 phases / 18 steps) + `execution-log.json` (DES-verify-integrity clean) + `.develop-progress.json`.
- Core production files:
  - Web adapter (NEW): `crates/foundry-app/src/invites_accept.rs` (`show_accept_form` GET + `submit_accept` POST, refactored into 3 named security helpers) + its Askama templates (set-password + the uniform refusal).
  - Route registration: `crates/foundry-app/src/lib.rs` (`build_router` — the two new PUBLIC `/invites/accept` routes).
  - Store seam (NEW): `crates/foundry-store/src/lib.rs` (`invite_accept_view` + `set_first_admin_password_and_consume`).
  - Password policy (NEW): `crates/foundry-auth/src/lib.rs` (`check_password_policy`, min-12).
  - Reused verbatim (shipped): `InviteToken::verify`, `hash_password`, `resolve_active_workspace`, the session + CSRF layers, the `claim_bootstrap_token` consume idiom, the `resource_not_found_page` refusal shape.
  - Acceptance: the invite-accept feature file + step defs in `crates/foundry-acceptance/`.
- Predecessors (the arc this completes): `docs/evolution/2026-06-13-web-provisioning-flow.md` (the browser provisioning surface whose D5 deferred this), `docs/evolution/2026-06-12-multi-workspace-provisioning.md` (the CLI-first surface), and `docs/evolution/2026-06-11-multi-workspace-tenancy.md` (the isolation core).
</content>
</invoke>
